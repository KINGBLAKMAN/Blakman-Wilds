//! Presentation only. `sim` is the whole game; this file feeds it input at a fixed 60 Hz,
//! interpolates between ticks for drawing, and draws what it says.
//!
//! Dev hooks (env vars, never active in a normal launch):
//!   REWRITE_CAPTURE=<file.png>  screenshot after 3 s, then exit
//!   REWRITE_AUTOPLAY=1          drive the real input path: run forward, attack, roll, jump
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use bevy::app::{RunFixedMainLoop, RunFixedMainLoopSystem};
use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use bevy::window::{CursorGrabMode, PrimaryWindow};
use sim::{Event as SimEvent, Input as SimInput, Sim, State};

#[derive(Resource)]
struct Game(Sim);


/// Input accumulated since the last tick (edges OR-ed so short taps survive).
#[derive(Resource, Default)]
struct Pending(SimInput);

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
struct Log(Vec<String>);

#[derive(Component)]
struct PlayerBody;
#[derive(Component)]
struct TargetBody(usize);
#[derive(Component)]
struct Hud;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window { title: "Rewrite sandbox - click to capture mouse".into(), ..default() }),
            ..default()
        }))
        .insert_resource(Time::<Fixed>::from_hz(sim::TICK_HZ))
        .insert_resource(ClearColor(Color::srgb(0.08, 0.09, 0.11)))
        .insert_resource(Game(Sim::new(sim::BoxWorld::arena(), Vec3::ZERO)))
        .insert_resource(CamRig { yaw: 0.0, pitch: -0.25, arm: sim::extracted::CAM_ARM, pivot: Vec3::ZERO, locked: false, swallow_click: false })
        .init_resource::<Pending>()
        .init_resource::<Rendered>()
        .init_resource::<Log>()
        .add_systems(Startup, setup)
        .add_systems(
            RunFixedMainLoop,
            (grab_cursor, look, gather, autoplay).chain().in_set(RunFixedMainLoopSystem::BeforeFixedMainLoop),
        )
        .add_systems(FixedUpdate, tick)
        .add_systems(Update, (interpolate, draw, follow_camera, hud, capture).chain())
        .run();
}

