use crate::coordinates::consts::*;
use crate::{BoardPos, CastleSide, Color, File, Rank};

use super::BitBoard;

const ALL_RANK_1: BitBoard = BitBoard(0x01_01_01_01_01_01_01_01);

const fn make_rank_bb(rank: Rank) -> BitBoard {
    BitBoard(ALL_FILE_A.0 << (rank.to_num() * 8))
}

const RANKS: [BitBoard; 8] = [
    make_rank_bb(Rank::R1),
    make_rank_bb(Rank::R2),
    make_rank_bb(Rank::R3),
    make_rank_bb(Rank::R4),
    make_rank_bb(Rank::R5),
    make_rank_bb(Rank::R6),
    make_rank_bb(Rank::R7),
    make_rank_bb(Rank::R8),
];

pub const fn rank(rank: Rank) -> BitBoard {
    RANKS[rank.to_num() as usize]
}

const ALL_FILE_A: BitBoard = BitBoard(0x00_00_00_00_00_00_00_FF);

const fn make_file_bb(file: File) -> BitBoard {
    BitBoard(ALL_RANK_1.0 << file.to_num())
}

const FILES: [BitBoard; 8] = [
    make_file_bb(File::A),
    make_file_bb(File::B),
    make_file_bb(File::C),
    make_file_bb(File::D),
    make_file_bb(File::E),
    make_file_bb(File::F),
    make_file_bb(File::G),
    make_file_bb(File::H),
];

pub const fn file(file: File) -> BitBoard {
    FILES[file.to_num() as usize]
}

pub const fn edges() -> BitBoard {
    BitBoard::new_empty()
        .union_with(rank(Rank::R1))
        .union_with(rank(Rank::R8))
        .union_with(file(File::A))
        .union_with(file(File::H))
}

// 0b1010_1010 == 0xAA
// 0b0101_0101 == 0x55
const LIGHT_SQUARES: BitBoard = BitBoard(0x55_AA_55_AA_55_AA_55_AA);

pub const fn color_squares(color: Color) -> BitBoard {
    match color {
        Color::White => LIGHT_SQUARES,
        Color::Black => LIGHT_SQUARES.inverse(),
    }
}

// Diagonals go in two directions
//    Where the file number - the rank number is constant (diagonals)
//       eg [F1, G2, H3]
//    Where the file number + the rank number is constant (antidiagonals)
//       Eg [A3, B2, C1]
//
// Label antidiagonals by the value of `filenum + ranknum` (values 0..=14)
// Label diagonals by the value of `filenum - ranknum + 21` (values 15..=29)
// Then can store the 30 diagonal mask bitboards in a single lookup table,
// indexed by those labels
const fn generate_diag_lookup() -> [BitBoard; 30] {
    let mut diag_table = [BitBoard::new_empty(); 30];
    let mut pos_int = 0u8;

    while pos_int < 64 {
        let pos = BoardPos::from_bitboard_offset(pos_int);
        let fnum = pos.file.to_num() as i32;
        let rnum = pos.rank.to_num() as i32;

        let d_idx = (fnum - rnum + 22) as usize;
        let ad_idx = (fnum + rnum) as usize;

        // NB: Can't use BitBoard::set() as the mutable receiver makes it
        // non-const
        diag_table[d_idx] = diag_table[d_idx].with_set(pos);
        diag_table[ad_idx] = diag_table[ad_idx].with_set(pos);

        pos_int += 1;
    }

    diag_table
}

const DIAG_LOOKUP: [BitBoard; 30] = generate_diag_lookup();

pub const fn diagonal(pos: BoardPos) -> BitBoard {
    DIAG_LOOKUP[(pos.file.to_num() + 22 - pos.rank.to_num()) as usize]
}

pub const fn antidiagonal(pos: BoardPos) -> BitBoard {
    DIAG_LOOKUP[(pos.file.to_num() + pos.rank.to_num()) as usize]
}

