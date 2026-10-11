//! Destructible props in the open world: shoot red barrels to blow up the
//! horde around them, break crates open for loot. The procedural world is
//! pure hashing, so destroyed props are remembered by their seed.

use crate::audio::Sfx;
use crate::game::{Game, PickKind, Pickup};
use crate::math::*;
use crate::world::{self, ObKind};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

thread_local! {
    static DESTROYED: RefCell<HashSet<u32>> = RefCell::new(HashSet::new());
    static DAMAGE: RefCell<HashMap<u32, f32>> = RefCell::new(HashMap::new());
}

pub fn is_destroyed(seed: u32) -> bool {
    DESTROYED.with(|d| {
        let d = d.borrow();
        !d.is_empty() && d.contains(&seed)
    })
}

pub fn reset() {
    DESTROYED.with(|d| d.borrow_mut().clear());
    DAMAGE.with(|d| d.borrow_mut().clear());
}

fn hp_of(kind: ObKind) -> Option<f32> {
    match kind {
        ObKind::Barrel => Some(25.0),
        ObKind::Crate => Some(60.0),
        _ => None,
    }
}

impl Game {
    /// A bullet at `pos` hits a destructible prop? Returns true if it was absorbed.
    pub fn bullet_vs_props(&mut self, pos: V2, dmg: f32) -> bool {
        if world::map().is_some() {
            return false; // authored maps keep their props (nav depends on them)
        }
        let mut hit = None;
        world::for_obstacles_in(pos - v2(40.0, 40.0), pos + v2(40.0, 40.0), |o| {
            if hit.is_none() && hp_of(o.kind).is_some() && o.pos.dist(pos) < o.r + 4.0 {
                hit = Some(*o);
            }
        });
        let Some(o) = hit else { return false };
        let max = hp_of(o.kind).unwrap_or(1.0);
        let left = DAMAGE.with(|d| {
            let mut d = d.borrow_mut();
            let e = d.entry(o.seed).or_insert(max);
            *e -= dmg;
            *e
        });
        self.burst(pos, 3, 160.0, if o.kind == ObKind::Barrel { "#b8321e" } else { "#8a6236" }, 2.5, 0.3, true, false);
        if left <= 0.0 {
            DESTROYED.with(|d| d.borrow_mut().insert(o.seed));
            match o.kind {
                ObKind::Barrel => {
                    let dmg = 60.0 + 8.0 * self.wave.n as f32;
                    self.explode(o.pos, 150.0, dmg, true);
                    self.light(o.pos, 400.0, 0.5, "#ff9040");
                    self.burst(o.pos, 30, 600.0, "#ffb24a", 5.0, 0.8, true, true);
                    self.decal_scorch(o.pos);
                }
                _ => {
                    self.burst(o.pos, 24, 320.0, "#8a6236", 4.0, 0.8, true, false);
                    self.sfx(Sfx::Hit, o.pos);
                    // loot
                    let r = self.rng.f();
                    let kind = if r < 0.18 {
                        Some(PickKind::Heart)
                    } else if r < 0.26 {
                        Some(PickKind::Frenzy)
                    } else if r < 0.3 {
                        Some(PickKind::Vacuum)
                    } else {
                        None
                    };
                    if let Some(k) = kind {
                        self.pickups.push(Pickup { pos: o.pos, kind: k, t: 0.0, dead: false });
                    }
                    for _ in 0..3 {
                        let v = 2 + self.wave.n / 3;
                        self.spawn_gem_pub(o.pos, v);
                    }
                }
            }
        }
        true
    }

    fn decal_scorch(&mut self, pos: V2) {
        let seed = self.rng.next_u32();
        if self.decals.len() >= 260 {
            self.decals.remove(0);
        }
        self.decals.push(crate::game::Decal { pos, r: 80.0, life: 30.0, color: "#1a1410", seed });
    }
}
