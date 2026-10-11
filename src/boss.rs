//! Boss 2.0: phases, enrage timers, weak points, telegraphed ground slams,
//! the Butcher's hook, the Hive Queen's spores, and the intro card.

use crate::audio::Sfx;
use crate::content::BossVar;
use crate::game::{Banner, EShot, Game, ZKind, DT};
use crate::math::*;
use crate::net;

/// A telegraphed area attack: a growing red circle, then damage.
pub struct Slam {
    pub pos: V2,
    pub r: f32,
    pub t: f32,
    pub dur: f32,
    pub dmg: f32,
    pub fire: bool,
}

pub struct BossIntro {
    pub var: BossVar,
    pub t: f32,
}

pub const INTRO_T: f32 = 3.0;
/// Seconds before a boss enrages.
pub const ENRAGE_T: f32 = 150.0;

impl Game {
    pub fn boss_intro(&mut self, var: BossVar) {
        self.boss_intro = Some(BossIntro { var, t: 0.0 });
        self.banner = None;
        self.zoom_punch = self.zoom_punch.max(0.1);
    }

    /// Hits on a boss's back (the glowing weak point) deal double and crit.
    pub fn weak_point(&self, j: usize, dir: V2) -> bool {
        let z = &self.zombies[j];
        z.kind == ZKind::Boss && dir.len2() > 0.01 && dir.dot(V2::from_angle(z.face)) > 0.55
    }

    /// Phase changes, enrage, slams and signature moves (host).
    pub fn update_bosses(&mut self) {
        let targets = self.targets();
        let mut events: Vec<(usize, u8)> = Vec::new();
        let mut slams: Vec<Slam> = Vec::new();
        let mut hooks: Vec<(V2, V2, f32)> = Vec::new();
        let mut spores: Vec<(V2, V2, f32)> = Vec::new();
        let mut swarms: Vec<V2> = Vec::new();
        let playing = self.phase == crate::game::Phase::Playing || self.net.is_some();
        for (i, z) in self.zombies.iter_mut().enumerate() {
            if z.dead || z.kind != ZKind::Boss {
                continue;
            }
            z.age += DT;
            let frac = z.hp / z.max_hp.max(1.0);
            let want = if frac < 0.33 { 3 } else if frac < 0.66 { 2 } else { 1 };
            if z.phase == 0 {
                z.phase = 1;
            }
            if want > z.phase {
                z.phase = want;
                events.push((i, want));
            }
            if z.age > ENRAGE_T && z.rage_t < 1000.0 {
                z.rage_t = 1.0e6;
                z.dmg *= 1.5;
                events.push((i, 9));
            }
            if !playing {
                continue;
            }
            let (_, tpos, _) = Game::nearest_target(&targets, z.pos);
            let dist = tpos.dist(z.pos);
            // slams from phase 2 (ranged bosses keep their own patterns)
            if z.phase >= 2 && !matches!(z.boss_var, BossVar::Matriarch | BossVar::HiveQueen) {
                z.fuse -= DT;
                if z.fuse <= 0.0 && dist < 520.0 {
                    z.fuse = if z.phase >= 3 { 4.0 } else { 6.5 };
                    let fire = z.boss_var == BossVar::CinderKing;
                    slams.push(Slam {
                        pos: tpos,
                        r: if z.phase >= 3 { 150.0 } else { 120.0 },
                        t: 0.0,
                        dur: 1.1,
                        dmg: z.dmg * 1.1,
                        fire,
                    });
                }
            }
            match z.boss_var {
                BossVar::Butcher => {
                    // hook: aim (state 3 = winding), then yank whoever is on the line
                    z.special_t -= DT;
                    if z.state == 0 && z.special_t <= 0.0 && dist < 460.0 && dist > 120.0 {
                        z.state = 3;
                        z.timer = 0.8;
                        z.charge_dir = (tpos - z.pos).norm();
                    }
                    if z.state == 3 {
                        z.timer -= DT;
                        if z.timer <= 0.0 {
                            z.state = 0;
                            z.timer = 2.0;
                            z.special_t = if z.phase >= 3 { 3.0 } else { 5.0 };
                            hooks.push((z.pos, z.charge_dir, z.dmg * 0.6));
                        }
                    }
                }
                BossVar::HiveQueen => {
                    z.special_t -= DT;
                    if z.special_t <= 0.0 {
                        z.special_t = if z.phase >= 3 { 1.6 } else { 2.6 };
                        let n = 2 + z.phase as usize;
                        for k in 0..n {
                            let a = (tpos - z.pos).angle() + (k as f32 - (n - 1) as f32 * 0.5) * 0.35;
                            spores.push((z.pos, V2::from_angle(a), z.dmg * 0.45));
                        }
                    }
                    z.timer -= DT;
                    if z.timer <= 0.0 {
                        z.timer = if z.phase >= 2 { 7.0 } else { 10.0 };
                        swarms.push(z.pos);
                    }
                }
                _ => {}
            }
        }
        for (i, ev) in events {
            let pos = self.zombies[i].pos;
            let name = self.zombies[i].boss_var.name();
            if ev == 9 {
                self.banner = Some(Banner {
                    title: "ENRAGED".into(),
                    sub: format!("{} has had enough", name),
                    t: 0.0,
                    dur: 2.0,
                    color: "#ff3a3a",
                });
                self.sfx(Sfx::BossRoar, pos);
                continue;
            }
            // phase shift: roar, shockwave, shield, adds
            let max = self.zombies[i].max_hp;
            self.zombies[i].shield = max * 0.08;
            self.banner = Some(Banner {
                title: format!("PHASE {}", ev),
                sub: name.to_string(),
                t: 0.0,
                dur: 1.8,
                color: "#ff9cff",
            });
            self.sfx(Sfx::BossRoar, pos);
            self.add_shake(0.6);
            self.zoom_punch = self.zoom_punch.max(0.06);
            self.ring(pos, 320.0, 0.5, "#ff9cff", 10.0);
            self.hurt_players_in(pos, 140.0, self.zombies[i].dmg * 0.5);
            let add = match self.zombies[i].boss_var {
                BossVar::HiveQueen => ZKind::Bat,
                BossVar::CinderKing => ZKind::Exploder,
                BossVar::Matriarch => ZKind::Spitter,
                _ => ZKind::Runner,
            };
            for k in 0..(2 + ev as usize * 2) {
                let p = pos + V2::from_angle(k as f32 * 1.3) * 110.0;
                self.spawn_zombie(add, p);
            }
        }
        for s in slams {
            self.slams.push(s);
        }
        for (from, dir, dmg) in hooks {
            self.butcher_hook(from, dir, dmg);
        }
        for (from, dir, dmg) in spores {
            self.eshots.push(EShot {
                pos: from + dir * 40.0,
                prev: from,
                vel: dir * 210.0,
                life: 4.0,
                dmg,
                dead: false,
                turn: 1.6,
                r: 8.0,
                grazed: false,
            });
        }
        for at in swarms {
            for k in 0..6 {
                let p = at + V2::from_angle(k as f32 * 1.05) * 70.0;
                self.spawn_zombie(ZKind::Bat, p);
            }
            self.sfx(Sfx::Groan, at);
        }
        self.update_slams();
    }

