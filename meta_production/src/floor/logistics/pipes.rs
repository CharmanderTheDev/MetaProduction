use std::cmp::{Ordering, PartialEq};
use std::collections::{HashMap, HashSet};
use std::io::pipe;
use std::ops::DerefMut;
use crate::floor::surface::{mix_buffers, Buffer, Port, PortMode};
use crate::geometry::{add_points, taxicab_distance, Direction, Point, Rectangle};

// The amount of traversal required for this is going to be significantly more than belts. If hashing proves too slow, sparse lists could be used instead.
pub struct PipeSystem {

    throughput_cap: f64,

    underground_range: u8,

    next_component_id: u64,

    surface_port_to_pipe_port: HashMap<u64, u64>,

    positions: HashMap<Point, PipeComponent>,

    pipe_nets: HashMap<u64, PipeNet>,

    pipes: HashMap<u64, Pipe>,
    undergrounds: HashMap<u64, Underground>,
    valves: HashMap<u64, Valve>,
    pumps: HashMap<u64, Pump>,
    tanks: HashMap<u64, Tank>,
    ports: HashMap<u64, PipePort>,
}

#[derive(Clone, Copy)]
enum PipeComponent {

    PIPE(u64),
    UNDERGROUND(u64),
    VALVE(u64),
    PORT(u64),
    PUMP(u64),
    TANK(u64),
}

struct PipeNet {

    buffer: Buffer,

    pipes: HashSet<u64>,
    valves: HashSet<u64>,
    ports: HashSet<u64>,
    pumps: HashSet<u64>,
    tanks: HashSet<u64>,
}

impl PipeNet {

    fn new() -> PipeNet {

        PipeNet {

            buffer: Buffer::new(0.0),
            pipes: HashSet::new(),
            valves: HashSet::new(),
            ports: HashSet::new(),
            pumps: HashSet::new(),
            tanks: HashSet::new(),
        }
    }
}

// Simply encodes which sides are connected to something
// Why is it bitpacked? I felt like it.
struct PipeShape {

    shape: u8,
}

impl PipeShape {

    fn is_connected(&self, direction: &Direction) -> bool{

        match direction {

            Direction::UP =>    { (self.shape & 0b0001) > 0 }
            Direction::DOWN =>  { (self.shape & 0b0010) > 0 }
            Direction::LEFT =>  { (self.shape & 0b0100) > 0 }
            Direction::RIGHT => { (self.shape & 0b1000) > 0 }
        }
    }
}

struct Pipe {

    pipenet: u64,

    up: bool,
    down: bool,
    left: bool,
    right: bool,
}

impl Pipe {

    fn alter_connection(&mut self, direction: &Direction, value: bool) {

        match direction {

            Direction::UP => { self.up = value; }
            Direction::DOWN => { self.down = value;  }
            Direction::LEFT => { self.left = value;  }
            Direction::RIGHT => { self.right = value;  }
        }
    }

    fn is_connected(&self, direction: &Direction) -> bool {

        match direction {

            Direction::UP => { self.up }
            Direction::DOWN => { self.down }
            Direction::LEFT => { self.left }
            Direction::RIGHT => { self.right }
        }
    }

    fn get_pipe_shape(&self) -> PipeShape {

        PipeShape { shape: if self.up { 0b0001 } else { 0 } + if self.down { 0b0010 } else { 0 } + if self.left { 0b0100 } else { 0 } + if self.right { 0b1000 } else { 0 } }
    }
}

struct Underground {

    position: Point,
    direction: Direction,

    pipenet: u64,

    link: Option<u64>,
}

enum ValveOutput {

    PORT(u64),
    PIPENET(u64),
}

enum ValveMode {

    INPUTMIN(f64),
    OUTPUTMIN(f64),
    INPUTMAX(f64),
    OUTPUTMAX(f64),
}

struct Valve {

    input_direction: Direction,
    output_direction: Direction,

    output: Option<ValveOutput>,

    function: ValveMode,
}

struct Pump {

    throughput_multiplier: f64,

    pipe_net: Option<(Point, u64)>, // the Point notates where it is connected through
}

struct Tank {

    capacity: f64,

    pipe_net: Option<(Point, u64)>,
}

#[derive(PartialEq)]
enum PortLogistics {

    INPUT,
    PIPENET(u64),
    VALVE(u64),
}

struct PipePort {

    surface_id: u64,

    logistics: Option<PortLogistics>,
}

impl PipeSystem {

