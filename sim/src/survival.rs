//! Survival gameplay: inventory, skills, gathering, crafting, hunger, day/night, wolves, death
//! and the Beacon win. All of it is driven from `Sim::step`; numbers come from `Content`.

use crate::content::{Content, EnemyKindId, ItemId, ItemKind, NodeKindId, RecipeId, Skill, ToolKind};
use crate::events::{CraftError, Event};
use crate::world::{self, BoxWorld};
use crate::{dir_of, turn_toward, yaw_of, Player, Sim, DT};
use glam::Vec3;

// ---------------------------------------------------------------------------------------------
// Inventory and skills

/// Item counts indexed by `ItemId`. A plain vector keeps iteration order fixed (determinism).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Inventory {
    counts: Vec<u32>,
}

impl Inventory {
    pub fn new(items: usize) -> Self {
        Self { counts: vec![0; items] }
    }

    pub fn count(&self, item: ItemId) -> u32 {
        self.counts.get(item).copied().unwrap_or(0)
    }

    pub fn add(&mut self, item: ItemId, n: u32) {
        if item >= self.counts.len() {
            self.counts.resize(item + 1, 0);
        }
        self.counts[item] += n;
    }

    pub fn has_all(&self, needs: &[(ItemId, u32)]) -> bool {
        needs.iter().all(|&(i, n)| self.count(i) >= n)
    }

    /// Removes the items if all are present; returns whether it did.
    pub fn take_all(&mut self, needs: &[(ItemId, u32)]) -> bool {
        if !self.has_all(needs) {
            return false;
        }
        for &(i, n) in needs {
            self.counts[i] -= n;
        }
        true
    }

    /// Non-empty stacks in item order.
    pub fn stacks(&self) -> impl Iterator<Item = (ItemId, u32)> + '_ {
        self.counts.iter().enumerate().filter(|(_, &n)| n > 0).map(|(i, &n)| (i, n))
    }

    /// The best tool of a kind you carry: highest tier, then highest power. Auto-selected on
    /// every swing, so there is no hotbar to manage.
    pub fn best_tool(&self, c: &Content, kind: ToolKind) -> Option<(ItemId, u32, f32)> {
        self.stacks()
            .filter_map(|(i, _)| match c.items[i].kind {
                ItemKind::Tool { kind: k, tier, power } if k == kind => Some((i, tier, power)),
                _ => None,
            })
            .max_by(|a, b| (a.1, a.2).partial_cmp(&(b.1, b.2)).unwrap())
    }

    /// What you hit a wolf with: the best weapon, else the strongest tool, else fists (None).
    pub fn best_weapon(&self, c: &Content) -> Option<(ItemId, f32)> {
        if let Some((i, _, p)) = self.best_tool(c, ToolKind::Weapon) {
            return Some((i, p));
        }
        self.stacks()
            .filter_map(|(i, _)| match c.items[i].kind {
                ItemKind::Tool { power, .. } => Some((i, power)),
                _ => None,
            })
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
    }

    /// The most filling food you carry.
    pub fn best_food(&self, c: &Content) -> Option<(ItemId, f32, f32)> {
        self.stacks()
            .filter_map(|(i, _)| match c.items[i].kind {
                ItemKind::Food { hunger, heal } => Some((i, hunger, heal)),
                _ => None,
            })
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Skills {
    pub xp: [f32; 4],
}

impl Skills {
    pub fn level(&self, c: &Content, s: Skill) -> u32 {
        c.level_for_xp(self.xp[s.index()])
    }
}

// ---------------------------------------------------------------------------------------------
// World objects

#[derive(Clone, Debug)]
pub struct Node {
    pub kind: NodeKindId,
    pub pos: Vec3,
    /// Hits left before the node is depleted.
    pub charges: u32,
    /// Seconds until a depleted node grows back.
    pub respawn: f32,
}

impl Node {
    pub fn alive(&self) -> bool {
        self.charges > 0
    }
}

/// Something built: a crafting station or the Beacon.
#[derive(Clone, Debug)]
pub struct Structure {
    pub item: ItemId,
    pub pos: Vec3,
    pub yaw: f32,
    pub lit: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WolfState {
    Wander,
    Chase,
    /// Telegraph: crouched, about to bite. Dodge now.
    Windup,
    Recover,
}

#[derive(Clone, Debug)]
pub struct Wolf {
    /// Stable id for events (indices shift when wolves die).
    pub id: u32,
    pub kind: EnemyKindId,
    pub pos: Vec3,
    pub yaw: f32,
    pub hp: f32,
    pub state: WolfState,
    pub timer: f32,
}

pub const WOLF_HEIGHT: f32 = 0.9;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GameState {
    Playing,
    /// Seconds until you wake at camp.
    Dead(f32),
    Victory,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Vitals {
    pub health: f32,
    pub hunger: f32,
    starving: bool,
}

/// Time of day in [0, 1): 0 midnight, 0.25 dawn, 0.5 noon.
#[derive(Clone, Debug, PartialEq)]
pub struct Clock {
    pub time: f32,
    pub day: u32,
    pub night: u32,
    pub is_night: bool,
}

impl Clock {
    pub fn night_at(c: &Content, time: f32) -> bool {
        time >= c.tunables.night_start || time < c.tunables.night_end
    }

    /// 0 at full dark, 1 at noon: for lighting.
    pub fn daylight(&self) -> f32 {
        (0.5 - 0.5 * (self.time * std::f32::consts::TAU).cos()).clamp(0.0, 1.0)
    }
}

/// xorshift64*: tiny, seeded, deterministic.
#[derive(Clone, Debug)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed.max(1))
    }

    pub fn next_u32(&mut self) -> u32 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        (self.0.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 32) as u32
    }

    /// Uniform in [0, 1).
    pub fn next_f32(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 / (1u32 << 24) as f32
    }
}

