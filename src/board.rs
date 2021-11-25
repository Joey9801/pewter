use crate::{BitBoard, BoardPos, Color, Piece};

#[derive(Clone, Copy)]
pub struct Board {
    piece_boards: [BitBoard; Piece::VARIANT_COUNT],
    color_boards: [BitBoard; Color::VARIANT_COUNT],
}

impl Board {
    pub const fn new_empty() -> Self {
        Self {
            piece_boards: [BitBoard::new_empty(); Piece::VARIANT_COUNT],
            color_boards: [BitBoard::new_empty(); Color::VARIANT_COUNT],
        }
    }

    pub const fn piece_board(&self, piece: Piece) -> BitBoard {
        self.piece_boards[piece.to_num() as usize]
    }

    pub const fn color_board(&self, color: Color) -> BitBoard {
        self.color_boards[color.to_num() as usize]
    }

    pub const fn color_piece_board(&self, color: Color, piece: Piece) -> BitBoard {
        self.color_board(color)
            .intersect_with(self.piece_board(piece))
    }

    pub const fn all_union_board(&self) -> BitBoard {
        let w = self.color_board(Color::White);
        let b = self.color_board(Color::Black);
        w.union_with(b)
    }

    pub const fn king_pos(&self, color: Color) -> Option<BoardPos> {
        (self
            .piece_board(Piece::King)
            .intersect_with(self.color_board(color)))
        .first_set()
    }

    pub fn xor_inplace(&mut self, color: Color, piece: Piece, arg: BitBoard) {
        self.color_boards[color.to_num() as usize].xor_inplace(arg);
        self.piece_boards[piece.to_num() as usize].xor_inplace(arg);
    }

    pub fn union_inplace(&mut self, color: Color, piece: Piece, arg: BitBoard) {
        self.color_boards[color.to_num() as usize].union_inplace(arg);
        self.piece_boards[piece.to_num() as usize].union_inplace(arg);
    }

    pub fn intersect_inplace(&mut self, color: Color, piece: Piece, arg: BitBoard) {
        self.color_boards[color.to_num() as usize].intersect_inplace(arg);
        self.piece_boards[piece.to_num() as usize].intersect_inplace(arg);
    }
    
    pub fn set(&mut self, color: Color, piece: Piece, pos: BoardPos) {
        debug_assert_eq!(self.get(pos), None);
        self.union_inplace(color, piece, BitBoard::single(pos))
    }
    
    pub fn clear(&mut self, color: Color, piece: Piece, pos: BoardPos) {
        debug_assert_eq!(self.get(pos), Some((color, piece)));
        self.intersect_inplace(color, piece, !BitBoard::single(pos))
    }

    pub fn get(&self, pos: BoardPos) -> Option<(Color, Piece)> {
        let color = if self.color_board(Color::White)[pos] {
            Color::White
        } else if self.color_board(Color::Black)[pos] {
            Color::Black
        } else {
            return None;
        };

        // Linear search through the pieces.
        // Could potentially be more efficient to do a sort of binary search through the pieces,
        // where we check against checking increasingly specific unions.
        // TODO: Benchmark alternative implementations.
        let piece = Piece::iter_all()
            .filter(|p| self.piece_board(*p)[pos])
            .next()
            .expect("Board piece/color bitboards are inconsistent");

        Some((color, piece))
    }

    pub fn add_piece(&mut self, pos: BoardPos, color: Color, piece: Piece) {
        let mask = BitBoard::single(pos);

        // Assert that the square is not already occupied
        debug_assert!(!(self.all_union_board() & mask).any());

        self.xor_inplace(color, piece, mask);
    }

    pub fn sanity_check_board(&self) {
        // The individual piece boards should not overlap
        for a in Piece::iter_all() {
            for b in Piece::iter_all() {
                if a != b {
                    let a_board = self.piece_board(a);
                    let b_board = self.piece_board(b);
                    assert!(!a_board.intersect_with(b_board).any());
                }
            }
        }

        // The colors should not overlap
        let white_board = self.color_board(Color::White);
        let black_board = self.color_board(Color::Black);
        assert!(!white_board.intersect_with(black_board).any());

        // The union of all the color boards should equal the union of all the piece boards
        let color_union = white_board.union_with(black_board);
        let mut piece_union = BitBoard::new_empty();
        for piece in Piece::iter_all() {
            piece_union = piece_union.union_with(self.piece_board(piece));
        }

        assert_eq!(piece_union, color_union);
    }
}
