// Local referee / benchmark: faithful port of the CodinGame "Snake" referee (Referee.java).
//
// cargo build --release
// cargo run --release --bin referee -- [--scale 0.2] [--rounds 3] [--threads 10] [--seed 1] [--bin path] [-v]
//
// Plays the static tests in testcases/ plus `rounds` rounds of random games with the
// random validator sizes, and prints an estimate of the CodinGame validator total.

use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Instant;

const W: i32 = 96;
const H: i32 = 54;
// sizes of the random validators ("Test case Random ...")
const RANDOM_SIZES: [usize; 7] = [50, 55, 62, 58, 32, 60, 70];

struct Game {
    name: String,
    rabbits: Vec<(i32, i32)>,
}

struct Outcome {
    score: i64,
    caught: usize,
    turns: i32,
    error: Option<String>,
    max_first_ms: f64,
    max_turn_ms: f64,
}

fn play(bin: &str, scale: f64, seed: u64, g: &Game) -> Outcome {
    let mut child = Command::new(bin)
        .env("SNAKE_SCALE", scale.to_string())
        .env("SNAKE_SEED", seed.to_string())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(if std::env::var("REF_STDERR").is_ok() { Stdio::inherit() } else { Stdio::null() })
        .spawn()
        .expect("cannot start solver");
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());

    let n = g.rabbits.len();
    let mut vis = vec![false; n];
    // pos[0] = tail, last = head (as in Snake.java)
    let mut pos: VecDeque<(i32, i32)> = (10..=14).map(|x| (x, 10)).collect();
    let mut length = 5usize;
    let mut lpos = (14, 10);
    let (mut score, mut combo, mut lturn) = (0i64, 1i64, -10000i64);
    let mut caught = 0;
    let mut error = None;
    let (mut max_first, mut max_turn) = (0f64, 0f64);
    let mut turn = 1i64;

    let mut init = String::new();
    init.push_str(&format!("{}\n", n));
    for &(x, y) in &g.rabbits {
        init.push_str(&format!("{} {}\n", x, y));
    }
    let mut first_input = Some(init);

    while turn <= 600 {
        let mut msg = first_input.take().unwrap_or_default();
        msg.push_str(&format!("{}\n", length));
        for &(x, y) in pos.iter().rev() {
            msg.push_str(&format!("{} {}\n", x, y));
        }
        let t0 = Instant::now();
        if stdin.write_all(msg.as_bytes()).and_then(|_| stdin.flush()).is_err() {
            error = Some("solver closed stdin".into());
            break;
        }
        let mut line = String::new();
        if stdout.read_line(&mut line).unwrap_or(0) == 0 {
            error = Some("no output".into());
            break;
        }
        let ms = t0.elapsed().as_secs_f64() * 1000.0;
        if turn == 1 {
            max_first = ms;
        } else {
            max_turn = max_turn.max(ms);
        }
        let parts: Vec<&str> = line.trim_end_matches(['\n', '\r']).split(' ').collect();
        let parsed = if parts.len() == 2 { parts[0].parse::<i32>().ok().zip(parts[1].parse::<i32>().ok()) } else { None };
        let (x, y) = match parsed {
            Some(p) => p,
            None => {
                error = Some(format!("bad output '{}'", line.trim()));
                break;
            }
        };
        if x < 0 || x >= W || y < 0 || y >= H {
            error = Some(format!("turn {}: out of bounds {} {}", turn, x, y));
            break;
        }
        if (x - lpos.0).abs() + (y - lpos.1).abs() > 1 {
            error = Some(format!("turn {}: bad coordinates {} {}", turn, x, y));
            break;
        }
        lpos = (x, y);
        if let Some(i) = (0..n).find(|&i| !vis[i] && g.rabbits[i] == (x, y)) {
            combo = if turn - lturn <= 2 { combo + 1 } else { 1 };
            let add = if combo > 1 { 15000 * combo } else { 0 };
            let pen = if lturn != -10000 && turn - lturn > 10 { turn * (turn - lturn) } else { 0 };
            score += 10000 + add - pen;
            if std::env::var("REF_TRACE").is_ok() {
                eprintln!("catch t={} gap={} combo={} add={} pen={}", turn, turn - lturn, combo, add, pen);
            }
            vis[i] = true;
            length += 1;
            caught += 1;
            lturn = turn;
        }
        if caught == n {
            break;
        }
        pos.push_back((x, y));
        if pos.len() > length {
            pos.pop_front();
        }
        let head = *pos.back().unwrap();
        if pos.iter().rev().skip(1).any(|&p| p == head) {
            error = Some(format!("turn {}: snake bites itself at {} {}", turn, x, y));
            break;
        }
        turn += 1;
    }
    drop(stdin);
    let _ = child.kill();
    let _ = child.wait();
    Outcome { score, caught, turns: turn.min(600) as i32, error, max_first_ms: max_first, max_turn_ms: max_turn }
}

