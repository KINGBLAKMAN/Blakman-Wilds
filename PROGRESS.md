# Emberwild progress: handoff to the local session (Blender)

Last updated 2026-10-10, end of the cloud session. Start here, then read `README.md` (design)
and `CLOUD_HANDOFF.md` (rules).

## First steps on the PC
```
cd C:\Users\tankl\OneDrive\Desktop\ThingsClaudeCanTouch\emberwild
git pull origin main
python tools/setup.py
cargo test --workspace          # expect 47 passed
cargo run -p rewrite-app        # play it
```
The local folder already has the first commit (516b16f). The pull brings in everything below.
If git refuses to push, sign in first (`gh auth login`, or Git Credential Manager in a browser):
the last local push failed silently because git wasn't logged in to GitHub.

## Where things stand
**Done and tested (47 tests, `cargo test --workspace`):**
- Movement core: walk/run/sprint, stamina, jump, dodge roll with i-frames, spring-arm camera.
- Survival systems in `sim/` (`survival.rs`, `content.rs`, `bot.rs`): inventory and automatic
  best-tool choice, gathering nodes with respawn, skills and XP, crafting with stations, hunger,
  6-minute day/night, wolves (wander, chase, telegraphed bite, recover), death and respawn at
  camp, Beacon victory.
- All content and tuning lives in `data/content.ron` (every number ESTIMATE).
- A scripted bot (`sim::bot`) plays from an empty bag to the lit Beacon in about 4.6 game
  minutes. `REWRITE_AUTOPLAY=1` shows it in the app.

**App (`app/src/main.rs`, Bevy 0.16):** island, your 11 `.glb` models, built stations, Beacon
flame, day/night lighting, HUD, Tab crafting panel, death and victory screens. Checked only
through headless screenshots (`docs/screenshots/`).

**Windows build:** a zip was cross-compiled in the cloud and sent in chat, but it has never been
launched on Windows. Use `package.ps1` locally for real builds.

**Not checked yet:** feel with a real mouse and keyboard, frame rate on a real GPU, and whether a
human run lands in the 10-20 minute target.

## Next: character and wolf models (Blender)
The player and the wolves are grey boxes. Build them with `art/asset_kit.py`, the same way as
`art/build_environment.py` (new script, e.g. `art/build_characters.py`), and export with
`export_asset(name, path, animations=True)` to `assets/models/player.glb` and `wolf.glb`.

### Conventions the game expects
- 1 unit = 1 m, origin at the feet (base centre), low poly, flat shaded, shared `PALETTE`.
- **Facing:** the game's forward is -Z (Bevy). The exporter maps Blender +Y to -Z, so the model
  must **face Blender +Y** (away from the Front-view camera). If it ends up backwards in game,
  rotate the child scene 180° in the app rather than re-exporting.
- **Size, so visuals match collision:**
  - Player: collision box 0.6 x 1.8 x 0.6 m (`Tuning::half_extents` = 0.3, 0.9, 0.3). Aim for
    about 1.8 m tall.
  - Wolf: radius 0.45, height 0.9 (`content.ron` enemy `radius`, `survival::WOLF_HEIGHT`).
    Body about 1.1 m long.
- One armature per character, with actions named exactly as below so the app can pick clips by
  name.

### Animation clips and the timings they must match
The sim owns all timing; animations only need to line up with it. Sim actions are in frames at
30 fps (`generated/extracted.rs` → `ACTIONS`).

