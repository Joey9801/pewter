# tourney

Pit two UCI engines against each other over many games and estimate which is
stronger. This is the harness for A/B-testing engine changes: build two
versions of `pewter`, point `tourney.py` at both, and read off the score, Elo
difference, and likelihood-of-superiority.

## Setup

Install the dependencies (using [uv](https://github.com/astral-sh/uv)):

```sh
uv sync
```

Build the engine you want to test from the repository root:

```sh
cargo build --release
```

## Running a match

Each engine under test is described by a small JSON file. See
[`engine.example.json`](./engine.example.json):

```json
{
    "name": "pewter",
    "path": "../target/release/pewter-engine",
    "options": {},
    "env": {}
}
```

- `path` is the engine binary. A relative path is resolved relative to the
  directory containing the JSON file.
- `options` are UCI options set via `setoption` before each game.
- `env` are environment variables passed to the engine process.

Copy the example, tweak it per version (different binaries, different options),
then run:

```sh
uv run tourney.py \
    --engine1-def old.json \
    --engine2-def new.json \
    --num-games 200
```

### Options

| Flag            | Default            | Description                                                       |
| --------------- | ------------------ | ----------------------------------------------------------------- |
| `--engine1-def` | *(required)*       | JSON definition of the first engine.                              |
| `--engine2-def` | *(required)*       | JSON definition of the second engine.                             |
| `--num-games`   | `10`               | Total games. Rounded down to an even number (half per colour).    |
| `--concurrency` | *(cpu count)*      | Games to run in parallel.                                         |
| `--tc`          | `60+0`             | Time control per side, `base[+increment]` in seconds.             |
| `--openings`    | *(none)*           | Optional file of opening lines to vary games (see below).         |
| `--db-path`     | `tourney.db`       | SQLite database results are stored in.                            |

Results are appended to the SQLite database, keyed by the engine's name, path,
binary checksum, and options, so repeated runs of the same build accumulate.

## Getting variety between games

`pewter`'s search is deterministic by default, so two games from the same
starting position with the same time control would be identical. There are two
ways to get a spread of distinct games; you can use either or both.

### Engine non-determinism (default)

By default every game starts from the standard position, and variety comes from
the engine itself. `pewter` exposes two UCI options for this:

| Option        | Default | Description                                                          |
| ------------- | ------- | -------------------------------------------------------------------- |
| `Wobble`      | `0`     | Score margin in centipawns. When > 0, the engine plays a random move from among those within this many centipawns of the best. `0` disables it. |
| `WobblePlies` | `0`     | Apply the wobble only for this many opening half-moves (plies), then play the best move.                                                        |

Restricting the randomness to the opening keeps the strength cost small — early
positions have many near-equal moves — while still branching the games apart.
Only one side needs it for a game to diverge, so you can enable it on just the
engine under test. Set them in that engine's JSON, e.g. wobble for the first
eight plies:

```json
{
    "name": "pewter",
    "path": "../target/release/pewter-engine",
    "options": { "Wobble": 30, "WobblePlies": 8 },
    "env": {}
}
```

Note that any non-zero `Wobble` makes the engine play slightly below its true
strength during those opening plies, so the measured Elo gap is a (conservative)
floor rather than an exact figure. If you leave `Wobble` at `0` on both engines
and don't supply openings, every game will be identical.

### Opening lines (`--openings`)

Alternatively, seed each game pair with a distinct opening line played out as
un-timed "book" moves, the same opening played once by each engine as White so
colours are balanced. This is strength-neutral (both engines get the same
positions) but requires maintaining a file of lines.

The bundled [`openings.txt`](./openings.txt) covers a spread of common, balanced
openings. The format is one opening per line, with an optional name before a `|`
and the moves in SAN after it:

```
Ruy Lopez | e4 e5 Nf3 Nc6 Bb5 a6
```

Lines beginning with `#` are comments. Enable them with
`--openings openings.txt`, or provide your own set. For a statistically
meaningful match, supply at least `num-games / 2` openings so that no two games
repeat the same line.

## Output

```
Summary of games between old and new (from old's perspective):
    All games:
        200 games:   72 wins,   56 draws,   72 losses (36.0% / 28.0% / 36.0%)
                  Elo: +0.0 [-40.1, +40.1] (95% CI), LOS: 50.0%
    Games as White:
        ...
    Games as Black:
        ...
```

- **Elo** is the estimated rating difference of engine 1 over engine 2, with a
  95% confidence interval.
- **LOS** (likelihood of superiority) is the probability that engine 1 is
  genuinely stronger given the decisive games.
