use crate::packed::*;

use super::graph::{GraphRecordIx, NodeRecordId, RecordIndex};

use super::index::OneBasedIndex;

#[inline]
const fn encode_dna_base(base: u8) -> u64 {
    match base {
        b'a' | b'A' => 0,
        b'c' | b'C' => 1,
        b'g' | b'G' => 2,
        b't' | b'T' => 3,
        _ => 4,
    }
}

#[inline]
const fn encoded_complement(val: u64) -> u64 {
    if val == 4 {
        4
    } else {
        3 - val
    }
}

#[inline]
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

    /*
    #[inline]
    pub(super) fn from_graph_record_ix(g_ix: GraphRecordIx) -> Option<Self> {
        if g_ix.is_null() {
            return None;
        }
        let s_ix = (g_ix.as_vec_value() - 1) as usize;

        Some(Self(s_ix))
    }
    */

    #[inline]
    fn as_vec_ix(&self) -> usize {
        self.0
    }
}

impl RecordIndex for SeqRecordIx {
    const RECORD_WIDTH: usize = 1;

    #[inline]
    fn from_one_based_ix<I: OneBasedIndex>(ix: I) -> Option<Self> {
        ix.to_record_ix(Self::RECORD_WIDTH).map(SeqRecordIx)
    }

    #[inline]
    fn to_one_based_ix<I: OneBasedIndex>(self) -> I {
        I::from_record_ix(self.0, Self::RECORD_WIDTH)
    }

    #[inline]
    fn to_vector_index(self, _: usize) -> usize {
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
    /// Add a new, empty sequence record.
    pub(super) fn append_empty_record(&mut self) -> bool {
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

    /// Adds a sequence and updates the sequence records for the provided `GraphRecordIx` to the correct length and offset.
    #[must_use]
    pub(super) fn add_sequence(
        &mut self,
        rec_id: NodeRecordId,
        seq: &[u8],
    ) -> Option<SeqRecordIx> {
        let seq_ix = SeqRecordIx::from_one_based_ix(rec_id)?;

        let seq_len = seq.len() as u64;
        let seq_offset = self.sequences.len();

        self.lengths.set(seq_ix.as_vec_ix(), seq_len);
        self.offsets.set(seq_ix.as_vec_ix(), seq_offset as u64);

        seq.iter()
            .for_each(|&b| self.sequences.append(encode_dna_base(b)));

        Some(seq_ix)
    }

    /// Overwrites the sequence for the provided `GraphRecordIx` with
    /// `seq`. The provided sequence must have exactly the same length
    /// as the old one.
    pub(super) fn overwrite_sequence(
        &mut self,
        rec_id: NodeRecordId,
        seq: &[u8],
    ) {
        let seq_ix = SeqRecordIx::from_one_based_ix(rec_id).unwrap();

        let old_len = self.lengths.get(seq_ix.to_vector_index(0)) as usize;
        let offset = self.offsets.get(seq_ix.to_vector_index(0)) as usize;

        assert!(old_len == seq.len());

        for (i, b) in seq.iter().copied().enumerate() {
            let ix = offset + i;
            self.sequences.set(ix, encode_dna_base(b));
        }
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

    #[inline]
    pub(super) fn length(&self, rec_id: NodeRecordId) -> usize {
        let seq_ix = SeqRecordIx::from_one_based_ix(rec_id).unwrap();
        self.lengths.get(seq_ix.as_vec_ix()) as usize
    }

    #[inline]
    pub(super) fn total_length(&self) -> usize {
        self.lengths.iter().sum::<u64>() as usize
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

pub struct PackedSeqIter<'a> {
    iter: PackedIntVecIter<'a>,
    length: usize,
    reverse: bool,
}

impl<'a> Iterator for PackedSeqIter<'a> {
    type Item = u8;

    #[inline]
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
    #[inline]
    fn len(&self) -> usize {
        self.length
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packedgraph_split_sequence() {
        use bstr::{BString, B};
        let mut seqs = Sequences::default();
        let g0 = GraphRecordIx::from_vec_value(1);
        seqs.append_empty_record();

        let s0 = seqs.add_sequence(g0, b"GTCCACTTTGTGT").unwrap();

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
