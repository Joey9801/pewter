use variant_count::VariantCount;

#[derive(Clone, Copy, Debug, PartialEq, Eq, VariantCount)]
pub enum Color {
    White,
    Black,
}

impl Color {
    pub const fn to_num(&self) -> u8 {
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
