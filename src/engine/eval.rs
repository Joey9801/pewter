pub type Evaluation = i32;

pub mod consts {
    use crate::{Piece, BoardPos};
    use super::Evaluation;

    pub const POS_INFINITY: Evaluation = Evaluation::MAX - 1024;
    pub const NEG_INFINITY: Evaluation = -POS_INFINITY;

    /// The score if the current player has been mated
    pub const MATE: Evaluation = NEG_INFINITY / 2;
    
    pub const DRAW: Evaluation = 0;

    /// The material value of each piece, in centipawns
    pub const fn piece_value(piece: Piece) -> Evaluation {
        match piece {
            Piece::Pawn => 100,
            Piece::Rook => 525,
            Piece::Knight => 350,
            Piece::Bishop => 350,
            Piece::King => 0,
            Piece::Queen => 1000,
        }
    }
}

/// The total value of material in centipawns for the given color
fn material_value(state: &State, color: Color) -> Evaluation {
    Piece::all()
        .map(|p| {
            let value = consts::piece_value(p);
            let bb = state.board.color_piece_board(color, p);
            (bb.count() as Evaluation) * value
        })
        .iter()
        .sum()
}

/// Total evaluation of the given state, from the perspective of the current player.
pub fn evaluate(state: &State) -> Evaluation {
    let mut our_score = 0;
    let mut opp_score = 0;

    let our_mat = material_value(state, state.to_play);
    let opp_mat = material_value(state, !state.to_play);
    
    our_score += our_mat;
    opp_score += opp_mat;
    let mat = material_diff(state);
    match state.to_play {
        Color::White => mat,
        Color::Black => -mat,
    }
    our_score - opp_score
}
