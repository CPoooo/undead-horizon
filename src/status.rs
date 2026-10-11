//! Elemental status effects on zombies, and their combos.
//!
//! - **Burn**: damage over time, refreshed by more fire.
//! - **Chill**: builds up and slows; at full chill the zombie freezes solid.
//! - **Shock**: shocked zombies take +25% damage and arc lightning when they die.
//! - **Poison**: stacks up to 10, each stack ticking damage.
//!
//! Combos: chill or freeze + shock = **Shatter** (a burst of damage), burn +
//! poison = **Blight** (a toxic blast that spreads poison).
//!
//! Statuses live on the host (it owns the zombies). Clients send element bits
//! with their hits and see statuses through four bits of the snapshot flags.

use crate::audio::Sfx;
use crate::game::{Game, ZKind, DT};
use crate::math::*;
use crate::net::Hit;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Elem {
    Burn,
    Chill,
    Shock,
    Poison,
}

impl Elem {
    /// Bit in hit flags and snapshot flags.
    pub fn bit(self) -> u8 {
        match self {
            Elem::Chill => 1 << 1,
            Elem::Burn => 1 << 4,
            Elem::Shock => 1 << 5,
            Elem::Poison => 1 << 6,
        }
    }
}

/// Snapshot bit for a frozen zombie.
pub const FROZEN_BIT: u8 = 1 << 7;

#[derive(Clone, Copy, Default, Debug)]
pub struct Status {
    pub burn_t: f32,
    pub burn_dps: f32,
    /// 0..1 build-up; slows, freezes at 1
    pub chill: f32,
    pub freeze_t: f32,
    pub shock_t: f32,
    pub poison: u8,
    pub poison_t: f32,
    /// damage-over-time tick timer
    pub dot_t: f32,
}

impl Status {
    pub fn speed_mult(&self) -> f32 {
        if self.freeze_t > 0.0 {
            0.0
        } else {
            1.0 - 0.55 * self.chill.min(1.0)
        }
    }
    pub fn frozen(&self) -> bool {
        self.freeze_t > 0.0
    }
    /// Snapshot bits (shares the byte with flash/state; see coop.rs).
    pub fn bits(&self) -> u8 {
        let mut b = 0;
        if self.chill > 0.05 {
            b |= Elem::Chill.bit();
        }
        if self.burn_t > 0.0 {
            b |= Elem::Burn.bit();
        }
        if self.shock_t > 0.0 {
            b |= Elem::Shock.bit();
        }
        if self.poison > 0 {
            b |= Elem::Poison.bit();
        }
        if self.freeze_t > 0.0 {
            b |= FROZEN_BIT;
        }
        b
    }
    /// Replica statuses from snapshot bits (visual only).
    pub fn from_bits(b: u8) -> Status {
        Status {
            chill: if b & Elem::Chill.bit() != 0 { 0.5 } else { 0.0 },
            burn_t: if b & Elem::Burn.bit() != 0 { 0.5 } else { 0.0 },
            shock_t: if b & Elem::Shock.bit() != 0 { 0.5 } else { 0.0 },
            poison: (b & Elem::Poison.bit() != 0) as u8,
            freeze_t: if b & FROZEN_BIT != 0 { 0.5 } else { 0.0 },
            ..Status::default()
        }
    }
}

impl Game {
    /// Base status strength, scaled by wave and damage upgrades.
    pub fn elem_power(&self) -> f32 {
        (6.0 + 1.2 * self.wave.n as f32) * self.dmg_mult().sqrt()
    }

    /// Apply an element to zombie `j`. Clients forward it to the host.
    pub fn afflict(&mut self, j: usize, e: Elem, power: f32) {
        if self.is_client() {
            let z = &self.zombies[j];
            if let Some(net) = &mut self.net {
                net.out_hits.push(Hit {
                    uid: z.uid,
                    dmg: 0.0,
                    dir: V2::ZERO,
                    knock: 0.0,
                    crit: false,
                    fx: e.bit(),
                });
            }
            return;
        }
        let boss = self.zombies[j].kind == ZKind::Boss;
        let pos = self.zombies[j].pos;
        let mut combo = None;
        {
            let st = &mut self.zombies[j].st;
            match e {
                Elem::Burn => {
                    st.burn_t = 3.0;
                    st.burn_dps = st.burn_dps.max(power);
                    if st.poison > 0 {
                        combo = Some("BLIGHT");
                    }
                }
                Elem::Chill => {
                    if st.shock_t > 0.0 {
                        combo = Some("SHATTER");
                    } else if st.freeze_t <= 0.0 {
                        st.chill += if boss { 0.12 } else { 0.5 };
                        if st.chill >= 1.0 {
                            st.chill = 0.0;
                            st.freeze_t = if boss { 0.6 } else { 1.6 };
                        }
                    }
                }
                Elem::Shock => {
                    if st.freeze_t > 0.0 || st.chill > 0.4 {
                        combo = Some("SHATTER");
                    }
                    st.shock_t = 2.5;
                }
                Elem::Poison => {
                    st.poison = (st.poison + 1).min(10);
                    st.poison_t = 4.0;
                    if st.burn_t > 0.0 {
                        combo = Some("BLIGHT");
                    }
                }
            }
        }
        match combo {
            Some("SHATTER") => self.shatter(j, pos),
            Some("BLIGHT") => self.blight(j, pos),
            _ => {}
        }
    }

