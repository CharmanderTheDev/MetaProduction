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
            direction.to_delta(),
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

    pub(crate) fn iterate_area(&self) -> RectangleAreaIterator { RectangleAreaIterator { min: self.min.clone(), max: self.max.clone(), current: Point { x: self.max.x, y: self.min.y - 1 } }}

    pub(crate) fn iterate_perimeter(&self) -> RectanglePerimeterIterator { RectanglePerimeterIterator { min: self.min.clone(), max: self.max.clone(), current: Point { x: self.min.x - 1, y: self.min.y }}}
}

pub struct RectangleAreaIterator {

    min: Point,
    max: Point,

    current: Point,
}

impl Iterator for RectangleAreaIterator {

    type Item = Point;

    fn next(&mut self) -> Option<Self::Item> {

        self.current.x += 1;
        if self.current.x > self.max.x {

            self.current.y += 1;
            self.current.x = self.min.x;

            if self.current.y > self.max.y {

                return None;
            }
        }

        Some(self.current.clone())
    }
}

/// Iterates around the perimeter, returning either 1 (for edges) or 2 (for corners) points directly connected to some point on the perimeter
pub struct RectanglePerimeterIterator {

    min: Point,
    max: Point,

    current: Point,
}

// iterates like this:
// 01234
// 5xxx6
// 7xxx8
// 9xxx0
// 12345

impl Iterator for RectanglePerimeterIterator {

    type Item = (Point, (Point, Option<Point>));

    fn next(&mut self) -> Option<Self::Item> {

        if self.current.y == self.min.y || self.current.y == self.max.y {

            self.current.x += 1;
            if self.current.x > self.max.x {

                (self.current.x, self.current.y) = (self.min.x, self.current.y + 1);
                if self.current.y > self.max.y {

                    return None
                }
            }
        }

        else {

            if self.current.x > self.min.x { (self.current.x, self.current.y) = (self.min.x, self.current.y + 1) }
            else { (self.current.x, self.current.y) = (self.max.x, self.current.y) }
        }

        Some((

            self.current.clone(),
            (match self.current.y {

                y if y == self.min.y => {

                    (
                        Point { x: self.current.x, y: self.min.y - 1},
                        match self.current.x {

                            x if x == self.min.x => Some( Point { x: self.min.x - 1, y: self.min.y }),
                            x if x == self.max.x => Some( Point { x: self.max.x + 1, y: self.min.y }),
                            _ => None
                        })
                }

                y if y == self.max.y => {

                    (
                        Point { x: self.current.x, y: self.current.y + 1},
                        match self.current.x {

                            x if x == self.min.x => Some(Point { x: self.min.x - 1, y: self.max.y }),
                            x if x == self.max.x => Some(Point { x: self.max.x + 1, y: self.max.y }),
                            _ => None
                        })
                }

                y @ _ => {

                    (
                        match self.current.x {

                            x if x == self.min.x => Point { x: self.min.x - 1, y: self.current.y },
                            x if x == self.max.x => Point { x: self.max.x + 1, y: self.current.y },
                            _ => { panic!("RectanglePerimeterIterator generating non-perimeter points")}
                        },
                        None)
                }
            }),
        ))
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

    pub(crate) fn to_delta(&self) -> &Point {

        match self {
            Direction::UP => &Point { x: 0, y: 1 },
            Direction::DOWN => &Point { x: 0, y: -1 },
            Direction::RIGHT => &Point { x: 1, y: 0 },
            Direction::LEFT => &Point { x: -1, y: 0 },
        }
    }

    /// returns the direction going from a to b. Only works if a and b are adjacent.
    pub fn from_points(a: &Point, b: &Point) -> Direction {

        if a.y < b.y { return Direction::UP }
        if a.y > b.y { return Direction::DOWN }
        if a.x > b.x { return Direction::LEFT }
        Direction::RIGHT
    }
}