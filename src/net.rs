//! Co-op networking: compact little-endian binary messages carried over WebRTC
//! data channels by the JS layer (`ui/coop.js`).
//!
//! Authority model:
//! - The **host** (slot 0) owns the world: zombies, waves, enemy shots, gems,
//!   pickups, and who killed what.
//! - Every player owns **their own character**: movement, aim, health, downed
//!   state, and their own bullets/powers. Clients simulate shots against their
//!   replica of the zombies and report hits; the host applies them. That keeps
//!   shooting instant for everyone regardless of latency.

use crate::math::{v2, V2};

pub const MAX_PLAYERS: usize = 3;

/// Bump whenever the wire format or sim rules change in a way that would
/// desync peers running different builds.
pub const PROTOCOL: u16 = 7;

pub const MSG_PSTATE: u8 = 1; // client -> host (unreliable): own player state
pub const MSG_HITS: u8 = 2; // client -> host (reliable): damage dealt to zombies
pub const MSG_SNAP: u8 = 3; // host -> clients (unreliable): world snapshot
pub const MSG_GEMS: u8 = 4; // host -> clients (unreliable): xp gem positions
pub const MSG_EVENTS: u8 = 5; // host -> clients (reliable): discrete events
pub const MSG_HELLO: u8 = 6; // everyone (reliable), once per match: protocol version
pub const MSG_WORLD: u8 = 7; // client -> host (reliable): bought a door / flipped the power

// events
pub const E_HURT: u8 = 1;
pub const E_KILL: u8 = 2;
pub const E_XP: u8 = 3;
pub const E_WAVE_START: u8 = 4;
pub const E_WAVE_CLEAR: u8 = 5;
pub const E_HEAL: u8 = 6;
pub const E_FRENZY: u8 = 7;
pub const E_GAMEOVER: u8 = 8;
pub const E_BLAST: u8 = 9;
pub const E_BOSS: u8 = 10;
pub const E_CHEST: u8 = 11; // slot: that player opens a chest
pub const E_PULL: u8 = 12; // slot, velocity: the Butcher hooked that player
pub const E_POWERUP: u8 = 13; // kind: everyone gets the power-up
pub const E_BOARDS: u8 = 14; // window, boards
pub const E_WORLD: u8 = 15; // kind, index: a door opened / the power came on

// player flags
pub const PF_DOWNED: u8 = 1;
pub const PF_DEAD: u8 = 2;
pub const PF_DASH: u8 = 4;
pub const PF_FRENZY: u8 = 8;
pub const PF_HURT: u8 = 16;
pub const PF_CONNECTED: u8 = 32;
pub const PF_MEDIC: u8 = 64; // revives teammates twice as fast
pub const PF_ABILITY: u8 = 128; // class ability active (heat sink / flames)

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Role {
    Host,
    Client,
}

/// What every peer knows about a player (sent in snapshots / player states).
#[derive(Clone, Default, Debug)]
pub struct NetPlayer {
    pub slot: u8,
    pub pos: V2,
    pub vel: V2,
    pub aim: f32,
    pub hp: f32,
    pub max_hp: f32,
    pub flags: u8,
    pub revive: f32,
    pub shots: u8,
    pub level: u8,
    pub skin: u8,
    pub hat: u8,
    pub tracer: u8,
    pub gun: u8,
}

impl NetPlayer {
    pub fn downed(&self) -> bool {
        self.flags & PF_DOWNED != 0
    }
    pub fn dead(&self) -> bool {
        self.flags & PF_DEAD != 0
    }
    pub fn alive(&self) -> bool {
        self.flags & (PF_DOWNED | PF_DEAD) == 0 && self.flags & PF_CONNECTED != 0
    }
}

/// A teammate as seen locally: interpolated between received states.
#[derive(Clone, Default)]
pub struct Remote {
    pub present: bool,
    pub name: String,
    pub state: NetPlayer,
    pub prev_pos: V2,
    pub recv_t: f64,
    pub interval: f64,
    pub last_shots: u8,
    pub walk: f32,
    pub recoil: f32,
}

impl Remote {
    pub fn render_pos(&self, now: f64) -> V2 {
        let k = if self.interval > 0.0 {
            ((now - self.recv_t) / self.interval).clamp(0.0, 1.0) as f32
        } else {
            1.0
        };
        self.prev_pos.lerp(self.state.pos, k)
    }
}

pub struct Hit {
    pub uid: u32,
    pub dmg: f32,
    pub dir: V2,
    pub knock: f32,
    pub crit: bool,
    /// element bits (status::Elem::bit) to apply on the host
    pub fx: u8,
}

pub struct Net {
    pub role: Role,
    pub me: u8,
    pub remotes: [Remote; MAX_PLAYERS],
    pub out_hits: Vec<Hit>,
    pub out_events: W,
    pub out_event_count: u16,
    pub snap_t: f64,
    pub gems_t: f64,
    pub state_t: f64,
    pub last_snap_at: f64,
    pub snap_interval: f64,
    pub tick: u32,
    pub game_over_sent: bool,
}

