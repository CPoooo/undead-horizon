//! Infinite, deterministic procedural world. Nothing is stored: every obstacle and
//! ground detail is derived from a hash of its cell coordinate, so the map is
//! endless and costs zero memory.

use crate::math::{hash2, hashf, v2, V2};

pub const OBST_CELL: f32 = 230.0;
pub const SAFE_RADIUS: f32 = 340.0;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ObKind {
    Tree,
    Rock,
    Grave,
}

#[derive(Clone, Copy)]
pub struct Obstacle {
    pub pos: V2,
    pub r: f32,
    pub kind: ObKind,
    pub seed: u32,
}

pub fn obstacle_at(cx: i32, cy: i32) -> Option<Obstacle> {
    let p = hashf(cx, cy, 11);
    // low-frequency "biome" density so there are forests and clearings
    let bx = (cx as f32 / 6.0).floor() as i32;
    let by = (cy as f32 / 6.0).floor() as i32;
    let density = 0.08 + hashf(bx, by, 99) * 0.28;
    if p > density {
        return None;
    }
    let seed = hash2(cx, cy, 7);
    let ox = 40.0 + hashf(cx, cy, 3) * (OBST_CELL - 80.0);
    let oy = 40.0 + hashf(cx, cy, 4) * (OBST_CELL - 80.0);
    let pos = v2(cx as f32 * OBST_CELL + ox, cy as f32 * OBST_CELL + oy);
    if pos.len() < SAFE_RADIUS {
        return None;
    }
    let k = hashf(cx, cy, 5);
    let (kind, r) = if k < 0.62 {
        (ObKind::Tree, 20.0 + hashf(cx, cy, 6) * 12.0)
    } else if k < 0.88 {
        (ObKind::Rock, 18.0 + hashf(cx, cy, 6) * 18.0)
    } else {
        (ObKind::Grave, 15.0)
    };
    Some(Obstacle { pos, r, kind, seed })
}

/// Visit every obstacle whose cell intersects the given AABB.
pub fn for_obstacles_in(min: V2, max: V2, mut f: impl FnMut(&Obstacle)) {
    let x0 = (min.x / OBST_CELL).floor() as i32 - 1;
    let y0 = (min.y / OBST_CELL).floor() as i32 - 1;
    let x1 = (max.x / OBST_CELL).floor() as i32 + 1;
    let y1 = (max.y / OBST_CELL).floor() as i32 + 1;
    for cy in y0..=y1 {
        for cx in x0..=x1 {
            if let Some(o) = obstacle_at(cx, cy) {
                f(&o);
            }
        }
    }
}

/// Push a circle out of any obstacles. Returns the corrected position and
/// the push normal (zero if no collision).
pub fn resolve(pos: V2, r: f32) -> (V2, V2) {
    let mut p = pos;
    let mut n = V2::ZERO;
    let cx = (p.x / OBST_CELL).floor() as i32;
    let cy = (p.y / OBST_CELL).floor() as i32;
    for dy in -1..=1 {
        for dx in -1..=1 {
            if let Some(o) = obstacle_at(cx + dx, cy + dy) {
                let d = p - o.pos;
                let rr = r + o.r;
                let l2 = d.len2();
                if l2 < rr * rr {
                    let l = l2.sqrt().max(0.001);
                    let push = d * (1.0 / l);
                    p = o.pos + push * rr;
                    n += push;
                }
            }
        }
    }
    (p, n)
}

/// Nearest obstacle blocking a straight line of travel (for zombie steering).
pub fn blocking(pos: V2, dir: V2, look: f32, r: f32) -> Option<Obstacle> {
    let cx = (pos.x / OBST_CELL).floor() as i32;
    let cy = (pos.y / OBST_CELL).floor() as i32;
    let mut best: Option<(f32, Obstacle)> = None;
    for dy in -1..=1 {
        for dx in -1..=1 {
            if let Some(o) = obstacle_at(cx + dx, cy + dy) {
                let to = o.pos - pos;
                let along = to.dot(dir);
                if along <= 0.0 || along > look + o.r {
                    continue;
                }
                let lateral = (to - dir * along).len();
                if lateral < o.r + r + 4.0 && best.map_or(true, |b| along < b.0) {
                    best = Some((along, o));
                }
            }
        }
    }
    best.map(|b| b.1)
}
