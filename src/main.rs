// CodinGame "Snake" (catch the rabbits) solver.
//
// Scoring (from the referee source): each catch at turn t gives
//   10000 + combo_bonus - penalty
//   combo: if t - last_catch <= 2 then combo += 1 else combo = 1; bonus = 15000 * combo if combo > 1
//   penalty: t * (t - last_catch) if a previous catch exists and t - last_catch > 10
//
// Strategy:
//   * Simulated annealing over the order of the remaining rabbits, scored with the exact
//     scoring formula on (corrected) Manhattan distances. The score of an order is the best
//     prefix: rabbits after the truncation point are simply never caught (late long hops cost
//     more than the 10000 they bring).
//   * Each turn, random rollouts of a real snake simulation follow the first targets of the
//     order; the best concrete move sequence (exact score + estimate of the rest) is kept as
//     a plan across turns and its first move is played.
//
// Single file so it can be pasted directly into CodinGame.

use std::collections::VecDeque;
use std::io::{self, BufRead, Write};
use std::time::Instant;

const W: i32 = 96;
const H: i32 = 54;
const NC: usize = (W * H) as usize;
const MAX_TURN: i32 = 600;
const NO_CATCH: i32 = -10000;
const INF: i32 = i32::MAX / 4;

// Time budgets (ms), multiplied by SNAKE_SCALE (env var, local benchmarking only).
const FIRST_TURN_MS: f64 = 850.0;
const TURN_MS: f64 = 35.0;
// Number of order targets a rollout follows.
const ROLLOUT_TARGETS: usize = 3;

// ---------------------------------------------------------------- utils

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        (((self.next() >> 32) * n as u64) >> 32) as usize
    }
    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
}

#[inline]
fn cell(x: i32, y: i32) -> u16 {
    (y * W + x) as u16
}

#[inline]
fn xy(c: u16) -> (i32, i32) {
    (c as i32 % W, c as i32 / W)
}

#[inline]
fn manh(a: (i32, i32), b: (i32, i32)) -> i32 {
    (a.0 - b.0).abs() + (a.1 - b.1).abs()
}

/// Up to 4 in-bound neighbours of a cell.
#[inline]
fn neighbors(c: u16) -> ([u16; 4], usize) {
    let (x, y) = xy(c);
    let mut out = [0u16; 4];
    let mut n = 0;
    if x + 1 < W {
        out[n] = c + 1;
        n += 1;
    }
    if x > 0 {
        out[n] = c - 1;
        n += 1;
    }
    if y + 1 < H {
        out[n] = c + W as u16;
        n += 1;
    }
    if y > 0 {
        out[n] = c - W as u16;
        n += 1;
    }
    (out, n)
}

/// Extra cost of hop p -> c when we arrived at p coming from pp and c lies straight behind
/// on the same line: the snake cannot reverse and must go around (+2).
#[inline]
fn turn_back_cost(pp: (i32, i32), p: (i32, i32), c: (i32, i32)) -> i32 {
    if pp.1 == p.1 && c.1 == p.1 && c.0 != p.0 && (pp.0 - p.0).signum() == (c.0 - p.0).signum() {
        return 2;
    }
    if pp.0 == p.0 && c.0 == p.0 && c.1 != p.1 && (pp.1 - p.1).signum() == (c.1 - p.1).signum() {
        return 2;
    }
    0
}

/// Score gained by a catch at turn `t`; returns (gain, new combo).
#[inline]
fn catch_gain(t: i32, last: i32, combo: i32) -> (i64, i32) {
    let gap = t - last;
    let combo = if gap <= 2 { combo + 1 } else { 1 };
    let add = if combo > 1 { 15000 * combo } else { 0 };
    let pen = if last != NO_CATCH && gap > 10 { t * gap } else { 0 };
    ((10000 + add - pen) as i64, combo)
}

#[derive(Clone, Copy)]
struct Bits([u64; 4]);