/// The squares a rook could move to if there were no other pieces on the board
///
/// Doesn't include the starting square
pub const fn rook_rays(pos: BoardPos) -> BitBoard {
    BitBoard::new_empty()
        .union_with(rank(pos.rank))
        .union_with(file(pos.file))
        .with_cleared(pos)
}

/// The squares a bishop could move to if there were no other pieces on the board
///
/// Doesn't include the starting square
pub const fn bishop_rays(pos: BoardPos) -> BitBoard {
    BitBoard::new_empty()
        .union_with(diagonal(pos))
        .union_with(antidiagonal(pos))
        .with_cleared(pos)
}

pub const fn queen_rays(pos: BoardPos) -> BitBoard {
    BitBoard::new_empty()
        .union_with(rook_rays(pos))
        .union_with(bishop_rays(pos))
}

/// All kingside castling moves
pub const fn castling_moves_kingside() -> BitBoard {
    BitBoard::new_empty()
        .with_set(E1)
        .with_set(G1)
        .with_set(E8)
        .with_set(G8)
}

/// All kingside castling moves
pub const fn castling_moves_queenside() -> BitBoard {
    BitBoard::new_empty()
        .with_set(E1)
        .with_set(C1)
        .with_set(E8)
        .with_set(C8)
}

/// The union of all castling moves
pub const fn castling_moves_all() -> BitBoard {
    BitBoard::new_empty()
        .union_with(castling_moves_kingside())
        .union_with(castling_moves_queenside())
}

/// The positions that are required to be empty in order to castle kingside
pub const fn castling_required_empty(color: Color, side: CastleSide) -> BitBoard {
    use CastleSide::*;
    use Color::*;

    match (color, side) {
        (White, Kingside) => between(E1, H1),
        (Black, Kingside) => between(E8, H8),
        (White, Queenside) => between(A1, E1),
        (Black, Queenside) => between(A8, E8),
    }
}

/// The positions that are required to be empty in order to castle kingside
pub const fn castling_required_not_check(color: Color, side: CastleSide) -> BitBoard {
    use CastleSide::*;
    use Color::*;

    match (color, side) {
        (White, Kingside) => between(E1, H1),
        (Black, Kingside) => between(E8, H8),
        (White, Queenside) => between(A1, E1),
        (Black, Queenside) => between(A8, E8),
    }
}

pub const fn double_pawn_moves(color: Color) -> BitBoard {
    match color {
        Color::White => BitBoard::new_empty()
            .union_with(rank(Rank::R2))
            .union_with(rank(Rank::R4)),
        Color::Black => BitBoard::new_empty()
            .union_with(rank(Rank::R7))
            .union_with(rank(Rank::R5)),
    }
}

/// An implementation of a line mask that computes it on the fly
///
/// The implementation is a little clunky to work around Rust const function
/// limitations, such that it can be used to populate lookup tables at compile
/// time.
const fn line_slow(a: BoardPos, b: BoardPos) -> BitBoard {
    if a.const_eq(&b) {
        BitBoard::new_empty()
    } else if a.file.to_num() == b.file.to_num() {
        file(a.file)
    } else if a.rank.to_num() == b.rank.to_num() {
        rank(a.rank)
    } else {
        let a_diag = diagonal(a);
        let a_antidiag = antidiagonal(a);
        let b_diag = diagonal(b);
        let b_antidiag = antidiagonal(b);

        if a_diag.const_eq(b_diag) {
            a_diag
        } else if a_antidiag.const_eq(b_antidiag) {
            a_antidiag
        } else {
            BitBoard::new_empty()
        }
    }
}

const fn compute_line_table() -> [[BitBoard; 64]; 64] {
    let mut table = [[BitBoard::new_empty(); 64]; 64];

    let mut source = 0u8;
    while source < 64 {
        let mut dest = 0u8;
        let a = BoardPos::from_bitboard_offset(source);
        while dest < 64 {
            let b = BoardPos::from_bitboard_offset(dest);

            table[source as usize][dest as usize] = line_slow(a, b);

            dest += 1;
        }
        source += 1;
    }

    table
}

static LINE_TABLE: [[BitBoard; 64]; 64] = compute_line_table();

