use std::iter::FromIterator;

use crate::BoardPos;

pub mod masks;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BitBoard(pub u64);

impl BitBoard {
    pub const fn new_empty() -> Self {
        Self(0)
    }

    pub const fn new_all() -> Self {
        Self(!0u64)
    }

    pub const fn single(pos: BoardPos) -> Self {
        Self(1 << pos.to_bitboard_offset())
    }

    pub const fn with_set(mut self, pos: BoardPos) -> Self {
        self.0 |= 1u64 << pos.to_bitboard_offset();
        self
    }

    pub fn set(&mut self, pos: BoardPos) {
        *self = self.with_set(pos);
    }

    pub const fn with_cleared(mut self, pos: BoardPos) -> Self {
        self.0 &= !(1u64 << pos.to_bitboard_offset());
        self
    }

    pub fn clear(&mut self, pos: BoardPos) {
        *self = self.with_cleared(pos);
    }

    pub const fn get(&self, pos: BoardPos) -> bool {
        self.0 & (1u64 << pos.to_bitboard_offset()) != 0
    }

    pub const fn const_eq(&self, other: Self) -> bool {
        self.0 == other.0
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

    pub const fn union_with(&self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    pub const fn intersect_with(&self, other: Self) -> Self {
        Self(self.0 & other.0)
    }

    pub const fn inverse(&self) -> Self {
        Self(!self.0)
    }

    pub const fn first_set(&self) -> Option<BoardPos> {
        let tzs = self.0.trailing_zeros() as u8;
        if tzs <= 63 {
            Some(BoardPos::from_bitboard_offset(tzs))
        } else {
            None
        }
    }

    pub fn iter_all(self) -> impl Iterator<Item = BoardPos> {
        (0..64)
            .filter(move |i| (1 << i) & self.0 != 0)
            .map(BoardPos::from_bitboard_offset)
    }

    pub fn pretty_format(&self) -> String {
        use crate::io::ascii::pretty_format;
        pretty_format(|pos| if self.get(pos) { '#' } else { ' ' })
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

impl From<BoardPos> for BitBoard {
    fn from(pos: BoardPos) -> Self {
        Self::single(pos)
    }
}

impl FromIterator<BoardPos> for BitBoard {
    fn from_iter<T: IntoIterator<Item = BoardPos>>(iter: T) -> Self {
        let mut bb = Self::new_empty();

        for pos in iter {
            bb.set(pos);
        }

        bb
    }
}

impl Iterator for BitBoard {
    type Item = BoardPos;

    fn next(&mut self) -> Option<Self::Item> {
        self.first_set()
    }
    
}