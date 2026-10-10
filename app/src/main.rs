//! Presentation only. `sim` is the whole game; this file feeds it input at a fixed 60 Hz,
//! interpolates the player between ticks, and draws what the sim says: the island, the Blender
//! models, wolves, day/night lighting and the HUD.
//!
//! Dev hooks (env vars, never active in a normal launch):
//!   REWRITE_CAPTURE=<file.png>   screenshot after REWRITE_CAPTURE_AT seconds (default 3), then exit
//!   REWRITE_AUTOPLAY=1           the scripted bot (`sim::bot`) plays through the real input path
//!   REWRITE_SPEED=<n>            run n sim ticks per fixed tick (fast-forward autoplay)
//!   REWRITE_SETUP=<a,b,...>      start from a staged state: camp (stations built, items given),
//!                                night (dusk with wolves near), victory (Beacon lit), craft (Tab open)
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use bevy::app::{RunFixedMainLoop, RunFixedMainLoopSystem};
use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use bevy::window::{CursorGrabMode, PrimaryWindow};
use sim::content::{ItemKind, Skill};
use sim::{CraftError, Event as SimEvent, GameState, Input as SimInput, Sim, State, WolfState};
use std::collections::HashMap;

#[derive(Resource)]
struct Game(Sim);

/// Input accumulated since the last tick (edges OR-ed so short taps survive).
#[derive(Resource, Default)]
struct Pending(SimInput);

#[derive(Resource)]
struct Autoplay {
    bot: Option<sim::bot::Bot>,
    speed: u32,
}

/// Previous and current tick poses, and the interpolated one to draw.
#[derive(Resource, Default)]
struct Rendered {
    prev: Vec3,
    prev_yaw: f32,
    pos: Vec3,
    yaw: f32,
}

#[derive(Resource)]
struct CamRig {
    yaw: f32,
    pitch: f32,
    /// Current spring-arm length (snaps in on collision, eases back out).
    arm: f32,
    /// Smoothed follow point.
    pivot: Vec3,
    locked: bool,
    /// The click that captured the mouse must not also attack.
    swallow_click: bool,
}

#[derive(Resource, Default)]
struct Feed(Vec<String>);

#[derive(Resource, Default)]
struct CraftPanel {
    open: bool,
    selected: usize,
}

/// Spawned visuals that mirror sim lists.
#[derive(Resource, Default)]
struct Mirrors {
    structures: usize,
    wolves: HashMap<u32, Entity>,
    flame: bool,
}

#[derive(Resource)]
struct Palette {
    wolf: Handle<StandardMaterial>,
    wolf_crouch: Handle<StandardMaterial>,
    wolf_body: Handle<Mesh>,
    wolf_head: Handle<Mesh>,
}

#[derive(Component)]
struct PlayerBody;
#[derive(Component)]
struct NodeVis {
    node: usize,
    depleted: bool,
}
#[derive(Component)]
struct WolfVis(u32);
#[derive(Component)]
struct WolfBody;
#[derive(Component)]
struct Sun;
#[derive(Component)]
struct FireLight;
#[derive(Component)]
enum Bar {
    Health,
    Stamina,
    Hunger,
}
#[derive(Component)]
enum HudText {
    Clock,
    Objective,
    Feed,
    Bag,
    Craft,
    Overlay,
    Hint,
}
#[derive(Component)]
struct CraftBox;
#[derive(Component)]
struct OverlayBox;

fn env(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.is_empty())
}

fn main() {
    let assets = asset_dir();
    let mut sim = Sim::island();
    let setup = env("REWRITE_SETUP").unwrap_or_default();
    let craft_open = stage(&mut sim, &setup);
    let autoplay = Autoplay {
        bot: env("REWRITE_AUTOPLAY").map(|_| sim::bot::Bot::new()),
        speed: env("REWRITE_SPEED").and_then(|s| s.parse().ok()).unwrap_or(1).max(1),
    };
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window { title: "Emberwild - click to capture mouse".into(), ..default() }),
                    ..default()
                })
                .set(AssetPlugin { file_path: assets, ..default() }),
        )
        .insert_resource(Time::<Fixed>::from_hz(sim::TICK_HZ))
        .insert_resource(ClearColor(Color::srgb(0.5, 0.7, 0.9)))
        .insert_resource(CamRig { yaw: if setup.is_empty() { 0.0 } else { 0.54 }, pitch: -0.3, arm: sim::extracted::CAM_ARM, pivot: Vec3::ZERO, locked: false, swallow_click: false })
        .insert_resource(CraftPanel { open: craft_open, selected: 0 })
        .insert_resource(autoplay)
        .insert_resource(Game(sim))
        .init_resource::<Pending>()
        .init_resource::<Rendered>()
        .init_resource::<Feed>()
        .init_resource::<Mirrors>()
        .add_systems(Startup, setup_scene)
        .add_systems(
            RunFixedMainLoop,
            (grab_cursor, look, gather_input, craft_keys).chain().in_set(RunFixedMainLoopSystem::BeforeFixedMainLoop),
        )
        .add_systems(FixedUpdate, tick)
        .add_systems(
            Update,
            (interpolate, draw_player, draw_nodes, draw_structures, draw_wolves, day_night, follow_camera, hud, capture).chain(),
        )
        .run();
}