/// Everything the survival layer owns, kept in one place on `Sim`.
#[derive(Clone, Debug)]
pub struct Survival {
    pub inv: Inventory,
    pub skills: Skills,
    pub vitals: Vitals,
    pub clock: Clock,
    pub nodes: Vec<Node>,
    pub structures: Vec<Structure>,
    pub wolves: Vec<Wolf>,
    pub state: GameState,
    pub camp: Vec3,
    pub wolf_spawns: Vec<Vec3>,
    rng: Rng,
    next_wolf: u32,
}

impl Survival {
    pub fn new(c: &Content, camp: Vec3) -> Self {
        let t = &c.tunables;
        Self {
            inv: Inventory::new(c.items.len()),
            skills: Skills::default(),
            vitals: Vitals { health: t.health_max, hunger: t.hunger_max, starving: false },
            clock: Clock { time: t.start_time, day: 1, night: 0, is_night: Clock::night_at(c, t.start_time) },
            nodes: Vec::new(),
            structures: Vec::new(),
            wolves: Vec::new(),
            state: GameState::Playing,
            camp,
            wolf_spawns: Vec::new(),
            rng: Rng::new(0x9E37_79B9_7F4A_7C15),
            next_wolf: 0,
        }
    }

    pub fn add_node(&mut self, c: &Content, kind: NodeKindId, pos: Vec3) {
        self.nodes.push(Node { kind, pos, charges: c.nodes[kind].charges, respawn: 0.0 });
    }

    /// Is a built station of this kind within reach of `pos`?
    pub fn station_near(&self, c: &Content, item: ItemId, pos: Vec3) -> bool {
        let r = c.tunables.station_radius;
        self.structures.iter().any(|s| s.item == item && flat_dist(s.pos, pos) <= r)
    }
}

pub fn flat_dist(a: Vec3, b: Vec3) -> f32 {
    Vec3::new(a.x - b.x, 0.0, a.z - b.z).length()
}

/// The island: a flat square of land with invisible walls at the shore.
pub fn island_world(half: f32) -> BoxWorld {
    let (h, t) = (half, half + 1.0);
    BoxWorld::default()
        .with([-t, -1.0, -t], [t, 0.0, t])
        .with([-t, 0.0, -t], [t, 3.0, -h])
        .with([-t, 0.0, h], [t, 3.0, t])
        .with([-t, 0.0, -h], [-h, 3.0, h])
        .with([h, 0.0, -h], [t, 3.0, h])
}

// ---------------------------------------------------------------------------------------------
// Rules

impl Sim {
    /// The Emberwild island from `data/content.ron`: nodes placed, player at camp.
    pub fn island() -> Self {
        Self::island_with(crate::content::Content::default())
    }

    pub fn island_with(content: Content) -> Self {
        let l = &content.layout;
        let camp = Vec3::new(l.camp.0, 0.0, l.camp.1);
        let mut sim = Sim::with_content(island_world(l.half_size), camp, content);
        let c = &sim.content;
        for (id, x, z) in &c.layout.nodes {
            let kind = c.node_kind(id).expect("validated on load");
            sim.survival.add_node(c, kind, Vec3::new(*x, 0.0, *z));
        }
        sim.survival.wolf_spawns = c.layout.wolf_spawns.iter().map(|&(x, z)| Vec3::new(x, 0.0, z)).collect();
        sim
    }