impl Bits {
    #[inline]
    fn get(&self, i: usize) -> bool {
        self.0[i >> 6] >> (i & 63) & 1 == 1
    }
    #[inline]
    fn set(&mut self, i: usize, v: bool) {
        if v {
            self.0[i >> 6] |= 1 << (i & 63);
        } else {
            self.0[i >> 6] &= !(1 << (i & 63));
        }
    }
}

// ---------------------------------------------------------------- world

struct World {
    rpos: Vec<(i32, i32)>,
    rab_at: Vec<i16>,
}

#[derive(Clone)]
struct Sim {
    body: VecDeque<u16>, // front = head
    occ: Vec<bool>,
    alive: Bits,
    t: i32, // number of moves done (= referee turn of the last move)
    last: i32,
    combo: i32,
    score: i64,
}

impl Sim {
    #[inline]
    fn head(&self) -> u16 {
        self.body[0]
    }

    #[inline]
    fn neck(&self) -> u16 {
        if self.body.len() > 1 { self.body[1] } else { self.body[0] }
    }

    #[inline]
    fn rabbit_at(&self, w: &World, c: u16) -> Option<usize> {
        let r = w.rab_at[c as usize];
        if r >= 0 && self.alive.get(r as usize) { Some(r as usize) } else { None }
    }

    #[inline]
    fn can_move(&self, w: &World, c: u16) -> bool {
        if !self.occ[c as usize] {
            return true;
        }
        // the tail cell is freed before the collision check, unless we grow this turn
        c == *self.body.back().unwrap() && self.rabbit_at(w, c).is_none()
    }

    fn legal_moves(&self, w: &World) -> ([u16; 4], usize) {
        let (nb, n) = neighbors(self.head());
        let mut out = [0u16; 4];
        let mut k = 0;
        for &c in &nb[..n] {
            if self.can_move(w, c) {
                out[k] = c;
                k += 1;
            }
        }
        (out, k)
    }

    /// Applies a legal move. Returns the caught rabbit, if any.
    fn step(&mut self, w: &World, c: u16) -> Option<usize> {
        self.t += 1;
        let r = self.rabbit_at(w, c);
        if let Some(r) = r {
            let (g, combo) = catch_gain(self.t, self.last, self.combo);
            self.score += g;
            self.combo = combo;
            self.last = self.t;
            self.alive.set(r, false);
        } else {
            let tail = self.body.pop_back().unwrap();
            self.occ[tail as usize] = false;
        }
        self.body.push_front(c);
        self.occ[c as usize] = true;
        r
    }
}

/// Scratch buffers for BFS.
struct Bfs {
    dist: Vec<i32>,
    rel: Vec<i32>,
    par: Vec<u16>,
    queue: Vec<u16>,
}

impl Bfs {
    fn new() -> Self {
        Bfs { dist: vec![INF; NC], rel: vec![0; NC], par: vec![0; NC], queue: Vec::with_capacity(NC) }
    }

    /// Time-aware BFS from the head: a body cell is enterable once the tail has passed it.
    /// Returns the number of reached cells. Stops early once `stop` is reached (if given).
    fn run(&mut self, sim: &Sim, stop: Option<u16>) -> usize {
        self.dist.iter_mut().for_each(|d| *d = INF);
        let l = sim.body.len() as i32;
        for (j, &c) in sim.body.iter().enumerate() {
            self.rel[c as usize] = l - j as i32;
        }
        let h = sim.head();
        self.queue.clear();
        self.queue.push(h);
        self.dist[h as usize] = 0;
        let mut qi = 0;
        while qi < self.queue.len() {
            let c = self.queue[qi];
            qi += 1;
            if Some(c) == stop {
                break;
            }
            let d = self.dist[c as usize] + 1;
            let (nb, n) = neighbors(c);
            for &nc in &nb[..n] {
                if self.dist[nc as usize] == INF && d >= self.rel[nc as usize] {
                    self.dist[nc as usize] = d;
                    self.par[nc as usize] = c;
                    self.queue.push(nc);
                }
            }
        }
        for &c in sim.body.iter() {
            self.rel[c as usize] = 0;
        }
        self.queue.len()
    }

