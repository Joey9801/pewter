use variant_count::VariantCount;
use bitflags::bitflags;

pub mod coordinates;
pub mod bitboard;
pub mod fen;

use bitboard::BitBoard;
use coordinates::{Rank, File, BoardPos, consts::*};


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
        const CR_WHITE_KS_MAINTAIN = 0b0001;
        const CR_WHITE_QS_MAINTAIN = 0b0010;
        const CR_BLACK_KS_MAINTAIN = 0b0100;
        const CR_BLACK_QS_MAINTAIN = 0b1000;
        const CR_MAINTAIN_MASK = 
            Self::CR_WHITE_KS_MAINTAIN.bits |
            Self::CR_WHITE_QS_MAINTAIN.bits |
            Self::CR_BLACK_KS_MAINTAIN.bits |
            Self::CR_BLACK_QS_MAINTAIN.bits;

        const CASTLE_KINGSIDE  = 0b0001_0000;
        const CASTLE_QUEENSIDE = 0b0010_0000;
        const ANY_CASTLING = Self::CASTLE_KINGSIDE.bits | Self::CASTLE_QUEENSIDE.bits;

        // This move is an en-passant capture
        const EP_CAPTURE = 0b0100_0000;

        // The double jump of a pawn as its first move
        const DOUBLE_PAWN = 0b1000_0000;

        const SPECIAL_MOVE =
            Self::ANY_CASTLING.bits |
            Self::EP_CAPTURE.bits |
            Self::DOUBLE_PAWN.bits;

        // This was a pawn move to the last rank, and it is being promoted into
        // something
        const PROMOTE_QUEEN  = 0b0001_0000_0000;
        const PROMOTE_ROOK   = 0b0010_0000_0000;
        const PROMOTE_BISHOP = 0b0100_0000_0000;
        const PROMOTE_KNIGHT = 0b1000_0000_0000;

        const ANY_PROMOTE =
            Self::PROMOTE_QUEEN.bits |
            Self::PROMOTE_ROOK.bits |
            Self::PROMOTE_BISHOP.bits |
            Self::PROMOTE_KNIGHT.bits;
    }
}

