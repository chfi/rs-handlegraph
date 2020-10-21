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

const fn encode_dna_base(base: u8) -> u64 {
    match base {
        b'a' | b'A' => 0,
        b'c' | b'C' => 1,
        b'g' | b'G' => 2,
        b't' | b'T' => 3,
        _ => 4,
    }
}

const fn encoded_complement(val: u64) -> u64 {
    if val == 4 {
        4
    } else {
        3 - val
    }
}

const fn decode_dna_base(byte: u64) -> u8 {
    match byte {
        0 => b'A',
        1 => b'C',
        2 => b'G',
        3 => b'T',
        _ => b'N',
    }
}

impl Sequences {
    const SIZE: usize = 1;

    fn add_record(&mut self, ix: usize, seq: &[u8]) {
        let seq_ix = self.sequences.len();
        self.indices.set(ix, seq_ix as u64);
        self.lengths.set(ix, seq.len() as u64);
        seq.iter()
            .for_each(|&b| self.sequences.append(encode_dna_base(b)));
    }

    fn get_sequence(&self, ix: usize) -> Vec<u8> {
        let start = self.indices.get(ix) as usize;
        let len = self.lengths.get(ix) as usize;
        let mut seq = Vec::with_capacity(len);
        for i in 0..len {
            let base = self.sequences.get(start + i);
            seq.push(decode_dna_base(base));
        }
        seq
    }

    fn length(&self, ix: usize) -> usize {
        self.lengths.get(ix) as usize
    }
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

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct EdgeIx(pub usize);

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

impl EdgeLists {
    const RECORD_SIZE: usize = 2;
    fn append_record(&mut self, handle: Handle, next: usize) -> EdgeIx {
        self.edge_lists.append(handle.as_integer());
        self.edge_lists.append(next as u64);
        let ix = self.edge_lists.len() / Self::RECORD_SIZE;
        EdgeIx(ix)
    }

    fn get_record(&self, ix: EdgeIx) -> EdgeRecord {
        let ix = (ix.0 - 1) * Self::RECORD_SIZE;
        let handle = Handle::from_integer(self.edge_lists.get(ix));
        let next = EdgeIx(self.edge_lists.get(ix + 1) as usize);
        EdgeRecord { handle, next }
    }

    fn next(&self, rec: EdgeRecord) -> Option<EdgeRecord> {
        if rec.next.0 != 0 {
            Some(self.get_record(rec.next))
        } else {
            None
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

    fn handle_graph_ix(&self, handle: Handle) -> Option<GraphIx> {
        let id = handle.id();
        let GraphIx(index) = self.get_node_index(id)?;
        Some(GraphIx((index - 1) * GraphRecord::SIZE))
    }

    fn graph_seq_record_ix(&self, graph_ix: GraphIx) -> usize {
        let ix = graph_ix.0;
        (ix * Sequences::SIZE) / GraphRecord::SIZE
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

    pub fn has_node(&self, id: NodeId) -> bool {
        self.get_node_index(id).is_some()
    }

    pub fn create_handle(&mut self, sequence: &[u8], id: NodeId) -> Handle {
        assert!(!sequence.is_empty() && id != NodeId::from(0));

        // todo make sure the node doesn't already exist

        let graph_ix = self.push_node_record(id);
        let seq_ix = self.graph_seq_record_ix(graph_ix);

        self.sequences.add_record(seq_ix, sequence);

        Handle::pack(id, false)
    }

    pub fn append_handle(&mut self, sequence: &[u8]) -> Handle {
        let id = NodeId::from(self.max_id + 1);
        self.create_handle(sequence, id)
    }

    pub fn create_edge(&mut self, left: Handle, right: Handle) -> Option<()> {
        let left_g_ix = self.handle_graph_ix(left)?;
        let right_g_ix = self.handle_graph_ix(right)?;

        let left_edge_g_ix = if left.is_reverse() {
            GraphRecord::start_edges_ix(left_g_ix)
        } else {
            GraphRecord::end_edges_ix(left_g_ix)
        };

        let right_edge_g_ix = if right.is_reverse() {
            GraphRecord::end_edges_ix(right_g_ix)
        } else {
            GraphRecord::start_edges_ix(right_g_ix)
        };

        let right_next = self.node_records.get(left_edge_g_ix);
        let edge_ix = self.edges.append_record(right, right_next as usize);

        self.node_records.set(left_edge_g_ix, edge_ix.0 as u64);

        if left_edge_g_ix == right_edge_g_ix {
            // todo reversing self edge records?
            return Some(());
        }

        let left_next = self.node_records.get(right_edge_g_ix);
        let edge_ix = self.edges.append_record(left.flip(), left_next as usize);

        self.node_records.set(right_edge_g_ix, edge_ix.0 as u64);

        Some(())
    }

    pub fn sequence(&self, handle: Handle) -> Vec<u8> {
        let graph_ix = self.handle_graph_ix(handle).unwrap();
        let seq_ix = self.graph_seq_record_ix(graph_ix);
        let seq = self.sequences.get_sequence(seq_ix);

        if handle.is_reverse() {
            dna::revcomp(seq)
        } else {
            seq
        }
    }

    pub fn length(&self, handle: Handle) -> usize {
        let graph_ix = self.handle_graph_ix(handle).unwrap();
        let seq_ix = self.graph_seq_record_ix(graph_ix);
        self.sequences.length(seq_ix)
    }
}
