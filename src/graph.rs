use std::collections::HashMap;

use gfa::gfa::{Link, Segment, GFA};

use crate::handle::{Edge, Handle, NodeId};
use crate::handlegraph::HandleGraph;

#[derive(Debug, Clone)]
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

#[derive(Debug)]
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

    pub fn from_gfa(gfa: &GFA) -> HashGraph {
        let mut graph = Self::new();

        // add segments
        for seg in gfa.segments.iter() {
            // TODO to keep things simple for now, we assume the
            // segment names in the GFA are all numbers
            let id = seg.name.parse::<u64>().expect(&format!(
                "Expected integer name in GFA, was {}\n",
                seg.name
            ));
            graph.create_handle(&seg.sequence, NodeId::from(id));
        }

        for link in gfa.links.iter() {
            // for each link in the GFA, get the corresponding handles
            // based on segment name and orientation
            let left_id = link
                .from_segment
                .parse::<u64>()
                .expect("Expected integer name in GFA link");

            let right_id = link
                .to_segment
                .parse::<u64>()
                .expect("Expected integer name in GFA link");

            let left = Handle::pack(
                NodeId::from(left_id),
                !link.from_orient.as_bool(),
            );
            let right =
                Handle::pack(NodeId::from(right_id), !link.to_orient.as_bool());

            graph.create_edge(&left, &right);
        }

        // add links

        graph
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

    #[test]
    fn can_create_handles() {
        let mut graph = HashGraph::new();
        let h1 = graph.append_handle("CAAATAAG");
        let h2 = graph.append_handle("A");
        let h3 = graph.append_handle("G");

        let n1 = graph.get_node_unsafe(&h1.id());
        let n2 = graph.get_node_unsafe(&h2.id());
        let n3 = graph.get_node_unsafe(&h3.id());

        assert_eq!(h1.id(), NodeId::from(1));
        assert_eq!(h3.id(), NodeId::from(3));

        assert_eq!(n1.sequence, "CAAATAAG");
        assert_eq!(n2.sequence, "A");
        assert_eq!(n3.sequence, "G");
    }

    #[test]
    fn can_create_edges() {
        let mut graph = HashGraph::new();
        let h1 = graph.append_handle("CAAATAAG");
        let h2 = graph.append_handle("A");
        let h3 = graph.append_handle("G");
        let h4 = graph.append_handle("TTG");

        graph.create_edge(&h1, &h2);
        graph.create_edge(&h1, &h3);
        graph.create_edge(&h2, &h4);
        graph.create_edge(&h3, &h4);

        let n1 = graph.get_node_unsafe(&h1.id());
        let n2 = graph.get_node_unsafe(&h2.id());
        let n3 = graph.get_node_unsafe(&h3.id());
        let n4 = graph.get_node_unsafe(&h4.id());

        assert_eq!(true, n1.right_edges.contains(&h2));
        assert_eq!(true, n1.right_edges.contains(&h3));

        assert_eq!(true, n2.left_edges.contains(&h1.flip()));
        assert_eq!(true, n2.right_edges.contains(&h4));
        assert_eq!(true, n3.left_edges.contains(&h1.flip()));
        assert_eq!(true, n3.right_edges.contains(&h4));

        assert_eq!(true, n4.left_edges.contains(&h2.flip()));
        assert_eq!(true, n4.left_edges.contains(&h3.flip()));
    }

    #[test]
    fn construct_from_gfa() {
        use gfa::parser::parse_gfa;
        use std::path::PathBuf;

        if let Some(gfa) = parse_gfa(&PathBuf::from("./lil.gfa")) {
            let graph = HashGraph::from_gfa(&gfa);
            let node_ids: Vec<_> = graph.graph.keys().collect();
            println!("Node IDs:");
            for id in node_ids.iter() {
                println!("{:?}", id);
                let node = graph.graph.get(id).unwrap();
                let lefts: Vec<_> = node
                    .left_edges
                    .iter()
                    .map(|h| graph.get_sequence(h))
                    .collect();
                println!("lefts: {:?}", lefts);
                println!("{:?}", graph.graph.get(id));
            }
        } else {
            panic!("Couldn't parse test GFA file!");
        }
    }
}