    /// Shortest path to `target` as a stack (last element = first move), if reachable.
    fn path(&mut self, sim: &Sim, target: u16, out: &mut Vec<u16>) -> bool {
        out.clear();
        self.run(sim, Some(target));
        if self.dist[target as usize] == INF || target == sim.head() {
            return false;
        }
        let mut c = target;
        while c != sim.head() {
            out.push(c);
            c = self.par[c as usize];
        }
        true
    }
}

// ---------------------------------------------------------------- order optimisation

#[derive(Clone, Copy)]
struct Base {
    t: i32,
    last: i32,
    combo: i32,
    head: (i32, i32),
    neck: (i32, i32),
}

impl Base {
    fn of(sim: &Sim) -> Base {
        Base { t: sim.t, last: sim.last, combo: sim.combo, head: xy(sim.head()), neck: xy(sim.neck()) }
    }
}

#[derive(Clone)]
struct Cache {
    t: Vec<i32>,
    combo: Vec<i32>,
    s: Vec<i64>,
    best: Vec<i64>,
    blen: Vec<u16>,
}

impl Cache {
    fn new(n: usize) -> Self {
        Cache { t: vec![0; n], combo: vec![0; n], s: vec![0; n], best: vec![0; n], blen: vec![0; n] }
    }
    fn copy_from(&mut self, o: &Cache, from: usize, n: usize) {
        self.t[from..n].copy_from_slice(&o.t[from..n]);
        self.combo[from..n].copy_from_slice(&o.combo[from..n]);
        self.s[from..n].copy_from_slice(&o.s[from..n]);
        self.best[from..n].copy_from_slice(&o.best[from..n]);
        self.blen[from..n].copy_from_slice(&o.blen[from..n]);
    }
}

/// Estimated gain of catching `order` (best prefix) from `b`. `first_d` optionally gives the
/// real distance from the head to each rabbit. Returns (gain, best prefix length).
fn eval_order(w: &World, b: &Base, order: &[u8], first_d: Option<&[i32]>) -> (i64, usize) {
    let (mut t, mut last, mut combo, mut s) = (b.t, b.last, b.combo, 0i64);
    let (mut best, mut blen) = (0i64, 0usize);
    let (mut pp, mut p) = (b.neck, b.head);
    for (k, &r) in order.iter().enumerate() {
        let c = w.rpos[r as usize];
        let d = match (k, first_d) {
            (0, Some(fd)) => fd[r as usize],
            _ => manh(p, c) + turn_back_cost(pp, p, c),
        };
        t += d;
        if t > MAX_TURN {
            break;
        }
        let (g, nc) = catch_gain(t, last, combo);
        s += g;
        combo = nc;
        last = t;
        if s > best {
            best = s;
            blen = k + 1;
        }
        pp = p;
        p = c;
    }
    (best, blen)
}

struct Planner {
    order: Vec<u8>,
    cur: Cache,
    work: Cache,
    first_d: Vec<i32>,
    scratch: Vec<u8>,
}

impl Planner {
    fn eval_from(&mut self, w: &World, b: &Base, from: usize) -> (i64, usize) {
        let n = self.order.len();
        let (mut t, mut last, mut combo, mut s, mut best, mut blen) = if from == 0 {
            (b.t, b.last, b.combo, 0i64, 0i64, 0usize)
        } else {
            let k = from - 1;
            let tt = self.cur.t[k];
            (tt, tt, self.cur.combo[k], self.cur.s[k], self.cur.best[k], self.cur.blen[k] as usize)
        };
        let o = &self.order;
        let wk = &mut self.work;
        let mut k = from;
        while k < n {
            if t > MAX_TURN {
                break;
            }
            let c = w.rpos[o[k] as usize];
            let d = if k == 0 {
                self.first_d[o[0] as usize]
            } else {
                let p = w.rpos[o[k - 1] as usize];
                let pp = if k == 1 { b.head } else { w.rpos[o[k - 2] as usize] };
                manh(p, c) + turn_back_cost(pp, p, c)
            };
            t += d;
            if t > MAX_TURN {
                break;
            }
            let (g, nc) = catch_gain(t, last, combo);
            s += g;
            combo = nc;
            last = t;
            if s > best {
                best = s;
                blen = k + 1;
            }
            wk.t[k] = t;
            wk.combo[k] = combo;
            wk.s[k] = s;
            wk.best[k] = best;
            wk.blen[k] = blen as u16;
            k += 1;
        }
        // beyond the turn limit: nothing more is caught
        while k < n {
            wk.t[k] = INF;
            wk.combo[k] = combo;
            wk.s[k] = s;
            wk.best[k] = best;
            wk.blen[k] = blen as u16;
            k += 1;
        }
        (best, blen)
    }

