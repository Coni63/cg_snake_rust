use crate::point::Point;
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap, HashSet, VecDeque};

pub fn best_rabbit_order(start: &Point, rabbits: &[Point]) -> Vec<Point> {
    let mut unvisited = rabbits.to_vec();
    let mut current = *start;
    let mut path = Vec::new();

    let mut initial_distance = 0;
    while !unvisited.is_empty() {
        // Find the closest point
        let (idx, _) = unvisited
            .iter()
            .enumerate()
            .min_by_key(|(_, p)| current.distance(p))
            .unwrap();

        let next = unvisited.remove(idx);
        path.push(next);
        initial_distance += current.distance(&next);
        current = next;
    }

    eprintln!("Initial distance: {}", initial_distance);

    // Optional: improve using 2-opt
    let opti_path = improve_path_2opt(start, path);
    let mut total_distance = 0;
    for i in 0..opti_path.len() - 1 {
        total_distance += opti_path[i].distance(&opti_path[i + 1]);
    }
    eprintln!("Optimized distance: {}", total_distance);

    opti_path
}

// Try to optimize order using 2-opt (swap edges to reduce total path)
fn improve_path_2opt(start: &Point, mut path: Vec<Point>) -> Vec<Point> {
    let mut improved = true;
    while improved {
        improved = false;

        for i in 0..path.len() {
            for j in (i + 2)..path.len() {
                let a = if i == 0 { *start } else { path[i - 1] };
                let b = path[i];
                let c = path[j - 1];
                let d = path[j];

                let before = a.distance(&b) + c.distance(&d);
                let after = a.distance(&c) + b.distance(&d);

                if after < before {
                    path[i..j].reverse();
                    improved = true;
                }
            }
        }
    }

    path
}

// fn a_star_path(start: Point, rabbits: &[Point], snake_body: &VecDeque<Point>) -> Option<Point> {
//     let directions = vec![
//         Point::new(0, -1),
//         Point::new(0, 1),
//         Point::new(-1, 0),
//         Point::new(1, 0),
//     ];

//     let mut next_direction: Option<Point> = None;
//     let mut min_distance = i32::MAX;

//     for direction in directions.iter() {
//         let next = start + *direction;
//         if !next.isInBound(GRID_WIDTH, GRID_HEIGHT) {
//             continue;
//         }

//         if snake_body.contains(&next) {
//             continue;
//         }

//         if rabbits.contains(&next) {
//             return Some(next);
//         }

//         let mut distance = 0;

//     }

//     next_direction
// }

pub fn a_star_pathfinding(
    start: &Point,
    goal: Point,
    snake_body: &VecDeque<Point>,
) -> Option<Vec<Point>> {
    let mut open_set: BinaryHeap<Reverse<(i32, Point)>> = BinaryHeap::new();
    let mut came_from: HashMap<Point, Point> = HashMap::new();

    let mut g_score: HashMap<Point, i32> = HashMap::new();
    g_score.insert(*start, 0);

    let h = start.distance(&goal);
    open_set.push(Reverse((h, *start)));

    let snake_body_set: HashSet<Point> = snake_body.iter().copied().collect();

    while let Some(Reverse((_, current))) = open_set.pop() {
        if current == goal {
            // Reconstruct path
            let mut total_path = vec![current];
            let mut curr = current;
            while let Some(prev) = came_from.get(&curr) {
                total_path.push(*prev);
                curr = *prev;
            }
            total_path.reverse();
            return Some(total_path);
        }

        for neighbor in current.neighbors() {
            if !neighbor.is_in_bound(96, 54) {
                continue; // outside bounds
            }
            if snake_body_set.contains(&neighbor) {
                continue; // collision with self, unless it’s the goal
            }

            let tentative_g_score = g_score.get(&current).unwrap_or(&i32::MAX) + 1;

            if tentative_g_score < *g_score.get(&neighbor).unwrap_or(&i32::MAX) {
                came_from.insert(neighbor, current);
                g_score.insert(neighbor, tentative_g_score);
                let f_score = tentative_g_score + neighbor.distance(&goal);
                open_set.push(Reverse((f_score, neighbor)));
            }
        }
    }

    None // no valid path found
}