    pub fn level(&self, s: Skill) -> u32 {
        self.survival.skills.level(&self.content, s)
    }

    pub fn alive(&self) -> bool {
        self.survival.state == GameState::Playing
    }

    pub(crate) fn add_xp(&mut self, s: Skill, amount: f32) {
        if amount <= 0.0 {
            return;
        }
        let before = self.level(s);
        self.survival.skills.xp[s.index()] += amount;
        self.events.push(Event::XpGained { skill: s, amount });
        let after = self.level(s);
        if after > before {
            self.events.push(Event::LevelUp { skill: s, level: after });
        }
    }

    /// Why a recipe can't be crafted right now, or None if it can.
    pub fn craft_blocker(&self, r: RecipeId) -> Option<CraftError> {
        let rec = &self.content.recipes[r];
        if self.level(Skill::Crafting) < rec.level {
            return Some(CraftError::Level);
        }
        if let Some(st) = rec.station_item {
            if !self.survival.station_near(&self.content, st, self.player.pos) {
                return Some(CraftError::Station);
            }
        }
        if !self.survival.inv.has_all(&rec.ins) {
            return Some(CraftError::Items);
        }
        None
    }

    pub(crate) fn craft(&mut self, r: RecipeId) {
        if r >= self.content.recipes.len() {
            return;
        }
        if let Some(err) = self.craft_blocker(r) {
            self.events.push(Event::CraftFailed { recipe: r, reason: err });
            return;
        }
        let rec = &self.content.recipes[r];
        let (out, count, xp) = (rec.out, rec.count, rec.xp);
        self.survival.inv.take_all(&rec.ins.clone());
        if let ItemKind::Structure { .. } = self.content.items[out].kind {
            // Built things go straight into the world in front of you.
            let p = &self.player;
            let pos = p.pos + p.facing() * self.content.tunables.place_distance;
            self.survival.structures.push(Structure { item: out, pos: Vec3::new(pos.x, 0.0, pos.z), yaw: p.yaw, lit: false });
            self.events.push(Event::Built { structure: self.survival.structures.len() - 1, item: out });
        } else {
            self.survival.inv.add(out, count);
        }
        self.events.push(Event::ItemCrafted { item: out, count });
        self.add_xp(Skill::Crafting, xp);
    }

    pub(crate) fn eat(&mut self) {
        let max = self.content.tunables.hunger_max;
        let health_max = self.content.tunables.health_max;
        let v = &mut self.survival.vitals;
        if v.hunger >= max {
            return;
        }
        let Some((item, hunger, heal)) = self.survival.inv.best_food(&self.content) else { return };
        self.survival.inv.take_all(&[(item, 1)]);
        let v = &mut self.survival.vitals;
        v.hunger = (v.hunger + hunger).min(max);
        v.health = (v.health + heal).min(health_max);
        v.starving = false;
        self.events.push(Event::Ate { item });
    }

    /// Interact: light the Beacon if you stand by it with fuel.
    pub(crate) fn interact(&mut self) {
        let c = &self.content;
        let range = c.tunables.interact_range;
        let pos = self.player.pos;
        let Some(i) = self
            .survival
            .structures
            .iter()
            .position(|s| s.item == c.beacon && !s.lit && flat_dist(s.pos, pos) <= range)
        else {
            return;
        };
        if !self.survival.inv.take_all(&[(c.fuel, 1)]) {
            self.events.push(Event::NeedFuel);
            return;
        }
        self.survival.structures[i].lit = true;
        self.survival.state = GameState::Victory;
        self.events.push(Event::BeaconLit);
    }

    pub(crate) fn hurt_player(&mut self, damage: f32) {
        if !self.alive() || self.player.invincible() {
            return;
        }
        let v = &mut self.survival.vitals;
        v.health = (v.health - damage).max(0.0);
        let hp_left = v.health;
        self.events.push(Event::PlayerHurt { damage, hp_left });
        if hp_left <= 0.0 {
            self.die();
        }
    }

    fn die(&mut self) {
        self.survival.state = GameState::Dead(self.content.tunables.respawn_delay);
        self.events.push(Event::PlayerDied);
    }

