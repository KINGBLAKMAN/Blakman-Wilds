//! The game as a pure, deterministic fixed-step state machine.
//!
//! `Sim::step(&Input)` advances everything by one 60 Hz tick. No engine types, no clock, no
//! randomness without a seed: the same inputs always give the same result, so every behaviour
//! can be tested headless and replayed exactly. The app only feeds input and draws state.

pub mod actions;
pub mod events;
pub mod input;
pub mod player;
pub mod tuning;
pub mod world;

#[cfg(test)]
mod tests;

/// The data table: generated from the installed game, or the ESTIMATE placeholder.
pub mod extracted {
    include!(concat!(env!("OUT_DIR"), "/extracted.rs"));
}

use glam::Vec3;

pub use events::Event;
pub use input::{Button, Input};
pub use player::{Player, State};
pub use tuning::Tuning;
pub use world::{Aabb, BoxWorld, SweepHit, World};

pub const TICK_HZ: f64 = 60.0;
pub const DT: f32 = 1.0 / 60.0;

/// Something to hit: a vertical capsule.
#[derive(Clone, Debug)]
pub struct Target {
    pub pos: Vec3,
    pub radius: f32,
    pub height: f32,
    pub hp: f32,
    pub max_hp: f32,
    pub respawn: f32,
}

impl Target {
    pub fn new(pos: Vec3, hp: f32) -> Self {
        Self { pos, radius: 0.4, height: 1.8, hp, max_hp: hp, respawn: 0.0 }
    }

    pub fn alive(&self) -> bool {
        self.hp > 0.0
    }
}

pub struct Sim {
    pub world: BoxWorld,
    pub tuning: Tuning,
    pub player: Player,
    pub targets: Vec<Target>,
    /// Events from the last tick only. Presentation (HUD, audio, VFX) reads these; it never
    /// reaches into gameplay state to decide what happened.
    pub events: Vec<Event>,
    /// Ticks both sides stay frozen after a blow lands.
    pub hit_stop: u32,
    pub tick: u64,
}

impl Sim {
    pub fn new(world: BoxWorld, spawn: Vec3) -> Self {
        let tuning = Tuning::default();
        let player = Player::new(spawn, &tuning);
        Self { world, tuning, player, targets: Vec::new(), events: Vec::new(), hit_stop: 0, tick: 0 }
    }

    /// The default sandbox: the arena with one target in front of the spawn.
    pub fn sandbox() -> Self {
        let mut sim = Self::new(BoxWorld::arena(), Vec3::ZERO);
        sim.targets.push(Target::new(Vec3::new(0.0, 0.0, -2.2), 100.0));
        sim
    }

    pub fn step(&mut self, input: &Input) {
        self.events.clear();
        self.tick += 1;

        if self.hit_stop > 0 {
            self.hit_stop -= 1;
            return;
        }
        self.player.step(input, &self.world, &self.tuning, &mut self.events);
        self.separate_bodies();
        self.resolve_hits();
        self.respawn_targets();
    }

    /// Characters are solid to each other: you cannot run or roll through a target.
    fn separate_bodies(&mut self) {
        let half = self.tuning.half_extents;
        for t in &self.targets {
            let p = &mut self.player;
            if !t.alive() || (p.pos.y - t.pos.y).abs() > t.height {
                continue;
            }
            let mut away = Vec3::new(p.pos.x - t.pos.x, 0.0, p.pos.z - t.pos.z);
            let min = t.radius + half.x;
            let dist = away.length();
            if dist >= min {
                continue;
            }
            if dist < 1e-4 {
                away = -p.facing();
            }
            let push = away.normalize() * (min - dist);
            // Push through the level's collision so a target can never shove you into a wall.
            let r = world::move_and_slide(&self.world, half, p.pos + Vec3::Y * half.y, push / DT, DT);
            p.pos = r.center - Vec3::Y * half.y;
        }
    }

    /// A swing lands when the weapon line actually reaches a target, once per action.
    fn resolve_hits(&mut self) {
        let Some((def, from, to)) = self.player.weapon_line() else { return };
        for (i, t) in self.targets.iter_mut().enumerate() {
            if !t.alive() {
                continue;
            }
            let (a, b) = (t.pos + Vec3::Y * t.radius, t.pos + Vec3::Y * (t.height - t.radius));
            if world::segment_distance(from, to, a, b) > t.radius + self.tuning.weapon_radius {
                continue;
            }
            t.hp = (t.hp - def.damage).max(0.0);
            self.player.mark_connected();
            self.hit_stop = self.tuning.hit_stop_ticks;
            self.events.push(Event::Hit { target: i, damage: def.damage, hp_left: t.hp });
            if !t.alive() {
                t.respawn = self.tuning.target_respawn;
                self.events.push(Event::Killed { target: i });
            }
            break;
        }
    }

    fn respawn_targets(&mut self) {
        for t in &mut self.targets {
            if !t.alive() {
                t.respawn -= DT;
                if t.respawn <= 0.0 {
                    t.hp = t.max_hp;
                }
            }
        }
    }
}

/// Yaw 0 faces -Z (Bevy's forward); positive yaw turns toward -X (counter-clockwise from above).
pub fn dir_of(yaw: f32) -> Vec3 {
    Vec3::new(-yaw.sin(), 0.0, -yaw.cos())
}

pub fn yaw_of(dir: Vec3) -> f32 {
    (-dir.x).atan2(-dir.z)
}

pub fn angle_diff(from: f32, to: f32) -> f32 {
    use std::f32::consts::{PI, TAU};
    (to - from + PI).rem_euclid(TAU) - PI
}

pub fn turn_toward(yaw: f32, goal: f32, max_step: f32) -> f32 {
    yaw + angle_diff(yaw, goal).clamp(-max_step, max_step)
}
