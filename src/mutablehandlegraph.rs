/*!

Traits for manipulating the nodes and edges of a graph.

*/

use crate::handle::{Edge, Handle, NodeId};

/// Methods for adding handles and edges to a graph.
pub trait AdditiveHandleGraph {
    /// Add a node with the provided sequence to the graph, letting
    /// the graph pick the node ID.
    fn append_handle(&mut self, sequence: &[u8]) -> Handle;

    /// Add a node with the provided sequence and ID to the graph.
    fn create_handle<T: Into<NodeId>>(
        &mut self,
        sequence: &[u8],
        node_id: T,
    ) -> Handle;

    /// Insert an edge into the graph. Implementations may panic if
    /// both handles of the edge do not already exist.
    fn create_edge(&mut self, edge: Edge);
}

/// Methods for removing handles and edges from a graph.
pub trait SubtractiveHandleGraph {
    /// Remove a handle from the graph, returning `true` if the handle
    /// existed.
    ///
    /// Implementations may destroy or otherwise modify paths that
    /// cover the handle.
    fn remove_handle(&mut self, handle: Handle) -> bool;

    /// Remove an edge from the graph, returning `true` if the edge
    /// existed.
    fn remove_edge(&mut self, edge: Edge) -> bool;

    fn clear_graph(&mut self);
}

/// Methods for manipulating handles that already exist in a graph.
pub trait MutableHandles: AdditiveHandleGraph {
    /// Divide the given handle at the provided `offsets`, in terms of
    /// the node's sequence. Creates `offsets.len() - 1` new handles,
    /// and updates the edges accordingly.
    ///
    /// Implementations should update paths that include a step on
    /// `handle` by inserting the new handles after that step.
    fn divide_handle(
        &mut self,
        handle: Handle,
        offsets: &[usize],
    ) -> Vec<Handle>;

    /// Divide the given handle at the provided offset, creating one
    /// new handle. Default implementation uses `divide_handle()`, and
    /// there's probably no need to provide another implementation.
    ///
    /// Implementations should update paths that include a step on
    /// `handle` by inserting the new handles after that step.
    fn split_handle(
        &mut self,
        handle: Handle,
        offset: usize,
    ) -> (Handle, Handle) {
        let handles = self.divide_handle(handle, &[offset]);
        (handles[0], handles[1])
    }

    /// Transform the node that `handle` corresponds to so that the
    /// orientation of `handle` becomes the node's forward
    /// orientation. I.e. if `handle` is reverse, the node will be
    /// reversed. Returns the new handle.
    fn apply_orientation(&mut self, handle: Handle) -> Handle;
}

/// Applying transformations to all IDs in a graph, and applying sort orders.
pub trait TransformNodeIds {
    /// Reassign all node IDs in the graph using the provided
    /// `transform` function. `transform` is `Copy + Send + Sync` as
    /// some implementations may perform part of the work in parallel.
    fn transform_node_ids<F>(&mut self, transform: F)
    where
        F: Fn(NodeId) -> NodeId + Copy + Send + Sync;

    /// Reassign all node IDs in the graph using the provided
    /// `transform` closure, which can hold and mutate local state.
    fn transform_node_ids_mut<F>(&mut self, transform: F)
    where
        F: FnMut(NodeId) -> NodeId;

    /// Reassign the node IDs using the provided ordering. `order`
    /// must have one element for each node in the graph, and the node
    /// IDs will be used to index the slice.
    fn apply_ordering(&mut self, order: &[Handle]);
}

/// A graph that supports all forms of handle- and edge-related
/// mutation.
///
/// Has a blanket implementation for all graphs that implement the
/// other traits in this module.
pub trait MutableHandleGraph:
    AdditiveHandleGraph + SubtractiveHandleGraph + MutableHandles + TransformNodeIds
{
}

impl<T> MutableHandleGraph for T where
    T: AdditiveHandleGraph
        + SubtractiveHandleGraph
        + MutableHandles
        + TransformNodeIds
{
}
