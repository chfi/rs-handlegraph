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

}
