use thiserror::Error;

use crate::{State, Move, Piece, Color, coordinates::consts::*, BoardPos, movegen::legal_moves, File, Rank, io::fen::parse_fen, state::GameResult};

pub struct Game {
    pub initial: State,
    pub moves: Vec<Move>,
    pub result: GameResult,
}

#[derive(Error, Debug)]
pub enum PgnParseError {
    #[error("A SAN encoded move could not be decoded")]
    BadMoveString,

    #[error("A SAN encoded move could have referred to multiple legal moves")]
    AmbiguousMove,

    #[error("A SAN encoded move was not legal in the current board state")]
    IllegalMove,
    
    #[error("This PGN parser does not have the ability to parse PGN files that contain comments")]
    Comments,
}

fn parse_san_move(state: &State, move_str: &str) -> Result<Move, PgnParseError> {
    if !move_str.is_ascii() || move_str.len() < 2 {
        return Err(PgnParseError::BadMoveString);
    }
    
    let mut move_str = move_str.as_bytes();

    // Strip redundant metadata that this move results in check/checkmate
    if move_str[move_str.len() - 1] == b'+' || move_str[move_str.len() - 1] == b'#' {
        move_str = &move_str[..(move_str.len() - 1)];
    }

    if move_str.len() < 2 {
        return Err(PgnParseError::BadMoveString);
    }
    
    // Parse and strip the promotion detail off the end
    let promotion = match move_str[move_str.len() - 1] {
        b'Q' => Some(Piece::Queen),
        b'R' => Some(Piece::Rook),
        b'B' => Some(Piece::Bishop),
        b'N' => Some(Piece::Knight),
        _ => None,
    };
    if promotion.is_some() {
        if move_str[move_str.len() - 2] != b'=' {
            return Err(PgnParseError::BadMoveString);
        }
        move_str = &move_str[..(move_str.len() - 2)];
    }

    if move_str.len() < 2 {
        return Err(PgnParseError::BadMoveString);
    }
    
    let dest_pos;
    let mut capture = false;
    let mut piece = None;
    let mut from_file = None;
    let mut from_rank = None;

    if move_str == b"O-O" || move_str == b"O-O-O" {
        // castling move
        dest_pos = match (state.to_play, move_str) {
            (Color::White, b"O-O") => G1,
            (Color::White, b"O-O-O") => C1,
            (Color::Black, b"O-O") => G8,
            (Color::Black, b"O-O-O") => C8,
            _ => unreachable!(),
        };
        piece = Some(Piece::King);
        from_file = Some(File::E);
        from_rank = match state.to_play {
            Color::White => Some(Rank::R1),
            Color::Black => Some(Rank::R8),
        };
    } else {
        // The last two characters should now be the destination square
        let dest_str = std::str::from_utf8(&move_str[(move_str.len() - 2)..])
            .unwrap(); // Already asserted the string is ascii above
        dest_pos = BoardPos::from_algebraic(dest_str)
            .ok_or(PgnParseError::BadMoveString)?;
        move_str = &move_str[..(move_str.len() - 2)];
        
        for b in move_str {
            match b {
                b'1'..=b'8' if from_rank.is_none() => from_rank = Some(Rank::from_num(b - b'1')),
                b'a'..=b'h' if from_file.is_none() => from_file = Some(File::from_num(b - b'a')),
                b'x' if !capture => capture = true,
                b'Q' if piece.is_none() => piece = Some(Piece::Queen),
                b'R' if piece.is_none() => piece = Some(Piece::Rook),
                b'B' if piece.is_none() => piece = Some(Piece::Bishop),
                b'N' if piece.is_none() => piece = Some(Piece::Knight),
                b'K' if piece.is_none() => piece = Some(Piece::King),
                _ => return Err(PgnParseError::BadMoveString),
            }
        }
    }
    let piece = piece.unwrap_or(Piece::Pawn);

    // Now we have parsed all that we can out of the move string, use what we have learned to
    // filter the list of legal moves in this position.
    //
    // If no moves match the filters, the SAN was an illegal move.
    // If multiple moves match the filters, the SAN was ambiguous

    let legal_moves = legal_moves(state);
    let mut candidate_moves = legal_moves
        .iter()
        .filter(|m| m.to == dest_pos)
        .filter(|m| state.board.get(m.from) == Some((state.to_play, piece)))
        .filter(|m| from_file.map(|f| m.from.file == f).unwrap_or(true))
        .filter(|m| from_rank.map(|r| m.from.rank == r).unwrap_or(true));
        
    let m = candidate_moves.next().ok_or(PgnParseError::IllegalMove)?;
    if candidate_moves.next().is_some() {
        Err(PgnParseError::AmbiguousMove)
    } else {
        Ok(m)
    }
}

