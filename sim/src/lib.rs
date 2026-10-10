//! The game as a pure, deterministic fixed-step state machine.
//!
//! `Sim::step(&Input)` advances everything by one 60 Hz tick. No engine types, no clock, no
//! randomness without a seed: the same inputs always give the same result, so every behaviour
//! can be tested headless and replayed exactly. The app only feeds input and draws state.

pub mod actions;
pub mod bot;
pub mod content;
pub mod events;
pub mod input;
pub mod player;
pub mod survival;
pub mod tuning;
pub mod world;

#[cfg(test)]
mod survival_tests;
#[cfg(test)]
mod tests;

/// The data table: generated from the installed game, or the ESTIMATE placeholder.
pub mod extracted {
    include!(concat!(env!("OUT_DIR"), "/extracted.rs"));
}

use glam::Vec3;

pub use content::{Content, Skill, ToolKind};
pub use events::{CraftError, Event};
pub use input::{Button, Input};
pub use player::{Player, State};
pub use survival::{GameState, Survival, WolfState};
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
    /// Game content (items, recipes, nodes, enemies, tunables) from data/content.ron.
    pub content: Content,
    /// Survival state: inventory, skills, vitals, clock, nodes, structures, wolves.
    pub survival: Survival,
}

impl Sim {
    pub fn new(world: BoxWorld, spawn: Vec3) -> Self {
        Self::with_content(world, spawn, Content::default())
    }

    /// `spawn` doubles as the camp you wake at after dying.
    pub fn with_content(world: BoxWorld, spawn: Vec3, content: Content) -> Self {
        let tuning = Tuning::default();
        let player = Player::new(spawn, &tuning);
        let survival = Survival::new(&content, spawn);
        Self { world, tuning, player, targets: Vec::new(), events: Vec::new(), hit_stop: 0, tick: 0, content, survival }
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
        if self.survival.state == GameState::Victory {
            return;
        }

        if self.hit_stop > 0 {
            self.hit_stop -= 1;
            return;
        }
        self.step_clock();
        if self.alive() {
            self.player.step(input, &self.world, &self.tuning, &mut self.events);
            if let Some(r) = input.craft {
                self.craft(r);
            }
            if input.eat.pressed {
                self.eat();
            }
            if input.interact.pressed {
                self.interact();
            }
        } else {
            self.step_dead();
        }
        self.separate_bodies();
        self.resolve_hits();
        self.respawn_targets();
        self.step_nodes();
        self.step_wolves();
        self.step_vitals();
    }

    /// Characters are solid to each other: you cannot run or roll through a target.
    fn separate_bodies(&mut self) {
        let half = self.tuning.half_extents;
        let nodes = self.survival.nodes.iter().filter(|n| n.alive()).map(|n| {
            let d = &self.content.nodes[n.kind];
            (n.pos, d.radius, d.height)
        });
        let built = self.survival.structures.iter().map(|s| (s.pos, self.content.footprint(s.item), 2.0));
        let bodies: Vec<_> = self.targets.iter().filter(|t| t.alive()).map(|t| (t.pos, t.radius, t.height)).chain(nodes).chain(built).collect();
        for (pos, radius, height) in bodies {
            let t = Target { pos, radius, height, hp: 1.0, max_hp: 1.0, respawn: 0.0 };
            let p = &mut self.player;
            if (p.pos.y - t.pos.y).abs() > t.height {
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

    /// A swing lands when the weapon line actually reaches something, once per action:
    /// a wolf first, then a resource node, then a sandbox target.
    fn resolve_hits(&mut self) {
        let Some((def, from, to)) = self.player.weapon_line() else { return };
        let r = self.tuning.weapon_radius;
        // Wolves and nodes: the swing sweeps down to the ground in front of you, so test reach
        // in the horizontal plane and only require the body to be at about your height.
        let feet = self.player.pos.y;
        let flat = |v: Vec3| Vec3::new(v.x, 0.0, v.z);
        let reaches = |pos: Vec3, radius: f32, height: f32| {
            let level = pos.y <= feet + 1.5 && pos.y + height >= feet - 0.5;
            level && world::segment_distance(flat(from), flat(to), flat(pos), flat(pos)) <= radius + r
        };
        let wolf = self.survival.wolves.iter().position(|w| {
            reaches(w.pos, self.content.enemies[w.kind].radius, survival::WOLF_HEIGHT)
        });
        if let Some(i) = wolf {
            self.player.mark_connected();
            self.hit_stop = self.tuning.hit_stop_ticks;
            self.hit_wolf(i);
            return;
        }
        let node = self.survival.nodes.iter().position(|n| {
            let d = &self.content.nodes[n.kind];
            n.alive() && reaches(n.pos, d.radius, d.height)
        });
        if let Some(i) = node {
            self.player.mark_connected();
            self.hit_stop = self.tuning.hit_stop_ticks;
            self.hit_node(i);
            return;
        }
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