/// The models folder: next to the exe (packaged build), in the working directory, or at the repo
/// root above `app/` (cargo run). An absolute path, so Bevy's own base folder doesn't matter.
fn asset_dir() -> String {
    let exe_dir = std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.to_path_buf()));
    let manifest = std::env::var_os("CARGO_MANIFEST_DIR").map(|m| std::path::PathBuf::from(m).join(".."));
    let candidates = [exe_dir, std::env::current_dir().ok(), manifest];
    candidates
        .into_iter()
        .flatten()
        .map(|d| d.join("assets"))
        .find(|d| d.join("models").is_dir())
        .map(|d| d.to_string_lossy().into_owned())
        .unwrap_or_else(|| "assets".into())
}

/// REWRITE_SETUP: jump to a state worth looking at. Returns whether the craft panel starts open.
fn stage(sim: &mut Sim, setup: &str) -> bool {
    let has = |k: &str| setup.split(',').any(|s| s.trim() == k);
    let give = |sim: &mut Sim, id: &str, n: u32| {
        let i = sim.content.item(id).unwrap();
        sim.survival.inv.add(i, n);
    };
    if has("camp") || has("victory") {
        sim.survival.skills.xp = [140.0, 90.0, 200.0, 60.0];
        for (id, n) in [("log", 40), ("stone", 40), ("iron_bar", 8), ("wolf_pelt", 2), ("torch", 2), ("cooked_meat", 3), ("berries", 5), ("stone_axe", 1), ("stone_pickaxe", 1), ("wooden_club", 1)] {
            give(sim, id, n);
        }
        for (yaw, out) in [(2.4f32, "campfire"), (1.2, "workbench"), (-1.2, "furnace"), (-2.6, "beacon")] {
            sim.player.yaw = yaw;
            let r = sim.content.recipe(out).unwrap();
            sim.step(&SimInput { craft: Some(r), ..default() });
        }
        // Stand back so the camp is in frame.
        sim.player.pos = Vec3::new(6.0, 0.0, 10.0);
        sim.player.yaw = 0.54;
        // A few gathered nodes, to show stumps.
        for n in sim.survival.nodes.iter_mut().filter(|n| n.pos.x < -10.0 && n.pos.x > -15.0) {
            n.charges = 0;
            n.respawn = 1000.0;
        }
    }
    if has("night") {
        let t = &sim.content.tunables;
        sim.survival.clock.time = t.night_start - 0.002;
        sim.survival.wolf_spawns = vec![Vec3::new(-6.0, 0.0, -8.0), Vec3::new(7.0, 0.0, -9.0)];
    }
    if has("victory") {
        let beacon = sim.survival.structures.iter().find(|s| s.item == sim.content.beacon).map(|s| s.pos);
        if let Some(b) = beacon {
            sim.player.pos = b + Vec3::new(0.0, 0.0, 1.5);
            sim.step(&SimInput { interact: sim::Button::DOWN, ..default() });
        }
    }
    has("craft")
}

fn model(assets: &AssetServer, name: &str) -> SceneRoot {
    SceneRoot(assets.load(GltfAssetLabel::Scene(0).from_asset(format!("models/{name}.glb"))))
}

