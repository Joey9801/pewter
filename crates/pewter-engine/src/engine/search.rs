use std::fmt::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crossbeam_channel::Sender;

use crate::engine::ordering::order_moves;
use pewter_core::{movegen::legal_moves, zobrist::ZobristHash, Color, Move, State};

use super::transposition::{NodeType, TranspositionTable};
use super::{eval, EngineError, Evaluation, PerfInfo, Timings};

/// How many nodes to search between polls of the stop flag / clock.
const STOP_CHECK_INTERVAL: u32 = 2048;

/// Time deducted from the allotted budget to account for the time taken to
/// actually transmit the chosen move.
const MOVE_OVERHEAD: Duration = Duration::from_millis(30);

/// The smallest amount of time we will ever budget for a move.
const MIN_MOVE_TIME: Duration = Duration::from_millis(5);

#[derive(Clone, Debug)]
pub struct SearchControls {
    /// Periodically ready by every search thread. The search will be terminated when this is true.
    pub stop: Arc<AtomicBool>,

    /// Outlet for periodic performance events during the search.
    pub perf_info: Option<Sender<PerfInfo>>,
}

#[derive(Clone, Debug)]
pub enum MoveChain {
    Terminal(Move),
    NonTerminal(Move, Box<MoveChain>),
}

impl MoveChain {
    fn iter(&self) -> MoveChainIter<'_> {
        MoveChainIter { curr: Some(self) }
    }

    fn first(&self) -> Move {
        match self {
            MoveChain::Terminal(m) => *m,
            MoveChain::NonTerminal(m, _) => *m,
        }
    }
}

pub struct MoveChainIter<'a> {
    curr: Option<&'a MoveChain>,
}

impl<'a> Iterator for MoveChainIter<'a> {
    type Item = Move;

    fn next(&mut self) -> Option<Self::Item> {
        let m = match self.curr {
            Some(MoveChain::Terminal(m)) => Some(*m),
            Some(MoveChain::NonTerminal(m, _)) => Some(*m),
            None => None,
        };

        self.curr = match self.curr {
            Some(MoveChain::NonTerminal(_, next)) => Some(next),
            _ => None,
        };

        m
    }
}

#[derive(Clone, Debug)]
pub struct Variation {
    /// Moves in this variation, in reverse order
    pub moves: MoveChain,

    /// The evaluated score of this variation
    pub eval: Evaluation,
}

impl Variation {
    pub fn format(&self) -> String {
        let mut out = String::new();
        for m in self.moves.iter() {
            write!(out, " {}", m).expect("write!() to a String failed");
        }

        out
    }
}

/// The soft/hard time budget for a single search.
///
/// The search will not begin a new iterative-deepening iteration once the soft
/// deadline has passed, and will abort mid-iteration once the hard deadline
/// passes.
#[derive(Clone, Copy, Debug)]
struct TimeBudget {
    soft: Duration,
    hard: Duration,
}

impl TimeBudget {
    /// Derive a time budget from the given timing information, or `None` if the
    /// search is unconstrained by time (depth- or node-limited only).
    fn from_timings(state: &State, timings: &Timings) -> Option<Self> {
        // An explicit "spend exactly this long" request pins soft == hard.
        if let Some(move_time) = timings.move_time {
            let t = move_time.saturating_sub(MOVE_OVERHEAD).max(MIN_MOVE_TIME);
            return Some(Self { soft: t, hard: t });
        }

        let (remaining, increment) = match state.to_play {
            Color::White => (timings.white_remaining, timings.white_increment),
            Color::Black => (timings.black_remaining, timings.black_increment),
        };

        // With no clock and no move time we cannot budget at all.
        let remaining = remaining?;

        // Aim to use ~1/25th of the remaining time plus most of the increment,
        // but never gamble more than 1/5th of the clock on a single move.
        let soft = remaining / 25 + (increment * 3) / 4;
        let hard = std::cmp::min(remaining / 5, soft * 3);

        let soft = soft.saturating_sub(MOVE_OVERHEAD);
        let hard = hard.saturating_sub(MOVE_OVERHEAD);

        // Clamp so that MIN_MOVE_TIME <= soft <= hard.
        let soft = soft.max(MIN_MOVE_TIME);
        let hard = hard.max(soft);

        Some(Self { soft, hard })
    }
}

/// Normalise a mate score for storage in the transposition table.
///
/// Search scores encode "mate in N plies from the root"; the table stores
/// positions independent of the path taken to reach them, so mate scores are
/// re-expressed as "mate in N plies from this node".
fn to_tt_score(score: Evaluation, ply_from_root: u8) -> Evaluation {
    if eval::is_mate_score(score) {
        if score > 0 {
            score + ply_from_root as Evaluation
        } else {
            score - ply_from_root as Evaluation
        }
    } else {
        score
    }
}

