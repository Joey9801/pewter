use std::{collections::HashMap};

use crate::{Move, State, state::GameResult, Color};

pub struct OpeningDb(HashMap<u64, Vec<DbResult>>);

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
    
    pub fn add_game(&mut self, game: crate::io::pgn::Game) {
        let mut state = game.initial;

        // Be a little defensive
        state.zobrist = crate::zobrist::calculate_entire_zobrist(&state);

        for m in game.moves {
            let existing_set = self.0.entry(state.zobrist)
                .or_insert(Vec::new());

            let result = match existing_set.iter().position(|r| r.m == m) {
                Some(idx) => existing_set.get_mut(idx).unwrap(),
                None => {
                    existing_set.push(DbResult {
                        m,
                        wins: 0,
                        draws: 0,
                        losses: 0
                    });

                    existing_set.last_mut().unwrap()
                },
            };

            match (state.to_play, game.result) {
                (Color::White, GameResult::WhiteWin) |
                (Color::Black, GameResult::BlackWin) => result.wins += 1,
                (Color::White, GameResult::BlackWin) |
                (Color::Black, GameResult::WhiteWin) => result.losses += 1,
                (_, GameResult::Draw) => result.draws += 1,
                (_, GameResult::Ongoing) => panic!("Can't add an ongoing game to the opening DB"),
            }
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
    
    pub fn query(&self, state: &State) -> &[DbResult] {
        match self.0.get(&state.zobrist) {
            Some(r) => r,
            None => &[],
        }
    }
}