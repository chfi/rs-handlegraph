//! Traits for immutable access to a HandleGraph, including its
//! nodes/handles, edges, and the sequences of nodes.
//!
//! With the exception of `HandleGraph` and `HandleGraphRef`, each of
//! these traits are centered on providing iterators to one specific
//! part of a graph. For instance, `HandleNeighbors` gives access to
//! the neighbors of a handle, using the associated type `Neighbors:
//! Iterator<Item = Handle>`.
//!
//! Most of the methods that define the immutable part of the
//! handlegraph behavior have default implementations in terms of
//! these iterators, though it may be desirable to provide specific
//! implementations in some cases.
//!
//! The methods on the iterator traits often take `self`, rather than
//! `&self`, because the iterators are expected to borrow (part of)
//! the graph, which means the associated types in the concrete
//! implementations will include lifetimes, and at this time the only
//! way to do that is by using a lifetime that's part of the trait's
//! implementing type.
//!
//! For this reason, you won't actually implement `AllHandles` for
//! `PackedGraph`, but rather for `&'a PackedGraph`.
//!
//! The `HandleGraphRef` trait is provided to make it more convenient
//! to write generic functions using any subset of the handlegraph
//! behaviors.

use crate::handle::{Direction, Edge, Handle, NodeId};

use rayon::prelude::*;

pub mod iter;

pub use self::iter::*;


/// Access all the handles in the graph as an iterator, and querying
/// the graph for number of nodes, and presence of a node by ID.
pub trait AllHandles: Sized {
    type Handles: Iterator<Item = Handle>;

    fn all_handles(self) -> Self::Handles;

    #[inline]
    fn node_count(self) -> usize {
        self.all_handles().count()
    }

    #[inline]
    fn has_node<I: Into<NodeId>>(self, n_id: I) -> bool {
        let n_id = n_id.into();
        self.all_handles().any(|h| h.id() == n_id)
    }
}

/// Parallel access to all the handles in the graph.
pub trait AllHandlesPar {
    type HandlesPar: ParallelIterator<Item = Handle>;

    fn all_handles_par(self) -> Self::HandlesPar;
}

/// Access all the edges in the graph as an iterator, and related and
/// querying the graph for number of edges.
pub trait AllEdges: Sized {
    type Edges: Iterator<Item = Edge>;

    fn all_edges(self) -> Self::Edges;

    #[inline]
    fn edge_count(self) -> usize {
        self.all_edges().count()
    }
}

/// Access to the neighbors of handles in the graph, and querying the
/// graph for a node's degree.
///
/// Implementors should make sure that handles are flipped correctly
/// depending on direction, e.g. using NeighborIter
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
/// Also contains some methods that don't fit into any of the other
/// traits.
pub trait HandleGraph {
    fn min_node_id(&self) -> NodeId;

    fn max_node_id(&self) -> NodeId;
}