    fn new(
        throughput_cap: f64,
        underground_range: u8
    ) -> PipeSystem {

        PipeSystem {

            throughput_cap,
            underground_range,

            next_component_id: 1,

            surface_port_to_pipe_port: HashMap::new(),

            positions: HashMap::new(),

            pipe_nets: HashMap::new(),

            pipes: HashMap::new(),
            undergrounds: HashMap::new(),
            valves: HashMap::new(),
            pumps: HashMap::new(),
            tanks: HashMap::new(),
            ports: HashMap::new(),
        }
    }

    fn get_new_component_id(&mut self) -> u64 {

        self.next_component_id += 1;
        self.next_component_id - 1
    }

    // The purpose of this function is to build a pipenet by exploring the space
    fn build_pipenet(&mut self, from: Direction, position: Point, already_explored: &mut HashSet<Point>, pipenet: &mut PipeNet, pipenet_id: u64, surface_ports: &HashMap<u64, Port>) {

        if already_explored.contains(&position) { return; }

        already_explored.insert(position.clone());

        if let Some(pipe_component) = self.positions.get(&position) {

            match pipe_component {

                PipeComponent::PIPE(pipe_id) => {

                    let pipe = self.pipes.get_mut(&pipe_id).unwrap();
                    let pipe_shape = pipe.get_pipe_shape();

                    pipenet.pipes.insert(*pipe_id);
                    pipe.pipenet = pipenet_id;
                    pipenet.buffer.max_quantity += 1.0;

                    for direction in Direction::enumerate() {

                        if direction == from.opposite()  || !pipe_shape.is_connected(&direction) { continue }

                        self.build_pipenet(direction, position.add_delta(&direction), already_explored, pipenet, pipenet_id, surface_ports);
                    }
                }
                PipeComponent::UNDERGROUND(ug_id) => {

                    let underground = self.undergrounds.get_mut(ug_id).unwrap();

                    if from != underground.direction { return }
                    underground.pipenet = pipenet_id;

                    if let Some(link) = underground.link {

                        self.build_pipenet(from, self.undergrounds.get(&link).unwrap().position.add_delta(&from), already_explored, pipenet, pipenet_id, surface_ports);
                    }
                }
                PipeComponent::VALVE(valve_id) => {

                    let valve = self.valves.get(valve_id).unwrap();
                    if valve.input_direction == from { pipenet.valves.insert(*valve_id); }
                    if valve.output_direction == from { self.valves.get_mut(valve_id).unwrap().output = Some(ValveOutput::PIPENET(pipenet_id)); }
                }
                PipeComponent::PUMP(pump_id) => {

                    // Pumps and tanks are attached modules that do not form a connective component of a PipeNet, but also can only be a part of one.
                    let pump = self.pumps.get_mut(pump_id).unwrap();
                    if pump.pipe_net == None { pipenet.pumps.insert(*pump_id); pump.pipe_net = Some((position, pipenet_id)); }
                }
                PipeComponent::TANK(tank_id) => {

                    let tank = self.tanks.get_mut(tank_id).unwrap();
                    pipenet.buffer.max_quantity += tank.capacity;

                    if tank.pipe_net == None { pipenet.tanks.insert(*tank_id); tank.pipe_net = Some((position, pipenet_id)); }
                }
                PipeComponent::PORT(port_id) => {

                    let port = self.ports.get_mut(&port_id).unwrap();
                    if port.logistics.is_some() { return }

                    match surface_ports.get(&port.surface_id).unwrap().mode {

                        PortMode::INPUT => {

                            pipenet.ports.insert(*port_id);
                            port.logistics = Some(PortLogistics::INPUT);
                        },

                        PortMode::OUTPUT => {

                            port.logistics = Some(PortLogistics::PIPENET(pipenet_id));
                        }
                    }
                }
            }
        }
    }

    /// the pipe net a consumes the pipe net b
    fn connect_pipe_nets(&mut self, id_a: u64, id_b: u64) {

        if id_a == id_b { return }

        let mut a = self.pipe_nets.remove(&id_a).unwrap();
        let b = self.pipe_nets.remove(&id_b).unwrap();

        a.pipes.extend(b.pipes);
        a.valves.extend(b.valves);
        a.ports.extend(b.ports);
        a.pumps.extend(b.pumps);
        a.tanks.extend(b.tanks);

        for pipe in &a.pipes { self.pipes.get_mut(pipe).unwrap().pipenet = id_a; }
        for valve in &a.valves { self.pipes.get_mut(valve).unwrap().pipenet = id_a; }
        for port in &a.ports { self.pipes.get_mut(port).unwrap().pipenet = id_a; }
        for pump in &a.pumps { self.pipes.get_mut(pump).unwrap().pipenet = id_a; }
        for tank in &a.tanks { self.pipes.get_mut(tank).unwrap().pipenet = id_a; }

        a.buffer = mix_buffers(&a.buffer, &b.buffer);

        self.pipe_nets.insert(id_a, a);
    }