/// Inverse of [`to_tt_score`]: convert a stored, node-relative mate score back
/// into a root-relative one.
fn from_tt_score(score: Evaluation, ply_from_root: u8) -> Evaluation {
    if eval::is_mate_score(score) {
        if score > 0 {
            score - ply_from_root as Evaluation
        } else {
            score + ply_from_root as Evaluation
        }
    } else {
        score
    }
}

pub struct Searcher<'a> {
    controls: SearchControls,

    /// Instant that the last call to self.search was made
    last_search_start: Instant,

    /// Instant that the last performance info message was emitted
    last_perf_info: Instant,

    /// The number of visited nodes that weren't transposition table hits
    nodes_searched: u64,

    /// Persistent transposition table, owned by the engine.
    t_table: &'a mut TranspositionTable,

    /// Zobrist hashes of every position played in the actual game up to and
    /// including the root position, used for repetition detection.
    game_history: Vec<ZobristHash>,

    /// Zobrist hashes of the positions along the current search line, used to
    /// detect repetitions that occur within the search tree itself.
    path: Vec<ZobristHash>,

    /// Once the soft deadline passes we stop starting new iterations.
    soft_deadline: Option<Instant>,

    /// Once the hard deadline passes we abort the search in progress.
    hard_deadline: Option<Instant>,

    /// True for `go infinite` searches, which ignore the clock entirely.
    infinite: bool,

    /// Latched once the search has decided to stop; every node thereafter bails
    /// out immediately.
    aborted: bool,

    /// Optional cap on the number of nodes to search.
    max_nodes: Option<u64>,

    /// Nodes searched since we last polled the stop conditions.
    nodes_since_check: u32,
}

struct SearchResult {
    eval: Evaluation,
    pv: Option<Variation>,
}

impl SearchResult {
    fn just_eval(eval: Evaluation) -> Self {
        Self { eval, pv: None }
    }
}

impl<'a> Searcher<'a> {
    pub fn new(
        controls: SearchControls,
        t_table: &'a mut TranspositionTable,
        game_history: Vec<ZobristHash>,
    ) -> Self {
        Self {
            controls,
            nodes_searched: 0,
            last_search_start: Instant::now(),
            last_perf_info: Instant::now(),
            t_table,
            game_history,
            path: Vec::new(),
            soft_deadline: None,
            hard_deadline: None,
            infinite: false,
            aborted: false,
            max_nodes: None,
            nodes_since_check: 0,
        }
    }

    pub fn search(
        &mut self,
        state: &State,
        max_depth: u8,
        max_nodes: Option<u64>,
        timings: Timings,
        infinite: bool,
    ) -> Result<Move, EngineError> {
        self.last_search_start = Instant::now();
        self.last_perf_info = Instant::now();
        self.nodes_searched = 0;
        self.nodes_since_check = 0;
        self.aborted = false;
        self.infinite = infinite;
        self.max_nodes = max_nodes;
        self.path.clear();

        let budget = if infinite {
            None
        } else {
            TimeBudget::from_timings(state, &timings)
        };
        self.soft_deadline = budget.map(|b| self.last_search_start + b.soft);
        self.hard_deadline = budget.map(|b| self.last_search_start + b.hard);

        // In time- or infinite-mode, only the clock and stop flag bound the
        // depth. In explicit-depth mode, honour the requested depth.
        let overall_max_depth = if infinite { u8::MAX } else { max_depth.max(1) };

        // A legal move to fall back on if we get aborted before completing even
        // the first depth.
        let fallback_move = legal_moves(state).iter().next();

        let mut last_pv: Option<Variation> = None;
        for depth in 1..=overall_max_depth {
            // Don't start a new (more expensive) iteration once the soft
            // deadline has passed, but always finish at least depth 1.
            if depth > 1 {
                if let Some(soft) = self.soft_deadline {
                    if Instant::now() >= soft {
                        tracing::debug!("Stopping search: soft deadline reached");
                        break;
                    }
                }
            }

            if self.controls.stop.load(Ordering::Relaxed) {
                tracing::debug!("Stopping search because stop signal received");
                break;
            }

            tracing::debug!("Beginning search at depth {depth}");
            let result = self.search_moves(
                state,
                0,
                depth,
                eval::consts::NEG_INFINITY,
                eval::consts::POS_INFINITY,
            )?;

            // If the search was aborted part way through this iteration its
            // result is from an incomplete depth. Discard it and play the best
            // move from the last fully-searched depth.
            if self.aborted {
                tracing::debug!("Discarding incomplete depth {depth}");
                break;
            }

            last_pv = result.pv;

            if let Some(pv) = last_pv.as_ref() {
                self.emit_depth_info(depth, pv.eval)?;
                tracing::info!("Searched depth {}, pv {}", depth, pv.format());
            }
        }

        self.emit_perf_msg()?;

        match last_pv.map(|pv| pv.moves.first()).or(fallback_move) {
            Some(m) => Ok(m),
            None => {
                if self.controls.stop.load(Ordering::Relaxed) {
                    Err(EngineError::EarlyStop)
                } else {
                    Err(EngineError::NoMoves)
                }
            }
        }
    }

