
use crate::{BoardPos, Color, File, Piece, Rank, State, StateFlags};

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
        _ => Err(FenParseError::InvalidPieceChar(symbol))
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
                let file = files.peek().ok_or(FenParseError::TooLargeRank)?;
                state.add_piece(color, piece, (*rank, **file).into())
            }
        }
    }

    todo!();
}

pub fn parse_fen(fen_str: &str) -> Result<State, FenParseError> {
    let mut state = State::new_empty();

    let mut fields = fen_str.split(" ");

    let placement_str = fields
        .next()
        .ok_or(FenParseError::MissingFields)?;
    parse_fen_placements(placement_str, &mut state)?;

    match fields.next().map(|s| s.chars().next()).flatten() {
        Some('w') => state.to_play = Color::White,
        Some('b') => state.to_play = Color::Black,
        Some(c) => return Err(FenParseError::InvalidColor(c)),
        None => return Err(FenParseError::MissingFields),
    }

    let castling_str = fields
        .next()
        .ok_or(FenParseError::MissingFields)?;
    for c in castling_str.chars() {
        match c {
            'K' => state.flags.insert(StateFlags::WHITE_CR_KS),
            'Q' => state.flags.insert(StateFlags::WHITE_CR_QS),
            'k' => state.flags.insert(StateFlags::BLACK_CR_KS),
            'q' => state.flags.insert(StateFlags::BLACK_CR_QS),
            '-' => (),
            _ => return Err(FenParseError::InvalidCastlingRightsChar(c))
        }
    }

    let en_passant_str = fields
        .next()
        .ok_or(FenParseError::MissingFields)?;
    state.en_passant_file = BoardPos::from_algebraic(en_passant_str)
        .map(|b| b.file);

    let halfmove_clock_str = fields
        .next()
        .ok_or(FenParseError::MissingFields)?;
    state.halfmove_clock = halfmove_clock_str.parse()
        .map_err(|_| FenParseError::InvalidNumber)?;
    

    let fullmove_counter_str = fields
        .next()
        .ok_or(FenParseError::MissingFields)?;
    state.fullmove_counter = fullmove_counter_str.parse()
        .map_err(|_| FenParseError::InvalidNumber)?;

    if fields.next().is_some() {
        Err(FenParseError::ExcessFields)
    } else {
        Ok(state)
    }
}

#[cfg(test)]
mod tests {
}