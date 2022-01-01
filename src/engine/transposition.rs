use std::collections::HashMap;

use rand::prelude::*;

use crate::{State, Move, zobrist::ZobristHash};

use super::Evaluation;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeType {
    Exact,
    LowerBound,
    UpperBound,
}

#[derive(Clone, Copy, Debug)]
pub struct TranspositionEntry {
    pub node_type: NodeType,
    pub node_value: Evaluation,
    pub m: Option<Move>,
    pub depth: u8,
}

pub struct TranspositionTable {
    /// The maximum number of keys that should appear in the table
    max_size: usize,

    storage: HashMap<ZobristHash, TranspositionEntry>,
    hit_count: usize,
    miss_count: usize,
}

impl TranspositionTable {
    pub fn new_empty() -> Self {
        let max_size = 100_000_000;
        Self {
            storage: HashMap::with_capacity(max_size),
            max_size,
            hit_count: 0,
            miss_count: 0,
        }
    }
    
    /// Returns a number between 0 and 1, representing how full this table is
    pub fn load(&self) -> f32 {
        self.storage.len() as f32 / self.max_size as f32
    }
    
    /// Returns the fraction of cache hits as a number between 0 and 1.
    pub fn hit_rate(&self) -> f32 {
       let total = self.hit_count + self.miss_count;
       if total > 0 {
           self.hit_count as f32 / total as f32
       } else {
           0f32
       }
    }
    
    /// Remove all entries from this table
    pub fn clear(&mut self) {
        self.storage.clear();
        self.hit_count = 0;
        self.miss_count = 0;
    }
    
    /// Insert the given evaluation into this table
    pub fn insert(&mut self, state: &State, depth: u8, node_value: Evaluation, node_type: NodeType, m: Option<Move>) {
        while self.storage.len() >= self.max_size {
            // TODO: non-random eviction
            let unlucky_key = thread_rng().gen_range(0..self.storage.len());
            let unlucky_key = *self.storage.keys().skip(unlucky_key).next().unwrap();
            self.storage.remove(&unlucky_key);
        }

        self.storage.insert(state.zobrist, TranspositionEntry {
            node_type,
            node_value,
            m,
            depth,
        });
    }
    
    pub fn probe(&self, state: &State, min_depth: u8, alpha: Evaluation, beta: Evaluation) -> Option<TranspositionEntry> {
        let entry = self.storage.get(&state.zobrist)?;
        
        // If the stored evaluation didn't look as far ahead as we need, this
        // is actually a cache miss
        if entry.depth < min_depth {
            return None;
        }
        
        match entry.node_type {
            // If we have the exact value for the node, unconditionally return
            // it
            NodeType::Exact => Some(*entry),
            NodeType::UpperBound => {
                if entry.node_value <= alpha {
                    // We don't know the exact value of this node, but we do
                    // know that it's not greater than Alpha, so the search
                    // definitely won't find a new best move in this subtree.
                    Some(*entry)
                } else {
                    None
                }
            },
            NodeType::LowerBound => {
                if entry.node_value >= beta {
                    // We don't know the exact value of this node, but we do
                    // know that it's not smaller than beta, so it's definitly
                    // safe to trigger a beta cutoff for this subtree.
                    Some(*entry)
                } else {
                    None
                }
            }
        }
    }
}