use fnv::FnvHashMap;

use gfa::{
    gfa::{Link, Segment, GFA},
    optfields::OptFields,
};

use crate::pathhandlegraph::MutableGraphPaths;
use crate::{
    handle::{Edge, Handle, NodeId},
    handlegraph::*,
    mutablehandlegraph::*,
    pathhandlegraph::PathId,
};

use super::{Node, Path};

/// A handlegraph implementation using `HashMap` to represent the
/// graph topology and nodes, and each path as a `Vec` of nodes.
#[derive(Debug)]
pub struct HashGraph {
    pub max_id: NodeId,
    pub min_id: NodeId,
    pub graph: FnvHashMap<NodeId, Node>,
    pub path_id: FnvHashMap<Vec<u8>, PathId>,
    pub paths: FnvHashMap<PathId, Path>,
}

impl Default for HashGraph {
    fn default() -> HashGraph {
        HashGraph {
            max_id: NodeId::from(0),
            min_id: NodeId::from(std::u64::MAX),
            graph: Default::default(),
            path_id: Default::default(),
            paths: Default::default(),
        }
    }
}

impl HashGraph {
    pub fn new() -> HashGraph {
        Default::default()
    }

    fn add_gfa_segment<'a, 'b, T: OptFields>(
        &'a mut self,
        seg: &'b Segment<usize, T>,
    ) {
        self.create_handle(&seg.sequence, seg.name as u64);
    }

    fn add_gfa_link<T: OptFields>(&mut self, link: &Link<usize, T>) {
        let left = Handle::new(link.from_segment as u64, link.from_orient);
        let right = Handle::new(link.to_segment as u64, link.to_orient);

        self.create_edge(Edge(left, right));
    }

    fn add_gfa_path<T: OptFields>(&mut self, path: &gfa::gfa::Path<usize, T>) {
        let path_id = self.create_path(&path.path_name, false).unwrap();
        for (name, orient) in path.iter() {
            self.path_append_step(path_id, Handle::new(name as u64, orient));
        }
    }

    pub fn from_gfa<T: OptFields>(gfa: &GFA<usize, T>) -> HashGraph {
        let mut graph = Self::new();
        gfa.segments.iter().for_each(|s| graph.add_gfa_segment(s));
        gfa.links.iter().for_each(|l| graph.add_gfa_link(l));
        gfa.paths.iter().for_each(|p| graph.add_gfa_path(p));
        graph
    }

    pub fn print_path(&self, path_id: &PathId) {
        let path = self.paths.get(&path_id).unwrap();
        println!("Path\t{}", path_id.0);
        for (ix, handle) in path.nodes.iter().enumerate() {
            let node = self.get_node(&handle.id()).unwrap();
            if ix != 0 {
                print!(" -> ");
            }
            let seq_str = std::str::from_utf8(&node.sequence).unwrap();
            print!("{}", seq_str);
        }

        println!();
    }

    pub fn print_occurrences(&self) {
        self.handles().for_each(|h| {
            let node = self.get_node(&h.id()).unwrap();
            let seq_str = std::str::from_utf8(&node.sequence).unwrap();
            println!("{} - {:?}", seq_str, node.occurrences);
        });
    }

    pub fn get_node(&self, node_id: &NodeId) -> Option<&Node> {
        self.graph.get(node_id)
    }

    pub fn get_node_unchecked(&self, node_id: &NodeId) -> &Node {
        self.graph.get(node_id).unwrap_or_else(|| {
            panic!("Tried getting a node that doesn't exist, ID: {:?}", node_id)
        })
    }

    pub fn get_node_mut(&mut self, node_id: &NodeId) -> Option<&mut Node> {
        self.graph.get_mut(node_id)
    }

    pub fn get_path(&self, path_id: &PathId) -> Option<&Path> {
        self.paths.get(path_id)
    }

    pub fn get_path_unchecked(&self, path_id: &PathId) -> &Path {
        self.paths
            .get(path_id)
            .unwrap_or_else(|| panic!("Tried to look up nonexistent path:"))
    }
}
