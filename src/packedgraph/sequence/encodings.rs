use crate::packed::{self, *};

const PACKED_BASE_ENCODING: [(u8, u8); 8] = [
    (b'a', 0),
    (b'A', 0),
    (b'c', 1),
    (b'C', 1),
    (b'g', 2),
    (b'G', 2),
    (b't', 3),
    (b'T', 3),
];

const PACKED_BASE_DECODING: [u8; 5] = [b'A', b'C', b'G', b'T', b'N'];

const PACKED_BASE_COMPLEMENT: [u8; 256] = {
    let mut table: [u8; 256] = [4; 256];

    let mut i = 0;
    while i < 4 {
        table[i] = 3 - (i as u8);
        i += 1;
    }

    table
};

// Packed 2-3 bit encoding using u64-size blocks

pub(crate) const DNA_3BIT_ENCODING_TABLE: [u64; 256] = {
    let mut table: [u64; 256] = [4; 256];

    let mut i = 0;
    while i < 8 {
        let (base, value) = PACKED_BASE_ENCODING[i];
        table[base as usize] = value as u64;
        i += 1;
    }

    table
};

#[inline]
pub(crate) const fn encode_dna_base_u64(base: u8) -> u64 {
    DNA_3BIT_ENCODING_TABLE[base as usize]
}

#[inline]
pub(crate) const fn encoded_complement_u64(val: u64) -> u64 {
    PACKED_BASE_COMPLEMENT[(val as u8) as usize] as u64
}

#[inline]
pub(crate) const fn decode_dna_base_u64(val: u64) -> u8 {
    if val > 3 {
        PACKED_BASE_DECODING[4]
    } else {
        PACKED_BASE_DECODING[val as usize]
    }
}

pub struct PackedSeqIter<'a> {
    pub(super) iter: packed::vector::Iter<'a>,
    pub(super) length: usize,
    pub(super) reverse: bool,
}

impl<'a> Iterator for PackedSeqIter<'a> {
    type Item = u8;

    #[inline]
    fn next(&mut self) -> Option<u8> {
        if self.reverse {
            let base = self.iter.next_back()?;
            Some(decode_dna_base_u64(encoded_complement_u64(base)))
        } else {
            let base = self.iter.next()?;
            Some(decode_dna_base_u64(base))
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }

    #[inline]
    fn count(self) -> usize {
        self.length
    }

    #[inline]
    fn last(mut self) -> Option<u8> {
        if self.reverse {
            let base = self.iter.next()?;
            Some(decode_dna_base_u64(encoded_complement_u64(base)))
        } else {
            let base = self.iter.last()?;
            Some(decode_dna_base_u64(base))
        }
    }
}

impl<'a> std::iter::ExactSizeIterator for PackedSeqIter<'a> {
    #[inline]
    fn len(&self) -> usize {
        self.length
    }
}

// Packed 4-bit encoding using u8-size blocks

pub(crate) const DNA_BASE_1_ENCODING_TABLE: [u8; 256] = {
    let mut table: [u8; 256] = [64; 256];
    let mut i = 0;
    while i < 8 {
        let (base, val) = PACKED_BASE_ENCODING[i];
        table[base as usize] = val << 4;
        i += 1;
    }
    table
};

pub(crate) const DNA_BASE_2_ENCODING_TABLE: [u8; 256] = {
    let mut table: [u8; 256] = [4; 256];
    let mut i = 0;
    while i < 8 {
        let (base, val) = PACKED_BASE_ENCODING[i];
        table[base as usize] = val;
        i += 1;
    }

    table
};

#[inline]
pub(crate) const fn encode_dna_base_1_u8(base: u8) -> u8 {
    DNA_BASE_1_ENCODING_TABLE[base as usize]
}

#[inline]
pub(crate) const fn encode_dna_base_2_u8(base: u8) -> u8 {
    DNA_BASE_2_ENCODING_TABLE[base as usize]
}

#[inline]
pub(crate) const fn encode_dna_pair_u8(bases: &[u8; 2]) -> u8 {
    encode_dna_base_1_u8(bases[0]) | encode_dna_base_2_u8(bases[1])
}

pub(crate) const DNA_PAIR_DECODING_TABLE: [[u8; 2]; 256] = {
    let mut table: [[u8; 2]; 256] = [[b'N', b'N']; 256];

    let mut i = 0;
    while i < 5 {
        let base_2 = PACKED_BASE_DECODING[i];
        table[i << 4 | 0xF] = [base_2, 0];

        let mut j = 0;
        while j < 5 {
            let base_1 = PACKED_BASE_DECODING[j];
            table[j << 4 | i] = [base_1, base_2];

            j += 1;
        }
        i += 1;
    }

    table
};

pub(crate) const DNA_PAIR_COMP_DECODING_TABLE: [[u8; 2]; 256] = {
    let mut table: [[u8; 2]; 256] = [[b'N', b'N']; 256];

    let mut i = 0;
    while i < 5 {
        let comp_2 = PACKED_BASE_COMPLEMENT[i];
        let base_2 = PACKED_BASE_DECODING[comp_2 as usize];
        table[i << 4 | 0xF] = [base_2, 0];

        let mut j = 0;
        while j < 5 {
            let comp_1 = PACKED_BASE_COMPLEMENT[j];
            let base_1 = PACKED_BASE_DECODING[comp_1 as usize];
            table[j << 4 | i] = [base_1, base_2];

            j += 1;
        }
        i += 1;
    }

    table
};

#[derive(Debug, Default, Clone)]
pub struct EncodedSequence {
    pub(crate) vec: Vec<u8>,
    len: usize,
}

crate::impl_space_usage!(EncodedSequence, [vec]);

impl EncodedSequence {
    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[inline]
    pub fn read_latest_mask(&self) -> u8 {
        if self.len % 2 == 0 {
            0x0F
        } else {
            0xF0
        }
    }

