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

use super::graph::{GraphRecordIx, GraphVecIx};

const fn encode_dna_base(base: u8) -> u64 {
    match base {
        b'a' | b'A' => 0,
        b'c' | b'C' => 1,
        b'g' | b'G' => 2,
        b't' | b'T' => 3,
        _ => 4,
    }
}

const fn encoded_complement(val: u64) -> u64 {
    if val == 4 {
        4
    } else {
        3 - val
    }
}

const fn decode_dna_base(byte: u64) -> u8 {
    match byte {
        0 => b'A',
        1 => b'C',
        2 => b'G',
        3 => b'T',
        _ => b'N',
    }
}

// An index into both the offset record and the length record for some
// sequence. It's a simple index into a packed vector, but the order
// must be the same as the node records vector in the graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SeqRecordIx(usize);

impl SeqRecordIx {
    #[inline]
    pub(super) fn new<I: Into<usize>>(x: I) -> Self {
        Self(x.into())
    }

    pub(super) fn from_graph_record_ix(g_ix: GraphRecordIx) -> Option<Self> {
        let vec_ix = g_ix.as_vec_ix()?;
        Some(Self(vec_ix.seq_record_ix()))
    }

    pub(super) fn as_graph_record_ix(&self) -> GraphRecordIx {
        let vec_ix = GraphVecIx::new(self.0);
        vec_ix.as_record_ix()
    }

    fn as_vec_ix(&self) -> usize {
        self.0
    }
}

#[derive(Debug, Clone)]
pub struct Sequences {
    sequences: PackedIntVec,
    lengths: PackedIntVec,
    offsets: PagedIntVec,
    removed_records: Vec<usize>,
}

impl Default for Sequences {
    fn default() -> Self {
        Sequences {
            sequences: Default::default(),
            lengths: Default::default(),
            offsets: PagedIntVec::new(super::graph::NARROW_PAGE_WIDTH),
            removed_records: Vec::new(),
        }
    }
}

impl Sequences {
    /// Add a new, empty sequence record if the next sequence record
    /// index matches the provided graph record index. If they do not
    /// match, abort and return `false`, return `true` if the records
    /// were successfully appended.
    pub(super) fn append_empty_record(&mut self, g_ix: GraphRecordIx) -> bool {
        let seq_ix = SeqRecordIx::new(self.lengths.len());
        let seq_g_ix = seq_ix.as_graph_record_ix();
        if seq_g_ix != g_ix {
            return false;
        }
        self.lengths.append(0);
        self.offsets.append(0);
        true
    }

    fn append_record(&mut self, offset: usize, length: usize) -> SeqRecordIx {
        let seq_ix = SeqRecordIx::new(self.lengths.len());

        self.lengths.append(length as u64);
        self.offsets.append(offset as u64);
        seq_ix
    }

    #[must_use]
    pub(super) fn append_sequence(
        &mut self,
        g_ix: GraphRecordIx,
        seq: &[u8],
    ) -> Option<SeqRecordIx> {
        let seq_ix = SeqRecordIx::new(self.lengths.len());

        if !self.append_empty_record(g_ix) {
            return None;
        }

        let seq_len = seq.len() as u64;
        self.lengths.set(seq_ix.as_vec_ix(), seq_len as u64);

        let seq_offset = self.sequences.len();
        self.offsets.set(seq_ix.as_vec_ix(), seq_offset as u64);

        seq.iter()
            .for_each(|&b| self.sequences.append(encode_dna_base(b)));

        Some(seq_ix)
    }

    /// Splits the sequence at the provided `SeqRecordIx` into
    /// multiple sequences, using the provided slice of `lengths` to
    /// create the new sequence records.

    /// The first element of the `lengths` slice is the new length of
    /// the sequence at the provided `seq_ix`; the second slice is the
    /// length of the first new sequence record, and so on. The sum of
    /// the provided lengths must be less than or equal to the length
    /// of the original sequence; if it's shorter, a final sequence
    /// record will be added with the missing length.
    ///
    /// Returns `None` if the `lengths` slice is somehow incorrect,
    /// otherwise returns the new indices of the new sequence records.
    /// New nodes/graph records *must* be added to match the new
    /// sequence records.
    #[must_use]
    pub(super) fn split_sequence(
        &mut self,
        seq_ix: SeqRecordIx,
        lengths: &[usize],
    ) -> Option<Vec<SeqRecordIx>> {
        let seq_offset = self.offsets.get(seq_ix.as_vec_ix()) as usize;
        let seq_len = self.lengths.get(seq_ix.as_vec_ix()) as usize;

        let lengths_sum: usize = lengths.iter().sum();
        if lengths_sum > seq_len {
            return None;
        }

        let extra_record = seq_len - lengths_sum;
        let new_count = if extra_record == 0 {
            lengths.len()
        } else {
            lengths.len() + 1
        };

        // shorten the length of the original record
        self.lengths.set(seq_ix.as_vec_ix(), lengths[0] as u64);

        let mut results = Vec::with_capacity(new_count);

        let mut offset = seq_offset + lengths[0];

        for &len in lengths.iter().skip(1) {
            let new_seq_ix = self.append_record(offset, len);
            results.push(new_seq_ix);
            offset += len;
        }

        if extra_record > 0 {
            let new_seq_ix = self.append_record(offset, extra_record);
            results.push(new_seq_ix);
        }

        Some(results)
    }

