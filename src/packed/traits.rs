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

pub trait Viewable<T>: PackedCollection + Sized
where
    T: From<u64> + Copy,
{
    fn view<'a>(&'a self, index: usize) -> ViewRef<'a, Self, T>;
}

impl<T, V> Viewable<T> for V
where
    T: From<u64> + Copy,
    V: PackedCollection + Sized,
{
    fn view<'a>(&'a self, index: usize) -> ViewRef<'a, Self, T> {
        ViewRef::new(self, index)
    }
}

pub trait MutViewable<T>: PackedCollection + Sized
where
    T: From<u64> + Into<u64> + Copy,
{
    fn view_mut<'a>(&'a mut self, index: usize) -> ViewMut<'a, Self, T>;
}

impl<T, V> MutViewable<T> for V
where
    T: From<u64> + Into<u64> + Copy,
    V: PackedCollection + Sized,
{
    fn view_mut<'a>(&'a mut self, index: usize) -> ViewMut<'a, Self, T> {
        ViewMut::new(self, index)
    }
}

pub struct ViewRef<'a, V, T>
where
    V: PackedCollection + Sized,
    T: From<u64> + Sized + Copy,
{
    collection: &'a V,
    index: usize,
    value: T,
}

impl<'a, V, T> ViewRef<'a, V, T>
where
    V: PackedCollection + Sized,
    T: From<u64> + Sized + Copy,
{
    fn new(collection: &'a V, index: usize) -> Self {
        let value = collection.get(index).into();
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
    T: From<u64> + Into<u64> + Copy,
{
    collection: &'a mut V,
    index: usize,
    value: T,
}

impl<'a, V, T> ViewMut<'a, V, T>
where
    V: PackedCollection,
    T: From<u64> + Into<u64> + Copy,
{
    pub fn new(collection: &'a mut V, index: usize) -> Self {
        let value = collection.get(index).into();
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
        self.collection.set(self.index, value.into())
    }
}