    #[inline]
    pub fn append_latest_mask(&self) -> u8 {
        if self.len % 2 == 0 {
            0xF0
        } else {
            0x0F
        }
    }

    #[inline]
    pub fn write_base(&mut self, index: usize, base: u8) {
        assert!(index < self.len);
        let slice_index = index / 2;
        let value = &mut self.vec[slice_index];
        if index % 2 == 0 {
            *value = (*value & 0x0F) | encode_dna_base_1_u8(base);
        } else {
            *value = (*value & 0xF0) | encode_dna_base_2_u8(base);
        }
    }

    #[inline]
    pub fn append_base(&mut self, base: u8) -> usize {
        if self.len % 2 == 0 {
            self.vec.push(encode_dna_base_1_u8(base) | 0x0F);
            self.len += 1;
            self.len - 1
        } else {
            if let Some(last) = self.vec.last_mut() {
                *last &= encode_dna_base_2_u8(base) | 0xF0;
                self.len += 1;
                self.len - 1
            } else {
                unreachable!();
            }
        }
    }

    #[inline]
    pub fn get_base(&self, index: usize) -> Option<u8> {
        let slice_index = index / 2;
        let encoded = self.vec.get(slice_index)?;
        if index % 2 == 0 {
            Some(encoded & 0x0F)
        } else {
            Some(encoded & 0xF0)
        }
    }

    #[inline]
    pub fn rewrite_section(&mut self, offset: usize, new_seq: &[u8]) {
        assert!(offset + new_seq.len() <= self.len && !new_seq.is_empty());

        let mut offset = offset;
        let mut new_seq = new_seq;

        if offset % 2 != 0 {
            self.write_base(offset, new_seq[0]);
            offset += 1;
            new_seq = &new_seq[1..];
        }

        for (ix, base) in new_seq.iter().copied().enumerate() {
            self.write_base(offset + ix, base);
        }
    }

    #[inline]
    pub fn append_seq(&mut self, seq: &[u8]) -> usize {
        assert!(!seq.is_empty());
        let offset = self.len;

        if self.len % 2 == 0 {
            let iter = EncodeIterSlice::new(seq);
            self.len += seq.len();
            self.vec.extend(iter);
            offset
        } else {
            if seq.len() == 1 {
                self.append_base(seq[0]);
                offset
            } else {
                self.append_base(seq[0]);
                let iter = EncodeIterSlice::new(&seq[1..]);
                self.len += seq.len() - 1;
                self.vec.extend(iter);
                offset
            }
        }
    }

    pub fn iter(
        &self,
        offset: usize,
        len: usize,
        reverse: bool,
    ) -> DecodeIter<'_> {
        assert!(offset + len <= self.len);
        DecodeIter {
            encoded: &self.vec,
            left: offset,
            right: offset + len - 1,
            reverse,
            done: false,
        }
    }
}

pub struct EncodeIterSlice<'a> {
    iter: std::slice::ChunksExact<'a, u8>,
    last: Option<u8>,
    done: bool,
}

impl<'a> EncodeIterSlice<'a> {
    fn new(seq: &'a [u8]) -> Self {
        let iter = seq.chunks_exact(2);
        let last = match iter.remainder() {
            &[base] => Some(encode_dna_base_1_u8(base)),
            _ => None,
        };

        Self {
            iter,
            last,
            done: false,
        }
    }
}

impl<'a> Iterator for EncodeIterSlice<'a> {
    type Item = u8;

