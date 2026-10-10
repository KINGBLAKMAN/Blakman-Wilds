//! Every number that shapes feel, in one place.
//!
//! Source rule: each field says where its value comes from.
//!   `SRC:` extracted (data/measurements.json → generated table; MEASURED or ESTIMATE there)
//!   `ESTIMATE:` tuned here, not yet measured
//! Extracted values are read from `crate::extracted`.

use crate::extracted;
use glam::Vec3;

#[derive(Clone, Debug)]
pub struct Tuning {
    /// SRC: extracted::WALK_SPEED (light stick tilt / walk key)
    pub walk_speed: f32,
    /// SRC: extracted::RUN_SPEED (default movement)
    pub run_speed: f32,
    /// SRC: extracted::SPRINT_SPEED (hold sprint, costs stamina)
    pub sprint_speed: f32,
    /// Stick tilt below which the character walks. ESTIMATE
    pub walk_tilt: f32,
    /// SRC: extracted::ACCEL, m/s² toward the wished velocity on the ground.
    pub accel: f32,
    /// SRC: extracted::BRAKING, m/s² braking with no input.
    pub decel: f32,
    /// SRC: extracted::AIR_ACCEL, m/s² of steering in the air.
    pub air_accel: f32,
    /// Degrees per second the body turns toward the move direction. ESTIMATE
    pub turn_rate_deg: f32,
    /// SRC: extracted::GRAVITY
    pub gravity: f32,
    /// SRC: extracted::JUMP_HEIGHT. Launch speed is sqrt(2 g h).
    pub jump_height: f32,
    /// Seconds after walking off a ledge that a jump still works. ESTIMATE
    pub coyote_time: f32,
    /// Seconds a jump press is remembered before landing. ESTIMATE
    pub jump_buffer: f32,
    /// Ground snap distance when walking down steps. ESTIMATE
    pub step_height: f32,
    /// Collision box half extents. ESTIMATE
    pub half_extents: Vec3,
    /// SRC: extracted::STAMINA_MAX
    pub stamina_max: f32,
    /// SRC: extracted::SPRINT_DRAIN, stamina per second while sprinting.
    pub sprint_drain: f32,
    /// SRC: extracted::STAMINA_REGEN, stamina per second once regen starts.
    pub stamina_regen: f32,
    /// SRC: extracted::STAMINA_REGEN_DELAY, seconds after the last spend before regen starts.
    pub stamina_regen_delay: f32,
    /// SRC: extracted::JUMP_STAMINA, stamina per jump.
    pub jump_stamina: f32,
    /// Thickness of the weapon line when testing hits. ESTIMATE (combat out of scope)
    pub weapon_radius: f32,
    /// Ticks both sides freeze when a blow lands. ESTIMATE (combat out of scope)
    pub hit_stop_ticks: u32,
    /// Seconds before a dead target stands back up. ESTIMATE (sandbox only)
    pub target_respawn: f32,
}

impl Default for Tuning {
    fn default() -> Self {
        Self {
            walk_speed: extracted::WALK_SPEED,
            run_speed: extracted::RUN_SPEED,
            sprint_speed: extracted::SPRINT_SPEED,
            walk_tilt: 0.55,
            accel: extracted::ACCEL,
            decel: extracted::BRAKING,
            air_accel: extracted::AIR_ACCEL,
            turn_rate_deg: 720.0,
            gravity: extracted::GRAVITY,
            jump_height: extracted::JUMP_HEIGHT,
            coyote_time: 0.1,
            jump_buffer: 0.15,
            step_height: 0.35,
            half_extents: Vec3::new(0.3, 0.9, 0.3),
            stamina_max: extracted::STAMINA_MAX,
            sprint_drain: extracted::SPRINT_DRAIN,
            stamina_regen: extracted::STAMINA_REGEN,
            stamina_regen_delay: extracted::STAMINA_REGEN_DELAY,
            jump_stamina: extracted::JUMP_STAMINA,
            weapon_radius: 0.1,
            hit_stop_ticks: 4,
            target_respawn: 2.0,
        }
    }
}
