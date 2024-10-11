# Pit two engines against each other and determine the strongest

import argparse
import os
import subprocess as sp
from dataclasses import dataclass
from pathlib import Path
from textwrap import dedent
from typing import Annotated, Literal

import chess
import chess.engine
from pydantic import BaseModel, Field
import sys
import termios
import tty


class UciOptionCheck(BaseModel):
    option_type: Literal["check"]
    checked: bool


class UciOptionSpin(BaseModel):
    option_type: Literal["spin"]
    value: int


class UciOptionCombo(BaseModel):
    option_type: Literal["combo"]
    value: str


class UciOptionString(BaseModel):
    option_type: Literal["string"]
    value: str


UciOption = Annotated[
    UciOptionCheck | UciOptionSpin | UciOptionCombo | UciOptionString,
    Field(discriminator="option_type"),
]


class EngineDef(BaseModel):
    name: str
    path: Path
    """Path to the engine binary.
    
    If a relative path is serialized, it will be interpreted as relative to the
    directory containing the serialized file. Eg if the file
    '/path/to/engine.json' contained the engine path './foo', it would refer to
    '/path/to/foo'.
    """

    options: dict[str, UciOption]
    env: dict[str, str]

    @classmethod
    def read(cls, path: Path) -> "EngineDef":
        content = path.read_text()
        engine = EngineDef.model_validate_json(content)
        if not engine.path.is_absolute():
            engine.path = (path.parent / engine.path).absolute().resolve()
        return engine


class Game:
    board: chess.Board
    engine_white: chess.engine.SimpleEngine
    engine_black: chess.engine.SimpleEngine
    
    def __init__(self, engine_white: EngineDef, engine_black: EngineDef):
        self.board = chess.Board()
        
        # TODO: Implement applying the UCI options
        self.engine_white = chess.engine.SimpleEngine.popen_uci(
            command=str(engine_white.path),
            env=engine_white.env,
        )
        
        self.engine_black = chess.engine.SimpleEngine.popen_uci(
            command=str(engine_black.path),
            env=engine_black.env,
        )
        
    @property
    def is_game_over(self):
        return self.board.is_game_over()
        
    def reset(self):
        """Reset the game state"""
        self.board = chess.Board()

    def play_single_move(self):
        if self.board.is_game_over():
            raise ValueError("Game is already over")
        
        engine = self.engine_white if self.board.turn == chess.WHITE else self.engine_black
        result = engine.play(self.board, chess.engine.Limit(time=0.1))
        self.board.push(result.move)
        
    def play_game(self) -> chess.Outcome:
        self.reset()
        
        while not self.is_game_over:
            self.play_single_move()
            
        return self.board.outcome()

if __name__ == "__main__":
    parser = argparse.ArgumentParser()

    parser.add_argument(
        "--engine-paths",
        type=Path,
        nargs="+",
        required=True,
            help="""Path to a json description of the engines to play in the tourney.
        
        Passing just a single engine will play it against itself, as if it were passed twice.
        """,
    )

    args = parser.parse_args()

    engines = [EngineDef.read(path) for path in args.engine_paths]
    
    if len(engines) == 1:
        game = Game(engines[0], engines[0])
    elif len(engines) == 2:
        game = Game(engines[0], engines[1])
    else:
        raise NotImplementedError("Full tourneys with more than 2 engines not implemented")
    
    def clear_terminal():
        """Clear the terminal screen."""
        os.system('clear' if os.name == 'posix' else 'cls')

    def set_raw_mode():
        """Set terminal to raw mode."""
        fd = sys.stdin.fileno()
        old_settings = termios.tcgetattr(fd)
        tty.setraw(fd)
        return old_settings

    def reset_terminal_mode(old_settings):
        """Reset terminal to original mode."""
        fd = sys.stdin.fileno()
        termios.tcsetattr(fd, termios.TCSADRAIN, old_settings)

    clear_terminal()
    old_settings = set_raw_mode()

    try:
        while not game.is_game_over:
            game.play_single_move()
            board_str = str(game.board).replace('\n', '\r\n')
            clear_terminal()
            print(board_str, end='\r\n')
            print(end='\r\n')
    finally:
        reset_terminal_mode(old_settings)
    
    print("Game over!")
    print(game.board.outcome())
    
    game.engine_white.quit()
    game.engine_black.quit()