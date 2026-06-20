use std::cmp::PartialEq;
use std::collections::{HashMap, HashSet};
use crate::geometry::Direction;
use crate::floor::surface::Port;
use crate::geometry::{add_points, Point};

#[derive(Default)]
pub struct BeltNet {

    global_throughput: f64,

    ports: HashMap<u64, BeltPort>,
    straights: HashMap<u64, StraightBelt>,
    splitters: HashMap<u64, Splitter>,
    mergers: HashMap<u64, Merger>,

    edges: HashMap<u64, StraightEdge>,

    surface_id_to_beltnet_id: HashMap<u64, u64>, // Tools to translate between port ids as defined by the surface to BeltPort ids as defined here
    beltnet_id_to_surface_id: HashMap<u64, u64>,

    next_component_id: u64,
    positions: HashMap<Point, NetComponent>,
}

type Buildings<'a> = (&'a HashMap<u64, BeltPort>, &'a HashMap<u64, StraightBelt>, &'a HashMap<u64, Splitter>, &'a HashMap<u64, Merger>, &'a HashMap<u64, StraightEdge>);
type BuildingsMut<'a> = (&'a mut HashMap<u64, BeltPort>, &'a mut HashMap<u64, StraightBelt>, &'a mut HashMap<u64, Splitter>, &'a mut HashMap<u64, Merger>, &'a mut HashMap<u64, StraightEdge>);

struct BeltComponent {

    direction: Direction,
    adjacent: Option<(NetComponent, BeltIOPart)>,
}

struct Buffer {

    quantity: f64,
    next_quantity: f64,

    max_quantity: f64,

    product_id: u64,
}

impl Buffer {

    // add as much as you can fit and return the remainder
    fn add(&mut self, quantity: f64) -> f64 {

        if self.max_quantity - self.quantity >= quantity { self.next_quantity += quantity; return 0.0; }

        self.next_quantity = self.max_quantity;

        quantity - (self.max_quantity - self.quantity) // The difference between how much was offered and how much extra space was left
    }

    // remove as much as you can and return the remainder
    fn subtract(&mut self, quantity: f64) -> f64 {

        if self.quantity >= quantity { self.next_quantity -= quantity; return 0.0; }

        self.next_quantity = 0.0;
        return quantity - self.quantity;
    }

    fn update(&mut self) {

        self.quantity = self.next_quantity;
        if(self.quantity == 0.0) { self.product_id = 0; }

    }

    fn clear(&mut self) {

        self.quantity = 0.0;
        self.next_quantity = 0.0;

        self.product_id = 0;
    }

}

#[derive(Clone, Copy, PartialEq)]
enum BeltIOPart {

    NONE,
    INPUT1,
    INPUT2,
    OUTPUT1,
    OUTPUT2,
}

impl BeltIOPart {

    fn opposite(&self) -> BeltIOPart {

        match self {

            BeltIOPart::NONE => BeltIOPart::NONE,
            BeltIOPart::INPUT1 => BeltIOPart::OUTPUT1,
            BeltIOPart::INPUT2 => BeltIOPart::OUTPUT2,
            BeltIOPart::OUTPUT1 => BeltIOPart::INPUT1,
            BeltIOPart::OUTPUT2 => BeltIOPart::INPUT2,
        }
    }

    fn reduce(&self) -> BeltIOPart {

        match self {

            BeltIOPart::NONE => BeltIOPart::NONE,
            BeltIOPart::INPUT1 | BeltIOPart::INPUT2 => BeltIOPart::INPUT1,
            BeltIOPart::OUTPUT1 | BeltIOPart::OUTPUT2 => BeltIOPart::OUTPUT1,
        }
    }
}

enum Priority {

    NONE,
    FIRST,
    SECOND,
}

#[derive(Clone, Copy)]
enum NetComponent {

    PORT(u64),
    STRAIGHT(u64),
    MERGER(u64),
    SPLITTER(u64),
}

impl NetComponent {

