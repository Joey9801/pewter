# Pit two engines against each other and determine the strongest

import argparse
import hashlib
import enum
import multiprocessing
import sqlite3
from multiprocessing import Pool
from pathlib import Path

import chess
import chess.engine
import chess.pgn
from pydantic import BaseModel
# from tqdm import tqdm


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
    CHECKMATE="Checkmate"
    STALEMATE="Stalemate"
    FIVEFOLD_REP="Fivefold repetition"
    INSUFFICIENT_MAT="Insufficient material"
    SEVENTY_FIVE_MOVE="Seventy-five move rule"
    UNKNOWN="Unknown"

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
    cur.execute("SELECT id FROM ending_types WHERE name = (?)", (str(ending), ))
    
    return cur.fetchone()[0]

# Compute checksum (MD5) for the engine binary
def compute_checksum(file_path: Path) -> str:
    hash_md5 = hashlib.md5()
    with open(file_path, "rb") as f:
        for chunk in iter(lambda: f.read(4096), b""):
            hash_md5.update(chunk)
    return hash_md5.hexdigest()


# Insert engine into the database
def insert_engine(db_path: Path, engine_def: EngineDef) -> int:
    checksum = compute_checksum(engine_def.path)

    # TODO: Check whether the engine is already in the db before creating it new

    conn = sqlite3.connect(db_path)
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

    black_engine = chess.engine.SimpleEngine.popen_uci(
        black_engine_def.path, env=black_engine_def.env
    )
    black_engine.configure(black_engine_def.options)

    # TODO: Set up a proper persistent clock for each side, this value ends up
    # just being a hint to the engine with no penalty for ignoring
    per_move_time_limit = 0.1

    board = chess.Board()
    pgn = chess.pgn.Game()

    pgn.headers["White"] = f"{white_engine_def.name} (id: {white_engine_id})"
    pgn.headers["Black"] = f"{black_engine_def.name} (id: {black_engine_id})"

    with white_engine, black_engine:
        # TODO: Add game lose condition for running out of clock
        while not board.is_game_over():
            if board.turn == chess.WHITE:
                result = white_engine.play(
                    board, chess.engine.Limit(time=per_move_time_limit)
                )
            else:
                result = black_engine.play(
                    board, chess.engine.Limit(time=per_move_time_limit)
                )

            pgn = pgn.add_main_variation(result.move)
            board.push(result.move)

        # Determine the result and ending kind of the game
        if board.is_checkmate():
            white_score, black_score = (0, 1) if board.turn == chess.WHITE else (1, 0)
            ending_type = EndingType.CHECKMATE
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
    engine1_id = insert_engine(args.db_path, engine1_def)

    engine2_def = EngineDef.read(args.engine2_def)
    engine2_id = insert_engine(args.db_path, engine2_def)

    # Prepare arguments for each process
    num_cores = multiprocessing.cpu_count()
    half_games = args.num_games // 2
    jobs = []

    if args.num_games % 2 != 0:
        print(f"Warn: odd number of games requesting, actually running {half_games} games per side")

    for i in range(half_games):
        jobs.append(
            (args.db_path, engine1_def, engine1_id, engine2_def, engine2_id)
        )  # Engine1 as White
        jobs.append(
            (args.db_path, engine2_def, engine2_id, engine1_def, engine1_id)
        )  # Engine2 as White

    # Run games in parallel using multiprocessing with progress bar
    with Pool(num_cores) as pool:
        # for _ in tqdm(
        #     pool.imap_unordered(play_games_parallel, jobs),
        #     total=len(jobs),
        #     desc="Running games",
        # ):
        #     pass

        for i, _ in enumerate(pool.imap_unordered(play_games_parallel, jobs)):
            print(f"Finished game {i + 1}/{len(jobs)}")


if __name__ == "__main__":
    main()