    fn full_eval(&mut self, w: &World, b: &Base) -> (i64, usize) {
        let n = self.order.len();
        if self.cur.t.len() < n {
            self.cur = Cache::new(n);
            self.work = Cache::new(n);
        }
        let r = self.eval_from(w, b, 0);
        self.cur.copy_from(&self.work, 0, n);
        r
    }

    /// Simulated annealing on the order until `deadline`.
    fn anneal(&mut self, w: &World, b: &Base, rng: &mut Rng, start: Instant, deadline: f64, t0: f64, t1: f64) -> (i64, usize) {
        let n = self.order.len();
        let (mut cur_val, mut cur_len) = self.full_eval(w, b);
        if n < 2 {
            return (cur_val, cur_len);
        }
        let mut best_val = cur_val;
        let mut best_len = cur_len;
        let mut best_order = self.order.clone();
        let t_begin = start.elapsed().as_secs_f64() * 1000.0;
        let span = (deadline - t_begin).max(1e-3);
        let mut temp = t0;
        let mut it: u64 = 0;
        loop {
            if it & 127 == 0 {
                let now = start.elapsed().as_secs_f64() * 1000.0;
                if now >= deadline {
                    break;
                }
                let frac = (now - t_begin) / span;
                temp = t0 * (t1 / t0).powf(frac);
            }
            it += 1;
            // moves mostly act on the useful part of the order
            let m = if rng.below(4) == 0 { n } else { (cur_len + 6).min(n).max(2) };
            let typ = rng.below(10);
            let (lo, hi);
            if typ < 4 {
                let i = rng.below(m);
                let j = rng.below(m);
                if i == j {
                    continue;
                }
                lo = i.min(j);
                hi = i.max(j);
                self.scratch.clear();
                self.scratch.extend_from_slice(&self.order[lo..=hi]);
                self.order[lo..=hi].reverse();
            } else if typ < 8 {
                let len = 1 + rng.below(3.min(m - 1));
                let a = rng.below(m - len + 1);
                let p = rng.below(n - len + 1);
                if p == a {
                    continue;
                }
                if p > a {
                    lo = a;
                    hi = p + len - 1;
                    self.scratch.clear();
                    self.scratch.extend_from_slice(&self.order[lo..=hi]);
                    self.order[a..p + len].rotate_left(len);
                } else {
                    lo = p;
                    hi = a + len - 1;
                    self.scratch.clear();
                    self.scratch.extend_from_slice(&self.order[lo..=hi]);
                    self.order[p..a + len].rotate_right(len);
                }
                if len > 1 && rng.below(2) == 0 {
                    self.order[p..p + len].reverse();
                }
            } else {
                let i = rng.below(m);
                let j = rng.below(n);
                if i == j {
                    continue;
                }
                lo = i.min(j);
                hi = i.max(j);
                self.scratch.clear();
                self.scratch.extend_from_slice(&self.order[lo..=hi]);
                self.order.swap(i, j);
            }
            let (v, l) = self.eval_from(w, b, lo);
            let delta = (v - cur_val) as f64;
            if delta >= 0.0 || rng.unit() < (delta / temp).exp() {
                cur_val = v;
                cur_len = l;
                self.cur.copy_from(&self.work, lo, n);
                if v > best_val {
                    best_val = v;
                    best_len = l;
                    best_order.copy_from_slice(&self.order);
                }
            } else {
                self.order[lo..=hi].copy_from_slice(&self.scratch);
            }
        }
        self.order = best_order;
        self.full_eval(w, b);
        (best_val, best_len)
    }
}