fn setup_scene(
    mut commands: Commands,
    game: Res<Game>,
    assets: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let sim = &game.0;
    commands.spawn((
        DirectionalLight { illuminance: 9000.0, shadows_enabled: true, ..default() },
        Transform::from_xyz(8.0, 20.0, 6.0).looking_at(Vec3::ZERO, Vec3::Y),
        Sun,
    ));
    commands.insert_resource(AmbientLight { brightness: 400.0, ..default() });

    // Land (the floor collision box) and the sea around it. Shore walls stay invisible.
    let half = sim.content.layout.half_size;
    let grass = materials.add(StandardMaterial { base_color: Color::srgb(0.36, 0.55, 0.27), perceptual_roughness: 0.95, ..default() });
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(half * 2.0, 1.0, half * 2.0))),
        MeshMaterial3d(grass),
        Transform::from_xyz(0.0, -0.5, 0.0),
    ));
    let sand = materials.add(StandardMaterial { base_color: Color::srgb(0.82, 0.76, 0.55), ..default() });
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(half * 2.0 + 6.0, 1.0, half * 2.0 + 6.0))),
        MeshMaterial3d(sand),
        Transform::from_xyz(0.0, -0.62, 0.0),
    ));
    let sea = materials.add(StandardMaterial { base_color: Color::srgba(0.12, 0.35, 0.55, 0.92), alpha_mode: AlphaMode::Blend, perceptual_roughness: 0.2, ..default() });
    commands.spawn((Mesh3d(meshes.add(Plane3d::default().mesh().size(600.0, 600.0))), MeshMaterial3d(sea), Transform::from_xyz(0.0, -0.35, 0.0)));
    // Camp marker: a ring of stones would be art; a flat disc reads fine for now.
    let dirt = materials.add(StandardMaterial { base_color: Color::srgb(0.45, 0.36, 0.25), ..default() });
    commands.spawn((Mesh3d(meshes.add(Cylinder::new(3.5, 0.02))), MeshMaterial3d(dirt), Transform::from_xyz(sim.survival.camp.x, 0.01, sim.survival.camp.z)));

    // Resource nodes: the live model and, where there is one, the depleted model (stump).
    for (i, n) in sim.survival.nodes.iter().enumerate() {
        let def = &sim.content.nodes[n.kind];
        let yaw = (i as f32 * 2.399).rem_euclid(std::f32::consts::TAU); // golden-angle variety
        let tf = Transform::from_translation(n.pos).with_rotation(Quat::from_rotation_y(yaw));
        commands.spawn((model(&assets, &def.model), tf, NodeVis { node: i, depleted: false }));
        if let Some(stump) = &def.depleted_model {
            commands.spawn((model(&assets, stump), tf, Visibility::Hidden, NodeVis { node: i, depleted: true }));
        }
    }

    // Player: grey-box until the character model exists (needs a local Blender session).
    let half_ext = sim.tuning.half_extents;
    commands
        .spawn((
            Mesh3d(meshes.add(Capsule3d::new(half_ext.x, half_ext.y * 2.0 - half_ext.x * 2.0))),
            MeshMaterial3d(materials.add(StandardMaterial { base_color: Color::srgb(0.85, 0.72, 0.45), ..default() })),
            Transform::default(),
            PlayerBody,
        ))
        .with_children(|p| {
            p.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.14, 0.14, 0.35))),
                MeshMaterial3d(materials.add(StandardMaterial { base_color: Color::srgb(0.25, 0.2, 0.15), ..default() })),
                Transform::from_xyz(0.0, 0.5, -0.33),
            ));
        });

    commands.insert_resource(Palette {
        wolf: materials.add(StandardMaterial { base_color: Color::srgb(0.32, 0.31, 0.33), ..default() }),
        wolf_crouch: materials.add(StandardMaterial { base_color: Color::srgb(0.85, 0.15, 0.1), emissive: LinearRgba::rgb(0.8, 0.05, 0.0), ..default() }),
        wolf_body: meshes.add(Cuboid::new(0.45, 0.45, 1.1)),
        wolf_head: meshes.add(Cuboid::new(0.32, 0.3, 0.4)),
    });

    commands.spawn((Camera3d::default(), Transform::from_xyz(0.0, 3.0, 6.0), DistanceFog {
        color: Color::srgba(0.6, 0.72, 0.85, 1.0),
        falloff: FogFalloff::Linear { start: 60.0, end: 160.0 },
        ..default()
    }));
    spawn_hud(&mut commands);
}

fn panel() -> (Node, BackgroundColor) {
    (Node { padding: UiRect::all(Val::Px(8.0)), flex_direction: FlexDirection::Column, row_gap: Val::Px(4.0), ..default() }, BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)))
}

fn text(kind: HudText, size: f32) -> (Text, TextFont, TextColor, HudText) {
    (Text::new(""), TextFont { font_size: size, ..default() }, TextColor(Color::WHITE), kind)
}