impl Net {
    pub fn new(role: Role, me: u8) -> Self {
        Net {
            role,
            me,
            remotes: Default::default(),
            out_hits: Vec::new(),
            out_events: W::new(),
            out_event_count: 0,
            snap_t: 0.0,
            gems_t: 0.0,
            state_t: 0.0,
            last_snap_at: 0.0,
            snap_interval: 0.05,
            tick: 0,
            game_over_sent: false,
        }
    }
    pub fn players(&self) -> usize {
        1 + self.remotes.iter().filter(|r| r.present).count()
    }
    /// Queue a host event; flushed once per frame as one reliable message.
    pub fn event(&mut self, f: impl FnOnce(&mut W)) {
        f(&mut self.out_events);
        self.out_event_count += 1;
    }
    pub fn take_events(&mut self) -> Option<Vec<u8>> {
        if self.out_event_count == 0 {
            return None;
        }
        let mut w = W::new();
        w.u8(MSG_EVENTS);
        w.u16(self.out_event_count);
        w.0.extend_from_slice(&self.out_events.0);
        self.out_events = W::new();
        self.out_event_count = 0;
        Some(w.0)
    }
}

// ---------------------------------------------------------------- byte io
pub struct W(pub Vec<u8>);

impl W {
    pub fn new() -> Self {
        W(Vec::with_capacity(256))
    }
    pub fn u8(&mut self, v: u8) {
        self.0.push(v);
    }
    pub fn u16(&mut self, v: u16) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    pub fn u32(&mut self, v: u32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    pub fn f32(&mut self, v: f32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    pub fn v2(&mut self, v: V2) {
        self.f32(v.x);
        self.f32(v.y);
    }
    pub fn angle(&mut self, a: f32) {
        self.u8(((a.rem_euclid(std::f32::consts::TAU) / std::f32::consts::TAU) * 256.0) as u8);
    }
}

pub struct R<'a> {
    b: &'a [u8],
    i: usize,
}

impl<'a> R<'a> {
    pub fn new(b: &'a [u8]) -> Self {
        R { b, i: 0 }
    }
    fn take<const N: usize>(&mut self) -> Option<[u8; N]> {
        let s = self.b.get(self.i..self.i + N)?;
        self.i += N;
        s.try_into().ok()
    }
    pub fn u8(&mut self) -> Option<u8> {
        Some(self.take::<1>()?[0])
    }
    pub fn u16(&mut self) -> Option<u16> {
        Some(u16::from_le_bytes(self.take::<2>()?))
    }
    pub fn u32(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(self.take::<4>()?))
    }
    pub fn f32(&mut self) -> Option<f32> {
        let v = f32::from_le_bytes(self.take::<4>()?);
        v.is_finite().then_some(v)
    }
    pub fn v2(&mut self) -> Option<V2> {
        Some(v2(self.f32()?, self.f32()?))
    }
    pub fn angle(&mut self) -> Option<f32> {
        Some(self.u8()? as f32 / 256.0 * std::f32::consts::TAU)
    }
}

pub fn write_player(w: &mut W, p: &NetPlayer) {
    w.u8(p.slot);
    w.v2(p.pos);
    w.v2(p.vel);
    w.f32(p.aim);
    w.f32(p.hp);
    w.f32(p.max_hp);
    w.u8(p.flags);
    w.u8((p.revive.clamp(0.0, 1.0) * 255.0) as u8);
    w.u8(p.shots);
    w.u8(p.level);
    w.u8(p.skin);
    w.u8(p.hat);
    w.u8(p.tracer);
    w.u8(p.gun);
}

pub fn read_player(r: &mut R) -> Option<NetPlayer> {
    Some(NetPlayer {
        slot: r.u8()?.min(MAX_PLAYERS as u8 - 1),
        pos: r.v2()?,
        vel: r.v2()?,
        aim: r.f32()?,
        hp: r.f32()?,
        max_hp: r.f32()?.max(1.0),
        flags: r.u8()?,
        revive: r.u8()? as f32 / 255.0,
        shots: r.u8()?,
        level: r.u8()?,
        skin: r.u8()?,
        hat: r.u8()?,
        tracer: r.u8()?,
        gun: r.u8()?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn player_roundtrip() {
        let p = NetPlayer {
            slot: 2,
            pos: v2(12.5, -40.0),
            vel: v2(3.0, 4.0),
            aim: 1.25,
            hp: 77.0,
            max_hp: 125.0,
            flags: PF_DOWNED | PF_CONNECTED,
            revive: 0.5,
            shots: 200,
            level: 9,
            skin: 3,
            hat: 6,
            tracer: 2,
            gun: 4,
        };
        let mut w = W::new();
        write_player(&mut w, &p);
        let q = read_player(&mut R::new(&w.0)).unwrap();
        assert_eq!(q.slot, 2);
        assert_eq!(q.pos, p.pos);
        assert_eq!(q.flags, p.flags);
        assert!((q.revive - 0.5).abs() < 0.01);
        assert_eq!((q.shots, q.level, q.hat, q.gun), (200, 9, 6, 4));
        // truncated input never panics
        for n in 0..w.0.len() {
            assert!(read_player(&mut R::new(&w.0[..n])).is_none());
        }
    }
}