    fn link(
        &self,

        (ports, straights, splitters, mergers, edges): BuildingsMut<'_>,

        component: NetComponent,
        belt_part: BeltIOPart,
        direction: Direction,

    ) {

        match self {

            NetComponent::PORT(id) => { ports.get_mut(&id).unwrap().link(component, belt_part, direction); }
            NetComponent::MERGER(id) => { mergers.get_mut(&id).unwrap().link(component, belt_part, direction); }
            NetComponent::SPLITTER(id) => { splitters.get_mut(&id).unwrap().link(component, belt_part, direction); }
            NetComponent::STRAIGHT(id) => { straights.get_mut(&id).unwrap().link(edges, component, belt_part, direction); }
        }

    }

    fn unlink(
        &self,

        (ports, straights, splitters, mergers, edges): BuildingsMut<'_>,

        direction: Direction,

    ) {

        match self {

            NetComponent::PORT(id) => { ports.get_mut(&id).unwrap().unlink(direction); }
            NetComponent::MERGER(id) => { mergers.get_mut(&id).unwrap().unlink(direction); }
            NetComponent::SPLITTER(id) => { splitters.get_mut(&id).unwrap().unlink(direction); }
            NetComponent::STRAIGHT(id) => { straights.get_mut(&id).unwrap().unlink(edges, direction); }
        }

    }

    fn direction_to_io(
        &self,

        (ports, straights, splitters, mergers, _): Buildings<'_>,

        direction: Direction,
    ) -> BeltIOPart{

        match self {

            NetComponent::PORT(id) => { ports.get(id).unwrap().direction_to_io(direction) }
            NetComponent::MERGER(id) => { mergers.get(id).unwrap().direction_to_io(direction) }
            NetComponent::SPLITTER(id) => { splitters.get(id).unwrap().direction_to_io(direction) }
            NetComponent::STRAIGHT(id) => { straights.get(id).unwrap().direction_to_io(direction) }
        }
    }

    fn clear(

        &self,

        (ports, straights, splitters, mergers, edges): BuildingsMut<'_>,

        mut previously_visited: HashSet<u64>,
    ) -> HashSet<u64> {

        match self {

            NetComponent::PORT(_) => { previously_visited },
            NetComponent::MERGER(id) => {

                if previously_visited.contains(&id) { return previously_visited; }

                let merger = mergers.get_mut(id).unwrap();

                let input1 = merger.input1.adjacent.clone();
                let input2 = merger.input2.adjacent.clone();
                let output = merger.output.adjacent.clone();

                merger.buffer1.clear();
                merger.buffer2.clear();

                previously_visited.insert(*id);

                if let Some((net_component, _)) = input1 { previously_visited = net_component.clear((ports, straights, splitters, mergers, edges), previously_visited); }
                if let Some((net_component, _)) = input2 { previously_visited = net_component.clear((ports, straights, splitters, mergers, edges), previously_visited); }
                if let Some((net_component, _)) = output { previously_visited = net_component.clear((ports, straights, splitters, mergers, edges), previously_visited); }

                previously_visited.remove(&id);
                previously_visited
            }
            NetComponent::SPLITTER(id) => {

                if previously_visited.contains(&id) { return previously_visited; }

                let splitter = splitters.get_mut(id).unwrap();

                let input = splitter.input.adjacent.clone();
                let output1 = splitter.output1.adjacent.clone();
                let output2 = splitter.output2.adjacent.clone();

                splitter.buffer.clear();

                previously_visited.insert(*id);

                if let Some((net_component, _)) = input { previously_visited = net_component.clear((ports, straights, splitters, mergers, edges), previously_visited); }
                if let Some((net_component, _)) = output1 { previously_visited = net_component.clear((ports, straights, splitters, mergers, edges), previously_visited); }
                if let Some((net_component, _)) = output2 { previously_visited = net_component.clear((ports, straights, splitters, mergers, edges), previously_visited); }

                previously_visited.remove(&id);
                previously_visited
            }
            NetComponent::STRAIGHT(id) => {

                if previously_visited.contains(&id) { return previously_visited; }

                let edge = edges.get_mut(&straights.get_mut(id).unwrap().edge).unwrap();
                let (source, destination) = (edge.source, edge.destination);

                previously_visited.insert(*id);

                if let Some((net_component, _)) = source { previously_visited = net_component.clear((ports, straights, splitters, mergers, edges), previously_visited); }
                if let Some((net_component, _)) = destination { previously_visited = net_component.clear((ports, straights, splitters, mergers, edges), previously_visited); }

                previously_visited.remove(&id);
                previously_visited
            }
        }
    }

