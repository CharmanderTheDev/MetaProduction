use std::collections::HashMap;
use crate::geometry::{spaces_intersect, Point, Space};
use crate::floor::{machine::Machine, logistics::belts::*};

struct Surface {

    pub area: Space,

    next_port_id: u64,
    port_ids: HashMap<u64, Port>,
    port_map: HashMap<Point, u64>,

    next_machine_id: u64,
    machines: HashMap<u64, Machine>,

    belts: BeltNet,
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

        let new_ports: Vec<(Point, f64, PortMode)> =
            new_machine.recipe.init(new_machine.space.top_left());

        let mut new_port_ids: Vec<u64> = vec![];

        for (position, max_quantity, mode) in new_ports {

            let new_port_id = self.get_next_port_id();
            new_port_ids.push(new_port_id);

            self.port_ids.insert(new_port_id, Port::new(position.clone(), max_quantity, mode));
            self.port_map.insert(position, new_port_id);
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

#[derive(PartialEq)]
pub enum PortMode {

    INPUT,
    OUTPUT,
}

pub(crate) struct Port {

    pub position: Point,

    pub buffer: Buffer,

    pub mode: PortMode,
}

impl Port {

    fn new(position: Point, max_quantity: f64, mode: PortMode ) -> Self {

        Port { position, buffer: Buffer::new(max_quantity), mode, }
    }
}

pub struct Buffer {

    pub quantity: f64,
    pub next_quantity: f64,

    pub max_quantity: f64,

    pub product_id: u64,
    pub next_product_id: u64,
}

// returns amount transferred, or None if item mixing occurred
pub fn buffer_transfer(from: &mut Buffer, to: &mut Buffer, mut throughput: f64) -> Option<f64> {

    if (from.product_id != to.product_id) && (to.product_id != 0) && (from.product_id != 0) { return None; }

    throughput = if from.quantity >= throughput { throughput } else { from.quantity }; // Only take as much as we can
    throughput = if to.max_quantity - to.quantity >= throughput { throughput } else { to.max_quantity - to.quantity }; // Only give as much as we can fit

    from.next_quantity -= throughput;
    to.next_quantity += throughput;

    if (to.product_id == 0) && (throughput != 0.0) { to.next_product_id = from.product_id; }

    Some(throughput)
}

// used in the fluid system
pub fn mix_buffers(a: &Buffer, b: &Buffer) -> Buffer {

    Buffer {

        quantity: a.quantity + b.quantity,
        next_quantity: a.quantity + b.quantity,

        max_quantity: a.max_quantity + b.max_quantity,

        product_id: if a.product_id != b.product_id { 1 } else { a.product_id },
        next_product_id: if a.product_id != b.product_id { 1 } else { a.product_id },
    }
}

impl Buffer {

    pub fn update(&mut self) {

        self.product_id = self.next_product_id;
        self.quantity = self.next_quantity;

        if(self.quantity == 0.0) { self.product_id = 0; }

    }

    pub(crate) fn clear(&mut self) {

        self.quantity = 0.0;
        self.next_quantity = 0.0;

        self.product_id = 0;
    }

    pub(crate) fn new(max_quantity: f64) -> Buffer {

        Buffer {

            quantity: 0.0,
            next_quantity: 0.0,

            product_id: 0,
            next_product_id: 0,

            max_quantity,
        }
    }

    pub fn remaining_space(&self) -> f64 { self.max_quantity - self.quantity }
}