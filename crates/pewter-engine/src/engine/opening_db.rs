use std::collections::{HashMap, HashSet};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use pewter_core::{io::pgn::Game, state::GameResult, Color, Move, State, zobrist::ZobristHash};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpeningDb(HashMap<ZobristHash, Vec<DbResult>>);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbResult {
    /// The potential move
    pub m: Move,

    /// The number of wins that follow from that move
    pub wins: u32,

    /// The number of draws that follow from that move
    pub draws: u32,

    /// The number of losses that follow from that move
    pub losses: u32,
}

impl DbResult {
    pub fn total_count(&self) -> u32 {
        self.wins + self.draws + self.losses
    }
}

impl OpeningDb {
    pub fn new_empty() -> Self {
        Self(HashMap::new())
    }

    pub fn add_game(&mut self, game: &Game) {
        let mut state = game.initial;

        // Be a little defensive
        state.zobrist = pewter_core::zobrist::calculate_entire_zobrist(&state);

        for m in &game.moves {
            let existing_set = self.0.entry(state.zobrist).or_insert(Vec::new());

            let result = match existing_set.iter().position(|r| r.m == *m) {
                Some(idx) => existing_set.get_mut(idx).unwrap(),
                None => {
                    existing_set.push(DbResult {
                        m: *m,
                        wins: 0,
                        draws: 0,
                        losses: 0,
                    });

                    existing_set.last_mut().unwrap()
                }
            };

            match (state.to_play, game.result) {
                (Color::White, GameResult::WhiteWin) | (Color::Black, GameResult::BlackWin) => {
                    result.wins += 1
                }
                (Color::White, GameResult::BlackWin) | (Color::Black, GameResult::WhiteWin) => {
                    result.losses += 1
                }
                (_, GameResult::Draw) => result.draws += 1,
                (_, GameResult::Ongoing) => panic!("Can't add an ongoing game to the opening DB"),
            }

            state = state.apply_move(*m);
        }
    }

    /// Remove all moves for which the given function returns false
    ///
    /// Eg `db.filter_moves(|x| x.total_count() >= 10);` to filter all moves that occur fewer than
    /// 10 times in the database
    pub fn filter_moves(&mut self, filter: impl Fn(&DbResult) -> bool) {
        for (_position, results) in self.0.iter_mut() {
            let mut i = 0;
            while i < results.len() {
                if filter(&results[i]) {
                    i += 1;
                } else {
                    results.remove(i);
                }
            }
        }
    }

    /// Merge two opening databases into one
    pub fn merge(mut self, mut other: Self) -> Self {
        for (key, other_values) in other.0.drain() {
            if !self.0.contains_key(&key) {
                self.0.insert(key, other_values);
                continue;
            }

            let this_values = self.0.get_mut(&key).unwrap();

            'other_v: for other_v in other_values {
                for this_v in this_values.iter_mut() {
                    if this_v.m == other_v.m {
                        this_v.wins += other_v.wins;
                        this_v.losses += other_v.losses;
                        this_v.draws += other_v.draws;
                        continue 'other_v;
                    }
                }

                this_values.push(other_v);
            }
        }

        self
    }

    /// Prune entries that have no moves left
    pub fn prune(&mut self, threshold: usize) {
        let empties = self
            .0
            .iter()
            .filter(|(_k, v)| v.len() <= threshold)
            .map(|(k, _v)| *k)
            .collect::<HashSet<_>>();

        for e in empties {
            self.0.remove(&e);
        }
    }

    pub fn serialize(&self) -> Result<Vec<u8>> {
        let dat = serde_cbor::to_vec(self)?;
        let compressed_dat = zstd::encode_all(&dat[..], 5)?;
        Ok(compressed_dat)
    }

    pub fn deserialize(data: &[u8]) -> Result<Self> {
        let decompressed_data = zstd::decode_all(data)?;
        let db = serde_cbor::from_slice(&decompressed_data)?;
        Ok(db)
    }

    pub fn query(&self, state: &State) -> &[DbResult] {
        match self.0.get(&state.zobrist) {
            Some(r) => r,
            None => &[],
        }
    }
}
