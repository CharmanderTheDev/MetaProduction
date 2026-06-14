use std::cmp::PartialEq;
use std::collections::HashMap;
use crate::floor::Direction;
use crate::floor::surface::Port;
use crate::geometry::{add_points, Point};

#[derive(Default)]
pub struct BeltNet {

    global_throughput: f64,

    ports: HashMap<u64, BeltPort>,
    straights: HashMap<u64, StraightBelt>,
    splitters: HashMap<u64, Splitter>,
    mergers: HashMap<u64, Merger>,

    ports_by_surface_id: HashMap<u64, u64>, // Essentially a translation from port IDs as defined by the surface to BeltPort IDs as defined here

    next_component_id: u64,
    positions: HashMap<Point, NetComponent>,
}

enum NetComponent {

    PORT(u64),
    STRAIGHT(u64),
    SPLITTER(u64),
    MERGER(u64),
}


// Refers to the first or second part of the dual section of a belt component. For example, the first/second input of a merger.
enum DualBeltPart {

    FIRST,
    SECOND,
}


// More general, used to categorize a side of a belt.
enum BeltPart {

    INPUT1,
    OUTPUT1,
    INPUT2,
    OUTPUT2,
    NONE,
}

impl BeltPart {

    fn to_dual_belt_part(&self) -> Option<DualBeltPart> {

        match self {

            BeltPart::INPUT1 => Some(DualBeltPart::FIRST),
            BeltPart::OUTPUT1 => Some(DualBeltPart::FIRST),
            BeltPart::INPUT2 => Some(DualBeltPart::SECOND),
            BeltPart::OUTPUT2 => Some(DualBeltPart::SECOND),
            _ => None,
        }
    }
}

struct Buffer {

    product_id: u64,

    quantity: f64,
    next_quantity: f64,
}

struct Connection {

    direction: Direction,
    link: Option<(NetComponent, BeltPart)>,
}

struct StraightBelt {

    position: Point,

    from: Connection,
    to: Connection,
}

impl StraightBelt {

    fn direction_to_part(&self, direction: Direction) -> BeltPart {

        if direction == self.from.direction { return BeltPart::INPUT1; }
        if direction == self.to.direction { return BeltPart::OUTPUT1; }

        return BeltPart::NONE;
    }
}

struct Splitter {

    // Realspace

    buffer1: Buffer,
    buffer2: Buffer,

    position: Point,

    from: Connection,

    to1: Connection,
    to2: Connection,

    priority: Option<DualBeltPart>,

    // Straight section reduction

    source: Option<PushRef>,

    destination1: Option<PullRef>,
    destination2: Option<PullRef>,
}

impl Splitter {

    // "what component of yours corresponds to this direction"
    fn direction_to_part(&self, direction: Direction) -> BeltPart {

        if direction == self.from.direction { return BeltPart::INPUT1; }
        if direction == self.to1.direction { return BeltPart::OUTPUT1; }
        if direction == self.to2.direction { return BeltPart::INPUT2; }

        BeltPart::NONE
    }
}

struct Merger {

    // Realspace

    buffer: Buffer,

    position: Point,

    from1: Connection,
    from2: Connection,

    to: Connection,

    priority: Option<DualBeltPart>,

    //  Straight section reduction

    source1: Option<PushRef>,
    source2: Option<PushRef>,

    destination: Option<PullRef>,
}

impl Merger {

    // "what component of yours corresponds to this direction"
    fn direction_to_part(&self, direction: Direction) -> BeltPart {

        if direction == self.from1.direction { return BeltPart::INPUT1; }
        if direction == self.from2.direction { return BeltPart::INPUT2; }
        if direction == self.to.direction { return BeltPart::OUTPUT1; }

        BeltPart::NONE
    }
}

struct BeltPort {

    surface_id: u64,

    source: Option<PushRef>,

    destination: Option<PullRef>,
}

enum PushRef {

    SPLITTER(u64, DualBeltPart),
    MERGER(u64),
    PORT(u64),
}

enum PullRef {

    SPLITTER(u64),
    MERGER(u64, DualBeltPart),
    PORT(u64),
}

enum BeltOopsie {

    CollisionOnPlacement,
    BadPortSurfaceID,
    PortConnectionOverload,

}

impl BeltNet {
    fn set_throughput(&mut self, throughput: f64) {
        self.global_throughput = throughput;
    }

    fn new_component_id(&mut self) -> u64 {
        self.next_component_id += 1;
        self.next_component_id - 1
    }

