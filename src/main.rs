// Many lookup tables are precomputed in const functions at compile time to
// improve runtime performance.
// Could potentially move these precomputations into an "init" style function to
// reduce build times, though this may limit the compiler's opportuities for
// inlining
#![feature(const_eval_limit)]
#![const_eval_limit = "20000000"]

pub mod bitboard;
pub mod chessmove;
pub mod color;
pub mod coordinates;
pub mod io;
pub mod movegen;
pub mod board;
pub mod piece;
pub mod state;

use coordinates::consts::*;

fn main() {
    println!("{}", bitboard::masks::between(B2, G2).pretty_format());
    println!("{}", bitboard::masks::between(B2, B7).pretty_format());
    println!("{}", bitboard::masks::between(C2, F5).pretty_format());
    println!("{}", bitboard::masks::between(B6, G1).pretty_format());
}