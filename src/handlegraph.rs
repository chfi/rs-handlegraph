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

    /// Sum up all the sequences in the graph
    fn total_length(&self) -> usize;

    fn degree(&self, handle: Handle, dir: Direction) -> usize;

    fn has_edge(&self, left: Handle, right: Handle) -> bool;
}

/// Convenience trait for collecting all the HandleGraph iterator
/// traits in a single bound. The `impl` on `&T`, which has the
/// additional bound that `T: HandleGraph`, makes it possible to use
/// this as the only bound in functions that are generic over
/// `HandleGraph` implementations.
pub trait HandleGraphRef:
    AllEdges + AllHandles + HandleNeighbors + HandleSequences + Copy
{
}

impl<'a, T> HandleGraphRef for &'a T
where
    T: HandleGraph,
    &'a T: AllEdges + AllHandles + HandleNeighbors + HandleSequences + Copy,
{
}
