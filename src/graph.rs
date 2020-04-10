use std::collections::HashMap;

use gfa::gfa::{Link, Segment, GFA};

use crate::handle::{Direction, Edge, Handle, NodeId};
use crate::handlegraph::HandleGraph;

#[derive(Debug, Clone)]
struct Node<'a> {
    sequence: String,
    left_edges: Vec<Handle>,
    right_edges: Vec<Handle>,
    occurrences: Vec<&'a PathMapping>,
}

impl<'a> Node<'a> {
    pub fn new(sequence: &str) -> Node<'a> {
        Node {
            sequence: sequence.to_string(),
            left_edges: vec![],
            right_edges: vec![],
            occurrences: vec![],
        }
    }
}

type PathId = i64;

#[derive(Debug)]
struct PathMapping {
    handle: Handle,
    path_id: PathId,
    prev: Option<Box<PathMapping>>,
    next: Option<Box<PathMapping>>,
}

impl PathMapping {
    fn new(handle: &Handle, path_id: PathId) -> Self {
        PathMapping {
            handle: *handle,
            path_id,
            prev: None,
            next: None,
        }
    }
}

#[derive(Debug)]
struct Path {
    head: Option<PathMapping>,
    tail: Option<PathMapping>,
    count: usize,
    path_id: PathId,
    name: String,
    is_circular: bool,
}

impl Path {
    fn new(name: &str, path_id: PathId, is_circular: bool) -> Self {
        Path {
            name: name.to_string(),
            path_id,
            is_circular,
            head: None,
            tail: None,
            count: 0,
        }
    }
}

#[derive(Debug)]
pub struct HashGraph<'a> {
    max_id: NodeId,
    min_id: NodeId,
    graph: HashMap<NodeId, Node<'a>>,
    path_id: HashMap<String, i64>,
    paths: HashMap<i64, Path>,
}

impl<'a> HashGraph<'a> {
    pub fn new() -> HashGraph<'a> {
        HashGraph {
            max_id: NodeId::from(0),
            min_id: NodeId::from(std::u64::MAX),
            graph: HashMap::new(),
            path_id: HashMap::new(),
            paths: HashMap::new(),
        }
    }

    pub fn from_gfa<'b>(gfa: &'b GFA) -> HashGraph<'a> {
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

impl<'a> HandleGraph for HashGraph<'a> {
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

    fn get_node_count(&self) -> usize {
        self.graph.len()
    }

    fn min_node_id(&self) -> NodeId {
        self.min_id
    }

    fn max_node_id(&self) -> NodeId {
        self.max_id
    }

    fn get_degree(&self, handle: &Handle, dir: Direction) -> usize {
        let node = self.get_node_unsafe(&handle.id());
        if dir == Direction::Left && handle.is_reverse()
            || dir == Direction::Right && !handle.is_reverse()
        {
            node.right_edges.len()
        } else
        // } else if dir == Direction::Left && !handle.is_reverse()
        //     || dir == Direction::Right && handle.is_reverse()
        {
            node.left_edges.len()
        }
    }

    fn has_edge(&self, left: &Handle, right: &Handle) -> bool {
        let left_node = self
            .graph
            .get(&left.id())
            .expect("Node doesn't exist for the given handle");

        None != left_node.right_edges.iter().find(|h| *h == right)
    }

    fn get_edge_count(&self) -> usize {
        self.graph
            .iter()
            .fold(0, |a, (_, v)| a + v.left_edges.len() + v.right_edges.len())
    }

    fn get_total_length(&self) -> usize {
        self.graph.iter().fold(0, |a, (_, v)| a + v.sequence.len())
    }

    fn get_base(&self, handle: &Handle, index: usize) -> char {
        char::from(
            self.get_node_unsafe(&handle.id()).sequence.as_bytes()[index],
        )
    }

    fn get_subsequence(
        &self,
        handle: &Handle,
        index: usize,
        size: usize,
    ) -> &str {
        &self.get_node_unsafe(&handle.id()).sequence[index..index + size]
    }

    fn forward(&self, handle: Handle) -> Handle {
        if handle.is_reverse() {
            handle.flip()
        } else {
            handle
        }
    }

    fn edge_handle(&self, left: &Handle, right: &Handle) -> Edge {
        let flipped_right = right.flip();
        let flipped_left = left.flip();

        if left > &flipped_right {
            Edge(flipped_right, flipped_left)
        } else if left == &flipped_right {
            if right > &flipped_left {
                Edge(flipped_right, flipped_left)
            } else {
                Edge(*left, *right)
            }
        } else {
            Edge(*left, *right)
        }
    }

    fn traverse_edge_handle(&self, edge: &Edge, left: &Handle) -> Handle {
        let Edge(el, er) = edge;
        if left == el {
            *er
        } else if left == &er.flip() {
            el.flip()
        } else {
            // TODO this should be improved -- this whole function, really
            panic!("traverse_edge_handle called with a handle that the edge didn't connect");
        }
    }

    fn follow_edges<F>(&self, handle: &Handle, dir: Direction, mut f: F) -> bool
    where
        F: FnMut(&Handle) -> bool,
    {
        let node = self.get_node_unsafe(&handle.id());
        let handles = if handle.is_reverse() != (dir == Direction::Left) {
            &node.left_edges
        } else {
            &node.right_edges
        };

        for h in handles.iter() {
            let cont = if dir == Direction::Left {
                f(&h.flip())
            } else {
                f(h)
            };

            if !cont {
                return false;
            }
        }
        true
    }

    fn for_each_handle<F>(&self, mut f: F) -> bool
    where
        F: FnMut(&Handle) -> bool,
    {
        for id in self.graph.keys() {
            if !f(&Handle::pack(*id, false)) {
                return false;
            }
        }

        true
    }
}

impl<'a> HashGraph<'a> {
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

    fn read_test_gfa() -> HashGraph<'static> {
        use gfa::parser::parse_gfa;
        use std::path::PathBuf;

        HashGraph::from_gfa(&parse_gfa(&PathBuf::from("./lil.gfa")).unwrap())
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
                println!("{:?}", Handle::pack(**id, false));
                let lefts: Vec<_> = node
                    .left_edges
                    .iter()
                    // .map(|h| graph.get_sequence(h))
                    .collect();
                println!("lefts: {:?}", lefts);
                let rights: Vec<_> = node
                    .right_edges
                    .iter()
                    // .map(|h| graph.get_sequence(h))
                    .collect();
                println!("rights: {:?}", rights);
                println!("{:?}", graph.graph.get(id));
            }
        } else {
            panic!("Couldn't parse test GFA file!");
        }
    }

    #[test]
    fn degree_is_correct() {
        let graph = read_test_gfa();

        let h1 = Handle::pack(NodeId::from(9), false);
        let h2 = Handle::pack(NodeId::from(3), false);

        assert_eq!(graph.get_degree(&h1, Direction::Right), 2);
        assert_eq!(graph.get_degree(&h1, Direction::Left), 2);
        assert_eq!(graph.get_degree(&h2, Direction::Right), 2);
        assert_eq!(graph.get_degree(&h2, Direction::Left), 1);
    }

    #[test]
    fn test_has_edge() {
        let graph = read_test_gfa();

        let h15 = Handle::from_integer(15);
        let h18 = Handle::from_integer(18);
        let h19 = h18.flip();
        let h20 = Handle::from_integer(20);

        assert_eq!(true, graph.has_edge(&h18, &h20));
        assert_eq!(true, graph.has_edge(&h19, &h20));
        assert_eq!(true, graph.has_edge(&h15, &h18));
    }
}
