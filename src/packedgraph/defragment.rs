use crate::packedgraph::index::OneBasedIndex;

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

    fn defragment(&mut self) -> Option<Self::Updates>;
}

#[macro_export]
macro_rules! assign_fields_local {
    ($self:ident, [$($field:ident),*]) => {
        $(
            $self.$field = $field;
        )*
    }
}

// useful whenever you need to copy only some fields from a struct
#[macro_export]
macro_rules! assign_for_fields {
    ($self:ident, $other:ident, [$($field:ident),*], $closure:expr) => {
        $(
            $self.$field = $closure($other.$field);
        )*
    }
}