    pub(super) fn iter(
        &self,
        seq_ix: SeqRecordIx,
        reverse: bool,
    ) -> PackedSeqIter<'_> {
        let offset = self.offsets.get(seq_ix.as_vec_ix()) as usize;
        let len = self.lengths.get(seq_ix.as_vec_ix()) as usize;

        let iter = self.sequences.iter_slice(offset, len);

        PackedSeqIter {
            iter,
            length: len,
            reverse,
        }
    }
}

impl Sequences {
    pub const SIZE: usize = 1;

    pub(super) fn add_record(&mut self, ix: usize, seq: &[u8]) {
        unimplemented!();
        // let seq_ix = self.sequences.len();
        // self.indices.set(ix, seq_ix as u64);
        // self.lengths.set(ix, seq.len() as u64);
        // seq.iter()
        //     .for_each(|&b| self.sequences.append(encode_dna_base(b)));
    }

    pub(super) fn set_record(
        &mut self,
        rec_ix: usize,
        seq_ix: usize,
        length: usize,
    ) {
        unimplemented!();
        // self.indices.set(rec_ix, seq_ix as u64);
        // self.lengths.set(rec_ix, length as u64);
    }

    #[inline]
    pub(super) fn length(&self, ix: usize) -> usize {
        unimplemented!();
        // self.lengths.get(ix) as usize
    }

    #[inline]
    pub(super) fn total_length(&self) -> usize {
        unimplemented!();
        // self.lengths.iter().sum::<u64>() as usize
    }

    #[inline]
    pub(super) fn base(&self, seq_ix: usize, base_ix: usize) -> u8 {
        unimplemented!();
        // let len = self.lengths.get(seq_ix) as usize;
        // assert!(base_ix < len);
        // let offset = self.indices.get(seq_ix) as usize;
        // let base = self.sequences.get(offset + base_ix);
        // decode_dna_base(base)
    }

    /*
    pub(super) fn iter(
        &self,
        seq_ix: usize,
        reverse: bool,
    ) -> PackedSeqIter<'_> {
        unimplemented!();
        // let offset = self.indices.get(seq_ix) as usize;
        // let len = self.lengths.get(seq_ix) as usize;

        // let iter = self.sequences.iter_slice(offset, len);

        // PackedSeqIter {
        //     iter,
        //     length: len,
        //     reverse,
        // }
    }
    */

    pub(super) fn divide_sequence(
        &mut self,
        seq_ix: usize,
        lengths: Vec<usize>,
        // ) -> Vec<(usize, usize)> {
    ) -> Vec<usize> {
        unimplemented!();
        // let mut results = Vec::new();

        // let offset = self.indices.get(seq_ix) as usize;
        // let len = self.lengths.get(seq_ix) as usize;

        // let mut indices = Vec::new();
        // let mut start = offset;

        // // for &l in lengths.iter().skip(1) {
        // for &l in lengths.iter() {
        //     start += l;
        //     indices.push(start);
        // }

        // /*
        // let indices = lengths
        //     .iter()
        //     .copied()
        //     .map(|l| l + offset)
        //     .collect::<Vec<_>>();
        // */

        // // create new records
        // // for (&i, &l) in indices.iter().skip(1).zip(lengths.iter().skip(1)) {
        // // for (&i, &l) in indices.iter().skip(1).zip(lengths.iter()) {
        // for (&i, &l) in indices.iter().zip(lengths.iter()) {
        //     results.push(self.lengths.len());
        //     self.lengths.append(l as u64);
        //     self.indices.append((i - 1) as u64);
        //     // results.push((i, l));
        // }

        // // update the original sequence
        // self.lengths.set(seq_ix, lengths[0] as u64);

        // results
    }
}

pub struct PackedSeqIter<'a> {
    iter: PackedIntVecIter<'a>,
    length: usize,
    reverse: bool,
}

impl<'a> Iterator for PackedSeqIter<'a> {
    type Item = u8;

    fn next(&mut self) -> Option<u8> {
        if self.reverse {
            let base = self.iter.next_back()?;
            Some(decode_dna_base(encoded_complement(base)))
        } else {
            let base = self.iter.next()?;
            Some(decode_dna_base(base))
        }
    }
}

impl<'a> std::iter::ExactSizeIterator for PackedSeqIter<'a> {
    fn len(&self) -> usize {
        self.length
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packedgraph_split_sequence() {
        let mut seqs = Sequences::default();
        let g0 = GraphRecordIx::from_vec_value(1);

        let s0 = seqs.append_sequence(g0, b"GTCCACTTTGTGT").unwrap();
        use bstr::{BString, B};

        let hnd = |x: u64| Handle::pack(x, false);

        let seq_bstr = |sq: &Sequences, ix: SeqRecordIx| -> BString {
            sq.iter(ix, false).collect()
        };
        assert_eq!(B("GTCCACTTTGTGT"), seq_bstr(&seqs, s0));

        let lens = vec![6, 3, 4];

        let seq_indices = seqs.split_sequence(s0, &lens).unwrap();

        assert_eq!(B("GTCCAC"), seq_bstr(&seqs, s0));

        assert_eq!(B("TTT"), seq_bstr(&seqs, seq_indices[0]));
        assert_eq!(B("GTGT"), seq_bstr(&seqs, seq_indices[1]));
    }
}