    // This is where the magic happens (finally written after like 850 lines of setup)
    fn push(&self, (ports, straights, splitters, mergers, edges): BuildingsMut<'_>, surface_ports: &mut HashMap<u64, Port>) -> Option<BeltNetGoof> {

        match self {

            NetComponent::PORT(id) => {

                todo!()
            }

            NetComponent::MERGER(id) => {

                todo!()
            }

            NetComponent::SPLITTER(id) => {

                todo!()
            }

            _ => {} // Straight belts dont push
        }

        None
    }
}

struct BeltPort {

    surface_id: u64,

    io: Option<(BeltComponent, BeltIOPart)>,
}

impl BeltPort {

    fn link(&mut self, component: NetComponent, belt_part: BeltIOPart, direction: Direction) {

        if let Some(_) = self.io { return; }

        // belt_part.opposite().reduce() serves to tell the port whether it is giving or receiving product based on what it is now linked to.
        self.io = Some((BeltComponent { direction, adjacent: Some((component, belt_part)) }, belt_part.opposite().reduce()));
    }

    fn unlink(&mut self, direction: Direction) {

        let belt_component: &mut BeltComponent = match &mut self.io { Some((belt_component, _)) => belt_component, None => return };

        if belt_component.direction == direction { self.io = None; }
    }

    fn direction_to_io(&self, direction: Direction) -> BeltIOPart {

        match &self.io { Some((belt_component, belt_io_part)) => if belt_component.direction == direction { *belt_io_part } else { BeltIOPart::NONE }, None => BeltIOPart::NONE }
    }
}

struct StraightBelt {

    input: BeltComponent,
    output: BeltComponent,

    edge: u64,
}

struct StraightEdge {

    source: Option<(NetComponent, BeltIOPart)>,
    destination: Option<(NetComponent, BeltIOPart)>,
}

impl StraightBelt {

    fn link(&mut self, edges: &mut HashMap<u64, StraightEdge>, component: NetComponent, belt_part: BeltIOPart, direction: Direction) {

        if direction == self.input.direction { // Assigning to both our adjacent and our corresponding edge

            self.input.adjacent = Some((component, belt_part));
            edges.get_mut(&self.edge).unwrap().source = Some((component, belt_part));
            return;
        }

        if direction == self.output.direction {

            self.output.adjacent = Some((component, belt_part));
            edges.get_mut(&self.edge).unwrap().source = Some((component, belt_part));
            return;
        }
    }

    // When removing a belt in between two others, the removal script with mutable access to the entire structure will create a new StraightEdge to propagate up the output direction while the input direction uses the old one.
    fn unlink(&mut self, edges: &mut HashMap<u64, StraightEdge>, direction: Direction) {

        if direction == self.input.direction {

            self.input.adjacent = None;
            edges.get_mut(&self.edge).unwrap().source = None
        }

        if direction == self.output.direction {

            self.output.adjacent = None;
            edges.get_mut(&self.edge).unwrap().destination = None;
        }
    }

    // Useful in propagating a new edge through output
    fn update_edge(&mut self, edge: u64) -> Option<NetComponent> {

        self.edge = edge;

        match self.output.adjacent { None => None, Some((net_component, _)) => Some(net_component) }
    }

    fn direction_to_io(&self, direction: Direction) -> BeltIOPart {

        if direction == self.input.direction { return BeltIOPart::INPUT1 }
        if direction == self.output.direction { return BeltIOPart::OUTPUT1 }

        BeltIOPart::NONE
    }
}

struct Splitter {

    input: BeltComponent,

    output1: BeltComponent,
    output2: BeltComponent,

    buffer: Buffer,

    priority: Priority
}

impl Splitter {

    fn link(&mut self, component: NetComponent, belt_part: BeltIOPart, direction: Direction) {

        if direction == self.input.direction {

            self.input.adjacent = Some((component, belt_part));
            return;
        }

        if direction == self.output1.direction {

            self.output1.adjacent = Some((component, belt_part));
            return;
        }

        if direction == self.output2.direction {

            self.output2.adjacent = Some((component, belt_part));
            return;
        }
    }

