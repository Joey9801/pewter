use crate::{Piece, State, chessmove::{Move, MoveFlags}, coordinates::{BoardPos, File, Rank}};

pub fn all_pseudo_legal(state: &State, moves: &mut Vec<Move>) {
    pawn_psuedo_legal(state, moves);
    rook_pseudo_legal(state, moves);
    knight_pseudo_legal(state, moves);
    bishop_pseudo_legal(state, moves);
}

fn pawn_psuedo_legal(state: &State, moves: &mut Vec<Move>) {
    for from in state.bitboard(state.to_play, Piece::Pawn).iter_all() {
        if let Some(one_up_rank) = from.rank.next_up(state.to_play) {
            let one_up_to = BoardPos::from_file_rank(from.file, one_up_rank);

            // Single pawn pushes
            if !state.all_union_bitboard[one_up_to] {
                moves.push(Move {
                    from,
                    to: one_up_to,
                    piece: Piece::Pawn,
                    capture_piece: None,
                    flags: MoveFlags::empty(),
                });


                // Double pawn push
                if let Some(two_up_rank) = one_up_rank.next_up(state.to_play) {
                    if from.rank == Rank::R2 || from.rank == Rank::R7 {
                        let two_up_to = BoardPos::from_file_rank(from.file, two_up_rank);

                        if !state.all_union_bitboard[two_up_to] {
                            moves.push(Move {
                                from,
                                to: two_up_to,
                                piece: Piece::Pawn,
                                capture_piece: None,
                                flags: MoveFlags::DOUBLE_PAWN,
                            });
                        }
                    }
                }
            }
        }
    }
}


const KNIGHT_MOVE_OFFSETS: &[(i8, i8)] = &[
    (2, -1),
    (2,  1),
    (1, -2),
    (1,  2),
    (-1, -2),
    (-1,  2),
    (-2, -1),
    (-2, 1),
];

fn knight_pseudo_legal(state: &State, moves: &mut Vec<Move>) {
    for from in state.bitboard(state.to_play, Piece::Knight).iter_all() {
        let from_nums = (from.file.to_num() as i8, from.rank.to_num() as i8);

        for to_nums in KNIGHT_MOVE_OFFSETS
            .iter()
            .map(|km_off| (from_nums.0 + km_off.0, from_nums.1 + km_off.1))
            .filter(|(x, _)| *x >= 0 && *x <= 7)
            .filter(|(_, x)| *x >= 0 && *x <= 7) {

            let to = BoardPos::from_file_rank(
                File::from_num(to_nums.0 as u8),
                Rank::from_num(to_nums.1 as u8),
            );

            match state.get(to) {
                Some((c, _)) if c == state.to_play => continue,
                Some((opp_c, piece)) => {
                    debug_assert!(opp_c == !state.to_play);
                    moves.push(Move {
                        from, to,
                        piece: Piece::Knight,
                        capture_piece: Some(piece),
                        flags: MoveFlags::empty(),
                    });
                }
                None => {
                    moves.push(Move {
                        from, to,
                        piece: Piece::Knight,
                        capture_piece: None,
                        flags: MoveFlags::empty(),
                    });
                }
            }
        }
    }
}

const BISHOP_DIRS: &[(i8, i8)] = &[
    (-1, -1),
    (-1, 1),
    (1, -1),
    (1, 1),
];

fn bishop_pseudo_legal(state: &State, moves: &mut Vec<Move>) {
    for from in state.bitboard(state.to_play, Piece::Bishop).iter_all() {
        let from_nums = (from.file.to_num() as i8, from.rank.to_num() as i8);

        for dir in BISHOP_DIRS {
            let mut to_nums = from_nums.clone();
            loop {
                to_nums.0 += dir.0;
                to_nums.1 += dir.1;

                if to_nums.0 < 0 || to_nums.0 >= 8 {
                    break;
                }
                if to_nums.1 < 0 || to_nums.1 >= 8 {
                    break;
                }

                let to = BoardPos::from_file_rank(
                    File::from_num(to_nums.0 as u8),
                    Rank::from_num(to_nums.1 as u8),
                );

                match state.get(to) {
                    Some((c, _)) if c == state.to_play => break,
                    Some((opp_c, piece)) => {
                        debug_assert!(opp_c == !state.to_play);
                        moves.push(Move {
                            from, to,
                            piece: Piece::Bishop,
                            capture_piece: Some(piece),
                            flags: MoveFlags::empty(),
                        });
                        break;
                    }
                    None => {
                        moves.push(Move {
                            from, to,
                            piece: Piece::Bishop,
                            capture_piece: None,
                            flags: MoveFlags::empty(),
                        });
                    }
                }
            }
        }
    }
}

const ROOK_DIRS: &[(i8, i8)] = &[
    (1, 0),
    (-1, 0),
    (0, 1),
    (0, -1),
];

fn rook_pseudo_legal(state: &State, moves: &mut Vec<Move>) {
    for from in state.bitboard(state.to_play, Piece::Rook).iter_all() {
        let from_nums = (from.file.to_num() as i8, from.rank.to_num() as i8);

        for dir in ROOK_DIRS {
            let mut to_nums = from_nums.clone();
            loop {
                to_nums.0 += dir.0;
                to_nums.1 += dir.1;

                if to_nums.0 < 0 || to_nums.0 >= 8 {
                    break;
                }
                if to_nums.1 < 0 || to_nums.1 >= 8 {
                    break;
                }

                let to = BoardPos::from_file_rank(
                    File::from_num(to_nums.0 as u8),
                    Rank::from_num(to_nums.1 as u8),
                );

                match state.get(to) {
                    Some((c, _)) if c == state.to_play => break,
                    Some((opp_c, piece)) => {
                        debug_assert!(opp_c == !state.to_play);
                        moves.push(Move {
                            from, to,
                            piece: Piece::Rook,
                            capture_piece: Some(piece),
                            flags: MoveFlags::empty(),
                        });
                        break;
                    }
                    None => {
                        moves.push(Move {
                            from, to,
                            piece: Piece::Rook,
                            capture_piece: None,
                            flags: MoveFlags::empty(),
                        });
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::fen::parse_fen;

    #[test]
    fn count_moves_from_starting() {
        let state = parse_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1")
            .expect("Expect test case FEN to be correct");

        let mut moves = Vec::new();
        all_pseudo_legal(&state, &mut moves);

        for m in moves.iter() {
            println!("{}", m.format_long_algebraic());
        }

        assert_eq!(moves.len(), 20);
    }
}