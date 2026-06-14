use crate::geometry::Point;

mod machine;
mod surface;
mod logistics;

#[derive(PartialEq)]
enum Direction {
    
    UP,
    DOWN,
    LEFT,
    RIGHT,
}

impl Direction {
    fn delta(&self) -> Point {

        match self {
            Direction::UP => Point { x: 0, y: 1 },
            Direction::DOWN => Point { x: 0, y: -1 },
            Direction::RIGHT => Point { x: 1, y: 0 },
            Direction::LEFT => Point { x: -1, y: 0 },
        }
    }

    fn opposite(&self) -> Direction {

        match self {
            Direction::UP => Direction::DOWN,
            Direction::DOWN => Direction::UP,
            Direction::RIGHT => Direction::LEFT,
            Direction::LEFT => Direction::RIGHT,
        }
    }

    fn enumerate() -> Vec<Direction> { vec![Direction::UP, Direction::DOWN, Direction::RIGHT, Direction::LEFT] }
}