    /// returns true if the pipe was placed, and false if it wasn't
    fn add_pipe(&mut self, position: Point, underground: Option<Underground>, surface_ports: &HashMap<u64, Port>) -> bool {

        let is_underground = underground.is_some();

        if self.positions.get(&position).is_some() { return false; }

        let mut pipe_nets: Vec<u64> = Vec::with_capacity(4);
        let mut pipe_components: Vec<PipeComponent> = Vec::with_capacity(4);
        let mut output_valves: Vec<u64> = Vec::with_capacity(4);

        let mut new_pipe = Pipe { pipenet: 0, up: false, down: false, left: false, right: false, };
        let new_pipe_id = self.get_new_component_id();

        // ug idatori
        for (direction, linked_position) in if let Some(ug) = underground {
            self.add_underground(position.clone(), ug)
        } else {
            Direction::enumerate().iter().map(|d| { (*d, position.add_delta(&d)) }).collect::<Vec<_>>()
        } {
            let adjacent = self.positions.get(&linked_position);
            if let Some(adjacent) = adjacent {

                match adjacent {

                    pipe_pipe_component @ PipeComponent::PIPE(pipe_id) => {

                        let pipe = self.pipes.get_mut(pipe_id).unwrap();

                        pipe.alter_connection(&direction.opposite(), true);
                        new_pipe.alter_connection(&direction, true);

                        pipe_nets.push(pipe.pipenet);
                        pipe_components.push(pipe_pipe_component.clone());
                    },

                    PipeComponent::UNDERGROUND(ug_id) => {

                        let underground = self.undergrounds.get(ug_id).unwrap();
                        if underground.direction == direction {

                            new_pipe.alter_connection(&direction, true);
                            pipe_nets.push(underground.pipenet);
                        }
                    },

                    valve_pipe_component @ PipeComponent::VALVE(valve_id) => {

                        let valve = self.valves.get(valve_id).unwrap();

                        if valve.input_direction == direction.opposite() { new_pipe.alter_connection(&direction, true); pipe_components.push(valve_pipe_component.clone()); }
                        if valve.output_direction == direction.opposite() { new_pipe.alter_connection(&direction, true); output_valves.push(*valve_id); }
                    },

                    tank_pipe_component @ PipeComponent::TANK(tank_id) => {

                        let tank = self.tanks.get_mut(tank_id).unwrap();

                        if tank.pipe_net.is_none() {

                            new_pipe.alter_connection(&direction, true);
                            tank.pipe_net = Some((linked_position, 0));
                            pipe_components.push(tank_pipe_component.clone());
                        }
                    },

                    pump_pipe_component @ PipeComponent::PUMP(pump_id) => {

                        let pump = self.tanks.get_mut(pump_id).unwrap();

                        if pump.pipe_net.is_none() {

                            new_pipe.alter_connection(&direction, true);
                            pump.pipe_net = Some((linked_position, 0));
                            pipe_components.push(pump_pipe_component.clone());
                        }
                    },

                    pipe_component => pipe_components.push(pipe_component.clone()),
                }
            }
        }

        if pipe_nets.len() >= 2 { for index in 1..pipe_nets.len() {

            self.connect_pipe_nets(pipe_nets[0], pipe_nets[index]);
        } }

        let pipe_net_id = if let Some(pipe_net_id) = pipe_nets.get(0) {
            *pipe_net_id
        } else {
            self.get_new_component_id()
        };

        for valve in output_valves { self.valves.get_mut(&valve).unwrap().output = Some(ValveOutput::PIPENET(pipe_net_id)); }

        let mut pipe_net = if let Some(pipe_net_id) = pipe_nets.get(0) {

            self.pipe_nets.remove(pipe_net_id).unwrap()

        } else {

            PipeNet {

                buffer: Buffer::new(
                    pipe_components.iter().map(|pipe_component| -> f64 {

                        match pipe_component {

                            PipeComponent::TANK(tank_id) => self.tanks.get(&tank_id).unwrap().capacity,
                            _ => 0.0
                        }
                    }).sum()
                ),

                pipes: HashSet::new(),
                valves: HashSet::new(),
                ports: HashSet::new(),
                pumps: HashSet::new(),
                tanks: HashSet::new(),
            }
        };

        pipe_components.iter().for_each(|pipe_component| {

            match pipe_component {

                PipeComponent::PIPE(pipe_id) => {

                    let pipe = self.pipes.get_mut(pipe_id).unwrap();
                    pipe.pipenet = pipe_net_id;

                    pipe_net.pipes.insert(*pipe_id);
                },
                PipeComponent::PORT(port_id) => {

                    let port = self.ports.get_mut(port_id).unwrap();
                    match surface_ports.get(&port.surface_id).unwrap().mode {

                        PortMode::INPUT => { pipe_net.ports.insert(*port_id); port.logistics = Some(PortLogistics::INPUT); },
                        PortMode::OUTPUT => { port.logistics = Some(PortLogistics::PIPENET(pipe_net_id)); },
                    }
                },
                PipeComponent::VALVE(valve_id) => { pipe_net.valves.insert(*valve_id); },
                PipeComponent::PUMP(pump_id) => {

                    let pump = self.pumps.get_mut(pump_id).unwrap();

                    pump.pipe_net = Some((pump.pipe_net.as_ref().unwrap().0.clone(), pipe_net_id));
                    pipe_net.pumps.insert(*pump_id);
                },
                PipeComponent::TANK(tank_id) => {

                    let tank = self.tanks.get_mut(tank_id).unwrap();

                    tank.pipe_net = Some((tank.pipe_net.as_ref().unwrap().0.clone(), pipe_net_id));
                    pipe_net.tanks.insert(*tank_id);
                },
                _ => {},
            };
        });

        if !is_underground {

                new_pipe.pipenet = pipe_net_id;
            pipe_net.pipes.insert(new_pipe_id);
            self.pipes.insert(new_pipe_id, new_pipe);
            self.positions.insert(position.clone(), PipeComponent::PIPE(new_pipe_id));
        }

        pipe_net.buffer.max_quantity += 1.0; // to account for the pipe we are adding in this function

        self.pipe_nets.insert(pipe_net_id, pipe_net);

        true
    }

