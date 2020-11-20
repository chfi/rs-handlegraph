use rayon::prelude::*;

use fnv::FnvHashMap;

use crate::{
    packed::{self, *},
    pathhandlegraph::*,
};

use super::{defragment::Defragment, OneBasedIndex, RecordIndex};

pub(crate) mod packedpath;
pub(crate) mod properties;

pub use self::{
    packedpath::{StepUpdate, *},
    properties::*,
};

#[derive(Debug, Clone)]
pub struct PackedPathNames {
    // TODO compress the names; don't store entire Vec<u8>s
    name_id_map: FnvHashMap<Vec<u8>, PathId>,
    names: PackedIntVec,
    lengths: PackedIntVec,
    offsets: PagedIntVec,
    removed: usize,
}

impl succinct::SpaceUsage for PackedPathNames {
    #[inline]
    fn is_stack_only() -> bool {
        false
    }

    #[inline]
    fn heap_bytes(&self) -> usize {
        // hashmap capacity only provides a lower bound, so this isn't
        // 100% accurate, but it's a small enough part of the entire
        // PackedGraph that it should be fine
        let map_capacity = self.name_id_map.capacity();
        let map_values_size = map_capacity * PathId::stack_bytes();
        let map_keys_size: usize =
            self.name_id_map.keys().map(|k| k.heap_bytes()).sum();

        map_values_size
            + map_keys_size
            + self.names.heap_bytes()
            + self.lengths.heap_bytes()
            + self.offsets.heap_bytes()
    }
}

impl Default for PackedPathNames {
    fn default() -> Self {
        PackedPathNames {
            name_id_map: Default::default(),
            names: Default::default(),
            lengths: Default::default(),
            offsets: PagedIntVec::new(super::graph::NARROW_PAGE_WIDTH),
            removed: 0,
        }
    }
}

impl Defragment for PackedPathNames {
    type Updates = ();

    fn defragment(&mut self) -> Option<()> {
        if self.removed == 0 {
            return None;
        }
        let total_len = self.offsets.len();

        let mut next_offset = 0;
        let mut next_id = 0;

        let new_len = total_len - self.removed;

        let mut new_names = Self::default();

        new_names.lengths.reserve(new_len);
        new_names.offsets.reserve(new_len);
        new_names.name_id_map.reserve(new_len);

        for ix in 0..total_len {
            let length = self.lengths.get(ix);

            if length != 0 {
                let name = self
                    .name_iter(PathId(ix as u64))
                    .unwrap()
                    .collect::<Vec<_>>();

                new_names.lengths.append(length);
                new_names.offsets.append(next_offset);
                name.iter().for_each(|&b| new_names.names.append(b as u64));

                new_names.name_id_map.insert(name, PathId(next_id));

                next_offset += length;
                next_id += 1;
            }
        }

        crate::assign_for_fields!(
            self,
            new_names,
            [name_id_map, lengths, offsets, names],
            |mut x| std::mem::take(&mut x)
        );

        self.removed = 0;

        Some(())
    }
}

impl PackedPathNames {
    pub(crate) fn add_name(&mut self, name: &[u8]) -> PathId {
        let path_id = PathId(self.lengths.len() as u64);

        self.name_id_map.insert(name.into(), path_id);

        let name_len = name.len() as u64;
        let name_offset = self.names.len() as u64;
        self.lengths.append(name_len);
        self.offsets.append(name_offset);

        name.iter().for_each(|&b| self.names.append(b as u64));

        path_id
    }

    pub(super) fn remove_id(&mut self, id: PathId) -> Option<()> {
        let name = self.name_iter(id)?.collect::<Vec<_>>();
        let _id = self.name_id_map.remove(&name);
        let ix = id.0 as usize;

        self.offsets.set(ix, 0);
        self.lengths.set(ix, 0);

        self.removed += 1;

        Some(())
    }

    pub(super) fn name_iter(
        &self,
        id: PathId,
    ) -> Option<packed::vector::IterView<'_, u8>> {
        let vec_ix = id.0 as usize;
        if vec_ix >= self.lengths.len() {
            return None;
        }

        let len = self.lengths.get_unpack(vec_ix);
        if len == 0 {
            return None;
        }

        let offset = self.offsets.get_unpack(vec_ix);
        let iter = self.names.iter_slice(offset, len).view();

        Some(iter)
    }
}

#[derive(Debug, Clone)]
pub struct PackedGraphPaths {
    paths: Vec<PackedPathSteps>,
    pub(crate) path_props: PathProperties,
    pub(crate) path_names: PackedPathNames,
    removed: usize,
}

crate::impl_space_usage!(PackedGraphPaths, [paths, path_props, path_names]);

