use thiserror::Error;

use crate::{
    coordinates::consts::*, io::fen::parse_fen, movegen::legal_moves, state::GameResult, BoardPos,
    Color, File, Move, Piece, Rank, State,
};

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

    #[error("There were non-ascii characters in the PGN file")]
    NonAscii,
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

    let dest_pos;
    let mut capture = false;
    let mut piece = None;
    let mut from_file = None;
    let mut from_rank = None;

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
        piece = Some(Piece::Pawn);
    }

    if move_str.len() < 2 {
        return Err(PgnParseError::BadMoveString);
    }

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
        let dest_str = std::str::from_utf8(&move_str[(move_str.len() - 2)..]).unwrap(); // Already asserted the string is ascii above
        dest_pos = BoardPos::from_algebraic(dest_str).ok_or(PgnParseError::BadMoveString)?;
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
        .filter(|m| from_rank.map(|r| m.from.rank == r).unwrap_or(true))
        .filter(|m| m.promotion == promotion);

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

    let number_count = token.chars().take_while(|c| c.is_ascii_digit()).count();

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

    if !pgn_str.is_ascii() {
        return Err(PgnParseError::NonAscii);
    }

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
            let token = strip_move_number(token);

            if token.len() == 0 {
                continue;
            }

            let m = match parse_san_move(&state, token) {
                Ok(m) => m,
                Err(PgnParseError::BadMoveString) => {
                    match token {
                        "1-0" | "1-" | "1" => result = GameResult::WhiteWin,
                        "0-1" | "0-" | "0" => result = GameResult::BlackWin,
                        "1/2-1/2" | "1/2-" | "1/2" => result = GameResult::Draw,
                        _ => return Err(PgnParseError::BadMoveString),
                    }
                    break;
                }
                Err(PgnParseError::AmbiguousMove) => {
                    println!("{}", state.pretty_format());
                    dbg!(token);
                    return Err(PgnParseError::AmbiguousMove);
                }
                Err(e) => return Err(e),
            };
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