    /// ONLY FOR INTERNAL USE, use add_pipe with Some(Underground) to add an underground.
    fn add_underground(&mut self, position: Point, mut underground: Underground) -> Vec<(Direction, Point)> {

        let ug_id = self.get_new_component_id();
        self.positions.insert(position.clone(), PipeComponent::UNDERGROUND(ug_id));

        let mut probe = position.clone();
        let delta = underground.direction.to_delta();

        for _ in 0..self.underground_range {

            probe.add(delta);
            if let Some(PipeComponent::UNDERGROUND(link_id)) = self.positions.get(&probe) {

                let link = self.undergrounds.get_mut(&link_id).unwrap();
                if (link.direction != underground.direction.opposite()) || underground.link.is_some() { continue; }

                link.link = Some(ug_id);
                underground.link = Some(*link_id);

                return vec![(underground.direction, add_points(&probe, delta)), (underground.direction.opposite(), position.add_delta(&underground.direction.opposite()))]
            }
        }

        let return_vec = vec![(underground.direction.opposite(), position.add_delta(&underground.direction.opposite()))];

        self.undergrounds.insert(ug_id, underground);

        return_vec
    }

    // Pipe down lad
    fn get_pipenet_id(&self, position: &Point) -> Option<u64> {

        match self.positions.get(position) {

            Some(PipeComponent::PIPE(pipe_id)) => Some(self.pipes.get(pipe_id).unwrap().pipenet),
            Some(PipeComponent::UNDERGROUND(ug_id)) => Some(self.undergrounds.get(ug_id).unwrap().pipenet),
            _ => None,
        }
    }

    fn remove(&mut self, position: &Point, surface_ports: &mut HashMap<u64, Port>) {

        match self.positions.remove(position) {

            Some(PipeComponent::PIPE(_)) | Some(PipeComponent::UNDERGROUND(_)) => { self.remove_pipe(position, surface_ports); }
            Some(PipeComponent::PORT(port_id)) => { self.remove_port(position, port_id); }
            Some(PipeComponent::VALVE(valve_id)) => { self.remove_valve(position, valve_id, surface_ports); }
            Some(PipeComponent::TANK(tank_id)) => { self.remove_tank(position, tank_id); }
            Some(PipeComponent::PUMP(pump_id)) => { self.remove_pump(position, pump_id); }

            _ => {}
        }
    }