impl Default for PackedGraphPaths {
    fn default() -> Self {
        Self {
            paths: Vec::new(),
            path_props: Default::default(),
            path_names: Default::default(),
            removed: 0,
        }
    }
}

impl Defragment for PackedGraphPaths {
    /// Defragmenting `PackedGraphPaths` implies also defragmenting
    /// all of the contained `PackedPathSteps`s. Defragmenting a
    /// `PackedPathSteps` can update its step indices, which means the node
    /// occurrences must be updated accordingly.
    type Updates = FnvHashMap<PathId, (PathId, FnvHashMap<StepPtr, StepPtr>)>;

    fn defragment(&mut self) -> Option<Self::Updates> {
        let total_len = self.paths.len();

        let mut new_props = PathProperties::default();
        // TODO the paths could be rewritten in place and the vector then shrunk
        let mut new_paths = Vec::with_capacity(self.path_count());

        let mut updates: Self::Updates = FnvHashMap::default();

        let mut next_id = 0usize;

        for ix in 0..total_len {
            let path_id = PathId(ix as u64);
            let path_deleted = self.paths[ix].path_deleted;

            if !path_deleted {
                let new_id = PathId(next_id as u64);

                let mut path = std::mem::take(self.paths.get_mut(ix).unwrap());
                let path_updates = path.defragment().unwrap_or_default();

                let mut properties = self.path_props.get_record(path_id);
                if let Some(new_head) = path_updates.get(&properties.head) {
                    properties.head = *new_head;
                }
                if let Some(new_tail) = path_updates.get(&properties.tail) {
                    properties.tail = *new_tail;
                }

                new_props.append_record(properties);

                updates.insert(path_id, (new_id, path_updates));

                new_paths.push(path);

                next_id += 1;
            }
        }

        self.paths = new_paths;
        self.path_props = new_props;

        self.path_names.defragment();

        self.removed = 0;

        Some(updates)
    }
}

pub struct PathsMutationCtx<'a> {
    paths: Vec<PackedPath<'a>>,
    properties: &'a mut PathProperties,
}

impl<'a> PathsMutationCtx<'a> {
    pub fn paths_slice(&self) -> &[PackedPath<'a>] {
        self.paths.as_slice()
    }

    pub fn paths_mut_slice(&mut self) -> &mut [PackedPath<'a>] {
        self.paths.as_mut_slice()
    }

    pub(crate) fn iter_mut<'b>(
        &'b mut self,
    ) -> std::slice::IterMut<'b, PackedPath<'a>> {
        self.paths.iter_mut()
    }

    pub(crate) fn par_iter_mut<'b>(
        &'b mut self,
    ) -> rayon::slice::IterMut<'b, PackedPath<'a>> {
        self.paths.par_iter_mut()
    }
}

impl<'a> Drop for PathsMutationCtx<'a> {
    fn drop(&mut self) {
        for path in self.paths.iter() {
            let path_id = path.path_id;
            let ix = path_id.0 as usize;

            self.properties.heads.set_pack(ix, path.head);
            self.properties.tails.set_pack(ix, path.tail);
            self.properties.circular.set_pack(ix, path.circular);
            self.properties
                .deleted_steps
                .set_pack(ix, path.deleted_steps);
        }
    }
}

impl<'a> std::ops::Index<PathId> for PathsMutationCtx<'a> {
    type Output = PackedPath<'a>;

    fn index(&self, id: PathId) -> &PackedPath<'a> {
        &self.paths[id.0 as usize]
    }
}

impl<'a> std::ops::Index<std::ops::Range<PathId>> for PathsMutationCtx<'a> {
    type Output = [PackedPath<'a>];

    fn index(&self, ids: std::ops::Range<PathId>) -> &[PackedPath<'a>] {
        let start = ids.start.0 as usize;
        let end = ids.end.0 as usize;
        &self.paths[start..end]
    }
}

impl<'a> std::ops::IndexMut<PathId> for PathsMutationCtx<'a> {
    fn index_mut(&mut self, id: PathId) -> &mut PackedPath<'a> {
        &mut self.paths[id.0 as usize]
    }
}

impl<'a> std::ops::IndexMut<std::ops::Range<PathId>> for PathsMutationCtx<'a> {
    fn index_mut(
        &mut self,
        ids: std::ops::Range<PathId>,
    ) -> &mut [PackedPath<'a>] {
        let start = ids.start.0 as usize;
        let end = ids.end.0 as usize;
        &mut self.paths[start..end]
    }
}

impl PackedGraphPaths {
    pub(super) fn create_path(&mut self, name: &[u8]) -> PathId {
        let path_id = self.paths.len() as u64;
        let packed_path = PackedPathSteps::default();
        self.paths.push(packed_path);

        self.path_props.append_empty();
        self.path_names.add_name(name);

        PathId(path_id)
    }

