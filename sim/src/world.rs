//! Collision, reduced to the two questions every character controller needs (as UE3, Havok and
//! most engines ask them): sweep a box along a path, and test a region for overlap. Any host
//! collision (boxes, triangle meshes, a physics engine) can implement `World`.

use glam::Vec3;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Aabb {
    pub min: Vec3,
    pub max: Vec3,
}

impl Aabb {
    pub fn new(min: Vec3, max: Vec3) -> Self {
        Self { min, max }
    }

    pub fn from_center_half(center: Vec3, half: Vec3) -> Self {
        Self { min: center - half, max: center + half }
    }

    pub fn center(&self) -> Vec3 {
        (self.min + self.max) * 0.5
    }

    pub fn half(&self) -> Vec3 {
        (self.max - self.min) * 0.5
    }

    /// Minkowski sum with a box of half size `half`.
    pub fn expand(&self, half: Vec3) -> Self {
        Self { min: self.min - half, max: self.max + half }
    }

    /// Strict overlap: touching faces do not count.
    pub fn overlaps(&self, o: &Aabb) -> bool {
        self.min.x < o.max.x
            && self.max.x > o.min.x
            && self.min.y < o.max.y
            && self.max.y > o.min.y
            && self.min.z < o.max.z
            && self.max.z > o.min.z
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SweepHit {
    /// Fraction of the delta travelled before contact, 0..=1.
    pub t: f32,
    /// Surface normal at the contact.
    pub normal: Vec3,
}

pub trait World {
    /// Sweep a box (half size `half`, centred at `start`) along `delta`; the first solid it runs
    /// into. Solids it starts inside (beyond touching) are ignored, like engine extent traces.
    fn sweep(&self, half: Vec3, start: Vec3, delta: Vec3) -> Option<SweepHit>;

    /// Whether anything solid is strictly inside `region`.
    fn overlaps(&self, region: &Aabb) -> bool;
}

/// Axis-aligned boxes: exact, simple, and enough for greybox levels and tests.
#[derive(Clone, Debug, Default)]
pub struct BoxWorld {
    pub boxes: Vec<Aabb>,
}

impl BoxWorld {
    pub fn with(mut self, min: [f32; 3], max: [f32; 3]) -> Self {
        self.boxes.push(Aabb::new(Vec3::from(min), Vec3::from(max)));
        self
    }

    /// Floor, walls, a 0.3 m step, a 1 m crate and a raised platform with a ledge.
    pub fn arena() -> Self {
        BoxWorld::default()
            .with([-20.0, -1.0, -20.0], [20.0, 0.0, 20.0])
            .with([-20.0, 0.0, -21.0], [20.0, 3.0, -20.0])
            .with([-20.0, 0.0, 20.0], [20.0, 3.0, 21.0])
            .with([-21.0, 0.0, -20.0], [-20.0, 3.0, 20.0])
            .with([20.0, 0.0, -20.0], [21.0, 3.0, 20.0])
            .with([3.0, 0.0, -2.0], [5.0, 0.3, 2.0])
            .with([6.0, 0.0, -1.0], [8.0, 1.0, 1.0])
            .with([-8.0, 0.0, 4.0], [-3.0, 2.0, 9.0])
    }
}

/// Ray vs box with the entry normal. None if missed, starting inside, or beyond t = 1.
fn ray_box(origin: Vec3, delta: Vec3, b: &Aabb) -> Option<(f32, Vec3)> {
    let mut t_enter = f32::NEG_INFINITY;
    let mut t_exit = f32::INFINITY;
    let mut normal = Vec3::ZERO;
    for axis in 0..3 {
        let (o, d, lo, hi) = (origin[axis], delta[axis], b.min[axis], b.max[axis]);
        if d.abs() < 1e-12 {
            // Parallel: must be strictly between the slabs (sliding along a face is free).
            if o <= lo || o >= hi {
                return None;
            }
            continue;
        }
        let inv = 1.0 / d;
        let (mut t1, mut t2) = ((lo - o) * inv, (hi - o) * inv);
        let mut sign = -1.0;
        if t1 > t2 {
            std::mem::swap(&mut t1, &mut t2);
            sign = 1.0;
        }
        if t1 > t_enter {
            t_enter = t1;
            normal = Vec3::ZERO;
            normal[axis] = sign;
        }
        t_exit = t_exit.min(t2);
        if t_enter >= t_exit {
            return None;
        }
    }
    if !(0.0..=1.0).contains(&t_enter) {
        return None;
    }
    Some((t_enter, normal))
}

impl World for BoxWorld {
    fn sweep(&self, half: Vec3, start: Vec3, delta: Vec3) -> Option<SweepHit> {
        self.boxes
            .iter()
            .filter_map(|b| ray_box(start, delta, &b.expand(half)))
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(t, normal)| SweepHit { t, normal })
    }

    fn overlaps(&self, region: &Aabb) -> bool {
        self.boxes.iter().any(|b| b.overlaps(region))
    }
}

/// Surfaces whose normal points up at least this much are floors.
pub const WALKABLE: f32 = 0.7;
const SKIN: f32 = 0.001;

#[derive(Clone, Copy, Debug)]
pub struct Slide {
    pub center: Vec3,
    pub vel: Vec3,
    pub floor: bool,
    pub wall: bool,
    pub ceiling: bool,
}

/// Collide-and-slide: move, and on contact remove the velocity into the surface and keep going.
pub fn move_and_slide(world: &dyn World, half: Vec3, mut center: Vec3, mut vel: Vec3, dt: f32) -> Slide {
    let mut delta = vel * dt;
    let (mut floor, mut wall, mut ceiling) = (false, false, false);
    for _ in 0..4 {
        let len = delta.length();
        if len < 1e-7 {
            break;
        }
        match world.sweep(half, center, delta) {
            None => {
                center += delta;
                break;
            }
            Some(hit) => {
                let safe = (hit.t - SKIN / len).max(0.0);
                center += delta * safe;
                let mut rest = delta * (1.0 - safe);
                rest -= hit.normal * rest.dot(hit.normal);
                let into = vel.dot(hit.normal);
                if into < 0.0 {
                    vel -= hit.normal * into;
                }
                if hit.normal.y >= WALKABLE {
                    floor = true;
                } else if hit.normal.y <= -WALKABLE {
                    ceiling = true;
                } else {
                    wall = true;
                }
                delta = rest;
            }
        }
    }
    Slide { center, vel, floor, wall, ceiling }
}

/// Snap down onto ground within `max_drop` (walking down steps without leaving the ground).
pub fn snap_down(world: &dyn World, half: Vec3, center: Vec3, max_drop: f32) -> Option<Vec3> {
    let delta = Vec3::NEG_Y * max_drop;
    let hit = world.sweep(half, center, delta)?;
    (hit.normal.y >= WALKABLE).then(|| center + delta * (hit.t - SKIN / max_drop).max(0.0))
}

/// Third-person spring arm: how far the camera can sit from `pivot` along `dir` (unit vector)
/// before a sphere-ish probe of `radius` hits level geometry. Never closer than `min_len`.
pub fn camera_arm(world: &dyn World, pivot: Vec3, dir: Vec3, max_len: f32, radius: f32, min_len: f32) -> f32 {
    match world.sweep(Vec3::splat(radius), pivot, dir * max_len) {
        Some(hit) => (max_len * hit.t - 0.05).max(min_len),
        None => max_len,
    }
}

/// Shortest distance between segments a-b and c-d (weapon line vs body capsule).
pub fn segment_distance(a: Vec3, b: Vec3, c: Vec3, d: Vec3) -> f32 {
    let (u, v, w) = (b - a, d - c, a - c);
    let (uu, uv, vv, uw, vw) = (u.dot(u), u.dot(v), v.dot(v), u.dot(w), v.dot(w));
    let denom = uu * vv - uv * uv;
    let mut s = if denom > 1e-8 { ((uv * vw - vv * uw) / denom).clamp(0.0, 1.0) } else { 0.0 };
    let t = if vv > 1e-8 { ((uv * s + vw) / vv).clamp(0.0, 1.0) } else { 0.0 };
    if uu > 1e-8 {
        s = ((uv * t - uw) / uu).clamp(0.0, 1.0);
    }
    (a + u * s).distance(c + v * t)
}
