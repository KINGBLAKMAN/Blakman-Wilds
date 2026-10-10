# Emberwild (repo: Blakman-Wilds)

An original survival-crafting game in Rust + Bevy 0.16: explore a small island, gather, craft,
level skills, survive wolf nights, and light the Beacon to win. Grey-box gameplay first, with
low-poly models built in Blender by script (`art/`).

## Run
```
python tools/setup.py          # data table → generated/extracted.rs
cargo run -p rewrite-app
cargo test --workspace
```
Controls (current movement core): WASD move · Alt walk · Shift sprint · Space jump · Ctrl dodge ·
mouse camera · Esc release mouse.

## Design (vertical slice, 10-20 minutes)
- **Loop:** explore → chop trees, mine rocks, pick berries → craft tools and stations → level skills
  (Woodcutting, Mining, Crafting, Combat) → survive nights → build and light the **Beacon** (win).
- **Lose:** death respawns you at camp; the clock keeps running, and nights get harder.
- **Entities:** player (health, stamina, hunger), tree / rock / iron rock / berry bush nodes (respawn),
  wolves, campfire / workbench / furnace, beacon.
- **Tools auto-select:** swing at a tree → best axe; rock → best pickaxe; wolf → best weapon.
- **Day/night:** 6-minute cycle; wolves hunt at night with telegraphed bites (dodge i-frames work).
- **Events:** ItemGathered, ItemCrafted, XpGained, LevelUp, PlayerHurt, PlayerDied, EnemyKilled,
  NightFell, DayBroke, BeaconLit.
- **Data:** items, recipes, nodes, enemies and tunables in data files; every number tagged.

## Status
- **Done:** the movement core inherited from the Dragonwilds movement study: walk/run/sprint, stamina
  (drain, regen delay), jump with stamina cost, dodge roll with i-frames, coyote and jump buffer,
  spring-arm third-person camera. 24 headless tests.
- **Art:** 11 environment models exported to `assets/models/*.glb` (trees, stump, rock, iron rock,
  berry bush, campfire, workbench, furnace, beacon and flame; 36-204 triangles each), built by
  `art/build_environment.py` on `art/asset_kit.py` (palette, low-poly helpers, glTF export).
  Rebuild in Blender: `exec(open(r"art/asset_kit.py").read()); exec(open(r"art/build_environment.py").read()); clear_scene(); build_all()`.
- **Next:** gameplay systems in `sim/` (inventory, gathering, crafting, skills, hunger, day/night,
  wolf AI, beacon win, game state), load the `.glb` models in `app/`, island layout, HUD
  (inventory, crafting panel, skills, objective), then character and wolf models with animations.

## Layout
| Path | What |
|---|---|
| `sim/` | The game as a pure 60 Hz state machine (no engine types); tests in `sim/src/tests.rs` |
| `app/` | Bevy: input, camera, drawing, HUD, capture hooks |
| `art/` | Blender scripts that build and export every model |
| `assets/models/` | Exported `.glb` models |
| `data/` | Tunables (`measurements.json` from the movement study; game content to come) |
| `tools/` | Data table generator (`setup.py`) |

Dev hooks: `REWRITE_CAPTURE=captures\shot.png` screenshots after 3 s and exits; `REWRITE_AUTOPLAY=1`
drives scripted input. `package.ps1` builds a release zip.
