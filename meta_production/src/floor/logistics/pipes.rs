use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::ops::DerefMut;
use crate::floor::surface::{mix_buffers, Buffer, Port, PortMode};
use crate::geometry::{taxicab_distance, Direction, Point};

// The amount of traversal required for this is going to be significantly more than belts. If hashing proves too slow, sparse lists could be used instead.
pub struct PipeSystem {

    throughput_cap: f64,

    underground_range: u8,

    next_component_id: u64,

    surface_port_to_pipe_port: HashMap<u64, u64>,

    positions: HashMap<Point, PipeComponent>,

    pipe_nets: HashMap<u64, PipeNet>,

    undergrounds: HashMap<u64, Underground>,
    valves: HashMap<u64, Valve>,
    pumps: HashMap<u64, Pump>,
    tanks: HashMap<u64, Tank>,
    ports: HashMap<u64, PipePort>,
}

#[derive(Clone, Copy)]
enum PipeComponent {

    PIPE(u64), // ID referring to the PipeNet it belongs to
    UNDERGROUND(u64),
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
    fn add_pipe(&mut self, position: Point, surface_ports: &HashMap<u64, Port>) -> bool {

        if self.positions.get(&position).is_some() { return false; }

        let mut pipe_nets: Vec<u64> = Vec::with_capacity(4);
        let mut pipe_components: Vec<PipeComponent> = Vec::with_capacity(4);
        let mut output_valves: Vec<u64> = Vec::with_capacity(4);

        for direction in Direction::enumerate() {

            match self.positions.get(&position.add_delta(&direction)) {

                Some(PipeComponent::PIPE(pipe_net_id)) => pipe_nets.push(*pipe_net_id),
                Some(PipeComponent::UNDERGROUND(ug_id)) => {

                    let underground = self.undergrounds.get(ug_id).unwrap();
                    if underground.direction == direction { pipe_nets.push(underground.pipenet); }
                }
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

    fn get_pipenet_id(&self, position: &Point) -> Option<u64> {

        match self.positions.get(position) {

            Some(PipeComponent::PIPE(net_id)) => Some(*net_id),
            Some(PipeComponent::UNDERGROUND(ug_id)) => Some(self.undergrounds.get(&ug_id).unwrap().pipenet),
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

                Some(PipeComponent::PIPE(_)) | Some(PipeComponent::UNDERGROUND(_)) => {

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
                    if let Some(PortOutput::PIPENET(port_output_id)) = port.output && port_output_id == original_pipenet_id {

                        port.output = None;
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

    }

    fn remove_valve(&mut self, position: &Point, valve_id: u64, surface_ports: &mut HashMap<u64, Port>) {

    }

    fn remove_tank(&mut self, position: &Point, tank_id: u64) {

    }

    fn remove_pump(&mut self, position: &Point, pump_id: u64) {

    }

    /// recursive, attempts to find a path by moving a closer to b.
    // utilizes a depth-first search, prioritizing paths that reduce the taxicab distance between the two points
    // to prevent endless loops, the function keeps track of points it has already visited
    fn path_between_pipes_startup(&self, a: &Point, b: &Point) -> bool {

        let mut visited: HashSet<Point> = HashSet::new();

        match self.positions.get(a) {

            Some(PipeComponent::PIPE(_)) => {

                Direction::enumerate().iter().any(|direction| {

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

        let mut directions = match self.positions.get(a) {

            Some(PipeComponent::PIPE(_)) => { Self::priority_deltas(a, b).iter().map(|direction| { (direction.clone(), a.add_delta(direction)) } ).collect::<Vec<_>>() },

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

            self.path_between_pipes(&point, b, b_underground, *direction, visited)
        })
    }
}