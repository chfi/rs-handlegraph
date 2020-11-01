use crate::{
    handle::{Direction, Edge, Handle, NodeId},
    handlegraph::*,
    mutablehandlegraph::{AdditiveHandleGraph, MutableHandleGraph},
};

use std::num::NonZeroUsize;

use super::GraphRecordIx;

use crate::pathhandlegraph::*;

use crate::packed::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PackedPathStep(Option<NonZeroUsize>);

impl PackedPathStep {
    #[inline]
    fn new<I: Into<usize>>(x: I) -> Self {
        Self(NonZeroUsize::new(x.into()))
    }

    #[inline]
    fn from_zero_based(x: usize) -> Self {
        let x = x + 1;
        Self::new(x)
    }

    #[inline]
    fn to_zero_based(self) -> Option<usize> {
        if let Some(ix) = self.0 {
            Some(ix.get() - 1)
        } else {
            None
        }
    }

    #[inline]
    #[allow(dead_code)]
    pub(super) fn empty() -> Self {
        Self(None)
    }
    #[inline]
    pub(super) fn is_null(&self) -> bool {
        self.0.is_none()
    }

    #[inline]
    pub(super) fn as_vec_value(&self) -> u64 {
        match self.0 {
            None => 0,
            Some(v) => v.get() as u64,
        }
    }

    #[inline]
    pub(super) fn from_vec_value(x: u64) -> Self {
        Self(NonZeroUsize::new(x as usize))
    }
}

pub struct PackedPath {
    steps: RobustPagedIntVec,
    links: RobustPagedIntVec,
    path_id: PathId,
    head: PackedPathStep,
    tail: PackedPathStep,
}

impl PackedPath {
    pub(super) fn new(path_id: PathId) -> Self {
        Self {
            path_id,
            steps: RobustPagedIntVec::new(super::graph::NARROW_PAGE_WIDTH),
            links: RobustPagedIntVec::new(super::graph::NARROW_PAGE_WIDTH),
            head: PackedPathStep::empty(),
            tail: PackedPathStep::empty(),
        }
    }

    pub fn len(&self) -> usize {
        self.steps.len()
    }

    pub(super) fn append_step(&mut self, handle: Handle) -> PackedPathStep {
        let ix = PackedPathStep::from_zero_based(self.len());
        self.steps.append(handle.as_integer());

        if self.head.is_null() {
            self.head = PackedPathStep::from_zero_based(0);
            self.tail = self.head;
        }

        self.links.append(ix as u64);
        self.links.append(0);

        if !self.tail.is_null() {
            // this is definitely super wrong
            self.links
                .set(ix - 1, self.tail.to_zero_based().unwrap() as u64);
        }

        ix
    }

    pub(super) fn prepend_step(&mut self, handle: Handle) -> PackedPathStep {
        let ix = PackedPathStep::from_zero_based(self.len());
        self.steps.append(handle.as_integer());

        if self.head.is_null() {
            self.head = PackedPathStep::from_zero_based(0);
            self.tail = self.head;
        }

        self.links.append(0);
        self.links.append(self.head.to_zero_based().unwrap() as u64);

        // self.links.set(self.hea

        // if !self.tail.is_null() {
        // this is definitely super wrong
        self.links
            .set(ix - 1, self.tail.to_zero_based().unwrap() as u64);
        // }

        ix
    }
}

pub struct PackedPathSteps<'a> {
    path: &'a PackedPath,
    current_step: usize,
    finished: bool,
}

impl<'a> PackedPathSteps<'a> {
    fn new(path: &'a PackedPath) -> Self {
        Self {
            path,
            current_step: 0,
            finished: false,
        }
    }

    /*
    fn next(&mut self) -> Option<(usize, Handle)> {
        if self.finished {
            return None;
        }

        let handle = Handle::from_integer(self.steps.get(self.current_step));
        let index = self.current_step;

        let link = self.current_step += 1;
    }
    */
}