    fn unlink(&mut self, direction: Direction) {

        if direction == self.input.direction {

            self.input.adjacent = None;
            return;
        }

        if direction == self.output1.direction {

            self.output1.adjacent = None;
            return;
        }

        if direction == self.output2.direction {

            self.output2.adjacent = None;
            return;
        }
    }

    fn direction_to_io(&self, direction: Direction) -> BeltIOPart {

        if direction == self.input.direction { return BeltIOPart::INPUT1 }
        if direction == self.output1.direction { return BeltIOPart::OUTPUT1 }
        if direction == self.output2.direction { return BeltIOPart::OUTPUT2 }

        BeltIOPart::NONE
    }
}

struct Merger {

    input1: BeltComponent,
    input2: BeltComponent,

    buffer1: Buffer,
    buffer2: Buffer,

    output: BeltComponent,

    priority: Priority,
}

impl Merger {

    fn link(&mut self, component: NetComponent, belt_part: BeltIOPart, direction: Direction) {

        if direction == self.input1.direction {

            self.input1.adjacent = Some((component, belt_part));
            return;
        }

        if direction == self.input2.direction {

            self.input2.adjacent = Some((component, belt_part));
            return;
        }

        if direction == self.output.direction {

            self.output.adjacent = Some((component, belt_part));
            return;
        }
    }

    fn unlink(&mut self, direction: Direction) {

        if direction == self.input1.direction {

            self.input1.adjacent = None;
            return;
        }

        if direction == self.input2.direction {

            self.input2.adjacent = None;
            return;
        }

        if direction == self.output.direction {

            self.output.adjacent = None;
            return;
        }
    }

    fn direction_to_io(&self, direction: Direction) -> BeltIOPart {

        if direction == self.input1.direction { return BeltIOPart::INPUT1 }
        if direction == self.input2.direction { return BeltIOPart::INPUT2 }
        if direction == self.output.direction { return BeltIOPart::OUTPUT1 }

        BeltIOPart::NONE
    }
}

enum BeltNetGoof {

    CollisionOnPlacement,
    NoBuildingToRemove,
    ItemMixing,
}

impl BeltNet {

    fn buildings(&self) -> Buildings {

        (&self.ports, &self.straights, &self.splitters, &self.mergers, &self.edges)
    }

    fn buildings_mut(&mut self) -> BuildingsMut {

        (&mut self.ports, &mut self.straights, &mut self.splitters, &mut self.mergers, &mut self.edges)
    }

    fn new_component_id(&mut self) -> u64 {

        self.next_component_id += 1;
        self.next_component_id - 1
    }

    fn add_port(&mut self, surface_id: u64, position: Point) -> Option<BeltNetGoof>{

        if let Some(_) = self.positions.get(&position) { return Some(BeltNetGoof::CollisionOnPlacement) }

        let new_id = self.new_component_id();
        self.positions.insert(position.clone(), NetComponent::PORT(new_id));

        self.surface_id_to_beltnet_id.insert(surface_id, new_id);
        self.beltnet_id_to_surface_id.insert(new_id, surface_id);

        let mut new_port: BeltPort = BeltPort { surface_id, io: None };

        for direction in Direction::enumerate() {
            let neighbor = match self.positions.get(&position.add_delta(direction)) {
                None => continue,
                Some(n) => n.clone()
            };

            new_port.link(neighbor.clone(), neighbor.direction_to_io(self.buildings(), direction.opposite()), direction);

            neighbor.link(self.buildings_mut(), NetComponent::PORT(new_id), new_port.direction_to_io(direction), direction.opposite());
        }

        self.ports.insert(new_id, new_port);

        None
    }

