//! A scripted player for the island: it plays through the same `Input` a person gives (steer,
//! swing, craft, eat, dodge, light). Tests use it for a headless playthrough to victory; the app
//! uses it for REWRITE_AUTOPLAY demos and screenshots.

use crate::survival::{self, WolfState};
use crate::{angle_diff, yaw_of, Button, Input, Sim};
use crate::events::Event;
use glam::{Vec2, Vec3};

#[derive(Clone, Debug)]
pub enum Goal {
    Gather(&'static str, u32),
    Craft(&'static str),
    /// Collect this many wolf pelts: wait at camp for night, then hunt.
    Hunt(u32),
    Light,
}

pub struct Bot {
    pub goals: Vec<Goal>,
    pub deaths: u32,
    pub kills: u32,
    pub dodges: u32,
}

impl Bot {
    pub fn new() -> Self {
        use Goal::*;
        let goals = vec![
            Gather("log", 8), Gather("stone", 4), Craft("campfire"), Craft("workbench"),
            Gather("log", 3), Craft("wooden_club"),
            Gather("log", 4), Gather("stone", 6), Craft("stone_axe"), Craft("stone_pickaxe"),
            Gather("log", 1), Craft("torch"),
            Gather("stone", 8), Gather("log", 2), Craft("furnace"),
            Gather("iron_ore", 18), Gather("log", 6),
            Craft("iron_bar"), Craft("iron_bar"), Craft("iron_bar"),
            Craft("iron_bar"), Craft("iron_bar"), Craft("iron_bar"),
            Gather("log", 20), Gather("stone", 20), Hunt(2), Craft("beacon"), Light,
        ];
        Self { goals, deaths: 0, kills: 0, dodges: 0 }
    }

    /// Steer toward `target`; once within `reach`, face it and swing.
    fn approach(s: &Sim, target: Vec3, reach: f32, swing: bool) -> Input {
        let to = Vec3::new(target.x - s.player.pos.x, 0.0, target.z - s.player.pos.z);
        let dist = to.length();
        let yaw = yaw_of(to);
        let mut inp = Input { look_yaw: yaw, ..Default::default() };
        if dist > reach {
            inp.mv = Vec2::Y;
            return inp;
        }
        inp.mv = Vec2::Y * 0.04; // turn on the spot
        let aligned = angle_diff(s.player.yaw, yaw).abs() < 0.15;
        if swing && aligned && s.player.grounded() && s.player.stamina > 0.0 {
            inp.attack = Button::DOWN;
        }
        inp
    }

    pub fn decide(&mut self, s: &Sim) -> Input {
        let p = s.player.pos;
        let sv = &s.survival;
        if s.alive() && sv.vitals.hunger < 50.0 && sv.inv.best_food(&s.content).is_some() {
            return Input { eat: Button::DOWN, ..Default::default() };
        }
        if s.alive() && sv.vitals.hunger < 45.0 {
            let berries = s.content.item("berries").unwrap();
            if let Some(bush) = sv
                .nodes
                .iter()
                .filter(|nd| nd.alive() && s.content.nodes[nd.kind].drop_item == berries)
                .min_by(|a, b| a.pos.distance(p).partial_cmp(&b.pos.distance(p)).unwrap())
            {
                return Self::approach(s, bush.pos, s.content.nodes[bush.kind].radius + 1.0, true);
            }
        }
        // Threats first: dodge a telegraphed bite, otherwise fight the nearest wolf.
        if let Some(w) = sv.wolves.iter().min_by(|a, b| a.pos.distance(p).partial_cmp(&b.pos.distance(p)).unwrap()) {
            let d = w.pos.distance(p);
            if w.state == WolfState::Windup && d < 2.4 && w.timer < 0.25 && s.player.grounded() && s.player.stamina > 0.0 {
                self.dodges += 1;
                let away = yaw_of(p - w.pos);
                return Input { look_yaw: away, mv: Vec2::Y, dodge: Button::DOWN, ..Default::default() };
            }
            if d < 7.0 {
                return Self::approach(s, w.pos, 1.4, true);
            }
        }
        let camp = sv.camp;
        loop {
            let Some(goal) = self.goals.first().cloned() else { return Input::default() };
            match goal {
                Goal::Gather(id, n) => {
                    let it = s.content.item(id).unwrap();
                    if sv.inv.count(it) >= n {
                        // Gather goals are cumulative targets for the next craft: done.
                        self.goals.remove(0);
                        continue;
                    }
                    let node = sv
                        .nodes
                        .iter()
                        .filter(|nd| nd.alive() && s.content.nodes[nd.kind].drop_item == it)
                        .min_by(|a, b| a.pos.distance(p).partial_cmp(&b.pos.distance(p)).unwrap())
                        .expect("a node to gather from");
                    let r = s.content.nodes[node.kind].radius;
                    return Self::approach(s, node.pos, r + 1.0, true);
                }
                Goal::Craft(out) => {
                    if flat(p, camp) > 0.6 {
                        return Self::approach(s, camp, 0.5, false);
                    }
                    return Input { craft: s.content.recipe(out), ..Default::default() };
                }
                Goal::Hunt(n) => {
                    if sv.inv.count(s.content.item("wolf_pelt").unwrap()) >= n {
                        self.goals.remove(0);
                        continue;
                    }
                    if let Some(w) = sv.wolves.iter().min_by(|a, b| a.pos.distance(p).partial_cmp(&b.pos.distance(p)).unwrap()) {
                        return Self::approach(s, w.pos, 1.4, true);
                    }
                    return Self::approach(s, camp, 0.5, false);
                }
                Goal::Light => {
                    let beacon = sv.structures.iter().find(|st| st.item == s.content.beacon).expect("beacon built");
                    let reach = s.content.tunables.interact_range + s.content.footprint(beacon.item);
                    if flat(p, beacon.pos) > reach {
                        return Self::approach(s, beacon.pos, reach - 0.2, false);
                    }
                    return Input { interact: Button::DOWN, ..Default::default() };
                }
            }
        }
    }

    pub fn observe(&mut self, s: &Sim) {
        for e in &s.events {
            match e {
                Event::ItemCrafted { item, .. } => {
                    if let Some(Goal::Craft(out)) = self.goals.first() {
                        if s.content.item(out) == Some(*item) {
                            self.goals.remove(0);
                        }
                    }
                }
                Event::PlayerDied => self.deaths += 1,
                Event::EnemyKilled { .. } => self.kills += 1,
                _ => {}
            }
        }
    }
}

fn flat(a: Vec3, b: Vec3) -> f32 {
    survival::flat_dist(a, b)
}

impl Default for Bot {
    fn default() -> Self {
        Self::new()
    }
}
