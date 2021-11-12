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

    pub const fn from_pos(pos: BoardPos) -> Self {
        Self(1 << pos.to_bitboard_offset())
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

    pub const fn union_with(&self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    pub const fn intersect_with(&self, other: Self) -> Self {
        Self(self.0 & other.0)
    }

    pub const fn inverse(&self) -> Self {
        Self(!self.0)
    }

    pub fn iter_all(self) -> impl Iterator<Item = BoardPos> {
        (0..64)
            .filter(move |i| (1 << i) & self.0 != 0)
            .map(BoardPos::from_bitboard_offset)
    }

    pub fn pretty_format(&self) -> String {
        use crate::Rank::*;
        use crate::File::*;

        let mut output = String::new();
        output.push_str(" +---+---+---+---+---+---+---+---+\n");

        for rank in [R8, R7, R6, R5, R4, R3, R2, R1].iter() {
            let mut first_col = true;
            for file in [A, B, C, D, E, F, G, H].iter() {
                if first_col {
                    output.push_str(" |");
                }
                output.push_str(&format!(" {} |", if self.get(BoardPos::from_file_rank(*file, *rank)) { '#' } else { ' ' }));
                first_col = false;
            }
            output.push_str(&format!(" {}\n", rank.to_num() + 1));
            output.push_str(" +---+---+---+---+---+---+---+---+\n");
        }

        output.push_str("   A   B   C   D   E   F   G   H\n");

        output
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
        Self::from_pos(pos)
    }
}