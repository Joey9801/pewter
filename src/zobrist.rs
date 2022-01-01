use serde::{Serialize, Deserialize};

use crate::state::{CastleRights, State};
use crate::{BoardPos, Color, Piece};

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
#[derive(Serialize, Deserialize)]
pub struct ZobristHash(u64);

impl ZobristHash {
    pub const fn null() -> Self {
        Self(0)
    }
}

impl std::ops::BitXorAssign for ZobristHash {
    fn bitxor_assign(&mut self, rhs: Self) {
        self.0 ^= rhs.0;
    }
}

pub mod consts {
    use super::ZobristHash;
    include!(concat!(env!("OUT_DIR"), "/zobrist_gen.rs"));
}

pub const fn castling_number(castle_rights: CastleRights) -> ZobristHash {
    let cr_num = castle_rights.bits() as usize;
    consts::ZOBRIST_CASTLING[cr_num]
}

pub const fn to_play_num(to_play: Color) -> ZobristHash {
    match to_play {
        Color::White => consts::ZOBRIST_WHITE_TURN,
        Color::Black => ZobristHash::null(),
    }
}

pub const fn piece_number(color: Color, piece: Piece, pos: BoardPos) -> ZobristHash {
    let idx = color.to_num() as usize * Piece::VARIANT_COUNT * 64
        + piece.to_num() as usize * 64
        + pos.to_bitboard_offset() as usize;

    consts::ZOBRIST_PSC[idx]
}

pub const fn ep_number(ep: Option<BoardPos>) -> ZobristHash {
    match ep {
        Some(ep) => consts::ZOBRIST_EP[ep.file.to_num() as usize],
        None => ZobristHash::null(),
    }
}

// Used when initialising positions, and for unit testing efficient updates
pub fn calculate_entire_zobrist(state: &State) -> ZobristHash {
    let mut zobrist_num = ZobristHash(0);

    zobrist_num ^= castling_number(state.castle_rights);
    zobrist_num ^= to_play_num(state.to_play);

    for color in [Color::White, Color::Black] {
        for piece in Piece::all() {
            let cp_board = state.board.color_piece_board(color, piece);
            for pos in cp_board.iter_set() {
                zobrist_num ^= piece_number(color, piece, pos);
            }
        }
    }

    if let Some(ep) = state.en_passant {
        zobrist_num ^= consts::ZOBRIST_EP[ep.file.to_num() as usize];
    }

    zobrist_num
}
