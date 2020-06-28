use crate::handle::{Direction, Edge, Handle, NodeId};

pub trait HandleGraph {
    fn has_node(&self, node_id: NodeId) -> bool;

    fn get_length(&self, handle: Handle) -> usize;

    fn get_sequence(&self, handle: Handle) -> &str;

    fn get_subsequence(
        &self,
        handle: Handle,
        index: usize,
        size: usize,
    ) -> &str {
        &self.get_sequence(handle)[index..index + size]
    }

    fn get_base(&self, handle: Handle, index: usize) -> char {
        char::from(self.get_sequence(handle).as_bytes()[index])
    }

    fn get_node_count(&self) -> usize;
    fn min_node_id(&self) -> NodeId;
    fn max_node_id(&self) -> NodeId;

    fn get_degree(&self, handle: Handle, dir: Direction) -> usize {
        std::iter::from_fn(self.handle_edges_iter_impl(handle, dir))
            .fold(0, |a, _| a + 1)
    }

    fn has_edge(&self, left: Handle, right: Handle) -> bool {
        std::iter::from_fn(self.handle_edges_iter_impl(left, Direction::Right))
            .any(|h| h == right)
    }

    fn get_edge_count(&self) -> usize;

    fn get_total_length(&self) -> usize {
        std::iter::from_fn(self.handle_iter_impl())
            .fold(0, |a, v| a + self.get_sequence(v).len())
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

    fn handle_edges_iter_impl<'a>(
        &'a self,
        handle: Handle,
        dir: Direction,
    ) -> Box<dyn FnMut() -> Option<Handle> + 'a>;

    fn handle_iter_impl<'a>(
        &'a self,
    ) -> Box<dyn FnMut() -> Option<Handle> + 'a>;

    fn edges_iter_impl<'a>(&'a self) -> Box<dyn FnMut() -> Option<Edge> + 'a>;
}

pub fn handle_edges_iter<'a, T: HandleGraph>(
    graph: &'a T,
    handle: Handle,
    dir: Direction,
) -> impl Iterator<Item = Handle> + 'a {
    std::iter::from_fn(graph.handle_edges_iter_impl(handle, dir))
}

pub fn handle_iter<'a, T: HandleGraph>(
    graph: &'a T,
) -> impl Iterator<Item = Handle> + 'a {
    std::iter::from_fn(graph.handle_iter_impl())
}

pub fn edges_iter<'a, T: HandleGraph>(
    graph: &'a T,
) -> impl Iterator<Item = Edge> + 'a {
    std::iter::from_fn(graph.edges_iter_impl())
}
