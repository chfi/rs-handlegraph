pub mod dna {

    include!(concat!(env!("OUT_DIR"), "/comp_table.rs"));

    /// A lookup-table for the DNA complements is generated at compile
    /// time by the build.rs script in the project root, and placed in
    /// the compilation out-dir under the name "comp_table.rs".
    #[inline]
    pub const fn comp_base(base: u8) -> u8 {
        DNA_COMP_TABLE[base as usize]
    }

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

    #[inline]
    pub fn rev_comp_iter<I, B>(seq: I) -> impl Iterator<Item = u8>
    where
        B: std::borrow::Borrow<u8>,
        I: IntoIterator<Item = B>,
        I::IntoIter: DoubleEndedIterator,
    {
        seq.into_iter().rev().map(|b| comp_base(*b.borrow()))
    }

    #[cfg(tests)]
    mod tests {
        use super::*;

        use bio::alphabets::dna;
        use quickcheck::{Arbitrary, Gen};

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
                let n = u8::arbitrary() % 8;
                Base::from_num(n)
            }
        }

        fn is_comp_isomorphic(b: Base) -> bool {
            let base = b.0;
            comp_base(comp_base(base)) == base
        }

        fn is_rev_comp_isomorphic(seq: Vec<Base>) -> bool {
            rev_comp(rev_comp(seq)) == seq
        }

        #[test]
        fn comp_isomorphic() {
            QuickCheck::new()
                .tests(10000)
                .quickcheck(is_comp_isomorphic as fn(Base) -> bool);
        }

        #[test]
        fn rev_comp_isomorphic() {
            QuickCheck::new()
                .tests(10000)
                .quickcheck(is_rev_comp_isomorphic as fn(Vec<Base>) -> bool);
        }
    }
}
