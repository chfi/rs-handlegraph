use crate::packed::{self, *};

use super::{
    defragment::Defragment,
    graph::{NodeRecordId, RecordIndex},
    index::OneBasedIndex,
};

// An index into both the offset record and the length record for some
// sequence. It's a simple index into a packed vector, but the order
// must be the same as the node records vector in the graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SeqRecordIx(usize);

crate::impl_space_usage_stack_newtype!(SeqRecordIx);

impl SeqRecordIx {
    #[inline]
    fn new<I: Into<usize>>(x: I) -> Self {
        Self(x.into())
    }
}

impl RecordIndex for SeqRecordIx {
    const RECORD_WIDTH: usize = 1;

    #[inline]
    fn from_one_based_ix<I: OneBasedIndex>(ix: I) -> Option<Self> {
        ix.to_record_start(Self::RECORD_WIDTH).map(SeqRecordIx)
    }

    #[inline]
    fn to_one_based_ix<I: OneBasedIndex>(self) -> I {
        I::from_record_start(self.0, Self::RECORD_WIDTH)
    }

    #[inline]
    fn record_ix(self, _: usize) -> usize {
        self.0
    }
}

#[derive(Debug, Clone)]
pub struct Sequences {
    pub sequences: FlexPagedVec,
    pub lengths: PackedIntVec,
    pub offsets: PagedIntVec,
    pub removed_records: Vec<SeqRecordIx>,
}

crate::impl_space_usage!(
    Sequences,
    [sequences, lengths, offsets, removed_records]
);

impl Default for Sequences {
    fn default() -> Self {
        Sequences {
            sequences: FlexPagedVec::new(2, 8_388_608),
            lengths: Default::default(),
            offsets: PagedIntVec::new(super::graph::NARROW_PAGE_WIDTH),
            removed_records: Vec::new(),
        }
    }
}

impl Sequences {
    /// Add a new, empty sequence record.

    pub(super) fn append_empty_record(&mut self) {
        self.lengths.append(0);
        self.offsets.append(0);
    }

    fn set_record(
        &mut self,
        seq_ix: SeqRecordIx,
        offset: usize,
        length: usize,
    ) {
        let ix = seq_ix.at_0();
        self.lengths.set_pack(ix, length);
        self.offsets.set_pack(ix, offset);
    }

    fn get_record(&self, seq_ix: SeqRecordIx) -> (usize, usize) {
        let ix = seq_ix.at_0();

        let offset: usize = self.offsets.get_unpack(ix);
        let length: usize = self.lengths.get_unpack(ix);

        (offset, length)
    }

    pub(super) fn clear_record(&mut self, seq_ix: SeqRecordIx) {
        let ix = seq_ix.at_0();

        self.offsets.set(ix, 0);
        self.lengths.set(ix, 0);

        self.removed_records.push(seq_ix);
    }

    fn append_record(&mut self, offset: usize, length: usize) -> SeqRecordIx {
        let seq_ix = SeqRecordIx::new(self.lengths.len());

        self.lengths.append(length as u64);
        self.offsets.append(offset as u64);
        seq_ix
    }

    /// Adds a sequence and updates the sequence records for the
    /// provided `NodeRecordId` to the correct length and offset.
    pub(super) fn add_sequence(
        &mut self,
        rec_id: NodeRecordId,
        seq: &[u8],
    ) -> Option<SeqRecordIx> {
        let seq_ix = SeqRecordIx::from_one_based_ix(rec_id)?;

        let len = seq.len();
        let offset = self.sequences.len();

        self.set_record(seq_ix, offset, len);

        self.sequences
            .append_iter(3, seq.iter().map(|&b| encode_dna_base(b)));

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

        let (old_len, offset) = self.get_record(seq_ix);

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
        let (seq_offset, seq_len) = self.get_record(seq_ix);

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
        self.lengths.set_pack(seq_ix.at_0(), lengths[0]);

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
        self.lengths.get_unpack(seq_ix.at_0())
    }

    #[inline]
    pub(super) fn total_length(&self) -> usize {
        self.lengths.iter().sum::<u64>() as usize
    }

    fn iter_impl(
        &self,
        offset: usize,
        len: usize,
        reverse: bool,
    ) -> PackedSeqIter<'_> {
        let iter = self.sequences.iter_slice(offset, len).unwrap();

        PackedSeqIter {
            iter,
            length: len,
            reverse,
        }
    }

    pub(super) fn iter(
        &self,
        seq_ix: SeqRecordIx,
        reverse: bool,
    ) -> PackedSeqIter<'_> {
        let (offset, len) = self.get_record(seq_ix);
        self.iter_impl(offset, len, reverse)
    }
}

impl Defragment for Sequences {
    /// Unlike Defragment implementations for things like edges, where
    /// the pre-defragmentation identifiers are used by other objects,
    /// and the updates need to be provided to ensure other objects
    /// don't hold invalidated indices, that's not the case here, as
    /// the sequence offsets are internal.
    type Updates = ();

