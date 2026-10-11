//! The horde wave engine shared by Infinite and Campaign: wave sizing, the
//! spawn trickle, boss waves and wave-clear rewards.

use crate::audio::Sfx;
use crate::content::{self, BossVar, CHAPTERS};
use crate::game::*;
use crate::math::*;
use crate::net;
use std::f32::consts::PI;

impl Game {
    pub(crate) fn is_boss_wave(&self, n: u32) -> bool {
        if self.is_campaign() {
            n == CHAPTERS[self.chapter()].waves
        } else {
            n % 5 == 0
        }
    }
    pub(crate) fn start_wave(&mut self, n: u32) {
        self.wave.n = n;
        let lvl = self.scale_n();
        let w = (lvl - 1) as f32;
        let base_total = 12.0 + 7.0 * w + w.powf(1.45);
        let camp = if self.is_campaign() { 0.85 } else { 1.0 };
        self.wave.total = (base_total * self.difficulty().count * camp) as u32;
        self.wave.spawned = 0;
        self.wave.timer = 0.6;
        let boss = self.is_boss_wave(n);
        self.wave.boss_pending = boss;
        self.wave.boss_timer = 2.5;
        let new_record = !self.is_campaign() && self.best_wave > 0 && n > self.best_wave;
        let (title, sub) = if self.is_campaign() {
            let ch = &CHAPTERS[self.chapter()];
            let t = if boss {
                format!("FINAL WAVE {}/{}", n, ch.waves)
            } else {
                format!("WAVE {}/{}", n, ch.waves)
            };
            let line = ch.radio.get((n - 1) as usize).copied().unwrap_or("");
            (t, line.to_string())
        } else {
            (
                if boss {
                    format!("WAVE {} — BOSS", n)
                } else {
                    format!("WAVE {}", n)
                },
                if new_record {
                    "NEW PERSONAL BEST!".into()
                } else {
                    format!(
                        "{} undead approach",
                        self.wave.total + if boss { 1 } else { 0 }
                    )
                },
            )
        };
        self.banner = Some(Banner {
            title,
            sub,
            t: 0.0,
            dur: if self.is_campaign() { 3.6 } else { 2.4 },
            color: if boss { "#ff6a8a" } else { "#e8f0ff" },
        });
        self.sfx(Sfx::WaveStart, self.player.pos);
        if let Some(net) = &mut self.net {
            let total = self.wave.total;
            net.event(|w| {
                w.u8(net::E_WAVE_START);
                w.u16(n.min(u16::MAX as u32) as u16);
                w.u16((total + boss as u32).min(u16::MAX as u32) as u16);
                w.u8(boss as u8);
            });
        }
    }
    pub(crate) fn spawn_boss(&mut self, var: BossVar, pos: V2) {
        self.spawn_zombie(ZKind::Boss, pos);
        let (hm, r, sm) = var.stats();
        if let Some(z) = self.zombies.last_mut() {
            z.boss_var = var;
            z.hp *= hm;
            z.max_hp = z.hp;
            z.r = r;
            z.speed *= sm;
            z.special_t = 2.5;
            if var == BossVar::BloomHeart {
                z.xp *= 3;
            }
        }
        if self.scale_n() >= 15 {
            let a = self.roll_affix();
            self.make_elite(a);
        }
    }
    pub(crate) fn campaign_victory(&mut self) {
        if self.victory_t >= 0.0 {
            return;
        }
        self.victory_t = 0.0;
        let bonus = (100.0 * (self.chapter() + 1) as f32 * self.difficulty().coins) as u32;
        self.coins += bonus;
        // the horde collapses with its master
        for z in self.zombies.iter_mut() {
            z.dead = true;
        }
        for g in self.gems.iter_mut() {
            g.magnet = true;
        }
        self.eshots.clear();
        let p = self.player.pos;
        self.ring(p, 900.0, 1.2, "#ffd36a", 14.0);
        self.burst(p, 60, 700.0, "#ffd36a", 4.0, 1.0, false, true);
        self.banner = Some(Banner {
            title: "CHAPTER COMPLETE".into(),
            sub: format!("{} · +{} coins", CHAPTERS[self.chapter()].title, bonus),
            t: 0.0,
            dur: 3.2,
            color: "#ffd36a",
        });
        self.audio.play(Sfx::WaveClear);
        self.audio.play(Sfx::LevelUp);
    }
    pub(crate) fn horde_waves(&mut self) {
        if self.victory_t >= 0.0 {
            return;
        }
        if self.wave.intermission > 0.0 {
            self.wave.intermission -= DT;
            if self.wave.intermission <= 0.0 {
                let n = self.wave.n + 1;
                self.start_wave(n);
            }
            return;
        }
        let n = self.wave.n;
        let lvl = self.scale_n();
        let max_alive = (150 + 8 * lvl).min(340);
        let alive = self.alive();
        self.wave.timer -= DT;
        if self.wave.spawned < self.wave.total && self.wave.timer <= 0.0 && alive < max_alive {
            let interval = (1.1 - lvl as f32 * 0.045).max(0.22) / self.difficulty().count;
            self.wave.timer = interval * self.rng.range(0.7, 1.3);
            let gmax = 3 + lvl / 2;
            let g =
                (2 + self.rng.below(gmax as usize) as u32).min(self.wave.total - self.wave.spawned);
            let targets = self.targets();
            let (_, ppos, pv) = targets[self.rng.below(targets.len())];
            let a = if pv.len2() > 900.0 && self.rng.chance(0.4) {
                pv.angle() + self.rng.range(-0.7, 0.7)
            } else {
                self.rng.angle()
            };
            let base = ppos + V2::from_angle(a) * self.spawn_r;
            for _ in 0..g {
                let k = self.pick_kind();
                let p = base + self.rng.unit() * self.rng.range(0.0, 70.0);
                self.spawn_zombie(k, p);
                if k == ZKind::Bat {
                    for _ in 0..4 {
                        let q = p + self.rng.unit() * 40.0;
                        self.spawn_zombie(ZKind::Bat, q);
                    }
                }
                if self.rng.chance(self.elite_chance()) {
                    let a = self.roll_affix();
                    self.make_elite(a);
                }
            }
            self.wave.spawned += g;
        }
        if self.wave.boss_pending {
            self.wave.boss_timer -= DT;
            if self.wave.boss_timer <= 0.0 {
                self.wave.boss_pending = false;
                let (var, twin) = if self.is_campaign() {
                    let ch = &CHAPTERS[self.chapter()];
                    (ch.boss, ch.twin)
                } else {
                    let i = ((n / 5).max(1) - 1) as usize % content::INFINITE_BOSSES.len();
                    (content::INFINITE_BOSSES[i], false)
                };
                let a = self.rng.angle();
                let center = self.targets()[0].1;
                let p = center + V2::from_angle(a) * (self.spawn_r - 40.0);
                self.spawn_boss(var, p);
                if let Some(net) = &mut self.net {
                    net.event(|w| {
                        w.u8(net::E_BOSS);
                        w.u8(var as u8);
                    });
                }
                if twin {
                    let p2 = center + V2::from_angle(a + PI) * (self.spawn_r - 40.0);
                    self.spawn_boss(var, p2);
                }
                self.toast = Some((var.name().to_string(), 2.5));
                self.zoom_punch = 0.08;
                self.sfx(Sfx::BossRoar, self.player.pos);
                self.add_shake(0.5);
            }
        }
        if self.wave.spawned >= self.wave.total && !self.wave.boss_pending && alive == 0 && n > 0 {
            self.wave_cleared();
        } else if n == 0 {
            self.wave.intermission = 0.01;
        }
    }
    pub(crate) fn wave_cleared(&mut self) {
        self.wave.intermission = 3.5;
        for g in self.gems.iter_mut() {
            g.magnet = true;
        }
        let heal = self.player.max_hp * 0.1;
        if self.player_alive() {
            self.player.hp = (self.player.hp + heal).min(self.player.max_hp);
        }
        let earned = ((10 + 5 * self.scale_n()) as f32 * self.difficulty().coins) as u32;
        self.score += 100 * self.wave.n as u64;
        self.coins += earned;
        if let Some(net) = &mut self.net {
            let n = self.wave.n;
            net.event(|w| {
                w.u8(net::E_WAVE_CLEAR);
                w.u16(n.min(u16::MAX as u32) as u16);
                w.u16(earned.min(u16::MAX as u32) as u16);
            });
        }
        self.banner = Some(Banner {
            title: format!("WAVE {} CLEARED", self.wave.n),
            sub: format!(
                "+{} coins · +{} HP · next wave incoming",
                earned, heal as i32
            ),
            t: 0.0,
            dur: 2.6,
            color: "#9cffb8",
        });
        self.sfx(Sfx::WaveClear, self.player.pos);
        self.ui_out.push(("wave", format!("{}", self.wave.n)));
    }
}
