use crate::handle::{Direction, Edge, Handle, NodeId};

/// Trait encapsulating the immutable aspects of a handlegraph
pub trait HandleGraph {
    fn has_node(&self, node_id: NodeId) -> bool;

    /// The length of the sequence of a given node
    fn length(&self, handle: Handle) -> usize;

    fn sequence(&self, handle: Handle) -> &str;

    fn subsequence(&self, handle: Handle, index: usize, size: usize) -> &str {
        &self.sequence(handle)[index..index + size]
    }

    fn base(&self, handle: Handle, index: usize) -> char {
        char::from(self.sequence(handle).as_bytes()[index])
    }

    fn min_node_id(&self) -> NodeId;
    fn max_node_id(&self) -> NodeId;

    /// Return the total number of nodes in the graph
    fn node_count(&self) -> usize;

    /// Return the total number of edges in the graph
    fn edge_count(&self) -> usize;

    fn degree(&self, handle: Handle, dir: Direction) -> usize {
        std::iter::from_fn(self.handle_edges_iter_impl(handle, dir))
            .fold(0, |a, _| a + 1)
    }

    fn has_edge(&self, left: Handle, right: Handle) -> bool {
        std::iter::from_fn(self.handle_edges_iter_impl(left, Direction::Right))
            .any(|h| h == right)
    }

    /// Sum up all the sequences in the graph
    fn total_length(&self) -> usize {
        std::iter::from_fn(self.handles_iter_impl())
            .fold(0, |a, v| a + self.sequence(v).len())
    }

    fn traverse_edge_handle(&self, edge: &Edge, left: Handle) -> Handle {
        let Edge(el, er) = *edge;

        if left == el {
            er
        } else if left == er.flip() {
            el.flip()
        } else {
            // TODO this should be improved -- this whole function, really
            panic!("traverse_edge_handle called with a handle that the edge didn't connect");
        }
    }

    /// Returns a closure that iterates through the neighbors of a
    /// handle in a given direction
    fn handle_edges_iter_impl<'a>(
        &'a self,
        handle: Handle,
        dir: Direction,
    ) -> Box<dyn FnMut() -> Option<Handle> + 'a>;

    /// Returns a closure that iterates through all the handles in the graph
    fn handles_iter_impl<'a>(
        &'a self,
    ) -> Box<dyn FnMut() -> Option<Handle> + 'a>;

    /// Returns a closure that iterates through all the edges in the graph
    fn edges_iter_impl<'a>(&'a self) -> Box<dyn FnMut() -> Option<Edge> + 'a>;
}

/// Constructs an iterator from handle_edges_iter_impl
pub fn handle_edges_iter<'a, T: HandleGraph>(
    graph: &'a T,
    handle: Handle,
    dir: Direction,
) -> impl Iterator<Item = Handle> + 'a {
    std::iter::from_fn(graph.handle_edges_iter_impl(handle, dir))
}

/// Constructs an iterator from handle_iter_impl
pub fn handles_iter<'a, T: HandleGraph>(
    graph: &'a T,
) -> impl Iterator<Item = Handle> + 'a {
    std::iter::from_fn(graph.handles_iter_impl())
}

/// Constructs an iterator from edges_iter_impl
pub fn edges_iter<'a, T: HandleGraph>(
    graph: &'a T,
) -> impl Iterator<Item = Edge> + 'a {
    std::iter::from_fn(graph.edges_iter_impl())
}
