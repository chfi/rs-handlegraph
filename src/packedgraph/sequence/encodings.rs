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
pub(crate) const fn encode_dna_base(base: u8) -> u64 {
    DNA_3BIT_ENCODING_TABLE[base as usize]
}

#[inline]
pub(crate) const fn encoded_complement(val: u64) -> u64 {
    PACKED_BASE_COMPLEMENT[(val as u8) as usize] as u64
}

#[inline]
pub(crate) const fn decode_dna_base(val: u64) -> u8 {
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

    #[inline]
    fn count(self) -> usize {
        self.length
    }

    #[inline]
    fn last(mut self) -> Option<u8> {
        if self.reverse {
            let base = self.iter.next()?;
            Some(decode_dna_base(encoded_complement(base)))
        } else {
            let base = self.iter.last()?;
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

            let decoded = decode_sequence(&encoded, seq.len());
            println!("  \t{}", decoded.as_bstr());

            assert_eq!(decoded, seq);
        }

        println!("---------------");

        for seq in seqs_2 {
            let encoded = encode_sequence(&seq);
            // print!("{}\t{:?}\t", seq.as_bstr(), encoded);
            // print_3_bits_vec(&encoded, false);

            let decoded = decode_sequence(&encoded, seq.len());
            // println!("  \t{}", decoded.as_bstr());

            assert_eq!(decoded, seq);
        }
    }
}
