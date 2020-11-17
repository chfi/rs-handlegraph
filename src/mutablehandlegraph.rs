use crate::handle::{Edge, Handle, NodeId};
use crate::handlegraph::HandleGraph;

pub trait SubtractiveHandleGraph {
    fn remove_handle(&mut self, handle: Handle) -> bool;

    fn remove_edge(&mut self, edge: Edge) -> bool;

    fn clear_graph(&mut self);
}

pub trait AdditiveHandleGraph {
    fn append_handle(&mut self, seq: &[u8]) -> Handle;

    fn create_handle<T: Into<NodeId>>(
        &mut self,
        seq: &[u8],
        node_id: T,
    ) -> Handle;

    fn create_edge(&mut self, edge: Edge);
}

pub trait MutableHandleGraph: HandleGraph {
    fn divide_handle(
        &mut self,
        handle: Handle,
        offsets: Vec<usize>,
    ) -> Vec<Handle>;

    fn split_handle(
        &mut self,
        handle: Handle,
        offset: usize,
    ) -> (Handle, Handle) {
        let handles = self.divide_handle(handle, vec![offset]);
        (handles[0], handles[1])
    }

    fn apply_orientation(&mut self, handle: Handle) -> Handle;
}

pub trait TransformNodeIds {
    fn transform_node_ids<F>(&mut self, transform: F)
    where
        F: Fn(NodeId) -> NodeId + Copy + Send + Sync;

    fn apply_ordering(&mut self, order: &[Handle]);
}

pub trait FullyMutableHandleGraph:
    AdditiveHandleGraph
    + SubtractiveHandleGraph
    + MutableHandleGraph
    + TransformNodeIds
{
}

impl<T> FullyMutableHandleGraph for T where
    T: AdditiveHandleGraph
        + SubtractiveHandleGraph
        + MutableHandleGraph
        + TransformNodeIds
{
}
