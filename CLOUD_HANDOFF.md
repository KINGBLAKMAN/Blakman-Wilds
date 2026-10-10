# Cloud session handoff: Emberwild

Read `README.md` first for the design. Run `git pull origin main` before starting.

## You handle (code only)
1. **Gameplay systems in `sim/`**, each with headless tests in `sim/src/tests.rs`:
   - inventory and items
   - gathering: tree, rock, iron rock and berry nodes, with respawn
   - crafting: recipes and stations (campfire, workbench, furnace)
   - skills and XP: Woodcutting, Mining, Crafting, Combat
   - hunger
   - day/night, on a 6-minute cycle
   - wolf AI: night hunts and telegraphed bites, which dodge i-frames avoid
   - lighting the beacon wins; dying respawns you at camp
   - game state and the events listed in the README
2. **Data files in `data/`:** items, recipes, nodes, enemies and tunables. Tag every number `ESTIMATE` (original game, so there's nothing to measure).
3. **`app/` (Bevy 0.16):**
   - load the existing `assets/models/*.glb`
   - island layout
   - HUD: inventory, crafting panel, skills, objective, health/stamina/hunger
4. Keep `cargo test --workspace` green and commit in small steps.

## Rules
- `sim/` stays engine-agnostic: no Bevy types, a fixed 60 Hz `Sim::step(&Input)`, deterministic. Bevy only reads the state and draws it.
- Don't break the movement core or its 24 existing tests.
- Run `python tools/setup.py` before `cargo` so `generated/extracted.rs` exists.
- If a step can't be verified (no display for screenshots, for example), say so; don't claim it works.

## Don't do (local-only, needs the user's PC)
- **No Blender work.** Don't edit or regenerate `art/*.py` or `assets/models/*.glb`. Blender runs only locally. Use grey-box placeholders for anything without a model, such as the player and the wolf.
- **No reverse engineering:** no Ghidra/REA and no decompiling games. Not needed: Emberwild is an original game.
- **Don't expect the user-level skills** (`game-rewrite`, `sim-core`, `movement-controller` and others). They live on the local PC, not in this repo.
- Don't force-push, rewrite history, or push anywhere except `origin main` (or a branch, if the user asks).

## Left for the next local session
- Player character and wolf models with animations (Blender), then wire them into `app/`.
- Art skills: blender-asset-pipeline, procedural-lowpoly-modeling, game-animation-blender.