fn spawn_hud(commands: &mut Commands) {
    // Vitals, top left.
    commands
        .spawn((Node { position_type: PositionType::Absolute, left: Val::Px(12.0), top: Val::Px(12.0), ..default() },))
        .with_children(|root| {
            root.spawn(panel()).with_children(|p| {
                for (bar, label, color) in [
                    (Bar::Health, "Health", Color::srgb(0.85, 0.2, 0.2)),
                    (Bar::Stamina, "Stamina", Color::srgb(0.3, 0.8, 0.3)),
                    (Bar::Hunger, "Hunger", Color::srgb(0.9, 0.6, 0.2)),
                ] {
                    p.spawn(Node { column_gap: Val::Px(6.0), align_items: AlignItems::Center, ..default() }).with_children(|row| {
                        row.spawn((Text::new(label), TextFont { font_size: 14.0, ..default() }, Node { width: Val::Px(62.0), ..default() }));
                        row.spawn((Node { width: Val::Px(180.0), height: Val::Px(12.0), ..default() }, BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.15))))
                            .with_children(|b| {
                                b.spawn((Node { width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() }, BackgroundColor(color), bar));
                            });
                    });
                }
            });
        });
    // Clock, top centre.
    commands
        .spawn(Node { position_type: PositionType::Absolute, top: Val::Px(12.0), width: Val::Percent(100.0), justify_content: JustifyContent::Center, ..default() })
        .with_children(|r| {
            r.spawn(panel()).with_children(|p| {
                p.spawn(text(HudText::Clock, 18.0));
            });
        });
    // Objective, top right; bag and skills below it.
    commands
        .spawn(Node { position_type: PositionType::Absolute, right: Val::Px(12.0), top: Val::Px(12.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(8.0), align_items: AlignItems::End, ..default() })
        .with_children(|r| {
            r.spawn(panel()).with_children(|p| {
                p.spawn(text(HudText::Objective, 16.0));
            });
            r.spawn(panel()).with_children(|p| {
                p.spawn(text(HudText::Bag, 14.0));
            });
        });
    // Feed, bottom left; key hints, bottom centre.
    commands
        .spawn(Node { position_type: PositionType::Absolute, left: Val::Px(12.0), bottom: Val::Px(12.0), ..default() })
        .with_children(|r| {
            r.spawn(panel()).with_children(|p| {
                p.spawn(text(HudText::Feed, 14.0));
            });
        });
    commands
        .spawn(Node { position_type: PositionType::Absolute, bottom: Val::Px(12.0), width: Val::Percent(100.0), justify_content: JustifyContent::Center, ..default() })
        .with_children(|r| {
            r.spawn(text(HudText::Hint, 14.0));
        });
    // Crafting panel (Tab), centre.
    commands
        .spawn((
            Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() },
            Visibility::Hidden,
            CraftBox,
        ))
        .with_children(|r| {
            r.spawn((Node { padding: UiRect::all(Val::Px(14.0)), ..default() }, BackgroundColor(Color::srgba(0.05, 0.04, 0.03, 0.88)))).with_children(|p| {
                p.spawn(text(HudText::Craft, 15.0));
            });
        });
    // Death / victory overlay.
    commands
        .spawn((
            Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
            Visibility::Hidden,
            OverlayBox,
        ))
        .with_children(|r| {
            r.spawn((Text::new(""), TextFont { font_size: 34.0, ..default() }, TextColor(Color::srgb(1.0, 0.85, 0.5)), TextLayout::new_with_justify(JustifyText::Center), HudText::Overlay));
        });
}

fn grab_cursor(
    mut window: Single<&mut Window, With<PrimaryWindow>>,
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut cam: ResMut<CamRig>,
) {
    let lock = if keys.just_pressed(KeyCode::Escape) {
        Some(false)
    } else if mouse.just_pressed(MouseButton::Left) && !cam.locked {
        Some(true)
    } else {
        None
    };
    if let Some(lock) = lock {
        cam.locked = lock;
        cam.swallow_click = lock;
        window.cursor_options.grab_mode = if lock { CursorGrabMode::Locked } else { CursorGrabMode::None };
        window.cursor_options.visible = !lock;
    }
}

fn look(motion: Res<AccumulatedMouseMotion>, mut cam: ResMut<CamRig>) {
    if cam.locked {
        cam.yaw -= motion.delta.x * 0.003;
        cam.pitch = (cam.pitch - motion.delta.y * 0.003).clamp(-1.2, 1.0);
    }
}

/// Fold this frame's devices into the pending tick input.
fn gather_input(keys: Res<ButtonInput<KeyCode>>, mouse: Res<ButtonInput<MouseButton>>, panel: Res<CraftPanel>, mut cam: ResMut<CamRig>, mut pending: ResMut<Pending>) {
    let i = &mut pending.0;
    let mut mv = Vec2::ZERO;
    let walking_keys = cam.locked && !panel.open;
    if walking_keys {
        if keys.pressed(KeyCode::KeyW) { mv.y += 1.0; }
        if keys.pressed(KeyCode::KeyS) { mv.y -= 1.0; }
        if keys.pressed(KeyCode::KeyD) { mv.x += 1.0; }
        if keys.pressed(KeyCode::KeyA) { mv.x -= 1.0; }
    }
    let walk = keys.pressed(KeyCode::AltLeft);
    i.mv = mv.normalize_or_zero() * if walk { 0.4 } else { 1.0 };
    i.look_yaw = cam.yaw;
    let key = |k: KeyCode| (keys.pressed(k), keys.just_pressed(k), keys.just_released(k));
    let (d, p, r) = key(KeyCode::Space);
    i.jump.accumulate(d, p, r);
    let (d, p, r) = key(KeyCode::ShiftLeft);
    i.sprint.accumulate(d, p, r);
    let (d, p, r) = key(KeyCode::ControlLeft);
    i.dodge.accumulate(d, p, r);
    let (d, p, r) = key(KeyCode::KeyE);
    i.interact.accumulate(d, p, r);
    let (d, p, r) = key(KeyCode::KeyF);
    i.eat.accumulate(d, p, r);
    if cam.swallow_click && !mouse.pressed(MouseButton::Left) {
        cam.swallow_click = false;
    }
    let attack_ok = cam.locked && !cam.swallow_click && !panel.open;
    i.attack.accumulate(
        mouse.pressed(MouseButton::Left) && attack_ok,
        mouse.just_pressed(MouseButton::Left) && attack_ok,
        mouse.just_released(MouseButton::Left),
    );
}

