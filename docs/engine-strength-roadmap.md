# Pewter engine strength roadmap

> Persisted implementation plan for a future agent/session. The near-term work
> is the "Tier 0" correctness + time-management bundle below; Tiers 1–2 are the
> longer roadmap. All changes are meant to be A/B-gated with the tourney harness
> (`tourney/tourney.py`, Elo ± 95% CI). Line numbers are from the state of the
> tree when this was written and may drift — re-locate by symbol if so.

## Context

Pewter has a working negamax/alpha-beta search with iterative deepening, a
transposition table, MVV-LVA + hash-move ordering, a quiescence search, and a
material + piece-square-table evaluation (plus bishop-pair, a nonlinear trading
bonus, and an endgame king-push term).

Four low-effort, high-impact defects hold it well below its potential:

1. **Time management is broken.** Per-move budget is `min(remaining/10,
   move_time)` and `move_time` defaults to 250 ms (`search.rs:136-145`), so the
   engine plays a flat ~250 ms/move even with 60 s on the clock. A second,
   hardcoded 500 ms check lives in `should_stop` (`search.rs:313-322`),
   independent of the clock; increments are never used. Largest real-game gain.
2. **No repetition / 50-move draw detection in search.** The engine can't tell
   it is repeating (→ rook-shuffle draws in self-play). `State` already carries
   `halfmove_clock` and `zobrist`; the searcher keeps no path/history.
3. **No mate-distance scoring.** `MATE` is a fixed constant returned with no ply
   adjustment (`search.rs:226`); can't prefer mate-in-1 over mate-in-5 and stores
   wrong mate values in the TT.
4. **TT rebuilt every move**, pre-allocates capacity for 100M entries
   (`transposition.rs:35-37`), evicts randomly in O(n) (`transposition.rs:68-73`),
   no size control. A persistent, fixed-size, depth-preferred table is near-free
   depth.

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

- **eval.rs**: add to `mod consts`: `MAX_MATE_PLY: Evaluation = 1024` and
  `MATE_THRESHOLD = -MATE - MAX_MATE_PLY` (large positive). Module helpers
  `is_mate_score(e)` (`e.abs() >= MATE_THRESHOLD`) and `mate_in_moves(e) ->
  Option<i32>` (plies `= -MATE - e.abs()`, moves `= (plies+1)/2`, signed by `e`).
- **search.rs**: checkmate return (`:226`) → `MATE + ply_from_root as Evaluation`
  (mated deeper = less bad → fastest mate / longest defence). Stalemate stays
  `DRAW`.
- **search.rs (TT normalisation, keep TT dumb)**: before each `t_table.insert`
  (`:248`, `:272`) adjust a mate score to distance-from-node (`if score>0
  {score+ply} else {score-ply}`); after a successful `probe` (`:211`) apply the
  inverse. `ordering.rs` only reads `.m` — no change.
- **main.rs (optional)**: once Stage 3 surfaces the root score, emit
  `InfoMessage { score: Some(InfoScore { mate: mate_in_moves(score)... }) }`.
  Protocol layer already formats `score mate N` (`uci.rs:195-205`).

Risk: keep the mate band (~1e9) clear of normal eval magnitudes — safe by
construction.

## Stage 2 — Repetition + 50-move draw detection

Game history is discarded today at `main.rs:98-99`; plumb it through.

- **zobrist.rs**: `ZobristHash.0` is private — add `pub const fn get(self) ->
  u64` (reused by Stage 4 indexing).
- **main.rs `Position`**: collect zobrist of the start position and every
  intermediate (`let mut history = vec![state.zobrist]; for m in moves { state =
  state.apply_move(m); history.push(state.zobrist); }`); call
  `engine.set_state(state, history)`.
- **engine_server.rs**: `EngineCommand::SetState(State, Vec<ZobristHash>)` +
  matching `set_state`.
- **engine/mod.rs**: add `game_history: Vec<ZobristHash>` to `Engine`; store in
  `set_board_state`; pass into searcher.
- **search.rs**: add `game_history` + `path: Vec<ZobristHash>` (reset `path` per
  `search()`). Top of `search_moves`, guarded by `ply_from_root > 0`: return
  `DRAW` if `state.zobrist` in `path` or `game_history` (twofold-within-search;
  scan back only `halfmove_clock` entries). Push/pop `state.zobrist` around the
  child loop, symmetric across returns. After the `moves.len()==0` block
  (`:224-230`, mate resolved first): `if state.halfmove_clock >= 100 { DRAW }`.
- **TT interaction**: repetition-draw nodes return *before* any `insert`, so a
  path-dependent draw is never cached. Quiescence needs no repetition handling.

Risk: history off-by-one (root included once; `ply>0` guard prevents root
self-match).

## Stage 3 — Time-management rewrite

- **search.rs** allocator → `TimeBudget { soft, hard }`:
  - `move_time` given → `soft = hard = move_time - OVERHEAD`.
  - else clock given → `soft = rem/25 + inc*3/4`, `hard = min(rem/5, soft*3)`,
    subtract `OVERHEAD` (~30 ms), clamp `MIN_MOVE (5ms) <= soft <= hard`.
  - else (no clock/movetime, not infinite) → `None`, depth-limited only.