    fn add_port(&mut self, position: &Point, mut port: PipePort, surface_id: u64, surface_ports: &mut HashMap<u64, Port>) -> bool {

        if self.positions.get(position).is_some() { return false }

        let surface_port = surface_ports.get(&surface_id).unwrap();

        let new_port_id = self.get_new_component_id();
        self.positions.insert(position.clone(), PipeComponent::PORT(new_port_id));

        Direction::enumerate().iter().for_each(|d| {

            match self.positions.get(&position.add_delta(d)) {

                Some(PipeComponent::PIPE(pipe_id)) => {

                    let pipe = self.pipes.get_mut(pipe_id).unwrap();
                    pipe.alter_connection(&d.opposite(), true);

                    match surface_port.mode {

                        PortMode::INPUT => { self.pipe_nets.get_mut(&pipe.pipenet).unwrap().ports.insert(new_port_id); },
                        PortMode::OUTPUT => { port.logistics = Some(PortLogistics::PIPENET(pipe.pipenet)); },
                    }

                    return
                },

                Some(PipeComponent::UNDERGROUND(ug_id)) => { let ug = self.undergrounds.get(ug_id).unwrap(); if (ug.direction == *d) {

                    match surface_port.mode {
                        PortMode::INPUT => { self.pipe_nets.get_mut(&ug.pipenet).unwrap().ports.insert(new_port_id); port.logistics = Some(PortLogistics::INPUT); },
                        PortMode::OUTPUT => { port.logistics = Some(PortLogistics::PIPENET(ug.pipenet)); },
                    }
                    return
                }},

                Some(PipeComponent::VALVE(valve_id)) => {

                    let valve = self.valves.get_mut(valve_id).unwrap();
                    if (PortMode::INPUT == surface_port.mode) && (valve.output_direction == d.opposite()) {

                        valve.output = Some(ValveOutput::PORT(new_port_id));
                        port.logistics = Some(PortLogistics::INPUT);
                        return
                    }
                    if (PortMode::OUTPUT == surface_port.mode) && (valve.input_direction == d.opposite()) {

                        port.logistics = Some(PortLogistics::VALVE(*valve_id));
                        return
                    }
                }

                _ => {}
            }
        });

        self.ports.insert(new_port_id, port);

        true
    }

    fn add_valve(&mut self, position: &Point, mut valve: Valve) -> bool {

        if self.positions.contains_key(position) { return false }

        let valve_id = self.get_new_component_id();

        match self.positions.get(&position.add_delta(&valve.output_direction)) {

            Some(PipeComponent::PIPE(pipe_id)) => {

                let pipe = self.pipes.get_mut(pipe_id).unwrap();
                pipe.alter_connection(&valve.output_direction.opposite(), true);

                valve.output = Some(ValveOutput::PIPENET(pipe.pipenet));
            }

            Some(PipeComponent::UNDERGROUND(ug_id)) => {

                let ug = self.undergrounds.get(ug_id).unwrap();
                if ug.direction == valve.output_direction {

                    valve.output = Some(ValveOutput::PIPENET(ug.pipenet));
                }
            }

            Some(PipeComponent::PORT(port_id)) => {

                let port = self.ports.get_mut(port_id).unwrap();

                if port.logistics.is_none() { valve.output = Some(ValveOutput::PORT(*port_id)); port.logistics = Some(PortLogistics::INPUT); }
            }

            _ => {}
        }

        match self.positions.get(&position.add_delta(&valve.input_direction)) {

            Some(PipeComponent::PIPE(pipe_id)) => {

                let pipe = self.pipes.get_mut(pipe_id).unwrap();
                pipe.alter_connection(&valve.input_direction.opposite(), true);

                self.pipe_nets.get_mut(&pipe.pipenet).unwrap().valves.insert(valve_id);
            }

            Some(PipeComponent::UNDERGROUND(ug_id)) => {

                let ug = self.undergrounds.get(ug_id).unwrap();

                if ug.direction == valve.input_direction {

                    self.pipe_nets.get_mut(&ug.pipenet).unwrap().valves.insert(valve_id);
                }
            }

            Some(PipeComponent::PORT(port_id)) => {

                let port = self.ports.get_mut(port_id).unwrap();

                if port.logistics.is_none() { port.logistics = Some(PortLogistics::VALVE(valve_id)); }
            }

            _ => {}
        }

        self.positions.insert(position.clone(), PipeComponent::VALVE(valve_id));
        self.valves.insert(valve_id, valve);

        true
    }