    pub(super) fn remove_path(
        &mut self,
        id: PathId,
    ) -> Option<Vec<StepUpdate>> {
        unimplemented!();
        /*
        let ix = id.0;

        let steps = {
            let path = self.path_ref(id)?;

            path.steps()
                .map(|(step_ix, _step)| step_ix)
                .collect::<Vec<_>>()
                unimplemented!();
        };

        let step_updates = self.with_path_mut_ctx(id, move |path_ref| {
            steps
                .into_iter()
                .filter_map(|step| path_ref.remove_step(step))
                .collect()
        })?;

        self.paths[ix as usize].path_deleted = true;

        self.path_names.remove_id(id)?;

        self.path_props.clear_record(id);

        self.removed += 1;

        Some(step_updates)
        */
    }

    pub fn path_count(&self) -> usize {
        self.paths.len() - self.removed
    }

    pub(super) fn path_ref(&self, id: PathId) -> Option<PackedPath<'_>> {
        unimplemented!();
        // let path_id = id;
        // let path = self.paths.get(id.0 as usize)?;
        // let properties = self.path_props.get_record(id);
        // Some(PackedPath::new(path_id, path, &properties))
    }

    pub(super) fn get_path_mut_ctx(
        &mut self,
        id: PathId,
    ) -> Option<PathsMutationCtx<'_>> {
        let path = self.paths.get_mut(id.0 as usize)?;
        let props = self.path_props.get_record(id);
        let properties = &mut self.path_props;

        let packed_path = PackedPath::new(id, path, &props);
        Some(PathsMutationCtx {
            paths: vec![packed_path],
            properties,
        })
    }

    pub(super) fn get_all_paths_mut_ctx(&mut self) -> PathsMutationCtx<'_> {
        let properties = &mut self.path_props;

        let paths = self
            .paths
            .iter_mut()
            .enumerate()
            .map(|(id, path)| {
                let path_id = PathId(id as u64);
                let props = properties.get_record(path_id);
                PackedPath::new(path_id, path, &props)
            })
            .collect::<Vec<_>>();

        PathsMutationCtx { paths, properties }
    }

    pub(super) fn with_path_mut_ctx<'a, F>(
        &'a mut self,
        id: PathId,
        f: F,
    ) -> Option<Vec<StepUpdate>>
    where
        F: FnOnce(&mut PackedPath<'a>) -> Vec<StepUpdate>,
    {
        let mut mut_ctx = self.get_path_mut_ctx(id)?;
        let ref_mut = mut_ctx.paths.first_mut()?;

        Some(f(ref_mut))
    }

    pub(super) fn with_all_paths_mut_ctx_par<'a, F>(
        &'a mut self,
        f: F,
    ) -> Vec<(PathId, Vec<StepUpdate>)>
    where
        F: Fn(PathId, &mut PackedPath<'a>) -> Vec<StepUpdate> + Sync,
    {
        let mut mut_ctx = self.get_all_paths_mut_ctx();
        let refs_mut = mut_ctx.par_iter_mut();

        refs_mut
            .map(|path| {
                let path_id = path.path_id;
                let steps = f(path_id, path);
                (path_id, steps)
            })
            .collect::<Vec<_>>()
    }

    // fn path_ref_<'a>(&'a self, id: PathId) -> Option<PackedPath_<PackedStepsShared<'a>>> {
    fn path_ref_<'a>(&'a self, id: PathId) -> Option<PackedRef<'a>> {
        let steps = self.paths.get(id.0 as usize)?;

        let properties = &self.path_props;
        let props = properties.get_record(id);

        Some(PackedPath_::new_ref(id, steps, &props))
    }

    pub(super) fn zip_with_paths_mut_ctx<'a, T, I, F>(
        &'a mut self,
        iter: I,
        mut f: F,
    ) -> Vec<(PathId, Vec<StepUpdate>)>
    where
        T: Send + Sync,
        I: IndexedParallelIterator<Item = T>,
        F: FnMut(T, PathId, &mut PackedPath<'a>) -> Vec<StepUpdate>
            + Send
            + Sync,
    {
        unimplemented!();
        // let mut mut_ctx = self.get_all_paths_mut_ctx();
        // let refs_mut = mut_ctx.par_iter_mut();

        // refs_mut
        //     .zip(iter)
        //     .map(|(path, val)| {
        //         let path_id = path.path_id;
        //         let steps = f(val, path_id, path);
        //         (path_id, steps)
        //     })
        //     .collect::<Vec<_>>()
    }

    pub(super) fn with_multipath_mut_ctx<'a, F>(
        &'a mut self,
        f: F,
    ) -> Vec<(PathId, Vec<StepUpdate>)>
    where
        F: Fn(PathId, &mut PackedPath<'a>) -> Vec<StepUpdate>,
    {
        let mut mut_ctx = self.get_all_paths_mut_ctx();
        let refs_mut = mut_ctx.iter_mut();

        refs_mut
            .map(|path| {
                let path_id = path.path_id;
                let steps = f(path_id, path);
                (path_id, steps)
            })
            .collect::<Vec<_>>()
    }
}
impl<'a> AllPathIds for &'a PackedPathNames {
    type PathIds = std::iter::Copied<
        std::collections::hash_map::Values<'a, Vec<u8>, PathId>,
    >;

