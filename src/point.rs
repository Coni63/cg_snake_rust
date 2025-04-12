#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

impl Point {
    pub fn new(x: i32, y: i32) -> Self {
        Point { x, y }
    }

    pub fn distance(&self, other: &Point) -> i32 {
        let dx = (self.x - other.x).abs();
        let dy = (self.y - other.y).abs();
        dx + dy
    }

    pub fn is_in_bound(&self, width: i32, height: i32) -> bool {
        self.x >= 0 && self.x < width && self.y >= 0 && self.y < height
    }

    pub fn neighbors(&self) -> Vec<Point> {
        vec![
            Point::new(self.x + 1, self.y),
            Point::new(self.x - 1, self.y),
            Point::new(self.x, self.y + 1),
            Point::new(self.x, self.y - 1),
        ]
    }
}

impl std::ops::Add for Point {
    type Output = Point;

    fn add(self, other: Point) -> Point {
        Point::new(self.x + other.x, self.y + other.y)
    }
}

impl Ord for Point {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // First compare x coordinates
        match self.x.cmp(&other.x) {
            std::cmp::Ordering::Equal => self.y.cmp(&other.y),
            ordering => ordering,
        }
    }
}

impl PartialOrd for Point {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