    fn search_moves(
        &mut self,
        state: &State,
        ply_from_root: u8,
        max_depth: u8,
        mut alpha: Evaluation,
        beta: Evaluation,
    ) -> Result<SearchResult, EngineError> {
        self.nodes_searched += 1;

        // Draw by repetition. Never applies at the root itself (a position is
        // only a repetition of an *earlier* one).
        if ply_from_root > 0 && self.is_repetition(state) {
            return Ok(SearchResult::just_eval(eval::consts::DRAW));
        }

        if ply_from_root > max_depth {
            let quiesce_score = self.quiescence_search(state, alpha, beta);
            return Ok(SearchResult::just_eval(quiesce_score));
        }

        let depth_remaining = max_depth - ply_from_root;

        // First, check the transposition table in case we've been here before,
        // un-normalising any stored mate distance to be relative to the root.
        //
        // Never take this early return at the root: the table entry carries no
        // principal variation, so returning here would leave us without a move
        // to actually play. The root always performs a real search (it still
        // benefits from the table for move ordering and deeper cutoffs).
        if ply_from_root > 0 {
            if let Some(tt) = self.t_table.probe(state, depth_remaining, alpha, beta) {
                return Ok(SearchResult {
                    eval: from_tt_score(tt.node_value, ply_from_root),

                    // TODO: Store+export PV in transposition table for Exact nodes
                    pv: None,
                });
            }
        }

        let mut moves = legal_moves(state).iter().collect::<Vec<Move>>();

        order_moves(state, &mut moves, &*self.t_table);

        if moves.len() == 0 {
            if state.in_check() {
                // Fold the distance from the root into the mate score so that
                // faster mates (and slower defences) are preferred.
                return Ok(SearchResult::just_eval(
                    eval::consts::MATE + ply_from_root as Evaluation,
                ));
            } else {
                return Ok(SearchResult::just_eval(eval::consts::DRAW));
            }
        }

        // Draw by the fifty-move rule. Checked only after mate/stalemate has
        // been resolved, so that delivering checkmate on the final reversible
        // move still scores as a mate rather than a draw.
        if state.halfmove_clock >= 100 {
            return Ok(SearchResult::just_eval(eval::consts::DRAW));
        }

        let mut best_move = None;
        let mut node_type = NodeType::UpperBound;
        let mut pv = None;

        // Record this position on the search path so that deeper nodes can
        // detect a repetition back to it.
        self.path.push(state.zobrist);

        for m in moves {
            let new_state = state.apply_move(m);
            let result =
                self.search_moves(&new_state, ply_from_root + 1, max_depth, -beta, -alpha)?;

            let score = -result.eval;

            // The move was too good, so the opponent wont allow this position to be reached in the
            // first place
            if score >= beta {
                // TODO: Should the inserted node value be `score` rather than `beta`?
                self.t_table.insert(
                    state,
                    depth_remaining,
                    to_tt_score(beta, ply_from_root),
                    NodeType::LowerBound,
                    None,
                );
                self.path.pop();
                return Ok(SearchResult::just_eval(beta));
            }

            if score > alpha {
                node_type = NodeType::Exact;
                best_move = Some(m);
                alpha = score;

                pv = Some(Variation {
                    moves: match result.pv {
                        Some(pv) => MoveChain::NonTerminal(m, Box::new(pv.moves)),
                        None => MoveChain::Terminal(m),
                    },
                    eval: alpha,
                });
            }

            self.maybe_emit_perf_msg(ply_from_root, max_depth)?;
            if self.should_stop() {
                break;
            }
        }

        self.path.pop();

        self.t_table.insert(
            state,
            depth_remaining,
            to_tt_score(alpha, ply_from_root),
            node_type,
            best_move,
        );

        Ok(SearchResult { eval: alpha, pv })
    }

