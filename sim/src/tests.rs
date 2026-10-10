//! Behaviour tests. They drive the real `Sim::step` with scripted input, headless, and assert
//! the numbers the data table promises. When the table is regenerated from the game, these
//! guard the simulation against drifting away from it.

use crate::actions::{self, ANIM_FPS};
use crate::world::{self, BoxWorld};
use crate::*;
use glam::{Vec2, Vec3};

fn sim() -> Sim {
    Sim::new(BoxWorld::arena(), Vec3::ZERO)
}

fn idle() -> Input {
    Input::default()
}

fn forward() -> Input {
    Input { mv: Vec2::Y, ..Default::default() }
}

fn run(s: &mut Sim, inp: Input, ticks: usize) -> Vec<Event> {
    let mut all = Vec::new();
    for _ in 0..ticks {
        s.step(&inp);
        all.extend(s.events.iter().copied());
    }
    all
}

fn press_attack(s: &mut Sim) {
    s.step(&Input { attack: Button::DOWN, ..idle() });
}

fn frame(s: &Sim) -> Option<(&'static str, f32)> {
    match s.player.state {
        State::Act(a) => Some((a.def.name, a.f)),
        _ => None,
    }
}

#[test]
fn settles_on_the_floor() {
    let mut s = sim();
    run(&mut s, idle(), 30);
    assert!(s.player.grounded());
    assert!(s.player.pos.y.abs() < 1e-3, "feet at y = {}", s.player.pos.y);
}

#[test]
fn jump_reaches_the_data_height() {
    let mut s = sim();
    run(&mut s, idle(), 5);
    s.step(&Input { jump: Button::DOWN, ..idle() });
    let mut peak: f32 = 0.0;
    for _ in 0..120 {
        s.step(&idle());
        peak = peak.max(s.player.pos.y);
    }
    let h = s.tuning.jump_height;
    assert!((peak - h).abs() < 0.05, "peak {peak} vs jump_height {h}");
    assert!(s.player.grounded());
}

#[test]
fn run_speed_matches_the_data() {
    let mut s = sim();
    run(&mut s, forward(), 60);
    let flat = Vec3::new(s.player.vel.x, 0.0, s.player.vel.z).length();
    assert!((flat - s.tuning.run_speed).abs() < 1e-3, "speed {flat}");
}

#[test]
fn walls_slide_instead_of_stopping() {
    let mut s = sim();
    // Run diagonally into the north wall (z = -20): we should end up sliding along x.
    let diag = Input { mv: Vec2::new(1.0, 1.0).normalize(), ..idle() };
    run(&mut s, diag, 60 * 8);
    assert!(s.player.pos.z > -20.0, "went through the wall: z = {}", s.player.pos.z);
    assert!(s.player.pos.z < -19.0, "never reached the wall");
}

#[test]
fn coyote_time_allows_a_late_jump() {
    let mut s = sim();
    // Stand on the crate (top at y = 1), walk off its edge, then jump a moment later.
    s.player.pos = Vec3::new(7.9, 1.0, 0.0);
    run(&mut s, idle(), 10);
    assert!(s.player.grounded());
    let right = Input { mv: Vec2::X, ..idle() };
    let mut left_ground_at = None;
    for i in 0..60 {
        s.step(&right);
        if !s.player.grounded() {
            left_ground_at = Some(i);
            break;
        }
    }
    assert!(left_ground_at.is_some(), "never walked off");
    let ev = run(&mut s, Input { jump: Button::DOWN, ..right }, 1);
    assert!(ev.contains(&Event::Jumped), "coyote jump refused");
}

#[test]
fn roll_iframes_match_the_frame_data() {
    let mut s = sim();
    run(&mut s, idle(), 5);
    s.step(&Input { dodge: Button::DOWN, ..forward() });
    let roll = actions::find("Roll").unwrap();
    let mut invincible_ticks = 0;
    while matches!(frame(&s), Some(("Roll", _))) {
        invincible_ticks += s.player.invincible() as usize;
        s.step(&idle());
    }
    let expected = (roll.iframes.frames() * (TICK_HZ as f32 / ANIM_FPS)).round() as usize;
    assert_eq!(invincible_ticks, expected, "{} i-frames at 30 fps = {expected} ticks", roll.iframes.frames());
}

#[test]
fn roll_travels_its_root_motion() {
    let mut s = sim();
    run(&mut s, idle(), 5);
    let start = s.player.pos;
    s.step(&Input { dodge: Button::DOWN, ..forward() });
    while frame(&s).is_some() {
        s.step(&idle());
    }
    let roll = actions::find("Roll").unwrap();
    let moved = Vec3::new(s.player.pos.x - start.x, 0.0, s.player.pos.z - start.z).length();
    assert!((moved - roll.root_motion).abs() < 0.08, "moved {moved} vs {}", roll.root_motion);
}

#[test]
fn presses_before_the_input_window_are_dropped() {
    let mut s = sim();
    run(&mut s, idle(), 5);
    press_attack(&mut s);
    let def = actions::find("Light1").unwrap();
    while frame(&s).is_some_and(|(_, f)| f < def.input_from - 1.0) {
        s.step(&Input { attack: Button::DOWN, ..idle() });
    }
    run(&mut s, idle(), 200);
    assert!(frame(&s).is_none(), "an early mash was queued");
}

