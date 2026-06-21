use std::collections::HashMap;
use crate::geometry::{spaces_intersect, Point, Space};
use crate::floor::machine::Machine;

struct Surface {

    pub area: Space,

    next_port_id: u64,
    port_ids: HashMap<u64, Port>,
    port_map: HashMap<Point, u64>,

    next_machine_id: u64,
    machines: HashMap<u64, Machine>,

    
}

impl Surface {

    fn tick(&mut self) {


        self.machines.iter_mut().for_each(
            |machine| machine.1.recipe.tick(&mut self.port_ids)
        );
    }

    fn remove_machine(&mut self, machine_id: u64) -> Option<Machine> {

        let dead_ports: Vec<u64> = (self.machines.get_mut(&machine_id)?).recipe.return_ports();

        for port in dead_ports {

            self.port_map.remove(&self.port_ids[&port].position);
            self.port_ids.remove(&port);
        }

        self.machines.remove(&machine_id)
    }

    /// attempts to add a machine and returns it back if the addition failed.
    fn add_machine(&mut self, mut new_machine: Machine) -> Option<Machine> {

        let collides = self.machines.iter_mut().any(
            |machine| spaces_intersect(&machine.1.space, &new_machine.space)
        );
        if(collides) { return Some(new_machine); }

        let new_ports: Vec<(Point, f64)> =
            new_machine.recipe.init(new_machine.space.top_left());

        let mut new_port_ids: Vec<u64> = vec![];

        for port in new_ports {

            let new_port_id = self.get_next_port_id();
            new_port_ids.push(new_port_id);

            self.port_ids.insert(new_port_id, Port::new(port.clone()));
            self.port_map.insert(port.0, new_port_id);
        }

        new_machine.recipe.give_ports(new_port_ids);

        let new_machine_id = self.get_next_machine_id();
        self.machines.insert(
            new_machine_id,
            new_machine
        );

        None
    }

    fn get_next_port_id(&mut self) -> u64 {

        self.next_port_id += 1;
        self.next_port_id - 1
    }

    fn get_next_machine_id(&mut self) -> u64 {

        self.next_machine_id += 1;
        self.next_machine_id - 1
    }
}

pub(crate) struct Port {

    pub position: Point,

    pub buffer: Buffer,
}

impl Port {

    fn new((position, max_quantity): (Point, f64)) -> Self {

        Port { position, buffer: Buffer { quantity: 0.0, next_quantity: 0.0, max_quantity, product_id: 0 }}
    }
}

pub struct Buffer {

    pub quantity: f64,
    pub next_quantity: f64,

    pub max_quantity: f64,

    pub product_id: u64,
}

// returns amount transferred, or None if item mixing occurred
pub fn buffer_transfer(from: &mut Buffer, to: &mut Buffer, mut throughput: f64) -> Option<f64> {

    if from.product_id != to.product_id { return None; }

    throughput = if from.quantity >= throughput { throughput } else { from.quantity }; // Only take as much as we can
    throughput = if to.max_quantity - to.quantity >= throughput { throughput } else { to.max_quantity - to.quantity }; // Only give as much as we can fit

    from.next_quantity -= throughput;
    to.next_quantity += throughput;

    Some(throughput)
}

impl Buffer {

    pub fn update(&mut self) {

        self.quantity = self.next_quantity;
        if(self.quantity == 0.0) { self.product_id = 0; }

    }

    pub(crate) fn clear(&mut self) {

        self.quantity = 0.0;
        self.next_quantity = 0.0;

        self.product_id = 0;
    }

}