    fn update_slams(&mut self) {
        if self.slams.is_empty() {
            return;
        }
        let mut hits = Vec::new();
        for s in self.slams.iter_mut() {
            s.t += DT;
            if s.t >= s.dur {
                hits.push((s.pos, s.r, s.dmg, s.fire));
            }
        }
        self.slams.retain(|s| s.t < s.dur);
        for (pos, r, dmg, fire) in hits {
            self.hurt_players_in(pos, r, dmg);
            self.ring(pos, r, 0.3, if fire { "#ff7a1f" } else { "#ff4060" }, 10.0);
            self.burst(pos, 30, 420.0, if fire { "#ffb24a" } else { "#6a4a3a" }, 5.0, 0.6, true, false);
            self.light(pos, r * 2.0, 0.3, if fire { "#ff8040" } else { "#ff6070" });
            self.sfx(Sfx::Explosion, pos);
            self.add_shake(0.35);
        }
    }

    /// The Butcher's hook: anyone on the line takes damage and is dragged in.
    pub(crate) fn butcher_hook(&mut self, from: V2, dir: V2, dmg: f32) {
        let len = 460.0;
        let end = from + dir * len;
        self.bolts.push(crate::game::Bolt { pts: vec![from, end], life: 0.2 });
        self.sfx(Sfx::Blade, from);
        for (slot, ppos, _) in self.targets() {
            if slot == 255 {
                continue;
            }
            let along = (ppos - from).dot(dir).clamp(0.0, len);
            if (from + dir * along).dist(ppos) > 34.0 {
                continue;
            }
            let pull = (from - ppos).norm() * 1100.0;
            if slot == self.my_slot() {
                if self.player.iframes <= 0.0 && self.player.dash_t <= 0.0 {
                    self.player.vel = pull;
                    self.hurt_player(dmg, from);
                }
            } else {
                self.hurt_slot(slot, dmg, from);
                if let Some(n) = &mut self.net {
                    n.event(|w| {
                        w.u8(net::E_PULL);
                        w.u8(slot);
                        w.v2(pull);
                    });
                }
            }
        }
    }

    pub fn tick_boss_intro(&mut self, dt: f32) {
        if let Some(b) = &mut self.boss_intro {
            b.t += dt;
            if b.t > INTRO_T {
                self.boss_intro = None;
            }
        }
    }
}
