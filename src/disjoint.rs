/*!
Rust implementation of the lock-free parallel disjoint set described
in the paper "Wait-free Parallel Algorithms for the Union-Find
Problem" by Richard J. Anderson and Heather Woll.

Based on the C++ implementation from
`https://github.com/vgteam/odgi/blob/master/src/dset64-gccAtomic.hpp`
*/

use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

pub struct DisjointSets {
    data: Vec<AtomicU64>,
    len: usize,
}

impl DisjointSets {
    pub fn new(size: usize) -> Self {
        let mut data = Vec::with_capacity(size * 2);
        for i in 0..size {
            data.push(AtomicU64::new(0));
            data.push(AtomicU64::new(i as u64));
        }
        Self { data, len: size }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }

    #[inline]
    pub fn rank(&self, id: u64) -> u64 {
        let ix = (id * 2) as usize;
        self.data[ix].load(Ordering::SeqCst)
    }

    #[inline]
    pub fn parent(&self, id: u64) -> u64 {
        let ix = ((id * 2) + 1) as usize;
        self.data[ix].load(Ordering::SeqCst)
    }

    #[inline]
    pub fn find(&self, mut id: u64) -> u64 {
        while id != self.parent(id) {
            let parent = self.parent(id);
            let new_parent = self.parent(parent);

            let ix = ((id * 2) + 1) as usize;
            self.data[ix].compare_and_swap(
                parent,
                new_parent,
                Ordering::SeqCst,
            );

            id = new_parent;
        }
        id
    }

    #[inline]
    pub fn same(&self, mut id_1: u64, mut id_2: u64) -> bool {
        loop {
            id_1 = self.find(id_1);
            id_2 = self.find(id_2);

            if id_1 == id_2 {
                return true;
            }
            if self.parent(id_1) == id_1 {
                return false;
            }
        }
    }

    #[inline]
    pub fn unite(&self, mut id_1: u64, mut id_2: u64) -> u64 {
        loop {
            id_1 = self.find(id_1);
            id_2 = self.find(id_2);

            if id_1 == id_2 {
                return id_1;
            }

            let mut r_1 = self.rank(id_1);
            let mut r_2 = self.rank(id_2);

            if r_1 > r_2 || (r_1 == r_2 && id_1 < id_2) {
                std::mem::swap(&mut r_1, &mut r_2);
                std::mem::swap(&mut id_1, &mut id_2);
            }

            let rank_ix = (id_1 * 2) as usize;
            let parent_ix = ((id_1 * 2) + 1) as usize;

            let old_parent = self.data[parent_ix].compare_and_swap(
                id_1,
                id_2,
                Ordering::SeqCst,
            );

            if old_parent != id_1 {
                continue;
            }

            if r_1 == r_2 {
                let _old_rank = self.data[rank_ix].compare_and_swap(
                    r_2,
                    r_2 + 1,
                    Ordering::SeqCst,
                );
            }

            break;
        }

        id_2
    }
}