pub fn parse_multi_pgn(
    multi_pgn_str: &str,
) -> Result<Vec<Result<Game, PgnParseError>>, PgnParseError> {
    if !multi_pgn_str.is_ascii() {
        return Err(PgnParseError::NonAscii);
    }

    let mut games = Vec::new();

    let mut this_pgn_start = 0;
    let mut this_pgn_end = 0;
    let mut in_tags = false;
    let mut line_start = 0;

    while line_start < multi_pgn_str.len() {
        let line_end = match multi_pgn_str[line_start..].find('\n') {
            Some(idx) => idx + line_start,
            None => multi_pgn_str.len() - 1,
        };
        let line = &multi_pgn_str[line_start..(line_end + 1)];

        if line.as_bytes()[0] == b'[' {
            if !in_tags {
                let last_pgn = &multi_pgn_str[this_pgn_start..this_pgn_end];
                if last_pgn.len() > 0 {
                    games.push(parse_single_pgn(last_pgn));
                }
                in_tags = true;
                this_pgn_start = line_start;
            }
        } else {
            in_tags = false;
        }

        this_pgn_end = line_end;
        line_start = line_end + 1;
    }

    let last_pgn = &multi_pgn_str[this_pgn_start..];
    if last_pgn.len() > 0 {
        games.push(parse_single_pgn(last_pgn));
    }

    Ok(games)
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

    const EXAMPLE_PGN: &'static str = r#"[Event "Superbet Classic 2021"]
[Site "Bucharest ROU"]
[Date "2021.06.05"]
[Round "1.5"]
[White "Deac,Bogdan-Daniel"]
[Black "Giri,A"]
[Result "1/2-1/2"]
[WhiteElo "2627"]
[BlackElo "2780"]
[ECO "D43"]

1.d4 d5 2.c4 c6 3.Nc3 Nf6 4.Nf3 e6 5.Bg5 h6 6.Bh4 dxc4 7.e4 g5 8.Bg3 b5 9.Be2 Bb7
10.Qc2 Nh5 11.Rd1 Nxg3 12.hxg3 Na6 13.a3 Bg7 14.e5 Qe7 15.Ne4 O-O-O 16.Nd6+ Rxd6
17.exd6 Qxd6 18.O-O g4 19.Ne5 Bxe5 20.dxe5 Qxe5 21.Bxg4 h5 22.Rfe1 Qf6 23.Bf3 h4
24.b3 cxb3 25.Qxb3 hxg3 26.fxg3 Qg7 27.Qd3 Nc7 28.Qd6 c5 29.Qd7+ Kb8 30.Bxb7 Kxb7
31.Rxe6 Qxg3 32.Qc6+ Kb8 33.Qd6 Qxd6 34.Rexd6 Kb7 35.Rf6 Rh7 36.Rd7 b4 37.axb4 cxb4
38.Kf2 a5 39.Ke2 Rg7 40.Rfxf7 Rxg2+ 41.Kd1 Rg1+ 42.Kc2 Rg2+ 43.Kb1 Rg1+ 44.Kb2 Rg2+
45.Kb1 Rg1+ 46.Kb2 Rg2+ 47.Kb1 Rg1+  1/2-1/2"#;

    #[test]
    fn test_parse_single_pgn() {
        let game =
            parse_single_pgn(EXAMPLE_PGN).expect("Expected EXAMPLE_PGN to parse successfully");

        assert_eq!(game.moves.len(), 94);
        assert_eq!(game.moves[0].format_long_algebraic(), "d2d4");
        assert_eq!(game.moves[93].format_long_algebraic(), "g2g1");
    }

    const EXAMPLE_MULTI_PGN: &'static str = r#"[Event "Superbet Classic 2021"]
[Site "Bucharest ROU"]
[Date "2021.06.05"]
[Round "1.5"]
[White "Deac,Bogdan-Daniel"]
[Black "Giri,A"]
[Result "1/2-1/2"]
[WhiteElo "2627"]
[BlackElo "2780"]
[ECO "D43"]

1.d4 d5 2.c4 c6 3.Nc3 Nf6 4.Nf3 e6 5.Bg5 h6 6.Bh4 dxc4 7.e4 g5 8.Bg3 b5 9.Be2 Bb7
10.Qc2 Nh5 11.Rd1 Nxg3 12.hxg3 Na6 13.a3 Bg7 14.e5 Qe7 15.Ne4 O-O-O 16.Nd6+ Rxd6
17.exd6 Qxd6 18.O-O g4 19.Ne5 Bxe5 20.dxe5 Qxe5 21.Bxg4 h5 22.Rfe1 Qf6 23.Bf3 h4
24.b3 cxb3 25.Qxb3 hxg3 26.fxg3 Qg7 27.Qd3 Nc7 28.Qd6 c5 29.Qd7+ Kb8 30.Bxb7 Kxb7
31.Rxe6 Qxg3 32.Qc6+ Kb8 33.Qd6 Qxd6 34.Rexd6 Kb7 35.Rf6 Rh7 36.Rd7 b4 37.axb4 cxb4
38.Kf2 a5 39.Ke2 Rg7 40.Rfxf7 Rxg2+ 41.Kd1 Rg1+ 42.Kc2 Rg2+ 43.Kb1 Rg1+ 44.Kb2 Rg2+
45.Kb1 0-1

[Event "Superbet Classic 2021"]
[Site "Bucharest ROU"]
[Date "2021.06.07"]
[Round "3.5"]
[White "Lupulescu,C"]
[Black "Giri,A"]
[Result "1-0"]
[WhiteElo "2656"]
[BlackElo "2780"]
[ECO "A28"]

1.c4 e5 2.Nc3 Nf6 3.Nf3 Nc6 4.e3 Bb4 5.Qc2 Bxc3 6.Qxc3 Qe7 7.d4 Ne4 8.Qd3 exd4
9.Nxd4 Nc5 10.Qd1 Nxd4 11.Qxd4 O-O 12.Be2 b6 13.O-O Bb7 14.f3 a5 15.Bd2 f5
16.Rad1 d6 17.b3 Rae8 18.Rf2 Rf6 19.Bd3 Qf7 20.Re2 Qh5 21.Be1 Be4 22.Bb1 Rg6
23.Bg3 Bxb1 24.Rxb1 Ne4 25.Rbe1 Nxg3 26.Qd5+ Kh8 27.hxg3 Rxg3 28.e4 Qxf3
29.Qf7 Rg8 30.exf5 Qc6 31.Rf2 Qc5 32.Re7 Qd4 33.Re8 Rxg2+ 34.Kxg2 Qg4+ 35.Kh2 Qh4+
36.Kg2 Qg4+ 37.Kf1 Qh3+ 38.Ke1 Qc3+ 39.Rd2  1-0"#;

    #[test]
    fn test_parse_multi_pgn() {
        let games = parse_multi_pgn(EXAMPLE_MULTI_PGN)
            .expect("Expected EXAMPLE_MULTI_PGN to parse successfully");

        assert_eq!(games.len(), 2);

        let g0 = games[0].as_ref().expect("Expected first game to parse");

        assert_eq!(g0.moves.len(), 89);
        assert_eq!(g0.result, GameResult::BlackWin);
        assert_eq!(g0.moves[0].format_long_algebraic(), "d2d4");
        assert_eq!(g0.moves[88].format_long_algebraic(), "b2b1");

        let g1 = games[1].as_ref().expect("Expected second game to parse");

        assert_eq!(g1.moves.len(), 77);
        assert_eq!(g1.result, GameResult::WhiteWin);
        assert_eq!(g1.moves[0].format_long_algebraic(), "c2c4");
        assert_eq!(g1.moves[76].format_long_algebraic(), "f2d2");
    }
}
