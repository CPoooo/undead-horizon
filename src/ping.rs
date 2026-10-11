//! Pings and quick chat for co-op: G drops a contextual marker ("Enemy",
//! "Need revive", "Box here", "Here") that the whole squad sees in the world,
//! on the minimap and as an edge arrow; emotes travel the same way.

use crate::audio::Sfx;
use crate::game::Game;
use crate::math::*;
use crate::net::{self, W};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum PingKind {
    Here = 0,
    Enemy = 1,
    Revive = 2,
    Box = 3,
    Loot = 4,
}

impl PingKind {
    pub fn from_u8(v: u8) -> PingKind {
        match v {
            1 => PingKind::Enemy,
            2 => PingKind::Revive,
            3 => PingKind::Box,
            4 => PingKind::Loot,
            _ => PingKind::Here,
        }
    }
    /// (label, icon, color)
    pub fn def(self) -> (&'static str, &'static str, &'static str) {
        match self {
            PingKind::Here => ("Here", "📍", "#7ad7ff"),
            PingKind::Enemy => ("Enemy", "⚠️", "#ff5050"),
            PingKind::Revive => ("Need revive", "🆘", "#9cffb8"),
            PingKind::Box => ("Box here", "❓", "#ffd36a"),
            PingKind::Loot => ("Loot", "💎", "#d58cff"),
        }
    }
}

pub struct Ping {
    pub pos: V2,
    pub kind: PingKind,
    pub slot: u8,
    pub t: f32,
    /// an emote bubble instead of a marker (index into content::EMOTES)
    pub emote: Option<usize>,
}

pub const PING_T: f32 = 6.0;

impl Game {
    /// Where the player is pointing: the cursor, or ahead of the aim on touch / pad.
    fn ping_point(&self) -> V2 {
        if self.input.mouse_seen && !self.input.is_touch && !self.input.pad_active {
            self.mouse_world
        } else {
            self.player.pos + V2::from_angle(self.player.aim) * 220.0
        }
    }

    pub fn try_ping(&mut self) -> bool {
        let at = self.ping_point();
        // what's there decides the ping
        let kind = if self
            .net
            .as_ref()
            .is_some_and(|n| n.remotes.iter().any(|r| r.present && r.state.downed() && r.state.pos.dist(at) < 120.0))
        {
            PingKind::Revive
        } else if self.zombies.iter().any(|z| !z.dead && !z.hidden && z.pos.dist(at) < z.r + 60.0) {
            PingKind::Enemy
        } else if self.pickups.iter().any(|p| p.pos.dist(at) < 80.0) {
            PingKind::Loot
        } else if self.near_box(at) {
            PingKind::Box
        } else {
            PingKind::Here
        };
        let pos = match kind {
            PingKind::Enemy => self
                .zombies
                .iter()
                .filter(|z| !z.dead && !z.hidden)
                .min_by(|a, b| a.pos.dist2(at).partial_cmp(&b.pos.dist2(at)).unwrap_or(std::cmp::Ordering::Equal))
                .map(|z| z.pos)
                .unwrap_or(at),
            _ => at,
        };
        self.send_ping(kind as u8, pos);
        true
    }

    fn near_box(&self, at: V2) -> bool {
        let (Some(ob), Some(m)) = (self.ob.as_ref(), crate::world::map()) else { return false };
        m.interact.get(ob.box_at).is_some_and(|b| v2(b.pos[0], b.pos[1]).dist(at) < 140.0)
    }

    /// Share an emote with the squad (shown over our head on their screens).
    pub fn share_emote(&mut self, idx: usize) {
        if self.hub.is_some() {
            // Home Base: relayed to party members through the social server
            self.ui_out.push(("hubemote", idx.to_string()));
        }
        if self.net.is_some() {
            let p = self.player.pos;
            self.send_ping(100 + idx as u8, p);
        }
    }

    fn send_ping(&mut self, kind: u8, pos: V2) {
        let me = self.my_slot();
        self.add_ping(me, kind, pos);
        if self.is_client() {
            let mut w = W::new();
            w.u8(net::MSG_PING);
            w.u8(kind);
            w.v2(pos);
            self.net_out.push((0, true, w.0));
        } else if let Some(n) = &mut self.net {
            n.event(|w| {
                w.u8(net::E_PING);
                w.u8(me);
                w.u8(kind);
                w.v2(pos);
            });
        }
    }

    /// A ping arrived (or we made one). Kind >= 100 is an emote.
    pub fn add_ping(&mut self, slot: u8, kind: u8, pos: V2) {
        if self.pings.len() >= 12 {
            self.pings.remove(0);
        }
        let emote = if kind >= 100 { Some((kind - 100) as usize) } else { None };
        if emote.is_none() {
            // one marker per player: a new ping replaces their old one
            self.pings.retain(|p| p.slot != slot || p.emote.is_some());
        }
        self.pings.push(Ping { pos, kind: PingKind::from_u8(kind), slot, t: 0.0, emote });
        if emote.is_none() {
            self.audio.play(Sfx::UiMove);
        }
    }

    /// Host: a client pinged; show it and tell everyone.
    pub fn relay_ping(&mut self, from: u8, kind: u8, pos: V2) {
        self.add_ping(from, kind, pos);
        if let Some(n) = &mut self.net {
            n.event(|w| {
                w.u8(net::E_PING);
                w.u8(from);
                w.u8(kind);
                w.v2(pos);
            });
        }
    }

    pub fn tick_pings(&mut self, dt: f32) {
        for p in self.pings.iter_mut() {
            p.t += dt;
        }
        self.pings.retain(|p| p.t < if p.emote.is_some() { 2.5 } else { PING_T });
    }
}