fn load_static() -> Vec<Game> {
    let mut games = Vec::new();
    let mut files: Vec<_> = std::fs::read_dir("testcases")
        .map(|d| d.filter_map(|e| e.ok()).map(|e| e.path()).collect())
        .unwrap_or_default();
    files.sort();
    for f in files {
        let txt = std::fs::read_to_string(&f).unwrap();
        let mut it = txt.lines();
        let n: usize = it.next().unwrap().trim().parse().unwrap();
        let rabbits = (0..n)
            .map(|_| {
                let v: Vec<i32> = it.next().unwrap().split_whitespace().map(|s| s.parse().unwrap()).collect();
                (v[0], v[1])
            })
            .collect();
        games.push(Game { name: f.file_stem().unwrap().to_string_lossy().into_owned(), rabbits });
    }
    games
}

fn random_game(name: String, n: usize, seed: &mut u64) -> Game {
    let mut next = |m: u64| {
        *seed ^= *seed << 13;
        *seed ^= *seed >> 7;
        *seed ^= *seed << 17;
        (*seed % m) as i32
    };
    let mut rabbits: Vec<(i32, i32)> = Vec::new();
    while rabbits.len() < n {
        let p = (next(96), next(54));
        if !rabbits.contains(&p) {
            rabbits.push(p);
        }
    }
    Game { name, rabbits }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let get = |k: &str| args.iter().position(|a| a == k).map(|i| args[i + 1].clone());
    let scale: f64 = get("--scale").map_or(1.0, |v| v.parse().unwrap());
    let rounds: usize = get("--rounds").map_or(2, |v| v.parse().unwrap());
    let threads: usize = get("--threads").map_or(10, |v| v.parse().unwrap());
    let mut seed: u64 = get("--seed").map_or(12345, |v| v.parse().unwrap());
    let bin = get("--bin").unwrap_or_else(|| {
        if cfg!(windows) { "target/release/cg_snake_rust.exe".into() } else { "target/release/cg_snake_rust".into() }
    });
    let verbose = args.iter().any(|a| a == "-v");
    let only_static = args.iter().any(|a| a == "--static");

    let mut games = load_static();
    if !only_static {
        for r in 0..rounds {
            for &n in &RANDOM_SIZES {
                games.push(random_game(format!("rnd{}_{}", r, n), n, &mut seed));
            }
        }
    }
    let games = Arc::new(games);
    let results: Arc<Mutex<Vec<Option<Outcome>>>> = Arc::new(Mutex::new((0..games.len()).map(|_| None).collect()));
    let next = Arc::new(Mutex::new(0usize));
    let start = Instant::now();
    let handles: Vec<_> = (0..threads)
        .map(|_| {
            let (games, results, next, bin) = (games.clone(), results.clone(), next.clone(), bin.clone());
            std::thread::spawn(move || loop {
                let i = {
                    let mut g = next.lock().unwrap();
                    let i = *g;
                    *g += 1;
                    i
                };
                if i >= games.len() {
                    break;
                }
                let o = play(&bin, scale, 1000 + i as u64, &games[i]);
                results.lock().unwrap()[i] = Some(o);
            })
        })
        .collect();
    for h in handles {
        h.join().unwrap();
    }

    let results = results.lock().unwrap();
    let mut static_total = 0i64;
    let mut rnd_total = 0i64;
    let mut fails = 0;
    let (mut mf, mut mt) = (0f64, 0f64);
    for (g, o) in games.iter().zip(results.iter()) {
        let o = o.as_ref().unwrap();
        mf = mf.max(o.max_first_ms);
        mt = mt.max(o.max_turn_ms);
        if o.error.is_some() {
            fails += 1;
        }
        if g.name.starts_with("rnd") {
            rnd_total += o.score;
        } else {
            static_total += o.score;
        }
        if verbose || o.error.is_some() {
            println!(
                "{:<10} n={:<3} score={:>8} caught={:>3} turns={:>3} first={:>6.1}ms turn={:>5.1}ms {}",
                g.name,
                g.rabbits.len(),
                o.score,
                o.caught,
                o.turns,
                o.max_first_ms,
                o.max_turn_ms,
                o.error.as_deref().unwrap_or("")
            );
        }
    }
    let score_of = |name: &str| {
        games.iter().zip(results.iter()).find(|(g, _)| g.name == name).map(|(_, o)| o.as_ref().unwrap().score)
    };
    println!("static total: {}", static_total);
    if !only_static && rounds > 0 {
        let rnd_mean = rnd_total as f64 / rounds as f64;
        println!("random validators (7 games) mean: {:.0}", rnd_mean);
        // validators: test1 & test3 (= test06, 70 rabbits), test2 (= test02, 60 rabbits) + 7 random
        if let (Some(a), Some(b)) = (score_of("test06"), score_of("test02")) {
            println!("estimated CG total: {:.0}", 2.0 * a as f64 + b as f64 + rnd_mean);
        }
    }
    println!("failures: {}  max first turn {:.1}ms  max turn {:.1}ms  ({:.1}s)", fails, mf, mt, start.elapsed().as_secs_f64());
}
