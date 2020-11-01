use crate::handle::{Edge, Handle, NodeId};
use crate::handlegraph::{HandleGraph, HandleGraphRef};

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

pub trait ModdableHandleGraph {
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

/// Trait encapsulating the mutable aspects of a handlegraph
/// WIP
pub trait MutableHandleGraph: HandleGraph {
    /*
    fn append_handle(&mut self, seq: &[u8]) -> Handle;

    fn create_handle<T: Into<NodeId>>(
        &mut self,
        seq: &[u8],
        node_id: T,
    ) -> Handle;

    fn create_edge(&mut self, edge: Edge);
    */

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

pub trait MutHandleGraphRef: HandleGraphRef {}

impl<'a, T> MutHandleGraphRef for &'a T
where
    T: HandleGraph,
    &'a T: HandleGraphRef,
{
}