    fn add_tank(&mut self, mut tank: Tank, hitbox: Rectangle) -> bool {

        if hitbox.iterate_area().any(|point| {

            self.positions.contains_key(&point)

        }) { return false; }

        let tank_id = self.get_new_component_id();

        hitbox.iterate_area().for_each(|point| { self.positions.insert(point.clone(), PipeComponent::TANK(tank_id)); });

        hitbox.iterate_perimeter().for_each(|(perimeter_point, (connected_point, additional_point))| {

            if tank.pipe_net.is_some() { return }

            (if let Some(additional_point) = additional_point { vec![connected_point, additional_point] } else { vec![connected_point] }).iter().for_each(|point| {

                if tank.pipe_net.is_some() { return }

                if let Some(pipe_component) = self.positions.get(point) { match pipe_component {

                    PipeComponent::PIPE(pipe_id) => {

                        let pipe = self.pipes.get_mut(pipe_id).unwrap();
                        pipe.alter_connection(&Direction::from_points(&perimeter_point, point), true);

                        tank.pipe_net = Some((perimeter_point.clone(), pipe.pipenet));
                        let pipenet = self.pipe_nets.get_mut(&pipe.pipenet).unwrap();

                        pipenet.tanks.insert(tank_id);
                        pipenet.buffer.max_quantity += tank.capacity;
                    }

                    PipeComponent::UNDERGROUND(ug_id) => {

                        let ug = self.undergrounds.get(ug_id).unwrap();
                        if ug.direction == Direction::from_points(&perimeter_point, point) {

                            tank.pipe_net = Some((perimeter_point.clone(), ug.pipenet));
                            let pipenet = self.pipe_nets.get_mut(&ug.pipenet).unwrap();

                            pipenet.tanks.insert(tank_id);
                            pipenet.buffer.max_quantity += tank.capacity;
                        }
                    }

                    _ => { } }
                }
            })
        });

        true
    }

    fn add_pump(&mut self, mut pump: Pump, hitbox: Rectangle) -> bool {

        if hitbox.iterate_area().any(|point| {

            self.positions.contains_key(&point)

        }) { return false; }

        let pump_id = self.get_new_component_id();

        hitbox.iterate_area().for_each(|point| { self.positions.insert(point.clone(), PipeComponent::PUMP(pump_id)); });

        hitbox.iterate_perimeter().for_each(|(perimeter_point, (connected_point, additional_point))| {

            if pump.pipe_net.is_some() { return }

            (if let Some(additional_point) = additional_point { vec![connected_point, additional_point] } else { vec![connected_point] }).iter().for_each(|point| {

                if pump.pipe_net.is_some() { return }

                if let Some(pipe_component) = self.positions.get(point) { match pipe_component {

                    PipeComponent::PIPE(pipe_id) => {

                        let pipe = self.pipes.get_mut(pipe_id).unwrap();
                        pipe.alter_connection(&Direction::from_points(&perimeter_point, point), true);

                        pump.pipe_net = Some((perimeter_point.clone(), pipe.pipenet));
                        let pipenet = self.pipe_nets.get_mut(&pipe.pipenet).unwrap();

                        pipenet.pumps.insert(pump_id);
                    }

                    PipeComponent::UNDERGROUND(ug_id) => {

                        let ug = self.undergrounds.get(ug_id).unwrap();
                        if ug.direction == Direction::from_points(&perimeter_point, point) {

                            pump.pipe_net = Some((perimeter_point.clone(), ug.pipenet));
                            let pipenet = self.pipe_nets.get_mut(&ug.pipenet).unwrap();

                            pipenet.pumps.insert(pump_id);
                        }
                    }

                    _ => { } }
                }
            })
        });

        true
    }

