# Pewter engine strength roadmap

> Persisted implementation plan for a future agent/session. The near-term work
> is the "Tier 0" correctness + time-management bundle below; Tiers 1–2 are the
> longer roadmap. All changes are meant to be A/B-gated with the tourney harness
> (`tourney/tourney.py`, Elo ± 95% CI). Line numbers are from the state of the
> tree when this was written and may drift — re-locate by symbol if so.
>
> **Status:** Tier 0 (Stages 1–4) is implemented. Tiers 1–2 remain future work.

## Context

Pewter has a working negamax/alpha-beta search with iterative deepening, a
transposition table, MVV-LVA + hash-move ordering, a quiescence search, and a
material + piece-square-table evaluation (plus bishop-pair, a nonlinear trading
bonus, and an endgame king-push term).

Four low-effort, high-impact defects hold it well below its potential:

1. **Time management is broken.** Per-move budget is `min(remaining/10,
   move_time)` and `move_time` defaults to 250 ms, so the engine plays a flat
   ~250 ms/move even with 60 s on the clock. A second, hardcoded 500 ms check
   lives in `should_stop`, independent of the clock; increments are never used.
   Largest real-game gain.
2. **No repetition / 50-move draw detection in search.** The engine can't tell
   it is repeating (→ rook-shuffle draws in self-play). `State` already carries
   `halfmove_clock` and `zobrist`; the searcher keeps no path/history.
3. **No mate-distance scoring.** `MATE` is a fixed constant returned with no ply
   adjustment; can't prefer mate-in-1 over mate-in-5 and stores wrong mate
   values in the TT.
4. **TT rebuilt every move**, pre-allocates capacity for 100M entries, evicts
   randomly in O(n), no size control. A persistent, fixed-size, depth-preferred
   table is near-free depth.

Tier 0 is pure engine work (no eval changes).

## Staging

Four independent commits, each A/B-tested against the previous binary. Order:
cheap correctness first so the strength stages don't amplify latent bugs.

1. Mate-distance scoring (tiny; prerequisite for Stage 4's TT mate adjustment).
2. Repetition + 50-move detection (needs history plumbing).
3. Time-management rewrite (largest single strength gain).
4. TT rework + persistence + `Hash` UCI option.

Stages 1→2→3 are independent; Stage 4's TT mate normalisation depends on Stage 1.

Critical files: `crates/pewter-engine/src/engine/search.rs`,
`.../engine/transposition.rs`, `.../engine/mod.rs`,
`crates/pewter-engine/src/main.rs`, `.../engine/engine_server.rs`,
`crates/pewter-core/src/zobrist.rs`.

---

## Stage 1 — Mate-distance scoring

`Evaluation = i32`, `MATE = NEG_INFINITY/2` (~ -1.07e9) — huge headroom for a
mate band.

- **eval.rs**: `MAX_MATE_PLY = 1024`, `MATE_THRESHOLD = -MATE - MAX_MATE_PLY`,
  and helpers `is_mate_score(e)` / `mate_in_moves(e)`.
- **search.rs**: checkmate return → `MATE + ply_from_root` (mated deeper = less
  bad → fastest mate / longest defence). Stalemate stays `DRAW`.
- **search.rs (TT normalisation)**: before each `insert` adjust a mate score to
  distance-from-node (`if score>0 {score+ply} else {score-ply}`); after a
  successful `probe` apply the inverse.
- **main.rs**: emit `score mate N` in the info output via `mate_in_moves`.

## Stage 2 — Repetition + 50-move draw detection

- **zobrist.rs**: `pub const fn get(self) -> u64`.
- **main.rs `Position`**: collect the zobrist of the start position and every
  intermediate move, and pass it via `set_state`.
- **engine_server.rs**: `EngineCommand::SetState(State, Vec<ZobristHash>)`.
- **engine/mod.rs**: `game_history: Vec<ZobristHash>` on `Engine`.
- **search.rs**: `game_history` + a per-search `path`. At the top of
  `search_moves` (guarded by `ply_from_root > 0`) return `DRAW` on a repetition
  within the last `halfmove_clock` plies. After mate/stalemate resolution,
  return `DRAW` if `halfmove_clock >= 100`.

## Stage 3 — Time-management rewrite

- **search.rs** allocator → `TimeBudget { soft, hard }`:
  - `move_time` given → `soft = hard = move_time - OVERHEAD`.
  - else clock given → `soft = rem/25 + inc*3/4`, `hard = min(rem/5, soft*3)`,
    subtract `OVERHEAD` (~30 ms), clamp `MIN_MOVE (5ms) <= soft <= hard`.
  - else → `None`, depth-limited only.
- `soft_deadline`/`hard_deadline`, `infinite`, `aborted`, `nodes_since_check`.
- `should_stop()` polls on a node counter (~every 2048 nodes) at all depths.
- ID loop: capture a fallback legal move; discard an aborted (partial) depth;
  stop before starting a new depth once the soft deadline passed. Honour
  `max_nodes`; surface the root score for reporting.

## Stage 4 — TT rework + persistence + `Hash` option

- **transposition.rs**: fixed-size `Vec<Option<Entry>>`, power-of-two capacity,
  full-key collision check + `generation`. Depth-preferred replacement (empty,
  same key, stale generation, or `new.depth >= slot.depth`). `with_mb`, `resize`,
  `clear`, `new_generation`.
- **Persist on `Engine`**: `search_best_move` bumps `new_generation()` then
  builds the `Searcher` borrowing the table. Never take a TT early-return at the
  root (it carries no PV).
- **`Hash` UCI option** + wire `SetOption { "Hash" }` → `set_hash_size` and
  `UciNewGame` → `new_game` (clear TT). `hashfull` reported per-mille.

---

## Verification

- **Unit tests** per module (mate helpers, repetition/fifty-move, `TimeBudget`,
  TT round-trip/replacement/generation).
- **No movegen regression**: `cargo test -p pewter-core` (includes perft).
- **UCI smoke test**: `uci` advertises `option name Hash type spin` + `uciok`;
  `setoption`/`ucinewgame`/`isready`; clock-based `go` searches for seconds (much
  deeper than the old 250 ms); `go movetime 1000` ≈ 1 s; `go infinite` + `stop`
  returns a legal move promptly.
- **A/B measurement**: build prev and new binaries and run
  `python tourney/tourney.py --engine1-def prev.json --engine2-def stage.json
  --num-games 1000 --tc 10+0.1 --concurrency <cores>`. Gate each stage on the
  95% CI lower bound > 0 against the immediately preceding binary.

## Later tiers (out of scope for Tier 0)

- **Tier 1 — search features**: killer/history ordering, null-move pruning, LMR,
  PVS + aspiration windows, check extensions, quiescence hardening (currently
  misses en-passant captures; no SEE/delta pruning).
- **Tier 2 — evaluation**: tapered midgame/endgame eval, king safety + a real
  king PST (currently zero), pawn structure (doubled/isolated/passed), mobility,
  rook-on-open-file, tempo.
