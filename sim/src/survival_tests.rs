//! Survival behaviour tests: headless, driving the real `Sim::step`. The last ones play the whole
//! island with a scripted bot, from an empty inventory to the lit Beacon.

use crate::content::{Content, ItemId, RecipeId, Skill, EMBEDDED};
use crate::survival::{island_world, GameState, WolfState};
use crate::*;
use glam::{Vec2, Vec3};

fn content() -> Content {
    Content::default()
}

/// An empty island with the player at the origin facing -Z.
fn bare() -> Sim {
    Sim::with_content(island_world(50.0), Vec3::ZERO, content())
}

fn item(s: &Sim, id: &str) -> ItemId {
    s.content.item(id).unwrap()
}

fn recipe(s: &Sim, out: &str) -> RecipeId {
    s.content.recipe(out).unwrap()
}

fn give(s: &mut Sim, id: &str, n: u32) {
    let i = item(s, id);
    s.survival.inv.add(i, n);
}

fn count(s: &Sim, id: &str) -> u32 {
    s.survival.inv.count(item(s, id))
}

/// A node `dist` metres in front of the player (who faces -Z).
fn node_ahead(s: &mut Sim, id: &str, dist: f32) -> usize {
    let k = s.content.node_kind(id).unwrap();
    s.survival.add_node(&s.content.clone(), k, Vec3::new(0.0, 0.0, -dist));
    s.survival.nodes.len() - 1
}

fn run(s: &mut Sim, inp: Input, ticks: usize) -> Vec<Event> {
    let mut all = Vec::new();
    for _ in 0..ticks {
        s.step(&inp);
        all.extend(s.events.iter().copied());
    }
    all
}

/// One full swing: press, then let the action play out.
fn swing(s: &mut Sim) -> Vec<Event> {
    let mut ev = run(s, Input { attack: Button::DOWN, ..Default::default() }, 1);
    ev.extend(run(s, Input::default(), 70));
    s.player.stamina = s.tuning.stamina_max; // isolate gathering from stamina pacing
    ev
}

fn craft(s: &mut Sim, out: &str) -> Vec<Event> {
    let r = recipe(s, out);
    run(s, Input { craft: Some(r), ..Default::default() }, 1)
}

fn has<F: Fn(&Event) -> bool>(ev: &[Event], f: F) -> bool {
    ev.iter().any(f)
}

#[test]
fn content_loads_and_every_reference_resolves() {
    let c = content();
    assert!(c.items.len() >= 15 && c.recipes.len() >= 10);
    for r in &c.recipes {
        assert_eq!(c.items[r.out].id, r.output);
        assert_eq!(r.ins.len(), r.inputs.len());
    }
    let s = Sim::island();
    assert_eq!(s.survival.nodes.len(), c.layout.nodes.len());
    assert!(!s.survival.wolf_spawns.is_empty());
}

#[test]
fn a_bad_id_in_the_data_is_named() {
    let broken = EMBEDDED.replace("drop: \"iron_ore\"", "drop: \"iron_oar\"");
    let err = Content::load(&broken).unwrap_err();
    assert!(err.contains("iron_oar"), "{err}");
}

#[test]
fn xp_curve_levels() {
    let c = content();
    assert_eq!(c.level_for_xp(0.0), 1);
    assert_eq!(c.level_for_xp(c.xp_for_level(2) - 0.1), 1);
    assert_eq!(c.level_for_xp(c.xp_for_level(2)), 2);
    assert_eq!(c.level_for_xp(c.xp_for_level(4)), 4);
    assert_eq!(c.level_for_xp(1e9), c.tunables.max_level);
}

#[test]
fn best_tool_is_picked_automatically() {
    let mut s = bare();
    give(&mut s, "stone_axe", 1);
    give(&mut s, "iron_axe", 1);
    give(&mut s, "stone_pickaxe", 1);
    let (best, tier, _) = s.survival.inv.best_tool(&s.content, ToolKind::Axe).unwrap();
    assert_eq!(best, item(&s, "iron_axe"));
    assert_eq!(tier, 2);
    // No weapon: you hit wolves with your strongest tool.
    assert_eq!(s.survival.inv.best_weapon(&s.content).unwrap().0, item(&s, "iron_axe"));
}

