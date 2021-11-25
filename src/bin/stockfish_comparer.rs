use std::{path::Path, process::{Command, Stdio, Child}, io::{BufReader, BufRead}, collections::HashSet};
use std::io::{Write};

use pewter::{io::fen::{parse_fen, format_fen}, Move, State};

fn parse_stockfish_perft_line(line: &str) -> (Move, usize) {
    debug_assert!(line.is_ascii());
    
    let split = line.find(':')
        .expect("Expected line to have a separating colon");

    let long_algebraic_str = &line[..split];
    let m = Move::from_long_algebraic(long_algebraic_str)
        .expect("Expected to be able to parse a move formatted by stockfish");

    let count: usize = line[(split + 2)..]
        .parse::<usize>()
        .expect("Expected to able to parse a number");
    
    (m, count)
}

struct StockfishInterface {
    child: Child
}

impl StockfishInterface {
    fn launch(exe: &Path) -> Self {
        let mut child = Command::new(exe)
            .stdout(Stdio::piped())
            .stdin(Stdio::piped())
            .spawn()
            .expect("Failed to launch stockfish subprocess");
        
        // Consume the init line that stockfish emits on startup
        let stdout = child
            .stdout
            .as_mut()
            .expect("Expected stockfish handle to have a stdout");
        let mut stdout = BufReader::new(stdout);

        let mut line = String::new();
        stdout.read_line(&mut line).unwrap();

        Self {
            child
        }
    }
    
    fn set_state(&mut self, state: State) {
        let stdin = self.child
            .stdin
            .as_mut()
            .expect("Expected stockfish handle to have a stdin");
        
        let fen_str = format_fen(&state);
        write!(stdin, "position fen {}\n", fen_str)
            .expect("Failed to write to stockfish stdin");
    }
    
    fn perft(&mut self, state: State, depth: u8) -> Vec<(Move, usize)> {
        self.set_state(state);
        
        let stdin = self.child
            .stdin
            .as_mut()
            .expect("Expected stockfish handle to have a stdin");
        
        write!(stdin, "go perft {}\n", depth)
            .expect("Failed to write to stockfish stdin");

        let stdout = self.child
            .stdout
            .as_mut()
            .expect("Expected stockfish handle to have a stdout");
        let lines = BufReader::new(stdout).lines();

        let mut output = Vec::new();
        let mut done = 0;
        for line in lines {
            let line = line.unwrap();
            if line.len() == 0 {
                done += 1
            }
            
            if done == 0 {
                output.push(parse_stockfish_perft_line(&line));
            } else if done == 2 {
                break;
            }
        }
        
        output
    }
}

enum MoveDifference {
    /// We generated a move that stockfish did not
    ExtraMove(Move),

    /// Stockfish generated a move that we did not
    MissingMove(Move),
}

struct Difference {
    position: State,
    move_difference: MoveDifference,
}

enum PerftComparison {
    Equal,
    MoveDiff(MoveDifference),
    SubtreeSizeDiff(Move)
}

fn compare_perft_outputs(mut a: Vec<(Move, usize)>, mut b: Vec<(Move, usize)>) -> PerftComparison  {
    a.sort_by_key(|(m, _count)| *m);
    b.sort_by_key(|(m, _count)| *m);

    let a_set = a.iter().map(|(m, _)| m).collect::<HashSet<_>>();
    let b_set = b.iter().map(|(m, _)| m).collect::<HashSet<_>>();
    match a_set.symmetric_difference(&b_set).next() {
        Some(m) => {
            if a_set.contains(m) {
                return PerftComparison::MoveDiff(MoveDifference::ExtraMove(**m))
            }

            if b_set.contains(m) {
                return PerftComparison::MoveDiff(MoveDifference::MissingMove(**m))
            }
        }
        None => (),
    }
    
    for ((m, a_count), (_m, b_count)) in a.iter().zip(b.iter()) {
        if a_count != b_count {
            return PerftComparison::SubtreeSizeDiff(*m);
        }
    }
    
    PerftComparison::Equal
}

fn find_minimal_difference(initial_state: State, mut sf: StockfishInterface, max_depth: u8) -> Option<Difference> {
    let mut depth = 1;
    let mut state = initial_state;
    loop {
        if depth > max_depth {
            break None;
        }

        let ours = pewter::movegen::perft_breakdown(state, depth);
        let stockfish = sf.perft(state, depth);
        
        match compare_perft_outputs(ours, stockfish) {
            PerftComparison::Equal => {
                println!("No differences found at depth {}", depth);
                depth += 1;
            }
            PerftComparison::MoveDiff(md) => break Some(Difference {
                position: state,
                move_difference: md
            }),
            PerftComparison::SubtreeSizeDiff(m) => {
                println!("Found difference after making {}, refining...", m);
                state = state.apply_move(m);
                state.board.sanity_check_board();
                depth -= 1;
            }
        }
    }
}

fn main() {
    let mut args = std::env::args();

    // First element in args is just our own exe path
    args.next();

    let stockfish_exe = args.next()
        .expect("Expected second argument to be the path to a stockfish exe");

    let stockfish_exe = Path::new(&stockfish_exe);
    if !stockfish_exe.exists() {
        println!("Cannot find given stockfish executable");
        return;
    }
    let sf = StockfishInterface::launch(stockfish_exe);

    let fen_str = args.next()
        .expect("Expected first argument to be a starting FEN string");
    let initial_state = parse_fen(&fen_str)
        .expect("Expected a valid FEN string for the first argument");
    
    if let Some(diff) = find_minimal_difference(initial_state, sf, 10) {
        println!("At the following position:");
        println!("fen: \"{}\"", format_fen(&diff.position));
        println!("{}", diff.position.pretty_format());
        
        match diff.move_difference {
            MoveDifference::ExtraMove(m) => println!("Pewter emitted the move \"{}\" while Stockfish did not", m),
            MoveDifference::MissingMove(m) => println!("Stockfish emitted the move \"{}\" while Pewter did not", m),
        }
        
        println!("Assorted state information:");
        println!("En-passant = {:?}", diff.position.en_passant);
        println!("Pinned:");
        println!("{}", diff.position.pinned.pretty_format());
        println!("Checkers:");
        println!("{}", diff.position.checkers.pretty_format());
    }
}