    fn defragment(&mut self) -> Option<()> {
        let total_len = self.offsets.len();
        let mut next_offset = 0;
        let mut new_seqs = Self::default();

        new_seqs
            .lengths
            .reserve(self.offsets.len() - self.removed_records.len());

        new_seqs
            .offsets
            .reserve(self.offsets.len() - self.removed_records.len());

        for ix in 0..total_len {
            let seq_ix = SeqRecordIx(ix);
            let (old_offset, length) = self.get_record(seq_ix);
            if length != 0 {
                let new_offset = next_offset;

                new_seqs.lengths.append(length as u64);
                new_seqs.offsets.append(new_offset as u64);

                let seq_iter = self.iter_impl(old_offset, length, false);
                new_seqs
                    .sequences
                    .append_iter(3, seq_iter.map(encode_dna_base));

                next_offset += length;
            }
        }

        crate::assign_for_fields!(
            self,
            new_seqs,
            [sequences, lengths, offsets],
            |mut x| std::mem::take(&mut x)
        );

        self.removed_records.clear();

        Some(())
    }
}

pub struct PackedSeqIter<'a> {
    iter: packed::vector::Iter<'a>,
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

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }

}

impl<'a> std::iter::ExactSizeIterator for PackedSeqIter<'a> {
    #[inline]
    fn len(&self) -> usize {
        self.length
    }
}

const fn dna_encoding_table() -> [u64; 256] {
    let mut table: [u64; 256] = [4; 256];

    let pairs: [(u8, u64); 8] = [
        (b'a', 0),
        (b'A', 0),
        (b'c', 1),
        (b'C', 1),
        (b'g', 2),
        (b'G', 2),
        (b't', 3),
        (b'T', 3),
    ];

    let mut i = 0;

    while i < 8 {
        let (base, value) = pairs[i];
        table[base as usize] = value;
        i += 1;
    }

    table
}

const fn encoded_complement_table() -> [u64; 5] {
    let mut table: [u64; 5] = [0; 5];
    let mut i = 0;

    while i < 5 {
        if i == 4 {
            table[i] = 4;
        } else {
            table[i] = 3 - (i as u64);
        }
        i += 1;
    }
    table
}

pub(crate) const DNA_ENCODING_TABLE: [u64; 256] = dna_encoding_table();
pub(crate) const ENCODED_COMPLEMENT_TABLE: [u64; 5] =
    encoded_complement_table();
pub(crate) const DNA_DECODING_TABLE: [u8; 5] = [b'A', b'C', b'G', b'T', b'N'];

#[inline]
pub(crate) const fn encode_dna_base(base: u8) -> u64 {
    DNA_ENCODING_TABLE[base as usize]
}

#[inline]
pub(crate) const fn encoded_complement(val: u64) -> u64 {
    if val > 3 {
        ENCODED_COMPLEMENT_TABLE[4]
    } else {
        let v = val as u8;
        ENCODED_COMPLEMENT_TABLE[v as usize]
    }
}

#[inline]
pub(crate) const fn decode_dna_base(val: u64) -> u8 {
    if val > 3 {
        DNA_DECODING_TABLE[4]
    } else {
        DNA_DECODING_TABLE[val as usize]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packedgraph_split_sequence() {
        use bstr::{BString, B};
        let mut seqs = Sequences::default();
        let g0 = NodeRecordId::unpack(1);
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

    #[test]
    fn defragment_sequence() {
        let mut seqs = Sequences::default();
        let g0 = NodeRecordId::unpack(1);
        let g1 = NodeRecordId::unpack(2);
        let g2 = NodeRecordId::unpack(3);
        let g3 = NodeRecordId::unpack(4);
        seqs.append_empty_record();
        seqs.append_empty_record();
        seqs.append_empty_record();
        seqs.append_empty_record();

        let s0 = seqs.add_sequence(g0, b"GTCCACTTTGTGT").unwrap();
        let s1 = seqs.add_sequence(g1, b"GTCCAGT").unwrap();
        let s2 = seqs.add_sequence(g2, b"CACGCTGT").unwrap();
        let s3 = seqs.add_sequence(g3, b"AAATGTAAA").unwrap();

        let total_len = seqs.sequences.len();
        assert_eq!(total_len, 37);

        assert_eq!(seqs.get_record(s0), (0, 13));
        assert_eq!(seqs.get_record(s1), (13, 7));
        assert_eq!(seqs.get_record(s2), (20, 8));
        assert_eq!(seqs.get_record(s3), (28, 9));

        seqs.clear_record(s2);

        assert_eq!(seqs.get_record(s0), (0, 13));
        assert_eq!(seqs.get_record(s1), (13, 7));
        assert_eq!(seqs.get_record(s2), (0, 0));
        assert_eq!(seqs.get_record(s3), (28, 9));

        let _new_offsets = seqs.defragment().unwrap();

        let total_len = seqs.sequences.len();
        assert_eq!(total_len, 29);

        assert_eq!(seqs.get_record(s0), (0, 13));
        assert_eq!(seqs.get_record(s1), (13, 7));
        assert_eq!(seqs.get_record(s2), (20, 9));

        seqs.clear_record(s0);

        assert_eq!(seqs.get_record(s0), (0, 0));
        assert_eq!(seqs.get_record(s1), (13, 7));
        assert_eq!(seqs.get_record(s2), (20, 9));

        let _new_offsets = seqs.defragment().unwrap();

        let total_len = seqs.sequences.len();
        assert_eq!(total_len, 16);

        assert_eq!(seqs.get_record(s0), (0, 7));
        assert_eq!(seqs.get_record(s1), (7, 9));
    }
}
