# Pit two engines against each other and determine the strongest

import argparse
import hashlib
from datetime import datetime, timedelta, timezone
import enum
import math
import multiprocessing
import sqlite3
from multiprocessing import Pool
from pathlib import Path

import chess
import chess.engine
import chess.pgn
from pydantic import BaseModel
from tqdm import tqdm


ConfigValue = str | bool | int | None


class EngineDef(BaseModel):
    name: str
    path: Path
    """Path to the engine binary.
    
    If a relative path is serialized, it will be interpreted as relative to the
    directory containing the serialized file. Eg if the file
    '/path/to/engine.json' contained the engine path './foo', it would refer to
    '/path/to/foo'.
    """

    options: dict[str, ConfigValue]
    env: dict[str, str]

    @classmethod
    def read(cls, path: Path) -> "EngineDef":
        content = path.read_text()
        engine = EngineDef.model_validate_json(content)
        if not engine.path.is_absolute():
            engine.path = (path.parent / engine.path).absolute().resolve()
        return engine


class EndingType(enum.StrEnum):
    CHECKMATE = "Checkmate"
    STALEMATE = "Stalemate"
    FIVEFOLD_REP = "Fivefold repetition"
    INSUFFICIENT_MAT = "Insufficient material"
    SEVENTY_FIVE_MOVE = "Seventy-five move rule"
    CLOCK = "Clock ran out"
    UNKNOWN = "Unknown"


class ChessClock:
    remaining: timedelta
    increment: timedelta
    started_at: datetime | None

    def __init__(self, starting: timedelta, increment: timedelta = timedelta(0)):
        self.remaining = starting
        self.increment = increment
        self.started_at = None

    @property
    def remaining_seconds(self) -> float:
        return self.remaining.total_seconds()

    @property
    def increment_seconds(self) -> float:
        return self.increment.total_seconds()

    def start(self):
        assert self.started_at is None
        self.started_at = datetime.now(tz=timezone.utc)

    def stop(self):
        assert self.started_at is not None
        now = datetime.now(tz=timezone.utc)
        diff = now - self.started_at
        self.remaining -= diff
        self.started_at = None

    def add_increment(self):
        """Apply the Fischer increment after a completed (non-flagging) move."""
        self.remaining += self.increment


class TimeControl(BaseModel):
    """A sudden-death or Fischer time control, per side."""

    base: timedelta
    increment: timedelta

    @classmethod
    def parse(cls, spec: str) -> "TimeControl":
        """Parse a "base[+increment]" spec, in seconds. Eg "60", "60+1", "90+0.5"."""
        spec = spec.strip()
        if "+" in spec:
            base_str, inc_str = spec.split("+", 1)
        else:
            base_str, inc_str = spec, "0"

        try:
            base = float(base_str)
            increment = float(inc_str)
        except ValueError as e:
            raise ValueError(
                f"Invalid time control {spec!r}, expected 'base[+increment]' in seconds"
            ) from e

        if base <= 0:
            raise ValueError(f"Time control base must be positive, got {base}")
        if increment < 0:
            raise ValueError(
                f"Time control increment must be non-negative, got {increment}"
            )

        return cls(
            base=timedelta(seconds=base),
            increment=timedelta(seconds=increment),
        )

    def __str__(self) -> str:
        return f"{self.base.total_seconds():g}+{self.increment.total_seconds():g}"


class Opening(BaseModel):
    """A named opening line, given as the moves to play out from the start position.

    An empty ``moves`` list means the game simply starts from the standard
    position, which is the default when no openings file is supplied.
    """

    name: str
    moves: list[str]
    """The opening moves, in UCI long-algebraic notation."""


