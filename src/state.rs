use bitflags::bitflags;

use crate::bitboard::masks;
use crate::coordinates::consts::*;
use crate::{BitBoard, Board, BoardPos, Color, File, Move, Piece, Rank};

bitflags! {
    pub struct CastleRights: u8 {
        const WHITE_KINGSIDE  = 0b0001;
        const WHITE_QUEENSIDE = 0b0010;
        const BLACK_KINGSIDE  = 0b0100;
        const BLACK_QUEENSIDE = 0b1000;

        const ALL_WHITE = Self::WHITE_KINGSIDE.bits | Self::WHITE_QUEENSIDE.bits;
        const ALL_BLACK = Self::BLACK_KINGSIDE.bits | Self::BLACK_QUEENSIDE.bits;
    }
}

pub struct State {
    pub to_play: Color,

    pub castle_rights: CastleRights,

    /// If the previous move was advancing a pawn two spaces, the position that the pawn skipped
    pub en_passant: Option<BoardPos>,

    /// Number of halfmoves since the last capture of pawn advance
    pub halfmove_clock: u8,

    // Number of full moves since game start. Incremented after each black move.
    pub fullmove_counter: u16,

    pub board: Board,
    pub pinned: BitBoard,
    pub checkers: BitBoard,
}

impl State {
    pub fn new_empty() -> Self {
        Self {
            to_play: Color::White,
            castle_rights: CastleRights::empty(),
            en_passant: None,
            halfmove_clock: 0,
            fullmove_counter: 0,
            board: Board::new_empty(),
            pinned: BitBoard::new_empty(),
            checkers: BitBoard::new_empty(),
        }
    }

    pub fn add_piece(&mut self, color: Color, piece: Piece, pos: BoardPos) {
        self.board.add_piece(pos, color, piece);
    }

    pub fn king_pos(&self, color: Color) -> BoardPos {
        self.board
            .king_pos(color)
            .expect("Expect valid game states to always have a king for each color")
    }

    fn set(&mut self, color: Color, piece: Piece, pos: BoardPos) {
        self.board
            .union_inplace(color, piece, BitBoard::single(pos));
    }

    fn clear(&mut self, color: Color, piece: Piece, pos: BoardPos) {
        self.board
            .intersect_inplace(color, piece, BitBoard::single(pos));
    }

    fn get(&self, pos: BoardPos) -> Option<(Color, Piece)> {
        self.board.get(pos)
    }

    fn set_pinners_and_checkers(&mut self) {
        self.pinned = BitBoard::new_empty();
        self.checkers = BitBoard::new_empty();

        let k_pos = self.king_pos(self.to_play);
        let color = self.board.color_board(!self.to_play);
        let queens = self.board.piece_board(Piece::Queen);

        let pinner_bishops = self
            .board
            .piece_board(Piece::Bishop)
            .union_with(queens)
            .intersect_with(color)
            .intersect_with(masks::bishop_rays(k_pos));

        let pinner_rooks = self
            .board
            .piece_board(Piece::Rook)
            .union_with(queens)
            .intersect_with(color)
            .intersect_with(masks::rook_rays(k_pos));

        let all_pinners = pinner_bishops.union_with(pinner_rooks);

        let union_board = self.board.all_union_board();
        for pos in all_pinners.iter_set() {
            let between = masks::between(pos, k_pos) & union_board;
            match between.count() {
                0 => self.checkers.set(pos),
                1 => self.pinned = self.pinned.union_with(between),
                _ => (),
            }
        }

        // TODO: Include knights and pawns in chekers
    }

