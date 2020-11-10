use crate::handle::{Direction, Edge, Handle, NodeId};

use rayon::prelude::*;

pub mod iter;

pub use self::iter::*;

/// Access all the handles in the graph as an iterator, and related
/// methods.
pub trait AllHandles: Sized {
    type Handles: Iterator<Item = Handle>;
    fn all_handles(self) -> Self::Handles;

    #[inline]
    #[no_mangle]
    extern fn node_count(self) -> usize {
        self.all_handles().count()
    }

    #[inline]
    #[no_mangle]
    extern fn has_node<I: Into<NodeId>>(self, n_id: I) -> bool {
        let n_id = n_id.into();
        self.all_handles().any(|h| h.id() == n_id)
    }
}

pub trait AllHandlesPar {
    type HandlesPar: ParallelIterator<Item = Handle>;

    fn all_handles_par(self) -> Self::HandlesPar;
}

/// Access all the edges in the graph as an iterator, and related
/// methods.
pub trait AllEdges: Sized {
    type Edges: Iterator<Item = Edge>;

    extern fn all_edges(self) -> Self::Edges;

    #[inline]
    extern fn edge_count(self) -> usize {
        self.all_edges().count()
    }
}

/// Access to the neighbors of any handle in the given direction, and related methods.
///
/// Implementors should make sure that handles are flipped correctly depending on direction, e.g. using NeighborIter
pub trait HandleNeighbors: Sized {
    type Neighbors: Iterator<Item = Handle>;

    fn neighbors(self, handle: Handle, dir: Direction) -> Self::Neighbors;

    #[inline]
    fn degree(self, handle: Handle, dir: Direction) -> usize {
        self.neighbors(handle, dir).count()
    }

    #[inline]
    fn has_edge(self, left: Handle, right: Handle) -> bool {
        self.neighbors(left, Direction::Right).any(|h| h == right)
    }
}

/// Access to the sequence of any node, and related methods such as retrieving subsequences, individual bases, and node lengths.
pub trait HandleSequences: Sized {
    type Sequence: Iterator<Item = u8>;

    fn sequence_iter(self, handle: Handle) -> Self::Sequence;

    /// Returns the sequence of a node in the handle's local forward
    /// orientation. Copies the sequence, as the sequence in the graph
    /// may be reversed depending on orientation.
    #[inline]
    fn sequence(self, handle: Handle) -> Vec<u8> {
        self.sequence_iter(handle.forward()).collect()
    }

    #[inline]
    fn subsequence(self, handle: Handle, start: usize, len: usize) -> Vec<u8> {
        self.sequence_iter(handle).skip(start).take(len).collect()
    }

    #[inline]
    fn base(self, handle: Handle, index: usize) -> u8 {
        self.sequence_iter(handle).nth(index).unwrap()
    }

    #[inline]
    fn node_len(self, handle: Handle) -> usize {
        self.sequence_iter(handle).count()
    }
}

/// Trait denoting that implementors have access to all the immutable
/// parts of the HandleGraph interface, and that implementors are
/// copyable references (i.e. immutable, shared references).

/// Collects all the HandleGraph iterator traits in a single bound.
/// The `impl` on `&T`, which has the additional bound that `T:
/// HandleGraph`, makes it possible to use this as the only bound in
/// functions that are generic over `HandleGraph` implementations.
pub trait HandleGraphRef:
    AllEdges + AllHandles + HandleNeighbors + HandleSequences + Copy
{
    fn total_length(self) -> usize {
        self.all_handles().map(|h| self.node_len(h)).sum()
    }
}

/// Trait denoting that shared references of an implementor has access
/// to all the HandleGraph methods.
///
/// Also contains some methods that don't fit into any of the other traits.
pub trait HandleGraph {
    extern fn min_node_id(&self) -> NodeId;

    extern fn max_node_id(&self) -> NodeId;
}
