use crate::handle::{Direction, Edge, Handle, NodeId};

/// Iterator over all handles in a graph
pub trait AllHandles {
    type Handles: Iterator<Item = Handle>;
    fn all_handles(self) -> Self::Handles;
}

/// Iterator over all edges in a graph
pub trait AllEdges {
    type Edges: Iterator<Item = Edge>;
    fn all_edges(self) -> Self::Edges;
}

/// Iterator over the neighbors of a handle in a given direction
///
/// Implementors should make sure that handles are flipped correctly depending on direction, e.g. using NeighborIter
pub trait HandleNeighbors {
    type Neighbors: Iterator<Item = Handle>;
    fn neighbors(self, handle: Handle, dir: Direction) -> Self::Neighbors;
}

/// Iterator adapter to create an Iterator over `Handle`s from an
/// iterator over &NodeId, in a way that can be used as the `Handles`
/// type in an `AllHandles` implementation.
pub struct NodeIdRefHandles<I>
where
    I: Iterator,
    I::Item: AsRef<NodeId>,
{
    iter: I,
}

impl<I> NodeIdRefHandles<I>
where
    I: Iterator,
    I::Item: AsRef<NodeId>,
{
    pub fn new(iter: I) -> Self {
        Self { iter }
    }
}

impl<I> Iterator for NodeIdRefHandles<I>
where
    I: Iterator,
    I::Item: AsRef<NodeId>,
{
    type Item = Handle;

    #[inline]
    fn next(&mut self) -> Option<Handle> {
        let id = self.iter.next()?.as_ref().to_owned();
        Some(Handle::pack(id, false))
    }
}

/// Wrapper struct for ensuring handles are flipped correctly when
/// iterating over the neighbors of a handle
pub struct NeighborIter<'a, I>
where
    I: Iterator<Item = &'a Handle>,
{
    flip: bool,
    iter: I,
}

impl<'a, I> NeighborIter<'a, I>
where
    I: Iterator<Item = &'a Handle>,
{
    pub fn new(iter: I, flip: bool) -> Self {
        Self { flip, iter }
    }
}

impl<'a, I> Iterator for NeighborIter<'a, I>
where
    I: Iterator<Item = &'a Handle>,
{
    type Item = Handle;

    #[inline]
    fn next(&mut self) -> Option<Handle> {
        let next = self.iter.next().copied();
        if self.flip {
            next.map(Handle::flip)
        } else {
            next
        }
    }
}
