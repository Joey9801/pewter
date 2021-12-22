use std::{env, fs::File, io::Write, path::Path};

use anyhow::Result;
use rand::prelude::*;

fn main() {
    generate_zobrist_numbers().unwrap();
}

fn generate_zobrist_numbers() -> Result<()> {
    let out_dir = env::var("OUT_DIR")?;
    let zobrist_path = Path::new(&out_dir).join("zobrist_gen.rs");
    let mut f = File::create(&zobrist_path)?;

    let mut rng = SmallRng::seed_from_u64(0xDEADBEEF12345678);

    writeln!(
        f,
        "/// A zobrist number for when the current player is White"
    )?;
    writeln!(
        f,
        "pub const ZOBRIST_WHITE_TURN: u64 = {};\n",
        rng.next_u64()
    )?;

    let num_colors = 2;
    let num_pieces = 6;
    let num_positions = 64;
    let psc_count = num_colors * num_pieces * num_positions;
    writeln!(
        f,
        "/// A single array to hold all of the piece/square/color zobrist numbers"
    )?;
    writeln!(f, "/// The index of a given piece/pos/color is:")?;
    writeln!(f, "///     color.to_num() * num_pieces * num_positions")?;
    writeln!(f, "///     + piece.to_num * num_positions")?;
    writeln!(f, "///     + pos.to_bitboard_offset()")?;
    write!(f, "pub const ZOBRIST_PSC: [u64; {}] = [\n", psc_count)?;
    for _ in 0..(psc_count / 4) {
        let a = rng.next_u64();
        let b = rng.next_u64();
        let c = rng.next_u64();
        let d = rng.next_u64();
        write!(f, "    {a:0>20}, {b:0>20}, {c:0>20}, {d:0>20},\n")?;
    }
    write!(f, "];\n\n").unwrap();

    writeln!(
        f,
        "/// One zobrist number for each of the 16 possible castling rights combinations"
    )?;
    write!(f, "pub const ZOBRIST_CASTLING: [u64; 16] = [\n")?;
    for _ in 0..4 {
        let a = rng.next_u64();
        let b = rng.next_u64();
        let c = rng.next_u64();
        let d = rng.next_u64();
        write!(f, "    {a:0>20}, {b:0>20}, {c:0>20}, {d:0>20},\n")?;
    }
    write!(f, "];\n\n").unwrap();

    writeln!(
        f,
        "/// One zobrist number for each file that could be en-passant"
    )?;
    write!(f, "pub const ZOBRIST_EP: [u64; 8] = [\n")?;
    for _ in 0..2 {
        let a = rng.next_u64();
        let b = rng.next_u64();
        let c = rng.next_u64();
        let d = rng.next_u64();
        write!(f, "    {a:0>20}, {b:0>20}, {c:0>20}, {d:0>20},\n")?;
    }
    write!(f, "];\n\n").unwrap();

    Ok(())
}
