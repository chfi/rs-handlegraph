/// A collection built from one or more packed vectors
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
/// u64, and unpacked to its original form.
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

impl PackedElement for u8 {
    #[inline]
    fn unpack(v: u64) -> u8 {
        use std::convert::TryFrom;
        if let Ok(u) = u8::try_from(v) {
            u
        } else {
            std::u8::MAX
        }
    }

    #[inline]
    fn pack(self) -> u64 {
        u64::from(self)
    }
}

/// A `Viewable` is a `PackedCollection` that we can get a "view" into
/// a single element of. This lets us get a "reference" to a specific
/// element in a packed collection.
///
/// The view converts element values from their packed
/// representation automatically, using the `PackedCollection` trait.
pub trait Viewable: PackedCollection + Sized {
    /// Get a `ViewRef` for at the provided index on this collection.
    fn view<T>(&self, index: usize) -> ViewRef<'_, Self, T>
    where
        T: PackedElement;

    /// Get the element at `index` and unpack it.
    #[inline]
    fn get_unpack<T>(&self, index: usize) -> T
    where
        T: PackedElement,
    {
        T::unpack(self.get(index))
    }
}

impl<V> Viewable for V
where
    V: PackedCollection + Sized,
{
    #[inline]
    fn view<T>(&self, index: usize) -> ViewRef<'_, Self, T>
    where
        T: PackedElement,
    {
        ViewRef::new(self, index)
    }
}

/// A `MutViewable` lets us get a "mutable view" into a single
/// element. This gives us what is essentially a mutable reference to
/// a single element in a packed collection, despite packed
/// collections not being indexable in that way.
///
/// The mutable view converts element values to/from their packed
/// representations automatically, using the `PackedCollection` trait.
pub trait MutViewable: PackedCollection + Sized {
    /// Get a `ViewMut` for at the provided index on this collection.
    fn view_mut<T>(&mut self, index: usize) -> ViewMut<'_, Self, T>
    where
        T: PackedElement;

    /// Set the element at `index` to the packed representation of `value`.
    #[inline]
    fn set_pack<T>(&mut self, index: usize, value: T)
    where
        T: PackedElement,
    {
        self.set(index, value.pack())
    }
}

impl<V> MutViewable for V
where
    V: PackedCollection + Sized,
{
    #[inline]
    fn view_mut<T>(&mut self, index: usize) -> ViewMut<'_, Self, T>
    where
        T: PackedElement,
    {
        ViewMut::new(self, index)
    }
}

/// A "view" into an element at single index of any `PackedCollection`,
/// unpacked from `u64` into `T`
#[derive(Debug, Clone, Copy)]
pub struct ViewRef<'a, V, T>
where
    V: PackedCollection + Sized,
    T: PackedElement,
{
    _collection: &'a V,
    _index: usize,
    value: T,
}

impl<'a, V, T> ViewRef<'a, V, T>
where
    V: PackedCollection + Sized,
    T: PackedElement,
{
    /// Build a new view into the provided collection, at the given index.
    fn new(collection: &'a V, index: usize) -> Self {
        let value = T::unpack(collection.get(index));
        Self {
            _collection: collection,
            _index: index,
            value,
        }
    }

    /// Get the value at the index. Since `ViewRef` has a shared
    /// reference to the packed collection, the value cannot change,
    /// so we just use the cached value.
    pub fn get(&self) -> T {
        self.value
    }
}

/// A mutable "view" into an element at single index of any
/// `PackedCollection`, unpacked from `u64` into `T`
#[derive(Debug)]
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
    /// Build a new mutable view into the provided collection, at the
    /// given index.
    fn new(collection: &'a mut V, index: usize) -> Self {
        let value = T::unpack(collection.get(index));
        Self {
            collection,
            index,
            value,
        }
    }

    /// Get the value at the index. Since `ViewMut` has a mutable
    /// reference to the packed collection, the value can only be
    /// changed by calling `set()` on this `ViewMut`, so we just use
    /// the cached value.
    pub fn get(&self) -> T {
        self.value
    }

    /// Update the value at the index. Also updates the cache in this
    /// `ViewMut`.
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

#[allow(unused_macros)]
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
