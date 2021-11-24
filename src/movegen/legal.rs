use crate::{BitBoard, BoardPos, CastleSide, MoveSet, Piece, State, bitboard::masks, chessmove::MoveSetChunk};

use super::pseudo_legal;

pub fn legal_moves(state: &State) -> MoveSet {
    let mut move_set = MoveSet::new_empty();
    let k_pos = state.king_pos(state.to_play);
    
    let checker_count = state.checkers.count();

    // With zero opposing pieces giving check, it is possible for pinned pieces to move along their
    // pinned line.
    if checker_count == 0 {
        for piece in Piece::iter_all() {
            let pinned_pieces = state.board
                .piece_board(piece)
                .intersect_with(state.pinned);

            for pos in pinned_pieces.iter_set() {
                let mut chunk = legal_move_chunk(state, piece, pos, BitBoard::new_all());
                chunk.dest_set.intersect_inplace(masks::line(pos, k_pos));
                move_set.push(chunk);
            }
        }
    }

    // With zero or one opposing pieces giving check, it is possible for any non-pinned piece to
    // have legal moves.
    if checker_count <= 1 {
        // If currently in check, this mask is the set of positions that a legal move could land on,
        // such that it either blocks or captures the single piece giving check.
        let check_mask = match state.checkers.first_set() {
            Some(pos) => masks::between(pos,k_pos).union_with(state.checkers),
            None => BitBoard::new_empty().inverse(),
        };

        let non_pinned_color_mask = state.board
            .color_board(state.to_play)
            .intersect_with(!state.pinned);

        for piece in Piece::iter_all() {
            let non_pinned_pieces = state.board
                .piece_board(piece)
                .intersect_with(non_pinned_color_mask);

            for pos in non_pinned_pieces.iter_set() {
                let mut chunk = legal_move_chunk(state, piece, pos, check_mask);
                chunk.dest_set.intersect_inplace(check_mask);
                move_set.push(chunk);
            }
        }
    } else {
        // If there are two (or somehow more) pieces giving check, the only piece that can possibly
        // have any legal moves is the king itself.
        let chunk = legal_move_chunk(state, Piece::King, state.king_pos(state.to_play), BitBoard::new_all());
        move_set.push(chunk);
    }
    
    move_set
}

fn legal_move_chunk(state: &State, piece: Piece, pos: BoardPos, check_mask: BitBoard) -> MoveSetChunk {
    let mut chunk = super::pseudo_legal::pseudo_legal_moves(state, piece, pos);
    
    match piece {
        Piece::Pawn => pawn_special(state, pos, &mut chunk),
        Piece::King => king_special(state, pos, &mut chunk),
        _ => (),
    }
    
    if piece != Piece::King {
        chunk.dest_set.intersect_with(check_mask);
    }
    
    chunk
}

// Handles adding en-passant moves
fn pawn_special(state: &State, pos: BoardPos, chunk: &mut MoveSetChunk) {
    let ep_pos = match state.en_passant {
        Some(pos) => pos,
        None => return,
    };

    let pa = pseudo_legal::pawn_attacks(state.to_play, pos, BitBoard::single(ep_pos));
    if !pa.any() {
        return;
    }

    let old_pawn_pos = ep_pos.forward(!state.to_play).unwrap();
    
    // The all-union board as it would be after the en-passant move
    let blockers = state.board.all_union_board()
        .with_set(ep_pos)
        .with_cleared(pos)
        .with_cleared(old_pawn_pos);
    
    let k_pos = state.king_pos(state.to_play);
    
    let opp_board = state.board.color_board(!state.to_play);
    
    // The set of opposition rooks/bishops/queens that have a line to our king
    let dangers = BitBoard::new_empty()
        .union_with(state.board.piece_board(Piece::Rook))
        .union_with(state.board.piece_board(Piece::Bishop))
        .union_with(state.board.piece_board(Piece::Queen))
        .intersect_with(opp_board)
        .intersect_with(masks::queen_rays(k_pos));

    // It is a legal en-passant move if every dangerous sliding piece has at least one blocker in
    // the way after the move has been executed.
    let legal_ep_move = dangers.iter_set()
        .map(|danger_pos| masks::between(k_pos, danger_pos))
        .map(|mask| blockers.intersect_with(mask))
        .all(|blockers| blockers.any());
    
    if legal_ep_move {
        chunk.dest_set.set(ep_pos);
    }
}

fn legal_king_pos(state: &State, pos: BoardPos) -> bool {
    // The all union board, but with the our king moved to the proposed position
    let combined = state.board.all_union_board()
        .intersect_with(!state.board.color_piece_board(state.to_play, Piece::King))
        .with_set(pos);

    let rooks = BitBoard::new_empty()
        .union_with(state.board.piece_board(Piece::Rook))
        .union_with(state.board.piece_board(Piece::Queen))
        .intersect_with(state.board.color_board(!state.to_play))
        .intersect_with(masks::rook_rays(pos));

    let bishops = BitBoard::new_empty()
        .union_with(state.board.piece_board(Piece::Bishop))
        .union_with(state.board.piece_board(Piece::Queen))
        .intersect_with(state.board.color_board(!state.to_play))
        .intersect_with(masks::bishop_rays(pos));
    
    let sliding_dangers = rooks.union_with(bishops);
    
    let sliding_piece_check = sliding_dangers.iter_set()
        .map(|attacker_pos| masks::between(pos, attacker_pos))
        .map(|mask| combined.intersect_with(mask))
        .any(|blockers| !blockers.any());
    
    let knight_check = state.board.color_piece_board(!state.to_play, Piece::Knight)
        .intersect_with(pseudo_legal::knight_moves(pos, BitBoard::new_empty()))
        .any();

    !(sliding_piece_check || knight_check)
}

/// Handles filtering out illegal king moves and adding castling moves
fn king_special(state: &State, k_pos: BoardPos, chunk: &mut MoveSetChunk) {
    // Filter the regular moves already in the chunk
    let mut legal_regular_moves = BitBoard::new_empty();
    for pos in chunk.dest_set.iter_set() {
        if legal_king_pos(state, pos) {
            legal_regular_moves.set(pos);
        }
    }
    chunk.dest_set = legal_regular_moves;

    // Castling
    if state.castle_rights.get(state.to_play, CastleSide::Kingside) {
        let mask = masks::castling_required_empty(state.to_play, CastleSide::Kingside);
        if !state.board.all_union_board().intersect_with(mask).any() {
            let passing = k_pos.left().unwrap();
            let dest  = passing.left().unwrap();
            
            if legal_king_pos(state, passing) && legal_king_pos(state, dest) {
                chunk.dest_set.set(dest);
            }
        }
    }

    if state.castle_rights.get(state.to_play, CastleSide::Queenside) {
        let mask = masks::castling_required_empty(state.to_play, CastleSide::Queenside);
        if !state.board.all_union_board().intersect_with(mask).any() {
            let passing = k_pos.right().unwrap();
            let dest  = passing.right().unwrap();
            
            if legal_king_pos(state, passing) && legal_king_pos(state, dest) {
                chunk.dest_set.set(dest);
            }
        }
    }
}