def load_openings(path: Path) -> list[Opening]:
    """Load opening lines from a text file.

    Each non-empty, non-comment (``#``) line describes one opening. The optional
    text before a ``|`` names the opening; the remainder is a sequence of
    space-separated moves in SAN (eg ``e4 e5 Nf3 Nc6``). Moves are validated and
    normalised to UCI as they are loaded, so a malformed line fails fast with a
    pointer to the offending line.
    """

    openings: list[Opening] = []
    for lineno, raw in enumerate(path.read_text().splitlines(), start=1):
        line = raw.split("#", 1)[0].strip()
        if not line:
            continue

        if "|" in line:
            name, moves_str = line.split("|", 1)
            name = name.strip()
        else:
            name, moves_str = "", line

        board = chess.Board()
        uci_moves: list[str] = []
        for token in moves_str.split():
            try:
                move = board.push_san(token)
            except ValueError as e:
                raise ValueError(
                    f"{path}:{lineno}: could not parse move {token!r} in opening line: {e}"
                ) from e
            uci_moves.append(move.uci())

        if not uci_moves:
            raise ValueError(f"{path}:{lineno}: opening line has no moves")

        openings.append(
            Opening(name=name or f"Opening {len(openings) + 1}", moves=uci_moves)
        )

    if not openings:
        raise ValueError(f"No openings found in {path}")

    return openings


DEFAULT_OPENINGS_PATH = Path(__file__).parent / "openings.txt"


CREATE_TABLES_SQL = """
CREATE TABLE IF NOT EXISTS ending_types (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE
);

CREATE TABLE IF NOT EXISTS engines (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    path TEXT NOT NULL,
    checksum TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS engine_options (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    engine_id INTEGER NOT NULL,
    option_name TEXT NOT NULL,
    option_value TEXT NOT NULL,
    FOREIGN KEY (engine_id) REFERENCES engines (id)
);

CREATE TABLE IF NOT EXISTS games (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    white_engine_id INTEGER NOT NULL,
    black_engine_id INTEGER NOT NULL,
    white_score REAL NOT NULL,
    black_score REAL NOT NULL,
    ending_type_id INTEGER NOT NULL,
    pgn TEXT NOT NULL,
    FOREIGN KEY (white_engine_id) REFERENCES engines (id),
    FOREIGN KEY (black_engine_id) REFERENCES engines (id),
    FOREIGN KEY (ending_type_id) REFERENCES ending_types (id)
);
"""


# Insert ending types into the database
def insert_ending_types(db_path: Path):
    conn = sqlite3.connect(db_path)
    cur = conn.cursor()
    for ending in EndingType:
        cur.execute(
            "INSERT OR IGNORE INTO ending_types (name) VALUES (?)", (str(ending),)
        )
    conn.commit()


def get_ending_type_id(db_path: Path, ending: EndingType) -> int:
    conn = sqlite3.connect(db_path)
    cur = conn.cursor()
    cur.execute("SELECT id FROM ending_types WHERE name = (?)", (str(ending),))

    return cur.fetchone()[0]


# Compute checksum (MD5) for the engine binary
def compute_checksum(file_path: Path) -> str:
    hash_md5 = hashlib.md5()
    with open(file_path, "rb") as f:
        for chunk in iter(lambda: f.read(4096), b""):
            hash_md5.update(chunk)
    return hash_md5.hexdigest()


def _get_engine(conn: sqlite3.Connection, engine_def: EngineDef) -> int | None:
    checksum = compute_checksum(engine_def.path)
    cur = conn.cursor()

    cur.execute(
        "SELECT id FROM engines WHERE name = ? and path = ? and checksum = ?",
        (engine_def.name, str(engine_def.path), checksum),
    )

    # The set of engines with the right name/path/checksum
    candidates = (row[0] for row in cur.fetchmany())

    def check_options(engine_id: int) -> bool:
        """Does the given engine id have exactly the right options"""

        cur.execute(
            "SELECT option_name, option_value FROM engine_options WHERE engine_id = ?",
            (engine_id,),
        )
        db_options = dict(cur.fetchmany())
        def_options = {k: str(v) for k, v in engine_def.options.items()}
        return db_options == def_options

    candidates = list(filter(check_options, candidates))

    if len(candidates) == 0:
        return None
    elif len(candidates) == 1:
        return candidates[0]
    else:
        msg = f"Found multiple identical engines in DB: {candidates}"
        raise RuntimeError(msg)


def _insert_engine(conn: sqlite3.Connection, engine_def: EngineDef) -> int:
    checksum = compute_checksum(engine_def.path)

    cur = conn.cursor()

    cur.execute(
        "INSERT OR IGNORE INTO engines (name, path, checksum) VALUES (?, ?, ?)",
        (engine_def.name, str(engine_def.path), checksum),
    )
    conn.commit()

    cur.execute(
        "SELECT id FROM engines WHERE name = ? AND path = ?",
        (engine_def.name, str(engine_def.path)),
    )
    engine_id = cur.fetchone()[0]

    # Insert UCI options for the engine
    for option_name, option_value in engine_def.options.items():
        cur.execute(
            "INSERT INTO engine_options (engine_id, option_name, option_value) VALUES (?, ?, ?)",
            (engine_id, option_name, str(option_value)),
        )

    return engine_id


