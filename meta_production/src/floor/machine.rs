use std::collections::HashMap;
use crate::geometry::{Space, Point};
use crate::floor::surface::{Port, PortMode};

pub trait Recipe {

    /// initializes recipe state and returns points of needed ports and their corresponding maximum quantities
    fn init(&mut self, machine_point: Point) -> Vec<(Point, f64, PortMode)>;

    /// advances recipe progress
    fn tick(&mut self, port_ids: &mut HashMap<u64, Port>);

    /// gives port ids to recipe in the order they were requested by [init]
    fn give_ports(&mut self, port_ids: Vec<u64>);

    /// returns port ids given by [give_ports]
    fn return_ports(&mut self) -> Vec<u64>;
}

pub struct Machine {

    pub id: u32,

    pub space: Space,
    pub recipe: Box<dyn Recipe>,
}