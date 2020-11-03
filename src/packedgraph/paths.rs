#![allow(dead_code)]
#![allow(unused_imports)]

use crate::{
    handle::{Direction, Edge, Handle, NodeId},
    handlegraph::*,
};

use fnv::FnvHashMap;

use super::{
    NodeRecordId, OneBasedIndex, PackedDoubleList, PackedList, PackedListIter,
    RecordIndex,
};

use crate::pathhandlegraph::*;

use crate::packed;
use crate::packed::*;

mod occurrences;
mod packedpath;
mod properties;

pub use self::packedpath::*;
pub use self::properties::*;

/// A zero-based index into both the corresponding path in the vector
/// of PackedPaths, as well as all the other property records for the
/// path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PathNameIx(usize);

impl PathNameIx {
    #[inline]
    fn new<I: Into<usize>>(x: I) -> Self {
        Self(x.into())
    }
}

pub struct PathNames {
    // TODO compress the names; don't store entire Vec<u8>s
    name_id_map: FnvHashMap<Vec<u8>, PathNameIx>,
    names: PackedIntVec,
    lengths: PackedIntVec,
    offsets: PagedIntVec,
}

impl Default for PathNames {
    fn default() -> Self {
        PathNames {
            name_id_map: Default::default(),
            names: Default::default(),
            lengths: Default::default(),
            offsets: PagedIntVec::new(super::graph::NARROW_PAGE_WIDTH),
        }
    }
}

impl PathNames {
    pub(super) fn add_name(&mut self, name: &[u8]) -> PathNameIx {
        let name_ix = PathNameIx::new(self.lengths.len());

        self.name_id_map.insert(name.into(), name_ix);

        let name_len = name.len() as u64;
        let name_offset = self.lengths.len() as u64;
        self.lengths.append(name_len);
        self.offsets.append(name_offset);

        name.iter().for_each(|&b| self.names.append(b as u64));

        name_ix
    }

    pub(super) fn name_iter(
        &self,
        ix: PathNameIx,
    ) -> Option<packed::vector::Iter<'_>> {
        let vec_ix = ix.0;
        if vec_ix >= self.lengths.len() {
            return None;
        }

        let offset = self.offsets.get_unpack(vec_ix);
        let len = self.lengths.get_unpack(vec_ix);
        let iter = self.names.iter_slice(offset, len);

        Some(iter)
    }
}

pub struct PackedGraphPaths {
    paths: Vec<PackedPath>,
    path_props: PathProperties,
    path_names: PathNames,
}

impl Default for PackedGraphPaths {
    fn default() -> Self {
        Self {
            paths: Vec::new(),
            path_props: Default::default(),
            path_names: Default::default(),
        }
    }
}

impl PackedGraphPaths {
    pub(super) fn create_path(&mut self, name: &[u8]) -> PathId {
        let path_id = self.paths.len() as u64;
        let packed_path = PackedPath::new();
        self.paths.push(packed_path);

        self.path_props.append_record();
        self.path_names.add_name(name);

        PathId(path_id)
    }

    pub fn len(&self) -> usize {
        self.paths.len()
    }

    /*
    pub(super) fn path_properties(
        &self,
        id: PathId,
    ) -> Option<PathPropertyRecord> {
        self.path_props.get_record(id)
    }
    */

    /*
    pub(super) fn get_path(&self, id: PathId) -> Option<PackedPathRef<'_>> {
        let path = self.paths.get(id.0 as usize)?;
        let properties = self.path_props.get_record(id)?;
        Some(PackedPathRef { path, properties })
    }

    pub(super) fn get_path_mut(
        &mut self,
        id: PathId,
    ) -> Option<PackedPathRefMut<'_>> {
        let path = self.paths.get_mut(id.0 as usize)?;
        let properties = self.path_props.get_record(id)?;
        Some(PackedPathRefMut { path, properties })
    }

    pub(super) fn get_paths_mut<'a, 'b>(
        &'a mut self,
        ids: &'b [PathId],
    ) -> Vec<PackedPathRefMut<'a>> {
        let props = ids
            .iter()
            .copied()
            .filter_map(|i| self.path_props.get_record(i))
            .collect::<Vec<_>>();

        let paths = self
            .paths
            .iter_mut()
            .enumerate()
            .filter_map(|(ix, path)| {
                let ix = PathId(ix as u64);
                if ids.contains(&ix) {
                    Some(path)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        props
            .into_iter()
            .zip(paths.into_iter())
            .map(|(properties, path)| PackedPathRefMut { path, properties })
            .collect()
    }
    */
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packedpath_new_path() {
        let mut p_path = PackedPath::new();

        let hnd = |x: u64| Handle::pack(x, false);

        let s1 = p_path.append_handle(hnd(1));
        let s2 = p_path.insert_after(s1, hnd(4)).unwrap();
        let s3 = p_path.insert_after(s2, hnd(3)).unwrap();
        let s4 = p_path.insert_after(s3, hnd(2)).unwrap();

        let steps_fwd = p_path
            .iter(s1, PathStepIx::null())
            .map(|(ix, step)| {
                (ix.to_vector_value(), u64::from(step.handle.id()))
            })
            .collect::<Vec<_>>();

        assert_eq!(steps_fwd, vec![(1, 1), (2, 4), (3, 3), (4, 2)]);

        let steps_bwd = p_path
            .iter(PathStepIx::null(), s4)
            .rev()
            .map(|(ix, step)| {
                (ix.to_vector_value(), u64::from(step.handle.id()))
            })
            .collect::<Vec<_>>();

        assert_eq!(steps_bwd, vec![(4, 2), (3, 3), (2, 4), (1, 1)]);

        let s5 = p_path.insert_before(s3, hnd(5)).unwrap();
        let s6 = p_path.insert_before(s1, hnd(6)).unwrap();

        let steps_fwd = p_path
            .iter(s6, PathStepIx::null())
            .map(|(ix, step)| {
                (ix.to_vector_value(), u64::from(step.handle.id()))
            })
            .collect::<Vec<_>>();

        assert_eq!(
            steps_fwd,
            vec![(6, 6), (1, 1), (2, 4), (5, 5), (3, 3), (4, 2)]
        );

        let steps_bwd = p_path
            .iter(PathStepIx::null(), s4)
            .rev()
            .map(|(ix, step)| {
                (ix.to_vector_value(), u64::from(step.handle.id()))
            })
            .collect::<Vec<_>>();

        assert_eq!(
            steps_bwd,
            vec![(4, 2), (3, 3), (5, 5), (2, 4), (1, 1), (6, 6)]
        );
    }
}