#[test]
fn punching_a_tree_gives_a_log_and_woodcutting_xp() {
    let mut s = bare();
    node_ahead(&mut s, "tree_pine", 1.2);
    let ev = swing(&mut s);
    assert_eq!(count(&s, "log"), 1);
    assert!(has(&ev, |e| matches!(e, Event::XpGained { skill: Skill::Woodcutting, .. })));
}

#[test]
fn an_axe_raises_the_yield() {
    let mut s = bare();
    node_ahead(&mut s, "tree_pine", 1.2);
    give(&mut s, "stone_axe", 1);
    swing(&mut s);
    assert_eq!(count(&s, "log"), 2, "base 1 + tier 1");
}

#[test]
fn a_node_depletes_then_grows_back() {
    let mut s = bare();
    let n = node_ahead(&mut s, "rock", 1.4);
    let charges = s.content.nodes[s.survival.nodes[n].kind].charges;
    let mut ev = Vec::new();
    for _ in 0..charges {
        ev.extend(swing(&mut s));
    }
    assert!(has(&ev, |e| *e == Event::NodeDepleted { node: n }));
    assert_eq!(count(&s, "stone"), charges);
    // Depleted: further swings get nothing.
    swing(&mut s);
    assert_eq!(count(&s, "stone"), charges);
    let respawn = s.content.nodes[s.survival.nodes[n].kind].respawn;
    let ev = run(&mut s, Input::default(), (respawn * 60.0) as usize + 10);
    assert!(has(&ev, |e| *e == Event::NodeRespawned { node: n }));
    assert!(s.survival.nodes[n].alive());
}

#[test]
fn iron_needs_a_pickaxe_and_mining_level() {
    let mut s = bare();
    let n = node_ahead(&mut s, "iron_rock", 1.4);
    let ev = swing(&mut s);
    assert!(has(&ev, |e| *e == Event::GatherBlocked { node: n }), "bare hands bounce off");
    give(&mut s, "stone_pickaxe", 1);
    let ev = swing(&mut s);
    assert!(has(&ev, |e| *e == Event::GatherBlocked { node: n }), "Mining 1 is too low");
    s.survival.skills.xp[Skill::Mining.index()] = s.content.xp_for_level(2);
    swing(&mut s);
    assert_eq!(count(&s, "iron_ore"), 2, "base 1 + pickaxe tier 1");
}

#[test]
fn crafting_checks_items_station_and_level() {
    let mut s = bare();
    let ev = craft(&mut s, "campfire");
    assert!(has(&ev, |e| matches!(e, Event::CraftFailed { reason: CraftError::Items, .. })));

    give(&mut s, "log", 20);
    give(&mut s, "stone", 20);
    let ev = craft(&mut s, "stone_axe");
    assert!(has(&ev, |e| matches!(e, Event::CraftFailed { reason: CraftError::Station, .. })));
    let ev = craft(&mut s, "furnace");
    assert!(has(&ev, |e| matches!(e, Event::CraftFailed { reason: CraftError::Level, .. })));

    let ev = craft(&mut s, "workbench");
    assert!(has(&ev, |e| matches!(e, Event::Built { .. })));
    assert_eq!(count(&s, "log"), 15);
    let ev = craft(&mut s, "stone_axe");
    assert!(has(&ev, |e| matches!(e, Event::ItemCrafted { .. })));
    assert_eq!(count(&s, "stone_axe"), 1);
}

#[test]
fn a_station_must_be_near() {
    let mut s = bare();
    give(&mut s, "log", 20);
    give(&mut s, "stone", 20);
    craft(&mut s, "workbench");
    s.player.pos = Vec3::new(20.0, 0.0, 0.0);
    let ev = craft(&mut s, "stone_axe");
    assert!(has(&ev, |e| matches!(e, Event::CraftFailed { reason: CraftError::Station, .. })));
}

