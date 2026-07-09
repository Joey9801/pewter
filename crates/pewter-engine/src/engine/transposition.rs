use pewter_core::{zobrist::ZobristHash, Move, State};

use super::Evaluation;

/// Default transposition table size in mebibytes.
pub const DEFAULT_HASH_MB: usize = 128;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeType {
    Exact,
    LowerBound,
    UpperBound,
}

#[derive(Clone, Copy, Debug)]
pub struct TranspositionEntry {
    /// Full key of the stored position, used to detect index collisions between
    /// two positions that happen to map to the same slot.
    pub key: ZobristHash,
    pub node_type: NodeType,
    pub node_value: Evaluation,
    pub m: Option<Move>,
    pub depth: u8,

    /// The search generation this entry was written in. Entries from an older
    /// generation are cheap to overwrite.
    pub generation: u8,
}

/// A fixed-size, direct-mapped transposition table.
///
/// The table is persistent across searches (owned by the [`Engine`]) and uses a
/// power-of-two number of slots so that indexing is a simple mask of the Zobrist
/// hash. Replacement is depth-preferred, with stale-generation and empty slots
/// always overwritten.
#[derive(Clone)]
pub struct TranspositionTable {
    storage: Vec<Option<TranspositionEntry>>,

    /// `capacity - 1`, where `capacity` is a power of two.
    capacity_mask: usize,

    /// Number of currently-occupied slots, tracked for [`Self::load`].
    occupancy: usize,

    /// The current search generation, bumped by [`Self::new_generation`].
    generation: u8,
}

/// The largest power of two that is `<= n`, for `n >= 1`.
fn prev_power_of_two(n: usize) -> usize {
    debug_assert!(n >= 1);
    let p = n.next_power_of_two();
    if p > n {
        p >> 1
    } else {
        p
    }
}

impl TranspositionTable {
    /// Create a table sized to occupy approximately `mb` mebibytes. The actual
    /// number of slots is rounded down to a power of two, and is always at
    /// least one.
    pub fn with_mb(mb: usize) -> Self {
        let entry_size = std::mem::size_of::<Option<TranspositionEntry>>();
        let bytes = mb.max(1) * 1024 * 1024;
        let capacity = prev_power_of_two((bytes / entry_size).max(1));

        Self {
            storage: vec![None; capacity],
            capacity_mask: capacity - 1,
            occupancy: 0,
            generation: 0,
        }
    }

    /// Resize the table to approximately `mb` mebibytes, discarding all entries.
    pub fn resize(&mut self, mb: usize) {
        *self = Self::with_mb(mb);
    }

    /// Build a table with an exact power-of-two number of slots. Used by tests
    /// to force index collisions.
    #[cfg(test)]
    fn with_slot_count(slots: usize) -> Self {
        assert!(slots.is_power_of_two());
        Self {
            storage: vec![None; slots],
            capacity_mask: slots - 1,
            occupancy: 0,
            generation: 0,
        }
    }

    /// The number of slots in the table.
    pub fn capacity(&self) -> usize {
        self.storage.len()
    }

    /// Returns a number between 0 and 1, representing how full this table is.
    pub fn load(&self) -> f32 {
        self.occupancy as f32 / self.storage.len() as f32
    }

    /// Remove all entries from this table.
    pub fn clear(&mut self) {
        for slot in self.storage.iter_mut() {
            *slot = None;
        }
        self.occupancy = 0;
        self.generation = 0;
    }

    /// Begin a new search generation. Entries written before this call become
    /// preferred eviction candidates.
    pub fn new_generation(&mut self) {
        self.generation = self.generation.wrapping_add(1);
    }

    #[inline(always)]
    fn index(&self, key: ZobristHash) -> usize {
        key.get() as usize & self.capacity_mask
    }

    /// Insert the given evaluation into this table.
    pub fn insert(
        &mut self,
        state: &State,
        depth: u8,
        node_value: Evaluation,
        node_type: NodeType,
        m: Option<Move>,
    ) {
        let idx = self.index(state.zobrist);
        let new_entry = TranspositionEntry {
            key: state.zobrist,
            node_type,
            node_value,
            m,
            depth,
            generation: self.generation,
        };

        let replace = match &self.storage[idx] {
            None => {
                self.occupancy += 1;
                true
            }
            Some(existing) => {
                // Always refresh our own entry, evict entries from a previous
                // search, and otherwise prefer the result that looked deepest.
                existing.key == state.zobrist
                    || existing.generation != self.generation
                    || depth >= existing.depth
            }
        };

        if replace {
            self.storage[idx] = Some(new_entry);
        }
    }

