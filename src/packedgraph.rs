use bio::alphabets::dna;
use bstr::BString;

use crate::{
    handle::{Direction, Edge, Handle, NodeId},
    handlegraph::{
        AllEdges, AllHandles, HandleGraph, HandleNeighbors, HandleSequences,
    },
};

pub mod graph;

use self::graph::{EdgeLists, EdgeRecord, Sequences};
pub use self::graph::{GraphIx, PackedGraph};

impl HandleGraph for PackedGraph {
    fn has_node(&self, id: NodeId) -> bool {
        self.get_node_index(id).is_some()
    }

    /// The length of the sequence of a given node
    fn length(&self, handle: Handle) -> usize {
        let graph_ix = self.handle_graph_ix(handle).unwrap();
        let seq_ix = graph_ix.to_seq_record_ix();
        self.sequences.length(seq_ix)
    }

    /// Returns the sequence of a node in the handle's local forward
    /// orientation. Copies the sequence, as the sequence in the graph
    /// may be reversed depending on orientation.
    fn sequence(&self, handle: Handle) -> Vec<u8> {
        let graph_ix = self.handle_graph_ix(handle).unwrap();
        let seq_ix = graph_ix.to_seq_record_ix();
        let seq = self.sequences.get_sequence(seq_ix);

        if handle.is_reverse() {
            dna::revcomp(seq)
        } else {
            seq
        }
    }

    fn subsequence(
        &self,
        handle: Handle,
        index: usize,
        size: usize,
    ) -> Vec<u8> {
        self.sequence(handle)[index..index + size].into()
    }

    fn base(&self, handle: Handle, index: usize) -> u8 {
        self.sequence(handle)[index]
    }

    fn min_node_id(&self) -> NodeId {
        self.min_id.into()
    }
    fn max_node_id(&self) -> NodeId {
        self.max_id.into()
    }

    /// Return the total number of nodes in the graph
    fn node_count(&self) -> usize {
        // need to make sure this is correct, especially once I add deletion
        self.id_graph_map.len()
    }

    /// Return the total number of edges in the graph
    fn edge_count(&self) -> usize {
        self.edges.edge_lists.len() / EdgeLists::RECORD_SIZE
        // need to make sure this is correct!
    }

    fn degree(&self, handle: Handle, dir: Direction) -> usize {
        self.handle_edges_iter(handle, dir).fold(0, |a, _| a + 1)
    }

    fn has_edge(&self, left: Handle, right: Handle) -> bool {
        self.handle_edges_iter(left, Direction::Right)
            .any(|h| h == right)
    }

    /// Sum up all the sequences in the graph
    fn total_length(&self) -> usize {
        self.handles_iter()
            .fold(0, |a, v| a + self.sequence(v).len())
    }

    fn handle_edges_iter<'a>(
        &'a self,
        handle: Handle,
        dir: Direction,
    ) -> Box<dyn Iterator<Item = Handle> + 'a> {
        unimplemented!();
    }

    fn handles_iter<'a>(&'a self) -> Box<dyn Iterator<Item = Handle> + 'a> {
        unimplemented!();
    }

    fn edges_iter<'a>(&'a self) -> Box<dyn Iterator<Item = Edge> + 'a> {
        unimplemented!();
    }
}