# Insert engine into the database
def get_or_insert_engine(db_path: Path, engine_def: EngineDef) -> int:
    conn = sqlite3.connect(db_path)

    engine_id = _get_engine(conn, engine_def) or _insert_engine(conn, engine_def)

    conn.commit()
    conn.close()

    return engine_id


# Insert game result into the database
def insert_game_result(
    db_path: Path,
    white_engine_id: int,
    black_engine_id: int,
    white_score: float,
    black_score: float,
    ending_type_id: int,
    pgn_str: str,
):
    conn = sqlite3.connect(db_path)
    cur = conn.cursor()
    cur.execute(
        "INSERT INTO games (white_engine_id, black_engine_id, white_score, black_score, ending_type_id, pgn) VALUES (?, ?, ?, ?, ?, ?)",
        (
            white_engine_id,
            black_engine_id,
            white_score,
            black_score,
            ending_type_id,
            pgn_str,
        ),
    )
    conn.commit()
    conn.close()


# Play a single game between two engines
def play_game(
    db_path: Path,
    white_engine_def: EngineDef,
    white_engine_id: int,
    black_engine_def: EngineDef,
    black_engine_id: int,
    time_control: TimeControl,
    opening: Opening,
):
    white_engine = chess.engine.SimpleEngine.popen_uci(
        white_engine_def.path, env=white_engine_def.env
    )
    white_engine.configure(white_engine_def.options)
    white_clock = ChessClock(time_control.base, time_control.increment)

    black_engine = chess.engine.SimpleEngine.popen_uci(
        black_engine_def.path, env=black_engine_def.env
    )
    black_engine.configure(black_engine_def.options)
    black_clock = ChessClock(time_control.base, time_control.increment)

    board = chess.Board()
    pgn = chess.pgn.Game()

    pgn.headers["White"] = f"{white_engine_def.name} (id: {white_engine_id})"
    pgn.headers["Black"] = f"{black_engine_def.name} (id: {black_engine_id})"
    pgn.headers["Opening"] = opening.name

    # Play out any opening line as un-timed "book" moves so that otherwise
    # deterministic engines produce a variety of games. This is empty in the
    # default configuration, where games start from the standard position and
    # variety comes from engine non-determinism instead.
    for uci in opening.moves:
        move = chess.Move.from_uci(uci)
        pgn = pgn.add_main_variation(move)
        board.push(move)

    with white_engine, black_engine:
        while not board.is_game_over():
            if board.turn == chess.WHITE:
                engine, clock = white_engine, white_clock
            else:
                engine, clock = black_engine, black_clock

            limit = chess.engine.Limit(
                white_clock=white_clock.remaining_seconds,
                black_clock=black_clock.remaining_seconds,
                white_inc=white_clock.increment_seconds,
                black_inc=black_clock.increment_seconds,
            )
            clock.start()
            result = engine.play(board, limit=limit)
            clock.stop()

            if clock.remaining_seconds < 0:
                break

            clock.add_increment()

            pgn = pgn.add_main_variation(result.move)
            board.push(result.move)

        # Determine the result and ending kind of the game
        if board.is_checkmate():
            white_score, black_score = (0, 1) if board.turn == chess.WHITE else (1, 0)
            ending_type = EndingType.CHECKMATE
        elif black_clock.remaining_seconds < 0:
            white_score, black_score = (1, 0)
            ending_type = EndingType.CLOCK
        elif white_clock.remaining_seconds < 0:
            white_score, black_score = (0, 1)
            ending_type = EndingType.CLOCK
        elif board.is_stalemate():
            white_score, black_score = 0.5, 0.5
            ending_type = EndingType.STALEMATE
        elif board.is_fivefold_repetition():
            white_score, black_score = 0.5, 0.5
            ending_type = EndingType.FIVEFOLD_REP
        elif board.is_insufficient_material():
            white_score, black_score = 0.5, 0.5
            ending_type = EndingType.INSUFFICIENT_MAT
        elif board.is_seventyfive_moves():
            white_score, black_score = 0.5, 0.5
            ending_type = EndingType.SEVENTY_FIVE_MOVE
        else:
            white_score, black_score = 0.5, 0.5
            ending_type = EndingType.UNKNOWN

        ending_type_id = get_ending_type_id(db_path, ending_type)

        # Store game in PGN format
        pgn.root().headers["Result"] = f"{white_score}-{black_score}"
        pgn_str = str(pgn.root())

        insert_game_result(
            db_path,
            white_engine_id=white_engine_id,
            black_engine_id=black_engine_id,
            white_score=white_score,
            black_score=black_score,
            ending_type_id=ending_type_id,
            pgn_str=pgn_str,
        )


