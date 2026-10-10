"""Where the installed game lives.

Two environment variables, each with a default and a clear error when wrong:
  GAME_DIR    the installed game folder (the one with the executable)
  GAME_FILES  a folder of files the user unpacked with community tools (if the game needs it)
"""
import os
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

# Fill these in per game: default install locations to try, and one file that proves the
# folder is right (so a wrong path fails here, with a message, before any work).
DEFAULT_GAME_DIRS = [
    r"C:\Program Files (x86)\Steam\steamapps\common\GAME_FOLDER",
    r"D:\SteamLibrary\steamapps\common\GAME_FOLDER",
]
GAME_DIR_MARKER = "GAME_EXE_NAME.exe"
FILES_MARKER = ""  # e.g. "chr/c0000.anibnd.dcx"; empty if the game needs no unpacking


def _fail(msg):
    print("\n" + msg + "\n", file=sys.stderr)
    sys.exit(1)


def game_dir():
    env = os.environ.get("GAME_DIR")
    candidates = [Path(env)] if env else [Path(p) for p in DEFAULT_GAME_DIRS]
    for c in candidates:
        if (c / GAME_DIR_MARKER).exists():
            return c
    _fail(
        f"Could not find the game (looked for {GAME_DIR_MARKER} in: "
        + ", ".join(str(c) for c in candidates)
        + ").\nSet GAME_DIR to your install folder, e.g. in PowerShell:\n"
        + '    $env:GAME_DIR = "D:\\Steam\\steamapps\\common\\GAME_FOLDER"'
    )


def game_files():
    p = Path(os.environ.get("GAME_FILES", ROOT / "game-files"))
    if FILES_MARKER and not (p / FILES_MARKER).exists():
        _fail(f"{p} does not contain {FILES_MARKER}. Unpack the game files (see README) and set GAME_FILES.")
    return p


def generated_dir():
    d = ROOT / "generated"
    d.mkdir(exist_ok=True)
    return d