// ---------------------------------------------------------------- rollouts / plan

struct Solver {
    w: World,
    rng: Rng,
    bfs: Bfs,
    planner: Planner,
    plan: Vec<u16>,
    alive: Bits,
    last: i32,
    combo: i32,
    score: i64,
    t: i32,
    scale: f64,
}

/// Value of a sim state reached by a rollout: exact score so far + estimate of the rest.
fn state_value(w: &World, bfs: &mut Bfs, sim: &Sim, order: &[u8], buf: &mut Vec<u8>) -> Option<i64> {
    let need = (sim.body.len() * 2 + 20).min(NC - sim.body.len());
    if bfs.run(sim, None) < need {
        return None; // trapped
    }
    buf.clear();
    buf.extend(order.iter().copied().filter(|&r| sim.alive.get(r as usize)));
    let (rest, _) = eval_order(w, &Base::of(sim), buf, None);
    Some(sim.score + rest)
}

impl Solver {
    /// One random rollout from `sim` following `targets`. Returns the move list.
    fn rollout(&mut self, start: &Sim, targets: &[u8], explore: f64) -> Option<(Sim, Vec<u16>)> {
        let w = &self.w;
        let mut sim = start.clone();
        let mut moves = Vec::with_capacity(64);
        let mut detour = Vec::new();
        for &tg in targets {
            let tc = w.rpos[tg as usize];
            let tcell = cell(tc.0, tc.1);
            let mut steps = 0;
            let mut bfs_calls = 0;
            detour.clear();
            while sim.alive.get(tg as usize) {
                if sim.t >= MAX_TURN {
                    return Some((sim, moves));
                }
                steps += 1;
                if steps > 250 {
                    return None;
                }
                let (lm, n) = sim.legal_moves(w);
                if n == 0 {
                    return None;
                }
                let hd = manh(xy(sim.head()), tc);
                let mut good = [0u16; 4];
                let mut ng = 0;
                let mut clean = [0u16; 4];
                let mut nclean = 0;
                for &c in &lm[..n] {
                    if manh(xy(c), tc) < hd {
                        good[ng] = c;
                        ng += 1;
                        if c == tcell || sim.rabbit_at(w, c).is_none() {
                            clean[nclean] = c;
                            nclean += 1;
                        }
                    }
                }
                let follow = match detour.last() {
                    Some(&c) if lm[..n].contains(&c) => Some(c),
                    _ => None,
                };
                let mv = if let Some(c) = follow {
                    detour.pop();
                    c
                } else if ng > 0 && self.rng.unit() >= explore {
                    if nclean > 0 && self.rng.unit() < 0.8 {
                        clean[self.rng.below(nclean)]
                    } else {
                        good[self.rng.below(ng)]
                    }
                } else if ng == 0 {
                    bfs_calls += 1;
                    if bfs_calls > 3 {
                        return None;
                    }
                    if self.bfs.path(&sim, tcell, &mut detour) && lm[..n].contains(detour.last().unwrap()) {
                        detour.pop().unwrap()
                    } else {
                        detour.clear();
                        lm[self.rng.below(n)]
                    }
                } else {
                    lm[self.rng.below(n)]
                };
                sim.step(w, mv);
                moves.push(mv);
            }
        }
        Some((sim, moves))
    }

    /// Replays a move list; None if it became illegal.
    fn replay(&self, start: &Sim, moves: &[u16]) -> Option<Sim> {
        let mut sim = start.clone();
        for &m in moves {
            let (lm, n) = sim.legal_moves(&self.w);
            if !lm[..n].contains(&m) {
                return None;
            }
            sim.step(&self.w, m);
        }
        Some(sim)
    }

