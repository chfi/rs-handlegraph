use crate::packedgraph::index::{OneBasedIndex, RecordIndex};

use num_traits::{
    identities,
    identities::{One, Zero},
    Num, NumAssign, PrimInt, ToPrimitive,
};

pub(crate) fn build_id_map_1_based<I: OneBasedIndex + Ord + std::hash::Hash>(
    removed: &mut [I],
    total_records: usize,
) -> Option<fnv::FnvHashMap<I, I>> {
    removed.sort();
    if removed.is_empty() {
        return None;
    }

    let max_ix = I::from_zero_based(total_records);

    Some(super::index::removed_id_map_as_u64(&removed, max_ix))
}

/// Trait for (packed) collections that can be defragmented.
pub trait Defragment {
    type Updates: Sized;

    fn fragmented_len(&self) -> usize;

    fn defragment(&mut self) -> Option<Self::Updates>;
}

#[macro_export]
macro_rules! defragment_loop_impl {
    ($for:ty, [$(($field:ident, $init:expr)),*]) => {
        impl $for {
            fn testing_stuff(&mut self) {
            $(
                let mut $field = $init;

            )*


                $(
                    self.$field = $field;
                )*
            }
        }
    };
}

#[macro_export]
macro_rules! defragment_loop {
    // ($self:ident, $length:ident, $inner:expr, [$(($field:ident, $init:expr)),*]) => {
    ($self:ident, $length:ident, [$(($field:ident, $init:expr)),*], $inner:expr) => {
        $(
            let mut $field = $init;

        )*

            for ix in 0..$length {
                $inner(ix);
            }

        $(
            $self.$field = $field;
        )*
    };
}

#[macro_export]
macro_rules! defragment_block {
    ($self:ident, [$(($field:ident, $init:expr)),*], $inner:block) => {
        $(
            let mut $field = $init;

        )*

            $inner

        $(
            $self.$field = $field;
        )*
    };
}