    fn all_path_ids(self) -> Self::PathIds {
        self.name_id_map.values().copied()
    }
}

impl<'a> PathNames for &'a PackedPathNames {
    type PathName = packed::vector::IterView<'a, u8>;

    fn get_path_name(self, id: PathId) -> Option<Self::PathName> {
        self.name_iter(id)
    }

    fn get_path_id(self, name: &[u8]) -> Option<PathId> {
        self.name_id_map.get(name).copied()
    }
}

impl<'a> PathNamesMut for &'a mut PackedPathNames {
    fn insert_name(self, name: &[u8]) -> Option<PathId> {
        if self.get_path_id(name).is_some() {
            None
        } else {
            Some(self.add_name(name))
        }
    }
}

// impl<'a> PathRefs for &'a PackedGraphPaths {
//     type Path = PackedPathRef<'a>;

//     fn path_ref(self, id: PathId) -> Option<PackedPathRef<'a>> {
//         self.path_ref(id)
//     }
// }

// impl<'a> AllPathRefs for &'a PackedGraphPaths {
//     type PathIds = &'a PackedPathNames;

//     fn all_paths_ref(self) -> Vec<Self::Path> {
//         self.path_names
//             .all_path_ids()
//             .filter_map(|p_id| self.path_ref(p_id))
//             .collect()
//     }
// }

/*
impl<'a> PathRefsMut for &'a mut PackedGraphPaths {
    type PathMut = PathMutContext<'a>;

    fn path_mut(self, id: PathId) -> Option<PathMutContext<'a>> {
        self.get_path_mut_ctx(id)
    }
}
*/

/*
impl<'a, 'b> AllPathRefsMut for &'a mut MultiPathMutContext<'b> {
    type MutCtx = &'a mut PackedPathRefMut<'b>;
    type PathRefsMut = std::slice::IterMut<'a, PackedPathRefMut<'b>>;

    fn all_paths_mut(self) -> Self::PathRefsMut {
        self.get_ref_muts()
    }
}

impl<'a> WithPathRefsMut for &'a mut PackedGraphPaths {
    type MutCtx = PackedPathRefMut<'a>;

    fn with_path_mut<F>(self, id: PathId, f: F) -> Option<Vec<StepUpdate>>
    where
        for<'b> F: Fn(&mut Self::MutCtx) -> Vec<StepUpdate>,
    {
        self.with_path_mut_ctx(id, f)
    }

    fn with_paths_mut<F>(self, f: F) -> Vec<(PathId, Vec<StepUpdate>)>
    where
        for<'b> F: Fn(PathId, &mut Self::MutCtx) -> Vec<StepUpdate>,
    {
        self.with_multipath_mut_ctx(f)
    }
}
*/

#[cfg(test)]
pub(crate) mod tests {
    use crate::handle::Handle;

    use super::*;

    /// A little DSL for generating paths because why not~
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
    pub(crate) enum StepOp {
        Append(usize),
        Prepend(usize),
        InsertMiddle(usize),
        RemoveFromStart(usize),
        RemoveFromEnd(usize),
    }

