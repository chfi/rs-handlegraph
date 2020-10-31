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

#[derive(Debug, Clone)]
pub struct Sequences {
    sequences: PackedIntVec,
    pub(super) lengths: PackedIntVec,
    pub(super) indices: PagedIntVec,
}

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

impl Sequences {
    pub const SIZE: usize = 1;

    pub(super) fn add_record(&mut self, ix: usize, seq: &[u8]) {
        let seq_ix = self.sequences.len();
        self.indices.set(ix, seq_ix as u64);
        self.lengths.set(ix, seq.len() as u64);
        seq.iter()
            .for_each(|&b| self.sequences.append(encode_dna_base(b)));
    }

    pub(super) fn set_record(
        &mut self,
        rec_ix: usize,
        seq_ix: usize,
        length: usize,
    ) {
        self.indices.set(rec_ix, seq_ix as u64);
        self.lengths.set(rec_ix, length as u64);
    }

    #[inline]
    pub(super) fn length(&self, ix: usize) -> usize {
        self.lengths.get(ix) as usize
    }

    #[inline]
    pub(super) fn total_length(&self) -> usize {
        self.lengths.iter().sum::<u64>() as usize
    }

    #[inline]
    pub(super) fn base(&self, seq_ix: usize, base_ix: usize) -> u8 {
        let len = self.lengths.get(seq_ix) as usize;
        assert!(base_ix < len);
        let offset = self.indices.get(seq_ix) as usize;
        let base = self.sequences.get(offset + base_ix);
        decode_dna_base(base)
    }

    pub(super) fn iter(
        &self,
        seq_ix: usize,
        reverse: bool,
    ) -> PackedSeqIter<'_> {
        let offset = self.indices.get(seq_ix) as usize;
        let len = self.lengths.get(seq_ix) as usize;

        let iter = self.sequences.iter_slice(offset, len);

        PackedSeqIter {
            iter,
            length: len,
            reverse,
        }
    }

    pub(super) fn divide_sequence(
        &mut self,
        seq_ix: usize,
        lengths: Vec<usize>,
        // ) -> Vec<(usize, usize)> {
    ) -> Vec<usize> {
        let mut results = Vec::new();

        let offset = self.indices.get(seq_ix) as usize;
        let len = self.lengths.get(seq_ix) as usize;

        let mut indices = Vec::new();
        let mut start = offset;

        // for &l in lengths.iter().skip(1) {
        for &l in lengths.iter() {
            start += l;
            indices.push(start);
        }

        /*
        let indices = lengths
            .iter()
            .copied()
            .map(|l| l + offset)
            .collect::<Vec<_>>();
        */

        // create new records
        // for (&i, &l) in indices.iter().skip(1).zip(lengths.iter().skip(1)) {
        // for (&i, &l) in indices.iter().skip(1).zip(lengths.iter()) {
        for (&i, &l) in indices.iter().zip(lengths.iter()) {
            results.push(self.lengths.len());
            self.lengths.append(l as u64);
            self.indices.append((i - 1) as u64);
            // results.push((i, l));
        }

        // update the original sequence
        self.lengths.set(seq_ix, lengths[0] as u64);

        results
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

impl Default for Sequences {
    fn default() -> Self {
        Sequences {
            sequences: Default::default(),
            lengths: Default::default(),
            indices: PagedIntVec::new(super::graph::NARROW_PAGE_WIDTH),
        }
    }
}
