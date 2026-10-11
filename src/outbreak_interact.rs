//! Outbreak interactables: what's in reach, the on-screen prompt, and using it.
//! Doors and the power switch are shared world state: the host applies them
//! and broadcasts; clients ask the host with MSG_WORLD.

use crate::audio::Sfx;
use crate::content::{gun_index, GUNS};
use crate::game::Game;
use crate::math::*;
use crate::modes::outbreak::{BoxSpin, Perk, MAX_BOARDS, SPIN_T, USE_R};
use crate::net::{self, W};
use crate::world;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Target {
    Door(usize),
    Wall(usize),
    Box,
    BoxTake,
    Perk(usize),
    Power,
    Upgrade(usize),
    Repair(usize),
}

pub struct Prompt {
    pub pos: V2,
    pub text: String,
    /// affordable / usable
    pub ok: bool,
}

pub const W_DOOR: u8 = 1;
pub const W_POWER: u8 = 2;

impl Game {
    fn ob_target(&self) -> Option<(Target, V2)> {
        let ob = self.ob.as_ref()?;
        let m = world::map()?;
        let p = self.player.pos;
        let mut best: Option<(f32, Target, V2)> = None;
        fn consider(best: &mut Option<(f32, Target, V2)>, p: V2, t: Target, at: V2, reach: f32) {
            let d = at.dist(p);
            if d < reach && best.as_ref().map_or(true, |b| d < b.0) {
                *best = Some((d, t, at));
            }
        }
        for (i, d) in m.doors.iter().enumerate() {
            if !d.open.get() {
                consider(&mut best, p, Target::Door(i), m.door_center(i), USE_R + 30.0);
            }
        }
        for (i, it) in m.interact.iter().enumerate() {
            let at = v2(it.pos[0], it.pos[1]);
            match it.kind.as_str() {
                "wall" => consider(&mut best, p, Target::Wall(i), at, USE_R),
                "perk" => consider(&mut best, p, Target::Perk(i), at, USE_R),
                "power" if !ob.power => consider(&mut best, p, Target::Power, at, USE_R),
                "upgrade" => consider(&mut best, p, Target::Upgrade(i), at, USE_R),
                "box" if i == ob.box_at && ob.box_away <= 0.0 => {
                    let offer = ob.spin.as_ref().map(|s| s.t >= SPIN_T && !s.hand).unwrap_or(false);
                    let spinning = ob.spin.is_some() && !offer;
                    if offer {
                        consider(&mut best, p, Target::BoxTake, at, USE_R);
                    } else if !spinning {
                        consider(&mut best, p, Target::Box, at, USE_R);
                    }
                }
                _ => {}
            }
        }
        if best.is_none() {
            for (i, w) in ob.windows.iter().enumerate() {
                if w.boards < MAX_BOARDS {
                    consider(&mut best, p, Target::Repair(i), w.pos, USE_R + 20.0);
                }
            }
        }
        best.map(|b| (b.1, b.2))
    }

