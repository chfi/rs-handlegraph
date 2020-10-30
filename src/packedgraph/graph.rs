use gfa::{
    gfa::{Link, Orientation, Segment, GFA},
    optfields::OptFields,
};

use crate::{
    handle::{Direction, Edge, Handle, NodeId},
    handlegraph::HandleGraph,
    mutablehandlegraph::MutableHandleGraph,
    packed::*,
};

static NARROW_PAGE_WIDTH: usize = 256;
static WIDE_PAGE_WIDTH: usize = 1024;

#[derive(Debug, Clone)]
pub struct Sequences {
    sequences: PackedIntVec,
    pub(super) lengths: PackedIntVec,
    pub(super) indices: PagedIntVec,
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

    pub(super) fn add_record(&mut self, ix: usize, seq: &[u8]) {
        let seq_ix = self.sequences.len();
        self.indices.set(ix, seq_ix as u64);
        self.lengths.set(ix, seq.len() as u64);
        seq.iter()
            .for_each(|&b| self.sequences.append(encode_dna_base(b)));
    }

    #[inline]
    pub(super) fn length(&self, ix: usize) -> usize {
        self.lengths.get(ix) as usize
    }

    #[inline]
    pub(super) fn total_length(&self) -> usize {
        self.lengths.iter().sum::<u64>() as usize
    }

    #[inline]
    pub(super) fn base(&self, seq_ix: usize, base_ix: usize) -> u8 {
        let len = self.lengths.get(seq_ix) as usize;
        assert!(base_ix < len);
        let offset = self.indices.get(seq_ix) as usize;
        let base = self.sequences.get(offset + base_ix);
        decode_dna_base(base)
    }

    pub(super) fn iter(
        &self,
        seq_ix: usize,
        reverse: bool,
    ) -> PackedSeqIter<'_> {
        let offset = self.indices.get(seq_ix) as usize;
        let len = self.lengths.get(seq_ix) as usize;

        let iter = self.sequences.iter_slice(offset, len);

        PackedSeqIter {
            iter,
            length: len,
            reverse,
        }
    }

    pub(super) fn divide_sequence(
        &mut self,
        seq_ix: usize,
        lengths: Vec<usize>,
    ) -> Vec<(usize, usize)> {
        let mut results = Vec::new();

        let offset = self.indices.get(seq_ix) as usize;
        let len = self.lengths.get(seq_ix) as usize;

        let indices = lengths
            .iter()
            .copied()
            .map(|l| l + offset)
            .collect::<Vec<_>>();

        // create new records
        for (&i, &l) in indices.iter().skip(1).zip(lengths.iter().skip(1)) {
            self.lengths.append(l as u64);
            self.indices.append((i - 1) as u64);
            results.push((i, l));
        }

        // update the original sequence
        self.lengths.set(seq_ix, lengths[0] as u64);

        results
    }
}

pub struct PackedSeqIter<'a> {
    iter: PackedIntVecIter<'a>,
    length: usize,
    reverse: bool,
}

impl<'a> Iterator for PackedSeqIter<'a> {
    type Item = u8;

    fn next(&mut self) -> Option<u8> {
        if self.reverse {
            let base = self.iter.next_back()?;
            Some(decode_dna_base(encoded_complement(base)))
        } else {
            let base = self.iter.next()?;
            Some(decode_dna_base(base))
        }
    }
}

impl<'a> std::iter::ExactSizeIterator for PackedSeqIter<'a> {
    fn len(&self) -> usize {
        self.length
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
    pub handle: Handle,
    next: EdgeIx,
}

/// A newtype for indexing into the EdgeLists *by record*.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct EdgeIx(pub(super) usize);

impl EdgeIx {
    #[inline]
    pub(super) fn from_edge_list_ix(ix: usize) -> Self {
        EdgeIx((ix / EdgeLists::RECORD_SIZE) + 1)
    }

