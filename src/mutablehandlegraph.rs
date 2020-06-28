use crate::handle::{Direction, Edge, Handle, NodeId};

use crate::handlegraph::HandleGraph;

pub trait MutableHandleGraph: HandleGraph {
    fn append_handle(&mut self, seq: &str) -> Handle;

    fn create_handle(&mut self, seq: &str, node_id: NodeId) -> Handle;

    fn create_edge(&mut self, edge: &Edge);

    fn divide_handle(
        &mut self,
        handle: Handle,
        offsets: Vec<usize>,
    ) -> Vec<Handle>;

    /*

    // this needs some additional functions first, such as reverse complement
    fn apply_orientation(&mut self, handle: &Handle) -> Handle;

    fn split_handle(
        &mut self,
        handle: &Handle,
        offset: usize,
    ) -> (Handle, Handle);

    */

    /*

    /// Alter the node that the given handle corresponds to so the orientation
    /// indicated by the handle becomes the node's local forward orientation.
    /// Rewrites all edges pointing to the node and the node's sequence to
    /// reflect this. Invalidates all handles to the node (including the one
    /// passed). Returns a new, valid handle to the node in its new forward
    /// orientation. Note that it is possible for the node's ID to change.
    /// Does not update any stored paths. May change the ordering of the underlying
    /// graph.
    virtual handle_t apply_orientation(const handle_t& handle) = 0;


    /// Split a handle's underlying node at the given offsets in the handle's
    /// orientation. Returns all of the handles to the parts. Other handles to
    /// the node being split may be invalidated. The split pieces stay in the
    /// same local forward orientation as the original node, but the returned
    /// handles come in the order and orientation appropriate for the handle
    /// passed in.
    /// Updates stored paths.
    virtual std::vector<handle_t> divide_handle(const handle_t& handle, const std::vector<size_t>& offsets) = 0;


    /// Specialization of divide_handle for a single division point
    inline std::pair<handle_t, handle_t> divide_handle(const handle_t& handle, size_t offset) {
        auto parts = divide_handle(handle, std::vector<size_t>{offset});
        return std::make_pair(parts.front(), parts.back());
    }

     */

    // I'll probably not implement the following methods yet

    /*

    /// Adjust the representation of the graph in memory to improve performance.
    /// Optionally, allow the node IDs to be reassigned to further improve
    /// performance.
    /// Note: Ideally, this method is called one time once there is expected to be
    /// few graph modifications in the future.
    virtual void optimize(bool allow_id_reassignment = true) = 0;


    /// Reorder the graph's internal structure to match that given.
    /// This sets the order that is used for iteration in functions like for_each_handle.
    /// Optionally may compact the id space of the graph to match the ordering, from 1->|ordering|.
    /// This may be a no-op in the case of graph implementations that do not have any mechanism to maintain an ordering.
    virtual void apply_ordering(const std::vector<handle_t>& order, bool compact_ids = false) = 0;

    /// Set a minimum id to increment the id space by, used as a hint during construction.
    /// May have no effect on a backing implementation.
    virtual void set_id_increment(const nid_t& min_id) = 0;

    /// Add the given value to all node IDs.
    /// Has a default implementation in terms of reassign_node_ids, but can be
    /// implemented more efficiently in some graphs.
    virtual void increment_node_ids(nid_t increment);


    /// Renumber all node IDs using the given function, which, given an old ID, returns the new ID.
    /// Modifies the graph in place. Invalidates all outstanding handles.
    /// If the graph supports paths, they also must be updated.
    /// The mapping function may return 0. In this case, the input ID will
    /// remain unchanged. The mapping function should not return any ID for
    /// which it would return 0.
    virtual void reassign_node_ids(const std::function<nid_t(const nid_t&)>& get_new_id) = 0;


    */
}
