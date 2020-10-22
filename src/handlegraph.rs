use crate::handle::{Direction, Edge, Handle, NodeId};

pub mod iter;

pub use self::iter::*;

/// Trait encapsulating the immutable aspects of a handlegraph
pub trait HandleGraph {
    fn has_node(&self, node_id: NodeId) -> bool;

    /// The length of the sequence of a given node
    fn length(&self, handle: Handle) -> usize;

    /// Returns the sequence of a node in the handle's local forward
    /// orientation. Copies the sequence, as the sequence in the graph
    /// may be reversed depending on orientation.
    fn sequence(&self, handle: Handle) -> Vec<u8>;

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

    fn min_node_id(&self) -> NodeId;
    fn max_node_id(&self) -> NodeId;

    /// Return the total number of nodes in the graph
    fn node_count(&self) -> usize;

    /// Return the total number of edges in the graph
    fn edge_count(&self) -> usize;

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

    /// Returns an iterator over the neighbors of a handle in a
    /// given direction
    fn handle_edges_iter<'a>(
        &'a self,
        handle: Handle,
        dir: Direction,
    ) -> Box<dyn Iterator<Item = Handle> + 'a>;

    /// Returns an iterator over all the handles in the graph
    fn handles_iter<'a>(&'a self) -> Box<dyn Iterator<Item = Handle> + 'a>;

    /// Returns an iterator over all the edges in the graph
    fn edges_iter<'a>(&'a self) -> Box<dyn Iterator<Item = Edge> + 'a>;
}