    /// The prompt shown above whatever is in reach.
    pub fn ob_prompt(&self) -> Option<Prompt> {
        let (t, at) = self.ob_target()?;
        let ob = self.ob.as_ref()?;
        let m = world::map()?;
        let pts = ob.points;
        let key = if self.input.is_touch { "Tap USE" } else { "[E]" };
        let (text, ok) = match t {
            Target::Door(i) => {
                let d = &m.doors[i].def;
                (format!("{} Open {} — {}", key, if d.label.is_empty() { "door" } else { &d.label }, d.cost), pts >= d.cost)
            }
            Target::Wall(i) => {
                let it = &m.interact[i];
                let g = gun_index(&it.data);
                let owned = ob.weapons.iter().any(|w| w.gun == g);
                if owned {
                    (format!("{} {} ammo — {}", key, GUNS[g].name, it.cost / 2), pts >= it.cost / 2)
                } else {
                    (format!("{} Buy {} — {}", key, GUNS[g].name, it.cost), pts >= it.cost)
                }
            }
            Target::Box => (format!("{} Mystery Box — 950", key), pts >= 950),
            Target::BoxTake => {
                let g = ob.spin.as_ref().map(|s| s.gun).unwrap_or(0);
                (format!("{} Take {}", key, GUNS[g].name), true)
            }
            Target::Perk(i) => {
                let it = &m.interact[i];
                match Perk::from_id(&it.id) {
                    Some(pk) if ob.has(pk) => (format!("{} (owned)", pk.def().0), false),
                    Some(pk) if !ob.power => (format!("{} — the power is off", pk.def().0), false),
                    Some(pk) => (format!("{} {} — {}", key, pk.def().0, it.cost), pts >= it.cost),
                    None => return None,
                }
            }
            Target::Power => (format!("{} Turn on the power", key), true),
            Target::Upgrade(i) => {
                let cost = m.interact[i].cost;
                let w = ob.weapon();
                if !ob.power {
                    ("Upgrade machine — the power is off".into(), false)
                } else if w.upgraded {
                    (format!("{} is already upgraded", w.name()), false)
                } else {
                    (format!("{} Upgrade {} — {}", key, w.name(), cost), pts >= cost)
                }
            }
            Target::Repair(_) => (format!("{} Hold to rebuild the barricade", key), true),
        };
        Some(Prompt { pos: at, text, ok })
    }

    /// Use whatever is in reach. Returns true when something was used.
    pub fn ob_interact(&mut self) -> bool {
        let Some((t, at)) = self.ob_target() else { return false };
        let Some(m) = world::map() else { return false };
        let deny = |g: &mut Game| {
            g.audio.play(Sfx::UiMove);
            g.toast = Some(("Not enough points".into(), 1.0));
        };
        match t {
            Target::Door(i) => {
                let cost = m.doors[i].def.cost;
                if !self.spend(cost) {
                    deny(self);
                    return true;
                }
                self.world_change(W_DOOR, i as u8);
            }
            Target::Wall(i) => {
                let it = &m.interact[i];
                let g = gun_index(&it.data);
                let owned = self.ob.as_ref().map(|o| o.weapons.iter().any(|w| w.gun == g)).unwrap_or(false);
                let cost = if owned { it.cost / 2 } else { it.cost };
                if !self.spend(cost) {
                    deny(self);
                    return true;
                }
                self.give_weapon(g);
                self.audio.play(Sfx::UiSelect);
                self.burst(at, 12, 200.0, "#ffd36a", 3.0, 0.4, false, true);
            }
            Target::Box => {
                if !self.spend(950) {
                    deny(self);
                    return true;
                }
                let gun = self.roll_box_gun();
                let ob = self.ob.as_mut().unwrap();
                ob.box_uses += 1;
                let hand = ob.box_uses >= 4 && self.rng.chance(0.14);
                if hand {
                    ob.points += 950; // refunded when the box flies away
                }
                ob.spin = Some(BoxSpin { t: 0.0, gun, hand });
                self.sfx(Sfx::ReadyUp, at);
                self.light(at, 260.0, 3.0, "#7ad7ff");
            }
            Target::BoxTake => {
                let gun = self.ob.as_mut().and_then(|o| o.spin.take()).map(|s| s.gun);
                if let Some(g) = gun {
                    self.give_weapon(g);
                    self.audio.play(Sfx::UiSelect);
                }
            }
            Target::Perk(i) => {
                let it = &m.interact[i];
                let Some(pk) = Perk::from_id(&it.id) else { return false };
                let (owned, power) = self.ob.as_ref().map(|o| (o.has(pk), o.power)).unwrap_or((true, false));
                if owned || !power {
                    self.audio.play(Sfx::UiMove);
                    return true;
                }
                if !self.spend(it.cost) {
                    deny(self);
                    return true;
                }
                self.give_perk(pk);
                self.burst(at, 20, 240.0, pk.def().2, 3.0, 0.6, false, true);
            }
            Target::Power => self.world_change(W_POWER, 0),
            Target::Upgrade(i) => {
                let cost = m.interact[i].cost;
                let ok = self.ob.as_ref().map(|o| o.power && !o.weapon().upgraded).unwrap_or(false);
                if !ok {
                    self.audio.play(Sfx::UiMove);
                    return true;
                }
                if !self.spend(cost) {
                    deny(self);
                    return true;
                }
                let ob = self.ob.as_mut().unwrap();
                let w = ob.weapon_mut();
                w.upgraded = true;
                w.mag = w.max_mag();
                w.reserve = w.max_reserve();
                let name = w.name();
                self.banner = Some(crate::game::Banner {
                    title: "UPGRADED".into(),
                    sub: name.to_string(),
                    t: 0.0,
                    dur: 2.2,
                    color: "#d58cff",
                });
                self.ring(at, 200.0, 0.6, "#d58cff", 10.0);
                self.burst(at, 40, 360.0, "#d58cff", 3.5, 0.8, false, true);
                self.audio.play(Sfx::LevelUp);
                self.say("upgraded");
            }
            Target::Repair(_) => {
                if let Some(ob) = self.ob.as_mut() {
                    ob.use_held = 0.25;
                }
            }
        }
        true
    }