    #[inline]
    fn next(&mut self) -> Option<u8> {
        if self.done {
            return None;
        }

        match self.iter.next() {
            None => {
                self.done = true;
                self.last
            }
            Some(bases) => {
                if let &[b1, b2] = bases {
                    Some(encode_dna_pair_u8(&[b1, b2]))
                } else {
                    unreachable!();
                }
            }
        }
    }
}

pub struct EncodeIter<I>
where
    I: Iterator<Item = u8> + ExactSizeIterator,
{
    iter: I,
}

impl<I> EncodeIter<I>
where
    I: Iterator<Item = u8> + ExactSizeIterator,
{
    fn new(iter: I) -> Self {
        Self { iter }
    }
}

impl<I> Iterator for EncodeIter<I>
where
    I: Iterator<Item = u8> + ExactSizeIterator,
{
    type Item = u8;

    #[inline]
    fn next(&mut self) -> Option<u8> {
        let first = self.iter.next()?;
        match self.iter.next() {
            None => Some(encode_dna_base_1_u8(first)),
            Some(second) => {
                Some(encode_dna_base_1_u8(first) | encode_dna_base_2_u8(second))
            }
        }
    }
}

pub struct DecodeIter<'a> {
    encoded: &'a [u8],
    left: usize,
    right: usize,
    done: bool,
    reverse: bool,
}

impl<'a> DecodeIter<'a> {
    pub(super) fn new(
        encoded: &'a [u8],
        offset: usize,
        length: usize,
        reverse: bool,
    ) -> Self {
        assert!(offset + length <= encoded.len() * 2);

        let left = offset;
        let right = offset + length - 1;

        Self {
            encoded,
            left,
            right,
            done: false,
            reverse,
        }
    }
}

impl<'a> Iterator for DecodeIter<'a> {
    type Item = u8;

    #[inline]
    fn next(&mut self) -> Option<u8> {
        if self.done {
            return None;
        }

        if !self.reverse {
            let slice_index = self.left / 2;

            let decoded =
                DNA_PAIR_DECODING_TABLE[self.encoded[slice_index] as usize];

            let item = if self.left % 2 == 0 {
                decoded[0]
            } else {
                decoded[1]
            };

            self.left += 1;
            if self.left > self.right {
                self.done = true;
            }

            Some(item)
        } else {
            let slice_index = self.right / 2;

            let decoded = DNA_PAIR_COMP_DECODING_TABLE
                [self.encoded[slice_index] as usize];

            let item = if self.right % 2 == 0 {
                decoded[0]
            } else {
                decoded[1]
            };

            self.right -= 1;
            if self.left > self.right {
                self.done = true;
            }

            Some(item)
        }
    }
}

pub fn encode_sequence(seq: &[u8]) -> Vec<u8> {
    let odd_len = seq.len() % 2 != 0;
    let res_len = (seq.len() / 2) + seq.len() % 2;

    let mut res = Vec::with_capacity(res_len);

    let chunks = seq.chunks_exact(2);

    let last = match chunks.remainder() {
        &[b] => encode_dna_base_1_u8(b) | 0xF,
        _ => 0,
    };

    for chunk in chunks {
        if let &[b1, b2] = chunk {
            res.push(encode_dna_pair_u8(&[b1, b2]));
        }
    }

    if odd_len {
        res.push(last);
    }

    res
}

pub fn decode_sequence(seq: &[u8], len: usize) -> Vec<u8> {
    let mut res = Vec::with_capacity(len);
    let mut remaining = len;

    for [b1, b2] in seq.iter().map(|&val| DNA_PAIR_DECODING_TABLE[val as usize])
    {
        if remaining < 2 {
            res.push(b1);
            break;
        } else {
            res.push(b1);
            res.push(b2);
            remaining -= 2;
        }
    }

    res
}

#[cfg(test)]
mod tests {
    use super::*;

    fn print_3_bits_vec(slice: &[u8], newline: bool) {
        for (ix, byte) in slice.iter().enumerate() {
            if ix != 0 {
                print!("  ");
            }
            let b1 = byte >> 4;
            let b2 = byte & 0b111;
            print!("{:03b} {:03b}", b1, b2);
        }
        if newline {
            println!();
        }
    }

