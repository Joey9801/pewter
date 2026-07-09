# Pewter

[![CI](https://github.com/joey9801/pewter/actions/workflows/ci.yml/badge.svg)](https://github.com/joey9801/pewter/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

## Overview

Pewter is a hobby chess engine written from scratch in the Rust programming
language. It is not a complete chess program — it speaks the [UCI
protocol](https://backscattering.de/chess/uci/) over stdin/stdout and is meant
to be driven by a chess GUI or by another program, rather than used directly.

The project is a Cargo workspace:

| Crate                        | Kind    | Description                                                              |
| ---------------------------- | ------- | ----------------------------------------------------------------------- |
| `pewter-core`                | library | Board representation, move generation, FEN/PGN/UCI parsing, Zobrist hashing. |
| `pewter-engine`              | binary  | The UCI engine itself (`pewter-engine`), plus search and evaluation.    |
| `pewter-opening-db-builder`  | binary  | Scrapes games and builds the opening book used by the engine.           |
| `pewter-search-debugger`     | binary  | Runs a single search on a position for debugging, without the UCI loop. |
| `pewter-stockfish-comparer`  | binary  | Compares Pewter's evaluation/search against Stockfish.                  |

## Building

Pewter targets a recent stable Rust toolchain and uses the 2024 edition, so it
needs Rust 1.85 or newer. Install Rust via [rustup](https://rustup.rs/) if you
don't already have it.

```sh
# Debug build of everything
cargo build --workspace

# Optimised build (use this for actually playing — it is dramatically stronger
# per unit time than the debug build)
cargo build --release
```

The release engine binary is written to `target/release/pewter-engine`.

## Testing

```sh
# Run the test suite
cargo test --workspace

# Formatting, lints and micro-benchmarks
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo bench -p pewter-core
```

These are the same checks the [CI workflow](.github/workflows/ci.yml) runs on
every push and pull request.

## Using the engine

`pewter-engine` is a UCI engine: it reads UCI commands on stdin and writes
responses on stdout. You can talk to it by hand to sanity-check it:

```sh
$ cargo run --release --bin pewter-engine
uci
# ... engine identifies itself and lists its options, ending with `uciok`
position startpos moves e2e4 e7e5
go depth 8
# ... engine searches and replies with `bestmove ...`
quit
```

Supported UCI options include `Hash` (transposition table size in MiB) and the
`Wobble` / `WobblePlies` pair used to add controlled randomness to the opening
(handy for generating varied self-play games).

### Playing in a chess GUI

Any UCI-compatible GUI can drive Pewter. Build a release binary and register
`target/release/pewter-engine` as a new UCI engine in your GUI of choice, for
example:

- [Cute Chess](https://cutechess.com/)
- [Arena](http://www.playwitharena.de/)
- [Banksia GUI](https://banksiagui.com/)
- [En Croissant](https://encroissant.org/)

The GUI handles the board, clocks and opponent; Pewter just needs the path to
the binary.

## Comparing two builds

The [`tourney/`](./tourney) directory contains a Python harness for A/B-testing
engine changes: it plays two UCI engines against each other over many games and
reports the score, Elo difference and likelihood of superiority. See
[`tourney/README.md`](./tourney/README.md) for details.

## Repository layout

- `crates/` — the Rust workspace (see the table above).
- `tourney/` — Python tournament harness for comparing engine builds.
- `docs/` — design notes, including the engine-strength roadmap.

## License

Pewter is distributed under the terms of the [MIT license](LICENSE).