/// Fast lookup table based for line masks.
///
/// Returns an empty mask if the two positions do not share a mask.
pub fn line(a: BoardPos, b: BoardPos) -> BitBoard {
    let a = a.to_bitboard_offset() as usize;
    let b = b.to_bitboard_offset() as usize;
    LINE_TABLE[a][b]
}

/// Compute the between BitBoard on the fly
const fn between_slow(a: BoardPos, b: BoardPos) -> BitBoard {
    const fn int_between(a: i8, b: i8, test: i8) -> bool {
        if a < b {
            a < test && test < b
        } else {
            b < test && test < a
        }
    }

    if a.const_eq(&b) {
        return BitBoard::new_empty();
    }

    let mut bb = line_slow(a, b);

    if a.rank.to_num() == b.rank.to_num() {
        let a = a.file.to_num() as i8;
        let b = b.file.to_num() as i8;

        let mut f = 0u8;
        while f < 8 {
            if !int_between(a, b, f as i8) {
                bb = bb.intersect_with(file(File::from_num(f)).inverse())
            }
            f += 1;
        }
    } else {
        let a = a.rank.to_num() as i8;
        let b = b.rank.to_num() as i8;

        let mut r = 0u8;
        while r < 8 {
            if !int_between(a, b, r as i8) {
                bb = bb.intersect_with(rank(Rank::from_num(r)).inverse())
            }
            r += 1;
        }
    }

    bb
}

// Precompute between(source, dest) for all pairs of squares.
// Written in a slightly strange way to keep it const-compatible.
const fn compute_between_table() -> [[BitBoard; 64]; 64] {
    let mut table = [[BitBoard::new_empty(); 64]; 64];

    let mut source = 0u8;
    while source < 64 {
        let mut dest = 0u8;
        let a = BoardPos::from_bitboard_offset(source);
        while dest < 64 {
            let b = BoardPos::from_bitboard_offset(dest);

            table[source as usize][dest as usize] = between_slow(a, b);

            dest += 1;
        }
        source += 1;
    }

    table
}

const BETWEEN_TABLE: [[BitBoard; 64]; 64] = compute_between_table();

pub const fn between(a: BoardPos, b: BoardPos) -> BitBoard {
    let a = a.to_bitboard_offset() as usize;
    let b = b.to_bitboard_offset() as usize;
    BETWEEN_TABLE[a][b]
}


const fn compute_pawn_pushes() -> [[BitBoard; 64]; 2] {
    let mut table = [[BitBoard::new_empty(); 64]; 2];

    // NB: Pawns cannot exist on the first/last rank
    let mut ranknum = 1;
    while ranknum < 7 {
        let rank = Rank::from_num(ranknum);
        let mut filenum = 0;
        while filenum < 8 {
            let file = File::from_num(filenum);
            let pos = BoardPos::from_file_rank(file, rank);
            
            let mut white_table = BitBoard::new_empty();
            let white_single = BoardPos::from_file_rank(
                file, Rank::from_num(ranknum + 1),
            );
            white_table = white_table.with_set(white_single);
            if ranknum == 1 {
                let white_double = BoardPos::from_file_rank(
                    file, Rank::from_num(ranknum + 2),
                );
                white_table = white_table.with_set(white_double);
            }
            
            let mut black_table = BitBoard::new_empty();
            let black_single = BoardPos::from_file_rank(
                file, Rank::from_num(ranknum - 1),
            );
            black_table = black_table.with_set(black_single);
            if ranknum == 6 {
                let black_double = BoardPos::from_file_rank(
                    file, Rank::from_num(ranknum - 2),
                );
                black_table = black_table.with_set(black_double);
            }

            table[Color::White.to_num() as usize][pos.to_bitboard_offset() as usize] = white_table;
            table[Color::Black.to_num() as usize][pos.to_bitboard_offset() as usize] = black_table;
            filenum += 1;
        }
        ranknum += 1;
    }
    
    table
}

const PAWN_PUSHES: [[BitBoard; 64]; 2] = compute_pawn_pushes();