    // This one is significantly larger to deal with straight belt -> edge optimization
    fn add_straight(&mut self, mut straight: StraightBelt, position: Point) -> Option<BeltNetGoof> {

        if let Some(_) = self.positions.get(&position) { return Some(BeltNetGoof::CollisionOnPlacement) }

        let new_id = self.new_component_id();
        self.positions.insert(position.clone(), NetComponent::STRAIGHT(new_id));

        let mut input_neighbor = self.positions.get(&position.add_delta(straight.input.direction)).cloned();
        let mut output_neighbor = self.positions.get(&position.add_delta(straight.output.direction)).cloned();

        let input_component = match input_neighbor { None => BeltIOPart::NONE, Some(net_component) => net_component.direction_to_io(self.buildings(), straight.input.direction.opposite()) };
        let output_component = match output_neighbor { None => BeltIOPart::NONE, Some(net_component) => net_component.direction_to_io(self.buildings(), straight.output.direction.opposite()) };

        // This block essentially says "if we are facing a part of our neighbor we cannot interface with, we consider there to be no neighbor at all"
        if input_component.reduce() != BeltIOPart::OUTPUT1 { input_neighbor = None; }
        if output_component.reduce() != BeltIOPart::INPUT1 { output_neighbor = None; }

        // Basic 2-way linkage between the belt and its neighbors
        let new_net_component = NetComponent::STRAIGHT(new_id);
        if let Some(input_nc) = input_neighbor
        {
            straight.input.adjacent = Some((input_nc, input_component));
            input_nc.link(self.buildings_mut(), new_net_component, BeltIOPart::INPUT1, straight.input.direction.opposite());
        }

        if let Some(output_nc) = output_neighbor {

            straight.output.adjacent = Some((output_nc, output_component));
            output_nc.link(self.buildings_mut(), new_net_component, BeltIOPart::OUTPUT1, straight.output.direction.opposite());
        }

        // More advanced and specific cases to do with edges
        match (input_neighbor, output_neighbor) {

            // Case 0: no neighbors :(
            (None, None) => {

                let new_edge_id = self.new_component_id();
                self.edges.insert(new_edge_id, StraightEdge { source: None, destination: None });
                straight.edge = new_edge_id;
            }

            // Case 1: straight input and no output
            (Some(NetComponent::STRAIGHT(input_id)), None) => {

                straight.edge = self.straights.get(&input_id).unwrap().edge;
            }

            // Case 2: no input and straight output
            (None, Some(NetComponent::STRAIGHT(output_id))) => {

                straight.edge = self.straights.get(&output_id).unwrap().edge;

            }

            // Case 3: straight input and output
            (Some(NetComponent::STRAIGHT(input_id)), Some(NetComponent::STRAIGHT(output_id))) => {

                let output_edge_id = self.straights.get(&output_id).unwrap().edge;
                let new_output = self.edges.get(&output_edge_id).unwrap().destination;
                self.edges.remove(&output_edge_id);

                let input_edge_id = self.straights.get(&input_id).unwrap().edge;
                let input_edge = self.edges.get_mut(&input_edge_id).unwrap();

                input_edge.destination = new_output;

                // Traverses down the belt chain updating belts with the new, merged edge.
                let mut belt_iterator = output_id;
                loop {

                    belt_iterator = match self.straights.get_mut(&belt_iterator).unwrap().update_edge(input_edge_id) { Some(NetComponent::STRAIGHT(next_id)) => next_id, _ => break }
                }

            }

            // Case 4: straight input and other output
            (Some(NetComponent::STRAIGHT(input_id)), Some(other_output)) => {

                let edge_id = self.straights.get(&input_id).unwrap().edge;
                let edge = self.edges.get_mut(&edge_id).unwrap();

                edge.destination = Some((other_output, output_component));

                straight.edge = edge_id;
            }

            // Case 5: other input and straight output
            (Some(other_input), Some(NetComponent::STRAIGHT(output_id))) => {

                let edge_id = self.straights.get(&output_id).unwrap().edge;
                let edge = self.edges.get_mut(&edge_id).unwrap();

                edge.source = Some((other_input, input_component));

                straight.edge = edge_id;
            }

            // Final case: each side (input, output) is either a non-straight or a None. The outcome is completely generic at this point
            (other_input @ _, other_output @ _) => {

                let new_edge_id = self.new_component_id();

                let new_edge = StraightEdge {

                    source: match other_input { None => None, Some(input_nc) => Some((input_nc, input_component)) },
                    destination: match other_output { None => None, Some(output_nc) => Some((output_nc, output_component)) },
                };

                self.edges.insert(new_edge_id, new_edge);
                straight.edge = new_edge_id;
            }
        }

        self.straights.insert(new_id, straight);

        None
    }

