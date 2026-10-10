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
Controls: click to capture the mouse · WASD move · Alt walk · Shift sprint · Space jump ·
Ctrl dodge (i-frames) · left click swing (gathers and fights; the best tool is picked
automatically) · Tab crafting panel (W/S choose, Enter craft) · F eat · E light the Beacon ·
Esc release mouse.

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
- **Movement core** (from the Dragonwilds movement study): walk/run/sprint, stamina (drain, regen
  delay), jump with stamina cost, dodge roll with i-frames, coyote and jump buffer, spring-arm
  third-person camera.
- **Survival gameplay, in `sim/`** (pure Rust, deterministic, tested): content from
  `data/content.ron` (items, recipes, nodes, wolves, tunables and the island layout; every number
  is ESTIMATE); inventory with automatic best-tool choice; tree/rock/iron/berry nodes gathered
  by the swing's hit window, with charges and respawn; Woodcutting/Mining/Crafting/Combat XP and
  levels (yield, damage and recipe gates); crafting with station proximity (campfire, workbench,
  furnace, smelting); hunger, eating and starvation; a 6-minute day/night clock; wolves at night
  (wander, chase, telegraphed bite that roll i-frames avoid, recover; more each night); death
  with respawn at camp; Beacon build + light = victory. 47 tests, including a scripted bot
  (`sim::bot`) that plays from an empty bag to the lit Beacon (about 4.6 game minutes, one
  night, two wolves) and replays identically.
- **App (`app/`, Bevy 0.16):** the island with the Blender `.glb` models (stumps when trees are
  felled), built stations and the Beacon flame, day/night sun and sky, campfire light, grey-box
  player and wolves (wolves glow red while winding up a bite), HUD (health/stamina/hunger bars,
  clock, objective, bag and skills, event feed, crafting panel, death and victory screens).
  Screenshots in `docs/screenshots/` were taken headless (Xvfb + software Vulkan).
- **Art:** 11 environment models in `assets/models/*.glb` (trees, stump, rock, iron rock, berry
  bush, campfire, workbench, furnace, beacon and flame; 36-204 triangles each), built by
  `art/build_environment.py` on `art/asset_kit.py`.
  Rebuild in Blender: `exec(open(r"art/asset_kit.py").read()); exec(open(r"art/build_environment.py").read()); clear_scene(); build_all()`.
- **Next (local, needs Blender):** player and wolf models with animations, then wire them in.
  Then playtest and tune `data/content.ron` (a human run should take 10-20 minutes).
- **Known gaps:** trees and rocks push you aside but aren't in the level collision (wolves walk
  through them); no sound yet; no save/load.

## Layout
| Path | What |
|---|---|
| `sim/` | The game as a pure 60 Hz state machine (no engine types); tests in `sim/src/tests.rs` (movement) and `sim/src/survival_tests.rs` |
| `app/` | Bevy: input, camera, drawing, HUD, capture hooks |
| `art/` | Blender scripts that build and export every model |
| `assets/models/` | Exported `.glb` models |
| `data/` | `content.ron` (game content and tunables, all ESTIMATE); `measurements.json` (movement study) |
| `tools/` | Data table generator (`setup.py`) |

Dev hooks: `REWRITE_CAPTURE=captures\shot.png` screenshots after `REWRITE_CAPTURE_AT` seconds
(default 3) and exits; `REWRITE_AUTOPLAY=1` lets the scripted bot play through the real input path
(`REWRITE_SPEED=6` fast-forwards); `REWRITE_SETUP=camp,night,victory,craft` starts from a staged
state for screenshots. `package.ps1` builds a release zip.
