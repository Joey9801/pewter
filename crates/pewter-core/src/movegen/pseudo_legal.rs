use crate::{
    chessmove::MoveSetChunk, BitBoard, BoardPos, Color, File, MoveSet, Piece, Rank, State, bitboard::masks,
};

pub fn all_pseudo_legal(state: &State) -> MoveSet {
    let mut ms = MoveSet::new_empty();

    for piece in Piece::all() {
        for pos in state
            .board
            .color_piece_board(state.to_play, piece)
            .iter_set()
        {
            let chunk = pseudo_legal_moves(state, piece, pos);
            ms.push(chunk);
        }
    }

    ms
}

/// All pseudo-legal moves for the given piece at the given location
///
/// Does not include the following special moves:
///    - En-passant pawn captures
///    - Castling
/// As both of these types of move have more in depth legality checking, and are handled specially
/// in the full legal move generator.
pub fn pseudo_legal_moves(state: &State, piece: Piece, source: BoardPos) -> MoveSetChunk {
    let color = state.to_play;
    let our_pieces = state.board.color_board(color);
    let opp_pieces = state.board.color_board(!color);

    match piece {
        Piece::Pawn => pawn_psuedo_legal(color, source, our_pieces, opp_pieces),
        Piece::Knight => knight_pseudo_legal(source, our_pieces),
        Piece::Rook => sliding_piece_pseudo_legal(source, our_pieces, opp_pieces, &ROOK_DIRS),
        Piece::Bishop => sliding_piece_pseudo_legal(source, our_pieces, opp_pieces, &BISHOP_DIRS),
        Piece::King => king_pseudo_legal(source, our_pieces),
        Piece::Queen => {
            let r = sliding_piece_pseudo_legal(source, our_pieces, opp_pieces, &ROOK_DIRS);
            let b = sliding_piece_pseudo_legal(source, our_pieces, opp_pieces, &BISHOP_DIRS);
            MoveSetChunk {
                source,
                dest_set: r.dest_set.union_with(b.dest_set),
                promotion: false,
            }
        }
    }
}

fn pawn_psuedo_legal(
    color: Color,
    source: BoardPos,
    our_pieces: BitBoard,
    opp_pieces: BitBoard,
) -> MoveSetChunk {
    let all_union = our_pieces.union_with(opp_pieces);

    let pushes = if source.rank == color.numbered_rank(2) &&
        all_union.get(BoardPos::from_file_rank(source.file, color.numbered_rank(3)))
    {
        // Can't jump over a piece with a double push
        BitBoard::new_empty()
    } else {
        masks::pawn_pushes(color, source).intersect_with(all_union.inverse())
    };
    
    let attacks = masks::pawn_attacks(color, source)
        .intersect_with(opp_pieces);

    let dest_set = pushes.union_with(attacks);
    let promotion = source.rank == color.numbered_rank(7);
    MoveSetChunk {
        source,
        dest_set,
        promotion,
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

fn knight_pseudo_legal(source: BoardPos, our_pieces: BitBoard) -> MoveSetChunk {
    MoveSetChunk {
        source,
        dest_set: knight_moves(source, our_pieces),
        promotion: false,
    }
}

pub fn knight_moves(source: BoardPos, blockers: BitBoard) -> BitBoard {
    let mut moves = BitBoard::new_empty();
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

        moves.set(dest);
    }

    moves.intersect_inplace(!blockers);
    moves
}

fn sliding_piece_pseudo_legal(
    source: BoardPos,
    our_pieces: BitBoard,
    opp_pieces: BitBoard,
    dirs: &[(i8, i8)],
) -> MoveSetChunk {
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

            let dest_mask = BitBoard::single(dest);
            if our_pieces.intersect_with(dest_mask).any() {
                break;
            }

            chunk.dest_set.set(dest);
            if opp_pieces.intersect_with(dest_mask).any() {
                break;
            }
        }
    }

    chunk
}

const BISHOP_DIRS: [(i8, i8); 4] = [(-1, -1), (-1, 1), (1, -1), (1, 1)];
const ROOK_DIRS: [(i8, i8); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];

const KING_MOVE_OFFSETS: &[(i8, i8)] = &[
    (-1, 1),
    (0, 1),
    (1, 1),
    (-1, -1),
    (0, -1),
    (1, -1),
    (-1, 0),
    (1, 0),
];

fn king_pseudo_legal(source: BoardPos, our_pieces: BitBoard) -> MoveSetChunk {
    MoveSetChunk {
        source,
        dest_set: king_moves(source, our_pieces),
        promotion: false,
    }
}

pub fn king_moves(source: BoardPos, our_pieces: BitBoard) -> BitBoard {
    let mut moves = BitBoard::new_empty();

    let source_nums = (source.file.to_num() as i8, source.rank.to_num() as i8);
    for dest_nums in KING_MOVE_OFFSETS
        .iter()
        .map(|km_off| (source_nums.0 + km_off.0, source_nums.1 + km_off.1))
        .filter(|(x, _)| *x >= 0 && *x <= 7)
        .filter(|(_, y)| *y >= 0 && *y <= 7)
    {
        let dest = BoardPos::from_file_rank(
            File::from_num(dest_nums.0 as u8),
            Rank::from_num(dest_nums.1 as u8),
        );

        moves.set(dest);
    }

    moves.intersect_inplace(!our_pieces);
    moves
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