    fn apply_castling(&mut self, m: Move) {
        let kingside = m.to.file == File::G;

        let required_flag = if self.to_play == Color::Black {
            if kingside {
                CastleRights::BLACK_KINGSIDE
            } else {
                CastleRights::BLACK_QUEENSIDE
            }
        } else {
            if kingside {
                CastleRights::WHITE_KINGSIDE
            } else {
                CastleRights::WHITE_QUEENSIDE
            }
        };
        debug_assert!(self.castle_rights.contains(required_flag));

        let from = match (self.to_play, kingside) {
            (Color::White, true) => H1,
            (Color::White, false) => A1,
            (Color::Black, true) => H8,
            (Color::Black, false) => A8,
        };
        let to = match (self.to_play, kingside) {
            (Color::White, true) => F1,
            (Color::White, false) => D1,
            (Color::Black, true) => F8,
            (Color::Black, false) => D8,
        };

        let op = BitBoard::single(from).union_with(BitBoard::single(to));
        self.board.xor_inplace(self.to_play, Piece::Rook, op);
    }

    fn update_castling_rights(&mut self, m: Move, piece: Piece, capture_piece: Option<Piece>) {
        if piece == Piece::King {
            // Moving the king removes all castling rights
            match self.to_play {
                Color::White => {
                    self.castle_rights.remove(CastleRights::ALL_WHITE);
                }
                Color::Black => {
                    self.castle_rights.remove(CastleRights::ALL_BLACK);
                }
            }
        }

        if piece == Piece::Rook {
            // Moving a rook removes castling rights on that side
            match self.to_play {
                Color::White => {
                    if m.from == A1 {
                        self.castle_rights.remove(CastleRights::WHITE_QUEENSIDE);
                    } else if m.from == H1 {
                        self.castle_rights.remove(CastleRights::WHITE_KINGSIDE);
                    }
                }
                Color::Black => {
                    if m.from == A8 {
                        self.castle_rights.remove(CastleRights::BLACK_QUEENSIDE);
                    } else if m.from == H8 {
                        self.castle_rights.remove(CastleRights::BLACK_KINGSIDE);
                    }
                }
            }
        }

        if let Some(Piece::Rook) = capture_piece {
            // Losing a rook means you can no longer castle on that side
            match !self.to_play {
                Color::White => {
                    if m.to == A1 {
                        self.castle_rights.remove(CastleRights::WHITE_QUEENSIDE);
                    } else if m.to == H1 {
                        self.castle_rights.remove(CastleRights::WHITE_KINGSIDE);
                    }
                }
                Color::Black => {
                    if m.to == A8 {
                        self.castle_rights.remove(CastleRights::BLACK_QUEENSIDE);
                    } else if m.to == H8 {
                        self.castle_rights.remove(CastleRights::BLACK_KINGSIDE);
                    }
                }
            }
        }
    }

    pub fn pretty_format(&self) -> String {
        use crate::io::ascii::pretty_format;

        fn sym(val: Option<(Color, Piece)>) -> char {
            if let Some((color, piece)) = val {
                match color {
                    Color::White => piece.to_char().to_ascii_uppercase(),
                    Color::Black => piece.to_char(),
                }
            } else {
                ' '
            }
        }

        pretty_format(|pos| sym(self.get(pos)))
    }

