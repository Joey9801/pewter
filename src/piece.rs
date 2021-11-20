use variant_count::VariantCount;

#[derive(Clone, Copy, Debug, VariantCount, PartialEq, Eq)]
pub enum Piece {
    Pawn,
    Rook,
    Knight,
    Bishop,
    King,
    Queen,
}

impl Piece {
    pub const fn to_num(&self) -> u8 {
        match self {
            Piece::Pawn => 0,
            Piece::Rook => 1,
            Piece::Knight => 2,
            Piece::Bishop => 3,
            Piece::King => 4,
            Piece::Queen => 5,
        }
    }

    pub const fn from_num(num: u8) -> Self {
        match num {
            0 => Piece::Pawn,
            1 => Piece::Rook,
            2 => Piece::Knight,
            3 => Piece::Bishop,
            4 => Piece::King,
            5 => Piece::Queen,
            _ => panic!("Invalid piece num"),
        }
    }

    pub const fn to_char(&self) -> char {
        match self {
            Piece::Pawn => 'p',
            Piece::Rook => 'r',
            Piece::Knight => 'n',
            Piece::Bishop => 'b',
            Piece::King => 'k',
            Piece::Queen => 'q',
        }
    }
    
    pub fn iter_all() -> impl Iterator<Item=Self> {
        [
            Piece::Pawn,
            Piece::Rook,
            Piece::Knight,
            Piece::Bishop,
            Piece::King,
            Piece::Queen,
        ].iter().cloned()
    }
}
