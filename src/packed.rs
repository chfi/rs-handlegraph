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