    /// Whether the given position repeats one already seen along the current
    /// search path or in the game history within the last `halfmove_clock`
    /// plies (positions further back are separated by an irreversible move and
    /// so cannot be repetitions).
    fn is_repetition(&self, state: &State) -> bool {
        let lookback = state.halfmove_clock as usize;
        if lookback == 0 {
            return false;
        }

        self.path
            .iter()
            .rev()
            .chain(self.game_history.iter().rev())
            .take(lookback)
            .any(|&h| h == state.zobrist)
    }

    fn quiescence_search(
        &mut self,
        state: &State,
        alpha: Evaluation,
        beta: Evaluation,
    ) -> Evaluation {
        let root_eval = eval::evaluate(state);
        if root_eval >= beta {
            return beta;
        }
        let mut alpha = std::cmp::max(alpha, root_eval);

        fn move_is_capture(state: &State, m: Move) -> bool {
            state.board.color_board(!state.to_play).get(m.to)
        }

        let mut moves = legal_moves(state)
            .iter()
            .filter(|m| move_is_capture(state, *m))
            .collect::<Vec<Move>>();
        order_moves(state, &mut moves, &*self.t_table);

        for m in moves {
            let new_state = state.apply_move(m);
            let score = -self.quiescence_search(&new_state, -beta, -alpha);
            if score >= beta {
                return beta;
            }

            alpha = std::cmp::max(alpha, score);
        }

        alpha
    }

    /// Poll the stop conditions roughly every [`STOP_CHECK_INTERVAL`] nodes.
    /// Once any condition fires, [`Self::aborted`] latches so that the rest of
    /// the search unwinds immediately.
    #[inline(always)]
    fn should_stop(&mut self) -> bool {
        if self.aborted {
            return true;
        }

        self.nodes_since_check += 1;
        if self.nodes_since_check < STOP_CHECK_INTERVAL {
            return false;
        }
        self.nodes_since_check = 0;

        if self.controls.stop.load(Ordering::Relaxed) {
            self.aborted = true;
            return true;
        }

        if let Some(max_nodes) = self.max_nodes {
            if self.nodes_searched >= max_nodes {
                self.aborted = true;
                return true;
            }
        }

        if !self.infinite {
            if let Some(hard) = self.hard_deadline {
                if Instant::now() >= hard {
                    self.aborted = true;
                    return true;
                }
            }
        }

        false
    }

    #[inline(always)]
    fn maybe_emit_perf_msg(&mut self, ply_from_root: u8, max_depth: u8) -> Result<(), EngineError> {
        if max_depth - ply_from_root >= 4 {
            if self.last_perf_info.elapsed().as_secs() > 3 {
                self.emit_perf_msg()?;
            }
        }

        Ok(())
    }

    fn emit_depth_info(&mut self, depth: u8, score: Evaluation) -> Result<(), EngineError> {
        if let Some(perf_sender) = &self.controls.perf_info {
            perf_sender.send(PerfInfo {
                transposition_load: self.t_table.load(),
                nodes: self.nodes_searched,
                nodes_per_second: self.nodes_per_second(),
                table_hits: 0,
                shredder_hits: 0,
                depth: Some(depth),
                score: Some(score),
            })?;
        }
        self.last_perf_info = Instant::now();

        Ok(())
    }

    fn emit_perf_msg(&mut self) -> Result<(), EngineError> {
        if let Some(perf_sender) = &self.controls.perf_info {
            perf_sender.send(PerfInfo {
                transposition_load: self.t_table.load(),
                nodes: self.nodes_searched,
                nodes_per_second: self.nodes_per_second(),
                table_hits: 0,
                shredder_hits: 0,
                depth: None,
                score: None,
            })?;
        }
        self.last_perf_info = Instant::now();

        Ok(())
    }

