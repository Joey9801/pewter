// Many lookup tables are precomputed in const functions at compile time to
// improve runtime performance.
// Could potentially move these precomputations into an "init" style function to
// reduce build times, though this may limit the compiler's opportuities for
// inlining
#![feature(const_eval_limit)]
#![const_eval_limit = "20000000"]

pub mod engine;

pub use crate::engine::Engine;
