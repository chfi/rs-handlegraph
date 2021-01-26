#[cfg(test)]
pub mod test;

pub mod validate;

pub mod dna {

    const fn comp_base_impl(base: u8) -> u8 {
        match base {
            b'A' => b'T',
            b'G' => b'C',
            b'C' => b'G',
            b'T' => b'A',
            b'Y' => b'R',
            b'R' => b'Y',
            b'W' => b'W',
            b'S' => b'S',
            b'K' => b'M',
            b'M' => b'K',
            b'D' => b'H',
            b'V' => b'B',
            b'H' => b'D',
            b'B' => b'V',
            _ => b'N',
        }
    }

    // loops can be used in const fns since Rust 1.46, meaning we can
    // build a lookup table at compile time
    const fn comp_base_table() -> [u8; 256] {
        let mut i = 0;
        let mut table: [u8; 256] = [0; 256];
        while i <= 255 {
            let offset = 32 * ((i as u8).is_ascii_lowercase() as u8);
            let comp = comp_base_impl((i as u8) - offset);

            if comp == b'N' {
                table[i] = i as u8;
            } else {
                table[i] = comp + offset;
            }

            i += 1;
        }
        table
    }

    const DNA_COMP_TABLE: [u8; 256] = comp_base_table();

    /// Retrieves the DNA complement for the provided base using a
    /// lookup-table built at compile time using the `const fn`
    /// `comp_base_table()`.
    #[inline]
    pub const fn comp_base(base: u8) -> u8 {
        DNA_COMP_TABLE[base as usize]
    }

    /// Calculates the reverse complement for a sequence provided as a
    /// double-ended iterator. Collects into a `Vec<u8>` for
    /// convenience.
    #[inline]
    pub fn rev_comp<I, B>(seq: I) -> Vec<u8>
    where
        B: std::borrow::Borrow<u8>,
        I: IntoIterator<Item = B>,
        I::IntoIter: DoubleEndedIterator,
    {
        seq.into_iter()
            .rev()
            .map(|b| comp_base(*b.borrow()))
            .collect()
    }

    /// Given a sequence provided as a double-ended iterator over
    /// nucleotides, returns an iterator over the reverse complement
    /// of the sequence.
    #[inline]
    pub fn rev_comp_iter<I, B>(seq: I) -> impl Iterator<Item = u8>
    where
        B: std::borrow::Borrow<u8>,
        I: IntoIterator<Item = B>,
        I::IntoIter: DoubleEndedIterator,
    {
        seq.into_iter().rev().map(|b| comp_base(*b.borrow()))
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        use quickcheck::{Arbitrary, Gen, QuickCheck};

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        struct Base(u8);

        impl Base {
            fn from_num(n: u8) -> Base {
                match n {
                    0 => Base(b'T'),
                    1 => Base(b'C'),
                    2 => Base(b'G'),
                    3 => Base(b'A'),
                    4 => Base(b't'),
                    5 => Base(b'c'),
                    6 => Base(b'g'),
                    7 => Base(b'a'),
                    _ => Base(b'N'),
                }
            }
        }

        impl From<u8> for Base {
            fn from(base: u8) -> Base {
                Base(base)
            }
        }

        impl Into<u8> for Base {
            fn into(self) -> u8 {
                self.0
            }
        }

        impl Arbitrary for Base {
            fn arbitrary<G: Gen>(g: &mut G) -> Base {
                let n = u8::arbitrary(g) % 8;
                Base::from_num(n)
            }
        }

        fn is_comp_isomorphic(b: Base) -> bool {
            let base = b.0;
            comp_base(comp_base(base)) == base
        }

        #[test]
        fn comp_isomorphic_check() {
            for x in 0..10 {
                let i = x as u8;
                let base = Base::from_num(i);
                let comp = comp_base(base.0);
                let back = comp_base(comp);
                println!(
                    "{:2} -> {} -> {} -> {}",
                    i,
                    char::from(base.0),
                    char::from(comp),
                    char::from(back),
                );
            }
        }

        #[test]
        fn comp_isomorphic() {
            QuickCheck::new()
                .tests(10000)
                .quickcheck(is_comp_isomorphic as fn(Base) -> bool);
        }

        fn is_rev_comp_isomorphic(seq: Vec<Base>) -> bool {
            let seq = seq.into_iter().map(|b| b.0).collect::<Vec<_>>();
            rev_comp(rev_comp(seq.clone())) == seq
        }

        #[test]
        fn rev_comp_isomorphic() {
            QuickCheck::new()
                .tests(10000)
                .quickcheck(is_rev_comp_isomorphic as fn(Vec<Base>) -> bool);
        }
    }
}
