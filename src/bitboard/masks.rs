use crate::{Color, coordinates::{BoardPos, File, Rank}};

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
//       eg F1, G2, H3
//    Where the file number + the rank number is constant (antidiagonals)
//       Eg A3, B2, C1
//
// Label antidiagonals by the value of `filenum + ranknum` (values 0..=14)
// Label diagonals by the value of `filenum - ranknum + 21` (values 15..=29)
// Then can store the 30 diagonal mask bitboards in a single lookup table
const DIAG_LOOKUP: [BitBoard; 30] = [
    BitBoard(0x00_00_00_00_00_00_00_01),
    BitBoard(0x00_00_00_00_00_00_01_02),
    BitBoard(0x00_00_00_00_00_01_02_04),
    BitBoard(0x00_00_00_00_01_02_04_08),
    BitBoard(0x00_00_00_01_02_04_08_10),
    BitBoard(0x00_00_01_02_04_08_10_20),
    BitBoard(0x00_01_02_04_08_10_20_40),
    BitBoard(0x01_02_04_08_10_20_40_80),
    BitBoard(0x02_04_08_10_20_40_80_00),
    BitBoard(0x04_08_10_20_40_80_00_00),
    BitBoard(0x08_10_20_40_80_00_00_00),
    BitBoard(0x10_20_40_80_00_00_00_00),
    BitBoard(0x20_40_80_00_00_00_00_00),
    BitBoard(0x40_80_00_00_00_00_00_00),
    BitBoard(0x80_00_00_00_00_00_00_00),
    BitBoard(0x01_00_00_00_00_00_00_00),
    BitBoard(0x02_01_00_00_00_00_00_00),
    BitBoard(0x04_02_01_00_00_00_00_00),
    BitBoard(0x08_04_02_01_00_00_00_00),
    BitBoard(0x10_08_04_02_01_00_00_00),
    BitBoard(0x20_10_08_04_02_01_00_00),
    BitBoard(0x40_20_10_08_04_02_01_00),
    BitBoard(0x80_40_20_10_08_04_02_01),
    BitBoard(0x00_80_40_20_10_08_04_02),
    BitBoard(0x00_00_80_40_20_10_08_04),
    BitBoard(0x00_00_00_80_40_20_10_08),
    BitBoard(0x00_00_00_00_80_40_20_10),
    BitBoard(0x00_00_00_00_00_80_40_20),
    BitBoard(0x00_00_00_00_00_00_80_40),
    BitBoard(0x00_00_00_00_00_00_00_80),
];

enum DiagonalType {
    Diagonal,
    AntiDiagonal,
}

const fn diag_lookup_idx(pos: BoardPos, diag_type: DiagonalType) -> usize {
    let offset = match diag_type {
        DiagonalType::Diagonal => 22,
        DiagonalType::AntiDiagonal => 0,
    };

    let ranknum_mul = match diag_type {
        DiagonalType::Diagonal => -1,
        DiagonalType::AntiDiagonal => 1,
    };

    (pos.file.to_num() as i32 + (ranknum_mul * pos.rank.to_num() as i32) + offset) as usize
}

pub const fn diagonal(pos: BoardPos) -> BitBoard {
    DIAG_LOOKUP[(pos.file.to_num() + 22 - pos.rank.to_num()) as usize]
}

pub const fn antidiagonal(pos: BoardPos) -> BitBoard {
    DIAG_LOOKUP[(pos.file.to_num() + pos.rank.to_num()) as usize]
}

pub const fn rook_rays(pos: BoardPos) -> BitBoard {
    BitBoard::new_empty()
        .union_with(rank(pos.rank))
        .union_with(file(pos.file))
        .intersect_with(BitBoard::from_pos(pos).inverse())
}

pub const fn bishop_rays(pos: BoardPos) -> BitBoard {
    BitBoard::new_empty()
        .union_with(diagonal(pos))
        .union_with(antidiagonal(pos))
        .intersect_with(BitBoard::from_pos(pos).inverse())
}

pub const fn queen_rays(pos: BoardPos) -> BitBoard {
    rook_rays(pos).union_with(bishop_rays(pos))
}

#[cfg(test)]
mod test {
    use super::*;

    // The code used to generate DIAG_LOOKUP above. Run explicitly with:
    //     cargo test -- --include-ignored bitboard::masks::test::dummy_generate_diagonals
    #[test]
    #[ignore]
    fn dummy_generate_diagonals() {
        let mut diag_table = [BitBoard::new_empty(); 30];

        for pos in BoardPos::iter_all() {
            diag_table[diag_lookup_idx(pos, DiagonalType::Diagonal)].set(pos);
            diag_table[diag_lookup_idx(pos, DiagonalType::AntiDiagonal)].set(pos);
        }

        for bb in diag_table.iter() {
            println!("    BitBoard({:#018x}),", bb.0)
        }

        assert!(false);
    }
}