# Pit two engines against each other and determine the strongest

import argparse
import hashlib
from datetime import datetime, timedelta, timezone
import enum
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
    started_at: datetime | None

    def __init__(self, starting: timedelta):
        self.remaining = starting
        self.started_at = None

    @property
    def remaining_seconds(self) -> float:
        return self.remaining.total_seconds()

    def start(self):
        assert self.started_at is None
        self.started_at = datetime.now(tz=timezone.utc)

    def stop(self):
        assert self.started_at is not None
        now = datetime.now(tz=timezone.utc)
        diff = now - self.started_at
        self.remaining -= diff
        self.started_at = None


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
):
    white_engine = chess.engine.SimpleEngine.popen_uci(
        white_engine_def.path, env=white_engine_def.env
    )
    white_engine.configure(white_engine_def.options)
    white_clock = ChessClock(timedelta(minutes=1))

    black_engine = chess.engine.SimpleEngine.popen_uci(
        black_engine_def.path, env=black_engine_def.env
    )
    black_engine.configure(black_engine_def.options)
    black_clock = ChessClock(timedelta(minutes=1))

    board = chess.Board()
    pgn = chess.pgn.Game()

    pgn.headers["White"] = f"{white_engine_def.name} (id: {white_engine_id})"
    pgn.headers["Black"] = f"{black_engine_def.name} (id: {black_engine_id})"

    with white_engine, black_engine:
        # TODO: Add game lose condition for running out of clock
        while not board.is_game_over():
            if board.turn == chess.WHITE:
                engine, clock = white_engine, white_clock
            else:
                engine, clock = black_engine, black_clock

            limit = chess.engine.Limit(
                white_clock=white_clock.remaining_seconds,
                black_clock=black_clock.remaining_seconds,
            )
            clock.start()
            result = engine.play(board, limit=limit)
            clock.stop()

            if clock.remaining_seconds < 0:
                break

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


def print_summary(db_path: Path, engine1_id: int, engine2_id: int):
    """Print a summary of all the games in the DB between the given two engines"""

    conn = sqlite3.connect(db_path)
    cur = conn.cursor()

    cur.execute(
        "select id, name from engines where id in (?, ?)", (engine1_id, engine2_id)
    )
    engine_names = dict(cur.fetchall())

    def print_row(games):
        # Eg:
        #    25 wins, 10 draws, 15 losses (50% / 20% / 30%)
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

        total = wins + draws + losses
        
        if total == 0:
            return
        
        win_pct = wins / total * 100
        draw_pct = draws / total * 100
        loss_pct = losses / total * 100

        print(
            f"       {total:>4} games: {wins:>4} wins, {draws:>4} draws, {losses:>4} losses ({win_pct:.1f}% / {draw_pct:.1f}% / {loss_pct:.1f}%)"
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

    print(f"Summary of games between {engine_names[engine1_id]} and {engine_names[engine2_id]}:")
    print("    All games:")
    print_row(games)
    
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

    args = parser.parse_args()

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

    for i in range(half_games):
        jobs.append(
            (args.db_path, engine1_def, engine1_id, engine2_def, engine2_id)
        )  # Engine1 as White
        jobs.append(
            (args.db_path, engine2_def, engine2_id, engine1_def, engine1_id)
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
