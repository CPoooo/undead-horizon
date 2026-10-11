//! Character kits: each survivor has a passive and an active ability on a
//! cooldown (Q by default). Gunner: Heat Sink. Engineer: Sentry Turret.
//! Medic: Heal Pulse. Pyro: Flame Cone.

use crate::audio::Sfx;
use crate::content::{Kit, CHARACTERS, GUNS};
use crate::game::{Bullet, Game, DT};
use crate::math::*;
use crate::net;
use crate::status::Elem;

pub struct Turret {
    pub pos: V2,
    pub life: f32,
    pub aim: f32,
    pub cd: f32,
}

pub const HEAT_T: f32 = 5.0;
pub const FLAME_T: f32 = 2.6;
pub const TURRET_T: f32 = 16.0;

impl Game {
    pub fn kit(&self) -> Kit {
        CHARACTERS[self.character].kit
    }

    /// +15% damage with the character's signature gun.
    pub fn signature_mult(&self) -> f32 {
        if CHARACTERS[self.character].signature == GUNS[self.gun].id {
            1.15
        } else {
            1.0
        }
    }

    pub fn try_ability(&mut self) -> bool {
        if self.ability_cd > 0.0 || !self.player_alive() {
            if self.ability_cd > 0.0 {
                self.toast = Some((format!("{} ready in {:.0}s", CHARACTERS[self.character].ability, self.ability_cd.ceil()), 0.9));
            }
            return true;
        }
        let ch = &CHARACTERS[self.character];
        self.ability_cd = ch.cd;
        let p = self.player.pos;
        match ch.kit {
            Kit::HeatSink => {
                self.ability_t = HEAT_T;
                self.ring(p, 120.0, 0.4, "#ffb03a", 6.0);
                self.sfx(Sfx::UltReady, p);
            }
            Kit::Turret => {
                let at = p + V2::from_angle(self.player.aim) * 40.0;
                let (at, _) = crate::world::resolve(at, 14.0);
                if self.turrets.len() >= 2 {
                    self.turrets.remove(0);
                }
                self.turrets.push(Turret { pos: at, life: TURRET_T, aim: self.player.aim, cd: 0.3 });
                self.burst(at, 12, 180.0, "#7ad7ff", 3.0, 0.4, false, true);
                self.sfx(Sfx::ReadyUp, at);
            }
            Kit::HealPulse => {
                let heal = self.player.max_hp * 0.35;
                self.player.hp = (self.player.hp + heal).min(self.player.max_hp);
                self.text(p, format!("+{}", heal as i32), "#9cffb8", 20.0);
                self.ring(p, 220.0, 0.6, "#9cffb8", 10.0);
                self.burst(p, 30, 300.0, "#9cffb8", 3.0, 0.6, false, true);
                self.light(p, 300.0, 0.5, "#9cffb8");
                self.sfx(Sfx::Heal, p);
                // shove zombies back (host owns them; clients push their replicas)
                for z in self.zombies.iter_mut() {
                    let d = z.pos - p;
                    let l = d.len();
                    if !z.dead && l < 220.0 && l > 0.1 {
                        z.vel += d * (1.0 / l) * 520.0;
                    }
                }
                // teammates in reach (host can tell everyone)
                if self.is_host() {
                    let mates: Vec<u8> = self
                        .targets()
                        .iter()
                        .filter(|t| t.0 != 255 && t.0 != self.my_slot() && t.1.dist(p) < 260.0)
                        .map(|t| t.0)
                        .collect();
                    if let Some(n) = &mut self.net {
                        for slot in mates {
                            n.event(|w| {
                                w.u8(net::E_HEAL);
                                w.u8(slot);
                                w.f32(heal);
                            });
                        }
                    }
                }
            }
            Kit::FlameCone => {
                self.ability_t = FLAME_T;
                self.sfx(Sfx::Explosion, p);
            }
        }
        true
    }