#[test]
fn smelting_at_the_furnace() {
    let mut s = bare();
    s.survival.skills.xp[Skill::Crafting.index()] = s.content.xp_for_level(2);
    give(&mut s, "log", 20);
    give(&mut s, "stone", 20);
    give(&mut s, "iron_ore", 6);
    craft(&mut s, "workbench");
    s.player.yaw = 1.5; // place the furnace beside the bench, not on it
    craft(&mut s, "furnace");
    craft(&mut s, "iron_bar");
    craft(&mut s, "iron_bar");
    assert_eq!(count(&s, "iron_bar"), 2);
    assert_eq!(count(&s, "iron_ore"), 0);
}

#[test]
fn crafting_levels_up_crafting() {
    let mut s = bare();
    give(&mut s, "log", 50);
    give(&mut s, "stone", 50);
    let mut ev = craft(&mut s, "campfire");
    ev.extend(craft(&mut s, "workbench"));
    assert!(has(&ev, |e| *e == Event::LevelUp { skill: Skill::Crafting, level: 2 }));
}

#[test]
fn hunger_drains_starves_and_food_restores() {
    let mut s = bare();
    let t = s.content.tunables.clone();
    let secs = 10.0;
    run(&mut s, Input::default(), (secs * 60.0) as usize);
    let expect = t.hunger_max - t.hunger_drain * secs;
    assert!((s.survival.vitals.hunger - expect).abs() < 0.05, "{}", s.survival.vitals.hunger);

    s.survival.vitals.hunger = 0.0;
    let ev = run(&mut s, Input::default(), 60);
    assert!(has(&ev, |e| *e == Event::Starving));
    assert!((s.survival.vitals.health - (t.health_max - t.starve_damage)).abs() < 0.1);

    give(&mut s, "berries", 1);
    give(&mut s, "cooked_meat", 1);
    let ev = run(&mut s, Input { eat: Button::DOWN, ..Default::default() }, 1);
    assert!(has(&ev, |e| matches!(e, Event::Ate { item } if *item == s.content.item("cooked_meat").unwrap())));
    assert!(s.survival.vitals.hunger > 30.0);
    assert_eq!(count(&s, "berries"), 1, "the most filling food is eaten first");
}

#[test]
fn starving_to_death_respawns_at_camp_with_the_clock_running() {
    let mut s = bare();
    s.player.pos = Vec3::new(10.0, 0.0, 10.0);
    s.survival.vitals.hunger = 0.0;
    s.survival.vitals.health = 1.0;
    let time_before = s.survival.clock.time;
    let ev = run(&mut s, Input::default(), 60);
    assert!(has(&ev, |e| *e == Event::PlayerDied));
    assert!(matches!(s.survival.state, GameState::Dead(_)));
    let wait = (s.content.tunables.respawn_delay * 60.0) as usize + 5;
    let ev = run(&mut s, Input::default(), wait);
    assert!(has(&ev, |e| *e == Event::PlayerRespawned));
    assert!(s.alive());
    assert!(s.player.pos.distance(s.survival.camp) < 0.1);
    assert_eq!(s.survival.vitals.health, s.content.tunables.health_max);
    assert!(s.survival.clock.time > time_before);
}

#[test]
fn night_falls_on_schedule_and_brings_wolves_then_day_sends_them_away() {
    let mut s = Sim::island();
    let t = s.content.tunables.clone();
    assert!(!s.survival.clock.is_night);
    let to_night = ((t.night_start - t.start_time) * t.day_length * 60.0) as usize;
    let ev = run(&mut s, Input::default(), to_night - 30);
    assert!(!has(&ev, |e| matches!(e, Event::NightFell { .. })));
    // Keep the player fed and out of reach so only the clock matters.
    s.player.pos = Vec3::new(0.0, 0.0, -45.0);
    let ev = run(&mut s, Input::default(), 60);
    assert!(has(&ev, |e| *e == Event::NightFell { night: 1 }));
    assert_eq!(s.survival.wolves.len() as u32, t.wolves_first_night);

    let night_len = ((1.0 - t.night_start + t.night_end) * t.day_length * 60.0) as usize;
    s.survival.vitals.hunger = t.hunger_max;
    let mut ev = Vec::new();
    for _ in 0..night_len + 60 {
        s.survival.vitals.health = t.health_max;
        s.step(&Input::default());
        ev.extend(s.events.iter().copied());
    }
    assert!(has(&ev, |e| *e == Event::DayBroke { day: 2 }));
    assert!(s.survival.wolves.is_empty());
}

