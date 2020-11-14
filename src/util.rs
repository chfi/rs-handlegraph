pub(crate) mod dna {
    #[inline]
    pub(crate) const fn comp_base(base: u8) -> u8 {
        match base {
            b'A' => b'T',
            b'G' => b'C',
            b'C' => b'G',
            b'T' => b'A',
            b'a' => b't',
            b'g' => b'c',
            b'c' => b'g',
            b't' => b'a',
            _ => b'N',
        }
    }
    #[inline]
    pub(crate) fn rev_comp<I>(seq: I) -> impl Iterator<Item = u8>
    where
        I: IntoIterator<Item = u8>,
        I::IntoIter: DoubleEndedIterator,
    {
        seq.into_iter().rev().map(comp_base)
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

        fn comp_same_as_bio(b: Base) -> bool {
            let base = b.0;
            let bio_comp = dna::complement(base);
            let comp = comp_base(base);
            bio_comp == comp
        }

        #[test]
        fn comp_base_vs_bio() {
            QuickCheck::new()
                .tests(10000)
                .quickcheck(comp_same_as_bio as fn(Base) -> bool);
        }

        fn rev_comp_same_as_bio(seq: Vec<Base>) -> bool {
            let bio_rev_comp = dna::revcomp(seq);
            let my_rev_comp = rev_comp(seq).collect::<Vec<_>>();
            bio_rev_comp == my_rev_comp
        }

        #[test]
        fn rev_comp_vs_bio() {
            QuickCheck::new()
                .tests(10000)
                .quickcheck(rev_comp_same_as_bio as fn(Vec<Base>) -> bool);
        }
    }
}
