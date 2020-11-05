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

use super::NodeIdIndexMap;

use crate::pathhandlegraph::*;

use crate::packed;
use crate::packed::*;

// mod occurrences;
mod packedpath;
mod properties;

// pub use self::occurrences::*;
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

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
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

pub(super) struct PathMutContext<'a> {
    path_ref_mut: PackedPathRefMut<'a>,
    path_properties: &'a mut PathProperties,
}

impl<'a> Drop for PathMutContext<'a> {
    fn drop(&mut self) {
        let path_id = self.path_ref_mut.path_id;
        let ix = path_id.0 as usize;
        let new_props = &self.path_ref_mut.properties;
        self.path_properties.heads.set_pack(ix, new_props.head);
        self.path_properties.heads.set_pack(ix, new_props.tail);
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

    pub(super) fn path_ref_mut<'a>(
        &'a mut self,
        id: PathId,
    ) -> Option<PackedPathRefMut<'a>> {
        let path_id = id;
        let path = self.paths.get_mut(id.0 as usize)?;
        let properties = self.path_props.get_record(id);
        Some(PackedPathRefMut::new(path_id, path, properties))
    }

    pub(super) fn path_properties_mut<'a>(
        &'a mut self,
        id: PathId,
    ) -> PathPropertyMut<'a> {
        self.path_props.record_mut(id)
    }

    pub(super) fn get_path_mut_ctx<'a>(
        &'a mut self,
        id: PathId,
    ) -> Option<PathMutContext<'a>> {
        let path_id = id;
        let path = self.paths.get_mut(id.0 as usize)?;
        let properties = self.path_props.get_record(id);
        let path_properties = &mut self.path_props;
        let path_ref_mut = PackedPathRefMut::new(path_id, path, properties);
        Some(PathMutContext {
            path_ref_mut,
            path_properties,
        })
    }
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

    #[test]
    fn packedgraph_paths_add() {
        let mut paths = PackedGraphPaths::default();

        let path_id = paths.create_path(b"path1");

        let hnd = |x: u64| Handle::pack(x, false);

        let (s1, s2, s3, s11) = {
            let mut path_mut = paths.path_ref_mut(path_id).unwrap();

            let (_, s1) = path_mut.append_handle(hnd(1));
            let (_, s2) = path_mut.append_handle(hnd(2));
            let (_, s3) = path_mut.append_handle(hnd(3));
            let (_, s11) = path_mut.append_handle(hnd(1));

            let head = path_mut.properties.get_head();
            let tail = path_mut.properties.get_tail();
            {
                let mut iter = path_mut.path.iter(head, PathStepIx::null());

                println!("{:?}", iter.next());
                println!("{:?}", iter.next());
                println!("{:?}", iter.next());
                println!("{:?}", iter.next());
            }
            (s1, s2, s3, s11)
        };

        let occur = &mut paths.occurrences;

        let o1 = occur.append_entry(path_id, s1);
        let o2 = occur.append_entry(path_id, s2);
        let o3 = occur.append_entry(path_id, s3);
        let o11 = occur.append_entry(path_id, s11);

        occur.add_link(o1, o11);

        let mut occur_iter = paths.occurrences.iter(o1);

        println!("{:?}", occur_iter.next());
        println!("{:?}", occur_iter.next());
        println!("{:?}", occur_iter.next());
        println!("{:?}", occur_iter.next());

        /*
        let node_rec = ix_map.get_index(NodeId::from(1)).unwrap();
        let head = path_mut.occurrences.get_node_head(node_rec);
        */
    }
}
