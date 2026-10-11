//! Death presentation: corpses that topple and fade instead of popping, gibs,
//! and elemental deaths (ash for fire, shards for ice, sparks for shock, goo
//! for poison).

use crate::content::BossVar;
use crate::game::{Game, Particle, ZKind};
use crate::math::*;
use crate::status::{Elem, FROZEN_BIT};

pub struct Corpse {
    pub pos: V2,
    pub vel: V2,
    pub face: f32,
    pub r: f32,
    pub kind: ZKind,
    pub boss_var: BossVar,
    pub tint: f32,
    /// seconds since death
    pub t: f32,
    /// status bits at death (decides the death style)
    pub elem: u8,
}

pub const CORPSE_T: f32 = 1.6;

impl Game {
    /// Leave a corpse for zombie `j` and play its elemental death.
    pub fn add_corpse(&mut self, j: usize) {
        let z = &self.zombies[j];
        if z.kind == ZKind::Bat {
            return; // bats just burst
        }
        let elem = z.st.bits();
        let c = Corpse {
            pos: z.pos,
            vel: z.vel * 0.6,
            face: z.face,
            r: z.r,
            kind: z.kind,
            boss_var: z.boss_var,
            tint: z.tint,
            t: 0.0,
            elem,
        };
        let (pos, r) = (z.pos, z.r);
        if self.corpses.len() >= 70 {
            self.corpses.remove(0);
        }
        self.corpses.push(c);
        self.death_style(pos, r, elem);
    }

    fn spray(&mut self, pos: V2, n: usize, speed: f32, color: &'static str, size: f32, life: f32, grav: bool, glow: bool, rise: f32) {
        for _ in 0..n {
            if self.particles.len() >= 1800 {
                return;
            }
            let dir = self.rng.unit();
            let l = life * self.rng.range(0.7, 1.2);
            self.particles.push(Particle {
                pos: pos + dir * self.rng.range(0.0, 8.0),
                vel: dir * speed * self.rng.range(0.3, 1.0) + v2(0.0, -rise),
                z: if grav { self.rng.range(6.0, 20.0) } else { 0.0 },
                vz: if grav { self.rng.range(120.0, 320.0) } else { 0.0 },
                life: l,
                max: l,
                size: size * self.rng.range(0.6, 1.3),
                color,
                grav,
                glow,
                drag: if grav { 1.4 } else { 2.5 },
            });
        }
    }

    fn death_style(&mut self, pos: V2, r: f32, elem: u8) {
        if elem & FROZEN_BIT != 0 || elem & Elem::Chill.bit() != 0 {
            self.spray(pos, 12, 300.0, "#d8fbff", 3.5, 0.8, true, false, 0.0);
            self.spray(pos, 6, 200.0, "#8fe8ff", 2.5, 0.5, false, true, 0.0);
        } else if elem & Elem::Burn.bit() != 0 {
            self.spray(pos, 10, 60.0, "#3a3a3a", 4.0, 1.2, false, false, 60.0);
            self.spray(pos, 8, 120.0, "#ff9a3a", 2.0, 0.6, false, true, 80.0);
        } else if elem & Elem::Shock.bit() != 0 {
            self.spray(pos, 14, 380.0, "#cfe0ff", 1.8, 0.3, false, true, 0.0);
        } else if elem & Elem::Poison.bit() != 0 {
            self.spray(pos, 12, 180.0, "#7dff3a", 3.0, 0.7, true, false, 0.0);
        } else if self.rng.chance(0.35) {
            // gibs: a couple of meaty chunks
            self.spray(pos, 2 + (r / 15.0) as usize, 300.0, "#7a2a20", 5.5, 1.0, true, false, 0.0);
        }
    }

    pub fn update_corpses(&mut self, dt: f32) {
        for c in self.corpses.iter_mut() {
            c.t += dt;
            c.pos += c.vel * dt;
            c.vel = c.vel * (1.0 - damp(6.0, dt));
        }
        self.corpses.retain(|c| c.t < CORPSE_T);
    }
}
