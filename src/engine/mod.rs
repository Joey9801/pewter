use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

use crate::{movegen::legal_moves, Move, State};

use anyhow::Result;
use crossbeam_channel::{SendError, Sender};
use rand::{seq::SliceRandom, thread_rng};
use thiserror::Error;

pub mod engine_server;
pub mod eval;

pub use engine_server::EngineServer;

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

#[derive(Clone, Debug)]
/// Assorted information about the recent mechanical performance of the engine
pub struct PerfInfo {
    /// Value between 0 and 1 representing how full the transposition table is
    pub transposition_load: f32,

    /// The number of nodes that have been visited during the current search.
    pub nodes: u64,

    /// The number of nodes searched per second since the start of the current search.
    pub nodes_per_second: f32,

    /// This many positions found in the endgame tablebases
    pub table_hits: u64,

    /// This many positions found in teh shredder endgame databases
    pub shredder_hits: u64,
}

#[derive(Clone, Error, Debug)]
pub enum EngineError {
    #[error("Cannot begin searching for a move as a search is already in progress")]
    AlreadySearching,

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

#[derive(Clone, Debug)]
pub struct SearchControls {
    /// Periodically ready by every search thread. The search will be terminated when this is true.
    pub stop: Arc<AtomicBool>,

    /// Outlet for periodic performance events during the search.
    pub perf_info: Option<Sender<PerfInfo>>,
}

pub struct Engine {
    board_state: Option<State>,
}

impl Engine {
    pub fn new() -> Self {
        Self { board_state: None }
    }

    pub fn set_board_state(&mut self, new_state: State) {
        self.board_state = Some(new_state);
    }

    pub fn search_best_move(
        &mut self,
        _infinite: bool,
        max_depth: Option<u8>,
        _max_nodes: Option<u64>,
        _timings: Option<Timings>,
        controls: SearchControls,
    ) -> Result<Move, EngineError> {
        let start_time = Instant::now();
        let mut last_perf_info = Instant::now();

        let state = &self.board_state.ok_or(EngineError::NoState)?;
        let depth = max_depth.unwrap_or(5);

        let mut alpha = eval::consts::NEG_INFINITY;
        let mut best_move = None;
        let mut moves = legal_moves(state).iter().collect::<Vec<_>>();
        let mut nodes_searched = 0;
        moves.shuffle(&mut thread_rng());
        for m in moves {
            let new_state = state.apply_move(m);
            let score = -alpha_beta_search(
                &new_state,
                depth,
                eval::consts::NEG_INFINITY,
                -alpha,
                &mut nodes_searched,
            );

            if best_move.is_none() || score > alpha {
                alpha = score;
                best_move = Some(m);
            }

            if controls.stop.load(Ordering::Relaxed) {
                log::info!("Received stop signal, stopping search early");
                break;
            }

            if last_perf_info.elapsed().as_secs() >= 3 {
                last_perf_info = Instant::now();

                if let Some(perf_sender) = &controls.perf_info {
                    perf_sender.send(PerfInfo {
                        transposition_load: 0.0,
                        nodes: nodes_searched,
                        nodes_per_second: nodes_searched as f32
                            / start_time.elapsed().as_secs_f32(),
                        table_hits: 0,
                        shredder_hits: 0,
                    })?;
                }
            }
        }

        if let Some(perf_sender) = &controls.perf_info {
            perf_sender.send(PerfInfo {
                transposition_load: 0.0,
                nodes: nodes_searched,
                nodes_per_second: nodes_searched as f32 / start_time.elapsed().as_secs_f32(),
                table_hits: 0,
                shredder_hits: 0,
            })?;
        }

        best_move.ok_or(EngineError::NoMoves)
    }
}

fn alpha_beta_search(
    state: &State,
    ply_depth: u8,
    mut alpha: i32,
    beta: i32,
    nodes_searched: &mut u64,
) -> i32 {
    *nodes_searched += 1;

    if ply_depth == 0 {
        return eval::evaluate(state);
    }

    let moves = legal_moves(state);

    if moves.len() == 0 && state.in_check() {
        return eval::consts::MATE;
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