    /// Dead: count down, then wake at camp. The clock keeps running meanwhile.
    pub(crate) fn step_dead(&mut self) {
        let GameState::Dead(t) = self.survival.state else { return };
        let t = t - DT;
        if t > 0.0 {
            self.survival.state = GameState::Dead(t);
            return;
        }
        let tun = &self.content.tunables;
        self.player = Player::new(self.survival.camp, &self.tuning);
        let v = &mut self.survival.vitals;
        v.health = tun.health_max;
        v.hunger = v.hunger.max(tun.respawn_hunger);
        v.starving = false;
        self.survival.state = GameState::Playing;
        self.events.push(Event::PlayerRespawned);
    }

    /// Hunger drains; empty hunger starves you; a full belly heals.
    pub(crate) fn step_vitals(&mut self) {
        if !self.alive() {
            return;
        }
        let t = &self.content.tunables;
        let v = &mut self.survival.vitals;
        v.hunger = (v.hunger - t.hunger_drain * DT).max(0.0);
        if v.hunger <= 0.0 {
            if !v.starving {
                v.starving = true;
                self.events.push(Event::Starving);
            }
            v.health = (v.health - t.starve_damage * DT).max(0.0);
            if v.health <= 0.0 {
                self.die();
            }
        } else if v.hunger > t.regen_hunger_min {
            v.health = (v.health + t.health_regen * DT).min(t.health_max);
        }
    }

    /// Advance the day/night clock; nightfall brings wolves, daybreak sends them away.
    pub(crate) fn step_clock(&mut self) {
        let len = self.content.tunables.day_length;
        let clock = &mut self.survival.clock;
        clock.time += DT / len;
        if clock.time >= 1.0 {
            clock.time -= 1.0;
        }
        let night = Clock::night_at(&self.content, clock.time);
        if night == clock.is_night {
            return;
        }
        clock.is_night = night;
        if night {
            clock.night += 1;
            let n = clock.night;
            self.events.push(Event::NightFell { night: n });
            let t = &self.content.tunables;
            let count = t.wolves_first_night + t.wolves_per_night * (n - 1);
            self.spawn_wolves(count);
        } else {
            clock.day += 1;
            let d = clock.day;
            self.events.push(Event::DayBroke { day: d });
            self.survival.wolves.clear();
        }
    }

    pub fn spawn_wolves(&mut self, count: u32) {
        let Some(kind) = self.content.enemy_kind("wolf") else { return };
        let spawns = &self.survival.wolf_spawns;
        if spawns.is_empty() {
            return;
        }
        let start = self.survival.rng.next_u32() as usize;
        for k in 0..count as usize {
            let base = self.survival.wolf_spawns[(start + k) % self.survival.wolf_spawns.len()];
            let jitter = Vec3::new(self.survival.rng.next_f32() - 0.5, 0.0, self.survival.rng.next_f32() - 0.5) * 4.0;
            self.spawn_wolf(kind, base + jitter);
        }
    }

    pub fn spawn_wolf(&mut self, kind: EnemyKindId, pos: Vec3) -> u32 {
        let id = self.survival.next_wolf;
        self.survival.next_wolf += 1;
        let hp = self.content.enemies[kind].hp;
        self.survival.wolves.push(Wolf { id, kind, pos, yaw: 0.0, hp, state: WolfState::Wander, timer: 0.0 });
        self.events.push(Event::WolfSpawned { wolf: id });
        id
    }

    pub(crate) fn step_nodes(&mut self) {
        for (i, n) in self.survival.nodes.iter_mut().enumerate() {
            if n.alive() {
                continue;
            }
            n.respawn -= DT;
            if n.respawn <= 0.0 {
                n.charges = self.content.nodes[n.kind].charges;
                self.events.push(Event::NodeRespawned { node: i });
            }
        }
    }

