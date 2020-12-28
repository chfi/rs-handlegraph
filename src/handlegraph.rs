//! Traits for immutable access to a HandleGraph, including its
//! nodes/handles, edges, and the sequences of nodes.
//!
//! With the exception of [`HandleGraph`] and [`HandleGraphRef`], each of
//! these traits are centered on providing iterators to one specific
//! part of a graph. For instance, [`IntoNeighbors`] gives access to
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
//! For this reason, you won't actually implement [`IntoHandles`] for
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

/// The base `HandleGraph` trait, with only a tiny subset of the
/// handlegraph behavior.
///
/// If you want to write code that's generic over handlegraphs, use
/// the bound [`HandleGraphRef`] instead.
pub trait HandleGraph {
    /// Return the minimum `NodeId` that exists in the graph.
    fn min_node_id(&self) -> NodeId;

    /// Return the maximum `NodeId` that exists in the graph.
    fn max_node_id(&self) -> NodeId;

    /// Return the number of nodes in the graph.
    fn node_count(&self) -> usize;

    /// Return the number of edges in the graph.
    fn edge_count(&self) -> usize;

    /// Returns the sum of the sequence lengths of all nodes in the
    /// graph.
    fn total_length(&self) -> usize;
}

/// Trait collecting all immutable trait bounds.
///
/// Trait denoting that implementors have access to all the immutable
/// parts of the HandleGraph interface, and that implementors are
/// copyable references (i.e. immutable, shared references).
///
/// Has a blanket implementation for all references that implement the
/// traits in question.
pub trait HandleGraphRef:
    IntoEdges + IntoHandles + IntoNeighbors + IntoSequences + Copy
{
}

impl<'a, T> HandleGraphRef for &'a T where
    &'a T: IntoEdges + IntoHandles + IntoNeighbors + IntoSequences + Copy
{
}

impl<'a, T> HandleGraphRef for &'a mut T where
    &'a mut T: IntoEdges + IntoHandles + IntoNeighbors + IntoSequences + Copy
{
}

/// Access all the handles in the graph as an iterator, and querying
/// the graph for number of nodes, and presence of a node by ID.
pub trait IntoHandles: Sized {
    /// The iterator through all of the graph's handles.
    type Handles: Iterator<Item = Handle>;

    /// Return an iterator on all the handles in the graph.
    fn handles(self) -> Self::Handles;

    /// Returns `true` if the node `node_id` exists in the graph. The
    /// default implementation uses `any` on the iterator from
    /// [`Self::handles`].
    #[inline]
    fn has_node<I: Into<NodeId>>(self, node_id: I) -> bool {
        let node_id = node_id.into();
        self.handles().any(|h| h.id() == node_id)
    }
}

/// Parallel access to all the handles in the graph.
pub trait IntoHandlesPar {
    /// The Rayon `ParallelIterator` through all the handles in the graph.
    type HandlesPar: ParallelIterator<Item = Handle>;

    /// Return a parallel iterator on all the handles in the graph.
    fn handles_par(self) -> Self::HandlesPar;
}

/// Access all the edges in the graph as an iterator, and related and
/// querying the graph for number of edges.
pub trait IntoEdges: Sized {
    /// The iterator through all the edges in the graph.
    type Edges: Iterator<Item = Edge>;

    /// Return an iterator that produces each of the edges in the graph.
    fn edges(self) -> Self::Edges;
}

/// Parallel access to all the edges in the graph.
pub trait IntoEdgesPar {
    /// The Rayon `ParallelIterator` through all the edges in the graph.
    type EdgesPar: ParallelIterator<Item = Edge>;

    /// Return a parallel iterator on all the edges in the graph.
    fn edges_par(self) -> Self::EdgesPar;
}

/// Access to the neighbors of handles in the graph, and querying the
/// graph for a node's degree.
///
/// Implementors should make sure that handles are flipped correctly
/// depending on direction, e.g. using NeighborIter
pub trait IntoNeighbors: Sized {
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
pub trait IntoSequences: Sized {
    type Sequence: Iterator<Item = u8>;

    /// Return an iterator on the bases of the sequence of `handle`.
    /// Implementations should take the orientation of `handle` into
    /// account, and produce the reverse complement of the sequence
    /// when appropriate.
    fn sequence(self, handle: Handle) -> Self::Sequence;

    /// Returns the sequence of the provided `handle` as an owned
    /// `Vec<u8>`. This is a convenience method that calls `collect`
    /// on [`Self::sequence`], so the default implementation should be
    /// fine for all use cases.
    #[inline]
    fn sequence_vec(self, handle: Handle) -> Vec<u8> {
        self.sequence(handle).collect()
    }

    /// Returns the subsequence of the provided `handle`, with the
    /// given offset `start` and length `len`, as an owned `Vec<u8>`.
    /// This is a convenience method that uses [`Self::sequence`],
    /// `skip`, `take`, and `collect`.
    #[inline]
    fn subsequence(self, handle: Handle, start: usize, len: usize) -> Vec<u8> {
        self.sequence(handle).skip(start).take(len).collect()
    }

    /// Returns the base at position `index` of the sequence of the
    /// provided `handle`, if it exists. This is a convenience method
    /// that uses [`Self::sequence`] and `nth`.
    #[inline]
    fn base(self, handle: Handle, index: usize) -> Option<u8> {
        self.sequence(handle).nth(index)
    }

    /// Returns the sequence length of the node at `handle`. The
    /// default implementation uses `count` on [`Self::sequence`],
    /// implementors may wish to change that.
    #[inline]
    fn node_len(self, handle: Handle) -> usize {
        self.sequence(handle).count()
    }
}
