"""One command to generate everything the project needs from the installed game.

    python tools/setup.py            (pass --example to test the pipeline without a game)

Each step is optional where the game data is optional (e.g. sound): say what was skipped and
why, and keep running. Fail early, with a message naming the folder, when inputs are wrong.
"""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import extract

print("1/1  Extracting the data table...")
extract.main()
# Add more generators here as the project grows, e.g.:
#   print("2/3  Baking animations...");  bake_anims.main()     -> assets/anims.bin (gitignored)
#   print("3/3  Baking sounds...");      bake_sounds.main()    -> assets/sounds/   (gitignored)
print("\nDone. Build with:  cargo run -p rewrite-app")