    fn remove_pipe(&mut self, position: &Point, surface_ports: &mut HashMap<u64, Port>) {

        let original_pipenet_id = self.get_pipenet_id(&position).unwrap();

        let mut needs_disconnection: Vec<(Point, PipeComponent)> = Vec::new();

        let pipes: Vec<(Direction, Point)> =
            match self.positions.get(position) {

                Some(PipeComponent::PIPE(_)) => {

                    Direction::enumerate().iter().map(|direction| {

                        (direction.clone(), position.add_delta(direction))

                    }).collect::<Vec<_>>()
                },

                // This bit here allows us to hijack the logic of remove_pipe to work for an underground pipe's removal as well
                Some(PipeComponent::UNDERGROUND(ug_id)) => {

                    let ug = self.undergrounds.remove(ug_id).unwrap();

                    if let Some(link) = ug.link {

                        self.undergrounds.get_mut(&link).unwrap().link = None;

                        Vec::from([
                            (ug.direction, self.undergrounds.get(&link).unwrap().position.add_delta(&ug.direction)),
                            (ug.direction.opposite(), ug.position.add_delta(&ug.direction.opposite())),
                        ])
                    } else {

                        Vec::from([
                            (ug.direction.opposite(), ug.position.add_delta(&ug.direction.opposite())),
                        ])
                    }
                },

                _ => { panic!("#HDWGH") }


            }.into_iter().filter_map(|(direction, point)| {

            match self.positions.get(&point) {

                Some(PipeComponent::PIPE(pipe_id)) => {

                    self.pipes.get_mut(pipe_id).unwrap().alter_connection(&direction.opposite(), false);
                    Some((direction.clone(), position.clone()))
                },

                Some(PipeComponent::UNDERGROUND(_)) => {

                    Some((direction.clone(), position.clone()))
                },

                Some(pipe_component) => { needs_disconnection.push((position.clone(), pipe_component.clone())); None }

                _ => { None },
            }
        }).collect();

        for (point, pipe_component) in needs_disconnection {

            match pipe_component {

                PipeComponent::PORT(port_id) => {

                    let port = self.ports.get_mut(&port_id).unwrap();

                    if let Some(PortLogistics::PIPENET(port_output_id)) = port.logistics && port_output_id == original_pipenet_id {

                        port.logistics = None;
                    }

                    if let Some(PortLogistics::INPUT) = port.logistics && self.pipe_nets.get(&original_pipenet_id).unwrap().ports.contains(&port_id) {

                        port.logistics = None;
                    }
                }
                PipeComponent::VALVE(valve_id) => {

                    let valve = self.valves.get_mut(&valve_id).unwrap();
                    if let Some(ValveOutput::PIPENET(valve_output_id)) = valve.output && valve_output_id == original_pipenet_id {

                        valve.output = None;
                    }
                }
                PipeComponent::PUMP(pump_id) => {

                    let pump = self.pumps.get_mut(&pump_id).unwrap();
                    if let Some((connection_point, pump_pipenet_id)) = &pump.pipe_net && (*pump_pipenet_id == original_pipenet_id) && (connection_point == &point) {

                        pump.pipe_net = None;
                    }
                }
                PipeComponent::TANK(tank_id) => {

                    let tank = self.tanks.get_mut(&tank_id).unwrap();
                    if let Some((connection_point, tank_pipenet_id)) = &tank.pipe_net && (*tank_pipenet_id == original_pipenet_id) && (connection_point == &point) {

                        tank.pipe_net = None;
                    }
                }

                _ => {}
            }
        }

        let mut networks: Vec<(Direction, Point)> = Vec::new();
        for (direction, point) in pipes {

            if networks.iter().any(|(_, net_point)| { self.path_between_pipes_startup(&point, net_point) }) { continue; }

            networks.push((direction, point));
        }

        // If we are splitting into two or more new networks, each network needs to re-discover what is a part of it.
        if networks.len() >= 2 {

            for (direction, point) in networks[0..].iter() {

                let mut new_pipenet = PipeNet::new();
                let new_pipenet_id = self.get_new_component_id();

                self.build_pipenet(*direction, point.clone(), &mut HashSet::new(), &mut new_pipenet, new_pipenet_id, surface_ports);
            }
        }
    }

    fn remove_port(&mut self, position: &Point, port_id: u64) {

        todo!()
    }

    fn remove_valve(&mut self, position: &Point, valve_id: u64, surface_ports: &mut HashMap<u64, Port>) {

        todo!()
    }

    fn remove_tank(&mut self, position: &Point, tank_id: u64) {

        todo!()
    }

    fn remove_pump(&mut self, position: &Point, pump_id: u64) {

        todo!()
    }

    /// recursive, attempts to find a path by moving a closer to b.
    // utilizes a depth-first search, prioritizing paths that reduce the taxicab distance between the two points
    // to prevent endless loops, the function keeps track of points it has already visited
    fn path_between_pipes_startup(&self, a: &Point, b: &Point) -> bool {

        let mut visited: HashSet<Point> = HashSet::new();

        match self.positions.get(a) {

            Some(PipeComponent::PIPE(pipe_id)) => {

                let pipe = self.pipes.get(pipe_id).unwrap();

                Direction::enumerate().iter()
                    .filter(|direction| { pipe.is_connected(direction) } )
                    .any(|direction| {

                        self.path_between_pipes(
                            &a.add_delta(direction),
                            b,
                            &match self.positions.get(b) {

                                Some(PipeComponent::PIPE(_)) => { None }
                                Some(PipeComponent::UNDERGROUND(ug_id)) => { Some( self.undergrounds.get(ug_id).unwrap()) },
                                _ => return false,
                            },
                            *direction,
                            &mut visited,
                        )
                    })
            }

            Some(PipeComponent::UNDERGROUND(ug_id)) => {

                let ug = self.undergrounds.get(ug_id).unwrap();

                // we try going into and coming out of the underground
                [
                    (a, ug.direction),
                    (&a.add_delta(&ug.direction.opposite()), ug.direction.opposite())
                ].iter().any(
                    |(a, from)| {

                        self.path_between_pipes(

                            a,
                            b,
                            &Some(ug),
                            *from,
                            &mut visited,
                        )
                    }
                )
            }

            _ => false,
        }
    }