    impl std::fmt::Display for StepOp {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                StepOp::Append(x) => write!(f, "A  {:>2}", x),
                StepOp::Prepend(x) => write!(f, "P  {:>2}", x),
                StepOp::InsertMiddle(x) => write!(f, "M  {:>2}", x),
                StepOp::RemoveFromStart(x) => write!(f, "RS {:>2}", x),
                StepOp::RemoveFromEnd(x) => write!(f, "RE {:>2}", x),
            }
        }
    }

    #[macro_export]
    macro_rules! step_op {
        () => {};
        (A $count:literal) => {
            StepOp::Append($count)
        };
        (P $count:literal) => {
            StepOp::Prepend($count)
        };
        (M $count:literal) => {
            StepOp::InsertMiddle($count)
        };
        (RS $count:literal) => {
            StepOp::RemoveFromStart($count)
        };
        (RE $count:literal) => {
            StepOp::RemoveFromEnd($count)
        };
    }

    #[macro_export]
    macro_rules! step_ops {
        () => {};
        ($($op:tt $count:literal),*) => {
            vec![$(crate::step_op!($op $count),)*]
        };
    }

    pub(crate) fn apply_step_ops(
        path: &mut PackedPathRefMut<'_>,
        ops: &[StepOp],
    ) -> Vec<StepUpdate> {
        let mut updates = Vec::new();

        let mut max_id = 0usize;

        for &op in ops.iter() {
            match op {
                StepOp::Append(x) => {
                    updates.extend(path.add_some_steps(&mut max_id, x, false));
                }
                StepOp::Prepend(x) => {
                    updates.extend(path.add_some_steps(&mut max_id, x, true));
                }
                StepOp::InsertMiddle(x) => {
                    updates
                        .extend(path.insert_many_into_middle(&mut max_id, x));
                }
                StepOp::RemoveFromStart(x) => {
                    updates.extend(path.remove_some_steps(x, true));
                }
                StepOp::RemoveFromEnd(x) => {
                    updates.extend(path.remove_some_steps(x, false));
                }
            }
        }

        updates
    }

    #[test]
    fn packedpathpaths_new_path() {
        let mut p_path = PackedPathSteps::default();

        let hnd = |x: u64| Handle::pack(x, false);

        let s1 = p_path.append_handle_record(hnd(1));
        let s2 = p_path.insert_after(s1, hnd(4)).unwrap();
        let s3 = p_path.insert_after(s2, hnd(3)).unwrap();
        let s4 = p_path.insert_after(s3, hnd(2)).unwrap();

        let steps_fwd = p_path
            .iter(s1, StepPtr::null())
            .map(|(ix, step)| {
                (ix.to_vector_value(), u64::from(step.handle.id()))
            })
            .collect::<Vec<_>>();

        assert_eq!(steps_fwd, vec![(1, 1), (2, 4), (3, 3), (4, 2)]);

        let steps_bwd = p_path
            .iter(StepPtr::null(), s4)
            .rev()
            .map(|(ix, step)| {
                (ix.to_vector_value(), u64::from(step.handle.id()))
            })
            .collect::<Vec<_>>();

        assert_eq!(steps_bwd, vec![(4, 2), (3, 3), (2, 4), (1, 1)]);

        let _s5 = p_path.insert_before(s3, hnd(5)).unwrap();
        let s6 = p_path.insert_before(s1, hnd(6)).unwrap();

        let steps_fwd = p_path
            .iter(s6, StepPtr::null())
            .map(|(ix, step)| {
                (ix.to_vector_value(), u64::from(step.handle.id()))
            })
            .collect::<Vec<_>>();

        assert_eq!(
            steps_fwd,
            vec![(6, 6), (1, 1), (2, 4), (5, 5), (3, 3), (4, 2)]
        );

        let steps_bwd = p_path
            .iter(StepPtr::null(), s4)
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
    fn packedgraphpaths_paths_add() {
        let hnd = |x: u64| Handle::pack(x, false);

        let mut paths = PackedGraphPaths::default();

        let path_1 = paths.create_path(b"path1");

        let pre_record = paths.path_props.get_record(path_1);

        assert!(pre_record.head.is_null());
        assert!(pre_record.tail.is_null());

        let _step_updates = {
            let mut path_mut = paths.get_path_mut_ctx(path_1).unwrap();
            let ref_mut = path_mut.paths.first_mut().unwrap();

            let steps = vec![1, 2, 3, 4, 3, 5]
                .into_iter()
                .map(|n| {
                    let h = hnd(n);
                    let s = ref_mut.append_handle(h);
                    s
                })
                .collect::<Vec<_>>();

            steps
        };

        let post_record = paths.path_props.get_record(path_1);

        // Heads & tails are path step indices, not handles
        assert_eq!(post_record.head.to_vector_value(), 1);
        assert_eq!(post_record.tail.to_vector_value(), 6);

        let path_ref = paths.path_ref(path_1).unwrap();

        // PackedPathRef implements the PathRef trait,
        // so we can step through the path
        let steps = path_ref
            .steps()
            .map(|(_ix, step)| step.handle)
            .collect::<Vec<_>>();

        let mut expected_steps =
            vec![hnd(1), hnd(2), hnd(3), hnd(4), hnd(3), hnd(5)];
        assert_eq!(steps, expected_steps);

        // The step iterator is double-ended, so we can reverse it
        let steps_rev = path_ref
            .steps()
            .rev()
            .map(|(_ix, step)| step.handle)
            .collect::<Vec<_>>();

        expected_steps.reverse();
        assert_eq!(steps_rev, expected_steps);
    }

    #[test]
    fn packedgraphpaths_path_with_mut_ctx() {
        let hnd = |x: u64| Handle::pack(x, false);

        let mut paths = PackedGraphPaths::default();

        let path_1 = paths.create_path(b"path1");

        let _steps = paths.with_path_mut_ctx(path_1, |ref_mut| {
            vec![1, 2, 3, 4, 3, 5]
                .into_iter()
                .map(|n| ref_mut.append_handle(hnd(n)))
                .collect::<Vec<_>>()
        });

        let post_record = paths.path_props.get_record(path_1);
        assert_eq!(post_record.head.to_vector_value(), 1);
        assert_eq!(post_record.tail.to_vector_value(), 6);

        let path_ref = paths.path_ref(path_1).unwrap();

        let steps = path_ref
            .steps()
            .map(|(_ix, step)| step.handle)
            .collect::<Vec<_>>();

        let expected_steps =
            vec![hnd(1), hnd(2), hnd(3), hnd(4), hnd(3), hnd(5)];
        assert_eq!(steps, expected_steps);
    }

    #[test]
    fn removing_steps() {
        let hnd = |x: u64| Handle::pack(x, false);
        let vec_hnd = |v: Vec<u64>| v.into_iter().map(hnd).collect::<Vec<_>>();

        let mut paths = PackedGraphPaths::default();

        let path_1 = paths.create_path(b"path1");
        let path_2 = paths.create_path(b"path2");
        let path_3 = paths.create_path(b"path3");

        let nodes_1 = vec_hnd(vec![1, 2, 3, 4, 5]);
        let nodes_2 = vec_hnd(vec![6, 2, 3, 7, 5]);
        let nodes_3 = vec_hnd(vec![1, 6, 2, 3, 4]);

        let inserts = vec![nodes_1, nodes_2, nodes_3];

        let print_step_updates = |step_updates: &[Vec<StepUpdate>]| {
            for (i, steps) in step_updates.iter().enumerate() {
                print!("{}", i);
                for su in steps.iter() {
                    let (u, handle, step) = match su {
                        StepUpdate::Insert { handle, step } => {
                            ("I", handle, step)
                        }
                        StepUpdate::Remove { handle, step } => {
                            ("R", handle, step)
                        }
                    };
                    let h = u64::from(handle.id());
                    let s = step.pack();
                    print!("\t({}, {:2}, {:2})", u, h, s);
                }
                println!();
            }
        };

        let print_path = |paths: &PackedGraphPaths, id: PathId| {
            let path_ref = paths.path_ref(id).unwrap();
            let head = path_ref.properties.head.pack();
            let tail = path_ref.properties.tail.pack();
            println!("path {:2} head {:4} tail {:4}", id.0, head, tail);
            println!(
                "    {:4}  {:4}  {:4}  {:4}",
                "step", "node", "prev", "next"
            );
            for (step_ix, step) in path_ref.steps() {
                let s_ix = step_ix.pack();
                let h = u64::from(step.handle.id());
                let p = step.prev.pack();
                let n = step.next.pack();
                println!("    {:4}  {:4}  {:4}  {:4}", s_ix, h, p, n);
            }
            println!();
        };

        let step_updates = {
            let mut mut_ctx = paths.get_multipath_mut_ctx();
            let paths_mut = mut_ctx.get_ref_muts();

            paths_mut
                .zip(inserts.iter())
                .map(|(path, steps)| {
                    steps
                        .iter()
                        .map(|h| path.append_handle(*h))
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>()
        };

        print_step_updates(&step_updates);

        print_path(&paths, path_1);
        print_path(&paths, path_2);
        print_path(&paths, path_3);

        let rem_1 = vec![1, 5];
        let rem_2 = vec![2, 3, 4];
        let rem_3 = vec![1, 3, 5];

        let remove: Vec<Vec<usize>> = vec![rem_1, rem_2, rem_3];

        let rem_step_updates = {
            let mut mut_ctx = paths.get_multipath_mut_ctx();
            let paths_mut = mut_ctx.get_ref_muts();

            paths_mut
                .zip(remove.iter())
                .map(|(path, steps)| {
                    steps
                        .iter()
                        .filter_map(|&step_ix| {
                            let ix = StepPtr::from_one_based(step_ix);
                            path.remove_step(ix)
                        })
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>()
        };

        print_path(&paths, path_1);
        print_path(&paths, path_2);
        print_path(&paths, path_3);

        print_step_updates(&rem_step_updates);
    }

    #[test]
    fn packedgraphpaths_multipaths() {
        let hnd = |x: u64| Handle::pack(x, false);

        let mut paths = PackedGraphPaths::default();

        let path_1 = paths.create_path(b"path1");
        let path_2 = paths.create_path(b"path2");
        let path_3 = paths.create_path(b"path3");

        let vec_hnd = |v: Vec<u64>| v.into_iter().map(hnd).collect::<Vec<_>>();

        // Path 1: 1 2 3 4 5
        // Path 2: 6 2 3 7 5
        // Path 3: 1 6 2 3 4

        let pre_1 = paths.path_props.get_record(path_1);
        let pre_2 = paths.path_props.get_record(path_2);
        let pre_3 = paths.path_props.get_record(path_3);

        assert!(pre_1.head.is_null() && pre_1.tail.is_null());
        assert!(pre_2.head.is_null() && pre_2.tail.is_null());
        assert!(pre_3.head.is_null() && pre_3.tail.is_null());

        let to_insert_1 = vec_hnd(vec![1, 2, 3, 4, 5]);
        let to_insert_2 = vec_hnd(vec![6, 2, 3, 7, 5]);
        let to_insert_3 = vec_hnd(vec![1, 6, 2, 3, 4]);

        let to_insert = vec![
            to_insert_1.clone(),
            to_insert_2.clone(),
            to_insert_3.clone(),
        ];

        let _step_updates = {
            let mut mut_ctx = paths.get_multipath_mut_ctx();
            let paths_mut = mut_ctx.get_ref_muts();

            paths_mut
                .zip(to_insert)
                .map(|(path, steps)| {
                    steps
                        .into_iter()
                        .map(|h| path.append_handle(h))
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>()
        };

        let post_1 = paths.path_props.get_record(path_1);
        let post_2 = paths.path_props.get_record(path_2);
        let post_3 = paths.path_props.get_record(path_3);

        assert_eq!(post_1.head.to_vector_value(), 1);
        assert_eq!(post_1.tail.to_vector_value(), 5);

        assert_eq!(post_2.head.to_vector_value(), 1);
        assert_eq!(post_2.tail.to_vector_value(), 5);

        assert_eq!(post_3.head.to_vector_value(), 1);
        assert_eq!(post_3.tail.to_vector_value(), 5);

        let ref_1 = paths.path_ref(path_1).unwrap();
        let ref_2 = paths.path_ref(path_2).unwrap();
        let ref_3 = paths.path_ref(path_3).unwrap();

        let get_steps = |path_ref: PackedPathRef<'_>| {
            path_ref
                .steps()
                .map(|(_ix, step)| step.handle)
                .collect::<Vec<_>>()
        };

        assert_eq!(get_steps(ref_1), to_insert_1);
        assert_eq!(get_steps(ref_2), to_insert_2);
        assert_eq!(get_steps(ref_3), to_insert_3);
    }

    #[test]
    fn defrag_path_names() {
        use bstr::B;

        let mut packed_names = PackedPathNames::default();

        let names = vec![
            B("path1"),
            B("another_path"),
            B("third-path"),
            B("path123"),
            B("path9812983"),
            B("AAAAAAAAAAA"),
            B("BBBBBBBBBBB"),
            B("CCCCCC"),
            B("DDDDDDDDDDDDDDDDD"),
            B("EEEE"),
            B("FFFFFFFFFFFFFFFFFFFFF"),
            B("GGGGGGGGGGGGG"),
        ];

        let _ids = names
            .iter()
            .map(|n| packed_names.add_name(n))
            .collect::<Vec<_>>();

        packed_names.remove_id(PathId(3)).unwrap();
        packed_names.remove_id(PathId(5)).unwrap();
        packed_names.remove_id(PathId(6)).unwrap();
        packed_names.remove_id(PathId(7)).unwrap();
        packed_names.remove_id(PathId(8)).unwrap();

        packed_names.defragment();

        let new_path_ids =
            (0..=6u64).into_iter().map(PathId).collect::<Vec<_>>();
        let kept_names = new_path_ids
            .iter()
            .filter_map(|&i| {
                let iter = packed_names.name_iter(i)?;
                let bytes = iter.collect::<Vec<_>>();
                let string = std::str::from_utf8(&bytes).unwrap();
                Some(String::from(string))
            })
            .collect::<Vec<String>>();

        let expected_names = vec![
            String::from("path1"),
            String::from("another_path"),
            String::from("third-path"),
            String::from("path9812983"),
            String::from("EEEE"),
            String::from("FFFFFFFFFFFFFFFFFFFFF"),
            String::from("GGGGGGGGGGGGG"),
        ];

        assert_eq!(kept_names, expected_names);
    }

    #[test]
    fn defrag_graph_paths() {
        use bstr::B;

        use crate::packedgraph::defragment::Defragment;

        let mut paths = PackedGraphPaths::default();

        let names = [
            B("path1"),
            B("pathAAAAAAAAA"),
            B("p3"),
            B("paaaaath8"),
            B("11233455"),
        ];

        let _path_ids = names
            .iter()
            .map(|n| paths.create_path(n))
            .collect::<Vec<_>>();

        /*
          Path 0 -  1  2  3  4  5  6
          Path 1 -  7  8  2  3  4  5  6
          Path 2 -  1  2  3  4  5  6  9 10
          Path 3 -  1  2 11 12  3  5  6
          Path 4 - 13 14  3  4 15 16
        */

        let path_ops = vec![
            step_ops![A 6],
            step_ops![A 6, RS 1, P 2],
            step_ops![A 6, A 2, RE 2, A 2],
            step_ops![A 4, RE 1, A 2, A 4, RE 4, M 2],
            step_ops![A 12, RS 2, P 2, RE 8, A 2],
        ];

        let _path_steps = path_ops
            .iter()
            .enumerate()
            .map(|(id, ops)| {
                paths.with_path_mut_ctx(PathId(id as u64), |ref_mut| {
                    apply_step_ops(ref_mut, &ops)
                })
            })
            .collect::<Vec<_>>();

        let handles_on = |paths: &PackedGraphPaths, id: u64| -> Vec<Handle> {
            let path_ref = paths.path_ref(PathId(id)).unwrap();
            let head = path_ref.properties.head;
            let tail = path_ref.properties.tail;
            let path = path_ref.path;
            packedpath::tests::path_handles(path, head, tail)
        };

        let vectors_for = |paths: &PackedGraphPaths,
                           id: u64|
        // (step_ix, node, prev, next)
         -> Vec<(usize, u64, u64, u64)> {
            let path_ref = paths.path_ref(PathId(id)).unwrap();
            let path = path_ref.path;
            packedpath::tests::path_vectors(path)
        };

        let pre_defrag_steps = (0..=4u64)
            .into_iter()
            .map(|id| handles_on(&paths, id))
            .collect::<Vec<_>>();

        // Several of the paths had some steps removed during their
        // construction, defragmenting will delete those records while
        // leaving the steps (as a series of handles) untouched

        let _updates = paths.defragment();

        let post_defrag_steps = (0..=4u64)
            .into_iter()
            .map(|id| handles_on(&paths, id))
            .collect::<Vec<_>>();

        assert_eq!(pre_defrag_steps, post_defrag_steps);

        // Remove paths 1 and 3

        let _step_updates = paths.remove_path(PathId(1)).unwrap();
        let _step_updates = paths.remove_path(PathId(3)).unwrap();

        let post_removal_steps = (0..=4u64)
            .into_iter()
            .map(|id| handles_on(&paths, id))
            .collect::<Vec<_>>();

        // The only change is that paths 1 and 3 are empty
        assert_eq!(post_removal_steps[0], pre_defrag_steps[0]);
        assert_eq!(post_removal_steps[2], pre_defrag_steps[2]);
        assert_eq!(post_removal_steps[4], pre_defrag_steps[4]);

        assert!(post_removal_steps[1].iter().all(|h| u64::from(h.id()) == 0));
        assert!(post_removal_steps[3].iter().all(|h| u64::from(h.id()) == 0));

        // Defragmenting paths, which will remove paths 1 and 3 and
        // shift the others into the IDs 0, 1, 2

        let _updates = paths.defragment();

        let post_defrag_steps = (0..=2u64)
            .into_iter()
            .map(|id| handles_on(&paths, id))
            .collect::<Vec<_>>();

        // Besides the change in PathId, the remaining paths are
        // unchanged
        assert_eq!(post_defrag_steps[0], pre_defrag_steps[0]);
        assert_eq!(post_defrag_steps[1], pre_defrag_steps[2]);
        assert_eq!(post_defrag_steps[2], pre_defrag_steps[4]);

        assert_eq!(
            vectors_for(&paths, 0),
            vec![
                (1, 1, 0, 2),
                (2, 2, 1, 3),
                (3, 3, 2, 4),
                (4, 4, 3, 5),
                (5, 5, 4, 6),
                (6, 6, 5, 0)
            ]
        );
        assert_eq!(
            vectors_for(&paths, 1),
            vec![
                (1, 1, 0, 2),
                (2, 2, 1, 3),
                (3, 3, 2, 4),
                (4, 4, 3, 5),
                (5, 5, 4, 6),
                (6, 6, 5, 7),
                (7, 9, 6, 8),
                (8, 10, 7, 0)
            ]
        );
        assert_eq!(
            vectors_for(&paths, 2),
            vec![
                (1, 3, 3, 2),
                (2, 4, 1, 5),
                (3, 14, 4, 1),
                (4, 13, 0, 3),
                (5, 15, 2, 6),
                (6, 16, 5, 0)
            ]
        );
    }
}
