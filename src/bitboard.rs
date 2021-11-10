use crate::BoardPos;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BitBoard(pub u64);

impl BitBoard {
    pub const fn new_empty() -> Self {
        Self(0)
    }

    pub const fn new_all() -> Self {
        Self(!0u64)
    }

    pub fn set(&mut self, pos: BoardPos) {
        self.0 |= 1u64 << pos.to_bitboard_offset();
    }

    pub fn clear(&mut self, pos: BoardPos) {
        self.0 &= !(1u64 << pos.to_bitboard_offset());
    }

    pub const fn get(&self, pos: BoardPos) -> bool {
        self.0 & (1u64 << pos.to_bitboard_offset()) != 0
    }

    pub const fn any(&self) -> bool {
        self.0 != 0
    }

    pub const fn all(&self) -> bool {
        self.0 == !0u64
    }

    pub const fn count(&self) -> u8 {
        self.0.count_ones() as u8
    }

    pub const fn union_with(&self, other: &Self) -> Self {
        Self(self.0 | other.0)
    }

    pub const fn intersect_with(&self, other: &Self) -> Self {
        Self(self.0 & other.0)
    }

    pub const fn inverse(&self) -> Self {
        Self(!self.0)
    }

    pub fn iter(self) -> impl Iterator<Item=BoardPos> {
        (0..64)
            .filter(move |i| (1 << i) & self.0 != 0)
            .map(BoardPos::from_bitboard_offset)
    }
}

impl Default for BitBoard {
    fn default() -> Self {
        Self(0u64)
    }
}

impl std::ops::Not for BitBoard {
    type Output = Self;

    fn not(self) -> Self::Output {
        Self(!self.0)
    }
}

impl std::ops::BitAnd for BitBoard {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self::Output {
        Self(self.0 & rhs.0)
    }
}

impl std::ops::BitOr for BitBoard {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl std::ops::Index<BoardPos> for BitBoard {
    type Output = bool;

    fn index(&self, idx: BoardPos) -> &Self::Output {
        // NB looks silly, but afaict required to get a static lifetime bool reference
        if self.get(idx) {
            &true
        } else {
            &false
        }
    }
}

const fn all_light_squares() -> BitBoard {
    let mut bb = BitBoard::new_empty();

    bb.0 |= 0b1010_1010 << 0;
    bb.0 |= 0b0101_0101 << 8;
    bb.0 |= 0b1010_1010 << 16;
    bb.0 |= 0b0101_0101 << 24;
    bb.0 |= 0b1010_1010 << 32;
    bb.0 |= 0b0101_0101 << 40;
    bb.0 |= 0b1010_1010 << 48;
    bb.0 |= 0b0101_0101 << 56;

    bb
}

pub const LIGHT_SQUARES: BitBoard = all_light_squares();
pub const DARK_SQUARES: BitBoard = all_light_squares().inverse();

#[cfg(test)]
mod test {
    use super::*;
    use crate::coordinates::consts::*;

    #[test]
    fn test_dark_sqares() {
        assert!(DARK_SQUARES[A1]);
        assert!(DARK_SQUARES[C1]);
        assert!(DARK_SQUARES[E1]);
        assert!(DARK_SQUARES[B2]);
        assert!(DARK_SQUARES[D2]);
        assert!(DARK_SQUARES[H2]);
        assert!(DARK_SQUARES[E5]);
        assert!(DARK_SQUARES[B6]);
        assert!(DARK_SQUARES[G7]);
    }
}