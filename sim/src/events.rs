//! What happened this tick. Gameplay pushes; HUD, audio, VFX and tests read.

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    Jumped,
    Landed { fall_speed: f32 },
    ActionStarted(&'static str),
    ActionEnded(&'static str),
    Hit { target: usize, damage: f32, hp_left: f32 },
    Killed { target: usize },
}
