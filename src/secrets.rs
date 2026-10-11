//! Outbreak extras: buyable traps, the riot-shield buildable, and the
//! Chapel easter egg (light three candles with the power on, then survive the
//! round: every perk, free).

use crate::audio::Sfx;
use crate::game::{Banner, Game, DT, PR};
use crate::math::*;
use crate::modes::outbreak::Perk;
use crate::status::Elem;
use crate::world;

pub struct Trap {
    /// index into the map's interactables
    pub idx: usize,
    pub pos: V2,
    pub r: f32,
    pub fire: bool,
    pub active: f32,
    pub cooldown: f32,
    pub tick: f32,
}

#[derive(Default)]
pub struct Secrets {
    pub traps: Vec<Trap>,
    /// shield parts picked up (by interactable index)
    pub parts: Vec<usize>,
    pub shield: bool,
    pub shield_hp: f32,
    pub candles: Vec<usize>,
    /// all candles lit: survive this round to finish the quest
    pub egg_round: Option<u32>,
    pub egg_done: bool,
}

pub const TRAP_ON: f32 = 25.0;
pub const TRAP_CD: f32 = 45.0;
pub const SHIELD_HP: f32 = 400.0;

impl Game {
    pub fn init_secrets(&mut self) {
        let Some(m) = world::map() else { return };
        let traps = m
            .interact
            .iter()
            .enumerate()
            .filter(|(_, it)| it.kind == "trap")
            .map(|(i, it)| {
                let (kind, r) = it.data.split_once(':').unwrap_or(("electric", "120"));
                Trap {
                    idx: i,
                    pos: v2(it.pos[0], it.pos[1]),
                    r: r.parse().unwrap_or(120.0),
                    fire: kind == "fire",
                    active: 0.0,
                    cooldown: 0.0,
                    tick: 0.0,
                }
            })
            .collect();
        self.secrets = Some(Box::new(Secrets {
            traps,
            ..Default::default()
        }));
    }

    pub fn trap_at(&self, idx: usize) -> Option<&Trap> {
        self.secrets.as_ref()?.traps.iter().find(|t| t.idx == idx)
    }

    pub fn activate_trap(&mut self, idx: usize) -> bool {
        let Some(s) = self.secrets.as_mut() else { return false };
        let Some(t) = s.traps.iter_mut().find(|t| t.idx == idx) else { return false };
        if t.active > 0.0 || t.cooldown > 0.0 {
            return false;
        }
        t.active = TRAP_ON;
        let p = t.pos;
        self.sfx(Sfx::Lightning, p);
        self.add_shake(0.2);
        true
    }

    pub fn update_traps(&mut self) {
        let Some(s) = self.secrets.as_mut() else { return };
        let mut zaps: Vec<(V2, f32, bool)> = Vec::new();
        for t in s.traps.iter_mut() {
            if t.active > 0.0 {
                t.active -= DT;
                t.tick -= DT;
                if t.tick <= 0.0 {
                    t.tick = 0.25;
                    zaps.push((t.pos, t.r, t.fire));
                }
                if t.active <= 0.0 {
                    t.cooldown = TRAP_CD;
                }
            } else if t.cooldown > 0.0 {
                t.cooldown -= DT;
            }
        }
        for (pos, r, fire) in zaps {
            let dmg = 40.0 + 25.0 * self.wave.n as f32;
            let mut hits = Vec::new();
            self.grid.query(pos, r + 30.0, |j| hits.push(j));
            for j in hits {
                let z = &self.zombies[j];
                if z.dead || z.pos.dist(pos) > r + z.r {
                    continue;
                }
                let elem = if fire { Elem::Burn } else { Elem::Shock };
                let power = self.elem_power();
                self.afflict(j, elem, power);
                if !self.zombies[j].dead {
                    let dir = (self.zombies[j].pos - pos).norm();
                    self.damage_zombie(j, dmg, dir, 60.0, false);
                }
            }
            // and it doesn't care who you are
            if self.player.pos.dist(pos) < r + PR {
                self.hurt_player(12.0, pos);
            }
            if fire {
                self.burst(pos, 10, r * 1.6, "#ff7a1f", 4.0, 0.4, false, true);
            } else {
                let a = self.rng.angle();
                let pts = vec![pos + V2::from_angle(a) * r, pos, pos + V2::from_angle(a + 2.4) * r];
                self.bolts.push(crate::game::Bolt { pts, life: 0.15 });
            }
            self.light(pos, r * 1.8, 0.2, if fire { "#ff8040" } else { "#a0c8ff" });
        }
    }