/// Tab opens the crafting panel; W/S or arrows pick, Enter (or click) crafts.
fn craft_keys(keys: Res<ButtonInput<KeyCode>>, mouse: Res<ButtonInput<MouseButton>>, game: Res<Game>, mut panel: ResMut<CraftPanel>, mut pending: ResMut<Pending>) {
    if keys.just_pressed(KeyCode::Tab) {
        panel.open = !panel.open;
    }
    if !panel.open {
        return;
    }
    let n = game.0.content.recipes.len();
    if keys.just_pressed(KeyCode::ArrowDown) || keys.just_pressed(KeyCode::KeyS) {
        panel.selected = (panel.selected + 1) % n;
    }
    if keys.just_pressed(KeyCode::ArrowUp) || keys.just_pressed(KeyCode::KeyW) {
        panel.selected = (panel.selected + n - 1) % n;
    }
    if keys.just_pressed(KeyCode::Enter) || mouse.just_pressed(MouseButton::Left) {
        pending.0.craft = Some(panel.selected);
    }
}

fn tick(
    mut game: ResMut<Game>,
    mut pending: ResMut<Pending>,
    mut auto: ResMut<Autoplay>,
    mut rendered: ResMut<Rendered>,
    mut feed: ResMut<Feed>,
) {
    rendered.prev = game.0.player.pos;
    rendered.prev_yaw = game.0.player.yaw;
    for _ in 0..auto.speed {
        let inp = match auto.bot.as_mut() {
            Some(bot) => bot.decide(&game.0),
            None => pending.0,
        };
        game.0.step(&inp);
        if let Some(bot) = auto.bot.as_mut() {
            bot.observe(&game.0);
        }
        pending.0.clear_edges();
        let sim = &game.0;
        for e in &sim.events {
            if let Some(line) = describe(sim, e) {
                feed.0.push(line);
            }
        }
    }
    let excess = feed.0.len().saturating_sub(7);
    feed.0.drain(..excess);
}

/// Feed line for an event (None for the noisy ones).
fn describe(sim: &Sim, e: &SimEvent) -> Option<String> {
    let c = &sim.content;
    let name = |i: usize| c.items[i].name.as_str();
    Some(match *e {
        SimEvent::ItemGathered { item, count } => format!("+{count} {}", name(item)),
        SimEvent::GatherBlocked { node } => {
            let d = &c.nodes[sim.survival.nodes[node].kind];
            match d.tool {
                Some(t) if d.min_tier > 0 => format!("Needs a {:?} (tier {}) and {} {}", t, d.min_tier, d.skill.map_or("", |s| s.name()), d.level),
                _ => "Too tough for you yet".into(),
            }
        }
        SimEvent::ItemCrafted { item, count } => format!("Crafted {} x{count}", name(item)),
        SimEvent::Built { item, .. } => format!("Built {}", name(item)),
        SimEvent::CraftFailed { reason, recipe } => format!("Can't craft {}: {}", name(c.recipes[recipe].out), reason_text(reason)),
        SimEvent::LevelUp { skill, level } => format!("{} level {level}!", skill.name()),
        SimEvent::Ate { item } => format!("Ate {}", name(item)),
        SimEvent::Starving => "You are starving!".into(),
        SimEvent::PlayerHurt { damage, .. } => format!("Bitten for {damage:.0}"),
        SimEvent::PlayerDied => "You died".into(),
        SimEvent::PlayerRespawned => "You wake at camp".into(),
        SimEvent::BiteDodged { .. } => "Dodged!".into(),
        SimEvent::WolfHit { damage, hp_left, .. } => format!("Hit wolf {damage:.0} ({hp_left:.0} left)"),
        SimEvent::EnemyKilled { kind } => format!("Killed a {}", c.enemies[kind].id),
        SimEvent::NightFell { night } => format!("Night {night} falls. Wolves are hunting."),
        SimEvent::DayBroke { day } => format!("Day {day} dawns"),
        SimEvent::NeedFuel => "The Beacon needs a Torch".into(),
        SimEvent::BeaconLit => "The Beacon is lit!".into(),
        _ => return None,
    })
}

fn reason_text(r: CraftError) -> &'static str {
    match r {
        CraftError::Level => "Crafting level too low",
        CraftError::Station => "station not nearby",
        CraftError::Items => "missing items",
    }
}

fn interpolate(game: Res<Game>, time: Res<Time<Fixed>>, mut r: ResMut<Rendered>) {
    let p = &game.0.player;
    // A teleport (respawn) snaps instead of gliding.
    let alpha = if r.prev.distance(p.pos) > 3.0 { 1.0 } else { time.overstep_fraction() };
    r.pos = r.prev.lerp(p.pos, alpha);
    r.yaw = r.prev_yaw + sim::angle_diff(r.prev_yaw, p.yaw) * alpha;
}

fn draw_player(game: Res<Game>, r: Res<Rendered>, mut q: Query<(&mut Transform, &mut Visibility), With<PlayerBody>>) {
    let half = game.0.tuning.half_extents;
    let Ok((mut tf, mut vis)) = q.single_mut() else { return };
    tf.translation = r.pos + Vec3::Y * half.y;
    tf.rotation = Quat::from_rotation_y(r.yaw);
    *vis = if matches!(game.0.survival.state, GameState::Dead(_)) { Visibility::Hidden } else { Visibility::Inherited };
}

