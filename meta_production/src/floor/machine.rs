use std::hash::{Hash, Hasher};
use crate::geometry::{Space, Point};

struct Port {
    x: i32,
    y: i32,

    product_id: u32,
    quantity: f32,
}
impl Hash for Port {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.product_id.hash(state);
    }
}

trait Recipe {

    fn init(&mut self);
    fn tick(&mut self, input_buffer: &mut Vec<Port>, output_buffer: &mut Vec<Port>);
}

struct Machine {

    id: u32,

    inputs: Vec<Port>,
    outputs: Vec<Port>,

    space: Space,

    recipe: dyn Recipe,
}