    #[inline]
    pub(super) fn to_edge_list_ix(&self) -> usize {
        if self.0 == 0 {
            0
        } else {
            (self.0 - 1) * EdgeLists::RECORD_SIZE
        }
    }
}

#[derive(Debug, Clone)]
pub struct EdgeLists {
    pub(super) edge_lists: PagedIntVec,
}

impl Default for EdgeLists {
    fn default() -> Self {
        EdgeLists {
            edge_lists: PagedIntVec::new(WIDE_PAGE_WIDTH),
        }
    }
}

impl EdgeLists {
    pub(super) const RECORD_SIZE: usize = 2;
    pub(super) fn append_record(
        &mut self,
        handle: Handle,
        next: EdgeIx,
    ) -> EdgeIx {
        let ix = EdgeIx::from_edge_list_ix(self.edge_lists.len());
        self.edge_lists.append(handle.as_integer());
        let next = next.0;
        self.edge_lists.append(next as u64);
        ix
    }

    pub(super) fn null_record(&mut self, ix: EdgeIx) {
        let ix = ix.to_edge_list_ix();
        self.edge_lists.set(ix, 0);
        self.edge_lists.set(ix + 1, 0);
    }

    #[inline]
    pub(super) fn get_record(&self, ix: EdgeIx) -> Option<EdgeRecord> {
        if ix == EdgeIx(0) {
            None
        } else {
            let ix = ix.to_edge_list_ix();
            let handle = Handle::from_integer(self.edge_lists.get(ix));
            let next = EdgeIx(self.edge_lists.get(ix + 1) as usize);
            Some(EdgeRecord { handle, next })
        }
    }

    pub(super) fn update_record<F>(&mut self, ix: EdgeIx, f: F)
    where
        F: Fn(EdgeRecord),
    {
        let record = self.get_record(ix);
        let new_record = f(record);
        let ix = ix.to_edge_list_ix();
        self.edge_lists.set(ix, new_record.handle);
        self.edge_lists.set(ix + 1, new_record.next);
    }

    #[inline]
    pub(super) fn get_target_at(&self, ix: EdgeIx) -> Option<Handle> {
        if ix == EdgeIx(0) {
            None
        } else {
            let ix = ix.to_edge_list_ix();
            let handle = Handle::from_integer(self.edge_lists.get(ix));
            Some(handle)
        }
    }

    #[inline]
    pub(super) fn get_next_at(&self, ix: EdgeIx) -> Option<EdgeIx> {
        if ix == EdgeIx(0) {
            None
        } else {
            let ix = ix.to_edge_list_ix() + 1;
            let next = self.edge_lists.get(ix);
            if next == 0 {
                None
            } else {
                Some(EdgeIx(next as usize))
            }
        }
    }

    #[inline]
    pub(super) fn next(&self, rec: EdgeRecord) -> Option<EdgeRecord> {
        self.get_record(rec.next)
    }

    pub(super) fn iter(&self, ix: EdgeIx) -> EdgeListIter<'_> {
        EdgeListIter::new(self, ix)
    }

    /// Removes the record for a given handle from the edge list
    /// starting at the provided index. If the handle exists in the
    /// edge list, returns the index that took its place, i.e. if the
    /// corresponding graph record needs to be updated.
    pub(super) fn remove_handle_from_list(
        &mut self,
        ix: EdgeIx,
        handle: Handle,
    ) -> Option<EdgeIx> {
        let h_ix = self.iter(ix).position(|(_, r)| r.handle == handle)?;

        if h_ix == 0 {
            // Only remove the record in question
            let next = self.get_record(ix)?.next;
            self.null_record(ix);
            Some(next)
        } else {
            let (h_edge_ix, h_record) = self.iter(ix).nth(h_ix)?;

            // Remove the record and set the preceding record's
            // pointer to the record in question's next pointer
            let (prec_ix, prec_rec) = self.iter(ix).nth(h_ix - 1)?;
            let prec_next_ix = prec_rec.next.to_edge_list_ix();

            let e_ix = h_record.next;
            let prec_ix = prec_ix.to_edge_list_ix() + 1;
            self.edge_lists.set(prec_ix, h_record.next.0 as u64);
            self.null_record(h_edge_ix);
            Some(ix)
        }
    }
}