**Player:**
| Clip | When it plays (sim state) | Length / key frames |
|---|---|---|
| `Idle` | `State::Ground`, speed ≈ 0 | loop |
| `Walk` | Ground, speed ≤ 2 m/s | loop, matched to 2.0 m/s |
| `Run` | Ground, about 5 m/s | loop, matched to 5.0 m/s |
| `Sprint` | `player.sprinting` | loop, matched to 7.5 m/s |
| `Jump` / `Fall` / `Land` | `State::Air`; `Event::Jumped`, `Event::Landed` | short, plus a fall loop |
| `Roll` | `State::Act("Roll")` | 24 frames; i-frames 0-10; travels 3.5 m (root motion is done in code, so animate in place) |
| `Swing1` | `State::Act("Light1")`, used for chopping, mining and fighting | 30 frames; the hit lands on frames 10-14 |
| `Swing2` | `State::Act("Light2")`, the chained second swing | 32 frames; hit on 11-15 |
| `Hurt` | `Event::PlayerHurt` | short, additive if possible |
| `Death` | `GameState::Dead` | about 3 s (respawn delay) |

**Wolf** (`WolfState` in `sim/src/survival.rs`):
| Clip | When | Length |
|---|---|---|
| `Walk` | `Wander`, 1.6 m/s | loop |
| `Run` | `Chase`, 5.4 m/s | loop |
| `Crouch` | `Windup` (the telegraph before the bite) | 0.6 s, readable: head down, haunches up |
| `Bite` | end of Windup → `Recover` | quick lunge, then recovery to 0.9 s |
| `Death` | wolf removed on `Event::EnemyKilled` | about 1 s; the app needs to keep the corpse a moment |

Timings come from `data/content.ron` (`windup`, `recover`, speeds). If an animation reads better
at a different length, change the number there and the sim follows.

### Wiring them into the app (`app/src/main.rs`)
- Player: `setup_scene` spawns a capsule with the `PlayerBody` component. Swap it for
  `SceneRoot(player.glb)` and drive an `AnimationPlayer` from the sim state in `draw_player`
  (pick the clip from `game.0.player.state` and speed; seek Act clips to `a.f / 30.0` seconds so
  they stay frame-exact with the sim).
- Wolves: `draw_wolves` spawns grey boxes under `WolfVis(id)`. Swap them for `wolf.glb`, choose
  the clip from `w.state`, and keep the red tint on `Crouch` (or an emissive eye flash) as the
  telegraph.
- Bevy 0.16 animation: `AnimationGraph` + `AnimationPlayer` on the scene's armature entity, found
  after the scene spawns (`SceneInstanceReady` observer). Clips come from
  `GltfAssetLabel::Animation(i)`, or by name through the `Gltf` asset's `named_animations`.
- Check it with `REWRITE_SETUP=night,camp REWRITE_CAPTURE=captures\wolves.png cargo run -p rewrite-app`
  (staged night with wolves near camp), and `REWRITE_AUTOPLAY=1` to watch everything animate.

### Other art notes
- The Beacon flame is spawned 5.35 m up, on the bowl (`draw_structures`). If the beacon model
  changes height, update that number.
- Structures have solid footprints in `content.ron` (campfire 0.7, workbench 0.9, furnace 0.9,
  beacon 1.7 m radius). Keep new models within them, or update the radius.
- Nice to have: a camp ground decal or stone ring (currently a flat brown disc), a wolf pelt and
  meat pickup icon, a torch model.

## After the models
1. Play a full run and tune `data/content.ron` toward 10-20 minutes for a human.
2. Write the art skills from what worked: `blender-asset-pipeline`,
   `procedural-lowpoly-modeling`, `game-animation-blender` (in `~/.claude/skills/`).
3. Known gaps: wolves walk through trees and rocks (nodes aren't in the level collision), no
   sound, no save/load.

## Key files
| File | What |
|---|---|
| `sim/src/survival.rs` | All survival rules (gathering, crafting, hunger, clock, wolves, death, win) |
| `sim/src/content.rs` + `data/content.ron` | Content data and loader |
| `sim/src/bot.rs` | Scripted player (tests, autoplay) |
| `sim/src/survival_tests.rs` | Survival tests, including the full playthrough |
| `app/src/main.rs` | Everything drawn: models, lighting, HUD, dev hooks |
| `art/asset_kit.py`, `art/build_environment.py` | Blender helpers and environment models |
