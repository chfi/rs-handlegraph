//! Traits for immutable access to a HandleGraph, including its
//! nodes/handles, edges, and the sequences of nodes.
//!
//! With the exception of [`HandleGraph`] and [`HandleGraphRef`], each of
//! these traits are centered on providing iterators to one specific
//! part of a graph. For instance, [`HandleNeighbors`] gives access to
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
//! For this reason, you won't actually implement [`AllHandles`] for
//! [`PackedGraph`](`crate::packedgraph::PackedGraph`), but rather for
//! `&'a PackedGraph`.
//!
//! The [`HandleGraphRef`] trait is provided to make it more convenient
//! to write generic functions using any subset of the handlegraph
//! behaviors.

use crate::handle::{Direction, Edge, Handle, NodeId};

use rayon::prelude::*;

pub mod iter;

pub use self::iter::*;

/// Access all the handles in the graph as an iterator, and querying
/// the graph for number of nodes, and presence of a node by ID.
pub trait AllHandles: Sized {
    /// The iterator through all of the graph's handles.
    type Handles: Iterator<Item = Handle>;

    /// Return an iterator on all the handles in the graph.
    fn all_handles(self) -> Self::Handles;

    /// Return the number of nodes in the graph. The default
    /// implementation calls `count` on the iterator from
    /// [`Self::all_handles`], so implementors may want to change that.
    #[inline]
    fn node_count(self) -> usize {
        self.all_handles().count()
    }

    /// Returns `true` if the node `node_id` exists in the graph. The
    /// default implementation uses `any` on the iterator from
    /// [`Self::all_handles`].
    #[inline]
    fn has_node<I: Into<NodeId>>(self, node_id: I) -> bool {
        let node_id = node_id.into();
        self.all_handles().any(|h| h.id() == node_id)
    }
}

/// Parallel access to all the handles in the graph.
pub trait AllHandlesPar {
    /// The Rayon `ParallelIterator` through all the handles in the graph.
    type HandlesPar: ParallelIterator<Item = Handle>;

    /// Return a parallel iterator on all the handles in the graph.
    fn all_handles_par(self) -> Self::HandlesPar;
}

/// Access all the edges in the graph as an iterator, and related and
/// querying the graph for number of edges.
pub trait AllEdges: Sized {
    /// The iterator through all the edges in the graph.
    type Edges: Iterator<Item = Edge>;

    /// Return an iterator that produces each of the edges in the graph.
    fn all_edges(self) -> Self::Edges;

    /// Return the number of edges in the graph. The default
    /// implementation calls `count` on the iterator from
    /// [`Self::all_edges`], so implementors may want to change that.
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

    /// Return an iterator of the `Handle`s adjacent to the given
    /// `handle`, in the provided `Direction`. Implementations should
    /// take the orientation of `handle` into account.
    fn neighbors(self, handle: Handle, dir: Direction) -> Self::Neighbors;

    /// Return the number of neighbors of `handle` in the given
    /// direction. The default implementation uses `count` on
    /// [`Self::neighbors`].
    #[inline]
    fn degree(self, handle: Handle, dir: Direction) -> usize {
        self.neighbors(handle, dir).count()
    }

    /// Returns `true` if the `left` handle has a connection on its
    /// right-hand side to the `right` handle. The default
    /// implementation uses `any` on [`Self::neighbors`].
    #[inline]
    fn has_edge(self, left: Handle, right: Handle) -> bool {
        self.neighbors(left, Direction::Right).any(|h| h == right)
    }
}

/// Access to the sequence of any node, and related methods such as
/// retrieving subsequences, individual bases, and node lengths.
pub trait HandleSequences: Sized {
    type Sequence: Iterator<Item = u8>;

    /// Return an iterator on the bases of the sequence of `handle`.
    /// Implementations should take the orientation of `handle` into
    /// account, and produce the reverse complement of the sequence
    /// when appropriate.
    fn sequence_iter(self, handle: Handle) -> Self::Sequence;

    /// Returns the sequence of the provided `handle` as an owned
    /// `Vec<u8>`. This is a convenience method that calls `collect`
    /// on [`Self::sequence_iter`], so the default implementation should be
    /// fine for all use cases.
    #[inline]
    fn sequence(self, handle: Handle) -> Vec<u8> {
        self.sequence_iter(handle).collect()
    }

    /// Returns the subsequence of the provided `handle`, with the
    /// given offset `start` and length `len`, as an owned `Vec<u8>`.
    /// This is a convenience method that uses [`Self::sequence_iter`],
    /// `skip`, `take`, and `collect`.
    #[inline]
    fn subsequence(self, handle: Handle, start: usize, len: usize) -> Vec<u8> {
        self.sequence_iter(handle).skip(start).take(len).collect()
    }

    /// Returns the base at position `index` of the sequence of the
    /// provided `handle`, if it exists. This is a convenience method
    /// that uses [`Self::sequence_iter`] and `nth`.
    #[inline]
    fn base(self, handle: Handle, index: usize) -> Option<u8> {
        self.sequence_iter(handle).nth(index)
    }

    /// Returns the sequence length of the node at `handle`. The
    /// default implementation uses `count` on [`Self::sequence_iter`],
    /// implementors may wish to change that.
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
    /// Returns the sum of the sequence lengths of all nodes in the
    /// graph. The default implementation maps
    /// [`HandleSequences::node_len`] over
    /// [`AllHandles::all_handles`].
    fn total_length(self) -> usize {
        self.all_handles().map(|h| self.node_len(h)).sum()
    }
}

/// NB: this trait is going to change, ignore the docs
///
/// Trait denoting that shared references of an implementor has access
/// to all the HandleGraph methods.
///
/// Also contains some methods that don't fit into any of the other
/// traits.
pub trait HandleGraph {
    /// Return the minimum `NodeId` that exists in the graph.
    fn min_node_id(&self) -> NodeId;

    /// Return the maximum `NodeId` that exists in the graph.
    fn max_node_id(&self) -> NodeId;
}
