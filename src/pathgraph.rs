use crate::handle::Handle;
use std::ops::Add;

pub trait PathHandleGraph {
    type PathHandle;
    type StepHandle;

    fn get_path_count(&self) -> usize;

    fn has_path(&self, name: &str) -> bool;

    fn get_path_handle(&self, name: &str) -> Option<Self::PathHandle>;

    fn get_path_name(&self, handle: &Self::PathHandle) -> &str;

    fn get_is_circular(&self, handle: &Self::PathHandle) -> bool;

    fn get_step_count(&self, handle: &Self::PathHandle) -> usize;

    fn get_handle_of_step(
        &self,
        step_handle: &Self::StepHandle,
    ) -> Option<Handle>;

    fn get_path_handle_of_step(
        &self,
        step_handle: &Self::StepHandle,
    ) -> Self::PathHandle;

    fn path_begin(&self, path_handle: &Self::PathHandle) -> Self::StepHandle;

    fn path_end(&self, path_handle: &Self::PathHandle) -> Self::StepHandle;

    fn path_back(&self, path_handle: &Self::PathHandle) -> Self::StepHandle;

    fn path_front_end(
        &self,
        path_handle: &Self::PathHandle,
    ) -> Self::StepHandle;

    fn has_next_step(&self, step_handle: &Self::StepHandle) -> bool;

    fn has_previous_step(&self, step_handle: &Self::StepHandle) -> bool;

    fn destroy_path(&mut self, path: &Self::PathHandle);

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
}