fn draw_nodes(game: Res<Game>, mut q: Query<(&NodeVis, &mut Visibility)>) {
    for (n, mut vis) in &mut q {
        let alive = game.0.survival.nodes[n.node].alive();
        let show = alive != n.depleted;
        vis.set_if_neq(if show { Visibility::Inherited } else { Visibility::Hidden });
    }
}

/// Built stations and the Beacon appear when the sim says they were built.
fn draw_structures(mut commands: Commands, game: Res<Game>, assets: Res<AssetServer>, mut mirrors: ResMut<Mirrors>) {
    let sim = &game.0;
    let all = &sim.survival.structures;
    for s in &all[mirrors.structures..] {
        let ItemKind::Structure { model: name, .. } = &sim.content.items[s.item].kind else { continue };
        let tf = Transform::from_translation(s.pos).with_rotation(Quat::from_rotation_y(s.yaw));
        let mut e = commands.spawn((model(&assets, name), tf));
        if sim.content.items[s.item].id == "campfire" {
            e.with_children(|c| {
                c.spawn((PointLight { color: Color::srgb(1.0, 0.6, 0.3), intensity: 0.0, range: 14.0, shadows_enabled: false, ..default() }, Transform::from_xyz(0.0, 0.8, 0.0), FireLight));
            });
        }
    }
    mirrors.structures = all.len();
    if !mirrors.flame {
        if let Some(b) = all.iter().find(|s| s.lit) {
            mirrors.flame = true;
            commands
                // The flame sits in the bowl on top of the pillar (5.4 m, from art/build_environment.py).
                .spawn((model(&assets, "beacon_flame"), Transform::from_translation(b.pos + Vec3::Y * 5.35)))
                .with_children(|c| {
                    c.spawn((PointLight { color: Color::srgb(1.0, 0.7, 0.35), intensity: 4_000_000.0, range: 60.0, shadows_enabled: false, ..default() }, Transform::from_xyz(0.0, 1.0, 0.0)));
                });
        }
    }
}

/// Wolves: grey-box bodies that turn red while crouched to bite (the telegraph).
fn draw_wolves(
    mut commands: Commands,
    game: Res<Game>,
    pal: Res<Palette>,
    mut mirrors: ResMut<Mirrors>,
    mut q: Query<(&WolfVis, &mut Transform)>,
    mut bodies: Query<(&ChildOf, &mut MeshMaterial3d<StandardMaterial>), With<WolfBody>>,
) {
    let wolves = &game.0.survival.wolves;
    mirrors.wolves.retain(|id, e| {
        let keep = wolves.iter().any(|w| w.id == *id);
        if !keep {
            commands.entity(*e).despawn();
        }
        keep
    });
    for w in wolves {
        if !mirrors.wolves.contains_key(&w.id) {
            let e = commands
                .spawn((Transform::from_translation(w.pos), Visibility::default(), WolfVis(w.id)))
                .with_children(|c| {
                    c.spawn((Mesh3d(pal.wolf_body.clone()), MeshMaterial3d(pal.wolf.clone()), Transform::from_xyz(0.0, 0.5, 0.0), WolfBody));
                    c.spawn((Mesh3d(pal.wolf_head.clone()), MeshMaterial3d(pal.wolf.clone()), Transform::from_xyz(0.0, 0.7, -0.65)));
                })
                .id();
            mirrors.wolves.insert(w.id, e);
        }
    }
    for (WolfVis(id), mut tf) in &mut q {
        if let Some(w) = wolves.iter().find(|w| w.id == *id) {
            let crouch = if w.state == WolfState::Windup { 0.75 } else { 1.0 };
            tf.translation = w.pos;
            tf.rotation = Quat::from_rotation_y(w.yaw);
            tf.scale = Vec3::new(1.0, crouch, 1.0);
        }
    }
    for (parent, mut mat) in &mut bodies {
        let Some(w) = mirrors.wolves.iter().find(|(_, e)| **e == parent.parent()).and_then(|(id, _)| wolves.iter().find(|w| w.id == *id)) else { continue };
        let want = if w.state == WolfState::Windup { &pal.wolf_crouch } else { &pal.wolf };
        if mat.0 != *want {
            mat.0 = want.clone();
        }
    }
}