    /// Wolf brains: wander until they see you, chase, crouch (telegraph), bite, recover.
    pub(crate) fn step_wolves(&mut self) {
        let target = self.alive().then_some(self.player.pos);
        let mut bites = Vec::new();
        for w in &mut self.survival.wolves {
            let def = &self.content.enemies[w.kind];
            let to_player = target.map(|p| Vec3::new(p.x - w.pos.x, 0.0, p.z - w.pos.z));
            let dist = to_player.map_or(f32::INFINITY, |d| d.length());
            let mut speed = 0.0;
            w.timer -= DT;
            match w.state {
                WolfState::Wander => {
                    if w.timer <= 0.0 {
                        w.yaw = self.survival.rng.next_f32() * std::f32::consts::TAU;
                        w.timer = 2.0 + 2.0 * self.survival.rng.next_f32();
                    }
                    speed = def.wander_speed;
                    if dist <= def.sight {
                        w.state = WolfState::Chase;
                    }
                }
                WolfState::Chase => {
                    if let Some(d) = to_player.and_then(|d| d.try_normalize()) {
                        w.yaw = turn_toward(w.yaw, yaw_of(d), 8.0 * DT);
                    }
                    speed = def.chase_speed;
                    if dist <= def.bite_range {
                        w.state = WolfState::Windup;
                        w.timer = def.windup;
                        self.events.push(Event::WolfTelegraph { wolf: w.id });
                        speed = 0.0;
                    } else if dist > def.sight * 1.5 {
                        w.state = WolfState::Wander;
                        w.timer = 0.0;
                    }
                }
                WolfState::Windup => {
                    if let Some(d) = to_player.and_then(|d| d.try_normalize()) {
                        w.yaw = turn_toward(w.yaw, yaw_of(d), 4.0 * DT);
                    }
                    if w.timer <= 0.0 {
                        // The bite lands on whatever is in front of the jaws now: roll away or
                        // through it (i-frames) and it whiffs.
                        let reach = def.bite_range + 0.4;
                        bites.push((w.id, dist <= reach, def.bite_damage));
                        w.state = WolfState::Recover;
                        w.timer = def.recover;
                    }
                }
                WolfState::Recover => {
                    if w.timer <= 0.0 {
                        w.state = WolfState::Chase;
                    }
                }
            }
            if speed > 0.0 {
                let half = Vec3::new(def.radius, WOLF_HEIGHT * 0.5, def.radius);
                let r = world::move_and_slide(&self.world, half, w.pos + Vec3::Y * (half.y + 0.01), dir_of(w.yaw) * speed, DT);
                w.pos = Vec3::new(r.center.x, w.pos.y, r.center.z);
                if r.wall && w.state == WolfState::Wander {
                    w.timer = 0.0;
                }
            }
        }
        for (id, in_reach, damage) in bites {
            if !in_reach || !self.alive() {
                continue;
            }
            if self.player.invincible() {
                self.events.push(Event::BiteDodged { wolf: id });
            } else {
                self.hurt_player(damage);
            }
        }
    }

    /// Damage a swing does to a wolf: best weapon (auto-selected) scaled by Combat level.
    pub fn weapon_damage(&self) -> f32 {
        let t = &self.content.tunables;
        let base = self.survival.inv.best_weapon(&self.content).map_or(t.unarmed_damage, |(_, p)| p);
        base * (1.0 + t.combat_damage_per_level * (self.level(Skill::Combat) - 1) as f32)
    }

    /// The swing's weapon line reached a wolf.
    pub(crate) fn hit_wolf(&mut self, idx: usize) {
        let damage = self.weapon_damage();
        let w = &mut self.survival.wolves[idx];
        w.hp = (w.hp - damage).max(0.0);
        let (id, hp_left, kind) = (w.id, w.hp, w.kind);
        self.events.push(Event::WolfHit { wolf: id, damage, hp_left });
        if hp_left > 0.0 {
            return;
        }
        self.survival.wolves.remove(idx);
        let def = &self.content.enemies[kind];
        let (drops, xp) = (def.drop_items.clone(), def.xp);
        for (item, n) in drops {
            self.survival.inv.add(item, n);
            self.events.push(Event::ItemGathered { item, count: n });
        }
        self.events.push(Event::EnemyKilled { kind });
        self.add_xp(Skill::Combat, xp);
    }

    /// The swing's weapon line reached a resource node: the best tool for it is used
    /// automatically; too weak a tool (or too low a level) just bounces off.
    pub(crate) fn hit_node(&mut self, idx: usize) {
        let c = &self.content;
        let n = &self.survival.nodes[idx];
        let def = &c.nodes[n.kind];
        let tier = def.tool.and_then(|k| self.survival.inv.best_tool(c, k)).map_or(0, |t| t.1);
        let level = def.skill.map_or(1, |s| self.level(s));
        if tier < def.min_tier || level < def.level {
            self.events.push(Event::GatherBlocked { node: idx });
            return;
        }
        let bonus = (level - 1) / c.tunables.yield_levels_per_bonus.max(1);
        let count = def.base_yield + tier + bonus;
        let (item, skill, xp, respawn) = (def.drop_item, def.skill, def.xp, def.respawn);
        self.survival.inv.add(item, count);
        self.events.push(Event::ItemGathered { item, count });
        let n = &mut self.survival.nodes[idx];
        n.charges -= 1;
        if n.charges == 0 {
            n.respawn = respawn;
            self.events.push(Event::NodeDepleted { node: idx });
        }
        if let Some(s) = skill {
            self.add_xp(s, xp);
        }
    }
}
