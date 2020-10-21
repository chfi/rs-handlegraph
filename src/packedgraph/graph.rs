use bio::alphabets::dna;
use bstr::BString;
use fnv::FnvHashMap;

use gfa::{
    gfa::{Link, Orientation, Segment, GFA},
    optfields::OptFields,
};

use crate::{
    handle::{Edge, Handle, NodeId},
    handlegraph::HandleGraph,
    mutablehandlegraph::MutableHandleGraph,
    packed::*,
    pathgraph::PathHandleGraph,
};

static NARROW_PAGE_WIDTH: usize = 256;
static WIDE_PAGE_WIDTH: usize = 1024;

#[derive(Debug, Clone)]
pub struct Sequences {
    sequences: PackedIntVec,
    lengths: PackedIntVec,
    indices: PagedIntVec,
}

impl Sequences {
    const SIZE: usize = 1;
}

impl Default for Sequences {
    fn default() -> Self {
        Sequences {
            sequences: Default::default(),
            lengths: Default::default(),
            indices: PagedIntVec::new(NARROW_PAGE_WIDTH),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct EdgeRecord {
    handle: Handle,
    next: EdgeIx,
}

#[derive(Debug, Clone)]
pub struct EdgeLists {
    edge_lists: PagedIntVec,
}

impl Default for EdgeLists {
    fn default() -> Self {
        EdgeLists {
            edge_lists: PagedIntVec::new(WIDE_PAGE_WIDTH),
        }
    }
}

#[derive(Default, Debug, Clone)]
pub struct GraphRecord {
    edges_start: usize,
    edges_end: usize,
}

impl GraphRecord {
    const SIZE: usize = 2;
    const START_OFFSET: usize = 0;
    const END_OFFSET: usize = 1;

    fn start_edges_ix(g_ix: GraphIx) -> usize {
        let ix = g_ix.0;
        g_ix.0 + Self::START_OFFSET
    }

    fn end_edges_ix(g_ix: GraphIx) -> usize {
        let ix = g_ix.0;
        g_ix.0 + Self::END_OFFSET
    }
}

#[derive(Debug, Clone)]
pub struct Graph {
    node_records: PagedIntVec,
    sequences: Sequences,
    edges: EdgeLists,
    id_graph_map: PackedDeque,
    max_id: u64,
    min_id: u64,
}

impl Default for Graph {
    fn default() -> Self {
        let sequences = Default::default();
        let edges = Default::default();
        let id_graph_map = Default::default();
        let node_records = PagedIntVec::new(NARROW_PAGE_WIDTH);
        let max_id = 0;
        let min_id = std::u64::MAX;
        Graph {
            sequences,
            edges,
            node_records,
            id_graph_map,
            max_id,
            min_id,
        }
    }
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct GraphIx(pub usize);

impl Graph {
    pub fn new() -> Self {
        Default::default()
    }

    fn new_record_ix(&mut self) -> GraphIx {
        let new_ix = self.node_records.len();
        self.node_records.append(0);
        self.node_records.append(0);
        self.sequences.lengths.append(0);
        self.sequences.indices.append(0);
        GraphIx(new_ix)
    }

    fn get_node_index(&self, id: NodeId) -> Option<GraphIx> {
        let id = u64::from(id);
        if id < self.min_id || id > self.max_id {
            return None;
        }
        let map_ix = id - self.min_id;
        let ix = self.id_graph_map.get(map_ix as usize);
        if ix == 0 {
            None
        } else {
            Some(GraphIx((ix - 1) as usize))
        }
    }

    fn push_node_record(&mut self, id: NodeId) -> GraphIx {
        let next_ix = self.new_record_ix();

        if self.id_graph_map.is_empty() {
            self.id_graph_map.push_back(0);
        } else {
            let id = u64::from(id);
            if id < self.min_id {
                let to_prepend = self.min_id - id;
                for _ in 0..to_prepend {
                    self.id_graph_map.push_front(0);
                }
            }
            if id > self.max_id {
                let to_append =
                    self.id_graph_map.len() - (id - self.min_id) as usize;
                for _ in 0..to_append {
                    self.id_graph_map.push_back(0);
                }
            }
        }

        let id = u64::from(id);

        self.min_id = self.min_id.min(id);
        self.max_id = self.max_id.max(id);

        let index = id - self.min_id;
        let value = self.node_records.len();

        self.id_graph_map.set(index as usize, value as u64);

        next_ix
    }
}
