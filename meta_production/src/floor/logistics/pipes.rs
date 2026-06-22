use std::collections::{HashMap, HashSet};
use std::ops::DerefMut;
use crate::floor::surface::{mix_buffers, Buffer, Port, PortMode};
use crate::geometry::{taxicab_distance, Direction, Point};

// The amount of traversal required for this is going to be significantly more than belts. If hashing proves too slow, sparse lists could be used instead.
pub struct PipeSystem {

    throughput_cap: f64,

    next_component_id: u64,

    surface_port_to_pipe_port: HashMap<u64, u64>,

    positions: HashMap<Point, PipeComponent>,

    pipe_nets: HashMap<u64, PipeNet>,

    valves: HashMap<u64, Valve>,
    pumps: HashMap<u64, Pump>,
    tanks: HashMap<u64, Tank>,
    ports: HashMap<u64, PipePort>,
}

#[derive(Clone, Copy)]
enum PipeComponent {

    PIPE(u64), // ID referring to the PipeNet it belongs to
    VALVE(u64),
    PORT(u64),
    PUMP(u64),
    TANK(u64),
}

struct PipeNet {

    buffer: Buffer,

    pipes: HashSet<Point>,
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

enum PortOutput {

    PIPENET(u64),
    VALVE(u64),
}

struct PipePort {

    surface_id: u64,

    output: Option<PortOutput>,
}

impl PipeSystem {

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

