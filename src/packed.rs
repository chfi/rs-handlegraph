pub mod deque;
pub mod paged;
pub mod robust;
pub mod traits;
pub mod vector;

pub use self::{
    deque::PackedDeque, paged::PagedIntVec, robust::RobustPagedIntVec,
    traits::*, vector::PackedIntVec,
};

#[macro_export]
macro_rules! impl_space_usage {
    ($for:ty, [$first:ident $(, $field:ident)*]) => {
        impl SpaceUsage for $for {
            #[inline]
            fn is_stack_only() -> bool {
                false
            }

            fn heap_bytes(&self) -> usize {
                self.$first.heap_bytes()
                    $(
                        + self.$field.heap_bytes()
                        )*
            }
        }
    };
}