#[test]
fn later_nights_bring_more_wolves() {
    let mut s = Sim::island();
    s.survival.clock.night = 2;
    s.survival.clock.time = s.content.tunables.night_start - 0.0001;
    run(&mut s, Input::default(), 60);
    let t = &s.content.tunables;
    assert_eq!(s.survival.wolves.len() as u32, t.wolves_first_night + 2 * t.wolves_per_night);
}

/// A wolf right next to the player, already hunting.
fn wolf_near(s: &mut Sim, dist: f32) -> u32 {
    let k = s.content.enemy_kind("wolf").unwrap();
    s.spawn_wolf(k, Vec3::new(0.0, 0.0, -dist))
}

#[test]
fn a_wolf_chases_telegraphs_and_bites() {
    let mut s = bare();
    let id = wolf_near(&mut s, 8.0);
    let def = s.content.enemies[0].clone();
    let mut ev = Vec::new();
    let mut telegraph_tick = None;
    for tick in 0..300 {
        s.step(&Input::default());
        if telegraph_tick.is_none() && has(&s.events, |e| *e == Event::WolfTelegraph { wolf: id }) {
            telegraph_tick = Some(tick);
        }
        ev.extend(s.events.iter().copied());
        if has(&s.events, |e| matches!(e, Event::PlayerHurt { .. })) {
            let t0 = telegraph_tick.expect("bit without a telegraph");
            let waited = (tick - t0) as f32 / 60.0;
            assert!((waited - def.windup).abs() < 0.05, "bite {waited}s after the telegraph");
            assert!((s.survival.vitals.health - (s.content.tunables.health_max - def.bite_damage)).abs() < 0.5);
            return;
        }
    }
    panic!("no bite: {ev:?}");
}

#[test]
fn rolling_through_the_bite_dodges_it() {
    let mut s = bare();
    let id = wolf_near(&mut s, 6.0);
    let def = s.content.enemies[0].clone();
    // Wait for the telegraph, then roll so the i-frames cover the moment the bite lands.
    for _ in 0..300 {
        s.step(&Input::default());
        if has(&s.events, |e| *e == Event::WolfTelegraph { wolf: id }) {
            break;
        }
    }
    let lead = (def.windup * 60.0) as usize - 6;
    run(&mut s, Input::default(), lead);
    let ev = run(&mut s, Input { dodge: Button::DOWN, ..Default::default() }, 1);
    let mut all = ev;
    all.extend(run(&mut s, Input::default(), 20));
    assert!(has(&all, |e| *e == Event::BiteDodged { wolf: id }), "{all:?}");
    assert_eq!(s.survival.vitals.health, s.content.tunables.health_max);
}

#[test]
fn killing_a_wolf_drops_meat_and_gives_combat_xp() {
    let mut s = bare();
    give(&mut s, "iron_sword", 1);
    let id = wolf_near(&mut s, 1.5);
    s.survival.wolves[0].state = WolfState::Recover; // hold still for the test
    s.survival.wolves[0].timer = 100.0;
    let mut ev = Vec::new();
    for _ in 0..4 {
        ev.extend(swing(&mut s));
    }
    assert!(has(&ev, |e| matches!(e, Event::WolfHit { wolf, .. } if *wolf == id)));
    assert!(has(&ev, |e| matches!(e, Event::EnemyKilled { .. })));
    assert!(s.survival.wolves.is_empty());
    assert_eq!(count(&s, "raw_meat"), 2);
    assert!(s.level(Skill::Combat) >= 2);
}

