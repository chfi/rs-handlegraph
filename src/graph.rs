use std::collections::HashMap;

use crate::handle::{Edge, Handle, NodeId};
use crate::handlegraph::HandleGraph;

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
            max_id: NodeId::from(0),
            min_id: NodeId::from(std::u64::MAX),
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
