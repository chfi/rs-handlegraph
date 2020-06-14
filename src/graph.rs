use std::collections::HashMap;

use gfa::gfa::{Link, Segment, GFA};

use crate::handle::{Direction, Edge, Handle, NodeId};
use crate::handlegraph::{handle_edges_iter, handle_iter, HandleGraph};
use crate::pathgraph::PathHandleGraph;

type PathId = i64;

#[derive(Debug, Clone, PartialEq)]
pub enum PathStep {
    Front(i64),
    End(i64),
    Step(i64, usize),
}

impl PathStep {
    pub fn index(&self) -> Option<usize> {
        if let Self::Step(_, ix) = self {
            Some(*ix)
        } else {
            None
        }
    }

    pub fn path_id(&self) -> PathId {
        match self {
            Self::Front(i) => *i,
            Self::End(i) => *i,
            Self::Step(i, _) => *i,
        }
    }
}

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

    fn lookup_step_handle(&self, step: &PathStep) -> Option<Handle> {
        match step {
            PathStep::Front(_) => None,
            PathStep::End(_) => None,
            PathStep::Step(_, ix) => Some(self.nodes[*ix]),
        }
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

        // add links
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

        // add paths
        for path in gfa.paths.iter() {
            let path: &gfa::gfa::Path = path;
            let path_id = graph.create_path_handle(&path.path_name, false);
            for segment in path.segment_names.iter() {
                let split = segment.split_at(segment.len() - 1);
                let id = split.0.parse::<u64>().unwrap();
                let dir = char::from(split.1.as_bytes()[0]) == '+';
                graph
                    .append_step(&path_id, Handle::pack(NodeId::from(id), dir));
            }
        }

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

    fn print_occurrences(&self) {
        self.for_each_handle(|h| {
            let node = self.get_node(&h.id()).unwrap();
            println!("{} - {:?}", node.sequence, node.occurrences);
            true
        });
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

    fn get_edge_count(&self) -> usize {
        self.graph
            .iter()
            .fold(0, |a, (_, v)| a + v.left_edges.len() + v.right_edges.len())
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

    fn for_each_edge<F>(&self, mut f: F) -> bool
    where
        F: FnMut(&Edge) -> bool,
    {
        self.for_each_handle(|handle| {
            let mut keep_going = true;

            self.follow_edges(handle, Direction::Right, |next| {
                if handle.id() <= next.id() {
                    keep_going = f(&Edge::edge_handle(handle, next));
                }
                keep_going
            });

            if keep_going {
                self.follow_edges(handle, Direction::Left, |prev| {
                    if handle.id() < prev.id()
                        || (handle.id() == prev.id() && prev.is_reverse())
                    {
                        keep_going = f(&Edge::edge_handle(prev, handle));
                    }
                    keep_going
                });
            }

            keep_going
        })
    }

    fn handle_edges_iter_impl<'a>(
        &'a self,
        handle: Handle,
        dir: Direction,
    ) -> Box<dyn FnMut() -> Option<Handle> + 'a> {
        let node = self.get_node_unsafe(&handle.id());
        let handles = if handle.is_reverse() != (dir == Direction::Left) {
            &node.left_edges
        } else {
            &node.right_edges
        };

        let mut iter = handles.iter();

        Box::new(move || iter.next().map(|i| i.clone()))
    }

    fn handle_iter_impl<'a>(
        &'a self,
    ) -> Box<dyn FnMut() -> Option<Handle> + 'a> {
        let mut iter = self.graph.keys().map(|i| Handle::pack(*i, false));

        Box::new(move || iter.next())
    }
}

