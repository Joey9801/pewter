use std::{sync::{Arc, atomic::{AtomicBool, Ordering}}, time::{Instant, Duration}};
use std::fmt::Write;

use crossbeam_channel::Sender;

use pewter_core::{State, Move, movegen::legal_moves, Color};
use crate::engine::ordering::order_moves;

use super::{eval, PerfInfo, EngineError, Evaluation, transposition::{TranspositionTable, NodeType}, Timings};

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
    NonTerminal(Move, Box<MoveChain>)
}

impl MoveChain {
    fn iter(&self) -> MoveChainIter {
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
            write!(out, " {}", m)
                .expect("write!() to a String failed");
        }
        
        out
    }
}

pub struct Searcher {
    controls: SearchControls,
    
    /// Instant that the last call to self.search was made
    last_search_start: Instant,
    
    /// Instant that the last performance info message was emitted
    last_perf_info: Instant,
    
    /// The number of visited nodes that weren't transposition table hits
    nodes_searched: u64,
    
    t_table: TranspositionTable,
    
    principal_variation: Option<Variation>,
}

struct SearchResult {
    eval: Evaluation,
    pv: Option<Variation>,
}

impl SearchResult {
    fn just_eval(eval: Evaluation) -> Self {
        Self {
            eval,
            pv: None,
        }
    }
}

impl Searcher {
    pub fn new(controls: SearchControls) -> Self {
        Self {
            controls,
            nodes_searched: 0,
            last_search_start: Instant::now(),
            last_perf_info: Instant::now(),
            t_table: TranspositionTable::new_empty(),
            principal_variation: None,
        }
    }
    
    pub fn search(&mut self, state: &State, max_depth: u8, timings: Timings, infinite: bool) -> Result<Move, EngineError> {
        self.last_search_start = Instant::now();
        self.last_perf_info = Instant::now();
        self.principal_variation = None;
        
        let remaining = match state.to_play {
            Color::White => timings.white_remaining,
            Color::Black => timings.black_remaining,
        }.unwrap_or(Duration::from_secs(60));

        let time_heuristic = std::cmp::min(remaining / 10, Duration::from_millis(250));
        
        let mut last_pv = None;
        for depth in 1.. {
            if !infinite && depth >= max_depth {
                tracing::debug!("Stopping search because reached max_depth of {max_depth}");
                break;
            }

            if !infinite && self.last_search_start.elapsed() > time_heuristic {
                tracing::debug!("Stopping search because of time heuristic");
                break;
            }
            
            if self.controls.stop.load(Ordering::Relaxed) {
                tracing::debug!("Stopping search because stop signal recieved");
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

            last_pv = result.pv;

            let last_pv = last_pv
                .as_ref()
                .expect("Search concluded without a principal variation");
            
            tracing::info!("Searched depth {}, pv {}", depth, last_pv.format());
        }

        self.emit_perf_msg()?;
        
        if self.controls.stop.load(Ordering::Relaxed) {
            Err(EngineError::EarlyStop)
        } else {
            last_pv
                .map(|pv| pv.moves.first())
                .ok_or(EngineError::NoMoves)
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

        if ply_from_root > max_depth {
            let quiesce_score = self.quiescence_search(state, alpha, beta);
            return Ok(SearchResult::just_eval(quiesce_score));
        }

        let depth_remaining = max_depth - ply_from_root;
        
        // First, check the transposition table in case we've been here before
        if let Some(tt) = self.t_table.probe(state, depth_remaining, alpha, beta) {
            return Ok(SearchResult {
                eval: tt.node_value,
                
                // TODO: Store+export PV in transposition table for Exact nodes
                pv: None,
            });
        }

        let mut moves = legal_moves(state)
            .iter()
            .collect::<Vec<Move>>();

        order_moves(state, &mut moves, &self.t_table);

        if moves.len() == 0 {
            if state.in_check() {
                return Ok(SearchResult::just_eval(eval::consts::MATE));
            } else {
                return Ok(SearchResult::just_eval(eval::consts::DRAW))
            }
        }


        let mut best_move = None;
        let mut node_type = NodeType::UpperBound;
        let mut pv = None;

        for m in moves {
            let new_state = state.apply_move(m);
            let result = self.search_moves(
                &new_state, 
                ply_from_root + 1,
                max_depth,
                -beta,
                -alpha
            )?;

            let score = -result.eval;

            // The move was too good, so the opponent wont allow this position to be reached in the
            // first place
            if score >= beta {
                // TODO: Should the inserted node value be `score` rather than `beta`?
                self.t_table.insert(state, depth_remaining, beta, NodeType::LowerBound, None);
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
            if self.should_stop(ply_from_root, max_depth) {
                break
            }
        }
        
        self.t_table.insert(state, depth_remaining, alpha, node_type, best_move);
        
        Ok(SearchResult {
            eval: alpha,
            pv,
        })
    }
    
    fn quiescence_search(&mut self, state: &State, alpha: Evaluation, beta: Evaluation) -> Evaluation {
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
        order_moves(state, &mut moves, &self.t_table);
        
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
    
    #[inline(always)]
    fn should_stop(&mut self, ply_from_root: u8, max_depth: u8) -> bool {
        if max_depth - ply_from_root >= 4 {
            self.controls.stop.load(Ordering::Relaxed)
        } else if ply_from_root == 0 {
            self.last_search_start.elapsed() > Duration::from_millis(500)
        } else {
            false
        }

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
    
    fn emit_perf_msg(&mut self) -> Result<(), EngineError> {
        if let Some(perf_sender) = &self.controls.perf_info {
            perf_sender.send(PerfInfo {
                transposition_load: self.t_table.load(),
                nodes: self.nodes_searched,
                nodes_per_second: self.nodes_searched as f32
                    / self.last_search_start.elapsed().as_secs_f32(),
                table_hits: 0,
                shredder_hits: 0,
            })?;
        }
        self.last_perf_info = Instant::now();
        
        Ok(())
    }
}