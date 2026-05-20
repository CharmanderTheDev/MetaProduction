use std::collections::HashMap;
use std::hash::{Hash, Hasher};

struct BeltNet {

    next_edge_id: u64,
    next_vertex_id: u64,
    edges: HashMap<u64, Edge>,
    vertices: HashMap<u64, Vertex>,


}

struct Edge {

    id: u64,

    from_vert: VertexID,
    to_vert: VertexID,
}

impl Hash for Edge {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
} impl PartialEq for Edge {
    fn eq(&self, other: &Self) -> bool { self.id == other.id }
} impl Eq for Edge {}


struct Vertex {

    id: u64,

    directions: (
        Option<Direction>,
        Option<Direction>,
        Option<Direction>,
        Option<Direction>,
    )
}

struct Direction {

    edge_id: u64,
    way: IO,
    priority: i8,
}

enum IO {
    In,
    Out
}

impl Hash for Vertex {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
} impl PartialEq for Vertex {
    fn eq(&self, other: &Self) -> bool { self.id == other.id }
} impl Eq for Vertex {}

struct VertexID {

    id: u64,
}

struct Belt {


}