    /// Applies a move, panicking if the move doesn't fit.
    ///
    /// When panicking, may leave this object in an invalid state.
    pub fn apply_move(&mut self, m: Move) {
        let from_bb = BitBoard::single(m.from);
        let to_bb = BitBoard::single(m.to);
        let move_bb = from_bb | to_bb;

        let (_color, piece) = self.get(m.from).expect("No piece on square being moved");
        debug_assert!(_color == self.to_play);

        let capture_piece = self.get(m.to).map(|(c, p)| {
            debug_assert!(c == !self.to_play);
            p
        });

        let op = BitBoard::single(m.from).union_with(BitBoard::single(m.to));
        self.board.xor_inplace(self.to_play, piece, op);

        // Handle all regular captures, where the destination square was
        // previously occupied by the piece being captured
        if let Some(capture_piece) = capture_piece {
            self.clear(!self.to_play, capture_piece, m.to);
        }

        // Handle en-passant captures
        if self.en_passant == Some(m.to) {
            debug_assert!(piece == Piece::Pawn);

            // The pos that we expect to find the ep-capturable pawn
            let ep_pawn_pos = match !self.to_play {
                Color::White => BoardPos::from_file_rank(m.to.file, Rank::R4),
                Color::Black => BoardPos::from_file_rank(m.to.file, Rank::R5),
            };
            self.clear(!self.to_play, Piece::Pawn, ep_pawn_pos);
        }

        if move_bb & masks::castling_moves_all() == move_bb {
            self.apply_castling(m);
        }
        self.update_castling_rights(m, piece, capture_piece);

        // Update the en-passant capturable state
        if (piece == Piece::Pawn) && (move_bb & masks::double_pawn_moves() == move_bb) {
            let rank = match self.to_play {
                Color::White => Rank::R3,
                Color::Black => Rank::R6,
            };
            self.en_passant = Some(BoardPos::from_file_rank(m.from.file, rank));
        } else {
            self.en_passant = None;
        }

        if let Some(promotion) = m.promotion {
            debug_assert!(piece == Piece::Pawn);
            debug_assert!(m.to.rank == Rank::R1 || m.to.rank == Rank::R8);
            self.clear(self.to_play, Piece::Pawn, m.to);
            self.set(self.to_play, promotion, m.to);
        }

        if capture_piece.is_some() || piece == Piece::Pawn {
            self.halfmove_clock = 0;
        } else {
            self.halfmove_clock += 1;
        }

        self.set_pinners_and_checkers();

        if self.to_play == Color::Black {
            self.fullmove_counter += 1;
        }
        self.to_play = !self.to_play;
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use crate::chessmove::Move;
    use crate::color::Color;
    use crate::coordinates::proptest_helpers::*;
    use crate::io::fen::{format_fen, parse_fen};
    use crate::piece::Piece;

    use proptest::strategy::{Just, Strategy};
    use proptest::{prop_oneof, proptest};

    fn arb_color() -> impl Strategy<Value = Color> {
        prop_oneof![Just(Color::White), Just(Color::Black),]
    }

    fn arb_piece() -> impl Strategy<Value = Piece> {
        prop_oneof![
            Just(Piece::Pawn),
            Just(Piece::Rook),
            Just(Piece::Knight),
            Just(Piece::Bishop),
            Just(Piece::King),
            Just(Piece::Queen),
        ]
    }

    proptest! {
        #[test]
        fn test_piece_num_roundtrips(piece in arb_piece()) {
            let num = piece.to_num();
            let piece2 = Piece::from_num(num);
            assert_eq!(piece, piece2);
        }

        #[test]
        fn test_state_add_piece(
            color in arb_color(),
            piece in arb_piece(),
            pos in arb_boardpos()
        ) {
            let mut state = State::new_empty();

            assert!(!state.board.color_board(color).any());
            state.add_piece(color, piece, pos);

            let bb = state.board.color_piece_board(color, piece);
            assert_eq!(bb, state.board.color_board(color));
            assert_eq!(bb, state.board.all_union_board());
            assert_eq!(bb.count(), 1);
            assert!(bb.get(pos));

            let other_bb = state.board.color_piece_board(!color, piece);
            assert!(!other_bb.any());
        }
    }

    fn test_apply_move_helper(fen_start: &str, lan_move: &str, expected_fen_end: &str) {
        let mut state =
            parse_fen(fen_start).expect("Expected test case to have valid starting FEN string");
        let m = Move::from_long_algebraic(lan_move)
            .expect("Expected test case to have valid LAN move string");

        state.apply_move(m);

        assert_eq!(expected_fen_end, format_fen(&state));
    }

    #[test]
    fn test_apply_move_1() {
        test_apply_move_helper(
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
            "e2e4",
            "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1",
        )
    }

    #[test]
    fn test_apply_move_2() {
        test_apply_move_helper(
            "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1",
            "c7c5",
            "rnbqkbnr/pp1ppppp/8/2p5/4P3/8/PPPP1PPP/RNBQKBNR w KQkq c6 0 2",
        )
    }

    #[test]
    fn test_apply_move_3() {
        test_apply_move_helper(
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQK2R w KQkq - 0 1",
            "e1g1",
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQ1RK1 b kq - 1 1",
        )
    }
}
