use crate::handle::{Direction, Edge, Handle, NodeId};

/// Trait encapsulating the immutable aspects of a handlegraph
pub trait HandleGraph {
    fn has_node(&self, node_id: NodeId) -> bool;

    /// The length of the sequence of a given node
    fn length(&self, handle: Handle) -> usize;

    fn sequence(&self, handle: Handle) -> &[u8];

    fn subsequence(&self, handle: Handle, index: usize, size: usize) -> &[u8] {
        &self.sequence(handle)[index..index + size]
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
