use crate::handle::{Edge, Handle, NodeId};

// TODO implementing paths later
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct PathHandle(u64);

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct StepHandle(u64);

pub trait PathHandleGraph {
    fn get_path_count(&self) -> usize;

    fn has_path(&self, name: &str) -> bool;

    fn get_path_handle(&self, name: &str) -> Option<PathHandle>;

    fn get_path_name(&self, handle: &PathHandle) -> String;

    fn get_is_circular(&self, handle: &PathHandle) -> bool;

    fn get_step_count(&self, step_handle: &StepHandle) -> usize;

    fn get_handle_of_step(&self, step_handle: &StepHandle) -> Handle;

    fn get_path_handle_of_step(&self, step_handle: &StepHandle) -> PathHandle;

    fn path_begin(&self, path_handle: &PathHandle) -> StepHandle;

    fn path_end(&self, path_handle: &PathHandle) -> StepHandle;

    fn path_back(&self, path_handle: &PathHandle) -> StepHandle;

    fn path_front_end(&self, path_handle: &PathHandle) -> StepHandle;

    fn has_next_step(&self, step_handle: &StepHandle) -> bool;

    fn has_previous_step(&self, step_handle: &StepHandle) -> bool;

    fn get_next_step(&self, step_handle: &StepHandle) -> StepHandle;

    fn get_previous_step(&self, step_handle: &StepHandle) -> StepHandle;

    /*
        fn for_each_path_handle(&self, f: F) -> bool
            where F: FnMut(&PathHandle) -> bool;

        fn for_each_step_handle(&self, f: F) -> bool
    */
}
