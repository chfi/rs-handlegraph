use std::ops::Add;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeId(u64);

impl From<u64> for NodeId {
    fn from(num: u64) -> Self {
        NodeId(num)
    }
}

impl Add<u64> for NodeId {
    type Output = Self;

    fn add(self, other: u64) -> Self {
        let NodeId(i) = self;
        NodeId(i + other)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Hash)]
pub struct Handle(u64);

impl Handle {
    pub fn as_integer(self) -> u64 {
        let Handle(i) = self;
        i
    }

    pub fn from_integer(i: u64) -> Self {
        Handle(i)
    }

    pub fn unpack_number(self) -> u64 {
        self.as_integer() >> 1
    }

    pub fn unpack_bit(self) -> bool {
        self.as_integer() & 1 != 0
    }

    pub fn pack(node_id: NodeId, is_reverse: bool) -> Handle {
        let NodeId(id) = node_id;
        if id < (0x1 << 63) {
            Handle::from_integer((id << 1) | is_reverse as u64)
        } else {
            panic!(
                "Tried to create a handle with a node ID that filled 64 bits"
            )
        }
    }

    pub fn id(&self) -> NodeId {
        NodeId(self.unpack_number())
    }

    pub fn is_reverse(&self) -> bool {
        self.unpack_bit()
    }

    pub fn flip(&self) -> Self {
        Handle(self.as_integer() ^ 1)
    }
}

#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub struct Edge(pub Handle, pub Handle);

impl Edge {
    fn edge_handle(left: &Handle, right: &Handle) -> Edge {
        let flipped_right = right.flip();
        let flipped_left = left.flip();

        if left > &flipped_right {
            Edge(flipped_right, flipped_left)
        } else if left == &flipped_right {
            if right > &flipped_left {
                Edge(flipped_right, flipped_left)
            } else {
                Edge(*left, *right)
            }
        } else {
            Edge(*left, *right)
        }
    }
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

        assert_eq!(h1.unpack_number(), h2.unpack_number());
        assert_eq!(h1.unpack_bit(), true);
        assert_eq!(h2.unpack_bit(), false);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Direction {
    Left,
    Right,
}