    /// Safe move when no rabbit is worth catching: keep away from rabbits, maximise space.
    fn wander(&mut self, sim: &Sim) -> u16 {
        let (lm, n) = sim.legal_moves(&self.w);
        if n == 0 {
            let (nb, _) = neighbors(sim.head());
            return nb[0];
        }
        let mut best = lm[0];
        let mut best_v = i64::MIN;
        let straight = {
            let (hx, hy) = xy(sim.head());
            let (nx, ny) = xy(sim.neck());
            (2 * hx - nx, 2 * hy - ny)
        };
        for &c in &lm[..n] {
            let mut s = sim.clone();
            s.step(&self.w, c);
            let space = self.bfs.run(&s, None) as i64;
            let mut v = space.min(600) * 1000;
            if sim.rabbit_at(&self.w, c).is_some() {
                v -= 10_000_000;
            }
            if xy(c) == straight {
                v += 10;
            }
            v += self.rng.below(5) as i64;
            if v > best_v {
                best_v = v;
                best = c;
            }
        }
        best
    }

    fn turn(&mut self, body: &[(i32, i32)], start: Instant) -> (i32, i32) {
        let first = self.t == 0;
        let budget = if first { FIRST_TURN_MS } else { TURN_MS } * self.scale;
        let head = cell(body[0].0, body[0].1);
        // detect a catch made by our last move
        if !first {
            let r = self.w.rab_at[head as usize];
            if r >= 0 && self.alive.get(r as usize) {
                let (g, c) = catch_gain(self.t, self.last, self.combo);
                self.score += g;
                self.combo = c;
                self.last = self.t;
                self.alive.set(r as usize, false);
            }
        }
        let mut occ = vec![false; NC];
        let mut dq = VecDeque::with_capacity(body.len() + 8);
        for &(x, y) in body {
            let c = cell(x, y);
            occ[c as usize] = true;
            dq.push_back(c);
        }
        let sim = Sim { body: dq, occ, alive: self.alive, t: self.t, last: self.last, combo: self.combo, score: self.score };

        // ---- order optimisation
        let alive = self.alive;
        self.planner.order.retain(|&r| alive.get(r as usize));
        self.bfs.run(&sim, None);
        let base = Base::of(&sim);
        let l = sim.body.len() as i32;
        for (r, &p) in self.w.rpos.iter().enumerate() {
            let c = cell(p.0, p.1);
            let d = self.bfs.dist[c as usize];
            self.planner.first_d[r] = if c == head {
                l.max(4)
            } else if d == INF {
                manh(base.head, p) + 20
            } else {
                d
            };
        }
        let (sa_ms, t0, t1) = if first {
            (budget * 0.85, env_f64("SA_T0", 8000.0), env_f64("SA_T1", 30.0))
        } else {
            (budget * 0.4, 400.0, 10.0)
        };
        let (pred, blen) = self.planner.anneal(&self.w, &base, &mut self.rng, start, sa_ms, t0, t1);
        if first || self.t % 100 == 0 {
            eprintln!("t={} score={} pred_total={} blen={} left={}", self.t, self.score, self.score + pred, blen, self.planner.order.len());
        }

        // ---- concrete plan
        let order = self.planner.order.clone();
        let targets: Vec<u8> = order[..blen.min(ROLLOUT_TARGETS)].to_vec();
        let mut buf = Vec::new();
        let mut best_plan: Option<(i64, Vec<u16>)> = None;
        if !self.plan.is_empty() {
            let plan = std::mem::take(&mut self.plan);
            if let Some(s) = self.replay(&sim, &plan) {
                if let Some(v) = state_value(&self.w, &mut self.bfs, &s, &order, &mut buf) {
                    best_plan = Some((v, plan));
                }
            }
        }
        if !targets.is_empty() {
            let mut k = 0u32;
            loop {
                if start.elapsed().as_secs_f64() * 1000.0 >= budget {
                    break;
                }
                k += 1;
                let explore = if k % 4 == 0 { 0.05 } else { 0.0 };
                if let Some((s, moves)) = self.rollout(&sim, &targets, explore) {
                    if moves.is_empty() {
                        continue;
                    }
                    if let Some(v) = state_value(&self.w, &mut self.bfs, &s, &order, &mut buf) {
                        if best_plan.as_ref().map_or(true, |(bv, _)| v > *bv) {
                            best_plan = Some((v, moves));
                        }
                    }
                }
            }
        }
        let mv = match best_plan {
            Some((_, mut plan)) if !plan.is_empty() => {
                let m = plan.remove(0);
                self.plan = plan;
                m
            }
            _ => {
                self.plan.clear();
                self.wander(&sim)
            }
        };
        self.t += 1;
        xy(mv)
    }
}

