use crate::handle::{Direction, Edge, Handle, NodeId};

// kinda based on libbdsg's hashgraph

// TODO other than NodeId, these shouldn't actually be u64 -- they're going
// to be bit/int vectors

// TODO implementing paths later
// #[derive(Debug, Clone, PartialEq, PartialOrd)]
// pub struct PathHandle(u64);

// #[derive(Debug, Clone, PartialEq, PartialOrd)]
// pub struct StepHandle(u64);

pub trait HandleGraph {
    fn has_node(&self, node_id: NodeId) -> bool;
    fn get_handle(&self, node_id: NodeId, is_reverse: bool) -> Handle;

    // fn get_id(&self, handle: &Handle) -> NodeId;
    // fn get_is_reverse(&self, handle: &Handle) -> bool;

    fn get_length(&self, handle: &Handle) -> usize;
    fn get_sequence(&self, handle: &Handle) -> &str;

    fn get_node_count(&self) -> usize;
    fn min_node_id(&self) -> NodeId;
    fn max_node_id(&self) -> NodeId;

    fn get_degree(&self, handle: &Handle, dir: Direction) -> usize;

    fn has_edge(&self, left: &Handle, right: &Handle) -> bool;

    fn get_edge_count(&self) -> usize;

    fn get_total_length(&self) -> usize;

    fn get_base(&self, handle: &Handle, index: usize) -> char;

    fn get_subsequence(
        &self,
        handle: &Handle,
        index: usize,
        size: usize,
    ) -> &str;

    fn forward(&self, handle: Handle) -> Handle;

    fn edge_handle(&self, left: &Handle, right: &Handle) -> Edge;

    fn traverse_edge_handle(&self, edge: &Edge, left: &Handle) -> Handle;

    fn follow_edges<F>(&self, handle: &Handle, dir: Direction, f: F) -> bool
    where
        F: FnMut(&Handle) -> bool;

    fn for_each_handle<F>(&self, f: F) -> bool
    where
        F: FnMut(&Handle) -> bool;

    /*
    // fn flip(&self, handle: &Handle) -> bool;

    */

    // pub fn iter_edges(&self) ->

    // template<typename Iteratee>
    // bool follow_edges(const handle_t& handle, bool go_left, const Iteratee& iteratee) const;

    // template<typename Iteratee>
    // bool for_each_handle(const Iteratee& iteratee, bool parallel = false) const;
    // template<typename Iteratee>
    // bool for_each_edge(const Iteratee& iteratee, bool parallel = false) const;
}