    /// Per-step ability upkeep (runs on every peer for its own player).
    pub fn update_abilities(&mut self) {
        self.ability_cd = (self.ability_cd - DT).max(0.0);
        self.ability_t = (self.ability_t - DT).max(0.0);
        let kit = self.kit();
        if kit == Kit::HealPulse && self.player_alive() {
            self.player.hp = (self.player.hp + 1.0 * DT).min(self.player.max_hp);
        }
        if kit == Kit::FlameCone && self.ability_t > 0.0 && self.player_alive() {
            self.flame_tick();
        }
        self.update_turrets();
    }

    fn flame_tick(&mut self) {
        let p = self.player.pos;
        let aim = self.player.aim;
        // visual stream
        for _ in 0..3 {
            if self.particles.len() < 1800 {
                let a = aim + self.rng.range(-0.35, 0.35);
                let l = self.rng.range(0.25, 0.45);
                self.particles.push(crate::game::Particle {
                    pos: p + V2::from_angle(aim) * 20.0,
                    vel: V2::from_angle(a) * self.rng.range(380.0, 560.0),
                    z: 6.0,
                    vz: 0.0,
                    life: l,
                    max: l,
                    size: self.rng.range(4.0, 8.0),
                    color: if self.rng.chance(0.5) { "#ff7a1f" } else { "#ffd23a" },
                    grav: false,
                    glow: true,
                    drag: 3.0,
                });
            }
        }
        if (self.time * 10.0) as i32 != ((self.time - DT) * 10.0) as i32 {
            self.light(p + V2::from_angle(aim) * 90.0, 200.0, 0.12, "#ff9040");
            let dmg = (6.0 + 1.5 * self.wave.n as f32) * self.dmg_mult();
            let power = self.elem_power() * 1.5;
            let mut hits = Vec::new();
            self.grid.query(p, 240.0, |j| hits.push(j));
            for j in hits {
                let z = &self.zombies[j];
                let d = z.pos - p;
                if z.dead || d.len() > 230.0 + z.r {
                    continue;
                }
                if wrap_angle(d.angle() - aim).abs() > 0.45 {
                    continue;
                }
                self.afflict(j, Elem::Burn, power);
                if !self.zombies[j].dead {
                    self.damage_zombie(j, dmg, d.norm(), 60.0, false);
                }
            }
        }
    }

    fn update_turrets(&mut self) {
        if self.turrets.is_empty() {
            return;
        }
        let dmg = 10.0 * 0.6 * self.dmg_mult();
        let mut shots: Vec<(V2, f32)> = Vec::new();
        for i in 0..self.turrets.len() {
            let tp = self.turrets[i].pos;
            let target = self.nearest_zombie(tp, 520.0, 0).map(|j| self.zombies[j].pos);
            let t = &mut self.turrets[i];
            t.life -= DT;
            t.cd -= DT;
            if let Some(zp) = target {
                let want = (zp - tp).angle();
                t.aim += wrap_angle(want - t.aim) * damp(12.0, DT);
                if t.cd <= 0.0 && wrap_angle(want - t.aim).abs() < 0.3 {
                    t.cd = 0.16;
                    shots.push((tp, t.aim));
                }
            }
        }
        self.turrets.retain(|t| t.life > 0.0);
        for (tp, a) in shots {
            let dir = V2::from_angle(a + self.rng.range(-0.04, 0.04));
            let start = tp + dir * 18.0;
            self.bullets.push(Bullet {
                pos: start,
                prev: start,
                vel: dir * 900.0,
                dmg,
                life: 0.6,
                pierce: 0,
                bounces: 0,
                r: 3.5,
                crit: false,
                last_hit: 0,
                dead: false,
                blast: 0.0,
                knock: 80.0,
                cosmetic: false,
                tracer: 2,
            });
            self.sfx(Sfx::Shoot, tp);
        }
    }

    /// Passive: Pyro hits can ignite.
    pub fn pyro_ignite(&mut self, j: usize) {
        if self.kit() == Kit::FlameCone && self.rng.chance(0.12) && !self.zombies[j].dead {
            let power = self.elem_power();
            self.afflict(j, Elem::Burn, power);
        }
    }
}
