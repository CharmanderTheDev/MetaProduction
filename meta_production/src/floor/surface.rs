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

        let new_ports: Vec<Point> =
            new_machine.recipe.init(new_machine.space.top_left());

        let mut new_port_ids: Vec<u64> = vec![];

        for port in new_ports {

            let new_port_id = self.get_next_port_id();
            new_port_ids.push(new_port_id);

            self.port_ids.insert(new_port_id, Port::new(port.clone()));
            self.port_map.insert(port, new_port_id);
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

    pub product_id: u64,
    pub quantity: f64,

    pub direction: PortDirection,
}

pub enum PortDirection {

    CLOSED,
    INPUT,
    OUTPUT,
    DUAL,
}
impl Port {

    fn new(position: Point) -> Self {

        Port { position, product_id: 0, quantity: 0.0, direction: PortDirection::CLOSED }
    }
}
