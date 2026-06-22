#[derive(Clone, Hash, Eq, PartialEq)]
pub struct Point {
    pub(crate) x: i32,
    pub(crate) y: i32,
}
impl Point {

    pub fn add(&mut self, other: &Point) {

        self.x += other.x;
        self.y += other.y;
    }

    pub fn add_delta(&self, direction: &Direction) -> Point {

        add_points(
            self,
            match direction {
                Direction::UP => &Point { x: 0, y: 1 },
                Direction::DOWN => &Point { x: 0, y: -1 },
                Direction::RIGHT => &Point { x: 1, y: 0 },
                Direction::LEFT => &Point { x: -1, y: 0 },
            },
        )
    }
}

pub fn add_points(a: &Point, b: &Point) -> Point {

    return Point { x: a.x + b.x, y: a.y + b.y };
}

pub fn taxicab_distance(a: &Point, b: &Point) -> i32 {

    (a.x - b.x).abs() + (a.y - b.y).abs()
}

#[derive(Clone)]
pub struct Rectangle {
    min: Point,
    max: Point,
}
impl Rectangle {
    fn from_coordinates(min_x: i32, min_y: i32, max_x: i32, max_y: i32) -> Self {
        Self { min: Point { x: min_x, y: min_y }, max: Point { x: max_x, y: max_y } }
    }

    fn from_points(min: Point, max: Point) -> Self {
        Self { min, max }
    }
}

fn rectangles_intersect(a: &Rectangle, b: &Rectangle) -> bool {
    a.min.x <= b.max.x && a.max.x >= b.min.x && a.min.y <= b.max.y && a.max.y >= b.min.y
}

pub struct Space {
    rectangles: Vec<Rectangle>,
    max_rectangle: Rectangle,
}
impl Space {
    fn new() -> Self {
        Self { rectangles: vec![], max_rectangle: Rectangle::from_coordinates(0, 0, 0, 0) }
    }

    fn add(&mut self, rectangle: Rectangle) -> &mut Space {

        if self.rectangles.is_empty() { self.max_rectangle = rectangle.clone(); }

        else {
            if self.max_rectangle.min.x > rectangle.min.x { self.max_rectangle.min.x = rectangle.min.x; }
            if self.max_rectangle.min.y > rectangle.min.y { self.max_rectangle.min.y = rectangle.min.y; }
            if self.max_rectangle.max.x < rectangle.max.x { self.max_rectangle.max.x = rectangle.max.x; }
            if self.max_rectangle.max.y < rectangle.max.y { self.max_rectangle.max.y = rectangle.max.y; }
        }

        self.rectangles.push(rectangle);

        self
    }

    pub(crate) fn top_left(&self) -> Point {

        return self.max_rectangle.min.clone();
    }
}

pub fn spaces_intersect(a: &Space, b: &Space) -> bool {

    // O(1) max rectangle check
    if !rectangles_intersect(&a.max_rectangle, &b.max_rectangle) { return false; }

    // O(n) max rectangles vs individual rectangles check
    if !a.rectangles.iter().any(|r| rectangles_intersect(r, &b.max_rectangle)) { return false; }
    if !b.rectangles.iter().any(|r| rectangles_intersect(r, &a.max_rectangle)) { return false; }

    // O(n^2) individual rectangles check
    for rectangle in &a.rectangles {
        if b.rectangles.iter().any(|r| rectangles_intersect(r, rectangle)) { return true; }
    }

    false
}

#[derive(PartialEq, Clone, Copy)]
pub enum Direction {

    UP,
    DOWN,
    LEFT,
    RIGHT,
}

impl Direction {

    pub(crate) fn opposite(&self) -> Direction {

        match self {
            Direction::UP => Direction::DOWN,
            Direction::DOWN => Direction::UP,
            Direction::RIGHT => Direction::LEFT,
            Direction::LEFT => Direction::RIGHT,
        }
    }

    pub(crate) fn enumerate() -> Vec<Direction> { vec![Direction::UP, Direction::DOWN, Direction::RIGHT, Direction::LEFT] }
}