use std::{path::Path, time::Duration};

use pewter_core::{Move, State, zobrist::ZobristHash};

use anyhow::Result;
use crossbeam_channel::SendError;
use rand::seq::IndexedRandom;
use thiserror::Error;

pub mod engine_server;
pub mod eval;
pub mod opening_db;
pub mod ordering;
pub mod search;
pub mod transposition;

pub use engine_server::EngineServer;
use eval::Evaluation;
use search::{SearchControls, Searcher, WobbleConfig};
use transposition::{DEFAULT_HASH_MB, TranspositionTable};

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

    /// Request from the engine host to spend exactly this much time on the next move
    pub move_time: Option<Duration>,
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

    /// The iterative-deepening depth this message reports the score for, if any.
    pub depth: Option<u8>,

    /// The score of the root position from the engine's point of view, in the
    /// engine's internal units, if known.
    pub score: Option<Evaluation>,
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

    /// Persistent, fixed-size transposition table, reused across searches.
    t_table: TranspositionTable,

    /// Zobrist hashes of every position played so far in the current game, up to
    /// and including `board_state`. Used for repetition detection during search.
    game_history: Vec<ZobristHash>,

    /// Optional root-move randomisation, for diversifying self-play games.
    wobble: WobbleConfig,
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

impl Engine {
    pub fn new() -> Self {
        Self {
            board_state: None,
            opening_db: None,
            t_table: TranspositionTable::with_mb(DEFAULT_HASH_MB),
            game_history: Vec::new(),
            wobble: WobbleConfig::default(),
        }
    }

    pub fn load_opening_db(&mut self, path: &Path) -> Result<()> {
        let data = std::fs::read(path)?;
        self.opening_db = Some(OpeningDb::deserialize(&data)?);
        Ok(())
    }

    /// Set the position to search from, along with the Zobrist hashes of every
    /// position that led to it (start position through to `new_state`
    /// inclusive), so that the search can detect repetitions.
    pub fn set_board_state(&mut self, new_state: State, game_history: Vec<ZobristHash>) {
        self.board_state = Some(new_state);
        self.game_history = game_history;
    }

    /// Resize the transposition table, discarding its contents.
    pub fn set_hash_size(&mut self, mb: usize) {
        self.t_table.resize(mb);
    }

    /// Configure root-move randomisation: play a random move from among those
    /// within `margin_cp` centipawns of the best, but only for the first
    /// `plies` half-moves of the game. A `margin_cp` of 0 disables it.
    pub fn set_wobble(&mut self, margin_cp: Evaluation, plies: u8) {
        self.wobble = WobbleConfig {
            margin: margin_cp,
            plies,
        };
    }

    /// Reset engine state for a fresh game, clearing the transposition table.
    pub fn new_game(&mut self) {
        self.t_table.clear();
        self.game_history.clear();
    }

    pub fn search_best_move(
        &mut self,
        infinite: bool,
        max_depth: Option<u8>,
        max_nodes: Option<u64>,
        timings: Option<Timings>,
        controls: SearchControls,
    ) -> Result<Move, EngineError> {
        let state = self.board_state.ok_or(EngineError::NoState)?;

        // Check for opening DB hits first
        if let Some(db) = &self.opening_db {
            let book_move = match db.query(&state) {
                [] => None,
                [r] => Some(r.m),
                multiple => Some(multiple.choose(&mut rand::rng()).unwrap().m),
            };

            if let Some(book_move) = book_move {
                tracing::info!("Responding with book move: {}", book_move);
                return Ok(book_move);
            }
        }

        let timings = timings.unwrap_or_default();

        // Bump the table generation so this search's writes are preferred over
        // entries left behind by previous searches.
        self.t_table.new_generation();

        let game_history = self.game_history.clone();
        let mut searcher = Searcher::new(controls, &mut self.t_table, game_history, self.wobble);
        searcher.search(
            &state,
            max_depth.unwrap_or(u8::MAX),
            max_nodes,
            timings,
            infinite,
        )
    }
}