/// Iterator for stepping through an edge list.
pub struct EdgeListIter<'a> {
    edge_lists: &'a EdgeLists,
    current: Option<(EdgeIx, EdgeRecord)>,
    finished: bool,
}

impl<'a> EdgeListIter<'a> {
    fn new(edge_lists: &'a EdgeLists, start: EdgeIx) -> Self {
        let cur_rec = edge_lists.get_record(start);
        let current = cur_rec.map(|r| (start, r));
        Self {
            edge_lists,
            current,
            finished: false,
        }
    }
}

impl<'a> Iterator for EdgeListIter<'a> {
    type Item = (EdgeIx, EdgeRecord);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if self.finished {
            return None;
        }
        if let Some((cur_ix, cur_rec)) = self.current {
            let item = (cur_ix, cur_rec);
            let next_rec = self.edge_lists.next(cur_rec);
            let next_ix = cur_rec.next;
            self.current = next_rec.map(|r| (next_ix, r));
            Some(item)
        } else {
            self.finished = true;
            None
        }
    }
}

#[derive(Default, Debug, Clone)]
pub struct GraphRecord {
    left_edges: usize,
    right_edges: usize,
}

impl GraphRecord {
    pub(super) const SIZE: usize = 2;
    pub(super) const START_OFFSET: usize = 0;
    pub(super) const END_OFFSET: usize = 1;
}

#[derive(Debug, Clone)]
pub struct PackedGraph {
    pub(super) graph_records: PagedIntVec,
    pub(super) sequences: Sequences,
    pub(super) edges: EdgeLists,
    pub(super) id_graph_map: PackedDeque,
    pub(super) max_id: u64,
    pub(super) min_id: u64,
}

impl Default for PackedGraph {
    fn default() -> Self {
        let sequences = Default::default();
        let edges = Default::default();
        let id_graph_map = Default::default();
        let graph_records = PagedIntVec::new(NARROW_PAGE_WIDTH);
        let max_id = 0;
        let min_id = std::u64::MAX;
        PackedGraph {
            sequences,
            edges,
            graph_records,
            id_graph_map,
            max_id,
            min_id,
        }
    }
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct GraphIx(usize);

impl GraphIx {
    pub(super) fn to_id_map_entry(&self) -> u64 {
        let ix = self.0 as u64;
        ix + 1
    }

    pub(super) fn from_id_map_entry(ix: u64) -> Option<Self> {
        if ix == 0 {
            None
        } else {
            Some(GraphIx((ix - 1) as usize))
        }
    }

    pub(super) fn from_graph_records_ix(ix: usize) -> Self {
        GraphIx(ix / GraphRecord::SIZE)
    }

    pub(super) fn to_seq_record_ix(&self) -> usize {
        let ix = self.0;
        ix * Sequences::SIZE
    }

    pub(super) fn left_edges_ix(&self) -> usize {
        let ix = self.0;
        (ix * GraphRecord::SIZE) + GraphRecord::START_OFFSET
    }

    pub(super) fn right_edges_ix(&self) -> usize {
        let ix = self.0;
        (ix * GraphRecord::SIZE) + GraphRecord::END_OFFSET
    }
}

impl PackedGraph {
    pub fn new() -> Self {
        Default::default()
    }

    pub(super) fn new_record_ix(&mut self) -> GraphIx {
        let new_ix = self.graph_records.len();
        self.graph_records.append(0);
        self.graph_records.append(0);
        self.sequences.lengths.append(0);
        self.sequences.indices.append(0);
        GraphIx::from_graph_records_ix(new_ix)
    }

    pub(super) fn get_graph_record(&self, ix: GraphIx) -> GraphRecord {
        let left_edges = self.graph_records.get(ix.left_edges_ix()) as usize;
        let right_edges = self.graph_records.get(ix.right_edges_ix()) as usize;
        GraphRecord {
            left_edges,
            right_edges,
        }
    }