// ---------------------------------------------------------------- main

fn env_f64(k: &str, d: f64) -> f64 {
    std::env::var(k).ok().and_then(|v| v.parse().ok()).unwrap_or(d)
}

fn main() {
    let stdin = io::stdin();
    let mut lines = stdin.lock().lines();
    let mut next_line = move || -> Option<String> { lines.next().and_then(|l| l.ok()) };

    let first_line = match next_line() {
        Some(l) => l,
        None => return,
    };
    let start = Instant::now();
    let n: usize = first_line.trim().parse().unwrap();
    let mut rpos = Vec::with_capacity(n);
    for _ in 0..n {
        let l = next_line().unwrap();
        let mut it = l.split_whitespace().map(|v| v.parse::<i32>().unwrap());
        rpos.push((it.next().unwrap(), it.next().unwrap()));
    }
    let mut rab_at = vec![-1i16; NC];
    let mut alive = Bits([0; 4]);
    for (i, &(x, y)) in rpos.iter().enumerate() {
        rab_at[cell(x, y) as usize] = i as i16;
        alive.set(i, true);
    }

    let scale = std::env::var("SNAKE_SCALE").ok().and_then(|v| v.parse().ok()).unwrap_or(1.0);
    let seed = std::env::var("SNAKE_SEED").ok().and_then(|v| v.parse().ok()).unwrap_or(0x9E3779B97F4A7C15u64);

    // greedy nearest-neighbour initial order from the start head
    let mut order: Vec<u8> = Vec::with_capacity(n);
    {
        let mut used = vec![false; n];
        let mut p = (14, 10);
        for _ in 0..n {
            let mut bi = 0;
            let mut bd = INF;
            for i in 0..n {
                if !used[i] && manh(p, rpos[i]) < bd {
                    bd = manh(p, rpos[i]);
                    bi = i;
                }
            }
            used[bi] = true;
            order.push(bi as u8);
            p = rpos[bi];
        }
    }

    let mut solver = Solver {
        w: World { rpos, rab_at },
        rng: Rng(seed | 1),
        bfs: Bfs::new(),
        planner: Planner { order, cur: Cache::new(n), work: Cache::new(n), first_d: vec![0; n], scratch: Vec::new() },
        plan: Vec::new(),
        alive,
        last: NO_CATCH,
        combo: 1,
        score: 0,
        t: 0,
        scale,
    };

    let out = io::stdout();
    let mut first = true;
    loop {
        let l = match next_line() {
            Some(l) => l,
            None => break,
        };
        let t_start = if first { start } else { Instant::now() };
        first = false;
        let ns: usize = match l.trim().parse() {
            Ok(v) => v,
            Err(_) => break,
        };
        let mut body = Vec::with_capacity(ns);
        for _ in 0..ns {
            let l = next_line().unwrap();
            let mut it = l.split_whitespace().map(|v| v.parse::<i32>().unwrap());
            body.push((it.next().unwrap(), it.next().unwrap()));
        }
        let (x, y) = solver.turn(&body, t_start);
        let el = t_start.elapsed().as_secs_f64() * 1000.0;
        if el > TURN_MS * solver.scale + 3.0 && solver.t > 1 {
            eprintln!("slow turn {}: {:.1}ms", solver.t, el);
        }
        let mut o = out.lock();
        writeln!(o, "{} {}", x, y).unwrap();
        o.flush().unwrap();
    }
}