pub const fn pawn_pushes(color: Color, pos: BoardPos) -> BitBoard {
    PAWN_PUSHES[color.to_num() as usize][pos.to_bitboard_offset() as usize]
}

const fn compute_pawn_attacks() -> [[BitBoard; 64]; 2] {
    let mut table = [[BitBoard::new_empty(); 64]; 2];

    // NB: Pawns cannot exist on the first/last rank, but fill out the masks for those positions
    // anyway, as they are sometimes used "backwards" to test for check.
    let mut ranknum = 0;
    while ranknum < 8 {
        let rank = Rank::from_num(ranknum);
        let mut filenum = 0;
        while filenum < 8 {
            let file = File::from_num(filenum);
            let pos = BoardPos::from_file_rank(file, rank);
            
            if ranknum < 7 {
                let mut white_table = BitBoard::new_empty();
                let white_ahead = Rank::from_num(ranknum + 1);
                if filenum > 0 {
                    let white_left = BoardPos::from_file_rank(
                        File::from_num(filenum - 1), white_ahead,
                    );
                    white_table = white_table.with_set(white_left);
                }
                if filenum < 7 {
                    let white_right = BoardPos::from_file_rank(
                        File::from_num(filenum + 1), white_ahead,
                    );
                    white_table = white_table.with_set(white_right);
                }
                table[Color::White.to_num() as usize][pos.to_bitboard_offset() as usize] = white_table;
            }
            
            if ranknum > 0 {
                let mut black_table = BitBoard::new_empty();
                let black_ahead = Rank::from_num(ranknum - 1);
                if filenum > 0 {
                    let black_left = BoardPos::from_file_rank(
                        File::from_num(filenum - 1), black_ahead,
                    );
                    black_table = black_table.with_set(black_left);
                }
                if filenum < 7 {
                    let black_right = BoardPos::from_file_rank(
                        File::from_num(filenum + 1), black_ahead,
                    );
                    black_table = black_table.with_set(black_right);
                }
                table[Color::Black.to_num() as usize][pos.to_bitboard_offset() as usize] = black_table;
            }

            filenum += 1;
        }
        ranknum += 1;
    }
    
    table
}

const PAWN_ATTACKS: [[BitBoard; 64]; 2] = compute_pawn_attacks();

pub const fn pawn_attacks(color: Color, pos: BoardPos) -> BitBoard {
    PAWN_ATTACKS[color.to_num() as usize][pos.to_bitboard_offset() as usize]
}


const KNIGHT_MOVE_OFFSETS: [(i8, i8); 8] = [
    (2, -1),
    (2, 1),
    (1, -2),
    (1, 2),
    (-1, -2),
    (-1, 2),
    (-2, -1),
    (-2, 1),
];

const fn compute_knight_moves() -> [BitBoard; 64] {
    let mut table = [BitBoard::new_empty(); 64];

    let mut source = 0;
    while source < 64 {
        let source_pos = BoardPos::from_bitboard_offset(source);

        let mut moves = BitBoard::new_empty();
        let source_f = source_pos.file.to_num() as i8;
        let source_r = source_pos.rank.to_num() as i8;
        
        let mut move_idx = 0;
        while move_idx < 8 {
            let dest_f = source_f + KNIGHT_MOVE_OFFSETS[move_idx].0;
            let dest_r = source_r + KNIGHT_MOVE_OFFSETS[move_idx].1;
            
            if dest_f >= 0 && dest_f <= 7 && dest_r >= 0 && dest_r <= 7 {
                let dest = BoardPos::from_file_rank(
                    File::from_num(dest_f as u8),
                    Rank::from_num(dest_r as u8)
                );
                
                moves = moves.with_set(dest);
            }

            move_idx += 1;
        }
        
        table[source as usize] = moves;
        source += 1;
    }

    table
}

const KNIGHT_MOVES: [BitBoard; 64] = compute_knight_moves();

pub const fn knight_moves(pos: BoardPos) -> BitBoard {
    KNIGHT_MOVES[pos.to_bitboard_offset() as usize]
}