/// Strips prefixes that match the pattern `[0-9]\.`.
/// 
///  - Turn a string of the form "13.Nxd3" into "Nxd3"
///  - Leaves strings without a move number prefix untouched
///  - Leaves results strings (eg "0-1", "1-", "1/2-1/2", etc..) untouched
fn strip_move_number(token: &str) -> &str {
    assert!(token.is_ascii());
    
    let number_count = token
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .count();
    
    if number_count > 0 {
        match token[number_count..].chars().next() {
            Some('.') => &token[(number_count + 1)..],
            _ => token,
        }
    } else {
        token
    }
}

pub fn parse_single_pgn(pgn_str: &str) -> Result<Game, PgnParseError> {
    // Make the following assumptions:
    //   - All games being parsed start from the normal starting position
    //   - There are no comments (';', '{', or '}' chars)

    let starting = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
    let mut state = parse_fen(starting).unwrap();
    let initial_state = state.clone();
    let mut moves = Vec::new();
    let mut result = GameResult::Ongoing;
    for line in pgn_str.lines() {
        if line.starts_with('[') {
            continue;
        }
        
        if line.len() == 0 {
            continue;
        }

        if line.contains('{') || line.contains('{') {
            return Err(PgnParseError::Comments);
        }
        
        for token in line.split_ascii_whitespace() {
            if token.len() == 0 {
                continue;
            }

            let token = strip_move_number(token);
            let m = parse_san_move(&state, token)?;
            state = state.apply_move(m);
            moves.push(m);

            result = state.game_result();
            if result != GameResult::Ongoing {
                break;
            }
        }
    }

    Ok(Game {
        initial: initial_state,
        moves,
        result,
    })
}

#[cfg(test)]
mod tests {
    use crate::io::fen::parse_fen;

    use super::*;

    #[test]
    fn test_parse_san() {
        // Taken from a random PGN file; Carlsen vs Brameld, 2001/01/06
        let moves = &[
            "e4", "Nf6", "e5", "Nd5", "d4", "d6", "Nf3", "Bg4", "Bc4", "e6", "O-O", "Nb6", "Be2",
            "Be7", "h3", "Bh5", "Bf4", "Nc6", "c3", "O-O", "Nbd2", "d5", "b4", "a5", "a3", "Qd7",
            "Qc2", "Bg6", "Bd3", "Rfc8", "Rfb1", "Bf8", "h4", "Ne7", "g3", "Qa4", "Ne1", "Qxc2",
            "Bxc2", "Bxc2", "Nxc2", "Na4", "Rb3", "b6", "Kf1", "c5", "bxc5", "bxc5", "dxc5",
            "Rxc5", "Nb1", "Rac8", "Be3", "Rc4", "Bd4", "Nc6", "Rb5", "Nxd4", "Nxd4", "Nxc3",
            "Nxc3", "Rxd4", "Ne2", "Ra4", "Ke1", "Rxa3", "Rab1", "Bb4+", "Kf1", "Rd3",
        ];
        
        let starting = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
        let mut state = parse_fen(starting).unwrap();
        for move_str in moves {
            let m = parse_san_move(&state, move_str);

            if m.is_err() {
                println!("{}", state.pretty_format());
                dbg!(move_str);
            }

            let m = m.expect("Expected test case to contain valid SAN moves");
            
            state = state.apply_move(m);
        }
    }
}