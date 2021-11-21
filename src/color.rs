use variant_count::VariantCount;

use crate::Rank;

#[derive(Clone, Copy, Debug, PartialEq, Eq, VariantCount)]
pub enum Color {
    White,
    Black,
}

impl Color {
    pub const fn to_num(self) -> u8 {
        match self {
            Color::White => 0,
            Color::Black => 1,
        }
    }

    pub const fn from_num(num: u8) -> Self {
        match num {
            0 => Self::White,
            1 => Self::Black,
            _ => panic!("Invalid color num"),
        }
    }
    
    pub const fn numbered_rank(self, num: u8) -> Rank {
        let num = match self {
            Color::White => num - 1,
            Color::Black => 9 - num,
        };
        Rank::from_num(num)
    }
}

impl std::ops::Not for Color {
    type Output = Self;

    fn not(self) -> Self::Output {
        match self {
            Color::White => Color::Black,
            Color::Black => Color::White,
        }
    }
}
