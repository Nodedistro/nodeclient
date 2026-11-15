use crate::world::{World, SIZE};
use glam::{vec3, Vec3};
pub const RADIUS: f32 = 0.30;
pub const HEIGHT: f32 = 1.80;
pub const EYE: f32 = 1.62;
pub const STEP: f32 = 1.0 / 120.0;

pub struct Player {
    pub position: Vec3,
    pub velocity: Vec3,
    pub grounded: bool,
}
impl Player {
    pub fn new(position: Vec3) -> Self {
        Self {
            position,
            velocity: Vec3::ZERO,
            grounded: false,
        }
    }
    pub fn eye(&self) -> Vec3 {
        self.position + Vec3::Y * EYE
    }
    pub fn tick(&mut self, world: &World, movement: Vec3, jump: bool, sprint: bool) {
        let movement = movement.normalize_or_zero() * if sprint { 7.0 } else { 4.3 };
        self.velocity.x = movement.x;
        self.velocity.z = movement.z;
        if jump && self.grounded {
            self.velocity.y = 8.0;
        }
        self.velocity.y = (self.velocity.y - 24.0 * STEP).max(-35.0);
        self.grounded = false;
        for axis in [0, 2, 1] {
            let delta = self.velocity[axis] * STEP;
            let mut proposed = self.position;
            proposed[axis] += delta;
            if collides(world, proposed) {
                // Binary search to the contact surface rather than stopping one tick short.
                let mut low = 0.0;
                let mut high = 1.0;
                for _ in 0..12 {
                    let mid = (low + high) * 0.5;
                    let mut p = self.position;
                    p[axis] += delta * mid;
                    if collides(world, p) {
                        high = mid;
                    } else {
                        low = mid;
                    }
                }
                self.position[axis] += delta * low;
                if axis == 1 && delta < 0.0 {
                    self.grounded = true;
                }
                self.velocity[axis] = 0.0;
            } else {
                self.position = proposed;
            }
        }
        if self.position.y < -20.0 {
            *self = Self::new(world.spawn());
        }
    }
    pub fn overlaps(&self, p: [i32; 3]) -> bool {
        let a = Vec3::from_array(p.map(|v| v as f32));
        let min = self.position - vec3(RADIUS, 0.0, RADIUS);
        let max = self.position + vec3(RADIUS, HEIGHT, RADIUS);
        (0..3).all(|i| min[i] < a[i] + 1.0 && max[i] > a[i])
    }
}
pub fn collides(world: &World, position: Vec3) -> bool {
    let min = position - vec3(RADIUS, 0.0, RADIUS);
    let max = position + vec3(RADIUS, HEIGHT, RADIUS);
    if min.x < 0.0 || min.z < 0.0 || max.x > SIZE[0] as f32 || max.z > SIZE[2] as f32 || min.y < 0.0
    {
        return true;
    }
    for y in min.y.floor() as i32..=(max.y - 0.0001).floor() as i32 {
        for z in min.z.floor() as i32..=(max.z - 0.0001).floor() as i32 {
            for x in min.x.floor() as i32..=(max.x - 0.0001).floor() as i32 {
                if world.solid([x, y, z]) {
                    return true;
                }
            }
        }
    }
    false
}
#[derive(Debug, PartialEq)]
pub struct Hit {
    pub block: [i32; 3],
    pub adjacent: [i32; 3],
}
/// Exact grid traversal, including negative directions and axis-aligned rays.
pub fn raycast(world: &World, origin: Vec3, direction: Vec3, reach: f32) -> Option<Hit> {
    if !origin.is_finite()
        || !direction.is_finite()
        || direction.length_squared() < 0.000001
        || !reach.is_finite()
        || reach < 0.0
    {
        return None;
    }
    let dir = direction.normalize();
    let mut cell = origin.floor().as_ivec3().to_array();
    let mut step = [0i32; 3];
    let mut next = [f32::INFINITY; 3];
    let mut stride = [f32::INFINITY; 3];
    for i in 0..3 {
        if dir[i] != 0.0 {
            step[i] = if dir[i] > 0.0 { 1 } else { -1 };
            let edge = cell[i] as f32 + if step[i] > 0 { 1.0 } else { 0.0 };
            next[i] = (edge - origin[i]) / dir[i];
            stride[i] = 1.0 / dir[i].abs();
        }
    }
    let mut previous = cell;
    for _ in 0..1024 {
        if world.solid(cell) {
            return Some(Hit {
                block: cell,
                adjacent: previous,
            });
        }
        let axis = if next[0] <= next[1] && next[0] <= next[2] {
            0
        } else if next[1] <= next[2] {
            1
        } else {
            2
        };
        if next[axis] > reach {
            return None;
        }
        previous = cell;
        cell[axis] += step[axis];
        next[axis] += stride[axis];
    }
    None
}
