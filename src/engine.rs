use std::time::{Duration, Instant};

use crate::{movegen::legal_moves, Color, Move, Piece, State};

use anyhow::Result;
use crossbeam_channel::{Receiver, Sender, unbounded, SendError};
use rand::{seq::SliceRandom, thread_rng};
use thiserror::Error;

#[derive(Clone, Copy, Debug)]
pub struct Timings {
    /// The amount of time the white player has remaining, or None if infinite time
    pub white_remaining: Option<Duration>,

    /// The amount of time the black player has remaining, or None if infinite time
    pub black_remaining: Option<Duration>,

    /// The amount of extra time white will get after making the next move
    pub white_increment: Duration,

    /// The amount of extra time black will get after making the next move
    pub black_increment: Duration,
}

#[derive(Clone, Copy, Debug)]
pub enum EngineCommand {
    SetState(State),

    /// Updates the engine's internal view of
    UpdateTimings(Timings),

    BeginSearch {
        /// Ignore all time controls, and compute until the Stop command is received
        infinite: bool,

        /// Automatically stop after searching this deep
        max_depth: Option<u8>,

        /// Automatically stop after searching this many nodes
        max_nodes: Option<u64>,
    },

    /// Immediately stop the in-progress search
    ///
    /// It is an error to send any message other than StopSearch while a search is in progress
    StopSearch,
}

#[derive(Clone, Debug)]
pub enum EngineMessage {
    /// Assorted information about the recent mechanical performance of the engine
    PerfInfo {
        /// Value between 0 and 1 representing how full the transposition table is
        transposition_load: f32,
        
        /// The number of nodes that have been visited during the current search.
        nodes: u64,

        /// The number of nodes searched per second since the start of the current search.
        nodes_per_second: f32,

        /// This many positions found in the endgame tablebases
        table_hits: u64,

        /// This many positions found in teh shredder endgame databases
        shredder_hits: u64,
    },

    /// The result of a search/evaluatiopn
    ///
    /// The search/evaluation should be considered ongoing until the message is seen.
    BestMove {
        best_move: Move,
        ponder_move: Option<Move>,
    },
    
    Error(EngineError),
}

#[derive(Clone, Error, Debug)]
pub enum EngineError {
    #[error("Tried to compute something before being given a state")]
    NoState,

    #[error("Asked for a best move, but no legal moves exist")]
    NoMoves,
    
    #[error("Failed to emit engine message")]
    SendError,
}

impl<T> From<SendError<T>> for EngineError {
    fn from(_: SendError<T>) -> Self {
        EngineError::SendError
    }
}

pub struct Engine {
    board_state: Option<State>,
    cmd_rx: Receiver<EngineCommand>,
    msg_tx: Sender<EngineMessage>,
}

impl Engine {
    pub fn startup() -> Result<(Sender<EngineCommand>, Receiver<EngineMessage>)> {
        let (cmd_tx, cmd_rx) = unbounded();
        let (msg_tx, msg_rx) = unbounded();

        std::thread::Builder::new()
            .name("Engine main".to_string())
            .spawn(move || {
                let mut engine = Self {
                    board_state: None,
                    cmd_rx,
                    msg_tx,
                };

                loop {
                    match engine.main_loop() {
                        Ok(()) => break,
                        Err(EngineError::SendError) => {
                            log::error!("Failed to send message from engine to main thread. Exiting.");
                            break;
                        },
                        Err(other) => {
                            if engine.msg_tx.send(EngineMessage::Error(other)).is_err() {
                                log::error!("Failed to send message from engine to main thread. Exiting.");
                                break;
                            }
                        }
                    }
                }
            })?;

        Ok((cmd_tx, msg_rx))
    }

    fn main_loop(&mut self) -> Result<(), EngineError> {
        for cmd in &self.cmd_rx {
            log::debug!("Engine command {:?}", cmd);
            match cmd {
                EngineCommand::SetState(s) => self.board_state = Some(s),
                EngineCommand::UpdateTimings(_t) => (),
                EngineCommand::StopSearch => (),
                EngineCommand::BeginSearch { .. } => {
                    let sw = Instant::now();
                    let best_move = self.search_best_move()?;
                    let elapsed = sw.elapsed();
                    log::debug!("Spent {:?} searching for best move, decided on {}", elapsed, best_move);
                    self.msg_tx.send(EngineMessage::BestMove {
                        best_move, ponder_move: None,
                    })?;
                },
            }
        }
        
        Ok(())
    }

    fn search_best_move(&self) -> Result<Move, EngineError> {
        let state = &self.board_state.ok_or(EngineError::NoState)?;

        let mut alpha = eval_consts::NEG_INFINITY;
        let mut best_move = None;
        let mut moves = legal_moves(state).iter().collect::<Vec<_>>();
        let mut nodes_searched = 0;
        moves.shuffle(&mut thread_rng());
        for m in moves {
            let new_state = state.apply_move(m);
            let score = -alpha_beta_search(&new_state, 5, eval_consts::NEG_INFINITY, -alpha, &mut nodes_searched);

            if best_move.is_none() || score > alpha {
                alpha = score;
                best_move = Some(m);
            }
        }
        
        self.msg_tx.send(EngineMessage::PerfInfo {
            transposition_load: 0.0,
            nodes: nodes_searched,
            nodes_per_second: 0.0,
            table_hits: 0,
            shredder_hits: 0,
        })?;

        best_move.ok_or(EngineError::NoMoves)
    }
}


mod eval_consts {
    use crate::Piece;
    
    pub const POS_INFINITY: i32 = i32::MAX - 1024;
    pub const NEG_INFINITY: i32 = i32::MIN + 1024;

    pub const MATE: i32 = NEG_INFINITY / 2;

    pub const PIECE_VALUES: [i32; Piece::VARIANT_COUNT] = [
        100,    // Pawn
        525,    // Rook
        350,    // Knight
        350,    // Bishop
        0,      // King
        1000,   // Queen
    ];
}

/// The linear difference in material, in centipawns
fn material_diff(state: &State) -> i32 {
    Piece::iter_all()
        .map(|p| {
            let value = eval_consts::PIECE_VALUES[p.to_num() as usize];
            let wb = state.board.color_piece_board(Color::White, p);
            let bb = state.board.color_piece_board(Color::Black, p);
            (wb.count() as i32 - bb.count() as i32) * value
        })
        .sum()
}

fn alpha_beta_search(state: &State, ply_depth: u8, mut alpha: i32, beta: i32, nodes_searched: &mut u64) -> i32 {
    // alpha => minimum score the maximizing player can be assured of
    // beta => maximum score the minimizing player can be assured of
    
    *nodes_searched += 1;

    if ply_depth == 0 {
        // Score relative to white
        let score = material_diff(state);

        // Return score relative to the current player
        return match state.to_play {
            Color::White => score,
            Color::Black => -score,
        };
    }

    let moves = legal_moves(state);

    if moves.len() == 0 && state.in_check() {
        return eval_consts::MATE;
    }

    for m in legal_moves(state).iter() {
        let new_state = state.apply_move(m);
        let score = -alpha_beta_search(&new_state, ply_depth - 1, -beta, -alpha, nodes_searched);
        if score >= beta {
            // Hard beta-cutoff
            return beta;
        }

        if score > alpha {
            alpha = score;
        }
    }

    alpha
}
