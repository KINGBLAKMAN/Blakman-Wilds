//! Frame-data actions (attacks, rolls, anything with windows).
//!
//! All timings are animation frames at `ANIM_FPS`. The simulation ticks at 60 Hz, so each tick
//! advances `FRAMES_PER_TICK` frames. The table itself lives in `crate::extracted::ACTIONS`.

use crate::{extracted, DT};

pub const ANIM_FPS: f32 = 30.0;
pub const FRAMES_PER_TICK: f32 = ANIM_FPS * DT;

/// Half-open frame range [from, to).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Window {
    pub from: f32,
    pub to: f32,
}

impl Window {
    pub const NEVER: Window = Window { from: 9999.0, to: 9999.0 };

    pub fn contains(&self, frame: f32) -> bool {
        frame >= self.from && frame < self.to
    }

    pub fn frames(&self) -> f32 {
        (self.to - self.from).max(0.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ActionDef {
    pub name: &'static str,
    /// Total length in frames.
    pub len: f32,
    /// Frames the weapon can connect.
    pub hit: Window,
    /// Frames the character cannot be hurt.
    pub iframes: Window,
    /// Presses before this frame are dropped (mashing early does nothing).
    pub input_from: f32,
    /// A buffered attack fires at this frame (the chain's next link).
    pub cancel_into_attack: f32,
    /// A buffered dodge fires at this frame.
    pub cancel_into_dodge: f32,
    /// Moving the stick ends the action from this frame.
    pub cancel_into_move: f32,
    /// Metres travelled forward over the whole action (linear stand-in for a root-motion curve).
    pub root_motion: f32,
    pub stamina: f32,
    pub damage: f32,
    /// Weapon line length from the chest, metres.
    pub reach: f32,
    /// Next link in the chain ("" = none).
    pub next: &'static str,
}

impl ActionDef {
    pub fn seconds(&self) -> f32 {
        self.len / ANIM_FPS
    }
}

pub fn find(name: &str) -> Option<&'static ActionDef> {
    extracted::ACTIONS.iter().find(|a| a.name == name)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Req {
    Attack,
    Dodge,
}

/// A running action.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Act {
    pub def: &'static ActionDef,
    /// Current frame.
    pub f: f32,
    pub buffered: Option<Req>,
    /// Already hit something this action (one hit per swing).
    pub connected: bool,
}

impl Act {
    pub fn new(def: &'static ActionDef) -> Self {
        Self { def, f: 0.0, buffered: None, connected: false }
    }
}
