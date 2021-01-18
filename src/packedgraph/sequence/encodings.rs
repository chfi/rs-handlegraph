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

pub(crate) const DNA_BASE_3BIT_ENCODING: [u8; 256] = {
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

const SHIFT_OFFSET_3BITS_U8: [u8; 8] = [5, 2, 7, 4, 1, 6, 3, 0];

const APPEND_MASK_3BITS_U8: [u8; 8] = [
    0b0001_1111,
    0b1110_0011,
    0b1111_1100,
    0b1000_1111,
    0b1111_0001,
    0b1111_1110,
    0b1100_0111,
    0b1111_1000,
];

const INDEXING_OFFSET_3BITS_U8: [u8; 256] = {
    let mut table: [u8; 256] = [0; 256];

    let mut i = 0;
    while i < 256 {
        let offset = match (i as u8) % 8 {
            0 | 1 | 2 => 0,
            3 | 4 | 5 => 1,
            _ => 2,
        };
        table[i] = offset;
        i += 1
    }

    table
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SequenceEncoding {
    BaseHalfByte,
    Base3Bits,
}

#[derive(Debug, Clone)]
pub struct EncodedSequence {
    pub(crate) vec: Vec<u8>,
    len: usize,
    encoding: SequenceEncoding,
}

impl EncodedSequence {
    pub fn new_half_byte() -> Self {
        Self {
            vec: Vec::new(),
            len: 0,
            encoding: SequenceEncoding::BaseHalfByte,
        }
    }

    pub fn new_3bits() -> Self {
        Self {
            vec: Vec::new(),
            len: 0,
            encoding: SequenceEncoding::Base3Bits,
        }
    }
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
    pub fn get_base(&self, index: usize, comp: bool) -> Option<u8> {
        assert!(index < self.len);

        match self.encoding {
            SequenceEncoding::BaseHalfByte => {
                let slice_index = index / 2;
                let value = &self.vec[slice_index];
                let mut enc_base = if index % 2 == 0 {
                    (value >> 4) & 0x0F
                } else {
                    value & 0x0F
                };

                if comp {
                    enc_base = PACKED_BASE_COMPLEMENT[enc_base as usize];
                }

                Some(PACKED_BASE_DECODING[enc_base as usize])
            }
            SequenceEncoding::Base3Bits => {
                let index_offset =
                    INDEXING_OFFSET_3BITS_U8[(index as u8) as usize];
                let index_base = 3 * (index >> 3);
                let byte_index = index_base + index_offset as usize;
                let shift = SHIFT_OFFSET_3BITS_U8[((index % 8) as u8) as usize];
                let bytes = match index % 8 {
                    x if x == 2 || x == 5 => {
                        let pair =
                            [self.vec[byte_index], self.vec[byte_index + 1]];
                        u16::from_le_bytes(pair)
                    }
                    _ => self.vec[byte_index] as u16,
                };

                let mut enc_base = ((bytes >> shift) & 0x07) as u8;

                if comp {
                    enc_base = PACKED_BASE_COMPLEMENT[enc_base as usize]
                }

                Some(PACKED_BASE_DECODING[enc_base as usize])
            }
        }
    }

    #[inline]
    pub fn append_base(&mut self, base: u8) -> usize {
        match self.encoding {
            SequenceEncoding::BaseHalfByte => {
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
            SequenceEncoding::Base3Bits => {
                let new_index = self.len;
                let enc_base = DNA_BASE_3BIT_ENCODING[base as usize];
                let len_mod_8 = self.len % 8;

                let mask = APPEND_MASK_3BITS_U8[(len_mod_8 as u8) as usize];
                let shift = SHIFT_OFFSET_3BITS_U8[(len_mod_8 as u8) as usize];
                match len_mod_8 {
                    0 => {
                        self.vec.push((enc_base << shift) | mask);
                    }
                    1 | 3 | 4 | 6 | 7 => {
                        if let Some(last) = self.vec.last_mut() {
                            *last &= (enc_base << shift) | mask;
                        }
                    }
                    x => {
                        let (last_shift, new_mask) =
                            if x == 2 { (1, 0x7F) } else { (2, 0x3F) };
                        if let Some(last) = self.vec.last_mut() {
                            *last &= mask | (enc_base >> last_shift);
                        }
                        self.vec.push((enc_base << shift) | new_mask);
                    }
                }

                self.len += 1;
                new_index
            }
        }
    }

    #[inline]
    pub fn append_seq(&mut self, seq: &[u8]) -> usize {
        assert!(!seq.is_empty());
        let new_index = self.len;

        match self.encoding {
            SequenceEncoding::BaseHalfByte => {
                if self.len % 2 == 0 {
                    let iter = EncodeIterSlice::new(seq);
                    self.len += seq.len();
                    self.vec.extend(iter);
                    new_index
                } else {
                    if seq.len() == 1 {
                        self.append_base(seq[0]);
                        new_index
                    } else {
                        self.append_base(seq[0]);
                        let iter = EncodeIterSlice::new(&seq[1..]);
                        self.len += seq.len() - 1;
                        self.vec.extend(iter);
                        new_index
                    }
                }
            }
            SequenceEncoding::Base3Bits => {
                if self.vec.capacity() < seq.len() {
                    self.vec.reserve(seq.len() - self.vec.capacity());
                }
                let diff = seq.len().min(8 - (self.len % 8));
                for i in 0..diff {
                    self.append_base(seq[i]);
                }
                if seq.len() <= diff {
                    return new_index;
                }
                let chunks = seq[diff..].chunks_exact(8);

                let rest = chunks.remainder().to_owned();

                for chunk in chunks {
                    let byte_0 = DNA_BASE_3BIT_ENCODING[chunk[0] as usize] << 5
                        | DNA_BASE_3BIT_ENCODING[chunk[1] as usize] << 2
                        | DNA_BASE_3BIT_ENCODING[chunk[2] as usize] >> 1;

                    let byte_1 = DNA_BASE_3BIT_ENCODING[chunk[2] as usize] << 7
                        | DNA_BASE_3BIT_ENCODING[chunk[3] as usize] << 4
                        | DNA_BASE_3BIT_ENCODING[chunk[4] as usize] << 1
                        | DNA_BASE_3BIT_ENCODING[chunk[5] as usize] >> 2;

                    let byte_2 = DNA_BASE_3BIT_ENCODING[chunk[5] as usize] << 6
                        | DNA_BASE_3BIT_ENCODING[chunk[6] as usize] << 3
                        | DNA_BASE_3BIT_ENCODING[chunk[7] as usize];

                    self.vec.push(byte_0);
                    self.vec.push(byte_1);
                    self.vec.push(byte_2);

                    self.len += 8;
                }

                for base in rest {
                    self.append_base(base);
                }

                new_index
            }
        }
    }

    #[inline]
    pub fn write_base(&mut self, index: usize, base: u8) {
        assert!(index < self.len);

        match self.encoding {
            SequenceEncoding::BaseHalfByte => {
                let slice_index = index / 2;
                let value = &mut self.vec[slice_index];
                let enc_base = DNA_BASE_3BIT_ENCODING[base as usize];
                if index % 2 == 0 {
                    *value = (*value & 0x0F) | enc_base << 4;
                } else {
                    *value = (*value & 0xF0) | enc_base;
                }
            }
            SequenceEncoding::Base3Bits => {
                let index_offset =
                    INDEXING_OFFSET_3BITS_U8[(index as u8) as usize];
                let index_base = 3 * (index >> 3);
                let byte_index = index_base + index_offset as usize;

                let enc_base = DNA_BASE_3BIT_ENCODING[base as usize];

                let len_mod_8 = self.len % 8;
                let mask = APPEND_MASK_3BITS_U8[(len_mod_8 as u8) as usize];
                let shift = SHIFT_OFFSET_3BITS_U8[(len_mod_8 as u8) as usize];

                match len_mod_8 {
                    x if x == 2 || x == 5 => {
                        let bytes = &mut self.vec[byte_index..byte_index + 1];

                        bytes[0] &= mask | (enc_base >> shift);

                        let right_len_mod_8 = (len_mod_8 + 1) % 8;

                        let shift = SHIFT_OFFSET_3BITS_U8
                            [(right_len_mod_8 as u8) as usize];

                        let mask = APPEND_MASK_3BITS_U8
                            [(right_len_mod_8 as u8) as usize];

                        bytes[1] &= mask | (enc_base << shift);
                    }
                    _ => {
                        let byte = &mut self.vec[byte_index];
                        *byte &= mask | (enc_base << shift);
                    }
                }
            }
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

    pub fn iter(
        &self,
        offset: usize,
        len: usize,
        reverse: bool,
    ) -> DecodeIter<'_> {
        assert!(offset + len <= self.len);

        let left = offset;
        let right = offset + len - 1;
        DecodeIter {
            encoded: &self.vec,
            left,
            right,
            reverse,
            done: false,
            encoding: self.encoding,
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
    encoding: SequenceEncoding,
}

impl<'a> DecodeIter<'a> {
    pub fn new_half_byte(
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
            encoding: SequenceEncoding::BaseHalfByte,
        }
    }

    pub fn new_3bits(
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
            encoding: SequenceEncoding::Base3Bits,
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

        match self.encoding {
            SequenceEncoding::BaseHalfByte => {
                let index = if self.reverse { self.right } else { self.left };
                let slice_index = index / 2;

                let decoded = if !self.reverse {
                    self.left += 1;
                    DNA_PAIR_DECODING_TABLE[self.encoded[slice_index] as usize]
                } else {
                    self.right -= 1;
                    DNA_PAIR_COMP_DECODING_TABLE
                        [self.encoded[slice_index] as usize]
                };

                let item = decoded[index % 2];

                if self.left > self.right || self.right == 0 {
                    self.done = true;
                }

                Some(item)
            }
            SequenceEncoding::Base3Bits => {
                let index = if !self.reverse {
                    self.left += 1;
                    self.left - 1
                } else {
                    if self.right > 0 {
                        self.right -= 1;
                        self.right + 1
                    } else {
                        self.done = true;
                        0
                    }
                };

                if self.left > self.right {
                    self.done = true;
                }

                let index_offset =
                    INDEXING_OFFSET_3BITS_U8[(index as u8) as usize];
                let index_base = 3 * (index >> 3);
                let byte_index = index_base + index_offset as usize;

                let ix_mod_8 = index % 8;
                let shift = SHIFT_OFFSET_3BITS_U8[(ix_mod_8 as u8) as usize];

                let mut enc_base = match index % 8 {
                    x if x == 2 || x == 5 => {
                        let pair = [
                            self.encoded[byte_index],
                            self.encoded[byte_index + 1],
                        ];
                        let bytes = u16::from_be_bytes(pair);
                        ((bytes >> shift) & 0x07) as u8
                    }
                    _ => {
                        let byte = self.encoded[byte_index];
                        ((byte >> shift) & 0x07) as u8
                    }
                };

                if self.reverse {
                    enc_base = PACKED_BASE_COMPLEMENT[enc_base as usize];
                }

                Some(PACKED_BASE_DECODING[enc_base as usize])
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
            let encode_iter = EncodeIter {
                iter: seq.iter().copied(),
            };
            let encoded = encode_iter.collect::<Vec<_>>();

            print!("{}\t{:?}\t", seq.as_bstr(), encoded);
            print_3_bits_vec(&encoded, false);

            let decoded =
                DecodeIter::new_half_byte(&encoded, 0, seq.len(), false)
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

            let decoded =
                DecodeIter::new_half_byte(&encoded, 0, seq.len(), false)
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
        DecodeIter::new_half_byte(&encoded, offset, len, rev)
            .collect::<Vec<_>>()
    }

    #[test]
    fn bytevec_3bit_encoding() {
        let mut encoded_seqs = EncodedSequence::new_3bits();

        let _c = encoded_seqs.append_base(b'C');
        let _a = encoded_seqs.append_base(b'A');
        let _g = encoded_seqs.append_base(b'G');
        let _t = encoded_seqs.append_base(b'T');
        let _n = encoded_seqs.append_base(b'N');
        let _s0 = encoded_seqs.append_seq(b"AAGTGCTAGTAGTTTAACTNGA");

        assert_eq!(
            &encoded_seqs.vec,
            &[33, 56, 2, 104, 176, 152, 77, 176, 11, 136, 127]
        );

        assert_eq!(encoded_seqs.len(), 27);
    }

    #[test]
    fn bytevec_4bit_encoding() {
        use bstr::{ByteSlice, B};

        let mut encoded_seqs = EncodedSequence::new_half_byte();

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

        assert_eq!(encoded_seqs.len(), 17);
    }
}