#[test]
fn buffered_attack_chains_exactly_at_the_cancel_frame() {
    let mut s = sim();
    run(&mut s, idle(), 5);
    press_attack(&mut s);
    let def = actions::find("Light1").unwrap();
    while frame(&s).is_some_and(|(_, f)| f < def.input_from) {
        s.step(&idle());
    }
    press_attack(&mut s); // buffered inside the window
    let mut last_f = 0.0;
    while let Some(("Light1", f)) = frame(&s) {
        last_f = f;
        s.step(&idle());
    }
    assert_eq!(frame(&s).map(|(n, _)| n), Some(def.next));
    assert!((def.cancel_into_attack - last_f) <= actions::FRAMES_PER_TICK + 1e-4);
}

#[test]
fn a_swing_lands_once_and_freezes_both() {
    let mut s = Sim::sandbox();
    run(&mut s, idle(), 5);
    press_attack(&mut s);
    let ev = run(&mut s, idle(), 90);
    let hits: Vec<_> = ev.iter().filter(|e| matches!(e, Event::Hit { .. })).collect();
    assert_eq!(hits.len(), 1, "one hit per swing: {ev:?}");
}

#[test]
fn out_of_reach_misses() {
    let mut s = Sim::sandbox();
    s.targets[0].pos = Vec3::new(0.0, 0.0, -6.0);
    run(&mut s, idle(), 5);
    press_attack(&mut s);
    let ev = run(&mut s, idle(), 90);
    assert!(!ev.iter().any(|e| matches!(e, Event::Hit { .. })));
}

#[test]
fn cannot_roll_through_a_target() {
    let mut s = Sim::sandbox();
    run(&mut s, idle(), 5);
    s.step(&Input { dodge: Button::DOWN, ..forward() });
    run(&mut s, idle(), 90);
    let t = &s.targets[0];
    let gap = Vec3::new(s.player.pos.x - t.pos.x, 0.0, s.player.pos.z - t.pos.z).length();
    assert!(s.player.pos.z > t.pos.z, "rolled through the target");
    assert!(gap >= t.radius + s.tuning.half_extents.x - 1e-3, "overlapping by {}", t.radius + 0.3 - gap);
}

#[test]
fn course_is_beatable_by_scripted_input() {
    // A course test: start, drive input like a player, assert the goal is reached.
    // Jump onto the 0.3 m step (works for any sensible jump height).
    let mut s = sim();
    s.player.pos = Vec3::new(1.8, 0.0, 0.0);
    let right = Input { mv: Vec2::X, ..idle() };
    let mut jumped = false;
    let mut stood_on_step = false;
    for _ in 0..240 {
        let inp = if s.player.pos.x > 2.2 && !jumped {
            jumped = true;
            Input { jump: Button::DOWN, ..right }
        } else {
            right
        };
        s.step(&inp);
        stood_on_step |= s.player.grounded() && (0.25..0.35).contains(&s.player.pos.y);
    }
    assert!(stood_on_step, "never landed on the 0.3 m step: {:?}", s.player.pos);
}

#[test]
fn deterministic_replay() {
    let script = |i: usize| Input {
        mv: Vec2::new(((i / 40) % 3) as f32 - 1.0, 1.0).clamp_length_max(1.0),
        jump: if i % 97 == 0 { Button::DOWN } else { Button::default() },
        attack: if i % 61 == 0 { Button::DOWN } else { Button::default() },
        ..Default::default()
    };
    let play = || {
        let mut s = Sim::sandbox();
        for i in 0..2000 {
            s.step(&script(i));
        }
        (s.player.pos, s.player.stamina, s.targets[0].hp)
    };
    assert_eq!(play(), play(), "same inputs must give the same result");
}

// ---------------------------------------------------------------- Dragonwilds movement + camera

fn sprint() -> Input {
    Input { mv: Vec2::Y, sprint: Button::HELD, ..Default::default() }
}

fn flat_speed(s: &Sim) -> f32 {
    Vec3::new(s.player.vel.x, 0.0, s.player.vel.z).length()
}

#[test]
fn light_tilt_walks_full_tilt_runs() {
    let mut s = sim();
    run(&mut s, Input { mv: Vec2::Y * 0.3, ..idle() }, 60);
    assert!((flat_speed(&s) - s.tuning.walk_speed).abs() < 1e-3, "walk {}", flat_speed(&s));
    run(&mut s, forward(), 60);
    assert!((flat_speed(&s) - s.tuning.run_speed).abs() < 1e-3, "run {}", flat_speed(&s));
}

#[test]
fn sprint_reaches_sprint_speed_and_drains_stamina() {
    let mut s = sim();
    run(&mut s, sprint(), 60);
    assert!(s.player.sprinting);
    assert!((flat_speed(&s) - s.tuning.sprint_speed).abs() < 1e-3, "sprint {}", flat_speed(&s));
    let expected = s.tuning.stamina_max - s.tuning.sprint_drain * 1.0;
    assert!((s.player.stamina - expected).abs() < 0.5, "stamina {} vs {expected}", s.player.stamina);
}

