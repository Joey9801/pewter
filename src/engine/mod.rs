use std::{path::Path, time::Duration};

use crate::{Move, State};

use anyhow::Result;
use crossbeam_channel::SendError;
use rand::{seq::SliceRandom, thread_rng};
use thiserror::Error;

pub mod engine_server;
pub mod eval;
pub mod opening_db;
pub mod transposition;
pub mod search;
pub mod ordering;

pub use engine_server::EngineServer;
use eval::Evaluation;
use search::{Searcher, SearchControls};

use opening_db::OpeningDb;

#[derive(Clone, Copy, Debug, Default)]
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

    /// This many positions found in the shredder endgame databases
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
    
    #[error("Engine was stopped before first result")]
    EarlyStop,
}

impl<T> From<SendError<T>> for EngineError {
    fn from(_: SendError<T>) -> Self {
        EngineError::SendError
    }
}

#[derive(Clone)]
pub struct Engine {
    board_state: Option<State>,
    opening_db: Option<OpeningDb>,
}

impl Engine {
    pub fn new() -> Self {
        Self {
            board_state: None,
            opening_db: None,
        }
    }

    pub fn load_opening_db(&mut self, path: &Path) -> Result<()> {
        let data = std::fs::read(path)?;
        self.opening_db = Some(OpeningDb::deserialize(&data)?);
        Ok(())
    }

    pub fn set_board_state(&mut self, new_state: State) {
        self.board_state = Some(new_state);
    }

    pub fn search_best_move(
        &mut self,
        infinite: bool,
        max_depth: Option<u8>,
        _max_nodes: Option<u64>,
        timings: Option<Timings>,
        controls: SearchControls,
    ) -> Result<Move, EngineError> {
        let state = &self.board_state.ok_or(EngineError::NoState)?;

        // Check for opening DB hits first
        if let Some(db) = &self.opening_db {
            let book_move = match db.query(state) {
                [] => None,
                [r] => Some(r.m),
                [multiple @ ..] => Some(multiple.choose(&mut thread_rng()).unwrap().m),
            };

            if let Some(book_move) = book_move {
                log::info!("Responding with book move: {}", book_move);
                return Ok(book_move);
            }
        }
        
        let timings = timings.unwrap_or(Timings::default());
        
        let mut searcher = Searcher::new(controls);
        searcher.search(state, max_depth.unwrap_or(6), timings, infinite)
    }
}