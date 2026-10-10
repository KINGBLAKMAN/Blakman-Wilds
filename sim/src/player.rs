//! The player: one explicit state machine. Each state is a variant; each tick runs exactly one
//! state's handler, which may start another state. No booleans-soup.

use crate::actions::{self, Act, ActionDef, Req, FRAMES_PER_TICK};
use crate::events::Event;
use crate::input::Input;
use crate::tuning::Tuning;
use crate::world::{move_and_slide, snap_down, World};
use crate::{dir_of, turn_toward, yaw_of, DT};
use glam::Vec3;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum State {
    Ground,
    Air,
    Act(Act),
}

impl State {
    pub fn name(&self) -> &'static str {
        match self {
            State::Ground => "Ground",
            State::Air => "Air",
            State::Act(a) => a.def.name,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Player {
    /// Feet position.
    pub pos: Vec3,
    pub vel: Vec3,
    pub yaw: f32,
    pub state: State,
    pub stamina: f32,
    /// Currently sprinting (fast, drains stamina).
    pub sprinting: bool,
    /// Sprint ran dry: no sprinting until the button is released and pressed again.
    sprint_locked: bool,
    /// Seconds until stamina starts regenerating.
    regen_wait: f32,
    coyote: f32,
    jump_buffer: f32,
    fall_peak_speed: f32,
}

impl Player {
    pub fn new(pos: Vec3, t: &Tuning) -> Self {
        Self {
            pos,
            vel: Vec3::ZERO,
            yaw: 0.0,
            state: State::Ground,
            stamina: t.stamina_max,
            sprinting: false,
            sprint_locked: false,
            regen_wait: 0.0,
            coyote: 0.0,
            jump_buffer: 0.0,
            fall_peak_speed: 0.0,
        }
    }

    pub fn facing(&self) -> Vec3 {
        dir_of(self.yaw)
    }

    pub fn grounded(&self) -> bool {
        self.state == State::Ground
    }

    pub fn invincible(&self) -> bool {
        matches!(self.state, State::Act(a) if a.def.iframes.contains(a.f))
    }

    /// The live weapon line this tick (chest to reach), if an action's hit window is open.
    pub fn weapon_line(&self) -> Option<(&'static ActionDef, Vec3, Vec3)> {
        match self.state {
            State::Act(a) if !a.connected && a.def.hit.contains(a.f) => {
                let chest = self.pos + Vec3::Y * 1.2;
                Some((a.def, chest, chest + self.facing() * a.def.reach))
            }
            _ => None,
        }
    }

    pub fn mark_connected(&mut self) {
        if let State::Act(a) = &mut self.state {
            a.connected = true;
        }
    }

    pub fn step(&mut self, inp: &Input, world: &dyn World, t: &Tuning, ev: &mut Vec<Event>) {
        self.jump_buffer = if inp.jump.pressed { t.jump_buffer } else { (self.jump_buffer - DT).max(0.0) };
        match self.state {
            State::Act(a) => self.act(a, inp, world, t, ev),
            State::Ground | State::Air => {
                self.locomotion(inp, world, t, ev);
            }
        }
        // Stamina: regen only after a pause since the last spend, and never while acting.
        if !inp.sprint.held {
            self.sprint_locked = false;
        }
        if self.regen_wait > 0.0 {
            self.regen_wait -= DT;
        } else if !matches!(self.state, State::Act(_)) {
            self.stamina = (self.stamina + t.stamina_regen * DT).min(t.stamina_max);
        }
    }

    fn try_start(&mut self, name: &str, t: &Tuning, ev: &mut Vec<Event>) -> bool {
        let Some(def) = actions::find(name) else { return false };
        // Souls rule: any stamina left lets you start; it can go negative and must regen.
        if self.stamina <= 0.0 {
            return false;
        }
        self.stamina -= def.stamina;
        self.regen_wait = t.stamina_regen_delay;
        self.sprinting = false;
        self.state = State::Act(Act::new(def));
        ev.push(Event::ActionStarted(def.name));
        true
    }

    fn wish(inp: &Input) -> Vec3 {
        let m = inp.mv.clamp_length_max(1.0);
        let fwd = dir_of(inp.look_yaw);
        let right = Vec3::new(-fwd.z, 0.0, fwd.x);
        fwd * m.y + right * m.x
    }

    fn locomotion(&mut self, inp: &Input, world: &dyn World, t: &Tuning, ev: &mut Vec<Event>) {
        let grounded = self.grounded();
        if grounded && inp.attack.pressed && self.try_start("Light1", t, ev) {
            return;
        }
        if grounded && inp.dodge.pressed && self.try_start("Roll", t, ev) {
            if let Some(dir) = Self::wish(inp).try_normalize() {
                self.yaw = yaw_of(dir);
            }
            return;
        }

        let wish = Self::wish(inp);
        let tilt = inp.mv.length().min(1.0);
        // Sprint: held, moving at run tilt, stamina left and not locked out. Starts only on the
        // ground; a sprint jump keeps sprint speed through the air (drain pauses while airborne).
        self.sprinting = inp.sprint.held
            && tilt >= t.walk_tilt
            && (grounded || self.sprinting)
            && self.stamina > 0.0
            && !self.sprint_locked;
        if self.sprinting && grounded {
            self.stamina -= t.sprint_drain * DT;
            self.regen_wait = t.stamina_regen_delay;
            if self.stamina <= 0.0 {
                self.stamina = 0.0;
                self.sprint_locked = true;
            }
        }
        let speed = if tilt < t.walk_tilt {
            t.walk_speed
        } else if self.sprinting {
            t.sprint_speed
        } else {
            t.run_speed
        };
        let target = wish.normalize_or_zero() * speed * if tilt > 0.05 { 1.0 } else { 0.0 };
        let rate = match (grounded, tilt > 0.05) {
            (true, true) => t.accel,
            (true, false) => t.decel,
            (false, _) => t.air_accel,
        };
        let flat = Vec3::new(self.vel.x, 0.0, self.vel.z);
        let dv = target - flat;
        let step = rate * DT;
        let flat = if dv.length() <= step { target } else { flat + dv.normalize() * step };
        self.vel.x = flat.x;
        self.vel.z = flat.z;
        if let Some(dir) = wish.try_normalize() {
            self.yaw = turn_toward(self.yaw, yaw_of(dir), t.turn_rate_deg.to_radians() * DT);
        }

        // Buffered press + coyote time: a jump pressed just before landing, or just after
        // running off an edge, still happens.
        // Jumping costs stamina (wiki: 10). ESTIMATE: any stamina above 0 allows the jump.
        if self.jump_buffer > 0.0 && (grounded || self.coyote > 0.0) && self.stamina > 0.0 {
            self.stamina -= t.jump_stamina;
            self.regen_wait = t.stamina_regen_delay;
            self.vel.y = (2.0 * t.gravity * t.jump_height).sqrt();
            self.jump_buffer = 0.0;
            self.coyote = 0.0;
            self.state = State::Air;
            ev.push(Event::Jumped);
        }

        let was_ground = self.state == State::Ground;
        let on_ground = self.integrate(world, t);
        if on_ground {
            if !was_ground {
                ev.push(Event::Landed { fall_speed: self.fall_peak_speed });
            }
            self.state = State::Ground;
            self.coyote = t.coyote_time;
            self.fall_peak_speed = 0.0;
        } else {
            if was_ground {
                self.state = State::Air;
            }
            self.coyote = (self.coyote - DT).max(0.0);
            self.fall_peak_speed = self.fall_peak_speed.max(-self.vel.y);
        }
    }

    fn act(&mut self, mut a: Act, inp: &Input, world: &dyn World, t: &Tuning, ev: &mut Vec<Event>) {
        let def = a.def;
        if a.f >= def.input_from {
            if inp.attack.pressed {
                a.buffered = Some(Req::Attack);
            } else if inp.dodge.pressed {
                a.buffered = Some(Req::Dodge);
            }
        }

        // Root motion: the action moves the body, not the stick.
        let speed = def.root_motion / def.seconds();
        let fwd = self.facing();
        self.vel.x = fwd.x * speed;
        self.vel.z = fwd.z * speed;
        self.integrate(world, t);
        a.f += FRAMES_PER_TICK;

        match a.buffered {
            Some(Req::Attack) if a.f >= def.cancel_into_attack && !def.next.is_empty() => {
                if self.try_start(def.next, t, ev) {
                    return;
                }
            }
            Some(Req::Dodge) if a.f >= def.cancel_into_dodge => {
                if let Some(dir) = Self::wish(inp).try_normalize() {
                    self.yaw = yaw_of(dir);
                }
                if self.try_start("Roll", t, ev) {
                    return;
                }
            }
            _ => {}
        }
        let moved = inp.mv.length_squared() > 0.01;
        if a.f >= def.len || (a.f >= def.cancel_into_move && moved) {
            self.vel.x = 0.0;
            self.vel.z = 0.0;
            self.state = State::Ground;
            ev.push(Event::ActionEnded(def.name));
            return;
        }
        self.state = State::Act(a);
    }

    /// Gravity + collide-and-slide + ground snap. Returns whether we stand on ground.
    fn integrate(&mut self, world: &dyn World, t: &Tuning) -> bool {
        // Exact for constant gravity: move with the tick's average vertical speed, so jump
        // heights match sqrt(2 g h) at any tick rate (plain Euler peaks ~5 cm low at 60 Hz).
        let vy_new = self.vel.y - t.gravity * DT;
        let half = t.half_extents;
        let move_vel = Vec3::new(self.vel.x, 0.5 * (self.vel.y + vy_new), self.vel.z);
        let r = move_and_slide(world, half, self.pos + Vec3::Y * half.y, move_vel, DT);
        let mut center = r.center;
        self.vel = Vec3::new(r.vel.x, vy_new, r.vel.z);
        if (r.floor && self.vel.y < 0.0) || (r.ceiling && self.vel.y > 0.0) {
            self.vel.y = 0.0;
        }
        let mut on_ground = r.floor && vy_new <= 0.0;
        if !on_ground && self.grounded() && self.vel.y <= 0.0 {
            if let Some(snapped) = snap_down(world, half, center, t.step_height) {
                center = snapped;
                on_ground = true;
            }
        }
        if on_ground {
            self.vel.y = 0.0;
        }
        self.pos = center - Vec3::Y * half.y;
        on_ground
    }
}