    pub(super) fn get_node_index(&self, id: NodeId) -> Option<GraphIx> {
        let id = u64::from(id);
        if id < self.min_id || id > self.max_id {
            return None;
        }
        let map_ix = id - self.min_id;
        let ix = self.id_graph_map.get(map_ix as usize);
        GraphIx::from_id_map_entry(ix)
    }

    pub(super) fn handle_graph_ix(&self, handle: Handle) -> Option<GraphIx> {
        self.get_node_index(handle.id())
    }

    pub(super) fn push_node_record(&mut self, id: NodeId) -> GraphIx {
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
                let ix = (id - self.min_id) as usize;
                if let Some(to_append) = ix.checked_sub(self.id_graph_map.len())
                {
                    for _ in 0..=to_append {
                        self.id_graph_map.push_back(0);
                    }
                }
            }
        }

        let id = u64::from(id);

        self.min_id = self.min_id.min(id);
        self.max_id = self.max_id.max(id);

        let index = id - self.min_id;
        let value = next_ix.to_id_map_entry();

        self.id_graph_map.set(index as usize, value);

        next_ix
    }

    #[inline]
    pub(super) fn append_graph_record_start_edge(
        &mut self,
        g_ix: GraphIx,
        edge: EdgeIx,
    ) {
        self.graph_records.set(g_ix.left_edges_ix(), edge.0 as u64);
    }

    #[inline]
    pub(super) fn append_graph_record_end_edge(
        &mut self,
        g_ix: GraphIx,
        edge: EdgeIx,
    ) {
        self.graph_records.set(g_ix.right_edges_ix(), edge.0 as u64);
    }

    #[inline]
    pub(super) fn swap_graph_record_elements(&mut self, a: usize, b: usize) {
        let a_val = self.graph_records.get(a);
        let b_val = self.graph_records.get(b);
        self.graph_records.set(a, b_val);
        self.graph_records.set(b, a_val);
    }

    #[inline]
    pub(super) fn handle_edge_record_ix(
        &self,
        handle: Handle,
        dir: Direction,
    ) -> usize {
        let g_ix = self.handle_graph_ix(handle).unwrap();

        use Direction as Dir;
        match (handle.is_reverse(), dir) {
            (false, Dir::Left) => g_ix.left_edges_ix(),
            (false, Dir::Right) => g_ix.right_edges_ix(),
            (true, Dir::Left) => g_ix.right_edges_ix(),
            (true, Dir::Right) => g_ix.left_edges_ix(),
        }
    }

    /// Get the EdgeList record index at the provided graph record
    /// *element* index. I.e. if `ix` is even, the first element of
    /// some record will be returned, if odd, the second element.
    #[inline]
    pub(super) fn get_edge_list_ix(&self, ix: usize) -> EdgeIx {
        let entry = self.graph_records.get(ix);
        EdgeIx(entry as usize)
    }

    pub(super) fn remove_handle_from_edge(
        &mut self,
        handle: Handle,
        dir: Direction,
        to_remove: Handle,
    ) {
        let graph_rec_ix = self.handle_edge_record_ix(handle, dir);
        let edge_ix = self.get_edge_list_ix(graph_rec_ix);

        if let Some(ix) = self.edges.remove_handle_from_list(edge_ix, to_remove)
        {
            if ix != edge_ix {
                self.graph_records.set(graph_rec_ix, ix.0 as u64);
            }
        }
    }

    /*
    pub(super) fn remove_handle(&mut self, handle: Handle) {
        let g_ix = self.handle_graph_ix(handle).unwrap();

        let left_neighbors = self.neighbors(handle, Direction::Left);
        let right_neighbors = self.neighbors(handle, Direction::Left);

        for other in left_neighbors {

        }
    }
    */
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packedgraph_node_record_indices() {
        let mut graph = PackedGraph::new();
        let id_0 = NodeId::from(5);
        let id_1 = NodeId::from(7);
        let id_2 = NodeId::from(3);
        let id_3 = NodeId::from(10);

        let _g_0 = graph.push_node_record(id_0);
        let _g_1 = graph.push_node_record(id_1);
        let _g_2 = graph.push_node_record(id_2);
        let _g_3 = graph.push_node_record(id_3);

        assert_eq!(graph.min_id, 3);
        assert_eq!(graph.max_id, 10);

        assert_eq!(graph.id_graph_map.get(2), 1);
        assert_eq!(graph.id_graph_map.get(4), 2);
        assert_eq!(graph.id_graph_map.get(0), 3);
        assert_eq!(graph.id_graph_map.get(7), 4);
    }

    #[test]
    fn packedgraph_create_handle() {
        let mut graph = PackedGraph::new();
        let n0 = NodeId::from(1);
        let n1 = NodeId::from(2);
        let n2 = NodeId::from(3);
        let h0 = graph.create_handle(b"GTCCA", n0);
        let h1 = graph.create_handle(b"AAA", n1);
        let h2 = graph.create_handle(b"GTGTGT", n2);

        let g0 = graph.handle_graph_ix(h0);
        assert_eq!(g0, Some(GraphIx(0)));

        let g1 = graph.handle_graph_ix(h1);
        assert_eq!(g1, Some(GraphIx(1)));

        let g2 = graph.handle_graph_ix(h2);
        assert_eq!(g2, Some(GraphIx(2)));

        let handles = vec![h0, h1, h2];

        use crate::handlegraph::{AllHandles, HandleGraph, HandleSequences};

        use bstr::{BString, B};

        let sequences = vec![B("GTCCA"), B("AAA"), B("GTGTGT")];

        let s0: BString = graph.sequence(h0).into();
        let s1: BString = graph.sequence(h1).into();
        let s2: BString = graph.sequence(h2).into();

        let s0_b: BString = graph.sequence_iter(h0).collect();
        let s1_b: BString = graph.sequence_iter(h1).collect();
        let s2_b: BString = graph.sequence_iter(h2).collect();

        assert_eq!(
            sequences,
            vec![s0.as_slice(), s1.as_slice(), s2.as_slice()]
        );

        assert_eq!(
            sequences,
            vec![s0_b.as_slice(), s1_b.as_slice(), s2_b.as_slice()]
        );

        let mut handles = graph.all_handles().collect::<Vec<_>>();
        handles.sort();
        let hnd = |x: u64| Handle::pack(x, false);
        assert_eq!(handles, vec![hnd(1), hnd(2), hnd(3)]);
    }

    #[test]
    fn packedgraph_create_edge() {
        use crate::handlegraph::{AllEdges, HandleNeighbors, HandleSequences};
        use bstr::{BString, B};

        let mut graph = PackedGraph::new();

        let seqs =
            vec![B("GTCCA"), B("AAA"), B("GTGTGT"), B("TTCT"), B("AGTAGT")];
        //   node     1          2           3           4          5

        let mut handles: Vec<Handle> = Vec::new();

        for seq in seqs.iter() {
            let handle = graph.append_handle(seq);
            handles.push(handle);
        }

        let hnd = |x: u64| Handle::pack(x, false);

        let edge = |l: u64, r: u64| Edge(hnd(l), hnd(r));

        graph.create_edge(edge(1, 2));
        graph.create_edge(edge(1, 3));

        graph.create_edge(edge(2, 4));

        graph.create_edge(edge(3, 4));
        graph.create_edge(edge(3, 5));

        graph.create_edge(edge(4, 5));

        let adj = |x: u64, left: bool| {
            let dir = if left {
                Direction::Left
            } else {
                Direction::Right
            };
            graph
                .neighbors(hnd(x), dir)
                .map(|h| u64::from(h.id()))
                .collect::<Vec<_>>()
        };

        assert!(adj(1, true).is_empty());
        assert_eq!(vec![3, 2], adj(1, false));

        assert_eq!(vec![1], adj(2, true));
        assert_eq!(vec![4], adj(2, false));

        assert_eq!(vec![1], adj(3, true));
        assert_eq!(vec![5, 4], adj(3, false));

        assert_eq!(vec![3, 2], adj(4, true));
        assert_eq!(vec![5], adj(4, false));

        assert_eq!(vec![4, 3], adj(5, true));
        assert!(adj(5, false).is_empty());

        let edges = graph.all_edges().collect::<Vec<_>>();

        assert_eq!(
            vec![
                edge(1, 3),
                edge(1, 2),
                edge(2, 4),
                edge(3, 5),
                edge(3, 4),
                edge(4, 5)
            ],
            edges
        );
    }

    #[test]
    fn packedgraph_remove_handle_edgelist() {
        use crate::handlegraph::HandleNeighbors;
        use bstr::B;

        let mut graph = PackedGraph::new();

        let seqs =
            vec![B("GTCCA"), B("AAA"), B("GTGTGT"), B("TTCT"), B("AGTAGT")];
        //   node     1          2           3           4          5

        let mut handles: Vec<Handle> = Vec::new();

        for seq in seqs.iter() {
            let handle = graph.append_handle(seq);
            handles.push(handle);
        }

        let hnd = |x: u64| Handle::pack(x, false);

        let edge = |l: u64, r: u64| Edge(hnd(l), hnd(r));

        graph.create_edge(edge(1, 5));
        graph.create_edge(edge(1, 2));
        graph.create_edge(edge(1, 3));
        graph.create_edge(edge(1, 4));

        let adj = |graph: &PackedGraph, x: u64, left: bool| {
            let dir = if left {
                Direction::Left
            } else {
                Direction::Right
            };
            graph
                .neighbors(hnd(x), dir)
                .map(|h| u64::from(h.id()))
                .collect::<Vec<_>>()
        };

        assert_eq!(vec![4, 3, 2, 5], adj(&graph, 1, false));

        graph.remove_handle_from_edge(hnd(1), Direction::Right, hnd(3));
        assert_eq!(vec![4, 2, 5], adj(&graph, 1, false));

        graph.remove_handle_from_edge(hnd(1), Direction::Right, hnd(4));
        assert_eq!(vec![2, 5], adj(&graph, 1, false));

        graph.remove_handle_from_edge(hnd(1), Direction::Right, hnd(5));
        assert_eq!(vec![2], adj(&graph, 1, false));

        graph.remove_handle_from_edge(hnd(1), Direction::Right, hnd(2));
        assert!(adj(&graph, 1, false).is_empty());
    }

    #[test]
    fn packedgraph_split_sequence() {
        let mut graph = PackedGraph::new();
        let n0 = NodeId::from(1);
        let n1 = NodeId::from(2);
        let n2 = NodeId::from(3);
        let h0 = graph.create_handle(b"GTCCA", n0);
        let h1 = graph.create_handle(b"AAA", n1);
        let h2 = graph.create_handle(b"GTGTGT", n2);

        use crate::handlegraph::{AllHandles, HandleGraph, HandleSequences};

        use bstr::{BString, B};

        let hnd = |x: u64| Handle::pack(x, false);

        let seq_bstr = |g: &PackedGraph, h: u64| -> BString {
            g.sequence_iter(hnd(h)).collect()
        };

        assert_eq!(seq_bstr(&graph, 1), B("GTCCA"));
        assert_eq!(seq_bstr(&graph, 2), B("AAA"));
        assert_eq!(seq_bstr(&graph, 3), B("GTGTGT"));

        let s_ix_1 = graph.handle_graph_ix(hnd(1)).unwrap();
        let s_ix_1 = s_ix_1.to_seq_record_ix();

        let splits = graph.sequences.divide_sequence(s_ix_1, vec![2, 3]);

        assert_eq!(seq_bstr(&graph, 1), B("GT"));
        assert_eq!(seq_bstr(&graph, 2), B("AAA"));
        assert_eq!(seq_bstr(&graph, 3), B("GTGTGT"));

        let (ix, _len) = splits[0];
        let new_seq = graph.sequences.iter(ix, false).collect::<BString>();
        assert_eq!(new_seq, B("CCA"));
    }
}