                PipeComponent::PIPE(_) => {

                    pipenet.pipes.insert(position.clone());
                    pipenet.buffer.max_quantity += 1.0;

                    self.positions.insert(position.clone(), PipeComponent::PIPE(pipenet_id));
                    for direction in Direction::enumerate() {

                        if direction == from.opposite() { continue }

                        self.build_pipenet(direction, position.add_delta(&direction), already_explored, pipenet, pipenet_id, surface_ports);
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
                    match surface_ports.get(&port.surface_id).unwrap().mode {

                        PortMode::INPUT => {

                            pipenet.ports.insert(*port_id);
                        },

                        PortMode::OUTPUT => {

                            port.output = Some(PortOutput::PIPENET(pipenet_id));
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

        for pipe in &b.pipes {

            self.positions.insert(pipe.clone(), PipeComponent::PIPE(id_a));
        }

        a.pipes.extend(b.pipes);
        a.valves.extend(b.valves);
        a.ports.extend(b.ports);
        a.pumps.extend(b.pumps);
        a.tanks.extend(b.tanks);

        a.buffer = mix_buffers(&a.buffer, &b.buffer);
    }

    /// returns true if the pipe was placed, and false if it wasn't
    fn add_pipe(&mut self, position: Point, surface_ports: &mut HashMap<u64, Port>) -> bool {

        if self.positions.get(&position).is_some() { return false; }

        let mut pipe_nets: Vec<u64> = Vec::with_capacity(4);
        let mut pipe_components: Vec<PipeComponent> = Vec::with_capacity(4);
        let mut output_valves: Vec<u64> = Vec::with_capacity(4);

        for direction in Direction::enumerate() {

            match self.positions.get(&position.add_delta(&direction)) {

                Some(PipeComponent::PIPE(pipe_net_id)) => pipe_nets.push(*pipe_net_id),
                Some(valve_pipe_component @ PipeComponent::VALVE(valve_id)) => {

                    let valve = self.valves.get(valve_id).unwrap();
                    if valve.input_direction == direction { pipe_components.push(valve_pipe_component.clone()); }
                    if valve.output_direction == direction { output_valves.push(*valve_id); }
                }
                Some(tank @ PipeComponent::TANK(tank_id)) => {

                    if self.tanks.get(tank_id).unwrap().pipe_net == None { pipe_components.push(tank.clone()); }
                }
                Some(pump @ PipeComponent::PUMP(pump_id)) => {

                    if self.pumps.get(pump_id).unwrap().pipe_net == None { pipe_components.push(pump.clone()); }
                }
                Some(pipe_component) => pipe_components.push(pipe_component.clone()),
                _ => {}
            }
        }

        if pipe_nets.len() >= 2 { for index in 1..pipe_nets.len() {

            self.connect_pipe_nets(pipe_nets[0], pipe_nets[index]);
        } }

        let pipe_net_id = if pipe_nets.len() >= 1 {
            pipe_nets[0]
        } else {
            self.get_new_component_id()
        };

        for valve in output_valves { self.valves.get_mut(&valve).unwrap().output = Some(ValveOutput::PIPENET(pipe_net_id)); }

        let pipe_net = if pipe_nets.len() >= 1 {

            let mut pipe_net = self.pipe_nets.remove(&pipe_nets[0]).unwrap();

            pipe_components.iter().for_each(|pipe_component| {

                match pipe_component {

                    PipeComponent::PORT(port_id) => {

                        let port = self.ports.get_mut(&port_id).unwrap();
                        match surface_ports.get(&port.surface_id).unwrap().mode {

                            PortMode::INPUT => { pipe_net.ports.insert(*port_id); },
                            PortMode::OUTPUT => { port.output = Some(PortOutput::PIPENET(pipe_net_id)); },
                        }
                    }
                    PipeComponent::VALVE(valve_id) => { pipe_net.valves.insert(*valve_id); }
                    PipeComponent::PUMP(pump_id) => { pipe_net.pumps.insert(*pump_id); }
                    PipeComponent::TANK(tank_id) => { pipe_net.tanks.insert(*tank_id); }
                    _ => {}
                };
            });

            pipe_net

        } else {

            let (mut valves, mut ports, mut pumps, mut tanks) : (HashSet<u64>, HashSet<u64>, HashSet<u64>, HashSet<u64>) = (HashSet::new(), HashSet::new(), HashSet::new(), HashSet::new());
            pipe_components.iter().for_each(|pipe_component| {

                match pipe_component {

                    PipeComponent::PORT(port_id) => { ports.insert(*port_id); }
                    PipeComponent::VALVE(valve_id) => { valves.insert(*valve_id); }
                    PipeComponent::PUMP(pump_id) => { pumps.insert(*pump_id); }
                    PipeComponent::TANK(tank_id) => { tanks.insert(*tank_id); }
                    _ => {}
                };
            });

            PipeNet {

                buffer: Buffer::new(
                    pipe_components.iter().map(|pipe_component| -> f64 {

                        match pipe_component {

                            PipeComponent::PIPE(_) => 1.0,
                            PipeComponent::TANK(tank_id) => self.tanks.get(&tank_id).unwrap().capacity,
                            _ => 0.0
                        }
                    }).sum()
                ),

                pipes: HashSet::from([position]),
                valves,
                ports,
                pumps,
                tanks,
            }
        };

        self.pipe_nets.insert(pipe_net_id, pipe_net);

        true
    }

    fn remove(&mut self, position: &Point, surface_ports: &mut HashMap<u64, Port>) {

        match self.positions.remove(position) {

            Some(PipeComponent::PIPE(_)) => { self.remove_pipe(position, surface_ports); }
            Some(PipeComponent::PORT(port_id)) => { self.remove_port(position, port_id); }
            Some(PipeComponent::VALVE(valve_id)) => { self.remove_valve(position, valve_id); }
            Some(PipeComponent::TANK(tank_id)) => { self.remove_tank(position, tank_id); }
            Some(PipeComponent::PUMP(pump_id)) => { self.remove_pump(position, pump_id); }

            _ => {}
        }
    }

    fn remove_pipe(&mut self, position: &Point, surface_ports: &mut HashMap<u64, Port>) {

        let primary_pipenet_id =
            if let Some(PipeComponent::PIPE(network_id)) = self.positions.get(position) { self.pipe_nets.get_mut(network_id).unwrap().pipes.remove(position); network_id } else { panic!("Attempted to remove a pipe that wasn't there??") };

        let mut needs_disconnection: Vec<(Point, PipeComponent)> = Vec::new();

        let pipes: Vec<(Direction, Point)> = Direction::enumerate().iter().map(|direction| {

            (direction, position.add_delta(direction))

        }).filter_map(|(direction, point)| {

            match self.positions.get(&point) {

                Some(PipeComponent::PIPE(_)) => {

                    Some((direction.clone(), position.clone()))
                },

                Some(pipe_component) => { needs_disconnection.push((position.clone(), pipe_component.clone())); None }

                _ => { None },
            }
        }).collect();

        let primary_pipenet = self.pipe_nets.get_mut(&primary_pipenet_id).unwrap();
        for (point, pipe_component) in needs_disconnection {

            match pipe_component {

                PipeComponent::PORT(port_id) => {

                    primary_pipenet.ports.remove(&port_id);
                    let port = self.ports.get_mut(&port_id).unwrap();
                    if let Some(PortOutput::PIPENET(port_output_id)) = port.output && port_output_id == *primary_pipenet_id {

                        port.output = None;
                    }
                }
                PipeComponent::VALVE(valve_id) => {

                    primary_pipenet.valves.remove(&valve_id);
                    let valve = self.valves.get_mut(&valve_id).unwrap();
                    if let Some(ValveOutput::PIPENET(valve_output_id)) = valve.output && valve_output_id == *primary_pipenet_id {

                        valve.output = None;
                    }
                }
                PipeComponent::PUMP(pump_id) => {

                    primary_pipenet.pumps.remove(&pump_id);
                    let pump = self.pumps.get_mut(&pump_id).unwrap();
                    if let Some((connection_point, pump_pipenet_id)) = &pump.pipe_net && (pump_pipenet_id == primary_pipenet_id) && (connection_point == &point) {

                        pump.pipe_net = None;
                    }
                }
                PipeComponent::TANK(tank_id) => {

                    primary_pipenet.tanks.remove(&tank_id);
                    let tank = self.tanks.get_mut(&tank_id).unwrap();
                    if let Some((connection_point, tank_pipenet_id)) = &tank.pipe_net && (tank_pipenet_id == primary_pipenet_id) && (connection_point == &point) {

                        tank.pipe_net = None;
                    }
                }

                _ => {}
            }
        }

        let mut networks: Vec<(Direction, Point)> = Vec::new();
        for (direction, point) in pipes {

            if networks.iter().any(|(_, net_point)| { self.path_between_pipes(&point, net_point, &mut HashSet::new())}) { continue; }

            networks.push((direction, point));
        }

        if networks.len() >= 2 {

            for (direction, point) in networks[1..].iter() {

                let mut new_pipenet = PipeNet::new();
                let new_pipenet_id = self.get_new_component_id();

                self.build_pipenet(*direction, point.clone(), &mut HashSet::new(), &mut new_pipenet, new_pipenet_id, surface_ports);
            }
        }
    }

    fn remove_port(&mut self, position: &Point, port_id: u64) {

    }

    fn remove_valve(&mut self, position: &Point, valve_id: u64) {

    }

    fn remove_tank(&mut self, position: &Point, tank_id: u64) {

    }

    fn remove_pump(&mut self, position: &Point, pump_id: u64) {

    }

    /// recursive, attempts to find a path by moving a and b closer together.
    // alternates between moving a and b, and utilizes a depth-first search, prioritizing paths that reduce the taxicab distance between the two points
    // to prevent endless loops, the function keeps track of points it has already visited
    fn path_between_pipes(&self, a: &Point, b: &Point, visited: &mut HashSet<Point> ) -> bool {

        if visited.contains(a) { return false; }
        visited.insert(a.clone());

        if taxicab_distance(a, b) <= 1 { return true; }

        if let (Some(PipeComponent::PIPE(_)), Some(PipeComponent::PIPE(_))) = (self.positions.get(a), self.positions.get(b)) {

            let mut directions = Direction::enumerate().iter().map(|direction| { (direction.clone(), b.add_delta(direction)) } ).collect::<Vec<_>>();
            directions.sort_unstable_by(|(_, v), (_, u)| { taxicab_distance(v, a).cmp(&taxicab_distance(u, a)) } );

            for (direction, point) in directions { if self.path_between_pipes(&point, a, visited ) { return true } }
        }

        false
    }
}