    pub fn probe(
        &self,
        state: &State,
        min_depth: u8,
        alpha: Evaluation,
        beta: Evaluation,
    ) -> Option<TranspositionEntry> {
        let idx = self.index(state.zobrist);
        let entry = self.storage[idx].as_ref()?;

        // A different position may map to the same slot; the full key check
        // rejects those collisions.
        if entry.key != state.zobrist {
            return None;
        }

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
            }
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

#[cfg(test)]
mod tests {
    use super::*;
    use pewter_core::io::fen::parse_fen;

    fn startpos() -> State {
        parse_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1").unwrap()
    }

    #[test]
    fn with_mb_capacity_is_power_of_two() {
        for mb in [1, 2, 5, 16, 64, 128, 256] {
            let table = TranspositionTable::with_mb(mb);
            assert!(table.capacity().is_power_of_two());
            assert!(table.capacity() >= 1);
        }
    }

    #[test]
    fn insert_probe_round_trip() {
        let mut table = TranspositionTable::with_mb(1);
        let state = startpos();
        table.insert(&state, 5, 42, NodeType::Exact, None);

        let entry = table
            .probe(&state, 0, -1000, 1000)
            .expect("expected a hit for the inserted position");
        assert_eq!(entry.node_value, 42);
        assert_eq!(entry.depth, 5);
    }

    #[test]
    fn shallow_entry_is_a_miss_for_deeper_probe() {
        let mut table = TranspositionTable::with_mb(1);
        let state = startpos();
        table.insert(&state, 2, 42, NodeType::Exact, None);
        assert!(table.probe(&state, 4, -1000, 1000).is_none());
    }

    #[test]
    fn same_key_is_always_refreshed() {
        let mut table = TranspositionTable::with_mb(1);
        let state = startpos();

        table.insert(&state, 8, 100, NodeType::Exact, None);
        // Re-storing the same position replaces the previous result with the
        // latest search's value.
        table.insert(&state, 3, 200, NodeType::Exact, None);

        let entry = table.probe(&state, 0, -1000, 1000).unwrap();
        assert_eq!(entry.depth, 3);
        assert_eq!(entry.node_value, 200);
    }

    #[test]
    fn colliding_key_replacement_is_depth_preferred() {
        // A single-slot table forces every position onto the same index.
        let mut table = TranspositionTable::with_slot_count(1);
        let deep = parse_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1").unwrap();
        let shallow = parse_fen("rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1").unwrap();

        table.insert(&deep, 8, 100, NodeType::Exact, None);
        // A shallower, different position must not evict the deeper entry.
        table.insert(&shallow, 3, 200, NodeType::Exact, None);
        assert_eq!(table.probe(&deep, 0, -1000, 1000).unwrap().depth, 8);
        assert!(table.probe(&shallow, 0, -1000, 1000).is_none());

        // A deeper different position does evict it.
        table.insert(&shallow, 9, 300, NodeType::Exact, None);
        assert!(table.probe(&deep, 0, -1000, 1000).is_none());
        assert_eq!(table.probe(&shallow, 0, -1000, 1000).unwrap().node_value, 300);
    }

    #[test]
    fn new_generation_frees_stale_entries() {
        // Force a collision so the stale-generation rule is what allows the
        // second, different position to take the slot.
        let mut table = TranspositionTable::with_slot_count(1);
        let old = parse_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1").unwrap();
        let new = parse_fen("rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1").unwrap();

        table.insert(&old, 8, 100, NodeType::Exact, None);
        table.new_generation();
        // Even though it is shallower, a fresh-generation entry evicts a stale
        // one from a previous search.
        table.insert(&new, 1, 200, NodeType::Exact, None);

        assert!(table.probe(&old, 0, -1000, 1000).is_none());
        let entry = table.probe(&new, 0, -1000, 1000).unwrap();
        assert_eq!(entry.node_value, 200);
    }

    #[test]
    fn clear_empties_the_table() {
        let mut table = TranspositionTable::with_mb(1);
        let state = startpos();
        table.insert(&state, 5, 42, NodeType::Exact, None);
        table.clear();
        assert!(table.probe(&state, 0, -1000, 1000).is_none());
        assert_eq!(table.load(), 0.0);
    }
}
