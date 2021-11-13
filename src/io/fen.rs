use crate::{BoardPos, CastleRights, File, Piece, Rank, State, color::Color};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FenParseError {
    MissingFields,

    ExcessFields,

    InvalidPieceChar(char),

    /// A single rank has more than 8 files
    TooLargeRank,

    /// The "next to move" field was not 'w' or 'b'
    InvalidColor(char),

    /// A castling rights char was not one of 'kqKQ-'
    InvalidCastlingRightsChar(char),

    /// There was an invalid number in the halfmove/fullmove counters
    InvalidNumber,
}

fn format_piece_symbol(color: Color, piece: Piece) -> char {
    match (color, piece) {
        (Color::Black, Piece::Pawn) => 'p',
        (Color::Black, Piece::Rook) => 'r',
        (Color::Black, Piece::Knight) => 'n',
        (Color::Black, Piece::Bishop) => 'b',
        (Color::Black, Piece::King) => 'k',
        (Color::Black, Piece::Queen) => 'q',
        (Color::White, Piece::Pawn) => 'P',
        (Color::White, Piece::Rook) => 'R',
        (Color::White, Piece::Knight) => 'N',
        (Color::White, Piece::Bishop) => 'B',
        (Color::White, Piece::King) => 'K',
        (Color::White, Piece::Queen) => 'Q',
    }
}

fn parse_piece_symbol(symbol: char) -> Result<(Color, Piece), FenParseError> {
    match symbol {
        'p' => Ok((Color::Black, Piece::Pawn)),
        'r' => Ok((Color::Black, Piece::Rook)),
        'n' => Ok((Color::Black, Piece::Knight)),
        'b' => Ok((Color::Black, Piece::Bishop)),
        'k' => Ok((Color::Black, Piece::King)),
        'q' => Ok((Color::Black, Piece::Queen)),
        'P' => Ok((Color::White, Piece::Pawn)),
        'R' => Ok((Color::White, Piece::Rook)),
        'N' => Ok((Color::White, Piece::Knight)),
        'B' => Ok((Color::White, Piece::Bishop)),
        'K' => Ok((Color::White, Piece::King)),
        'Q' => Ok((Color::White, Piece::Queen)),
        _ => Err(FenParseError::InvalidPieceChar(symbol)),
    }
}

const FEN_RANKS: [Rank; 8] = [
    Rank::R8,
    Rank::R7,
    Rank::R6,
    Rank::R5,
    Rank::R4,
    Rank::R3,
    Rank::R2,
    Rank::R1,
];

const FEN_FILES: [File; 8] = [
    File::A,
    File::B,
    File::C,
    File::D,
    File::E,
    File::F,
    File::G,
    File::H,
];

fn parse_fen_placements(placement_str: &str, state: &mut State) -> Result<(), FenParseError> {
    for (rank, rank_str) in FEN_RANKS.iter().zip(placement_str.splitn(8, "/")) {
        let mut files = FEN_FILES.iter().peekable();
        for sym in rank_str.chars() {
            if sym.is_ascii_digit() {
                for _ in 0..(sym.to_digit(10).unwrap()) {
                    files.next();
                }
            } else {
                let (color, piece) = parse_piece_symbol(sym)?;
                let file = files.next().ok_or(FenParseError::TooLargeRank)?;
                state.add_piece(color, piece, (*rank, *file).into())
            }
        }
    }

    Ok(())
}

pub fn parse_fen(fen_str: &str) -> Result<State, FenParseError> {
    let mut state = State::new_empty();

    let mut fields = fen_str.split(" ");

    let placement_str = fields.next().ok_or(FenParseError::MissingFields)?;
    parse_fen_placements(placement_str, &mut state)?;

    match fields.next().map(|s| s.chars().next()).flatten() {
        Some('w') => state.to_play = Color::White,
        Some('b') => state.to_play = Color::Black,
        Some(c) => return Err(FenParseError::InvalidColor(c)),
        None => return Err(FenParseError::MissingFields),
    }

    let castling_str = fields.next().ok_or(FenParseError::MissingFields)?;
    for c in castling_str.chars() {
        match c {
            'K' => state.castle_rights.insert(CastleRights::WHITE_KINGSIDE),
            'Q' => state.castle_rights.insert(CastleRights::WHITE_QUEENSIDE),
            'k' => state.castle_rights.insert(CastleRights::BLACK_KINGSIDE),
            'q' => state.castle_rights.insert(CastleRights::BLACK_QUEENSIDE),
            '-' => (),
            _ => return Err(FenParseError::InvalidCastlingRightsChar(c)),
        }
    }

    let en_passant_str = fields.next().ok_or(FenParseError::MissingFields)?;
    state.en_passant = BoardPos::from_algebraic(en_passant_str);

    let halfmove_clock_str = fields.next().ok_or(FenParseError::MissingFields)?;
    state.halfmove_clock = halfmove_clock_str
        .parse()
        .map_err(|_| FenParseError::InvalidNumber)?;

    let fullmove_counter_str = fields.next().ok_or(FenParseError::MissingFields)?;
    state.fullmove_counter = fullmove_counter_str
        .parse()
        .map_err(|_| FenParseError::InvalidNumber)?;

    if fields.next().is_some() {
        Err(FenParseError::ExcessFields)
    } else {
        Ok(state)
    }
}

fn format_fen_positions(state: &State, out: &mut String) {
    for rank in FEN_RANKS.iter() {
        let mut empty_squares = 0;

        for file in FEN_FILES.iter() {
            match state.get(BoardPos::from_file_rank(*file, *rank)) {
                Some((color, piece)) => {
                    if empty_squares > 0 {
                        out.push_str(&format!("{}", empty_squares));
                        empty_squares = 0;
                    }
                    out.push_str(&format!("{}", format_piece_symbol(color, piece)));
                }
                None => empty_squares += 1,
            };
        }

        if empty_squares > 0 {
            out.push_str(&format!("{}", empty_squares));
        }
        if *rank != FEN_RANKS[FEN_RANKS.len() - 1] {
            out.push_str("/");
        }
    }
}

pub fn format_fen(state: &State) -> String {
    // Should be more than enough for the largest possible FEN string
    let mut out = String::with_capacity(128);

    format_fen_positions(state, &mut out);

    match state.to_play {
        Color::White => out.push_str(" w "),
        Color::Black => out.push_str(" b "),
    }

    if state.castle_rights.contains(CastleRights::WHITE_KINGSIDE) {
        out.push('K');
    }
    if state.castle_rights.contains(CastleRights::WHITE_QUEENSIDE) {
        out.push('Q');
    }
    if state.castle_rights.contains(CastleRights::BLACK_KINGSIDE) {
        out.push('k');
    }
    if state.castle_rights.contains(CastleRights::BLACK_QUEENSIDE) {
        out.push('q');
    }

    if state.castle_rights.is_empty() {
        out.push('-');
    }

    if let Some(ep) = state.en_passant {
        out.push_str(&format!(" {}", ep.to_algebraic()));
    } else {
        out.push_str(" -");
    }

    out.push_str(&format!(
        " {} {}",
        state.halfmove_clock, state.fullmove_counter
    ));

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const FEN_EXAMPLES: [&'static str; 1] =
        ["rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"];

    #[test]
    fn test_fen_parse_roundtrips() {
        for example_fen_str in FEN_EXAMPLES.iter() {
            dbg!(example_fen_str);
            let state = parse_fen(example_fen_str).expect("Expected example FEN string to parse");
            let roundtripped_fen_str = format_fen(&state);
            assert_eq!(example_fen_str, &roundtripped_fen_str);
        }
    }
}