    #[test]
    fn new_sequence_encoding() {
        use bstr::{ByteSlice, B};

        let bases = vec![b'A', b'C', b'G', b'T', b'N'];

        let seqs_0 = vec![b"A", b"c", b"g", b"T", b"N", b"Q"];

        let seqs_1 = {
            let mut seqs = Vec::new();
            for &b_1 in bases.iter() {
                for &b_2 in bases.iter() {
                    seqs.push([b_1, b_2]);
                }
            }
            seqs
        };

        let seqs_2 = vec![
            B("GTCA"),
            B("AAGTGCTAGT"),
            B("ATA"),
            B("AGTA"),
            B("GTCCA"),
            B("GGGT"),
            B("AACT"),
            B("AACAT"),
            B("AGCC"),
        ];

        let encoded_bases = seqs_0
            .iter()
            .map(|&seq| encode_sequence(seq))
            .collect::<Vec<_>>();

        assert_eq!(
            encoded_bases,
            [[0x0F], [0x1F], [0x2F], [0x3F], [0x4F], [0x4F]]
        );

        println!("---------------");

        for seq in seqs_1 {
            // let encode_iter = EncodeIterSlice::new(&seq);
            let encode_iter = EncodeIter {
                iter: seq.iter().copied(),
            };
            let encoded = encode_iter.collect::<Vec<_>>();

            // let encoded = encode_sequence(&seq);
            print!("{}\t{:?}\t", seq.as_bstr(), encoded);
            print_3_bits_vec(&encoded, false);

            // let decoded = decode_sequence(&encoded, seq.len());
            let decoded = DecodeIter::new(&encoded, 0, seq.len(), false)
                .collect::<Vec<_>>();
            println!("  \t{}", decoded.as_bstr());

            assert_eq!(decoded, seq);
        }

        println!("---------------");

        for seq in seqs_2 {
            let encode_iter = EncodeIter {
                iter: seq.iter().copied(),
            };
            let encoded = encode_iter.collect::<Vec<_>>();
            print!("{}\t{:?}\t", seq.as_bstr(), encoded);
            print_3_bits_vec(&encoded, false);

            let decoded = DecodeIter::new(&encoded, 0, seq.len(), false)
                .collect::<Vec<_>>();
            println!("  \t{}", decoded.as_bstr());

            assert_eq!(decoded, seq);
        }
    }

    fn decode_seq(
        encoded: &[u8],
        offset: usize,
        len: usize,
        rev: bool,
    ) -> Vec<u8> {
        DecodeIter::new(&encoded, offset, len, rev).collect::<Vec<_>>()
    }

    #[test]
    fn encoded_sequence_vec() {
        use bstr::{ByteSlice, B};

        let mut encoded_seqs = EncodedSequence::default();

        let seqs = vec![
            B("GTCA"),
            B("AAGTGCTAGT"),
            B("ATA"),
            B("AGTA"),
            B("GTCCA"),
            B("GGGT"),
            B("AACT"),
            B("AACAT"),
            B("AGCC"),
        ];

        let c = encoded_seqs.append_base(b'C');
        let a = encoded_seqs.append_base(b'a');
        let n = encoded_seqs.append_base(b'Q');

        println!("c {}", c);
        println!("a {}", a);
        println!("n {}", n);

        println!("Vector");
        for &val in encoded_seqs.vec.iter() {
            print!("  {:2X}", val);
        }
        println!();

        assert_eq!(&encoded_seqs.vec, &[0x10, 0x4F]);

        let s0 = encoded_seqs.append_seq(&seqs[0]);
        println!("s0 {}", s0);

        println!("Vector");
        for &val in encoded_seqs.vec.iter() {
            print!("  {:2X}", val);
        }
        println!();

        let decoded_s0 =
            decode_seq(&encoded_seqs.vec, s0, seqs[0].len(), false);
        println!("s0 - {}", decoded_s0.as_bstr());

        assert_eq!(&decoded_s0, b"GTCA");

        let s1 = encoded_seqs.append_seq(&seqs[1]);
        println!("s1 {}", s1);

        println!("Vector");
        for &val in encoded_seqs.vec.iter() {
            print!("  {:2X}", val);
        }
        println!();

        let decoded_s1 =
            decode_seq(&encoded_seqs.vec, s1, seqs[1].len(), false);
        println!("s1 - {}", decoded_s1.as_bstr());

        assert_eq!(&decoded_s1, b"AAGTGCTAGT");

        encoded_seqs.rewrite_section(s1 + 2, b"CAC");

        let decoded_s1 =
            decode_seq(&encoded_seqs.vec, s1, seqs[1].len(), false);
        println!("s1 - {}", decoded_s1.as_bstr());

        assert_eq!(&decoded_s1, b"AACACCTAGT");

        println!(" -- reverse complement -- ");

        let decoded_s0 = decode_seq(&encoded_seqs.vec, s0, seqs[0].len(), true);
        println!("s0 - {} - {}", decoded_s0.len(), decoded_s0.as_bstr());

        assert_eq!(&decoded_s0, b"TGAC");

        let decoded_s1 = decode_seq(&encoded_seqs.vec, s1, seqs[1].len(), true);

        println!("s1 - {} - {}", decoded_s1.len(), decoded_s1.as_bstr());
        assert_eq!(&decoded_s1, b"ACTAGGTGTT");
    }
}