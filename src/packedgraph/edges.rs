#![allow(dead_code)]
#![allow(unused_assignments)]
#![allow(unused_variables)]
#![allow(unused_mut)]
#![allow(unused_imports)]

use gfa::{
    gfa::{Link, Orientation, Segment, GFA},
    optfields::OptFields,
};

use crate::{
    handle::{Direction, Edge, Handle, NodeId},
    handlegraph::HandleGraph,
    mutablehandlegraph::MutableHandleGraph,
    packed::*,
};

use std::num::NonZeroUsize;

/// The index for an edge record. Valid indices are natural numbers
/// starting from 1, each denoting a *record*. An edge list index of
/// zero denotes a lack of an edge, or the empty edge list.
///
/// As zero is used to represent no edge/the empty edge list,
/// `Option<NonZeroUsize>` is a natural fit for representing this.
#[derive(Debug, Clone, Copy)]
pub struct EdgeListIx(Option<NonZeroUsize>);

impl EdgeListIx {
    /// Create a new `EdgeListIx` by wrapping a `usize`. Should only
    /// be used in the PackedGraph edge list internals.
    #[inline]
    fn new<I: Into<usize>>(x: I) -> Self {
        Self(NonZeroUsize::new(x.into()))
    }

    /// Transforms the `EdgeListIx` into an index that can be used to
    /// get the first element of a record from an edge list vector.
    /// Returns None if the `EdgeListIx` is None.
    ///
    /// `x -> (x - 1) * 2`
    #[inline]
    pub fn as_vec_ix(&self) -> Option<EdgeVecIx> {
        let x = self.0?.get();
        Some(EdgeVecIx((x - 1) * 2))
    }
}

/// The index into the underlying packed vector that is used to
/// represent the edge lists.

/// Each edge list record takes up two elements, so an `EdgeVecIx` is
/// always even. They also start from zero, so there's an offset by one
/// compared to `EdgeListIx`, besides the record size.
#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct EdgeVecIx(usize);

impl EdgeVecIx {
    /// Create a new `EdgeVecIx` by wrapping a `usize`. Should only
    /// be used in the PackedGraph edge list internals.
    #[inline]
    fn new<I: Into<usize>>(x: I) -> Self {
        Self(x.into())
    }

    /// Transforms the `EdgeVecIx` into an index that denotes a record
    /// in the edge list. The resulting `EdgeListIx` will always
    /// contain a value, never `None`.
    ///
    /// `x -> (x / 2) + 1`
    #[inline]
    pub fn as_list_ix(&self) -> EdgeListIx {
        EdgeListIx::new((self.0 / 2) + 1)
    }

    #[inline]
    fn handle_ix(&self) -> usize {
        self.0
    }

    #[inline]
    fn next_ix(&self) -> usize {
        self.0 + 1
    }
}
