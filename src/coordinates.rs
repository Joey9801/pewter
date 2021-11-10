#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rank {
    R1,
    R2,
    R3,
    R4,
    R5,
    R6,
    R7,
    R8,
}

impl Rank {
    pub const fn to_num(&self) -> u8 {
        match self {
            Rank::R1 => 0,
            Rank::R2 => 1,
            Rank::R3 => 2,
            Rank::R4 => 3,
            Rank::R5 => 4,
            Rank::R6 => 5,
            Rank::R7 => 6,
            Rank::R8 => 7,
        }
    }

    pub const fn from_num(num: u8) -> Self {
        match num {
            0 => Rank::R1,
            1 => Rank::R2,
            2 => Rank::R3,
            3 => Rank::R4,
            4 => Rank::R5,
            5 => Rank::R6,
            6 => Rank::R7,
            7 => Rank::R8,
            _ => panic!("Invalid Rank number"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum File {
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
}

impl File {
    pub const fn to_num(&self) -> u8 {
        match self {
            File::A => 0,
            File::B => 1,
            File::C => 2,
            File::D => 3,
            File::E => 4,
            File::F => 5,
            File::G => 6,
            File::H => 7,
        }
    }

    pub const fn from_num(num: u8) -> Self {
        match num {
            0 => File::A,
            1 => File::B,
            2 => File::C,
            3 => File::D,
            4 => File::E,
            5 => File::F,
            6 => File::G,
            7 => File::H,
            _ => panic!("Invalid File number"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BoardPos {
    pub rank: Rank,
    pub file: File,
}

impl BoardPos {
    pub const fn to_bitboard_offset(&self) -> u8 {
        self.rank.to_num() * 8 + self.file.to_num()
    }

    pub const fn from_bitboard_offset(offset: u8) -> Self {
        Self {
            rank: Rank::from_num(offset / 8),
            file: File::from_num(offset % 8),
        }
    }

    pub const fn from_file_rank(file: File, rank: Rank) -> Self {
        Self { rank, file }
    }

    pub fn from_algebraic(pos: &str) -> Option<Self> {
        let mut c = pos.chars();
        let file = match c.next() {
            Some('a') => File::A,
            Some('b') => File::B,
            Some('c') => File::C,
            Some('d') => File::D,
            Some('e') => File::E,
            Some('f') => File::F,
            Some('g') => File::G,
            Some('h') => File::H,
            Some(_) | None => return None,
        };
        let rank = match c.next() {
            Some('1') => Rank::R1,
            Some('2') => Rank::R2,
            Some('3') => Rank::R3,
            Some('4') => Rank::R4,
            Some('5') => Rank::R5,
            Some('6') => Rank::R6,
            Some('7') => Rank::R7,
            Some('8') => Rank::R8,
            Some(_) | None => return None,
        };

        Some(Self { rank, file })
    }
}

impl From<(Rank, File)> for BoardPos {
    fn from((rank, file): (Rank, File)) -> Self {
        Self { rank, file }
    }
}

pub mod consts {
    use super::Rank::*;
    use super::File::*;
    use super::BoardPos;

    pub const A1: BoardPos = BoardPos::from_file_rank(A, R1);
    pub const B1: BoardPos = BoardPos::from_file_rank(B, R1);
    pub const C1: BoardPos = BoardPos::from_file_rank(C, R1);
    pub const D1: BoardPos = BoardPos::from_file_rank(D, R1);
    pub const E1: BoardPos = BoardPos::from_file_rank(E, R1);
    pub const F1: BoardPos = BoardPos::from_file_rank(F, R1);
    pub const G1: BoardPos = BoardPos::from_file_rank(G, R1);
    pub const H1: BoardPos = BoardPos::from_file_rank(H, R1);
    pub const A2: BoardPos = BoardPos::from_file_rank(A, R2);
    pub const B2: BoardPos = BoardPos::from_file_rank(B, R2);
    pub const C2: BoardPos = BoardPos::from_file_rank(C, R2);
    pub const D2: BoardPos = BoardPos::from_file_rank(D, R2);
    pub const E2: BoardPos = BoardPos::from_file_rank(E, R2);
    pub const F2: BoardPos = BoardPos::from_file_rank(F, R2);
    pub const G2: BoardPos = BoardPos::from_file_rank(G, R2);
    pub const H2: BoardPos = BoardPos::from_file_rank(H, R2);
    pub const A3: BoardPos = BoardPos::from_file_rank(A, R3);
    pub const B3: BoardPos = BoardPos::from_file_rank(B, R3);
    pub const C3: BoardPos = BoardPos::from_file_rank(C, R3);
    pub const D3: BoardPos = BoardPos::from_file_rank(D, R3);
    pub const E3: BoardPos = BoardPos::from_file_rank(E, R3);
    pub const F3: BoardPos = BoardPos::from_file_rank(F, R3);
    pub const G3: BoardPos = BoardPos::from_file_rank(G, R3);
    pub const H3: BoardPos = BoardPos::from_file_rank(H, R3);
    pub const A4: BoardPos = BoardPos::from_file_rank(A, R4);
    pub const B4: BoardPos = BoardPos::from_file_rank(B, R4);
    pub const C4: BoardPos = BoardPos::from_file_rank(C, R4);
    pub const D4: BoardPos = BoardPos::from_file_rank(D, R4);
    pub const E4: BoardPos = BoardPos::from_file_rank(E, R4);
    pub const F4: BoardPos = BoardPos::from_file_rank(F, R4);
    pub const G4: BoardPos = BoardPos::from_file_rank(G, R4);
    pub const H4: BoardPos = BoardPos::from_file_rank(H, R4);
    pub const A5: BoardPos = BoardPos::from_file_rank(A, R5);
    pub const B5: BoardPos = BoardPos::from_file_rank(B, R5);
    pub const C5: BoardPos = BoardPos::from_file_rank(C, R5);
    pub const D5: BoardPos = BoardPos::from_file_rank(D, R5);
    pub const E5: BoardPos = BoardPos::from_file_rank(E, R5);
    pub const F5: BoardPos = BoardPos::from_file_rank(F, R5);
    pub const G5: BoardPos = BoardPos::from_file_rank(G, R5);
    pub const H5: BoardPos = BoardPos::from_file_rank(H, R5);
    pub const A6: BoardPos = BoardPos::from_file_rank(A, R6);
    pub const B6: BoardPos = BoardPos::from_file_rank(B, R6);
    pub const C6: BoardPos = BoardPos::from_file_rank(C, R6);
    pub const D6: BoardPos = BoardPos::from_file_rank(D, R6);
    pub const E6: BoardPos = BoardPos::from_file_rank(E, R6);
    pub const F6: BoardPos = BoardPos::from_file_rank(F, R6);
    pub const G6: BoardPos = BoardPos::from_file_rank(G, R6);
    pub const H6: BoardPos = BoardPos::from_file_rank(H, R6);
    pub const A7: BoardPos = BoardPos::from_file_rank(A, R7);
    pub const B7: BoardPos = BoardPos::from_file_rank(B, R7);
    pub const C7: BoardPos = BoardPos::from_file_rank(C, R7);
    pub const D7: BoardPos = BoardPos::from_file_rank(D, R7);
    pub const E7: BoardPos = BoardPos::from_file_rank(E, R7);
    pub const F7: BoardPos = BoardPos::from_file_rank(F, R7);
    pub const G7: BoardPos = BoardPos::from_file_rank(G, R7);
    pub const H7: BoardPos = BoardPos::from_file_rank(H, R7);
    pub const A8: BoardPos = BoardPos::from_file_rank(A, R8);
    pub const B8: BoardPos = BoardPos::from_file_rank(B, R8);
    pub const C8: BoardPos = BoardPos::from_file_rank(C, R8);
    pub const D8: BoardPos = BoardPos::from_file_rank(D, R8);
    pub const E8: BoardPos = BoardPos::from_file_rank(E, R8);
    pub const F8: BoardPos = BoardPos::from_file_rank(F, R8);
    pub const G8: BoardPos = BoardPos::from_file_rank(G, R8);
    pub const H8: BoardPos = BoardPos::from_file_rank(H, R8);
}

#[cfg(test)]
pub mod proptest_helpers {
    use super::*;

    use proptest::strategy::{Just, Strategy};
    use proptest::{ prop_oneof, prop_compose};

    pub fn arb_file() -> impl Strategy<Value=File> {
        prop_oneof![
            Just(File::A),
            Just(File::B),
            Just(File::C),
            Just(File::D),
            Just(File::E),
            Just(File::F),
            Just(File::G),
            Just(File::H),
        ]
    }

    pub fn arb_rank() -> impl Strategy<Value=Rank> {
        prop_oneof![
            Just(Rank::R1),
            Just(Rank::R2),
            Just(Rank::R3),
            Just(Rank::R4),
            Just(Rank::R5),
            Just(Rank::R6),
            Just(Rank::R7),
            Just(Rank::R8),
        ]
    }

    prop_compose! {
        pub fn arb_boardpos()(file in arb_file(), rank in arb_rank()) -> BoardPos {
            BoardPos { file, rank }
        }
    }

}

#[cfg(test)]
mod test {
    use super::*;
    use super::proptest_helpers::*;

    use proptest::proptest;

    proptest! {
        #[test]
        fn test_rank_num_roundtrips(rank in arb_rank()) {
            let num = rank.to_num();
            let rank2 = Rank::from_num(num);
            assert_eq!(rank, rank2);
        }

        #[test]
        fn test_file_num_roundtrips(file in arb_file()) {
            let num = file.to_num();
            let file2 = File::from_num(num);
            assert_eq!(file, file2);
        }

        #[test]
        fn test_boardpos_bitoffset_roundtrips(pos in arb_boardpos()) {
            let offset = pos.to_bitboard_offset();
            let pos2 = BoardPos::from_bitboard_offset(offset);
            assert_eq!(pos, pos2);
        }

        #[test]
        fn test_boardpos_bitoffset_in_range(pos in arb_boardpos()) {
            assert!(pos.to_bitboard_offset() <= 63);
        }
    }
}