#[test]
fn combat_level_scales_weapon_damage() {
    let mut s = bare();
    let base = s.weapon_damage();
    assert_eq!(base, s.content.tunables.unarmed_damage);
    s.survival.skills.xp[Skill::Combat.index()] = s.content.xp_for_level(3);
    assert!((s.weapon_damage() - base * (1.0 + 2.0 * s.content.tunables.combat_damage_per_level)).abs() < 1e-4);
}

#[test]
fn lighting_the_beacon_needs_fuel_and_wins() {
    let mut s = bare();
    s.survival.skills.xp[Skill::Crafting.index()] = s.content.xp_for_level(4);
    give(&mut s, "log", 30);
    give(&mut s, "stone", 30);
    give(&mut s, "iron_bar", 6);
    give(&mut s, "wolf_pelt", 2);
    craft(&mut s, "workbench");
    s.player.yaw = std::f32::consts::PI; // build the beacon behind, on clear ground
    craft(&mut s, "beacon");
    let ev = run(&mut s, Input { interact: Button::DOWN, ..Default::default() }, 1);
    assert!(has(&ev, |e| *e == Event::NeedFuel));
    give(&mut s, "torch", 1);
    let ev = run(&mut s, Input { interact: Button::DOWN, ..Default::default() }, 1);
    assert!(has(&ev, |e| *e == Event::BeaconLit));
    assert_eq!(s.survival.state, GameState::Victory);
    // The world freezes on the victory screen.
    let tick_pos = s.player.pos;
    run(&mut s, Input { mv: Vec2::Y, ..Default::default() }, 30);
    assert_eq!(s.player.pos, tick_pos);
}

// ---------------------------------------------------------------------------------------------
// Scripted playthrough

#[derive(Clone, Debug)]
enum Goal {
    Gather(&'static str, u32),
    Craft(&'static str),
    /// Collect this many wolf pelts: wait at camp for night, then hunt.
    Hunt(u32),
    Light,
}

/// Plays the island through the same `Input` a person gives: steer, swing, craft, eat, dodge.
struct Bot {
    goals: Vec<Goal>,
    deaths: u32,
    kills: u32,
    dodges: u32,
}

impl Bot {
    fn new() -> Self {
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

    fn decide(&mut self, s: &Sim) -> Input {
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
                    if flat(p, beacon.pos) > 1.8 {
                        return Self::approach(s, beacon.pos, 1.6, false);
                    }
                    return Input { interact: Button::DOWN, ..Default::default() };
                }
            }
        }
    }

    fn observe(&mut self, s: &Sim) {
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

/// Returns (ticks to victory, the bot, a fingerprint of the final state).
fn play_island(max_minutes: usize) -> (Option<u64>, Bot, String) {
    let mut s = Sim::island();
    let mut bot = Bot::new();
    for _ in 0..max_minutes * 60 * 60 {
        let inp = bot.decide(&s);
        s.step(&inp);
        bot.observe(&s);
        if s.survival.state == GameState::Victory {
            let fp = format!("{:?} {:?} {:?}", s.player.pos, s.survival.inv, s.survival.skills);
            return (Some(s.tick), bot, fp);
        }
    }
    let fp = format!("{:?} goals left {:?}", s.player.pos, bot.goals);
    (None, bot, fp)
}

#[test]
fn scripted_playthrough_lights_the_beacon() {
    let (ticks, bot, fp) = play_island(30);
    let ticks = ticks.unwrap_or_else(|| panic!("no victory in 30 min: {fp}"));
    let minutes = ticks as f32 / 3600.0;
    println!("victory in {minutes:.1} min, deaths {}, wolves killed {}, dodges {}", bot.deaths, bot.kills, bot.dodges);
    assert!(minutes < 30.0);
}

#[test]
fn the_whole_playthrough_replays_identically() {
    let (a, _, fa) = play_island(30);
    let (b, _, fb) = play_island(30);
    assert_eq!(a, b);
    assert_eq!(fa, fb);
}