impl MoveFlags {
    const fn promoted_piece(&self) -> Option<Piece> {
        if self.intersects(MoveFlags::ANY_PROMOTE) {
            if self.intersects(MoveFlags::PROMOTE_QUEEN | MoveFlags::PROMOTE_ROOK) {
                if self.contains(MoveFlags::PROMOTE_QUEEN) {
                    Some(Piece::Queen)
                } else {
                    Some(Piece::Rook)
                }
            } else {
                if self.contains(MoveFlags::PROMOTE_BISHOP) {
                    Some(Piece::Bishop)
                } else {
                    Some(Piece::Knight)
                }
            }
        } else {
            None
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Move {
    pub from: BoardPos,
    pub to: BoardPos,

    pub piece: Piece,
    pub capture_piece: Option<Piece>,

    pub flags: MoveFlags,
}

bitflags! {
    // NB: Intentionally laid out the same as the first four bits of MoveFlags.
    pub struct CastleRights: u8 {
        const WHITE_KINGSIDE  = 0b0001;
        const WHITE_QUEENSIDE = 0b0010;
        const BLACK_KINGSIDE  = 0b0100;
        const BLACK_QUEENSIDE = 0b1000;
    }
}

pub struct State {
    pub to_play: Color,

    pub castle_rights: CastleRights,

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
            castle_rights: CastleRights::empty(),
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

    fn bitboard_mut(&mut self, color: Color, piece: Piece) -> &mut BitBoard {
        match color {
            Color::White => &mut self.white_bitboards[piece.to_num() as usize],
            Color::Black => &mut self.black_bitboards[piece.to_num() as usize],
        }
    }

    pub fn color_union_bitboard(&self, color: Color) -> &BitBoard {
        match color {
            Color::White => &self.white_union_bitboard,
            Color::Black => &self.black_union_bitboard,
        }
    }

    fn color_union_bitboard_mut(&mut self, color: Color) -> &mut BitBoard {
        match color {
            Color::White => &mut self.white_union_bitboard,
            Color::Black => &mut self.black_union_bitboard,
        }
    }

    fn set(&mut self, color: Color, piece: Piece, pos: BoardPos) {
        let bb = self.bitboard_mut(color, piece);
        debug_assert!(!bb[pos]);
        bb.set(pos);

        let cu_bb = self.color_union_bitboard_mut(color);
        debug_assert!(!cu_bb[pos]);
        cu_bb.set(pos);
    }

    fn clear(&mut self, color: Color, piece: Piece, pos: BoardPos) {
        let bb = self.bitboard_mut(color, piece);
        debug_assert!(bb[pos]);
        bb.clear(pos);

        let cu_bb = self.color_union_bitboard_mut(color);
        debug_assert!(cu_bb[pos]);
        cu_bb.clear(pos);
    }

    /// Applies a move, panicking if the move doesn't fit.
    ///
    /// When panicking, may leave this object in an invalid state.
    pub fn apply_move(&mut self, m: Move) {
        self.clear(self.to_play, m.piece, m.from);
        self.set(self.to_play, m.piece, m.to);

        // Handle all regular captures, where the destination square was
        // previously occupied by the piece being captured
        if let Some(capture_piece) = m.capture_piece {
            self.clear(!self.to_play, capture_piece, m.to);
        }

        // Handle en-passant captures
        if m.flags.contains(MoveFlags::EP_CAPTURE) {
            debug_assert!(self.en_passant_file == Some(m.to.file));
            
            // The pos that we expect to find the ep-capturable pawn
            let ep_pawn_pos = match !self.to_play {
                Color::White => BoardPos::from_file_rank(m.to.file, Rank::R4),
                Color::Black => BoardPos::from_file_rank(m.to.file, Rank::R5),
            };
            self.clear(!self.to_play, Piece::Pawn, ep_pawn_pos);
        }

        // Handle castling
        if m.flags.intersects(MoveFlags::ANY_CASTLING) {
            // For a castling move, the regular move fields should describe the
            // movement of the rook. This block handles the movement of the king.
            debug_assert!(m.piece == Piece::Rook);
            debug_assert!(m.capture_piece.is_none());

            let kingside = m.flags.contains(MoveFlags::CASTLE_KINGSIDE);
            let from = match self.to_play {
                Color::White => E1,
                Color::Black => E8,
            };
            let to = match (self.to_play, kingside) {
                (Color::White, true) => G1,
                (Color::White, false) => C1,
                (Color::Black, true) => G8,
                (Color::Black, false) => C8,
            };
            self.clear(self.to_play, Piece::King, from);
            self.set(self.to_play, Piece::King, to);
        }

        // Update the castling rights
        self.castle_rights.bits &= (m.flags.bits & MoveFlags::CR_MAINTAIN_MASK.bits) as u8;

        // Update the en-passant capturable state
        if m.flags.contains(MoveFlags::DOUBLE_PAWN) {
            self.en_passant_file = Some(m.from.file);
        } else {
            self.en_passant_file = None;
        }

        if let Some(promoted_piece) = m.flags.promoted_piece() {
            debug_assert!(m.piece == Piece::Pawn);
            debug_assert!(m.to.rank == Rank::R1 || m.to.rank == Rank::R8);
            self.clear(self.to_play, Piece::Pawn, m.to);
            self.set(self.to_play, promoted_piece, m.to);
        }

        if m.capture_piece.is_some() || m.piece == Piece::Pawn {
            self.halfmove_clock = 0;
        } else {
            self.halfmove_clock += 1;
        }

        if self.to_play == Color::Black {
            self.fullmove_counter += 1;
        }
        self.to_play = !self.to_play;
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
    use proptest::{proptest, prop_oneof};

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