    pub fn pick_part(&mut self, idx: usize) -> bool {
        let Some(s) = self.secrets.as_mut() else { return false };
        if s.parts.contains(&idx) || s.shield {
            return false;
        }
        s.parts.push(idx);
        let n = s.parts.len();
        self.toast = Some((format!("Shield part {}/3", n), 1.8));
        self.audio.play(Sfx::Pickup);
        true
    }

    pub fn build_shield(&mut self) -> bool {
        let Some(s) = self.secrets.as_mut() else { return false };
        if s.parts.len() < 3 || s.shield {
            return false;
        }
        s.shield = true;
        s.shield_hp = SHIELD_HP;
        self.banner = Some(Banner {
            title: "RIOT SHIELD".into(),
            sub: "Blocks most damage from the front".into(),
            t: 0.0,
            dur: 2.4,
            color: "#9fdcff",
        });
        self.audio.play(Sfx::LevelUp);
        true
    }

    /// The shield soaks frontal hits. Returns the damage that gets through.
    pub fn shield_block(&mut self, dmg: f32, from: V2) -> f32 {
        let aim = self.player.aim;
        let pos = self.player.pos;
        let Some(s) = self.secrets.as_mut() else { return dmg };
        if !s.shield {
            return dmg;
        }
        let to = (from - pos).norm();
        if to.dot(V2::from_angle(aim)) < 0.4 {
            return dmg;
        }
        let blocked = dmg * 0.8;
        s.shield_hp -= blocked;
        if s.shield_hp <= 0.0 {
            s.shield = false;
            s.parts.clear();
            self.toast = Some(("Your shield broke! The parts are back where you found them.".into(), 2.5));
            self.audio.play(Sfx::Crit);
        } else {
            self.sfx(Sfx::Blade, pos);
        }
        dmg - blocked
    }

    pub fn light_candle(&mut self, idx: usize) -> bool {
        let power = self.ob.as_ref().map(|o| o.power).unwrap_or(false);
        let Some(s) = self.secrets.as_mut() else { return false };
        if s.candles.contains(&idx) || s.egg_done {
            return false;
        }
        if !power {
            self.toast = Some(("The wick won't take. Something needs power…".into(), 2.0));
            return true;
        }
        s.candles.push(idx);
        let lit = s.candles.len();
        if lit == 3 {
            s.egg_round = Some(self.wave.n);
            self.banner = Some(Banner {
                title: "THE BLOOM STIRS".into(),
                sub: "Survive this round".into(),
                t: 0.0,
                dur: 3.0,
                color: "#ff6ab0",
            });
            self.say("you should not have done that");
            self.audio.play(Sfx::BossRoar);
        } else {
            self.toast = Some((format!("A candle flickers to life ({}/3)", lit), 2.0));
            self.audio.play(Sfx::Heal);
        }
        true
    }

    /// The easter egg completes when the round it started in is survived.
    pub fn check_egg(&mut self) {
        let Some(s) = self.secrets.as_mut() else { return };
        match s.egg_round {
            Some(r) if !s.egg_done && self.wave.n > r => {}
            _ => return,
        }
        s.egg_done = true;
        for pk in [Perk::IronGut, Perk::Quickdraw, Perk::SecondWind, Perk::Fleetfoot] {
            self.give_perk(pk);
        }
        self.banner = Some(Banner {
            title: "SECRET COMPLETE".into(),
            sub: "The Bloom's whisper fades… every perk is yours".into(),
            t: 0.0,
            dur: 4.0,
            color: "#ffd36a",
        });
        self.ui_out.push(("achievement", "asylum_egg".into()));
        self.audio.play(Sfx::LevelUp);
    }
}
