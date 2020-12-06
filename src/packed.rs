pub mod deque;
pub mod paged;
pub mod robust;
pub mod traits;
pub mod vector;

pub use self::{
    deque::PackedDeque,
    paged::{IdentityCodec, PagedCodec, PagedIntVec},
    robust::RobustPagedIntVec,
    traits::*,
    vector::PackedIntVec,
};

#[inline]
pub fn width_for(value: u64) -> usize {
    64 - value.leading_zeros() as usize
}

#[macro_export]
macro_rules! impl_space_usage {
    ($for:ty, [$first:ident $(, $field:ident)*]) => {
        impl succinct::SpaceUsage for $for {
            #[inline]
            fn is_stack_only() -> bool {
                false
            }

            #[inline]
            fn heap_bytes(&self) -> usize {
                self.$first.heap_bytes()
                    $(
                        + self.$field.heap_bytes()
                        )*
            }
        }
    };
}
