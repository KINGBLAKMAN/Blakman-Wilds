//! One tick's worth of player intent. The app folds every rendered frame into a pending `Input`
//! and hands it to `Sim::step` at each fixed tick, then clears the edges.

use glam::Vec2;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Button {
    pub held: bool,
    /// Went down since the last tick.
    pub pressed: bool,
    /// Went up since the last tick.
    pub released: bool,
}

impl Button {
    pub const DOWN: Button = Button { held: true, pressed: true, released: false };
    pub const HELD: Button = Button { held: true, pressed: false, released: false };
    pub const UP: Button = Button { held: false, pressed: false, released: true };

    /// Fold one rendered frame in. Edges are OR-ed, so a tap shorter than a tick is never lost.
    pub fn accumulate(&mut self, down: bool, just_pressed: bool, just_released: bool) {
        self.held = down;
        self.pressed |= just_pressed;
        self.released |= just_released;
    }

    pub fn clear_edges(&mut self) {
        self.pressed = false;
        self.released = false;
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Input {
    /// Move stick: x right, y forward, length at most 1 (tilt matters: walk vs run).
    pub mv: Vec2,
    /// Camera yaw the stick is relative to.
    pub look_yaw: f32,
    pub jump: Button,
    pub dodge: Button,
    pub attack: Button,
    /// Hold to sprint (costs stamina).
    pub sprint: Button,
    /// Light the Beacon when standing by it.
    pub interact: Button,
    /// Eat the most filling food you carry.
    pub eat: Button,
    /// Craft this recipe (index into `Content::recipes`) this tick.
    pub craft: Option<usize>,
}

impl Input {
    pub fn clear_edges(&mut self) {
        for b in [&mut self.jump, &mut self.dodge, &mut self.attack, &mut self.sprint, &mut self.interact, &mut self.eat] {
            b.clear_edges();
        }
        self.craft = None;
    }
}
