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
| `--openings`    | `openings.txt`     | File of opening lines used to vary games (see below).             |
| `--db-path`     | `tourney.db`       | SQLite database results are stored in.                            |

Results are appended to the SQLite database, keyed by the engine's name, path,
binary checksum, and options, so repeated runs of the same build accumulate.

## Openings

`pewter`'s search is deterministic, so without variety every game from the
start position would be identical. Each game pair is therefore seeded with a
distinct opening line played out as un-timed "book" moves, and the same opening
is played once by each engine as White so colours are balanced.

The bundled [`openings.txt`](./openings.txt) covers a spread of common,
balanced openings. The format is one opening per line, with an optional name
before a `|` and the moves in SAN after it:

```
Ruy Lopez | e4 e5 Nf3 Nc6 Bb5 a6
```

Lines beginning with `#` are comments. Provide your own set with `--openings`.
For a statistically meaningful match, supply at least `num-games / 2` openings
so that no two games repeat the same line.

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
