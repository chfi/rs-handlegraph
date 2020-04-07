use std::collections::HashMap;
use std::ops::Add;

// kinda based on libbdsg's hashgraph

// TODO other than NodeId, these shouldn't actually be u64 -- they're going
// to be bit/int vectors
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeId(u64);

impl Add<u64> for NodeId {
    type Output = Self;

    fn add(self, other: u64) -> Self {
        let NodeId(i) = self;
        NodeId(i + other)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Hash)]
pub struct Handle(u64);

impl Handle {
    pub fn as_integer(self) -> u64 {
        let Handle(i) = self;
        i
    }

    pub fn from_integer(i: u64) -> Self {
        Handle(i)
    }

    pub fn unpack_number(self) -> u64 {
        self.as_integer() >> 1
    }

    pub fn unpack_bit(self) -> bool {
        self.as_integer() & 1 != 0
    }

    pub fn pack(node_id: NodeId, is_reverse: bool) -> Handle {
        let NodeId(id) = node_id;
        if id < (0x1 << 63) {
            Handle::from_integer((id << 1) | is_reverse as u64)
        } else {
            panic!("Tried to create a handle with a node ID that filled 64 bits")
        }
    }

    fn id(&self) -> NodeId {
        NodeId(self.unpack_number())
    }

    fn is_reverse(&self) -> bool {
        self.unpack_bit()
    }

    fn flip(&self) -> Self {
        Handle(self.as_integer() ^ 1)
    }
}

#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub struct Edge(Handle, Handle);

// TODO implementing paths later
// #[derive(Debug, Clone, PartialEq, PartialOrd)]
// pub struct PathHandle(u64);

// #[derive(Debug, Clone, PartialEq, PartialOrd)]
// pub struct StepHandle(u64);

pub trait HandleGraph {
    fn has_node(&self, node_id: NodeId) -> bool;
    fn get_handle(&self, node_id: NodeId, is_reverse: bool) -> Handle;

    // fn get_id(&self, handle: &Handle) -> NodeId;
    // fn get_is_reverse(&self, handle: &Handle) -> bool;

    /*
    fn flip(&self, handle: &Handle) -> bool;
    fn get_length(&self, handle: &Handle) -> usize;
    fn get_sequence(&self, handle: &Handle) -> &str;
    fn get_node_count(&self) -> usize;
    fn min_node_id(&self) -> NodeId;
    fn max_node_id(&self) -> NodeId;

    fn get_degree(&self, handle: &Handle, go_left: bool) -> usize;

    fn has_edge(&self, left: &Handle, right: &Handle) -> bool;

    fn get_edge_count(&self) -> usize;

    fn get_total_length(&self) -> usize;

    fn get_base(&self, handle: &Handle, index: usize) -> char;

    fn get_subsequence(&self, handle: &Handle, index: usize, size: usize) -> &str;

    fn forward(&self, handle: &Handle) -> &Handle;

    fn edge_handle(&self, left: &Handle, right: &Handle) -> Edge;

    fn traverse_edge_handle(&self, edge: &Edge, left: &Handle) -> Handle;
    */

    // pub fn iter_edges(&self) ->

    // template<typename Iteratee>
    // bool follow_edges(const handle_t& handle, bool go_left, const Iteratee& iteratee) const;

    // template<typename Iteratee>
    // bool for_each_handle(const Iteratee& iteratee, bool parallel = false) const;
    // template<typename Iteratee>
    // bool for_each_edge(const Iteratee& iteratee, bool parallel = false) const;
}

struct Node {
    sequence: String,
    left_edges: Vec<Handle>,
    right_edges: Vec<Handle>,
}

impl Node {
    pub fn new(sequence: &str) -> Node {
        Node {
            sequence: sequence.to_string(),
            left_edges: vec![],
            right_edges: vec![],
        }
    }
}

pub struct HashGraph {
    max_id: NodeId,
    min_id: NodeId,
    graph: HashMap<NodeId, Node>,
    // path_id: HashMap<String, i64>,
    // paths: HashMap<i64, Path>,
    // next_path_id: i64,
}

impl HashGraph {
    pub fn new() -> HashGraph {
        HashGraph {
            max_id: NodeId(0),
            min_id: NodeId(std::u64::MAX),
            graph: HashMap::new(),
        }
    }

    fn get_node(&self, node_id: &NodeId) -> Option<&Node> {
        self.graph.get(node_id)
    }

    fn get_node_unsafe(&self, node_id: &NodeId) -> &Node {
        self.graph.get(node_id).expect(&format!(
            "Tried getting a node that doesn't exist, ID: {:?}",
            node_id
        ))
    }
}

impl HandleGraph for HashGraph {
    fn has_node(&self, node_id: NodeId) -> bool {
        self.graph.contains_key(&node_id)
    }

    fn get_handle(&self, node_id: NodeId, is_reverse: bool) -> Handle {
        Handle::pack(node_id, is_reverse)
    }

    fn get_sequence(&self, handle: &Handle) -> &str {
        &self.get_node_unsafe(&handle.id()).sequence
    }

    fn get_length(&self, handle: &Handle) -> usize {
        self.get_sequence(handle).len()
    }
}

impl HashGraph {
    pub fn create_handle(&mut self, sequence: &str, node_id: NodeId) -> Handle {
        self.graph.insert(node_id, Node::new(sequence));
        self.max_id = std::cmp::max(self.max_id, node_id);
        self.min_id = std::cmp::min(self.min_id, node_id);
        self.get_handle(node_id, false)
    }
    pub fn append_handle(&mut self, sequence: &str) -> Handle {
        self.create_handle(sequence, self.max_id + 1)
    }

    pub fn create_edge(&mut self, left: &Handle, right: &Handle) {
        let add_edge = {
            let left_node = self
                .graph
                .get(&left.id())
                .expect("Node doesn't exist for the given handle");

            None == left_node.right_edges.iter().find(|h| *h == right)
        };

        if add_edge {
            let left_node = self
                .graph
                .get_mut(&left.id())
                .expect("Node doesn't exist for the given handle");
            if left.is_reverse() {
                left_node.left_edges.push(*right);
            } else {
                left_node.right_edges.push(*right);
            }
            if left != &right.flip() {
                let right_node = self
                    .graph
                    .get_mut(&right.id())
                    .expect("Node doesn't exist for the given handle");
                if right.is_reverse() {
                    right_node.right_edges.push(left.flip());
                } else {
                    right_node.left_edges.push(left.flip());
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Handle::pack is an isomorphism; Handle <=> (u63, bool)
    #[test]
    fn handle_is_isomorphism() {
        let u: u64 = 597283742;
        let h = Handle::pack(NodeId(u), true);
        assert_eq!(h.unpack_number(), u);
        assert_eq!(h.unpack_bit(), true);
    }

    // Handle::pack should panic when the provided NodeId is invalid
    // (i.e. uses the 64th bit
    #[test]
    #[should_panic]
    fn handle_pack_panic() {
        Handle::pack(NodeId(std::u64::MAX), true);
    }

    #[test]
    fn handle_flip() {
        let u: u64 = 597283742;
        let h1 = Handle::pack(NodeId(u), true);
        let h2 = h1.flip();

        assert_eq!(h1.unpack_number(), h2.unpack_number());
        assert_eq!(h1.unpack_bit(), true);
        assert_eq!(h2.unpack_bit(), false);
    }
}
