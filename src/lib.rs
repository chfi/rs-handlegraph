/*!
A Rusty take on the
[`libhandlegraph`](https://github.com/vgteam/libhandlegraph)
interface for variation graph access and manipulation.

# Overview

This crate provides a number of traits that together encapsulate the
handlegraph interface. While these traits do not form a direct mirror
of the C++ interface, (almost) all of the features should exist and be
usable.

In addition to the abstract traits, there are currently two concrete
handlegraphs that implement them:

* [`HashGraph`](hashgraph::HashGraph) is a `HashMap`-based graph that
  does nothing to reduce its memory footprint, but is fast.
* [`PackedGraph`](packedgraph::PackedGraph) uses packed integer
  vectors to greatly reduce memory usage.


# The interface

The handlegraph interface is split into three categories, each
consisting of a number of traits that encapsulate a subset of the
functionality implied by their category.

* [`handlegraph`] is for immutable access to the nodes and edges of a graph
* [`mutablehandlegraph`] is for mutable access to nodes and edges
* [`pathhandlegraph`] is for both immutable and mutable access to the paths embedded in a graph


# `Handle`s and `NodeId`s

The core types, used all over the various traits, are defined in [`handle`]:

* [`NodeId`](handle::NodeId) is a newtype used as a node identifier
* [`Handle`](handle::Handle) represents a specific orientation of a node
* [`Edge`](handle::Edge) is a newtype for edges in a specific order

# Misc.

* [`conversion`] has some functions for converting from GFA to a handlegraph and back
* [`packed`] is where the packed vector collection types used by `PackedGraph` are implemented

*/

pub mod handle;

pub mod handlegraph;
pub mod mutablehandlegraph;
pub mod pathhandlegraph;

pub mod hashgraph;
pub mod packedgraph;

pub mod conversion;
pub mod disjoint;
pub mod packed;
pub mod util;

pub mod algorithms;
pub mod consensus;

pub mod path_position;
