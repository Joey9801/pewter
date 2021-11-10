use bitflags::bitflags;

use crate::coordinates::{BoardPos, File};
use crate::{Piece, State};

bitflags! {
    pub struct MoveFlags: u8 {
        const CASTLE_KINGSIDE  = 0b0001;
        const CASTLE_QUEENSIDE = 0b0010;
        const ANY_CASTLING = Self::CASTLE_KINGSIDE.bits | Self::CASTLE_QUEENSIDE.bits;

        // This move is an en-passant capture
        const EP_CAPTURE = 0b0100;

        // The double jump of a pawn as its first move
        const DOUBLE_PAWN = 0b1000;

        const SPECIAL_MOVE =
            Self::ANY_CASTLING.bits |
            Self::EP_CAPTURE.bits |
            Self::DOUBLE_PAWN.bits;

        // This was a pawn move to the last rank, and it is being promoted into
        // something
        const PROMOTE_QUEEN  = 0b0001_0000;
        const PROMOTE_ROOK   = 0b0010_0000;
        const PROMOTE_BISHOP = 0b0100_0000;
        const PROMOTE_KNIGHT = 0b1000_0000;

        const ANY_PROMOTE =
            Self::PROMOTE_QUEEN.bits |
            Self::PROMOTE_ROOK.bits |
            Self::PROMOTE_BISHOP.bits |
            Self::PROMOTE_KNIGHT.bits;
    }
}

impl MoveFlags {
    pub const fn promoted_piece(&self) -> Option<Piece> {
        if self.intersects(MoveFlags::ANY_PROMOTE) {
            if self.intersects(MoveFlags::PROMOTE_QUEEN.union(MoveFlags::PROMOTE_ROOK)) {
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

#[derive(Clone, Copy, Debug)]
pub enum ParseLongAlgebraicError {
    MissingChars,
    NonAsciiBytes,
    InvalidSquare,
    NoPiece,
    WrongColor,
    CaptureSelf,
    BadCastle,
    BadPromotion,
}

impl Move {
    pub fn from_long_algebraic(
        state: &State,
        algebraic_str: &str,
    ) -> Result<Self, ParseLongAlgebraicError> {
        if algebraic_str.len() < 4 {
            return Err(ParseLongAlgebraicError::MissingChars);
        }
        if !algebraic_str.is_ascii() {
            return Err(ParseLongAlgebraicError::NonAsciiBytes);
        }

        let from = BoardPos::from_algebraic(&algebraic_str[0..2])
            .ok_or(ParseLongAlgebraicError::InvalidSquare)?;
        let to = BoardPos::from_algebraic(&algebraic_str[2..4])
            .ok_or(ParseLongAlgebraicError::InvalidSquare)?;

        let (color, piece) = state.get(from).ok_or(ParseLongAlgebraicError::NoPiece)?;

        if color != state.to_play {
            return Err(ParseLongAlgebraicError::WrongColor);
        }

        let mut m = Move {
            from,
            to,
            piece,
            capture_piece: None,
            flags: MoveFlags::empty(),
        };

        if let Some((capture_color, capture_piece)) = state.get(to) {
            if capture_color == state.to_play {
                return Err(ParseLongAlgebraicError::CaptureSelf);
            }

            m.capture_piece = Some(capture_piece);
        }

        if m.capture_piece.is_none() && m.piece == Piece::Pawn && m.from.file != m.to.file {
            // Good enough heuristic for parsing - not aiming for full legal
            // move checking in this parser.
            m.flags.set(MoveFlags::EP_CAPTURE, true);
        }

        if m.piece == Piece::Pawn && (m.from.manhattan_distance(&m.to) > 1) {
            m.flags.set(MoveFlags::DOUBLE_PAWN, true);
        }

        if m.piece == Piece::King && (m.from.manhattan_distance(&m.to) > 1) {
            // Again, not nearly good enough for full legality checking, but
            // enough of a heuristic.

            if m.to.file == File::G {
                m.flags.set(MoveFlags::CASTLE_KINGSIDE, true);
            } else if m.to.file == File::C {
                m.flags.set(MoveFlags::CASTLE_QUEENSIDE, true);
            } else {
                return Err(ParseLongAlgebraicError::BadCastle);
            }
        }

        if algebraic_str.as_bytes().len() > 4 {
            match &algebraic_str[5..6] {
                "q" => m.flags.set(MoveFlags::PROMOTE_QUEEN, true),
                "r" => m.flags.set(MoveFlags::PROMOTE_ROOK, true),
                "b" => m.flags.set(MoveFlags::PROMOTE_BISHOP, true),
                "n" => m.flags.set(MoveFlags::PROMOTE_KNIGHT, true),
                _ => return Err(ParseLongAlgebraicError::BadPromotion),
            }
        }

        Ok(m)
    }
}
