use crate::{Color, Piece, State, BoardPos};

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

    pub const STARTING_MATERIAL: Evaluation =
        piece_value(Piece::Pawn) * 8 +
        piece_value(Piece::Rook) * 2 +
        piece_value(Piece::Knight) * 2 +
        piece_value(Piece::Bishop) * 2 +
        piece_value(Piece::Queen);

    const PAWN_SQUARE_TABLE: [Evaluation; 64] = [
        0,  0,  0,  0,  0,  0,  0,  0,
        50, 50, 50, 50, 50, 50, 50, 50,
        10, 10, 20, 30, 30, 20, 10, 10,
         5,  5, 10, 25, 25, 10,  5,  5,
         0,  0,  0, 20, 20,  0,  0,  0,
         5, -5,-10,  0,  0,-10, -5,  5,
         5, 10, 10,-20,-20, 10, 10,  5,
         0,  0,  0,  0,  0,  0,  0,  0
    ];
    
    const KNIGHT_SQUARE_TABLE: [Evaluation; 64] = [
        -50,-40,-30,-30,-30,-30,-40,-50,
        -40,-20,  0,  0,  0,  0,-20,-40,
        -30,  0, 10, 15, 15, 10,  0,-30,
        -30,  5, 15, 20, 20, 15,  5,-30,
        -30,  0, 15, 20, 20, 15,  0,-30,
        -30,  5, 10, 15, 15, 10,  5,-30,
        -40,-20,  0,  5,  5,  0,-20,-40,
        -50,-40,-30,-30,-30,-30,-40,-50, 
    ];
    
    const BISHOP_SQUARE_TABLE: [Evaluation; 64] = [
        -20,-10,-10,-10,-10,-10,-10,-20,
        -10,  0,  0,  0,  0,  0,  0,-10,
        -10,  0,  5, 10, 10,  5,  0,-10,
        -10,  5,  5, 10, 10,  5,  5,-10,
        -10,  0, 10, 10, 10, 10,  0,-10,
        -10, 10, 10, 10, 10, 10, 10,-10,
        -10,  5,  0,  0,  0,  0,  5,-10,
        -20,-10,-10,-10,-10,-10,-10,-20,
    ];
    
    const ROOK_SQUARE_TABLE: [Evaluation; 64] = [
        0,  0,  0,  0,  0,  0,  0,  0,
        5, 10, 10, 10, 10, 10, 10,  5,
       -5,  0,  0,  0,  0,  0,  0, -5,
       -5,  0,  0,  0,  0,  0,  0, -5,
       -5,  0,  0,  0,  0,  0,  0, -5,
       -5,  0,  0,  0,  0,  0,  0, -5,
       -5,  0,  0,  0,  0,  0,  0, -5,
        0,  0,  0,  5,  5,  0,  0,  0
    ];
    
    const QUEEN_SQUARE_TABLE: [Evaluation; 64] = [
        -20,-10,-10, -5, -5,-10,-10,-20,
        -10,  0,  0,  0,  0,  0,  0,-10,
        -10,  0,  5,  5,  5,  5,  0,-10,
         -5,  0,  5,  5,  5,  5,  0, -5,
          0,  0,  5,  5,  5,  5,  0, -5,
        -10,  5,  5,  5,  5,  5,  0,-10,
        -10,  0,  5,  0,  0,  0,  0,-10,
        -20,-10,-10, -5, -5,-10,-10,-20
    ];
    
    const NULL_SQUARE_TABLE: [Evaluation; 64] = [
       0, 0, 0, 0, 0, 0, 0, 0,  
       0, 0, 0, 0, 0, 0, 0, 0,  
       0, 0, 0, 0, 0, 0, 0, 0,  
       0, 0, 0, 0, 0, 0, 0, 0,  
       0, 0, 0, 0, 0, 0, 0, 0,  
       0, 0, 0, 0, 0, 0, 0, 0,  
       0, 0, 0, 0, 0, 0, 0, 0,  
       0, 0, 0, 0, 0, 0, 0, 0,  
    ];
    
    const CENTER_MANHATTEN_DISTANCE: [u8; 64] = [
        6, 5, 4, 3, 3, 4, 5, 6,
        5, 4, 3, 2, 2, 3, 4, 5,
        4, 3, 2, 1, 1, 2, 3, 4,
        3, 2, 1, 0, 0, 1, 2, 3,
        3, 2, 1, 0, 0, 1, 2, 3,
        4, 3, 2, 1, 1, 2, 3, 4,
        5, 4, 3, 2, 2, 3, 4, 5,
        6, 5, 4, 3, 3, 4, 5, 6,
    ];
    
    pub const fn piece_square_table(piece: Piece) -> [Evaluation; 64] {
        match piece {
            Piece::Pawn => PAWN_SQUARE_TABLE,
            Piece::Knight => KNIGHT_SQUARE_TABLE,
            Piece::Bishop => BISHOP_SQUARE_TABLE,
            Piece::Rook => ROOK_SQUARE_TABLE,
            Piece::Queen => QUEEN_SQUARE_TABLE,
            _ => NULL_SQUARE_TABLE
        }
    }
    
    pub const fn center_manhatten_distance(pos: BoardPos) -> Evaluation {
        CENTER_MANHATTEN_DISTANCE[pos.to_bitboard_offset() as usize] as Evaluation
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

fn piece_square_value_single(color: Color, piece: Piece, pos: BoardPos) -> Evaluation {
    let table = consts::piece_square_table(piece);
    let index = match color {
        Color::White => pos.to_bitboard_offset(),
        Color::Black => 63 - pos.to_bitboard_offset(),
    };
    
    table[index as usize]
}

fn piece_square_value(state: &State, color: Color) -> Evaluation {
    Piece::all()
        .iter()
        .map(|&piece| {
            let bb = state.board.color_piece_board(color, piece);
            bb.iter_set()
                .map(|pos| piece_square_value_single(color, piece, pos))
                .sum::<Evaluation>()
        })
        .sum()
}

fn endgame_weight(state: &State, color: Color, mat: Evaluation) -> f32 {
    let pv = consts::piece_value(Piece::Pawn);
    let pawn_count = state.board.color_piece_board(color, Piece::Pawn).count() as i32;
    let eg_mat = mat - pawn_count * pv;

    let reference_eg_mat = consts::piece_value(Piece::Rook) * 2
        + consts::piece_value(Piece::Bishop)
        + consts::piece_value(Piece::Knight);
    
    // No end-game material is "maximally end-game"
    // More than the reference end-game material is "minimally end-game"
    
    let x = eg_mat as f32 / reference_eg_mat as f32;
    if x > 1f32 {
        1f32
    } else {
        1f32 - x
    }
}

/// In the endgame, it is beneficial to push the opponent king to the edges of the board.
///
/// This method returns more positive evaluation the closer the opponents king is to the sides, but
/// only if in the endgame.
fn push_opp_king_to_sides(state: &State, color: Color, weight: f32, our_mat: Evaluation, opp_mat: Evaluation) -> Evaluation {
    if our_mat < (opp_mat + consts::piece_value(Piece::Pawn) * 2) {
        return 0;
    }
    
    let opp_king_pos = state.board.king_pos(!color)
        .expect("There is no opponent king");

    let score = consts::center_manhatten_distance(opp_king_pos) * 10;

    (score as f32 * weight) as Evaluation
}

fn nonlinear_material_diff(our_mat: Evaluation, opp_mat: Evaluation) -> Evaluation {
    // A material difference is more meaningful when there is less material on the board

    let diff = our_mat - opp_mat;
    let total = our_mat + opp_mat;
    
    ((diff as f32 / total as f32) * 100.0) as Evaluation
}

/// Total evaluation of the given state, from the perspective of the current player.
pub fn evaluate(state: &State) -> Evaluation {
    let mut our_score = 0;
    let mut opp_score = 0;

    let our_mat = material_value(state, state.to_play);
    let opp_mat = material_value(state, !state.to_play);
    
    our_score += our_mat;
    opp_score += opp_mat;
    
    our_score += nonlinear_material_diff(our_mat, opp_mat);
    
    // Having a pair of bishops is more than twice as good as having a single bishop
    if state.board.color_piece_board(state.to_play, Piece::Bishop).count() > 1 {
        our_score += 100;
    }
    if state.board.color_piece_board(!state.to_play, Piece::Bishop).count() > 1 {
        opp_score += 100;
    }

    our_score += piece_square_value(state, state.to_play);
    opp_score += piece_square_value(state, !state.to_play);
    
    let our_eg_weight = endgame_weight(state, state.to_play, opp_mat);
    let opp_eg_weight = endgame_weight(state, !state.to_play, opp_mat);
    
    our_score += push_opp_king_to_sides(state, state.to_play, our_eg_weight, our_mat, opp_mat);
    opp_score += push_opp_king_to_sides(state, !state.to_play, opp_eg_weight, opp_mat, our_mat);

    our_score - opp_score
}