    fn belt_source(&self, belt: u64) -> Option<PushRef> {
        let belt_struct: &StraightBelt = self.straights.get(&belt).unwrap();

        match &belt_struct.from.link {
            None => None,
            Some((net_component, belt_part)) => {
                match net_component {
                    NetComponent::PORT(port_id) => { Some(PushRef::PORT(*port_id)) }
                    NetComponent::MERGER(merger_id) => { Some(PushRef::MERGER(*merger_id)) }
                    NetComponent::SPLITTER(splitter_id) => { Some(PushRef::SPLITTER(*splitter_id, belt_part.to_dual_belt_part()?)) }
                    NetComponent::STRAIGHT(straight_id) => { self.belt_source(*straight_id) }
                }
            }
        }
    }

    fn belt_destination(&self, belt: u64) -> Option<PullRef> {
        let belt_struct: &StraightBelt = self.straights.get(&belt).unwrap();

        match &belt_struct.to.link {
            None => None,
            Some((net_component, belt_part)) => {
                match net_component {
                    NetComponent::PORT(port_id) => { Some(PullRef::PORT(*port_id)) }
                    NetComponent::MERGER(merger_id) => { Some(PullRef::MERGER(*merger_id, belt_part.to_dual_belt_part()?)) }
                    NetComponent::SPLITTER(splitter_id) => { Some(PullRef::SPLITTER(*splitter_id)) }
                    NetComponent::STRAIGHT(straight_id) => { self.belt_destination(*straight_id) }
                }
            }
        }
    }

    fn add_port(&mut self, port_ids: &HashMap<u64, Port>, port: u64) -> Result<(), BeltOopsie> {

        todo!("currently set up to establish connections between a port and a neighbor but not a neighbor and the new port.");

        // Error checking
        let port_ref: &Port = match port_ids.get(&port) {
            Some(p) => p,
            None => return Err(BeltOopsie::BadPortSurfaceID)
        };
        if self.positions.get(&port_ref.position).is_some() { return Err(BeltOopsie::CollisionOnPlacement) }

        // Placing into world
        let new_id: u64 = self.new_component_id();

        let mut new_port: BeltPort = BeltPort { surface_id: port, source: None, destination: None };

        self.positions.insert(port_ref.position.clone(), NetComponent::PORT(new_id));
        self.ports_by_surface_id.insert(port, new_id);

        // Establishing connections
        for direction in Direction::enumerate() {
            let neighbor: &NetComponent = match self.positions.get(&add_points(&port_ref.position, &direction.delta())) {
                None => { continue; }
                Some(n) => n,
            };

            match neighbor {

                NetComponent::PORT(_) => {}

                NetComponent::SPLITTER(splitter_id) => {

                    let splitter: &Splitter = self.splitters.get(splitter_id).unwrap();

                    match splitter.direction_to_part(direction.opposite()) {

                        BeltPart::INPUT1 => {

                            if new_port.destination.is_some() { return Err(BeltOopsie::PortConnectionOverload); }

                            new_port.destination = Some(PullRef::SPLITTER(*splitter_id));
                        }

                        side @ (BeltPart::OUTPUT1 | BeltPart::OUTPUT2) => {

                            if new_port.source.is_some() { return Err(BeltOopsie::PortConnectionOverload); }

                            new_port.source = Some(PushRef::SPLITTER(*splitter_id, side.to_dual_belt_part().unwrap()))
                        }

                        _ => {} // If it's None we can leave the source and destination alone
                    }
                }

                NetComponent::MERGER(merger_id) => {

                    let merger: &Merger = self.mergers.get(merger_id).unwrap();

                    match merger.direction_to_part(direction.opposite()) {

                        side @ (BeltPart::INPUT1 | BeltPart::INPUT2) => {

                            if new_port.destination.is_some() { return Err(BeltOopsie::PortConnectionOverload); }

                            new_port.destination = Some(PullRef::MERGER(*merger_id, side.to_dual_belt_part().unwrap()));
                        }

                        BeltPart::OUTPUT1 => {

                            if new_port.source.is_some() { return Err(BeltOopsie::PortConnectionOverload); }

                            new_port.source = Some(PushRef::MERGER(*merger_id));
                        }

                        _ => {} // If it's None we can leave the source and destination alone
                    }
                }

                NetComponent::STRAIGHT(straight_id) => {

                    let straight: &StraightBelt = self.straights.get(&straight_id).unwrap();

                    match straight.direction_to_part(direction.opposite()) {

                        BeltPart::INPUT1 => {

                            match self.belt_destination(*straight_id) {

                                None => {}
                                Some(pull_ref) => {

                                    if new_port.destination.is_some() { return Err(BeltOopsie::PortConnectionOverload); }

                                    new_port.destination = Some(pull_ref);
                                }
                            }
                        }

                        BeltPart::OUTPUT1 => {

                            match self.belt_source(*straight_id) {

                                None => {}
                                Some(push_ref) => {

                                    if new_port.source.is_some() { return Err(BeltOopsie::PortConnectionOverload); }

                                    new_port.source = Some(push_ref);
                                }
                            }
                        }

                        _ => {}
                    }
                }
            }
        }


        Ok(())
    }
}