/// Sun arc, light colour and sky follow the sim clock.
fn day_night(
    game: Res<Game>,
    mut sun: Query<(&mut Transform, &mut DirectionalLight), With<Sun>>,
    mut fires: Query<&mut PointLight, With<FireLight>>,
    mut ambient: ResMut<AmbientLight>,
    mut clear: ResMut<ClearColor>,
    mut fog: Query<&mut DistanceFog>,
) {
    let clock = &game.0.survival.clock;
    let day = clock.daylight();
    let angle = (clock.time - 0.25) * std::f32::consts::TAU; // 0 at dawn, PI/2 at noon
    if let Ok((mut tf, mut light)) = sun.single_mut() {
        let dir = Vec3::new(angle.cos(), angle.sin().max(0.15), 0.35).normalize();
        *tf = Transform::from_translation(dir * 50.0).looking_at(Vec3::ZERO, Vec3::Y);
        light.illuminance = 400.0 + 10_000.0 * day;
        let warm = 1.0 - (angle.sin().abs()).min(1.0);
        light.color = Color::srgb(1.0, 1.0 - 0.35 * warm, 1.0 - 0.6 * warm).mix(&Color::srgb(0.55, 0.6, 0.9), 1.0 - day);
    }
    ambient.brightness = 260.0 + 360.0 * day;
    ambient.color = Color::srgb(0.6, 0.65, 1.0).mix(&Color::WHITE, day);
    let sky = Color::srgb(0.03, 0.04, 0.09).mix(&Color::srgb(0.52, 0.72, 0.92), day);
    clear.0 = sky;
    if let Ok(mut f) = fog.single_mut() {
        f.color = sky;
    }
    for mut l in &mut fires {
        l.intensity = 30_000.0 + 400_000.0 * (1.0 - day);
    }
}

/// Over-the-shoulder spring arm: smoothed pivot, collision pulls the camera in instantly and it
/// eases back out (the usual UE SpringArm behaviour). Numbers come from the data table.
fn follow_camera(
    time: Res<Time>,
    game: Res<Game>,
    r: Res<Rendered>,
    mut cam: ResMut<CamRig>,
    mut q: Query<(&mut Transform, &mut Projection), With<Camera3d>>,
) {
    use sim::extracted::{CAM_ARM, CAM_FOV, CAM_HEIGHT, CAM_LAG, CAM_SHOULDER};
    let Ok((mut tf, mut proj)) = q.single_mut() else { return };
    let dt = time.delta_secs();
    let k = 1.0 - (-CAM_LAG * dt).exp();
    let target = r.pos + Vec3::Y * CAM_HEIGHT;
    cam.pivot = if cam.pivot == Vec3::ZERO { target } else { cam.pivot + (target - cam.pivot) * k };
    // Victory: a slow orbit that frames the lit Beacon.
    if game.0.survival.state == GameState::Victory {
        cam.yaw += 0.15 * dt;
        cam.pitch = -0.05;
    }
    let rot = Quat::from_euler(EulerRot::YXZ, cam.yaw, cam.pitch, 0.0);
    let pivot = cam.pivot + rot * Vec3::X * CAM_SHOULDER;
    let back = rot * Vec3::Z;
    // A survival game wants a little more view than the movement study's arm.
    let won = game.0.survival.state == GameState::Victory;
    let arm = CAM_ARM * if won { 4.0 } else { 1.6 };
    let want = sim::world::camera_arm(&game.0.world, pivot, back, arm, 0.2, 0.3);
    cam.arm = if want < cam.arm { want } else { cam.arm + (want - cam.arm) * k };
    tf.translation = pivot + back * cam.arm;
    tf.rotation = rot;
    if let Projection::Perspective(p) = proj.as_mut() {
        p.fov = CAM_FOV.to_radians();
    }
}

/// What to do next, read from the sim state.
fn objective(sim: &Sim) -> String {
    let c = &sim.content;
    let sv = &sim.survival;
    let built = |id: &str| sv.structures.iter().any(|s| c.items[s.item].id == id);
    let have = |id: &str| sv.inv.count(c.item(id).unwrap());
    let goal = if sv.state == GameState::Victory {
        "The Beacon burns. You made it.".to_string()
    } else if !built("campfire") {
        "Gather logs and stones (left click),\nthen build a Campfire (Tab)".into()
    } else if !built("workbench") {
        "Build a Workbench (Tab)".into()
    } else if have("stone_pickaxe") == 0 {
        "Craft a Stone Pickaxe at the Workbench".into()
    } else if !built("furnace") {
        "Build a Furnace (Crafting 2)".into()
    } else if !built("beacon") && have("iron_bar") < 6 {
        format!("Mine iron in the south (Mining 2),\nsmelt Iron Bars: {}/6", have("iron_bar"))
    } else if !built("beacon") && have("wolf_pelt") < 2 {
        format!("Survive the night: wolf pelts {}/2", have("wolf_pelt"))
    } else if !built("beacon") {
        "Build the Beacon (Crafting 4)".into()
    } else if have("torch") == 0 {
        "Craft a Torch at the Campfire".into()
    } else {
        "Light the Beacon (E)".into()
    };
    format!("OBJECTIVE\n{goal}")
}

