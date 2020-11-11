use crate::packedgraph::index::{OneBasedIndex, RecordIndex};

pub(crate) fn build_id_map<I: OneBasedIndex + Ord + std::hash::Hash>(
    removed: &mut [I],
    total_records: usize,
) -> Option<fnv::FnvHashMap<I, I>> {
    removed.sort();
    let first_removed = removed.first().copied()?;

    let max_ix = I::from_zero_based(total_records);

    Some(super::index::removed_id_map_as_u64(&removed, max_ix))
}

/// Trait for (packed) collections that can be defragmented.
pub trait Defragment {
    type Index: OneBasedIndex;

    /// Defragments the collection and returns a map from indices in
    /// the fragmented collection into the kept indices in the
    /// defragmented collection.
    fn defragment(
        &mut self,
    ) -> Option<fnv::FnvHashMap<Self::Index, Self::Index>>;
}

/*
#[macre_export]
macro_rules! impl_defragment {
    ($for:ty, $ix:ty, $removed:ident, [($field0:ident, $vec0:ty) $(, ($fieldn:ident, $vecn:ty))*]) => {
        impl $for {
            pub(crate) fn defragment_(&mut self) -> Option<fnv::FnvHashMap<$ix, $ix>> {
                self.$removed.sort();

                let first_removed = self.$removed.first().copied()?;

                let num_records = self.len();


                let total_records = num_records + self.$removed.len();

                let max_ix = $ix::from_zero_based(total_records);


                let mut id_map =
                    crate::packedgraph::index::removed_id_map_as_u64(&self.$removed, max_ix);

                for ix in 1..(first_removed.pack()) {
                    let x = $ix::unpack(ix);
                    id_map.insert(x, x);
                }

                let mut $field0 = $vec0::new(


            }
        }
    }
}
    */
