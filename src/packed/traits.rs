use succinct::{IntVec, IntVecMut, IntVector};

pub trait PackedCollection {
    fn len(&self) -> usize;

    #[inline]
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn set(&mut self, index: usize, value: u64);

    fn get(&self, index: usize) -> u64;

    fn append(&mut self, value: u64);

    fn pop(&mut self);

    fn clear(&mut self);
}

/// An element that can be packed into a PackedCollection element as a
/// u64, and unpacked again
pub trait PackedElement: Sized + Copy {
    fn unpack(v: u64) -> Self;

    fn pack(self) -> u64;
}

impl PackedElement for bool {
    #[inline]
    fn unpack(v: u64) -> bool {
        v == 1
    }

    #[inline]
    fn pack(self) -> u64 {
        self.into()
    }
}

pub trait Viewable: PackedCollection + Sized {
    fn view<'a, T>(&'a self, index: usize) -> ViewRef<'a, Self, T>
    where
        T: PackedElement;
}

impl<V> Viewable for V
where
    V: PackedCollection + Sized,
{
    fn view<'a, T>(&'a self, index: usize) -> ViewRef<'a, Self, T>
    where
        T: PackedElement,
    {
        ViewRef::new(self, index)
    }
}

pub trait MutViewable: PackedCollection + Sized {
    fn view_mut<'a, T>(&'a mut self, index: usize) -> ViewMut<'a, Self, T>
    where
        T: PackedElement;
}

impl<V> MutViewable for V
where
    V: PackedCollection + Sized,
{
    fn view_mut<'a, T>(&'a mut self, index: usize) -> ViewMut<'a, Self, T>
    where
        T: PackedElement,
    {
        ViewMut::new(self, index)
    }
}

pub struct ViewRef<'a, V, T>
where
    V: PackedCollection + Sized,
    T: PackedElement,
{
    collection: &'a V,
    index: usize,
    value: T,
}

impl<'a, V, T> ViewRef<'a, V, T>
where
    V: PackedCollection + Sized,
    T: PackedElement,
{
    fn new(collection: &'a V, index: usize) -> Self {
        let value = T::unpack(collection.get(index));
        Self {
            collection,
            index,
            value,
        }
    }

    pub fn get(&self) -> T {
        self.value
    }
}

pub struct ViewMut<'a, V, T>
where
    V: PackedCollection,
    T: PackedElement,
{
    collection: &'a mut V,
    index: usize,
    value: T,
}

impl<'a, V, T> ViewMut<'a, V, T>
where
    V: PackedCollection,
    T: PackedElement,
{
    pub fn new(collection: &'a mut V, index: usize) -> Self {
        let value = T::unpack(collection.get(index));
        Self {
            collection,
            index,
            value,
        }
    }

    pub fn get(&self) -> T {
        self.value
    }

    pub fn set(&mut self, value: T) {
        self.value = value;
        self.collection.set(self.index, value.pack())
    }
}

// Can't use a generic implementation for T: From<u64> + Into<u64>
// because "upstream crates may add a new impl of trait
// `std::convert::From<u64>` for type `bool` in future versions"
macro_rules! impl_packed_element_as {
    ($for:ty) => {
        impl PackedElement for $for {
            #[inline]
            fn unpack(v: u64) -> $for {
                v as $for
            }

            #[inline]
            fn pack(self) -> u64 {
                self as u64
            }
        }
    };
}

macro_rules! impl_packed_element_from_into {
    ($for:ty) => {
        impl PackedElement for $for {
            #[inline]
            fn unpack(v: u64) -> $for {
                v.into()
            }

            #[inline]
            fn pack(self) -> u64 {
                self.into()
            }
        }
    };
}

impl_packed_element_as!(usize);
impl_packed_element_as!(u64);
