use crate::handle::{Direction, Edge, Handle, NodeId};

/// Iterator over all handles in a graph
pub trait AllHandles {
    type Handles: Iterator<Item = Handle>;
    fn all_handles(self) -> Self::Handles;
}

/// Iterator adapter to create an Iterator over `Handle`s from an
/// iterator over &NodeId, in a way that can be used as the `Handles`
/// type in an `AllHandles` implementation.
pub struct NodeIdRefHandles<'a, I>
where
    I: Iterator<Item = &'a NodeId> + 'a,
{
    iter: I,
}

impl<'a, I> NodeIdRefHandles<'a, I>
where
    I: Iterator<Item = &'a NodeId> + 'a,
{
    pub fn new(iter: I) -> Self {
        Self { iter }
    }
}

impl<'a, I> Iterator for NodeIdRefHandles<'a, I>
where
    I: Iterator<Item = &'a NodeId> + 'a,
{
    type Item = Handle;

    #[inline]
    fn next(&mut self) -> Option<Handle> {
        let id = self.iter.next()?.to_owned();
        Some(Handle::pack(id, false))
    }
}

/// Iterator over all edges in a graph
pub trait AllEdges {
    type Edges: Iterator<Item = Edge>;
    fn all_edges(self) -> Self::Edges;
}

/// Utility struct for iterating through the edges of a single handle,
/// for use with EdgesIter
struct HandleEdgesIter<I>
where
    I: Iterator<Item = Handle>,
{
    left_neighbors: Option<I>,
    right_neighbors: Option<I>,
    handle: Handle,
}

impl<I> HandleEdgesIter<I>
where
    I: Iterator<Item = Handle>,
{
    fn new(handle: Handle, left: I, right: I) -> Self {
        Self {
            handle,
            left_neighbors: Some(left),
            right_neighbors: Some(right),
        }
    }

    fn next_left_edge(&mut self) -> Option<Edge> {
        let left_neighbors = self.left_neighbors.as_mut()?;
        loop {
            if let Some(prev_l) = left_neighbors.next() {
                if self.handle.id() < prev_l.id()
                    || self.handle.id() == prev_l.id() && prev_l.is_reverse()
                {
                    return Some(Edge::edge_handle(prev_l, self.handle));
                }
            } else {
                self.left_neighbors = None;
                return None;
            }
        }
    }

    fn next_right_edge(&mut self) -> Option<Edge> {
        let right_neighbors = self.right_neighbors.as_mut()?;
        loop {
            if let Some(next_r) = right_neighbors.next() {
                if self.handle.id() <= next_r.id() {
                    return Some(Edge::edge_handle(self.handle, next_r));
                }
            } else {
                self.right_neighbors = None;
                return None;
            }
        }
    }
}

impl<I> Iterator for HandleEdgesIter<I>
where
    I: Iterator<Item = Handle>,
{
    type Item = Edge;

    #[inline]
    fn next(&mut self) -> Option<Edge> {
        if self.left_neighbors.is_none() && self.right_neighbors.is_none() {
            return None;
        }

        let next = self.next_right_edge();
        if next.is_some() {
            return next;
        }

        let next = self.next_left_edge();
        if next.is_some() {
            return next;
        }

        None
    }
}

impl<I> std::iter::FusedIterator for HandleEdgesIter<I> where
    I: Iterator<Item = Handle>
{
}

/// Utility struct for iterating over all edges of a HandleGraph for
/// which we can already iterate over all handles and their neighbors.
pub struct EdgesIter<G>
where
    G: HandleNeighbors + AllHandles + Copy,
{
    neighbors: Option<HandleEdgesIter<G::Neighbors>>,
    handles: G::Handles,
    graph: G,
}

impl<G> EdgesIter<G>
where
    G: HandleNeighbors + AllHandles + Copy,
{
    pub fn new(graph: G) -> Self {
        let handles = graph.all_handles();
        let mut edges_iter = Self {
            graph,
            handles,
            neighbors: None,
        };

        edges_iter.has_next_handle();
        edges_iter
    }

    fn has_next_handle(&mut self) -> bool {
        if self.neighbors.is_some() {
            true
        } else {
            if let Some(handle) = self.handles.next() {
                let left = self.graph.neighbors(handle, Direction::Left);
                let right = self.graph.neighbors(handle, Direction::Right);
                let neighbors = HandleEdgesIter::new(handle, left, right);
                self.neighbors = Some(neighbors);
                true
            } else {
                false
            }
        }
    }
}

impl<G> Iterator for EdgesIter<G>
where
    G: HandleNeighbors + AllHandles + Copy,
{
    type Item = Edge;

    fn next(&mut self) -> Option<Edge> {
        loop {
            let neighbors = self.neighbors.as_mut()?;
            let next_edge = neighbors.next();
            if next_edge.is_some() {
                return next_edge;
            } else {
                self.neighbors = None;
            }
            if !self.has_next_handle() {
                return None;
            }
        }
    }
}

impl<G> std::iter::FusedIterator for EdgesIter<G> where
    G: HandleNeighbors + AllHandles + Copy
{
}

/// Iterator over the neighbors of a handle in a given direction
///
/// Implementors should make sure that handles are flipped correctly depending on direction, e.g. using NeighborIter
pub trait HandleNeighbors {
    type Neighbors: Iterator<Item = Handle>;
    fn neighbors(self, handle: Handle, dir: Direction) -> Self::Neighbors;
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

/// Iterator over the sequence of a node. Implementors should only
/// define `sequence_iter_impl` that returns a DoubleEndedIterator
/// over the sequence bases in their forward orientation, and users
/// should only use `sequence_iter`, which automatically wraps the
/// iterator so that it steps through the reverse complement when the
/// handle is reversed.
pub trait HandleSequences: Sized {
    type Sequence: Iterator<Item = u8>;
    fn sequence_iter(self, handle: Handle) -> Self::Sequence;
}

/// Iterator adapter that transforms an iterator over ASCII-encoded
/// bases into an iterator over the sequence or its reverse
/// complement.
pub struct SequenceIter<I>
where
    I: Iterator<Item = u8>,
    I: DoubleEndedIterator,
{
    iter: I,
    reversing: bool,
}

impl<I> SequenceIter<I>
where
    I: Iterator<Item = u8>,
    I: DoubleEndedIterator,
{
    pub fn new(iter: I, reversing: bool) -> Self {
        Self { iter, reversing }
    }
}

impl<I> Iterator for SequenceIter<I>
where
    I: Iterator<Item = u8>,
    I: DoubleEndedIterator,
{
    type Item = u8;

    fn next(&mut self) -> Option<u8> {
        if self.reversing {
            self.iter.next_back().map(bio::alphabets::dna::complement)
        } else {
            self.iter.next()
        }
    }
}
