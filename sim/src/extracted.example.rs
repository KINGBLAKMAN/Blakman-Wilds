// FALLBACK copy of the generated table. The real one is generated/extracted.rs: python tools/setup.py
// Plain // comments only: this file is include!d.

use crate::actions::{ActionDef, Window};

pub const SOURCE: &str = "Dragonwilds data (5/18 public or measured, 1 from video)";

// ESTIMATE: WALK_SPEED: shot 1 (walk) not measured yet [m/s]
pub const WALK_SPEED: f32 = 2.0;
// ESTIMATE: RUN_SPEED: shot 2 (run) not measured yet [m/s]
pub const RUN_SPEED: f32 = 5.0;
// ESTIMATE: SPRINT_SPEED: shot 3 (sprint) not measured yet [m/s]
pub const SPRINT_SPEED: f32 = 7.5;
// ESTIMATE: ACCEL: shot 4 (start from standstill) not measured yet [m/s2]
pub const ACCEL: f32 = 20.0;
// ESTIMATE: BRAKING: shot 5 (stop from run) not measured yet [m/s2]
pub const BRAKING: f32 = 25.0;
// ESTIMATE: AIR_ACCEL: shot 7 (air steering) not measured yet [m/s2]
pub const AIR_ACCEL: f32 = 4.0;
// ESTIMATE: GRAVITY: shot 6 (jump) not measured yet; UE default 9.8 x GravityScale 2 is common [m/s2]
pub const GRAVITY: f32 = 19.6;
// ESTIMATE: JUMP_HEIGHT: shot 6 (jump beside a wall) not measured yet; video: a running jump onto a raised block took about 25 frames (0.42 s) airborne (Z-JD38KiMfA f1679-1704), not enough to separate height from gravity [m]
pub const JUMP_HEIGHT: f32 = 1.0;
// SRC: PUBLIC STAMINA_MAX: dragonwilds.runescape.wiki/w/Stamina (rev 66030, 2026-08-26): maximum stamina 100 (rest above 50) [points]
pub const STAMINA_MAX: f32 = 100.0;
// SRC: PUBLIC SPRINT_DRAIN: dragonwilds.runescape.wiki/w/Stamina (rev 66030, 2026-08-26): Running 4/s [points/s]
pub const SPRINT_DRAIN: f32 = 4.0;
// SRC: PUBLIC STAMINA_REGEN: dragonwilds.runescape.wiki/w/Stamina (rev 66030, 2026-08-26): regen 6.0/s at 100 hydration (0.75/s at 0, +1.5/s per 25) [points/s]
pub const STAMINA_REGEN: f32 = 6.0;
// SRC: PUBLIC STAMINA_REGEN_DELAY: dragonwilds.runescape.wiki/w/Stamina (rev 66030, 2026-08-26): regen starts 3s after the last stamina-costing action [s]
pub const STAMINA_REGEN_DELAY: f32 = 3.0;
// ESTIMATE: CAM_FOV: game settings default / shot 11 [deg (vertical)]
pub const CAM_FOV: f32 = 80.0;
// ESTIMATE: CAM_ARM: shot 11 (camera framing) [m]
pub const CAM_ARM: f32 = 3.2;
// ESTIMATE: CAM_HEIGHT: shot 11 (pivot height) [m]
pub const CAM_HEIGHT: f32 = 1.6;
// ESTIMATE: VIDEO CAM_SHOULDER: YouTube Z-JD38KiMfA (Chasm Dash gold run, 1.0, 720p60) and h7zIeNwdXCU (agility update, 720p60): character stays horizontally centred while running in every third-person shot (e.g. Z-JD38KiMfA f3540-3660, h7zIeNwdXCU 94-126 s) [m right]
pub const CAM_SHOULDER: f32 = 0.0;
// ESTIMATE: CAM_LAG: shot 12 (frames to settle after a stop) [1/s]
pub const CAM_LAG: f32 = 10.0;
// SRC: PUBLIC JUMP_STAMINA: dragonwilds.runescape.wiki/w/Stamina (rev 66030, 2026-08-26): Jumping 10 per action; 0.12.1 patch notes: jump cost reduced from 20 to 10 (current) [points]
pub const JUMP_STAMINA: f32 = 10.0;

pub const ACTIONS: &[ActionDef] = &[
    // SRC: PUBLIC+ESTIMATE: stamina 20 from dragonwilds.runescape.wiki/w/Stamina (rev 66030, 2026-08-26) (Dodging 20); timing and distance still ESTIMATE (shot 10)
    ActionDef {
        name: "Roll",
        len: 24.0,
        hit: Window::NEVER,
        iframes: Window { from: 0.0, to: 10.0 },
        input_from: 12.0,
        cancel_into_attack: 20.0,
        cancel_into_dodge: 22.0,
        cancel_into_move: 20.0,
        root_motion: 3.5,
        stamina: 20.0,
        damage: 0.0,
        reach: 0.0,
        next: "",
    },
    // SRC: ESTIMATE: combat is out of scope; template placeholder
    ActionDef {
        name: "Light1",
        len: 30.0,
        hit: Window { from: 10.0, to: 14.0 },
        iframes: Window::NEVER,
        input_from: 8.0,
        cancel_into_attack: 18.0,
        cancel_into_dodge: 16.0,
        cancel_into_move: 24.0,
        root_motion: 0.6,
        stamina: 12.0,
        damage: 30.0,
        reach: 1.6,
        next: "Light2",
    },
    // SRC: ESTIMATE: combat is out of scope; template placeholder
    ActionDef {
        name: "Light2",
        len: 32.0,
        hit: Window { from: 11.0, to: 15.0 },
        iframes: Window::NEVER,
        input_from: 8.0,
        cancel_into_attack: 20.0,
        cancel_into_dodge: 16.0,
        cancel_into_move: 26.0,
        root_motion: 0.7,
        stamina: 12.0,
        damage: 32.0,
        reach: 1.6,
        next: "Light1",
    },
];