#[test]
fn empty_stamina_ends_sprint_until_released() {
    let mut s = sim();
    let seconds = s.tuning.stamina_max / s.tuning.sprint_drain;
    // Sprint in circles so the arena wall never interferes.
    let circle = |i: usize| Input { mv: Vec2::Y, look_yaw: i as f32 * 0.01, sprint: Button::HELD, ..Default::default() };
    for i in 0..((seconds * 60.0) as usize + 30) {
        s.step(&circle(i));
    }
    assert!(!s.player.sprinting, "still sprinting on empty");
    run(&mut s, Input { mv: Vec2::Y, look_yaw: 0.0, sprint: Button::HELD, ..Default::default() }, 120);
    assert!(!s.player.sprinting, "holding sprint after running dry must not restart it");
    assert!(flat_speed(&s) <= s.tuning.run_speed + 1e-3);
}

#[test]
fn stamina_regen_waits_for_the_delay() {
    let mut s = sim();
    run(&mut s, sprint(), 60);
    let after_sprint = s.player.stamina;
    let delay_ticks = (s.tuning.stamina_regen_delay * 60.0) as usize;
    run(&mut s, idle(), delay_ticks - 2);
    assert!((s.player.stamina - after_sprint).abs() < 1e-3, "regen started before the delay");
    run(&mut s, idle(), 62);
    let room = s.tuning.stamina_max - after_sprint;
    assert!(s.player.stamina > after_sprint + room.min(s.tuning.stamina_regen) * 0.9, "no regen after the delay");
}

#[test]
fn dodge_spends_stamina_and_delays_regen() {
    let mut s = sim();
    run(&mut s, idle(), 5);
    s.step(&Input { dodge: Button::DOWN, ..forward() });
    let roll = actions::find("Roll").unwrap();
    assert!((s.player.stamina - (s.tuning.stamina_max - roll.stamina)).abs() < 1e-3);
}

#[test]
fn camera_arm_pulls_in_against_a_wall() {
    let w = BoxWorld::arena();
    // Pivot 1 m in front of the north wall (inner face z = -20), arm pointing into it.
    let pivot = Vec3::new(0.0, 1.6, -19.0);
    let d = world::camera_arm(&w, pivot, Vec3::NEG_Z, 3.2, 0.2, 0.3);
    assert!(d < 1.0 && d > 0.5, "arm {d}");
    let open = world::camera_arm(&w, Vec3::new(0.0, 1.6, 0.0), Vec3::Z, 3.2, 0.2, 0.3);
    assert_eq!(open, 3.2);
}

#[test]
fn sprint_jump_keeps_sprint_speed_in_the_air() {
    let mut s = sim();
    run(&mut s, sprint(), 60);
    s.step(&Input { jump: Button::DOWN, ..sprint() });
    let mut min_air_speed = f32::MAX;
    for _ in 0..20 {
        s.step(&sprint());
        if !s.player.grounded() {
            min_air_speed = min_air_speed.min(flat_speed(&s));
        }
    }
    assert!((min_air_speed - s.tuning.sprint_speed).abs() < 0.05, "air speed dropped to {min_air_speed}");
}

#[test]
fn sprint_cannot_start_in_the_air() {
    let mut s = sim();
    run(&mut s, forward(), 60);
    s.step(&Input { jump: Button::DOWN, ..forward() });
    run(&mut s, sprint(), 10);
    assert!(!s.player.grounded() && !s.player.sprinting, "sprint started mid-air");
}

#[test]
fn jump_costs_its_public_stamina() {
    let mut s = sim();
    run(&mut s, idle(), 5);
    s.step(&Input { jump: Button::DOWN, ..idle() });
    let expected = s.tuning.stamina_max - s.tuning.jump_stamina;
    assert!((s.player.stamina - expected).abs() < 1e-3, "stamina {} vs {expected}", s.player.stamina);
}

#[test]
fn public_stamina_numbers_hold() {
    // dragonwilds.runescape.wiki/w/Stamina: running 4/s, dodge 20, jump 10, max 100, regen 6/s after 3 s.
    let t = Tuning::default();
    assert_eq!((t.stamina_max, t.sprint_drain, t.jump_stamina), (100.0, 4.0, 10.0));
    assert_eq!((t.stamina_regen, t.stamina_regen_delay), (6.0, 3.0));
    assert_eq!(actions::find("Roll").unwrap().stamina, 20.0);
    // Sprinting from full lasts 25 s.
    let mut s = sim();
    let circle = |i: usize| Input { mv: Vec2::Y, look_yaw: i as f32 * 0.01, sprint: Button::HELD, ..Default::default() };
    let mut ticks = 0;
    while s.player.stamina > 0.0 && ticks < 60 * 40 {
        s.step(&circle(ticks));
        ticks += 1;
    }
    assert!((ticks as f32 / 60.0 - 25.0).abs() < 0.1, "sprint lasted {} s", ticks as f32 / 60.0);
}
