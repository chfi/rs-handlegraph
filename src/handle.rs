use gfa::gfa::Orientation;
use std::cmp::Ordering;
use std::ops::Add;

/// Newtype that represents a node in the graph, no matter the
/// graph implementation
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct NodeId(pub u64);

crate::impl_space_usage_stack_newtype!(NodeId);

impl NodeId {
    #[inline]
    pub fn is_zero(&self) -> bool {
        self.0 == 0
    }
}

impl std::fmt::Display for NodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<u64> for NodeId {
    #[inline]
    fn from(num: u64) -> Self {
        NodeId(num)
    }
}

impl From<usize> for NodeId {
    #[inline]
    fn from(num: usize) -> Self {
        NodeId(num as u64)
    }
}

impl From<NodeId> for u64 {
    #[inline]
    fn from(id: NodeId) -> Self {
        id.0
    }
}

impl From<NodeId> for usize {
    #[inline]
    fn from(id: NodeId) -> Self {
        id.0 as usize
    }
}

impl From<i32> for NodeId {
    #[inline]
    fn from(num: i32) -> Self {
        NodeId(num as u64)
    }
}

impl Add<u64> for NodeId {
    type Output = Self;

    #[inline]
    fn add(self, other: u64) -> Self {
        NodeId(self.0 + other)
    }
}

/// A Handle is a node ID with an orientation, packed as a single u64
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Hash, Eq, Ord)]
#[repr(transparent)]
pub struct Handle(pub u64);

/// Returns the forward-oriented `Handle` for a `NodeId`
impl From<NodeId> for Handle {
    #[inline]
    fn from(id: NodeId) -> Handle {
        Handle(id.0 << 1)
    }
}

/// Unpacks the `NodeId` from a `Handle`
impl From<Handle> for NodeId {
    #[inline]
    fn from(h: Handle) -> NodeId {
        h.id()
    }
}

/// A `Handle` can be packed for use in a packed collection -- it is
/// already a packed `u64`.
impl crate::packed::PackedElement for Handle {
    #[inline]
    fn unpack(v: u64) -> Self {
        Handle(v)
    }

    #[inline]
    fn pack(self) -> u64 {
        self.0
    }
}

impl Handle {
    #[inline]
    pub fn as_integer(self) -> u64 {
        self.0
    }

    #[inline]
    pub const fn from_integer(i: u64) -> Self {
        Handle(i)
    }

    #[inline]
    pub fn unpack_number(self) -> u64 {
        self.as_integer() >> 1
    }

    #[inline]
    pub fn unpack_bit(self) -> bool {
        self.as_integer() & 1 != 0
    }

    #[inline]
    pub fn new<T: Into<NodeId>>(id: T, orient: Orientation) -> Handle {
        let id: NodeId = id.into();
        let uint: u64 = id.into();
        let is_reverse = orient != Orientation::Forward;
        if uint < (0x1 << 63) {
            Handle::from_integer((uint << 1) | is_reverse as u64)
        } else {
            panic!(
                "Tried to create a handle with a node ID that filled 64 bits"
            )
        }
    }

    #[inline]
    pub fn pack<T: Into<NodeId>>(id: T, is_reverse: bool) -> Handle {
        let id: NodeId = id.into();
        let uint: u64 = id.into();
        if uint < (0x1 << 63) {
            Handle::from_integer((uint << 1) | is_reverse as u64)
        } else {
            panic!(
                "Tried to create a handle with a node ID that filled 64 bits"
            )
        }
    }

    #[inline]
    pub fn id(self) -> NodeId {
        NodeId(self.unpack_number())
    }

    #[inline]
    pub fn is_reverse(&self) -> bool {
        self.unpack_bit()
    }

    #[inline]
    pub fn flip(self) -> Self {
        Handle(self.as_integer() ^ 1)
    }

    #[inline]
    pub fn forward(self) -> Self {
        if self.is_reverse() {
            self.flip()
        } else {
            self
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Hash, Eq, Ord)]
pub struct Edge(pub Handle, pub Handle);

impl Edge {
    /// Construct an edge, taking the orientation of the handles into account
    #[inline]
    pub fn edge_handle(left: Handle, right: Handle) -> Edge {
        let flipped_right = right.flip();
        let flipped_left = left.flip();

        match left.cmp(&flipped_right) {
            Ordering::Greater => Edge(flipped_right, flipped_left),
            Ordering::Equal => {
                if right > flipped_left {
                    Edge(flipped_right, flipped_left)
                } else {
                    Edge(left, right)
                }
            }
            Ordering::Less => Edge(left, right),
        }
    }
}

/// Enum for handle orientation
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Direction {
    Left,
    Right,
}

#[cfg(test)]
mod tests {
    use super::*;

    // Handle::pack is an isomorphism; Handle <=> (u63, bool)
    #[test]
    fn handle_is_isomorphism() {
        let u: u64 = 597283742;
        let h = Handle::pack(NodeId(u), true);
        assert_eq!(h.unpack_number(), u);
        assert_eq!(h.unpack_bit(), true);
    }

    // Handle::pack should panic when the provided NodeId is invalid
    // (i.e. uses the 64th bit
    #[test]
    #[should_panic]
    fn handle_pack_panic() {
        Handle::pack(NodeId(std::u64::MAX), true);
    }

    #[test]
    fn handle_flip() {
        let u: u64 = 597283742;
        let h1 = Handle::pack(NodeId(u), true);
        let h2 = h1.flip();

        let h3 = Handle::pack(NodeId(u), false);
        println!("{:?}, {}, {}", h1, h1.unpack_bit(), h1.is_reverse());
        println!("{:?}, {}, {}", h2, h2.unpack_bit(), h2.is_reverse());
        println!("{:?}, {}, {}", h3, h3.unpack_bit(), h3.is_reverse());

        assert_eq!(h1.unpack_number(), h2.unpack_number());
        assert_eq!(h1.unpack_bit(), true);
        assert_eq!(h2.unpack_bit(), false);
    }
}