#[allow(clippy::too_many_arguments)]
fn hud(
    game: Res<Game>,
    feed: Res<Feed>,
    panel: Res<CraftPanel>,
    mut bars: Query<(&Bar, &mut Node)>,
    mut texts: Query<(&HudText, &mut Text)>,
    mut craft_box: Query<&mut Visibility, (With<CraftBox>, Without<OverlayBox>)>,
    mut overlay: Query<&mut Visibility, (With<OverlayBox>, Without<CraftBox>)>,
) {
    let sim = &game.0;
    let c = &sim.content;
    let sv = &sim.survival;
    let t = &c.tunables;
    for (bar, mut node) in &mut bars {
        let frac = match bar {
            Bar::Health => sv.vitals.health / t.health_max,
            Bar::Stamina => sim.player.stamina / sim.tuning.stamina_max,
            Bar::Hunger => sv.vitals.hunger / t.hunger_max,
        };
        node.width = Val::Percent(frac.clamp(0.0, 1.0) * 100.0);
    }
    if let Ok(mut v) = craft_box.single_mut() {
        v.set_if_neq(if panel.open { Visibility::Inherited } else { Visibility::Hidden });
    }
    let overlay_text = match sv.state {
        GameState::Dead(t) => Some(format!("You died\nwaking at camp in {:.0}", t.ceil())),
        GameState::Victory => Some(format!(
            "THE BEACON IS LIT\n\nYou survived {} night{} and won on day {}.\nTime {:.1} min",
            sv.clock.night,
            if sv.clock.night == 1 { "" } else { "s" },
            sv.clock.day,
            sim.tick as f32 / 3600.0
        )),
        GameState::Playing => None,
    };
    if let Ok(mut v) = overlay.single_mut() {
        v.set_if_neq(if overlay_text.is_some() { Visibility::Inherited } else { Visibility::Hidden });
    }

    for (kind, mut text) in &mut texts {
        let s = match kind {
            HudText::Clock => {
                let hours = (sv.clock.time * 24.0) as u32;
                let mins = ((sv.clock.time * 24.0).fract() * 60.0) as u32;
                let phase = if sv.clock.is_night { format!("Night {}", sv.clock.night.max(1)) } else { "Day".into() };
                format!("Day {}  {:02}:{:02}  {}", sv.clock.day, hours, mins, phase)
            }
            HudText::Objective => objective(sim),
            HudText::Feed => feed.0.join("\n"),
            HudText::Bag => {
                let mut out = String::from("BAG\n");
                let mut any = false;
                for (i, n) in sv.inv.stacks() {
                    any = true;
                    out += &format!("{:<14}{n:>3}\n", c.items[i].name);
                }
                if !any {
                    out += "(empty)\n";
                }
                out += "\nSKILLS\n";
                for s in Skill::ALL {
                    let xp = sv.skills.xp[s.index()];
                    let lv = c.level_for_xp(xp);
                    let next = c.xp_for_level(lv + 1);
                    out += &format!("{:<12}{lv:>2}  {:.0}/{:.0}\n", s.name(), xp, next);
                }
                out.trim_end().to_string()
            }
            HudText::Craft => craft_text(sim, panel.selected),
            HudText::Overlay => overlay_text.clone().unwrap_or_default(),
            HudText::Hint => {
                let mut h = String::from("LMB swing/gather   Tab craft   F eat   E light Beacon   Ctrl dodge   Shift sprint   Space jump");
                if sim.player.invincible() {
                    h = format!("[i-frames]   {h}");
                }
                if let State::Act(_) = sim.player.state {
                } else if sv.wolves.iter().any(|w| w.state == WolfState::Windup && w.pos.distance(sim.player.pos) < 3.0) {
                    h = format!("WOLF BITING - DODGE!   {h}");
                }
                h
            }
        };
        if text.0 != s {
            text.0 = s;
        }
    }
}

fn craft_text(sim: &Sim, selected: usize) -> String {
    let c = &sim.content;
    let mut out = String::from("CRAFTING   (W/S choose, Enter or click to craft, Tab close)\n\n");
    for (i, r) in c.recipes.iter().enumerate() {
        let mark = if i == selected { ">" } else { " " };
        let status = match sim.craft_blocker(i) {
            None => "ready".to_string(),
            Some(CraftError::Level) => format!("needs Crafting {}", r.level),
            Some(CraftError::Station) => format!("needs {}", r.station.as_deref().map(|s| c.items[c.item(s).unwrap()].name.as_str()).unwrap_or("?")),
            Some(CraftError::Items) => "missing items".into(),
        };
        let ins: Vec<String> = r.ins.iter().map(|&(it, n)| format!("{n} {}", c.items[it].name)).collect();
        out += &format!("{mark} {:<14} {:<26} [{}]\n", c.items[r.out].name, ins.join(", "), status);
    }
    out.trim_end().to_string()
}

fn capture(time: Res<Time>, mut commands: Commands, mut exit: EventWriter<AppExit>, mut taken: Local<bool>) {
    let Some(path) = std::env::var_os("REWRITE_CAPTURE") else { return };
    let at: f32 = env("REWRITE_CAPTURE_AT").and_then(|s| s.parse().ok()).unwrap_or(3.0);
    let t = time.elapsed_secs();
    if !*taken && t > at {
        *taken = true;
        commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path.to_string_lossy().into_owned()));
    }
    if t > at + 1.5 {
        exit.write(AppExit::Success);
    }
}