    fn nodes_per_second(&self) -> f32 {
        let elapsed = self.last_search_start.elapsed().as_secs_f32();
        if elapsed > 0.0 {
            self.nodes_searched as f32 / elapsed
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::eval::mate_in_moves;
    use pewter_core::io::fen::parse_fen;

    fn controls() -> SearchControls {
        SearchControls {
            stop: Arc::new(AtomicBool::new(false)),
            perf_info: None,
        }
    }

    /// Run `search_moves` from the root at a fixed depth, with no time or node
    /// limits, returning the chosen move and the root score.
    fn search_root(
        fen: &str,
        depth: u8,
        game_history: Vec<ZobristHash>,
    ) -> (Option<Move>, Evaluation) {
        let state = parse_fen(fen).unwrap();
        let mut t_table = TranspositionTable::with_mb(1);
        let mut searcher = Searcher::new(controls(), &mut t_table, game_history);
        let result = searcher
            .search_moves(
                &state,
                0,
                depth,
                eval::consts::NEG_INFINITY,
                eval::consts::POS_INFINITY,
            )
            .unwrap();
        (result.pv.map(|pv| pv.moves.first()), result.eval)
    }

    #[test]
    fn finds_mate_in_one() {
        // White plays Ra1-a8#: the black king on g8 is boxed in by its own
        // pawns and every escape square is covered by the rook.
        let fen = "6k1/5ppp/8/8/8/8/8/R6K w - - 0 1";
        let (best_move, score) = search_root(fen, 4, vec![]);

        let best_move = best_move.expect("expected a best move");
        assert_eq!(format!("{}", best_move), "a1a8");
        assert!(eval::is_mate_score(score));
        assert_eq!(mate_in_moves(score), Some(1));
    }

    #[test]
    fn fifty_move_rule_scores_draw() {
        // Not in check, legal moves available, but the halfmove clock has hit
        // 100 — the position is a draw regardless of the material imbalance.
        let fen = "8/8/8/4k3/8/4K3/8/R7 w - - 100 1";
        let (_best_move, score) = search_root(fen, 4, vec![]);
        assert_eq!(score, eval::consts::DRAW);
    }

    #[test]
    fn checkmate_takes_precedence_over_fifty_move_rule() {
        // Back-rank mate with the halfmove clock at 100: mate must win over the
        // fifty-move draw.
        let fen = "6k1/8/8/8/8/8/5PPP/3r2K1 w - - 100 1";
        let (_best_move, score) = search_root(fen, 2, vec![]);
        assert!(eval::is_mate_score(score));
        assert_eq!(mate_in_moves(score), Some(0));
    }

    #[test]
    fn repetition_detected_against_game_history() {
        // A reversible position whose hash is already in the game history is a
        // repetition (the halfmove clock is non-zero so the lookback window is
        // open).
        let state = parse_fen("4k3/8/8/8/8/8/8/4K3 w - - 8 20").unwrap();
        let mut t_table = TranspositionTable::with_mb(1);
        let searcher = Searcher::new(controls(), &mut t_table, vec![state.zobrist]);
        assert!(searcher.is_repetition(&state));
    }

    #[test]
    fn repetition_detected_along_search_path() {
        let state = parse_fen("4k3/8/8/8/8/8/8/4K3 w - - 8 20").unwrap();
        let mut t_table = TranspositionTable::with_mb(1);
        let mut searcher = Searcher::new(controls(), &mut t_table, vec![]);
        searcher.path.push(state.zobrist);
        assert!(searcher.is_repetition(&state));
    }

    #[test]
    fn no_repetition_when_halfmove_clock_zero() {
        // With a zero halfmove clock the lookback window is closed, so even a
        // matching hash is not a repetition.
        let state = parse_fen("4k3/8/8/8/8/8/8/4K3 w - - 0 20").unwrap();
        let mut t_table = TranspositionTable::with_mb(1);
        let searcher = Searcher::new(controls(), &mut t_table, vec![state.zobrist]);
        assert!(!searcher.is_repetition(&state));
    }

    fn budget(timings: Timings) -> Option<TimeBudget> {
        let state = parse_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1").unwrap();
        TimeBudget::from_timings(&state, &timings)
    }

    #[test]
    fn move_time_budget_subtracts_overhead() {
        let b = budget(Timings {
            move_time: Some(Duration::from_millis(1000)),
            ..Timings::default()
        })
        .unwrap();
        assert_eq!(b.soft, Duration::from_millis(1000) - MOVE_OVERHEAD);
        assert_eq!(b.hard, b.soft);
    }

    #[test]
    fn clock_budget_is_sane() {
        let cases = [
            // (remaining_ms, increment_ms)
            (60_000u64, 0u64),
            (10_000, 100),
            (100, 0),
        ];

        for (rem, inc) in cases {
            let remaining = Duration::from_millis(rem);
            let b = budget(Timings {
                white_remaining: Some(remaining),
                white_increment: Duration::from_millis(inc),
                ..Timings::default()
            })
            .unwrap();

            assert!(
                b.soft >= MIN_MOVE_TIME,
                "soft below minimum for {}+{}",
                rem,
                inc
            );
            assert!(b.soft <= b.hard, "soft exceeded hard for {}+{}", rem, inc);
            assert!(
                b.hard <= remaining / 5,
                "hard exceeded a fifth of the clock for {}+{}",
                rem,
                inc
            );
        }
    }

    #[test]
    fn no_budget_without_clock_or_move_time() {
        assert!(budget(Timings::default()).is_none());
    }
}
