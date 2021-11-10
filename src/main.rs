use variant_count::VariantCount;
use bitflags::bitflags;

pub mod coordinates;
pub mod bitboard;
pub mod fen;

use bitboard::BitBoard;
use coordinates::{Rank, File, BoardPos};


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
}


#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Color {
    White,
    Black,
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

bitflags! {
    pub struct MoveFlags: u16 {
        const CAPTURES_PAWN    = 0b0000_0000_0000_0001;
        const CAPTURES_PAWN_EP = 0b0000_0000_0000_0010;
        const CAPTURES_ROOK    = 0b0000_0000_0000_0100;
        const CAPTURES_KNIGHT  = 0b0000_0000_0000_1000;
        const CAPTURES_BISHOP  = 0b0000_0000_0001_0000;
        const CAPTURES_KING    = 0b0000_0000_0010_0000;
        const CAPTURES_QUEEN   = 0b0000_0000_0100_0000;

        /// This moves causes the loss of white's rights to castle kingside
        const REVOKES_W_CR_KS  = 0b0000_0000_1000_0000;
        const REVOKES_W_CR_QS  = 0b0000_0001_0000_0000;
        const REVOKES_B_CR_KS  = 0b0000_0010_0000_0000;
        const REVOKES_B_CR_QS  = 0b0000_0100_0000_0000;
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Move {
    pub from: BoardPos,
    pub to: BoardPos,

    /// Redundant information, to avoid searching all bitboards for the source piece
    pub piece: Piece,

    pub flags: MoveFlags,
}

bitflags! {
    pub struct StateFlags: u32 {
        /// White has castle-rights kingside
        const WHITE_CR_KS = 0b00000001;

        /// White has castle-rights queenside
        const WHITE_CR_QS = 0b00000010;

        /// Black has castle-rights kingside
        const BLACK_CR_KS = 0b00000100;

        /// Black has castle-rights queenside
        const BLACK_CR_QS = 0b00001000;
    }
}

pub struct State {
    pub to_play: Color,

    pub flags: StateFlags,

    /// If the previous move was advancing a pawn two spaces, the file of that pawn
    pub en_passant_file: Option<File>,

    /// Number of halfmoves since the last capture of pawn advance
    pub halfmove_clock: u8,

    // Number of full moves since game start. Incremented after each black move.
    pub fullmove_counter: u16,

    pub white_bitboards: [BitBoard; Piece::VARIANT_COUNT],
    pub black_bitboards: [BitBoard; Piece::VARIANT_COUNT],

    pub white_union_bitboard: BitBoard,
    pub black_union_bitboard: BitBoard,
    pub all_union_bitboard: BitBoard,
}

impl State {
    pub fn new_empty() -> Self {
        Self {
            to_play: Color::White,
            flags: StateFlags::empty(),
            en_passant_file: None,
            halfmove_clock: 0,
            fullmove_counter: 0,
            white_bitboards: [BitBoard::new_empty(); Piece::VARIANT_COUNT],
            black_bitboards: [BitBoard::new_empty(); Piece::VARIANT_COUNT],
            white_union_bitboard: BitBoard::new_empty(),
            black_union_bitboard: BitBoard::new_empty(),
            all_union_bitboard: BitBoard::new_empty(),
        }
    }

    pub fn add_piece(&mut self, color: Color, piece: Piece, pos: BoardPos) {
        if self.all_union_bitboard.get(pos) {
            panic!("Attempting to add a piece on top of another");
        }

        match color {
            Color::White => {
                self.white_union_bitboard.set(pos);
                self.white_bitboards[piece.to_num() as usize].set(pos);
            },
            Color::Black => {
                self.black_union_bitboard.set(pos);
                self.black_bitboards[piece.to_num() as usize].set(pos);
            },
        }

        self.all_union_bitboard.set(pos);
    }

    pub fn bitboard(&self, color: Color, piece: Piece) -> &BitBoard {
        match color {
            Color::White => &self.white_bitboards[piece.to_num() as usize],
            Color::Black => &self.black_bitboards[piece.to_num() as usize],
        }
    }

    pub fn color_union_bitboard(&self, color: Color) -> &BitBoard {
        match color {
            Color::White => &self.white_union_bitboard,
            Color::Black => &self.black_union_bitboard,
        }
    }
}

fn main() {
    println!("Hello, world!");
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::coordinates::proptest_helpers::*;

    use proptest::strategy::{Just, Strategy};
    use proptest::{proptest, prop_oneof, prop_compose};

    fn arb_color() -> impl Strategy<Value=Color> {
        prop_oneof![
            Just(Color::White),
            Just(Color::Black),
        ]
    }

    fn arb_piece() -> impl Strategy<Value=Piece> {
        prop_oneof![
            Just(Piece::Pawn),
            Just(Piece::Rook),
            Just(Piece::Knight),
            Just(Piece::Bishop),
            Just(Piece::King),
            Just(Piece::Queen),
        ]
    }

    proptest! {
        #[test]
        fn test_piece_num_roundtrips(piece in arb_piece()) {
            let num = piece.to_num();
            let piece2 = Piece::from_num(num);
            assert_eq!(piece, piece2);
        }

        #[test]
        fn test_state_add_piece(
            color in arb_color(),
            piece in arb_piece(),
            pos in arb_boardpos()
        ) {
            let mut state = State::new_empty();

            assert!(!state.color_union_bitboard(color).any());
            state.add_piece(color, piece, pos);

            let bb = state.bitboard(color, piece);
            assert_eq!(bb, state.color_union_bitboard(color));
            assert_eq!(bb, &state.all_union_bitboard);
            assert_eq!(bb.count(), 1);
            assert!(bb.get(pos));

            let other_bb = state.bitboard(!color, piece);
            assert!(!other_bb.any());
        }
    }
}