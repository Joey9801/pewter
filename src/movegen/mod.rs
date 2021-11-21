use crate::{BitBoard, BoardPos, File, MoveSet, Piece, Rank, State, chessmove::MoveSetChunk};

pub mod bmi;

pub fn all_pseudo_legal(state: &State) -> MoveSet {
    let mut moves = MoveSet::new_empty();

    pawn_psuedo_legal(state, &mut moves);
    knight_pseudo_legal(state, &mut moves);
    bishop_pseudo_legal(state, &mut moves);
    rook_pseudo_legal(state, &mut moves);
    
    moves
}

fn pawn_psuedo_legal(state: &State, moves: &mut MoveSet) {
    let color = state.to_play;
    let all_bb = state.board.all_union_board();
    let opp_bb = state.board.color_board(!color);

    // The pawns which are still on their starting square
    for source in state
        .board
        .color_piece_board(state.to_play, Piece::Pawn)
        .iter_set()
    {
        // Pawns should never exist in the last rank of the color - they must be promoted
        debug_assert!(source.rank != color.numbered_rank(8));

        let mut chunk = MoveSetChunk::new_empty(source);

        // Single pushes
        let single_dest = source.forward(color).unwrap();
        if !all_bb[single_dest] {
            chunk.dest_set.set(single_dest);
        }

        // Attacks
        if let Some(attack_pos) = single_dest.left() {
            if opp_bb[attack_pos] {
                chunk.dest_set.set(attack_pos)
            }
        }
        if let Some(attack_pos) = single_dest.right() {
            if opp_bb[attack_pos] {
                chunk.dest_set.set(attack_pos)
            }
        }
        
        // Double pushes
        if source.rank == color.numbered_rank(2) {
            let double_dest = single_dest.forward(color).unwrap();
            let m = BitBoard::single(single_dest)
                .union_with(BitBoard::single(double_dest));

            if !all_bb.intersect_with(m).any() {
                chunk.dest_set.set(double_dest);
            }
        }
        
        // Promotions
        if source.rank == color.numbered_rank(7) {
            chunk.promotion = true;
        }

        if chunk.dest_set.any() {
            moves.chunks.push(chunk);
        }
    }
}

const KNIGHT_MOVE_OFFSETS: &[(i8, i8)] = &[
    (2, -1),
    (2, 1),
    (1, -2),
    (1, 2),
    (-1, -2),
    (-1, 2),
    (-2, -1),
    (-2, 1),
];

fn knight_pseudo_legal(state: &State, moves: &mut MoveSet) {
    let our_pieces = state.board.color_board(state.to_play);

    for source in state
        .board
        .color_piece_board(state.to_play, Piece::Knight)
        .iter_all()
    {
        let mut chunk = MoveSetChunk::new_empty(source);

        let source_nums = (source.file.to_num() as i8, source.rank.to_num() as i8);
        for dest_nums in KNIGHT_MOVE_OFFSETS
            .iter()
            .map(|km_off| (source_nums.0 + km_off.0, source_nums.1 + km_off.1))
            .filter(|(x, _)| *x >= 0 && *x <= 7)
            .filter(|(_, y)| *y >= 0 && *y <= 7)
        {
            let dest = BoardPos::from_file_rank(
                File::from_num(dest_nums.0 as u8),
                Rank::from_num(dest_nums.1 as u8),
            );
            
            chunk.dest_set.set(dest);
        }
        
        chunk.dest_set.intersect_inplace(!our_pieces);
        if chunk.dest_set.any() {
            moves.chunks.push(chunk);
        }
    }
}

fn sliding_piece_pseudo_legal(state: &State, moves: &mut MoveSet, dirs: [(i8, i8); 4], pieces: BitBoard) {
    for source in pieces.iter_set() {
        let mut chunk = MoveSetChunk::new_empty(source);

        let source_nums = (source.file.to_num() as i8, source.rank.to_num() as i8);
        for dir in dirs {
            let mut dest_nums = source_nums.clone();
            loop {
                dest_nums.0 += dir.0;
                dest_nums.1 += dir.1;

                if dest_nums.0 < 0 || dest_nums.0 >= 8 {
                    break;
                }
                if dest_nums.1 < 0 || dest_nums.1 >= 8 {
                    break;
                }

                let dest = BoardPos::from_file_rank(
                    File::from_num(dest_nums.0 as u8),
                    Rank::from_num(dest_nums.1 as u8),
                );
                

                match state.board.get(dest) {
                    Some((c, _)) if c == state.to_play => break,
                    _ => {
                        chunk.dest_set.set(dest);
                    }
                }
            }
        }
        
        if chunk.dest_set.any() {
            moves.chunks.push(chunk);
        }
    }
}

const BISHOP_DIRS: [(i8, i8); 4] = [(-1, -1), (-1, 1), (1, -1), (1, 1)];

fn bishop_pseudo_legal(state: &State, moves: &mut MoveSet) {
    let bishops = BitBoard::new_empty()
        .union_with(state.board.piece_board(Piece::Bishop))
        .union_with(state.board.piece_board(Piece::Queen))
        .intersect_with(state.board.color_board(state.to_play));
    sliding_piece_pseudo_legal(state, moves, BISHOP_DIRS, bishops);
}

const ROOK_DIRS: [(i8, i8); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];

fn rook_pseudo_legal(state: &State, moves: &mut MoveSet) {
    let rooks = BitBoard::new_empty()
        .union_with(state.board.piece_board(Piece::Rook))
        .union_with(state.board.piece_board(Piece::Queen))
        .intersect_with(state.board.color_board(state.to_play));
    sliding_piece_pseudo_legal(state, moves, ROOK_DIRS, rooks);
}


#[cfg(test)]
mod test {
    use super::*;
    use crate::io::fen::parse_fen;

    #[test]
    fn count_moves_from_starting() {
        let state = parse_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1")
            .expect("Expect test case FEN to be correct");

        let moves = all_pseudo_legal(&state);

        for m in moves.iter() {
            println!("{}", m.format_long_algebraic());
        }

        assert_eq!(moves.len(), 20);
    }
}