# Function to play games in parallel using multiprocessing
def play_games_parallel(args: dict[str, any]):
    play_game(*args)


def elo_diff_and_ci(wins: int, draws: int, losses: int) -> tuple[float, float, float]:
    """Estimate the Elo difference and a 95% confidence interval from a match result.

    Returns (elo, lower, upper) from the perspective of the engine that scored
    ``wins``/``draws``/``losses``. Bounds may be +/-inf for a clean sweep.
    """

    n = wins + draws + losses
    score = (wins + 0.5 * draws) / n

    # Standard error of the mean score, treating each game's score as a sample.
    win_p, draw_p, loss_p = wins / n, draws / n, losses / n
    variance = (
        win_p * (1 - score) ** 2
        + draw_p * (0.5 - score) ** 2
        + loss_p * (0 - score) ** 2
    )
    stderr = math.sqrt(variance / n)

    def score_to_elo(x: float) -> float:
        if x <= 0:
            return float("-inf")
        if x >= 1:
            return float("inf")
        return -400 * math.log10(1 / x - 1)

    return (
        score_to_elo(score),
        score_to_elo(score - 1.96 * stderr),
        score_to_elo(score + 1.96 * stderr),
    )


def likelihood_of_superiority(wins: int, losses: int) -> float:
    """The probability that the engine is genuinely stronger, given decisive games."""

    if wins + losses == 0:
        return 0.5
    return 0.5 * (1 + math.erf((wins - losses) / math.sqrt(2 * (wins + losses))))


def print_summary(db_path: Path, engine1_id: int, engine2_id: int):
    """Print a summary of all the games in the DB between the given two engines"""

    conn = sqlite3.connect(db_path)
    cur = conn.cursor()

    cur.execute(
        "select id, name from engines where id in (?, ?)", (engine1_id, engine2_id)
    )
    engine_names = dict(cur.fetchall())

    def tally(games) -> tuple[int, int, int]:
        wins = draws = losses = 0
        for white_id, black_id, white_score, black_score, _ in games:
            if white_id == engine1_id:
                if white_score > black_score:
                    wins += 1
                elif white_score < black_score:
                    losses += 1
                else:
                    draws += 1
            else:
                if white_score < black_score:
                    wins += 1
                elif white_score > black_score:
                    losses += 1
                else:
                    draws += 1
        return wins, draws, losses

    def print_row(games, stats: bool = False):
        # Eg:
        #    25 wins, 10 draws, 15 losses (50% / 20% / 30%)
        wins, draws, losses = tally(games)

        total = wins + draws + losses

        if total == 0:
            return

        win_pct = wins / total * 100
        draw_pct = draws / total * 100
        loss_pct = losses / total * 100

        print(
            f"       {total:>4} games: {wins:>4} wins, {draws:>4} draws, {losses:>4} losses ({win_pct:.1f}% / {draw_pct:.1f}% / {loss_pct:.1f}%)"
        )

        if stats:
            elo, lo, hi = elo_diff_and_ci(wins, draws, losses)
            los = likelihood_of_superiority(wins, losses)
            print(
                f"                  Elo: {elo:+.1f} [{lo:+.1f}, {hi:+.1f}] (95% CI), "
                f"LOS: {los * 100:.1f}%"
            )

    cur.execute(
        """
        select
            white_engine_id,
            black_engine_id,
            white_score,
            black_score,
            ending_types.name
        from games
        join ending_types on games.ending_type_id = ending_types.id
        where white_engine_id in (?, ?) and black_engine_id in (?, ?)
    """,
        (engine1_id, engine2_id, engine1_id, engine2_id),
    )

    games = cur.fetchall()

    print(
        f"Summary of games between {engine_names[engine1_id]} and "
        f"{engine_names[engine2_id]} (from {engine_names[engine1_id]}'s perspective):"
    )
    print("    All games:")
    print_row(list(games), stats=True)

    print("    Games as White:")
    print_row(filter(lambda g: g[0] == engine1_id, games))

    print("    Games as Black:")
    print_row(filter(lambda g: g[1] == engine1_id, games))


