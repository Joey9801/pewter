// Many lookup tables are precomputed in const functions at compile time to
// improve runtime performance.
// Could potentially move these precomputations into an "init" style function to
// reduce build times, though this may limit the compiler's opportuities for
// inlining
#![feature(const_eval_limit)]
#![const_eval_limit = "20000000"]

pub mod bitboard;
pub mod board;
pub mod chessmove;
pub mod color;
pub mod coordinates;
pub mod io;
pub mod movegen;
pub mod piece;
pub mod state;

pub use crate::bitboard::BitBoard;
pub use crate::board::Board;
pub use crate::chessmove::{Move, MoveSet};
pub use crate::color::Color;
pub use crate::coordinates::{BoardPos, File, Rank};
pub use crate::piece::Piece;
pub use crate::state::{CastleSide, CastleRights, State};
