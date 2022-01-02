use std::cmp::Reverse;

use pewter_core::{Move, State, Piece};

use super::{eval::{Evaluation, self}, transposition::TranspositionTable};

fn predicted_score(state: &State, m: Move, hash_move: Option<Move>) -> Evaluation {
    let mut score = 0;
    
    let piece = state.board.get(m.from)
        .expect("Move doesn't target a piece")
        .1;

    if let Some((_color, capture_piece)) = state.board.get(m.to) {
        // Capturing a high value piece with a low value piece is best
        score += eval::consts::piece_value(capture_piece) - eval::consts::piece_value(piece);

        // Capturing anything is better than capturing nothing, so add enough to make sure that the
        // score is still higher.
        score += eval::consts::piece_value(Piece::Queen) + 10;
    }
    
    if let Some(promotion) = m.promotion {
        score += eval::consts::piece_value(promotion);
    }
    
    if hash_move == Some(m) {
        score += 10000;
    }

    score
}

pub fn order_moves(state: &State, moves: &mut [Move], t: &TranspositionTable) {
    let hash_move = t
        .probe(state, 0, eval::consts::POS_INFINITY, eval::consts::NEG_INFINITY)
        .map(|e| e.m)
        .flatten();

    moves.sort_by_cached_key(|m| Reverse(predicted_score(state, *m, hash_move)));
}