def main():
    parser = argparse.ArgumentParser(
        description="Run chess engines against each other using UCI."
    )
    parser.add_argument(
        "--engine1-def",
        type=Path,
        required=True,
        help="Path to the config file for the first engine under test",
    )
    parser.add_argument(
        "--engine2-def",
        type=Path,
        required=True,
        help="Path to the config file for the second engine under test",
    )
    parser.add_argument(
        "--num-games", type=int, default=10, help="Number of games to run."
    )
    parser.add_argument(
        "--concurrency",
        type=int,
        default=multiprocessing.cpu_count(),
        help="The number of games to run concurrently",
    )
    parser.add_argument(
        "--db-path", default="tourney.db", help="SQLite database file to store results."
    )
    parser.add_argument(
        "--tc",
        type=TimeControl.parse,
        default=TimeControl.parse("60+0"),
        help="Time control per side as 'base[+increment]' in seconds (default: 60+0).",
    )
    parser.add_argument(
        "--openings",
        type=Path,
        default=None,
        help=(
            "Optional file of opening lines to vary games. If omitted, every "
            "game starts from the standard position and variety must come from "
            "engine non-determinism (e.g. the Wobble UCI option). Pass "
            f"'{DEFAULT_OPENINGS_PATH.name}' for the bundled set."
        ),
    )

    args = parser.parse_args()

    if args.openings is None:
        # No openings file: play every game from the standard start position and
        # rely on the engines themselves to vary the games.
        openings = [Opening(name="Startpos", moves=[])]
        using_openings_file = False
    else:
        openings = load_openings(args.openings)
        using_openings_file = True

    # Setup SQLite database
    conn = sqlite3.connect(args.db_path)
    cur = conn.cursor()
    cur.executescript(CREATE_TABLES_SQL)
    conn.commit()
    conn.close()

    # Insert ending types and engines into the database
    insert_ending_types(args.db_path)

    engine1_def = EngineDef.read(args.engine1_def)
    engine1_id = get_or_insert_engine(args.db_path, engine1_def)

    engine2_def = EngineDef.read(args.engine2_def)
    engine2_id = get_or_insert_engine(args.db_path, engine2_def)

    # Prepare arguments for each process
    half_games = args.num_games // 2
    jobs = []

    if args.num_games % 2 != 0:
        print(
            f"Warn: odd number of games requesting, actually running {half_games} games per side"
        )

    if using_openings_file and half_games > len(openings):
        print(
            f"Warn: only {len(openings)} openings available for {half_games} game pairs; "
            "openings will repeat, so some games will be identical for deterministic engines"
        )
    elif not using_openings_file and half_games > 1:
        print(
            "Note: no openings file, so every game starts from the standard position. "
            "Variety relies on engine non-determinism (e.g. the Wobble UCI option); "
            "without it, all games will be identical."
        )

    # Each opening is played once with each engine as White, so the two engines
    # face the same positions with colours reversed.
    for i in range(half_games):
        opening = openings[i % len(openings)]
        jobs.append(
            (
                args.db_path,
                engine1_def,
                engine1_id,
                engine2_def,
                engine2_id,
                args.tc,
                opening,
            )
        )  # Engine1 as White
        jobs.append(
            (
                args.db_path,
                engine2_def,
                engine2_id,
                engine1_def,
                engine1_id,
                args.tc,
                opening,
            )
        )  # Engine2 as White

    # Run games in parallel using multiprocessing with progress bar
    with Pool(args.concurrency) as pool:
        for _ in tqdm(
            pool.imap_unordered(play_games_parallel, jobs),
            total=len(jobs),
            desc="Running games",
        ):
            pass

    print_summary(args.db_path, engine1_id, engine2_id)


if __name__ == "__main__":
    main()
