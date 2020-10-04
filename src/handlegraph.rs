use crate::handle::{Direction, Edge, Handle, NodeId};

/// Trait encapsulating the immutable aspects of a handlegraph
pub trait HandleGraph {
    fn has_node(&self, node_id: NodeId) -> bool;

    /// The length of the sequence of a given node
    fn length(&self, handle: Handle) -> usize;

    /// Returns the sequence of a node in the handle's local forward
    /// orientation. Copies the sequence, as the sequence in the graph
    /// may be reversed depending on orientation.
    fn sequence(&self, handle: Handle) -> Vec<u8>;

    /// Returns a slice with sequence for a node's handle. Avoids
    /// copying but doesn't have to take the handle orientation into
    /// account!
    fn sequence_slice(&self, handle: Handle) -> &[u8];

    fn sequence_iter(&self, handle: Handle) -> SeqIter<'_> {
        let seq = self.sequence_slice(handle);
        SeqIter::new(seq, handle.is_reverse())
    }

    fn subsequence(
        &self,
        handle: Handle,
        index: usize,
        size: usize,
    ) -> Vec<u8> {
        self.sequence(handle)[index..index + size].into()
    }

    fn base(&self, handle: Handle, index: usize) -> u8 {
        self.sequence(handle)[index]
    }

    fn min_node_id(&self) -> NodeId;
    fn max_node_id(&self) -> NodeId;

    /// Return the total number of nodes in the graph
    fn node_count(&self) -> usize;

    /// Return the total number of edges in the graph
    fn edge_count(&self) -> usize;

    fn degree(&self, handle: Handle, dir: Direction) -> usize {
        self.handle_edges_iter(handle, dir).fold(0, |a, _| a + 1)
    }

    fn has_edge(&self, left: Handle, right: Handle) -> bool {
        self.handle_edges_iter(left, Direction::Right)
            .any(|h| h == right)
    }

    /// Sum up all the sequences in the graph
    fn total_length(&self) -> usize {
        self.handles_iter()
            .fold(0, |a, v| a + self.sequence(v).len())
    }

    fn traverse_edge_handle(&self, edge: &Edge, left: Handle) -> Handle {
        let Edge(el, er) = *edge;

        if left == el {
            er
        } else if left == er.flip() {
            el.flip()
        } else {
            // TODO this should be improved -- this whole function, really
            panic!("traverse_edge_handle called with a handle that the edge didn't connect");
        }
    }

    /// Returns an iterator over the neighbors of a handle in a
    /// given direction
    fn handle_edges_iter<'a>(
        &'a self,
        handle: Handle,
        dir: Direction,
    ) -> Box<dyn Iterator<Item = Handle> + 'a>;

    /// Returns an iterator over all the handles in the graph
    fn handles_iter<'a>(&'a self) -> Box<dyn Iterator<Item = Handle> + 'a>;

    /// Returns an iterator over all the edges in the graph
    fn edges_iter<'a>(&'a self) -> Box<dyn Iterator<Item = Edge> + 'a>;
}

/// An iterator over a sequence that takes orientation into account.
pub struct SeqIter<'a> {
    slice: &'a [u8],
    reversing: bool,
    index: usize,
}

impl<'a> SeqIter<'a> {
    fn new(sequence: &'a [u8], reversing: bool) -> SeqIter<'a> {
        let index = if reversing { sequence.len() - 1 } else { 0 };
        SeqIter {
            slice: sequence,
            reversing,
            index,
        }
    }
}

impl<'a> Iterator for SeqIter<'a> {
    type Item = u8;

    #[inline]
    fn next(&mut self) -> Option<u8> {
        if self.reversing && self.index > 0 {
            let out = bio::alphabets::dna::complement(self.slice[self.index]);
            self.index -= 1;
            Some(out)
        } else if !self.reversing && self.index < self.slice.len() {
            let out = self.slice[self.index];
            self.index += 1;
            Some(out)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seq_iter_forward() {
        let seq = b"AGCTYRWSKMDVHBN";
        let seq_iter = SeqIter::new(seq, false);
        let iter_out = seq_iter.collect::<Vec<u8>>();
        assert_eq!(iter_out, Vec::from(&seq[..]));
    }

    #[test]
    fn seq_iter_reverse() {
        let seq = b"AGCTYRWSKMDVHBN";
        let seq_iter = SeqIter::new(seq, true);
        let iter_out = seq_iter.collect::<Vec<u8>>();
        assert_eq!(&iter_out, b"NVDBHKMSWYRAGC");
    }
}
