use crate::handle::Handle;

/// Trait encapsulating the immutable path-related aspects of a handlegraph
pub trait PathHandleGraph {
    // These associated types may be removed in the future if it turns
    // out it's better to use a single set of types for all
    // handlegraph implementations
    /// A handle to a path in the graph, can also be viewed as a path identifier
    type PathHandle;
    /// A handle to a specific step on a specific path in the graph
    type StepHandle;

    fn path_count(&self) -> usize;

    fn has_path(&self, name: &[u8]) -> bool;

    /// Paths have string names as well as handles
    fn name_to_path_handle(&self, name: &[u8]) -> Option<Self::PathHandle>;

    fn path_handle_to_name(&self, handle: &Self::PathHandle) -> &[u8];

    fn is_circular(&self, handle: &Self::PathHandle) -> bool;

    fn step_count(&self, handle: &Self::PathHandle) -> usize;

    /// Get the (node) handle that a step handle points to
    fn handle_of_step(&self, step_handle: &Self::StepHandle) -> Option<Handle>;

    fn path_handle_of_step(
        &self,
        step_handle: &Self::StepHandle,
    ) -> Self::PathHandle;

    /// Get the first step of the path
    fn path_begin(&self, path_handle: &Self::PathHandle) -> Self::StepHandle;

    /// Get the last step of the path
    fn path_end(&self, path_handle: &Self::PathHandle) -> Self::StepHandle;

    /// Get a step *beyond* the end of the path
    fn path_back(&self, path_handle: &Self::PathHandle) -> Self::StepHandle;

    /// Get a step *before* the end of the path
    fn path_front_end(
        &self,
        path_handle: &Self::PathHandle,
    ) -> Self::StepHandle;

    fn has_next_step(&self, step_handle: &Self::StepHandle) -> bool;

    fn has_previous_step(&self, step_handle: &Self::StepHandle) -> bool;

    fn path_bases_len(&self, path_handle: &Self::PathHandle) -> Option<usize>;

    fn position_of_step(&self, step_handle: &Self::StepHandle)
        -> Option<usize>;

    fn step_at_position(
        &self,
        path_handle: &Self::PathHandle,
        pos: usize,
    ) -> Option<Self::StepHandle>;

    fn destroy_path(&mut self, path: &Self::PathHandle);

    fn next_step(&self, step_handle: &Self::StepHandle) -> Self::StepHandle;

    fn previous_step(&self, step_handle: &Self::StepHandle)
        -> Self::StepHandle;

    fn create_path_handle(
        &mut self,
        name: &[u8],
        is_circular: bool,
    ) -> Self::PathHandle;

    fn append_step(
        &mut self,
        path: &Self::PathHandle,
        to_append: Handle,
    ) -> Self::StepHandle;

    fn prepend_step(
        &mut self,
        path: &Self::PathHandle,
        to_prepend: Handle,
    ) -> Self::StepHandle;

    fn rewrite_segment(
        &mut self,
        begin: &Self::StepHandle,
        end: &Self::StepHandle,
        new_segment: Vec<Handle>,
    ) -> (Self::StepHandle, Self::StepHandle);

    /// Returns an iterator over all path identifiers in the graph
    fn paths_iter<'a>(
        &'a self,
    ) -> Box<dyn Iterator<Item = &'a Self::PathHandle> + 'a>;

    /// Returns an iterator over all the steps that
    /// cross through the given node handle, across all the paths in
    /// the graph
    fn occurrences_iter<'a>(
        &'a self,
        handle: Handle,
    ) -> Box<dyn Iterator<Item = Self::StepHandle> + 'a>;

    /// Returns an iterator over all the steps in a path
    fn steps_iter<'a>(
        &'a self,
        path: &'a Self::PathHandle,
    ) -> Box<dyn Iterator<Item = Self::StepHandle> + 'a>;
}
