//! Elite affixes: tougher zombies with one special trait, a colored aura,
//! an icon and a nameplate. More common in later waves; they drop chests.

use crate::audio::Sfx;
use crate::game::{EShot, Game, ZKind, DT};
use crate::math::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Affix {
    None = 0,
    /// a shield soaks damage first
    Shielded,
    /// much faster
    Hasted,
    /// splits into smaller zombies on death
    Splitting,
    /// regenerates, and heals by biting
    Vampiric,
    /// blows up on death (telegraphed)
    Volatile,
    /// bounces some bullets back as spit
    Reflective,
}

pub const AFFIXES: [Affix; 6] = [
    Affix::Shielded,
    Affix::Hasted,
    Affix::Splitting,
    Affix::Vampiric,
    Affix::Volatile,
    Affix::Reflective,
];

impl Affix {
    pub fn from_u8(v: u8) -> Affix {
        AFFIXES
            .iter()
            .copied()
            .find(|a| *a as u8 == v)
            .unwrap_or(Affix::None)
    }
    /// (name, icon, aura color)
    pub fn def(self) -> (&'static str, &'static str, &'static str) {
        match self {
            Affix::None => ("", "", "#ffffff"),
            Affix::Shielded => ("Shielded", "🛡️", "#6cc8ff"),
            Affix::Hasted => ("Hasted", "⏩", "#ffb03a"),
            Affix::Splitting => ("Splitting", "➗", "#c0ff6a"),
            Affix::Vampiric => ("Vampiric", "🦇", "#ff3a5a"),
            Affix::Volatile => ("Volatile", "💣", "#ff7a1f"),
            Affix::Reflective => ("Reflective", "🪞", "#e0e0ff"),
        }
    }
}

impl Game {
    /// Chance that a wave spawn is an elite.
    pub fn elite_chance(&self) -> f32 {
        let n = self.scale_n();
        if n < 4 {
            0.0
        } else {
            (0.015 + 0.004 * n as f32).min(0.12)
        }
    }

    /// Turn the most recently spawned zombie into an elite.
    pub fn make_elite(&mut self, affix: Affix) {
        let Some(z) = self.zombies.last_mut() else { return };
        if affix == Affix::None {
            return;
        }
        z.affix = affix;
        let boss = z.kind == ZKind::Boss;
        if !boss {
            z.hp *= 3.0;
            z.max_hp = z.hp;
            z.r *= 1.22;
            z.xp *= 4;
        }
        match affix {
            Affix::Shielded => z.shield = z.max_hp * 0.6,
            Affix::Hasted => z.speed *= if boss { 1.25 } else { 1.6 },
            _ => {}
        }
    }

    pub fn roll_affix(&mut self) -> Affix {
        AFFIXES[self.rng.below(AFFIXES.len())]
    }

    /// Damage passes through a shield first. Returns the damage left over.
    pub fn shield_absorb(&mut self, j: usize, dmg: f32) -> f32 {
        let z = &mut self.zombies[j];
        if z.shield <= 0.0 {
            return dmg;
        }
        let soaked = dmg.min(z.shield);
        z.shield -= soaked;
        if z.shield <= 0.0 {
            let p = z.pos;
            self.ring(p, 50.0, 0.3, "#6cc8ff", 5.0);
            self.burst(p, 14, 260.0, "#9fdcff", 3.0, 0.4, false, true);
            self.push_sfx(Sfx::Crit, p);
            self.push_text(p + v2(0.0, -26.0), "SHIELD BROKEN".into(), "#9fdcff", 13.0);
        }
        dmg - soaked
    }

    /// A reflective elite was shot: sometimes the round comes back as spit.
    pub fn reflect_shot(&mut self, j: usize, from: V2) {
        if self.zombies[j].affix != Affix::Reflective || !self.rng.chance(0.25) {
            return;
        }
        let z = &self.zombies[j];
        let dir = (from - z.pos).norm();
        self.eshots.push(EShot {
            pos: z.pos + dir * z.r,
            prev: z.pos,
            vel: dir * 380.0,
            life: 1.6,
            dmg: z.dmg * 0.5,
            dead: false,
            turn: 0.0,
            r: 6.5,
            grazed: false,
                delay: 0.0,
        });
    }

    /// Per-step elite upkeep (host): vampiric regeneration.
    pub fn update_elites(&mut self) {
        for z in self.zombies.iter_mut() {
            if !z.dead && z.affix == Affix::Vampiric && z.hp < z.max_hp {
                z.hp = (z.hp + z.max_hp * 0.025 * DT).min(z.max_hp);
            }
        }
    }

    /// A vampiric elite bit a player: it heals.
    pub fn elite_bite(&mut self, j: usize, dmg: f32) {
        let z = &mut self.zombies[j];
        if z.affix == Affix::Vampiric {
            z.hp = (z.hp + dmg * 2.0).min(z.max_hp);
            let p = z.pos;
            self.push_text(p, format!("+{}", (dmg * 2.0) as i32), "#ff3a5a", 13.0);
        }
    }

    /// Elite death effects (host).
    pub fn elite_death(&mut self, pos: V2, kind: ZKind, affix: Affix, r: f32) {
        match affix {
            Affix::Splitting => {
                let child = if kind == ZKind::Brute { ZKind::Walker } else { ZKind::Runner };
                for k in 0..3 {
                    let p = pos + V2::from_angle(k as f32 * 2.1) * (r * 0.8);
                    self.spawn_zombie(child, p);
                    if let Some(z) = self.zombies.last_mut() {
                        z.hp *= 0.5;
                        z.max_hp = z.hp;
                        z.r *= 0.8;
                        z.vel = (p - pos).norm() * 260.0;
                    }
                }
                self.burst(pos, 20, 260.0, "#c0ff6a", 3.5, 0.6, true, false);
            }
            Affix::Volatile => {
                let dmg = 30.0 * self.difficulty().dmg * (1.0 + 0.05 * self.wave.n as f32);
                self.volatile.push((pos, 0.7, dmg));
                self.ring(pos, 130.0, 0.7, "#ff7a1f", 4.0);
            }
            _ => {}
        }
        if affix != Affix::None && self.rng.chance(0.3) {
            self.pickups.push(crate::game::Pickup {
                pos: pos + v2(0.0, 18.0),
                kind: crate::game::PickKind::Chest,
                t: 0.0,
                dead: false,
            });
        }
    }

    /// Volatile elites' delayed blasts.
    pub fn update_volatile(&mut self) {
        if self.volatile.is_empty() {
            return;
        }
        let mut boom = Vec::new();
        for v in self.volatile.iter_mut() {
            v.1 -= DT;
            if v.1 <= 0.0 {
                boom.push((v.0, v.2));
            }
        }
        self.volatile.retain(|v| v.1 > 0.0);
        for (p, dmg) in boom {
            self.explode(p, 130.0, dmg, true);
        }
    }
}
