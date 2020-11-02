use succinct::{IntVec, IntVecMut, IntVector};

pub mod traits;

pub mod deque;
pub mod paged;
pub mod robust;
pub mod vector;

pub use self::{
    deque::PackedDeque, paged::PagedIntVec, robust::RobustPagedIntVec,
    traits::*, vector::PackedIntVec,
};

use quickcheck::{Arbitrary, Gen};

impl Arbitrary for PackedIntVec {
    fn arbitrary<G: Gen>(g: &mut G) -> PackedIntVec {
        let mut intvec = PackedIntVec::new();
        let u64_vec: Vec<u64> = Vec::arbitrary(g);

        for v in u64_vec {
            intvec.append(v);
        }
        intvec
    }
}

impl Arbitrary for PagedIntVec {
    fn arbitrary<G: Gen>(g: &mut G) -> PagedIntVec {
        let mut paged = PagedIntVec::new(64);
        let u64_vec: Vec<u64> = Vec::arbitrary(g);

        for v in u64_vec {
            paged.append(v);
        }
        paged
    }
}

impl Arbitrary for RobustPagedIntVec {
    fn arbitrary<G: Gen>(g: &mut G) -> RobustPagedIntVec {
        let only_first = bool::arbitrary(g);

        let page_pow = u32::arbitrary(g) % 4;
        let page_size = 16 << page_pow;

        assert!(page_size % 2 == 0 && page_size >= 16 && page_size <= 256);
        let mut paged = RobustPagedIntVec::new(page_size);
        let mut values: Vec<u64> = Vec::arbitrary(g);

        if !only_first {
            while values.len() < page_size {
                let v = u64::arbitrary(g);
                values.push(v);
            }
        }

        values.into_iter().for_each(|v| paged.append(v));

        paged
    }
}

impl Arbitrary for PackedDeque {
    fn arbitrary<G: Gen>(g: &mut G) -> PackedDeque {
        let front: Vec<u64> = Vec::arbitrary(g);
        let back: Vec<u64> = Vec::arbitrary(g);
        let front_first = bool::arbitrary(g);

        let mut deque = PackedDeque::new();

        if front_first {
            front.into_iter().for_each(|v| deque.push_front(v));
            back.into_iter().for_each(|v| deque.push_back(v));
        } else {
            back.into_iter().for_each(|v| deque.push_back(v));
            front.into_iter().for_each(|v| deque.push_front(v));
        }
        deque
    }
}

#[cfg(test)]
mod tests {

    use quickcheck::quickcheck;

    use super::*;

    #[test]
    fn test_intvec_append() {
        let mut intvec = PackedIntVec::new();

        assert_eq!(intvec.len(), 0);
        assert_eq!(intvec.width(), 1);

        intvec.append(1);
        assert_eq!(intvec.len(), 1);
        assert_eq!(intvec.width(), 1);

        intvec.append(2);
        assert_eq!(intvec.len(), 2);
        assert_eq!(intvec.width(), 2);

        intvec.append(10);
        assert_eq!(intvec.len(), 3);
        assert_eq!(intvec.width(), 4);

        intvec.append(120);
        assert_eq!(intvec.len(), 4);
        assert_eq!(intvec.width(), 7);

        intvec.append(3);
        assert_eq!(intvec.len(), 5);
        assert_eq!(intvec.width(), 7);

        let vector = vec![1, 2, 10, 120, 3];
        assert!(intvec.iter().eq(vector.into_iter()));
    }

    quickcheck! {
        fn prop_intvec_append(intvec: PackedIntVec, value: u64) -> bool {
            let mut intvec = intvec;

            let filled_before = intvec.len();
            let width_before = intvec.width();

            intvec.append(value);

            let filled_correct = intvec.len() == filled_before + 1;
            let last_val = intvec.get(intvec.len() - 1);
            let width_after = intvec.width();

            filled_correct && last_val == value && width_after >= width_before
        }
    }

    quickcheck! {
        fn prop_intvec_pop(intvec: PackedIntVec) -> bool {
            let mut intvec = intvec;

            let filled_before = intvec.len();
            let width_before = intvec.width();

            intvec.pop();

            let filled_after = intvec.len();
            let width_after = intvec.width();

            let filled_correct = if filled_before > 0 {
                filled_after == filled_before - 1
            } else {
                filled_after == filled_before
            };

            filled_correct &&
                width_before == width_after
        }
    }

