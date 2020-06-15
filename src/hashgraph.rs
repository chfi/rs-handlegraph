use std::collections::HashMap;

use gfa::gfa::{Link, Segment, GFA};

use crate::handle::{Direction, Edge, Handle, NodeId};
use crate::handlegraph::{handle_edges_iter, handle_iter, HandleGraph};
use crate::pathgraph::PathHandleGraph;

pub type PathId = i64;

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
pub struct Node {
    pub sequence: String,
    pub left_edges: Vec<Handle>,
    pub right_edges: Vec<Handle>,
    pub occurrences: HashMap<PathId, usize>,
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
pub struct Path {
    pub path_id: PathId,
    pub name: String,
    pub is_circular: bool,
    pub nodes: Vec<Handle>,
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
    pub max_id: NodeId,
    pub min_id: NodeId,
    pub graph: HashMap<NodeId, Node>,
    pub path_id: HashMap<String, i64>,
    pub paths: HashMap<i64, Path>,
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

            let left = Handle::pack(left_id, !link.from_orient.as_bool());
            let right = Handle::pack(right_id, !link.to_orient.as_bool());

            graph.create_edge(&left, &right);
        }

        // add paths
        for path in gfa.paths.iter() {
            let path: &gfa::gfa::Path = path;
            let path_id = graph.create_path_handle(&path.path_name, false);
            for (name, orient) in path.segment_names.iter() {
                let id = name.parse::<u64>().unwrap();
                graph.append_step(&path_id, Handle::pack(id, orient.as_bool()));
            }
        }

        graph
    }

    pub fn print_path(&self, path_id: &PathId) {
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

    pub fn print_occurrences(&self) {
        self.for_each_handle(|h| {
            let node = self.get_node(&h.id()).unwrap();
            println!("{} - {:?}", node.sequence, node.occurrences);
            true
        });
    }

    pub fn get_node(&self, node_id: &NodeId) -> Option<&Node> {
        self.graph.get(node_id)
    }

    pub fn get_node_unsafe(&self, node_id: &NodeId) -> &Node {
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

    fn edges_iter_impl<'a>(&'a self) -> Box<dyn FnMut() -> Option<Edge> + 'a> {
        let handles = std::iter::from_fn(self.handle_iter_impl());

        let neighbors = move |handle: Handle| {
            let right_neighbors = std::iter::from_fn(
                self.handle_edges_iter_impl(handle, Direction::Right),
            )
            .filter_map(move |next| {
                if handle.id() <= next.id() {
                    Some(Edge::edge_handle(&handle, &next))
                } else {
                    None
                }
            });

            let left_neighbors = std::iter::from_fn(
                self.handle_edges_iter_impl(handle, Direction::Left),
            )
            .filter_map(move |prev| {
                if (handle.id() < prev.id())
                    || (handle.id() == prev.id() && prev.is_reverse())
                {
                    Some(Edge::edge_handle(&prev, &handle))
                } else {
                    None
                }
            });

            right_neighbors.chain(left_neighbors)
        };

        let mut edges = handles.map(neighbors).flatten();

        Box::new(move || edges.next())
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

    fn next_step(&self, step: &Self::StepHandle) -> Self::StepHandle {
        match step {
            PathStep::Front(pid) => self.path_begin(pid),
            PathStep::End(pid) => self.path_end(pid),
            PathStep::Step(pid, ix) => {
                if *ix < self.get_step_count(pid) - 1 {
                    PathStep::Step(*pid, ix + 1)
                } else {
                    self.path_end(pid)
                }
            }
        }
    }

    fn previous_step(&self, step: &Self::StepHandle) -> Self::StepHandle {
        match step {
            PathStep::Front(pid) => self.path_front_end(pid),
            PathStep::End(pid) => self.path_back(pid),
            PathStep::Step(pid, ix) => {
                if *ix > 0 {
                    PathStep::Step(*pid, ix - 1)
                } else {
                    self.path_end(pid)
                }
            }
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

    fn for_each_path_handle<F>(&self, mut f: F) -> bool
    where
        F: FnMut(&PathId) -> bool,
    {
        for ph in self.paths.keys() {
            if !f(&ph) {
                return false;
            }
        }
        true
    }

    fn for_each_step_on_handle<F>(&self, handle: &Handle, mut f: F) -> bool
    where
        F: FnMut(&PathStep) -> bool,
    {
        let node: &Node = self.get_node_unsafe(&handle.id());
        for (path, ix) in node.occurrences.iter() {
            if !f(&PathStep::Step(*path, *ix)) {
                return false;
            }
        }
        true
    }

    fn paths_iter_impl<'a>(
        &'a self,
    ) -> Box<dyn FnMut() -> Option<&'a Self::PathHandle> + 'a> {
        let mut iter = self.paths.keys();

        Box::new(move || iter.next())
    }

    fn handle_occurrences_iter<'a>(
        &'a self,
        handle: &Handle,
    ) -> Box<dyn FnMut() -> Option<Self::StepHandle> + 'a> {
        let node: &Node = self.get_node_unsafe(&handle.id());

        let mut iter =
            node.occurrences.iter().map(|(k, v)| PathStep::Step(*k, *v));

        Box::new(move || iter.next())
    }
}

pub fn paths_iter<'a, T: PathHandleGraph>(
    graph: &'a T,
) -> impl Iterator<Item = &'a <T as PathHandleGraph>::PathHandle> + 'a {
    std::iter::from_fn(graph.paths_iter_impl())
}

pub fn occurrences_iter<'a, T: PathHandleGraph>(
    graph: &'a T,
    handle: &Handle,
) -> impl Iterator<Item = <T as PathHandleGraph>::StepHandle> + 'a {
    std::iter::from_fn(graph.handle_occurrences_iter(handle))
}