    fn add_splitter(&mut self, mut splitter: Splitter, position: Point) -> Option<BeltNetGoof> {

        if self.positions.get(&position).is_some() { return Some(BeltNetGoof::CollisionOnPlacement); }

        splitter.buffer.max_quantity = 2.0 * self.global_throughput;

        let new_id = self.new_component_id();

        let input = self.positions.get(&position.add_delta(splitter.input.direction)).cloned();
        let output1 = self.positions.get(&position.add_delta(splitter.output1.direction)).cloned();
        let output2 = self.positions.get(&position.add_delta(splitter.output2.direction)).cloned();

        let new_net_component = NetComponent::SPLITTER(new_id);

        if let Some(input) = input {

            let input_io_component = input.direction_to_io(self.buildings(), splitter.input.direction.opposite());
            if input_io_component.reduce() == BeltIOPart::OUTPUT1 {

                splitter.link(input, input_io_component, splitter.input.direction);
                input.link(self.buildings_mut(), new_net_component, BeltIOPart::INPUT1, splitter.input.direction.opposite());
            }
        }

        if let Some(output1) = output1 {

            let output1_io_component = output1.direction_to_io(self.buildings(), splitter.output1.direction.opposite());
            if output1_io_component.reduce() == BeltIOPart::INPUT1 {

                splitter.link(output1, output1_io_component, splitter.output1.direction);
                output1.link(self.buildings_mut(), new_net_component, BeltIOPart::OUTPUT1, splitter.output1.direction.opposite());
            }
        }

        if let Some(output2) = output2 {

            let output2_io_component = output2.direction_to_io(self.buildings(), splitter.output2.direction.opposite());
            if output2_io_component.reduce() == BeltIOPart::INPUT1 {

                splitter.link(output2, output2_io_component, splitter.output2.direction);
                output2.link(self.buildings_mut(), new_net_component, BeltIOPart::OUTPUT2, splitter.output2.direction.opposite())
            }
        }

        None
    }

    fn add_merger(&mut self, mut merger: Merger, position: Point) -> Option<BeltNetGoof> {

        if self.positions.get(&position).is_some() { return Some(BeltNetGoof::CollisionOnPlacement); }

        merger.buffer1.max_quantity = 2.0 * self.global_throughput;
        merger.buffer2.max_quantity = 2.0 * self.global_throughput;

        let new_id = self.new_component_id();

        let input1 = self.positions.get(&position.add_delta(merger.input1.direction)).cloned();
        let input2 = self.positions.get(&position.add_delta(merger.input2.direction)).cloned();
        let output = self.positions.get(&position.add_delta(merger.output.direction)).cloned();

        let new_net_component = NetComponent::SPLITTER(new_id);

        if let Some(input1) = input1 {

            let input_io_component = input1.direction_to_io(self.buildings(), merger.input1.direction.opposite());
            if input_io_component.reduce() == BeltIOPart::OUTPUT1 {

                merger.link(input1, input_io_component, merger.input1.direction);
                input1.link(self.buildings_mut(), new_net_component, BeltIOPart::INPUT1, merger.input1.direction.opposite());
            }
        }

        if let Some(input2) = input2 {

            let output1_io_component = input2.direction_to_io(self.buildings(), merger.input2.direction.opposite());
            if output1_io_component.reduce() == BeltIOPart::INPUT1 {

                merger.link(input2, output1_io_component, merger.input2.direction);
                input2.link(self.buildings_mut(), new_net_component, BeltIOPart::OUTPUT1, merger.input2.direction.opposite());
            }
        }

        if let Some(output) = output {

            let output2_io_component = output.direction_to_io(self.buildings(), merger.output.direction.opposite());
            if output2_io_component.reduce() == BeltIOPart::INPUT1 {

                merger.link(output, output2_io_component, merger.output.direction);
                output.link(self.buildings_mut(), new_net_component, BeltIOPart::OUTPUT2, merger.output.direction.opposite())
            }
        }

        None
    }

    fn remove(&mut self, position: &Point) -> Option<BeltNetGoof> {

        match self.positions.remove(position) {

            None => return Some(BeltNetGoof::NoBuildingToRemove),

            Some(NetComponent::PORT(port_id)) => self.remove_port(port_id),
            Some(NetComponent::STRAIGHT(straight_id)) => self.remove_straight(straight_id),
            Some(NetComponent::SPLITTER(splitter_id)) => self.remove_splitter(splitter_id),
            Some(NetComponent::MERGER(merger_id)) => self.remove_merger(merger_id),
        };

        return None
    }