    quickcheck! {
        fn prop_intvec_get(vector: Vec<u64>) -> bool {
            let mut intvec = PackedIntVec::new();
            for &x in vector.iter() {
                intvec.append(x);
            }

            for ix in 0..vector.len() {
                let a = vector[ix];
                let b = intvec.get(ix);
                if a != b {
                    return false;
                }
            }

            true
        }
    }

    quickcheck! {
        fn prop_intvec_iter(vector: Vec<u64>) -> bool {
            let mut intvec = PackedIntVec::new();
            for &x in vector.iter() {
                intvec.append(x);
            }

            vector.into_iter().eq(intvec.iter())
        }
    }

    quickcheck! {
        fn prop_paged_append(paged: PagedIntVec, value: u64) -> bool {
            let mut paged = paged;

            let entries_before = paged.len();

            paged.append(value);

            let entries_correct = paged.len() == entries_before + 1;
            let last_val_correct = paged.get(paged.len() - 1) == value;

            entries_correct && last_val_correct
        }
    }

    quickcheck! {
        fn prop_paged_set(paged: PagedIntVec, ix: usize, value: u64) -> bool {
            let mut paged = paged;
            if paged.len() == 0 {
                return true;
            }
            let ix = ix % paged.len();

            let len_before = paged.len();
            let pages_before = paged.pages.len();
            paged.set(ix, value);

            let set_correct = paged.get(ix) == value;
            let len_correct = paged.len() == len_before;
            let pages_correct = paged.pages.len() == pages_before;

            set_correct && len_correct && pages_correct
        }
    }

    quickcheck! {
        fn prop_paged_pop(paged: PagedIntVec) -> bool {
            let mut paged = paged;

            let len_before = paged.len();

            paged.pop();

            let len_correct = if len_before == 0 {
                paged.len() == 0
            } else {
                paged.len() == len_before - 1
            };

            len_correct
        }
    }

    quickcheck! {
        fn prop_robust_append(paged: RobustPagedIntVec, value: u64) -> bool {
            let mut paged = paged;

            let entries_before = paged.len();

            paged.append(value);

            let entries_correct = paged.len() == entries_before + 1;
            let last_val_correct = paged.get(paged.len() - 1) == value;

            entries_correct && last_val_correct
        }
    }

    quickcheck! {
        fn prop_robust_set(paged: RobustPagedIntVec, ix: usize, value: u64) -> bool {
            let mut paged = paged;
            if paged.len() == 0 {
                return true;
            }
            let ix = ix % paged.len();

            let len_before = paged.len();
            let first_len_before = paged.first_page.len();
            let pages_before = paged.other_pages.pages.len();
            paged.set(ix, value);

            let set_correct = paged.get(ix) == value;
            let len_correct = paged.len() == len_before;
            let first_len_correct = paged.first_page.len() == first_len_before;
            let pages_correct = paged.other_pages.pages.len() == pages_before;

            set_correct && len_correct && first_len_correct && pages_correct
        }
    }

    quickcheck! {
        fn prop_robust_pop(paged: RobustPagedIntVec) -> bool {
            let mut paged = paged;

            let len_before = paged.len();

            paged.pop();

            if len_before == 0 {
                paged.len() == 0
            } else {
                paged.len() == len_before - 1
            }
        }
    }

    quickcheck! {
        fn prop_deque_push_front(deque: PackedDeque, val: u64) -> bool {
            let mut deque = deque;
            let len = deque.len();

            deque.push_front(val);

            deque.len() == len + 1 &&
            deque.get(0) == val
        }
    }

    quickcheck! {
        fn prop_deque_push_back(deque: PackedDeque, val: u64) -> bool {
            let mut deque = deque;
            let len = deque.len();

            deque.push_back(val);

            deque.len() == len + 1 &&
                deque.get(deque.len() - 1) == val
        }
    }

    quickcheck! {
        fn prop_deque_pop_back(deque: PackedDeque) -> bool {
            let mut deque = deque;
            let len = deque.len();

            if len <= 1 {
                deque.pop_back();
                deque.len() == 0
            } else {
                let second_last = deque.get(deque.len() - 2);
                deque.pop_back();
                deque.len() == len - 1 &&
                    deque.get(deque.len() - 1) == second_last
            }
        }
    }

    quickcheck! {
        fn prop_deque_pop_front(deque: PackedDeque) -> bool {
            let mut deque = deque;
            let len = deque.len();

            if len <= 1 {
                deque.pop_front();
                deque.len() == 0
            } else {
                let second = deque.get(1);
                deque.pop_front();

                deque.len() == len - 1 &&
                    deque.get(0) == second
            }
        }
    }
}
