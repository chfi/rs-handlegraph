use std::collections::HashMap;

use gfa::gfa::{Link, Segment, GFA};

use crate::handle::{Direction, Edge, Handle, NodeId};
use crate::handlegraph::HandleGraph;
use crate::pathgraph::PathHandleGraph;

type PathId = i64;
type PathStep = (i64, usize);

#[derive(Debug, Clone)]
struct Node {
    sequence: String,
    left_edges: Vec<Handle>,
    right_edges: Vec<Handle>,
    occurrences: HashMap<PathId, usize>,
}

impl Node {
    pub fn new(sequence: &str) -> Node {
        Node {
            sequence: sequence.to_string(),
            left_edges: vec![],
            right_edges: vec![],
            occurrences: HashMap::new(),
        }
    }
}

#[derive(Debug)]
struct Path {
    path_id: PathId,
    name: String,
    is_circular: bool,
    nodes: Vec<Handle>,
}

impl Path {
    fn new(name: &str, path_id: PathId, is_circular: bool) -> Self {
        Path {
            name: name.to_string(),
            path_id,
            is_circular,
            nodes: vec![],
        }
    }

    fn lookup_step_handle(&self, step: &PathStep) -> Handle {
        let (_, ix) = step;
        self.nodes[*ix]
    }
}

#[derive(Debug)]
pub struct HashGraph {
    max_id: NodeId,
    min_id: NodeId,
    graph: HashMap<NodeId, Node>,
    path_id: HashMap<String, i64>,
    paths: HashMap<i64, Path>,
}