    // Helper function to determine the order in which movements should be explored in path_between_pipes
    // Yes it is hard-coded, yes by hand. Ouch. I sincerely hope having it all lined up makes it easier to look at.
    fn priority_deltas(a: &Point, b: &Point) -> Vec<Direction> {


        match (a.x.cmp(&b.x), a.y.cmp(&b.y)) {

            // When both are different, priority goes as follows: get closer on y-axis > get closer on x-axis > get farther on y-axis > get farther on x-axis
            (Ordering::Greater, Ordering::Greater) => { vec![Direction::DOWN, Direction::LEFT,  Direction::UP,   Direction::RIGHT] },
            (Ordering::Less,    Ordering::Less   ) => { vec![Direction::UP,   Direction::RIGHT, Direction::DOWN, Direction::LEFT ] },
            (Ordering::Greater, Ordering::Less   ) => { vec![Direction::DOWN, Direction::RIGHT, Direction::UP,   Direction::LEFT ] },
            (Ordering::Less,    Ordering::Greater) => { vec![Direction::UP,   Direction::LEFT,  Direction::DOWN, Direction::RIGHT] },

            // When only one is different, priority goes as follows: get closer on bad axis > increase good axis > decrease good axis > get farther on bad axis
            (Ordering::Equal,   Ordering::Greater) => { vec![Direction::LEFT,  Direction::UP,    Direction::DOWN, Direction::RIGHT] },
            (Ordering::Equal,   Ordering::Less   ) => { vec![Direction::RIGHT, Direction::UP,    Direction::DOWN, Direction::LEFT ] },
            (Ordering::Greater, Ordering::Equal  ) => { vec![Direction::DOWN,  Direction::RIGHT, Direction::LEFT, Direction::UP   ] },
            (Ordering::Less,    Ordering::Equal  ) => { vec![Direction::UP,    Direction::RIGHT, Direction::LEFT, Direction::DOWN ] },

            // Why would we ever need to see this case? Who knows.
            (Ordering::Equal,   Ordering::Equal  ) => { Direction::enumerate() }
        }
    }

    fn path_between_pipes(&self, a: &Point, b: &Point, b_underground: &Option<&Underground>, from: Direction, visited: &mut HashSet<Point> ) -> bool {

        if visited.contains(a) { return false; }
        visited.insert(a.clone());

        if (a == b) && ( b_underground.is_none_or(|underground| { underground.direction == from } ) ) { return true }

        let directions = match self.positions.get(a) {

            Some(PipeComponent::PIPE(pipe_id)) => {

                let pipe = self.pipes.get(pipe_id).unwrap();
                Self::priority_deltas(a, b).iter().filter_map(|direction| { if pipe.is_connected(direction) { Some((direction.clone(), a.add_delta(direction))) } else { None } } ).collect::<Vec<_>>()
            },

            Some(PipeComponent::UNDERGROUND(ug_id)) => {

                let ug = self.undergrounds.get(ug_id).unwrap();

                if ug.direction != from { return false; }

                let mut directions = if let Some(link) = ug.link {

                    Vec::from([
                        (ug.direction, self.undergrounds.get(&link).unwrap().position.add_delta(&ug.direction)),
                        (ug.direction.opposite(), a.add_delta(&ug.direction.opposite())),
                    ])
                } else {

                    Vec::from([
                        (ug.direction.opposite(), a.add_delta(&ug.direction.opposite())),
                    ])
                };

                directions.sort_unstable_by(|(_, v), (_, u)| { taxicab_distance(v, b).cmp(&taxicab_distance(u, b)) } );

                directions
            }

            _ => return false
        };

        directions.iter().any(|(direction, point)| {

            if *direction == from.opposite() { false } else { self.path_between_pipes(&point, b, b_underground, *direction, visited) }
        })
    }
}