    fn roll_box_gun(&mut self) -> usize {
        let owned: Vec<usize> = self.ob.as_ref().map(|o| o.weapons.iter().map(|w| w.gun).collect()).unwrap_or_default();
        let pool: Vec<(usize, f32)> = GUNS
            .iter()
            .enumerate()
            .filter(|(i, _)| !owned.contains(i))
            .map(|(i, g)| (i, if matches!(g.id, "ray" | "wave") { 0.6 } else { 1.0 }))
            .collect();
        let total: f32 = pool.iter().map(|p| p.1).sum();
        let mut r = self.rng.f() * total;
        for (i, w) in &pool {
            r -= w;
            if r <= 0.0 {
                return *i;
            }
        }
        pool.first().map(|p| p.0).unwrap_or(0)
    }

    /// Change shared world state: the host applies and broadcasts it; a client asks the host.
    pub fn world_change(&mut self, kind: u8, idx: u8) {
        if self.is_client() {
            let mut w = W::new();
            w.u8(net::MSG_WORLD);
            w.u8(kind);
            w.u8(idx);
            self.net_out.push((0, true, w.0));
            // optimistic: show it right away
            self.apply_world(kind, idx);
            return;
        }
        self.apply_world(kind, idx);
        if let Some(n) = &mut self.net {
            n.event(|w| {
                w.u8(net::E_WORLD);
                w.u8(kind);
                w.u8(idx);
            });
        }
    }

    pub fn apply_world(&mut self, kind: u8, idx: u8) {
        let Some(m) = world::map() else { return };
        match kind {
            W_DOOR => {
                if m.open_door(idx as usize) {
                    let at = m.door_center(idx as usize);
                    self.burst(at, 30, 260.0, "#8a6034", 4.0, 0.8, true, false);
                    self.sfx(Sfx::Explosion, at);
                    self.add_shake(0.2);
                    if let Some(f) = self.flow.as_mut() {
                        f.doors_sig = u64::MAX;
                    }
                }
            }
            W_POWER => {
                let Some(ob) = self.ob.as_mut() else { return };
                if ob.power {
                    return;
                }
                ob.power = true;
                self.banner = Some(crate::game::Banner {
                    title: "POWER ON".into(),
                    sub: "Perk machines and the upgrade machine are live".into(),
                    t: 0.0,
                    dur: 3.0,
                    color: "#7ad7ff",
                });
                self.audio.play(Sfx::MatchStart);
                self.say("power on");
                self.add_shake(0.4);
            }
            _ => {}
        }
    }
}
