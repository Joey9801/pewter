use crate::state::{CastleRights, State};
use crate::{BoardPos, Color, Piece};

pub mod consts {
    include!(concat!(env!("OUT_DIR"), "/zobrist_gen.rs"));
}

pub const fn castling_number(castle_rights: CastleRights) -> u64 {
    let cr_num = castle_rights.bits() as usize;
    consts::ZOBRIST_CASTLING[cr_num]
}

pub const fn to_play_num(to_play: Color) -> u64 {
    match to_play {
        Color::White => consts::ZOBRIST_WHITE_TURN,
        Color::Black => 0,
    }
}

pub const fn piece_number(color: Color, piece: Piece, pos: BoardPos) -> u64 {
    let idx = color.to_num() as usize * Piece::VARIANT_COUNT * 64
        + piece.to_num() as usize * 64
        + pos.to_bitboard_offset() as usize;

    consts::ZOBRIST_PSC[idx]
}

pub const fn ep_number(ep: Option<BoardPos>) -> u64 {
    match ep {
        Some(ep) => consts::ZOBRIST_EP[ep.file.to_num() as usize],
        None => 0,
    }
}

// Used when initialising positions, and for unit testing efficient updates
pub fn calculate_entire_zobrist(state: &State) -> u64 {
    let mut zobrist_num = 0;

    zobrist_num ^= castling_number(state.castle_rights);
    zobrist_num ^= to_play_num(state.to_play);
    
    for color in [Color::White, Color::Black] {
        for piece in Piece::iter_all() {
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