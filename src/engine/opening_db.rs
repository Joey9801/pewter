use crate::{Move, State};

pub struct OpeningDb {}

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
        todo!()
    }
    
    pub fn add_games(&mut self, games: &[crate::io::pgn::Game]) {
        todo!()
    }
    
    /// Remove all moves for which the given function returns false
    /// 
    /// Eg `db.filter_moves(|x| x.total_count() < 10);` to filter all moves that occur fewer than
    /// 10 times in the database
    pub fn filter_moves(&mut self, filter: impl Fn(DbResult) -> bool) {
        todo!()
    }
    
    pub fn query(&self, state: &State) -> &[DbResult] {
        todo!();
    }
}