fn setup(
    mut commands: Commands,
    game: Res<Game>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        DirectionalLight { illuminance: 9000.0, shadows_enabled: true, ..default() },
        Transform::from_xyz(8.0, 20.0, 6.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.insert_resource(AmbientLight { brightness: 400.0, ..default() });

    // Level geometry straight from the sim's collision boxes: what you see is what collides.
    let grey = materials.add(StandardMaterial { base_color: Color::srgb(0.45, 0.46, 0.48), ..default() });
    for b in &game.0.world.boxes {
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::from_size(b.max - b.min))),
            MeshMaterial3d(grey.clone()),
            Transform::from_translation(b.center()),
        ));
    }

    let half = game.0.tuning.half_extents;
    commands
        .spawn((
            Mesh3d(meshes.add(Capsule3d::new(half.x, half.y * 2.0 - half.x * 2.0))),
            MeshMaterial3d(materials.add(StandardMaterial { base_color: Color::srgb(0.85, 0.75, 0.4), ..default() })),
            Transform::default(),
            PlayerBody,
        ))
        .with_children(|p| {
            // Nose: shows facing.
            p.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.12, 0.12, 0.35))),
                MeshMaterial3d(materials.add(StandardMaterial { base_color: Color::srgb(0.2, 0.2, 0.2), ..default() })),
                Transform::from_xyz(0.0, 0.5, -0.35),
            ));
        });

    for (i, t) in game.0.targets.iter().enumerate() {
        commands.spawn((
            Mesh3d(meshes.add(Capsule3d::new(t.radius, t.height - t.radius * 2.0))),
            MeshMaterial3d(materials.add(StandardMaterial { base_color: Color::srgb(0.7, 0.3, 0.3), ..default() })),
            Transform::from_translation(t.pos + Vec3::Y * t.height * 0.5),
            TargetBody(i),
        ));
    }

    commands.spawn((Camera3d::default(), Transform::from_xyz(0.0, 3.0, 6.0)));
    commands
        .spawn((
            Node { position_type: PositionType::Absolute, left: Val::Px(12.0), top: Val::Px(12.0), padding: UiRect::all(Val::Px(8.0)), ..default() },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.5)),
        ))
        .with_children(|p| {
            p.spawn((Text::new(""), TextFont { font_size: 16.0, ..default() }, TextColor(Color::WHITE), Hud));
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
fn gather(keys: Res<ButtonInput<KeyCode>>, mouse: Res<ButtonInput<MouseButton>>, mut cam: ResMut<CamRig>, mut pending: ResMut<Pending>) {
    let i = &mut pending.0;
    let mut mv = Vec2::ZERO;
    if cam.locked {
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
    if cam.swallow_click && !mouse.pressed(MouseButton::Left) {
        cam.swallow_click = false;
    }
    let attack_ok = cam.locked && !cam.swallow_click;
    i.attack.accumulate(
        mouse.pressed(MouseButton::Left) && attack_ok,
        mouse.just_pressed(MouseButton::Left) && attack_ok,
        mouse.just_released(MouseButton::Left),
    );
}

/// REWRITE_AUTOPLAY: scripted presses through the same pending input a player fills.
fn autoplay(time: Res<Time>, mut pending: ResMut<Pending>, mut done: Local<[bool; 4]>) {
    if std::env::var_os("REWRITE_AUTOPLAY").is_none() {
        return;
    }
    let t = time.elapsed_secs();
    let i = &mut pending.0;
    // Sprint forward, dodge mid-sprint, keep sprinting, jump: shows sprint speed, stamina and the camera.
    if (0.3..3.2).contains(&t) {
        i.mv = Vec2::Y;
        i.sprint.accumulate(true, t < 0.32, false);
    }
    for (k, at) in [1.2f32, 2.4, 9.0, 9.0].into_iter().enumerate() {
        if t > at && !done[k] {
            done[k] = true;
            match k {
                0 => i.dodge.accumulate(true, true, false),
                1 => i.jump.accumulate(true, true, false),
                _ => {}
            }
        }
    }
}

fn tick(mut game: ResMut<Game>, mut pending: ResMut<Pending>, mut rendered: ResMut<Rendered>, mut log: ResMut<Log>) {
    rendered.prev = game.0.player.pos;
    rendered.prev_yaw = game.0.player.yaw;
    game.0.step(&pending.0);
    pending.0.clear_edges();
    for e in &game.0.events {
        let line = match e {
            SimEvent::Hit { damage, hp_left, .. } => format!("hit {damage:.0} -> {hp_left:.0} hp"),
            SimEvent::Killed { .. } => "target down".to_string(),
            SimEvent::ActionStarted(name) => format!("> {name}"),
            SimEvent::Landed { fall_speed } => format!("landed {fall_speed:.1} m/s"),
            other => format!("{other:?}"),
        };
        log.0.push(line);
    }
    let excess = log.0.len().saturating_sub(8);
    log.0.drain(..excess);
}

fn interpolate(game: Res<Game>, time: Res<Time<Fixed>>, mut r: ResMut<Rendered>) {
    let p = &game.0.player;
    // A teleport (respawn) snaps instead of gliding.
    let alpha = if r.prev.distance(p.pos) > 3.0 { 1.0 } else { time.overstep_fraction() };
    r.pos = r.prev.lerp(p.pos, alpha);
    r.yaw = r.prev_yaw + sim::angle_diff(r.prev_yaw, p.yaw) * alpha;
}

fn draw(
    game: Res<Game>,
    r: Res<Rendered>,
    mut player: Query<&mut Transform, (With<PlayerBody>, Without<TargetBody>)>,
    mut targets: Query<(&TargetBody, &mut Transform, &mut Visibility), Without<PlayerBody>>,
) {
    let half = game.0.tuning.half_extents;
    if let Ok(mut tf) = player.single_mut() {
        tf.translation = r.pos + Vec3::Y * half.y;
        tf.rotation = Quat::from_rotation_y(r.yaw);
    }
    for (TargetBody(i), mut tf, mut vis) in &mut targets {
        let t = &game.0.targets[*i];
        tf.translation = t.pos + Vec3::Y * t.height * 0.5;
        *vis = if t.alive() { Visibility::Inherited } else { Visibility::Hidden };
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
    let rot = Quat::from_euler(EulerRot::YXZ, cam.yaw, cam.pitch, 0.0);
    let pivot = cam.pivot + rot * Vec3::X * CAM_SHOULDER;
    let back = rot * Vec3::Z;
    let want = sim::world::camera_arm(&game.0.world, pivot, back, CAM_ARM, 0.2, 0.3);
    cam.arm = if want < cam.arm { want } else { cam.arm + (want - cam.arm) * k };
    tf.translation = pivot + back * cam.arm;
    tf.rotation = rot;
    if let Projection::Perspective(p) = proj.as_mut() {
        p.fov = CAM_FOV.to_radians();
    }
}

fn hud(game: Res<Game>, log: Res<Log>, mut q: Query<&mut Text, With<Hud>>) {
    let Ok(mut text) = q.single_mut() else { return };
    let p = &game.0.player;
    let frame = match p.state {
        State::Act(a) => format!(" f{:.1}/{:.0}{}", a.f, a.def.len, if p.invincible() { " [i-frames]" } else { "" }),
        _ => String::new(),
    };
    let speed = Vec3::new(p.vel.x, 0.0, p.vel.z).length();
    let bar = "#".repeat((p.stamina / game.0.tuning.stamina_max * 20.0).clamp(0.0, 20.0) as usize);
    text.0 = format!(
        "{}{}{}\nspeed {:.2} m/s\nstamina [{:<20}] {:.0}\ndata: {}\nWASD move  Alt walk  Shift sprint  Space jump  Ctrl dodge  Esc mouse\n\n{}",
        p.state.name(),
        if p.sprinting { " (sprint)" } else { "" },
        frame,
        speed,
        bar,
        p.stamina,
        sim::extracted::SOURCE,
        log.0.join("\n")
    );
}

fn capture(time: Res<Time>, mut commands: Commands, mut exit: EventWriter<AppExit>, mut taken: Local<bool>) {
    let Some(path) = std::env::var_os("REWRITE_CAPTURE") else { return };
    let t = time.elapsed_secs();
    if !*taken && t > 3.0 {
        *taken = true;
        commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path.to_string_lossy().into_owned()));
    }
    if t > 4.5 {
        exit.write(AppExit::Success);
    }
}