    fn shatter(&mut self, j: usize, pos: V2) {
        let z = &mut self.zombies[j];
        z.st.freeze_t = 0.0;
        z.st.chill = 0.0;
        z.st.shock_t = 0.0;
        let boss = z.kind == ZKind::Boss;
        let dmg = z.max_hp * if boss { 0.04 } else { 0.35 } + self.elem_power() * 3.0;
        self.push_text(pos + v2(0.0, -24.0), "SHATTER".into(), "#bff4ff", 16.0);
        self.ring(pos, 70.0, 0.3, "#d8fbff", 6.0);
        self.burst(pos, 18, 320.0, "#d8fbff", 3.0, 0.5, true, true);
        self.push_sfx(Sfx::Crit, pos);
        self.damage_zombie(j, dmg, v2(0.0, 0.0), 0.0, true);
    }

    fn blight(&mut self, j: usize, pos: V2) {
        self.zombies[j].st.burn_t = 0.0;
        let power = self.elem_power();
        self.push_text(pos + v2(0.0, -24.0), "BLIGHT".into(), "#b6ff5a", 16.0);
        self.ring(pos, 110.0, 0.4, "#9bff4a", 8.0);
        self.burst(pos, 22, 260.0, "#8fd43a", 4.0, 0.7, false, true);
        self.push_sfx(Sfx::Spit, pos);
        let mut hits = Vec::new();
        self.grid.query(pos, 140.0, |k| hits.push(k));
        for k in hits {
            let z = &mut self.zombies[k];
            if z.dead || z.pos.dist(pos) > 110.0 + z.r {
                continue;
            }
            z.st.poison = (z.st.poison + 3).min(10);
            z.st.poison_t = 4.0;
            let dir = (z.pos - pos).norm();
            self.damage_zombie(k, power * 2.0, dir, 120.0, false);
        }
    }

    /// Tick statuses: decay, damage over time. Host and solo only.
    pub fn update_status(&mut self) {
        let mut dots: Vec<(usize, f32, &'static str)> = Vec::new();
        for (i, z) in self.zombies.iter_mut().enumerate() {
            if z.dead {
                continue;
            }
            let st = &mut z.st;
            st.burn_t -= DT;
            st.freeze_t -= DT;
            st.shock_t -= DT;
            st.chill = (st.chill - DT * 0.25).max(0.0);
            if st.poison > 0 {
                st.poison_t -= DT;
                if st.poison_t <= 0.0 {
                    st.poison = 0;
                }
            }
            st.dot_t -= DT;
            if st.dot_t <= 0.0 {
                st.dot_t = 0.5;
                let mut d = 0.0;
                if st.burn_t > 0.0 {
                    d += st.burn_dps * 0.5;
                }
                if st.poison > 0 {
                    d += st.poison as f32 * 1.6 * 0.5 * (1.0 + 0.05 * self.wave.n as f32);
                }
                if d > 0.0 {
                    let col = if st.burn_t > 0.0 { "#ff9a3a" } else { "#9be04a" };
                    dots.push((i, d, col));
                }
            }
        }
        for (i, d, col) in dots {
            if self.zombies[i].dead {
                continue;
            }
            self.dot_damage(i, d, col);
        }
    }

    /// Damage over time: no knockback, small colored numbers, quiet.
    fn dot_damage(&mut self, j: usize, dmg: f32, col: &'static str) {
        let pos = self.zombies[j].pos;
        let z = &mut self.zombies[j];
        z.hp -= dmg;
        z.last_hitter = z.last_hitter.min(250);
        self.damage_dealt += dmg as f64;
        self.push_text(pos, format!("{}", dmg.round() as i32), col, 11.0);
        if self.zombies[j].hp <= 0.0 {
            self.kill_zombie(j);
        }
    }

    /// Shocked zombies take extra damage.
    pub fn status_damage_mult(&self, j: usize) -> f32 {
        if self.zombies[j].st.shock_t > 0.0 {
            1.25
        } else {
            1.0
        }
    }

    /// A shocked zombie died: lightning arcs to two neighbours.
    pub fn shock_discharge(&mut self, pos: V2, uid: u32) {
        let mut targets: Vec<(f32, usize)> = Vec::new();
        self.grid.query(pos, 180.0, |k| {
            let z = &self.zombies[k];
            if !z.dead && z.uid != uid {
                targets.push((z.pos.dist2(pos), k));
            }
        });
        targets.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        let dmg = self.elem_power() * 2.0;
        let mut pts = vec![pos];
        for (_, k) in targets.into_iter().take(2) {
            let zp = self.zombies[k].pos;
            pts.push(zp);
            let dir = (zp - pos).norm();
            self.damage_zombie(k, dmg, dir, 80.0, false);
        }
        if pts.len() > 1 {
            self.bolts.push(crate::game::Bolt { pts, life: 0.18 });
        }
    }
}
