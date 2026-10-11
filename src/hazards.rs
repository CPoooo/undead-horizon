//! Terrain hazards in the procedural world (Infinite + Campaign): quicksand
//! patches bog you down, ice patches make you slide. They're rare, there are
//! more of them on harder difficulties (`Difficulty::hazards`), none on Rookie
//! and none near the start. The Frozen Reach only has ice.
//!
//! Patches are a pure function of the world cell and the density, so every
//! co-op peer sees the same ones without any syncing.

use crate::game::Game;
use crate::math::*;
use crate::world::{self, Biome};

/// One candidate patch per cell of this size.
pub const HAZ_CELL: f32 = 640.0;
/// Chance a cell has a patch at density 1 (Nightmare).
const CHANCE: f32 = 0.3;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Hazard {
    Quicksand,
    Ice,
}

#[derive(Clone, Copy, Debug)]
pub struct Patch {
    pub pos: V2,
    pub r: f32,
    pub kind: Hazard,
    pub seed: u32,
}

pub fn patch_at(cx: i32, cy: i32, density: f32) -> Option<Patch> {
    if density <= 0.0 || hashf(cx, cy, 901) > density * CHANCE {
        return None;
    }
    let pos = v2(
        (cx as f32 + 0.2 + 0.6 * hashf(cx, cy, 902)) * HAZ_CELL,
        (cy as f32 + 0.2 + 0.6 * hashf(cx, cy, 903)) * HAZ_CELL,
    );
    if pos.len() < world::SAFE_RADIUS + 420.0 {
        return None;
    }
    let r = 70.0 + hashf(cx, cy, 904) * 70.0;
    let kind = if world::biome_at(pos) == Biome::Snow || hashf(cx, cy, 905) < 0.35 {
        Hazard::Ice
    } else {
        Hazard::Quicksand
    };
    Some(Patch { pos, r, kind, seed: hash2(cx, cy, 906) })
}

/// Every patch overlapping the box.
pub fn for_patches_in(min: V2, max: V2, density: f32, mut f: impl FnMut(&Patch)) {
    if density <= 0.0 {
        return;
    }
    let x0 = ((min.x - 150.0) / HAZ_CELL).floor() as i32;
    let y0 = ((min.y - 150.0) / HAZ_CELL).floor() as i32;
    let x1 = ((max.x + 150.0) / HAZ_CELL).floor() as i32;
    let y1 = ((max.y + 150.0) / HAZ_CELL).floor() as i32;
    for cy in y0..=y1 {
        for cx in x0..=x1 {
            if let Some(p) = patch_at(cx, cy, density) {
                f(&p);
            }
        }
    }
}

/// The hazard under a point, if any. Patches are drawn as soft ellipses
/// (wider than tall), so the test uses the same shape.
pub fn hazard_at(p: V2, density: f32) -> Option<Hazard> {
    if density <= 0.0 {
        return None;
    }
    let cx = (p.x / HAZ_CELL).floor() as i32;
    let cy = (p.y / HAZ_CELL).floor() as i32;
    for dy in -1..=1 {
        for dx in -1..=1 {
            if let Some(h) = patch_at(cx + dx, cy + dy, density) {
                let d = p - h.pos;
                let e = (d.x / h.r).powi(2) + (d.y / (h.r * 0.75)).powi(2);
                if e < 0.85 {
                    return Some(h.kind);
                }
            }
        }
    }
    None
}

impl Game {
    /// Hazard density for this run: only the procedural world (Infinite and
    /// Campaign) has hazards.
    pub fn hazard_density(&self) -> f32 {
        if self.world_has_sky() && matches!(self.mode, crate::modes::Mode::Infinite | crate::modes::Mode::Campaign(_)) {
            self.difficulty().hazards
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hazards_scale_with_difficulty_and_skip_the_start() {
        let count = |d: f32| {
            let mut n = 0;
            for_patches_in(v2(-8000.0, -8000.0), v2(8000.0, 8000.0), d, |_| n += 1);
            n
        };
        assert_eq!(count(0.0), 0, "Rookie has none");
        let (half, full) = (count(0.5), count(1.0));
        assert!(half > 0 && full > half, "more on harder difficulties ({half} vs {full})");
        // rare: well under a third of the cells even on Nightmare
        let cells = (16000.0f32 / HAZ_CELL).powi(2);
        assert!((full as f32) < cells * 0.35, "{full} of {cells}");
        // never at the start
        for x in -6..=6 {
            for y in -6..=6 {
                assert!(hazard_at(v2(x as f32 * 60.0, y as f32 * 60.0), 1.0).is_none());
            }
        }
        // a patch's centre is inside a hazard; snow patches are ice
        for_patches_in(v2(-8000.0, -8000.0), v2(8000.0, 8000.0), 1.0, |p| {
            assert!(hazard_at(p.pos, 1.0).is_some());
            if world::biome_at(p.pos) == Biome::Snow {
                assert_eq!(p.kind, Hazard::Ice);
            }
        });
    }
}