impl HashGraph {
    pub fn create_handle(&mut self, sequence: &str, node_id: NodeId) -> Handle {
        self.graph.insert(node_id, Node::new(sequence));
        self.max_id = std::cmp::max(self.max_id, node_id);
        self.min_id = std::cmp::min(self.min_id, node_id);
        Handle::pack(node_id, false)
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

    fn get_handle_of_step(&self, step: &Self::StepHandle) -> Option<Handle> {
        self.paths
            .get(&step.path_id())
            .unwrap()
            .lookup_step_handle(step)
    }

    fn get_path_handle_of_step(
        &self,
        step: &Self::StepHandle,
    ) -> Self::PathHandle {
        step.path_id()
    }

    fn path_begin(&self, path: &Self::PathHandle) -> Self::StepHandle {
        PathStep::Step(*path, 0)
    }

    fn path_end(&self, path: &Self::PathHandle) -> Self::StepHandle {
        PathStep::End(*path)
    }

    fn path_back(&self, path: &Self::PathHandle) -> Self::StepHandle {
        PathStep::Step(*path, self.get_step_count(path) - 1)
    }

    fn path_front_end(&self, path: &Self::PathHandle) -> Self::StepHandle {
        PathStep::Front(*path)
    }

    fn has_next_step(&self, step: &Self::StepHandle) -> bool {
        // TODO this might be an off-by-one error
        if let PathStep::End(_) = step {
            false
        } else {
            true
        }
    }

    fn has_previous_step(&self, step: &Self::StepHandle) -> bool {
        if let PathStep::Front(_) = step {
            false
        } else {
            true
        }
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
        self.path_id.insert(name.to_string(), path_id);
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
        let step = (*path_id, path.nodes.len() - 1);
        let node: &mut Node = self.graph.get_mut(&to_append.id()).unwrap();
        node.occurrences.insert(step.0, step.1);
        PathStep::Step(*path_id, path.nodes.len() - 1)
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
        node.occurrences.insert(*path_id, 0);
        PathStep::Step(*path_id, 0)
    }

    fn rewrite_segment(
        &mut self,
        begin: &Self::StepHandle,
        end: &Self::StepHandle,
        new_segment: Vec<Handle>,
    ) -> (Self::StepHandle, Self::StepHandle) {
        // extract the index range from the begin and end handles

        if begin.path_id() != end.path_id() {
            panic!("Tried to rewrite path segment between two different paths");
        }

        let path_id = begin.path_id();
        let path_len = self.paths.get(&path_id).unwrap().nodes.len();

        let step_index = |s: &Self::StepHandle| match s {
            PathStep::Front(_) => 0,
            PathStep::End(_) => path_len - 1,
            PathStep::Step(_, i) => *i,
        };

        let l = step_index(begin);
        let r = step_index(end);

        let range = l..=r;

        // first delete the occurrences of the nodes in the range
        for handle in self
            .paths
            .get(&path_id)
            .unwrap()
            .nodes
            .iter()
            .skip(l)
            .take(r - l + 1)
        {
            let node: &mut Node = self.graph.get_mut(&handle.id()).unwrap();
            node.occurrences.remove(&path_id);
        }

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
            node.occurrences.insert(path_id, ix);
        }

        // return the new beginning and end step handles: even if the
        // input steps were Front and/or End, the output steps exist
        // on the path
        (PathStep::Step(path_id, l), PathStep::Step(path_id, r))
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
    fn graph_follow_edges() {
        let mut graph = path_graph();
        let h1 = Handle::pack(NodeId::from(1), false);
        let h2 = Handle::pack(NodeId::from(2), false);
        let h3 = Handle::pack(NodeId::from(3), false);
        let h4 = Handle::pack(NodeId::from(4), false);
        let h5 = Handle::pack(NodeId::from(5), false);
        let h6 = Handle::pack(NodeId::from(6), false);

        // add some more edges to make things interesting

        graph.create_edge(&h1, &h4);
        graph.create_edge(&h1, &h6);

        let mut h1_edges_r = vec![];

        graph.follow_edges(&h1, Direction::Right, |&h| {
            h1_edges_r.push(h);
            true
        });

        assert_eq!(h1_edges_r, vec![h2, h3, h4, h6]);

        let mut h4_edges_l = vec![];
        let mut h4_edges_r = vec![];

        graph.follow_edges(&h4, Direction::Left, |&h| {
            h4_edges_l.push(h);
            true
        });

        graph.follow_edges(&h4, Direction::Right, |&h| {
            h4_edges_r.push(h);
            true
        });

        assert_eq!(h4_edges_l, vec![h3, h1]);
        assert_eq!(h4_edges_r, vec![h6]);
    }

    #[test]
    fn graph_handle_edges_iter() {
        let mut graph = path_graph();
        let h1 = Handle::pack(NodeId::from(1), false);
        let h2 = Handle::pack(NodeId::from(2), false);
        let h3 = Handle::pack(NodeId::from(3), false);
        let h4 = Handle::pack(NodeId::from(4), false);
        let h5 = Handle::pack(NodeId::from(5), false);
        let h6 = Handle::pack(NodeId::from(6), false);

        graph.create_edge(&h1, &h4);
        graph.create_edge(&h1, &h6);

        let mut iter = handle_edges_iter(&graph, h1, Direction::Right);

        assert_eq!(Some(h2), iter.next());
        assert_eq!(Some(h3), iter.next());
        assert_eq!(Some(h4), iter.next());
        assert_eq!(Some(h6), iter.next());
        assert_eq!(None, iter.next());
    }

    #[test]
    fn graph_handle_iter() {
        let mut graph = path_graph();
        let h1 = Handle::pack(NodeId::from(1), false);
        let h2 = Handle::pack(NodeId::from(2), false);
        let h3 = Handle::pack(NodeId::from(3), false);
        let h4 = Handle::pack(NodeId::from(4), false);
        let h5 = Handle::pack(NodeId::from(5), false);
        let h6 = Handle::pack(NodeId::from(6), false);

        let iter = handle_iter(&graph);

        let nodes: Vec<_> = vec![h1, h2, h3, h4, h5, h6]
            .into_iter()
            .map(|x| x.id())
            .collect();

        let mut iter_nodes: Vec<NodeId> = vec![];

        for h in iter {
            iter_nodes.push(h.id())
        }

        assert!(iter_nodes.iter().all(|n| graph.get_node(n).is_some()));
        assert!(nodes.iter().all(|n| iter_nodes.contains(n)));
    }

    #[test]
    fn graph_for_each_edge() {
        let mut graph = path_graph();
        let h1 = Handle::pack(NodeId::from(1), false);
        let h2 = Handle::pack(NodeId::from(2), false);
        let h3 = Handle::pack(NodeId::from(3), false);
        let h4 = Handle::pack(NodeId::from(4), false);
        let h5 = Handle::pack(NodeId::from(5), false);
        let h6 = Handle::pack(NodeId::from(6), false);

        graph.create_edge(&h1, &h4);
        graph.create_edge(&h1, &h6);

        graph.create_edge(&h4, &h2);
        graph.create_edge(&h6, &h2);

        graph.create_edge(&h3, &h5);

        /* The graph looks like:
               v--------\
        1   -> 2 -> 5 -> 6
        |\     ^-/--^   ^^
        \ \     /\--   / |
         \ \-> 3 -> 4-/  |
          ----------^   /
           \-----------/

        Right edges:
        1 -> [2, 3, 4, 6]
        2 -> [5]
        3 -> [4, 5]
        4 -> [6]
        5 -> [6]
        6 -> []

        Left edges:
        4 -> [2]
        6 -> [2]
         */

        let mut edges: Vec<_> = vec![
            Edge::edge_handle(&h1, &h2),
            Edge::edge_handle(&h1, &h3),
            Edge::edge_handle(&h1, &h4),
            Edge::edge_handle(&h1, &h6),
            Edge::edge_handle(&h2, &h5),
            Edge::edge_handle(&h4, &h2),
            Edge::edge_handle(&h6, &h2),
            Edge::edge_handle(&h3, &h4),
            Edge::edge_handle(&h3, &h5),
            Edge::edge_handle(&h4, &h6),
            Edge::edge_handle(&h5, &h6),
        ];

        edges.sort();

        let mut edges_found: Vec<_> = Vec::new();

        graph.for_each_edge(|e| {
            let Edge(hl, hr) = e;
            edges_found.push(e.clone());
            let nl = hl.id();
            let nr = hr.id();
            println!("{:?} -> {:?}", nl, nr);
            true
        });

        edges_found.sort();

        assert_eq!(edges, edges_found);
    }

    #[test]
    fn append_prepend_path() {
        let mut graph = path_graph();

        let h1 = Handle::pack(NodeId::from(1), false);
        let h2 = Handle::pack(NodeId::from(2), false);
        let h3 = Handle::pack(NodeId::from(3), false);
        let h4 = Handle::pack(NodeId::from(4), false);
        let h5 = Handle::pack(NodeId::from(5), false);
        let h6 = Handle::pack(NodeId::from(6), false);

        // Add a path 3 -> 5

        let p1 = graph.create_path_handle("path-1", false);
        graph.append_step(&p1, h3);
        graph.append_step(&p1, h5);

        // Add another path 1 -> 3 -> 4 -> 6

        let p2 = graph.create_path_handle("path-2", false);
        graph.append_step(&p2, h1);
        let p2_3 = graph.append_step(&p2, h3);
        let p2_4 = graph.append_step(&p2, h4);
        graph.append_step(&p2, h6);

        let test_node = |graph: &HashGraph,
                         nid: u64,
                         o1: Option<&usize>,
                         o2: Option<&usize>| {
            let n = graph.get_node(&NodeId::from(nid)).unwrap();
            assert_eq!(o1, n.occurrences.get(&p1));
            assert_eq!(o2, n.occurrences.get(&p2));
        };

        // At this point, node 3 should have two occurrences entries,
        // index 0 for path 1, index 1 for path 2
        test_node(&graph, 3, Some(&0), Some(&1));

        // Node 1 should have only one occurrence at the start of path 2
        test_node(&graph, 1, None, Some(&0));

        // Node 6 should have only one occurrence at the end of path 2
        test_node(&graph, 6, None, Some(&3));

        // Now, append node 6 to path 1

        graph.append_step(&p1, h6);

        // Node 6 should also occur at the end of path 1
        test_node(&graph, 6, Some(&2), Some(&3));

        // The other nodes should be unaffected
        test_node(&graph, 1, None, Some(&0));
        test_node(&graph, 4, None, Some(&2));

        test_node(&graph, 3, Some(&0), Some(&1));
        test_node(&graph, 5, Some(&1), None);

        // Now, prepend node 1 to path 1
        graph.prepend_step(&p1, h1);

        // Node 1 should be the first in both paths
        test_node(&graph, 1, Some(&0), Some(&0));

        // The other nodes should have had 1 added to their
        // occurrences in path 1, while the path 2 ones should be the
        // same
        test_node(&graph, 3, Some(&1), Some(&1));
        test_node(&graph, 5, Some(&2), None);
        test_node(&graph, 6, Some(&3), Some(&3));

        test_node(&graph, 4, None, Some(&2));

        // At this point path 1 is 1 -> 3 -> 5 -> 6, path 2 is unmodified
        // Rewrite the segment 3 -> 4 in path 2 with the empty path
        graph.rewrite_segment(&p2_3, &p2_4, vec![]);

        // Node 1 should be the same
        test_node(&graph, 1, Some(&0), Some(&0));

        // Node 6 should have been decremented by 2 in path 2
        test_node(&graph, 6, Some(&3), Some(&1));

        // Nodes 3, 4 should be empty in path 2
        test_node(&graph, 3, Some(&1), None);
        test_node(&graph, 4, None, None);

        // Rewrite the segment 1 -> 6 in path 2 with the segment
        // 6 -> 4 -> 5 -> 3 -> 1 -> 2
        graph.rewrite_segment(
            &PathStep::Step(1, 0),
            &PathStep::Step(1, 1),
            vec![h6, h4, h5, h3, h1, h2],
        );

        // The path 2 occurrences should be correctly updated for all nodes
        test_node(&graph, 1, Some(&0), Some(&4));
        test_node(&graph, 2, None, Some(&5));
        test_node(&graph, 3, Some(&1), Some(&3));
        test_node(&graph, 4, None, Some(&1));
        test_node(&graph, 5, Some(&2), Some(&2));
        test_node(&graph, 6, Some(&3), Some(&0));

        // Rewrite the segment Front(_) .. 5 in path 1 with the segment [2, 3]
        graph.rewrite_segment(
            &PathStep::Front(0),
            &PathStep::Step(0, 2),
            vec![h2, h3],
        );

        // Now path 1 is 2 -> 3 -> 6
        test_node(&graph, 1, None, Some(&4));
        test_node(&graph, 2, Some(&0), Some(&5));
        test_node(&graph, 3, Some(&1), Some(&3));
        test_node(&graph, 5, None, Some(&2));
        test_node(&graph, 6, Some(&2), Some(&0));

        // Rewrite the segment 3 .. End(_) in path 2 with the segment [1]
        graph.rewrite_segment(
            &PathStep::Step(1, 3),
            &PathStep::End(1),
            vec![h1],
        );

        // Now path 2 is 6 -> 4 -> 5 -> 1
        test_node(&graph, 1, None, Some(&3));
        test_node(&graph, 2, Some(&0), None);
        test_node(&graph, 3, Some(&1), None);
        test_node(&graph, 4, None, Some(&1));
        test_node(&graph, 5, None, Some(&2));
        test_node(&graph, 6, Some(&2), Some(&0));

        graph.print_path(&p1);
        graph.print_path(&p2);

        graph.print_occurrences();
    }
}
