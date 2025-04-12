mod path_optimizer;
mod point;

use path_optimizer::{a_star_pathfinding, best_rabbit_order};
use point::Point;
use std::{collections::VecDeque, io};

macro_rules! parse_input {
    ($x:expr, $t:ident) => {
        $x.trim().parse::<$t>().unwrap()
    };
}

fn read_rabbits() -> Vec<Point> {
    let mut rabbits = Vec::<Point>::new();

    let mut input_line = String::new();
    io::stdin().read_line(&mut input_line).unwrap();
    let n = parse_input!(input_line, i32);
    for _ in 0..n as usize {
        let mut input_line = String::new();
        io::stdin().read_line(&mut input_line).unwrap();
        let inputs = input_line.split(" ").collect::<Vec<_>>();
        let xpoints = parse_input!(inputs[0], i32);
        let ypoints = parse_input!(inputs[1], i32);
        let point = Point::new(xpoints, ypoints);
        rabbits.push(point);
    }
    rabbits
}
fn read_snake() -> VecDeque<Point> {
    let mut snake = VecDeque::<Point>::new();
    let mut input_line = String::new();
    io::stdin().read_line(&mut input_line).unwrap();
    let nsnake = parse_input!(input_line, i32);
    for _ in 0..nsnake as usize {
        let mut input_line = String::new();
        io::stdin().read_line(&mut input_line).unwrap();
        let inputs = input_line.split(" ").collect::<Vec<_>>();
        let xsnake = parse_input!(inputs[0], i32);
        let ysnake = parse_input!(inputs[1], i32);
        let point = Point::new(xsnake, ysnake);
        snake.push_front(point); // head is added to the back, tail is added to the front
    }

    snake
}

fn main() {
    let mut rabbits = read_rabbits();

    let mut first = true;
    let snake = read_snake();
    let head = snake.back().unwrap();
    // eprintln!("Rabbits: {:?}", rabbits);
    // eprintln!("Snake: {:?}", snake);

    if first {
        first = false;
        rabbits = best_rabbit_order(head, &rabbits);
        // println!("Best order:");
        // for p in order {
        //     println!("{:?}", p);
        // }
    }

    if let Some(next_position) = a_star_pathfinding(head, rabbits[0], &snake) {
        // println!("Next position: {:?}", next_position);
        println!("{:?} {:?}", next_position[0].x, next_position[0].y);
    } else {
        // println!("No path found");
        println!("0 0");
    }
}
