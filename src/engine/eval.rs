use crate::{Color, Piece, State};

pub mod consts {
    use crate::Piece;

    pub const POS_INFINITY: i32 = i32::MAX - 1024;
    pub const NEG_INFINITY: i32 = i32::MIN + 1024;

    /// The score if the current player has been mated
    pub const MATE: i32 = NEG_INFINITY / 2;

    /// The material value of each piece, in centipawns
    pub const fn piece_value(piece: Piece) -> i32 {
        match piece {
            Piece::Pawn => 1000,
            Piece::Rook => 525,
            Piece::Knight => 350,
            Piece::Bishop => 350,
            Piece::King => 0,
            Piece::Queen => 1000,
        }
    }
}

/// The linear difference in material, in centipawns, from white's perspective.
pub fn material_diff(state: &State) -> i32 {
    Piece::all()
        .map(|p| {
            let value = consts::piece_value(p);
            let wb = state.board.color_piece_board(Color::White, p);
            let bb = state.board.color_piece_board(Color::Black, p);
            (wb.count() as i32 - bb.count() as i32) * value
        })
        .iter()
        .sum()
}

/// Total evaluation of the given state, from the perspective of the current player.
pub fn evaluate(state: &State) -> i32 {
    // TODO: more intelligent eval

    let mat = material_diff(state);
    match state.to_play {
        Color::White => mat,
        Color::Black => -mat,
    }
}
