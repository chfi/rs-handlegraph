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

    fn has_path(&self, name: &str) -> bool;

    /// Paths have string names as well as handles
    fn name_to_path_handle(&self, name: &str) -> Option<Self::PathHandle>;

    fn path_handle_to_name(&self, handle: &Self::PathHandle) -> &str;

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

    fn destroy_path(&mut self, path: &Self::PathHandle);

    fn next_step(&self, step_handle: &Self::StepHandle) -> Self::StepHandle;

    fn previous_step(&self, step_handle: &Self::StepHandle)
        -> Self::StepHandle;

    fn create_path_handle(
        &mut self,
        name: &str,
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

    /// Returns a closure that iterates through all path identifiers in the graph
    fn paths_iter_impl<'a>(
        &'a self,
    ) -> Box<dyn FnMut() -> Option<&'a Self::PathHandle> + 'a>;

    /// Returns a closure that iterates through all the steps that
    /// cross through the given node handle, across all the paths in
    /// the graph
    fn occurrences_iter_impl<'a>(
        &'a self,
        handle: &Handle,
    ) -> Box<dyn FnMut() -> Option<Self::StepHandle> + 'a>;
}

/// Constructs an iterator from paths_iter_impl
pub fn paths_iter<'a, T: PathHandleGraph>(
    graph: &'a T,
) -> impl Iterator<Item = &'a <T as PathHandleGraph>::PathHandle> + 'a {
    std::iter::from_fn(graph.paths_iter_impl())
}

/// Constructs an iterator from paths_iter_impl
pub fn occurrences_iter<'a, T: PathHandleGraph>(
    graph: &'a T,
    handle: &Handle,
) -> impl Iterator<Item = <T as PathHandleGraph>::StepHandle> + 'a {
    std::iter::from_fn(graph.occurrences_iter_impl(handle))
}