    fn remove_port(&mut self, mut id: u64) {

        id = self.surface_id_to_beltnet_id.remove(&id).unwrap();

        match self.ports.get(&id).unwrap().io {
            Some((BeltComponent { direction, adjacent: Some((net_component, _)) }, _)) => net_component.unlink(self.buildings_mut(), direction.opposite()),
            _ => {},
        }

        self.beltnet_id_to_surface_id.remove(&id);
        self.ports.remove(&id);
    }

    fn remove_straight(&mut self, id: u64) {

        let straight = self.straights.remove(&id).unwrap();

        if let Some((input_net_component, _)) = straight.input.adjacent { input_net_component.unlink(self.buildings_mut(), straight.input.direction); }
        if let Some((output_net_component, _)) = straight.output.adjacent { output_net_component.unlink(self.buildings_mut(), straight.output.direction); }

        // splits the two edges and updates down the output side with the belt's new edge.
        if let (
            Some((NetComponent::STRAIGHT(_), _)),
            Some((NetComponent::STRAIGHT(output_straight_id), _)),

            ) = (straight.input.adjacent, straight.output.adjacent) {

            let new_edge_id = self.new_component_id();
            let new_edge = StraightEdge { source: None, destination: self.edges.get(&straight.edge).unwrap().destination };
            self.edges.insert(new_edge_id, new_edge);

            let mut belt_iterator = output_straight_id;
            loop {

                belt_iterator = match self.straights.get_mut(&belt_iterator).unwrap().update_edge(new_edge_id) { Some(NetComponent::STRAIGHT(next_straight_id)) => next_straight_id, _ => break, };
            }
        }
    }

    fn remove_splitter(&mut self, id: u64) {

        let splitter = self.splitters.remove(&id).unwrap();

        if let Some((input_net_component, _)) = splitter.input.adjacent { input_net_component.unlink(self.buildings_mut(), splitter.input.direction); }
        if let Some((output1_net_component, _)) = splitter.output1.adjacent { output1_net_component.unlink(self.buildings_mut(), splitter.output1.direction); }
        if let Some((output2_net_component, _)) = splitter.output2.adjacent { output2_net_component.unlink(self.buildings_mut(), splitter.output2.direction); }
    }

    fn remove_merger(&mut self, id: u64) {

        let merger = self.mergers.remove(&id).unwrap();

        if let Some((input1_net_component, _)) = merger.input1.adjacent { input1_net_component.unlink(self.buildings_mut(), merger.input1.direction); }
        if let Some((input2_net_component, _)) = merger.input2.adjacent { input2_net_component.unlink(self.buildings_mut(), merger.input2.direction); }
        if let Some((output_net_component, _)) = merger.output.adjacent { output_net_component.unlink(self.buildings_mut(), merger.output.direction); }
    }

    // see the clear function, a highly aggressive recursive function that clears buffers across every belt accessible from the target in all directions. Will be useful to the player later.
    fn clear_product(&mut self, position: &Point) {

        if let Some(target) = self.positions.get_mut(position) { target.clone().clear(self.buildings_mut(), HashSet::new()); }
    }

    fn tick(&mut self, ports: &mut HashMap<u64, Port>) -> Option<BeltNetGoof> {

        let mut item_mixing: bool = false;

        for port_id in self.ports.keys().copied().collect::<Vec<u64>>() {

            if let Some(BeltNetGoof::ItemMixing) = NetComponent::PORT(port_id).push(self.buildings_mut(), ports) { item_mixing = true; }
        }

        for splitter_id in self.splitters.keys().copied().collect::<Vec<u64>>() {

            if let Some(BeltNetGoof::ItemMixing) = NetComponent::PORT(splitter_id).push(self.buildings_mut(), ports) { item_mixing = true; }
        }

        for merger_id in self.mergers.keys().copied().collect::<Vec<u64>>() {

            if let Some(BeltNetGoof::ItemMixing) = NetComponent::PORT(merger_id).push(self.buildings_mut(), ports) { item_mixing = true; }
        }

        if item_mixing { Some(BeltNetGoof::ItemMixing) } else { None }
    }
}

//In sim mode this network will benefit from further compilation. A flat Vec of objects pointing towards each-others indexes with zero overhead for insertion/deletion would speed up simulation significantly