impl HashGraph {
    pub fn new() -> HashGraph {
        HashGraph {
            max_id: NodeId::from(0),
            min_id: NodeId::from(std::u64::MAX),
            graph: HashMap::new(),
            path_id: HashMap::new(),
            paths: HashMap::new(),
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

    fn print_path(&self, path_id: &PathId) {
        let path = self.paths.get(&path_id).unwrap();
        println!("Path\t{}", path_id);
        for (ix, handle) in path.nodes.iter().enumerate() {
            let node = self.get_node(&handle.id()).unwrap();
            if ix != 0 {
                print!(" -> ");
            }
            print!("{}", node.sequence);
        }

        println!("");
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

impl PathHandleGraph for HashGraph {
    type PathHandle = PathId;
    type StepHandle = PathStep;

    fn get_path_count(&self) -> usize {
        self.path_id.len()
    }

    fn has_path(&self, name: &str) -> bool {
        self.path_id.contains_key(name)
    }

    fn get_path_handle(&self, name: &str) -> Option<Self::PathHandle> {
        self.path_id.get(name).map(|i| i.clone())
    }

    fn get_path_name(&self, handle: &Self::PathHandle) -> &str {
        if let Some(p) = self.paths.get(&handle) {
            &p.name
        } else {
            panic!("Tried to look up nonexistent path:")
        }
    }

    fn get_is_circular(&self, handle: &Self::PathHandle) -> bool {
        if let Some(p) = self.paths.get(&handle) {
            p.is_circular
        } else {
            panic!("Tried to look up nonexistent path:")
        }
    }

    fn get_step_count(&self, handle: &Self::PathHandle) -> usize {
        if let Some(p) = self.paths.get(&handle) {
            p.nodes.len()
        } else {
            panic!("Tried to look up nonexistent path:")
        }
    }

    fn get_handle_of_step(&self, step: &Self::StepHandle) -> Handle {
        self.paths.get(&step.0).unwrap().lookup_step_handle(step)
    }

    fn get_path_handle_of_step(
        &self,
        step: &Self::StepHandle,
    ) -> Self::PathHandle {
        step.0
    }

    fn path_begin(&self, path: &Self::PathHandle) -> Self::StepHandle {
        (*path, 0)
    }

    fn path_end(&self, path: &Self::PathHandle) -> Self::StepHandle {
        (*path, self.get_step_count(path))
    }

    fn path_back(&self, path: &Self::PathHandle) -> Self::StepHandle {
        (*path, self.get_step_count(path) - 1)
    }

    fn path_front_end(&self, path: &Self::PathHandle) -> Self::StepHandle {
        (*path, 0) // TODO should be -1; maybe I should use Option<usize>
    }

    fn has_next_step(&self, step: &Self::StepHandle) -> bool {
        // TODO this might be an off-by-one error
        step.1 < self.get_step_count(&step.0)
    }

    fn has_previous_step(&self, step: &Self::StepHandle) -> bool {
        step.1 > 0
    }

    fn destroy_path(&mut self, path: &Self::PathHandle) {
        let p: &Path = self.paths.get(&path).unwrap();

        for handle in p.nodes.iter() {
            let node: &mut Node = self.graph.get_mut(&handle.id()).unwrap();
            node.occurrences.remove(path);
        }
        // for h in self.paths.get(&path).unwrap().
        self.paths.remove(&path);
    }

    fn create_path_handle(
        &mut self,
        name: &str,
        is_circular: bool,
    ) -> Self::PathHandle {
        let path_id = self.paths.len() as i64;
        let path = Path::new(name, path_id, is_circular);
        self.paths.insert(path_id, path);
        path_id
    }

    fn append_step(
        &mut self,
        path_id: &Self::PathHandle,
        to_append: Handle,
    ) -> Self::StepHandle {
        let path: &mut Path = self.paths.get_mut(path_id).unwrap();
        path.nodes.push(to_append);
        let node: &mut Node = self.graph.get_mut(&to_append.id()).unwrap();
        let step = (*path_id, path.nodes.len());
        node.occurrences.insert(*path_id, 0);
        step
    }

    // TODO update occurrences in nodes
    fn prepend_step(
        &mut self,
        path_id: &Self::PathHandle,
        to_prepend: Handle,
    ) -> Self::StepHandle {
        let path: &mut Path = self.paths.get_mut(path_id).unwrap();
        // update occurrences in nodes already in the graph
        for h in path.nodes.iter() {
            let node: &mut Node = self.graph.get_mut(&h.id()).unwrap();
            *node.occurrences.get_mut(path_id).unwrap() += 1;
        }
        path.nodes.insert(0, to_prepend);
        let node: &mut Node = self.graph.get_mut(&to_prepend.id()).unwrap();
        node.occurrences.insert(*path_id, path.nodes.len());
        (*path_id, 0)
    }

    fn rewrite_segment(
        &mut self,
        begin: &Self::StepHandle,
        end: &Self::StepHandle,
        new_segment: Vec<Handle>,
    ) -> (Self::StepHandle, Self::StepHandle) {
        // extract the index range from the begin and end handles
        let (path_id, l) = begin;
        let (_, r) = end;
        let range = l..=r;
        // get a &mut to the path's vector of handles
        let handles: &mut Vec<Handle> =
            &mut self.paths.get_mut(&path_id).unwrap().nodes;

        let r = l + new_segment.len();
        // replace the range of the path's handle vector with the new segment
        handles.splice(range, new_segment);

        // update occurrences
        for (ix, handle) in
            self.paths.get(&path_id).unwrap().nodes.iter().enumerate()
        {
            let node: &mut Node = self.graph.get_mut(&handle.id()).unwrap();
            node.occurrences.insert(*path_id, ix);
        }

        // return the new beginning and end step handles
        // the start index is the same,
        // the end index is the start index + the length of new_segment
        (*begin, (*path_id, r))
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

    fn read_test_gfa() -> HashGraph {
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

    fn path_graph() -> HashGraph {
        let mut graph = HashGraph::new();
        let h1 = graph.create_handle("1", NodeId::from(1));
        let h2 = graph.create_handle("2", NodeId::from(2));
        let h3 = graph.create_handle("3", NodeId::from(3));
        let h4 = graph.create_handle("4", NodeId::from(4));
        let h5 = graph.create_handle("5", NodeId::from(5));
        let h6 = graph.create_handle("6", NodeId::from(6));

        /*
        edges
        1  -> 2 -> 5 -> 6
          \-> 3 -> 4 /
         */
        graph.create_edge(&h1, &h2);
        graph.create_edge(&h2, &h5);
        graph.create_edge(&h5, &h6);

        graph.create_edge(&h1, &h3);
        graph.create_edge(&h3, &h4);
        graph.create_edge(&h4, &h6);

        graph
    }

    #[test]
    fn append_prepend_path() {
        let mut graph = path_graph();

        let h1 = graph.get_handle(NodeId::from(1), false);
        let h2 = graph.get_handle(NodeId::from(2), false);
        let h3 = graph.get_handle(NodeId::from(3), false);
        let h4 = graph.get_handle(NodeId::from(4), false);
        let h5 = graph.get_handle(NodeId::from(5), false);
        let h6 = graph.get_handle(NodeId::from(6), false);

        // Add a path 1 -> 2 -> 5 -> 6

        let p1 = graph.create_path_handle("path-1", false);
        graph.append_step(&p1, h1);
        graph.append_step(&p1, h2);
        graph.append_step(&p1, h5);
        graph.append_step(&p1, h6);

        // Add another path 1 -> 3 -> 4 -> 6

        let p2 = graph.create_path_handle("path-2", false);
        graph.append_step(&p2, h1);
        graph.append_step(&p2, h3);
        graph.append_step(&p2, h4);
        graph.append_step(&p2, h6);

        graph.print_path(&p1);
        graph.print_path(&p2);
    }
}