- Replace ad-hoc timing with `soft_deadline`/`hard_deadline: Option<Instant>`,
  `infinite: bool`, `aborted: bool`, `nodes_since_check: u32`. **Delete** the
  250 ms heuristic (`:136-145`) and the 500 ms check in `should_stop`.
- Rewrite `should_stop()` to poll on a node counter (~every 2048 nodes) at **all
  depths** (remove `>=4` / `ply==0` gating): abort on stop flag or `hard_deadline`
  passed; latch `aborted`.
- ID loop: capture a fallback legal move before looping (abort during depth 1
  still yields a legal bestmove); `if aborted break` **before** committing
  `last_pv` (never play from a partial depth); stop before starting a new depth
  once `soft_deadline` passed. Keep explicit `depth`/`infinite` modes working.
- Cheap extras: honor the ignored `max_nodes` (`mod.rs:112`) in `should_stop`;
  surface the root score from `search()` for Stage 1 reporting.

Risk: never skip depth 1; ensure `soft <= hard` after clamp; reset
`nodes_since_check` per search.

## Stage 4 — TT rework + persistence + `Hash` option

- **transposition.rs (rewrite)**: fixed-size `Vec<Option<Entry>>`, power-of-two
  `capacity`, entry stores full `key: ZobristHash` (collision check) +
  `generation: u8`. `index = zobrist.get() as usize & (capacity-1)`. Keep
  existing depth/bound logic in `probe`. Replacement: write if empty, same key,
  stale generation, or `new.depth >= slot.depth`. API: `with_mb`, `resize`,
  `clear`, `new_generation`, `load`. Drop `HashMap`, the 100M pre-alloc, `rand`.
- **Persist on `Engine`**: move `t_table` onto `Engine` (init
  `with_mb(DEFAULT_HASH_MB=128)`); `search_best_move` calls `new_generation()`
  then builds `Searcher<'a>` borrowing `&'a mut TranspositionTable`. Remove
  `new_empty()` from `Searcher::new` (`search.rs:120`). Existing call-sites and
  `order_moves(&TranspositionTable)` unchanged.
- **`Hash` UCI option** (only `debug` exists, `main.rs:11-43`): add `hash_mb:
  usize` (default 128), an `OptionType::Spin` entry (min 1, max 4096), parse in
  `set_value`. **Wire the currently-dropped commands** in `handle_uci_cmd`
  (`main.rs:120` `_ => ()`): `SetOption { "Hash" }` → `engine.set_hash_size(mb)`;
  `UciNewGame` → `engine.new_game()` (clear TT). Add
  `EngineCommand::SetHashSize`/`NewGame` + handlers in `engine_server.rs`.
- **Minor fix**: `hashfull` is per-mille; `main.rs:130` should be `(load *
  1000.0) as u16`, not `* 100000`.

Risk: power-of-two capacity required for mask indexing (`capacity >= 1`);
`generation: u8` wraps harmlessly.

---

## Verification

**Unit tests** (per module `#[cfg(test)]`):
- Mate: mate-in-1/-2 FENs → `is_mate_score` true, `mate_in_moves` 1/2, bestmove
  is mating move; forced-mate-against → longest-defence move.
- Repetition: seed `game_history`, search repeating state → `DRAW`; path-only
  twofold too. 50-move: `halfmove_clock=100` + legal moves → `DRAW`; + checkmate
  on board → still mate.
- TimeBudget: movetime → `move_time-overhead`; `soft<=hard`, `hard<=rem/5` for
  60s+0 / 10s+0.1 / 100ms+0; `None` with no clock/movetime.
- TT: insert→probe round-trip; same-index different-key → miss; depth-preferred
  replacement keeps deeper; `new_generation` frees stale; `with_mb`/`resize`
  power-of-two capacity.

**No movegen regression**: `cargo test -p pewter-core` (includes perft).

**UCI smoke test** (pipe stdin to `target/release/pewter-engine`): `uci` shows
`option name Hash type spin` + `uciok`; `setoption name Hash value 64` /
`ucinewgame` / `isready`; `position startpos moves e2e4 e7e5` + `go wtime 60000
btime 60000 winc 100 binc 100` returns a bestmove in ~soft time (seconds, much
deeper than the old 250 ms); `go movetime 1000` ≈ 1 s; `go infinite` then `stop`
returns promptly with a legal move.

**A/B measurement** (the point of the bundle): build prev and new binaries, two
engine JSONs pointing at each `target/release/pewter-engine` (Stage 4 sets
`"options": {"Hash": 64}`):
```
python tourney/tourney.py --engine1-def prev.json --engine2-def stage.json \
  --num-games 1000 --tc 10+0.1 --concurrency <cores>
```
Harness prints `Elo: +X [lo, hi] (95% CI)`. Gate each stage on **CI lower bound >
0** over a large batch (1000–2000 games), tested against the immediately
preceding binary.

## Later tiers (out of scope for Tier 0)

- **Tier 1 — search features**: killer/history ordering, null-move pruning, LMR,
  PVS + aspiration windows, check extensions, quiescence hardening (currently
  misses en-passant captures; no SEE/delta pruning).
- **Tier 2 — evaluation**: tapered midgame/endgame eval, king safety + a real
  king PST (currently zero), pawn structure (doubled/isolated/passed), mobility,
  rook-on-open-file, tempo.
