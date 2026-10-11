//! Co-op glue for `Game`: network I/O, snapshots, replicas, downed/revive.
//! See `net.rs` for the authority model and wire format.

use crate::audio::Sfx;
use crate::modes::Mode;
use crate::content::{self, BossVar, GUNS, INFINITE_BOSSES};
use crate::game::*;
use crate::math::*;
use crate::net::*;
use crate::status::Elem;
use crate::upgrades::Up;

const SNAP_HZ: f64 = 20.0;
const STATE_HZ: f64 = 30.0;
const GEMS_HZ: f64 = 12.0;
/// host-only state for host migration (M3a)
const CHECKPOINT_HZ: f64 = 1.0;
const REVIVE_TIME: f32 = 3.0;
const REVIVE_RADIUS: f32 = 75.0;
pub const BLEED_TIME: f32 = 30.0;

/// A player the host simulates against: (slot, position, velocity).
pub type Target = (u8, V2, V2);

impl Game {
    pub fn coop(&self) -> bool {
        self.net.is_some()
    }
    pub fn is_client(&self) -> bool {
        matches!(&self.net, Some(n) if n.role == Role::Client)
    }
    pub fn is_host(&self) -> bool {
        matches!(&self.net, Some(n) if n.role == Role::Host)
    }
    pub fn my_slot(&self) -> u8 {
        self.net.as_ref().map(|n| n.me).unwrap_or(0)
    }
    pub fn player_alive(&self) -> bool {
        !self.player.downed && !self.player.dead
    }

    /// Start a co-op run. `opts` = the usual start options plus
    /// `role=host|client;me=<slot>;names=a,b,c;present=1,1,0`.
    pub fn start_coop(&mut self, opts: &str) {
        let mut role = Role::Host;
        let mut me = 0u8;
        let mut names: Vec<String> = Vec::new();
        let mut present: Vec<bool> = Vec::new();
        for kv in opts.split(';') {
            let Some((k, v)) = kv.split_once('=') else {
                continue;
            };
            match k {
                "role" => {
                    role = if v == "client" {
                        Role::Client
                    } else {
                        Role::Host
                    }
                }
                "me" => me = v.parse::<u8>().unwrap_or(0).min(MAX_PLAYERS as u8 - 1),
                "names" => names = v.split(',').map(|s| s.chars().take(16).collect()).collect(),
                "present" => present = v.split(',').map(|s| s == "1").collect(),
                _ => {}
            }
        }
        self.start(opts);
        // co-op supports Infinite, Campaign (the host's chapter), Outbreak and Siege
        if !matches!(self.mode, Mode::Campaign(_) | Mode::Outbreak | Mode::Siege) {
            self.mode = crate::modes::Mode::Infinite;
        }
        let mut net = Net::new(role, me);
        for slot in 0..MAX_PLAYERS {
            if slot as u8 == me {
                continue;
            }
            let r = &mut net.remotes[slot];
            r.present = present.get(slot).copied().unwrap_or(false);
            r.name = names.get(slot).cloned().unwrap_or_default();
            r.state.slot = slot as u8;
            r.state.flags = PF_CONNECTED;
            r.state.hp = 100.0;
            r.state.max_hp = 100.0;
            // spread out a little so nobody spawns on top of each other
            r.state.pos = v2((slot as f32 - 1.0) * 60.0, 0.0);
            r.prev_pos = r.state.pos;
        }
        self.player.pos = v2((me as f32 - 1.0) * 60.0, 0.0);
        self.player.prev = self.player.pos;
        if role == Role::Client {
            self.wave.intermission = 0.0;
        }
        // joining a match in progress: spectate until the wave is cleared
        let late = opts.split(';').any(|kv| kv == "late=1");
        let n = net.players();
        // say hello so peers on a different build bail out instead of desyncing
        let peers: Vec<u8> = if role == Role::Host {
            (0..MAX_PLAYERS as u8)
                .filter(|s| *s != me && net.remotes[*s as usize].present)
                .collect()
        } else {
            vec![0]
        };
        for slot in peers {
            let mut w = W::new();
            w.u8(MSG_HELLO);
            w.u16(PROTOCOL);
            self.net_out.push((slot, true, w.0));
        }
        self.net = Some(net);
        if late {
            self.late_join = true;
            self.player.dead = true;
            self.player.hp = 0.0;
            self.banner = Some(Banner {
                title: "JOINING THE FIGHT".into(),
                sub: "Spectating · you drop in when this wave is cleared".into(),
                t: 0.0,
                dur: 4.0,
                color: "#7ad7ff",
            });
            return;
        }
        self.banner = Some(Banner {
            title: "CO-OP".into(),
            sub: format!(
                "{} survivors · {} · stick together, revive each other",
                n,
                self.difficulty().name
            ),
            t: 0.0,
            dur: 3.0,
            color: "#9cffb8",
        });
    }

    /// A player joined (or rejoined) mid-match in `slot`. Everyone marks them
    /// present; the host also sends them the world state that otherwise only
    /// travels as one-off events (doors, power, box, boards, defenses, crystal).
    pub fn peer_joined(&mut self, slot: u8, name: &str) {
        let me = self.my_slot();
        if slot as usize >= MAX_PLAYERS || slot == me {
            return;
        }
        let host = self.is_host();
        let Some(net) = &mut self.net else { return };
        let r = &mut net.remotes[slot as usize];
        let n: String = name.chars().take(16).collect();
        r.name = if n.is_empty() { format!("Player {}", slot + 1) } else { n };
        r.present = true;
        r.recv_t = 0.0;
        r.state = NetPlayer { slot, flags: PF_CONNECTED, hp: 100.0, max_hp: 100.0, ..r.state.clone() };
        self.toast = Some((format!("{} joined the fight", r.name), 2.5));
        if !host {
            return;
        }
        let mut hello = W::new();
        hello.u8(MSG_HELLO);
        hello.u16(PROTOCOL);
        self.net_out.push((slot, true, hello.0));
        let sync = self.encode_sync();
        self.net_out.push((slot, true, sync));
    }

    /// Host: the persistent world state as a batch of events for a late joiner.
    fn encode_sync(&self) -> Vec<u8> {
        use crate::outbreak_interact::{W_BOXMOVE, W_DOOR, W_POWER};
        let mut ev = W::new();
        let mut n = 0u16;
        if let Some(m) = crate::world::map() {
            for (i, d) in m.doors.iter().enumerate() {
                if d.open.get() {
                    ev.u8(E_WORLD);
                    ev.u8(W_DOOR);
                    ev.u8(i as u8);
                    n += 1;
                }
            }
        }
        if let Some(ob) = &self.ob {
            if ob.power {
                ev.u8(E_WORLD);
                ev.u8(W_POWER);
                ev.u8(0);
                n += 1;
            }
            ev.u8(E_WORLD);
            ev.u8(W_BOXMOVE);
            ev.u8(ob.box_at as u8);
            n += 1;
            for (i, w) in ob.windows.iter().enumerate() {
                ev.u8(E_BOARDS);
                ev.u8(i as u8);
                ev.u8(w.boards);
                n += 1;
            }
        }
        if let Some(s) = &self.siege {
            for d in &s.defenses {
                ev.u8(E_DEFENSE);
                ev.u32(d.uid);
                ev.u8(d.kind as u8);
                ev.v2(d.pos);
                ev.u8(d.tier);
                n += 1;
            }
            ev.u8(E_CRYSTAL);
            ev.f32(s.crystal_hp);
            n += 1;
        }
        let mut w = W::new();
        w.u8(MSG_EVENTS);
        w.u16(n);
        w.0.extend_from_slice(&ev.0);
        w.0
    }

    /// Wave cleared: players who bled out (or joined mid-wave) drop back in
    /// next to a living teammate.
    pub fn respawn_spectator(&mut self) {
        if !self.coop() || !self.player.dead || !matches!(self.phase, Phase::Playing | Phase::LevelUp | Phase::Paused) {
            return;
        }
        let now = self.real_time;
        let near = self.net.as_ref().and_then(|n| {
            n.remotes.iter().find(|r| r.present && r.state.alive()).map(|r| r.render_pos(now))
        });
        let p = &mut self.player;
        p.dead = false;
        p.downed = false;
        p.revive = 0.0;
        p.hp = p.max_hp * 0.5;
        p.hp_ghost = p.hp;
        p.iframes = 3.0;
        if let Some(at) = near {
            p.pos = at + v2(50.0, 0.0);
            p.prev = p.pos;
        }
        let pos = p.pos;
        self.late_join = false;
        self.banner = Some(Banner {
            title: "BACK IN THE FIGHT".into(),
            sub: "You respawned with your squad".into(),
            t: 0.0,
            dur: 2.6,
            color: "#9cffb8",
        });
        self.ring(pos, 140.0, 0.6, "#9cffb8", 6.0);
        self.push_sfx(Sfx::Heal, pos);
    }

    // ------------------------------------------------------------ host migration
    /// Host: everything a client would need to carry on as host that snapshots
    /// and events don't already replicate.
    fn encode_checkpoint(&self) -> Vec<u8> {
        let mut w = W::new();
        w.u8(MSG_CHECKPOINT);
        w.u16(self.wave.total.min(u16::MAX as u32) as u16);
        w.u16(self.wave.spawned.min(u16::MAX as u32) as u16);
        w.f32(self.wave.timer);
        w.f32(self.wave.boss_timer);
        w.u8(self.wave.boss_pending as u8);
        w.u32(self.next_uid);
        w.f32(self.time);
        match &self.ob {
            Some(ob) => {
                w.u8(1);
                w.u8(ob.hound_round as u8);
                w.u16(ob.next_hounds.min(u16::MAX as u32) as u16);
                w.u16(ob.drops_round.min(u16::MAX as u32) as u16);
                w.u16(ob.box_uses.min(u16::MAX as u32) as u16);
            }
            None => w.u8(0),
        }
        match &self.siege {
            Some(s) => {
                w.u8(1);
                w.u8(matches!(s.phase, crate::modes::siege::SiegePhase::Combat) as u8);
                w.f32(s.build_t);
                w.f32(s.spawn_t);
                w.u32(s.next_uid);
                w.u8(s.queue.len().min(8) as u8);
                for q in s.queue.iter().take(8) {
                    w.u8(q.len().min(255) as u8);
                    for k in q.iter().take(255) {
                        w.u8(*k as u8);
                    }
                }
            }
            None => w.u8(0),
        }
        w.0
    }

    fn apply_checkpoint(&mut self, r: &mut R) {
        let parsed = (|| {
            let total = r.u16()?;
            let spawned = r.u16()?;
            let timer = r.f32()?;
            let boss_timer = r.f32()?;
            let boss_pending = r.u8()? != 0;
            let next_uid = r.u32()?;
            let time = r.f32()?;
            Some((total, spawned, timer, boss_timer, boss_pending, next_uid, time))
        })();
        let Some((total, spawned, timer, boss_timer, boss_pending, next_uid, time)) = parsed else { return };
        self.wave.total = total as u32;
        self.wave.spawned = spawned as u32;
        self.wave.timer = timer;
        self.wave.boss_timer = boss_timer;
        self.wave.boss_pending = boss_pending;
        self.next_uid = self.next_uid.max(next_uid);
        self.time = self.time.max(time);
        if r.u8() == Some(1) {
            let (Some(hr), Some(nh), Some(dr), Some(bu)) = (r.u8(), r.u16(), r.u16(), r.u16()) else { return };
            if let Some(ob) = self.ob.as_mut() {
                ob.hound_round = hr != 0;
                ob.next_hounds = nh as u32;
                ob.drops_round = dr as u32;
                ob.box_uses = bu as u32;
            }
        }
        if r.u8() == Some(1) {
            let (Some(ph), Some(bt), Some(st), Some(nu), Some(np)) = (r.u8(), r.f32(), r.f32(), r.u32(), r.u8()) else { return };
            let mut queue = Vec::new();
            for _ in 0..np {
                let Some(c) = r.u8() else { return };
                let mut q = Vec::new();
                for _ in 0..c {
                    let Some(k) = r.u8() else { return };
                    q.push(zkind_from(k));
                }
                queue.push(q);
            }
            if let Some(s) = self.siege.as_mut() {
                s.phase = if ph != 0 { crate::modes::siege::SiegePhase::Combat } else { crate::modes::siege::SiegePhase::Build };
                s.build_t = bt;
                s.spawn_t = st;
                s.next_uid = s.next_uid.max(nu);
                s.queue = queue;
            }
        }
    }

    /// Host migration (M3b): the host is gone and we have the lowest slot left.
    /// Take over the world from our replica plus the last checkpoint.
    pub fn promote(&mut self, old: u8) {
        let me = self.my_slot();
        let Some(net) = &mut self.net else { return };
        if net.role == Role::Host {
            return;
        }
        net.role = Role::Host;
        net.host_slot = me;
        net.game_over_sent = false;
        for s in [old, 0] {
            if let Some(r) = net.remotes.get_mut(s as usize) {
                r.present = false;
            }
        }
        self.predicted.clear();
        self.eshots.clear(); // replicas carry no velocity or damage
        // replicas carry base stats: scale them back up to this wave
        let n = self.scale_n();
        let df = self.difficulty();
        let sm = (1.0 + (0.018 * (n - 1) as f32).min(0.35)) * if self.mutators.fast { 1.3 } else { 1.0 };
        let mut top = self.next_uid;
        for z in self.zombies.iter_mut() {
            let def = z.kind.def();
            let bs = if z.kind == ZKind::Boss { z.boss_var.stats().2 } else { 1.0 };
            z.speed = def.speed * sm * df.speed * bs;
            z.dmg = def.dmg * (1.0 + 0.04 * (n - 1) as f32) * df.dmg;
            z.xp = def.xp;
            top = top.max(z.uid + 1);
        }
        self.next_uid = top;
        if self.wave.intermission <= 0.0 && self.wave.total == 0 {
            // no checkpoint yet: treat what's alive as the rest of the wave
            self.wave.total = self.alive();
            self.wave.spawned = self.wave.total;
        }
        // spectating with nobody left standing would end the run on the spot
        let anyone_up = self.net.as_ref().is_some_and(|n| n.remotes.iter().any(|r| r.present && r.state.alive()));
        if self.player.dead && !anyone_up {
            self.respawn_spectator();
        }
        self.banner = Some(Banner {
            title: "YOU'RE THE HOST NOW".into(),
            sub: "The host left · the match carries on".into(),
            t: 0.0,
            dur: 3.0,
            color: "#ffd36a",
        });
    }

    /// Host migration seen from another client: `new` took over from `old`.
    pub fn host_moved(&mut self, old: u8, new: u8) {
        let Some(net) = &mut self.net else { return };
        net.host_slot = new;
        net.last_snap_at = 0.0;
        if let Some(r) = net.remotes.get_mut(old as usize) {
            r.present = false;
        }
        let name = net.remotes.get(new as usize).map(|r| r.name.clone()).unwrap_or_default();
        self.toast = Some((format!("Host left · {} is hosting now", name), 3.0));
    }

    pub fn peer_left(&mut self, slot: u8) {
        let host_left = slot == 0 && self.is_client();
        // after a migration the host's link is still key 0, but its player is its own slot
        if host_left {
            let hs = self.net.as_ref().map(|n| n.host_slot).unwrap_or(0);
            if hs != 0 {
                if let Some(r) = self.net.as_mut().and_then(|n| n.remotes.get_mut(hs as usize)) {
                    r.present = false;
                }
            }
        }
        if let Some(net) = &mut self.net {
            if let Some(r) = net.remotes.get_mut(slot as usize) {
                if r.present {
                    r.present = false;
                    let name = r.name.clone();
                    self.toast = Some((format!("{} left the game", name), 2.5));
                }
            }
        }
        if host_left && matches!(self.phase, Phase::Playing | Phase::LevelUp | Phase::Paused) {
            self.toast = Some(("The host left. Game over.".into(), 3.0));
            self.enter_dying();
        }
    }

    pub fn enter_dying(&mut self) {
        if matches!(self.phase, Phase::Dying | Phase::GameOver) {
            return;
        }
        self.phase = Phase::Dying;
        self.dying_t = 0.0;
        self.audio.play(Sfx::GameOver);
        self.add_shake(0.8);
    }

    /// Players zombies can chase (alive, connected). Never empty: falls back to
    /// our own position with slot 255 (no one to hurt).
    pub fn targets(&self) -> Vec<Target> {
        let mut t = Vec::with_capacity(MAX_PLAYERS);
        if self.player_alive() {
            t.push((self.my_slot(), self.player.pos, self.player.vel));
        }
        if let Some(net) = &self.net {
            for r in net.remotes.iter() {
                if r.present && r.state.alive() {
                    t.push((r.state.slot, r.state.pos, r.state.vel));
                }
            }
        }
        if t.is_empty() {
            t.push((255, self.player.pos, V2::ZERO));
        }
        t
    }

    pub fn nearest_target(targets: &[Target], p: V2) -> Target {
        let mut best = targets[0];
        let mut bd = f32::MAX;
        for t in targets {
            let d = t.1.dist2(p);
            if d < bd {
                bd = d;
                best = *t;
            }
        }
        best
    }

    /// Damage a specific player: ourselves directly, teammates via an event.
    pub fn hurt_slot(&mut self, slot: u8, dmg: f32, from: V2) {
        if slot == 255 {
            return;
        }
        if slot == self.my_slot() {
            self.hurt_player(dmg, from);
        } else if let Some(net) = &mut self.net {
            net.event(|w| {
                w.u8(E_HURT);
                w.u8(slot);
                w.f32(dmg);
                w.v2(from);
            });
        }
    }

    pub fn hurt_players_in(&mut self, pos: V2, radius: f32, dmg: f32) {
        for (slot, p, _) in self.targets() {
            if p.dist(pos) < radius + PR {
                self.hurt_slot(slot, dmg, pos);
            }
        }
    }

    /// Credit a kill to the local player (combo, ultimate charge, vampirism).
    pub fn credit_kill(&mut self) {
        self.kills += 1;
        self.combo += 1;
        self.combo_t = 2.5;
        self.best_combo = self.best_combo.max(self.combo);
        let was = self.player.ult;
        self.player.ult = (self.player.ult + 1.0 / (70.0 + 15.0 * self.ult_uses as f32)).min(1.0);
        if was < 1.0 && self.player.ult >= 1.0 && !self.ult_ready_played {
            self.ult_ready_played = true;
            let p = self.player.pos;
            self.push_sfx(Sfx::UltReady, p);
            self.toast = Some(("ULTIMATE READY — press R".into(), 2.2));
            self.say("ultimate ready");
        }
        let vl = self.ups.get(Up::Vampire);
        let pact = self.has_evo(crate::upgrades::Evo::BloodPact);
        let chance = 0.04 * vl as f32 * if pact { 3.0 } else { 1.0 };
        if vl > 0 && self.rng.chance(chance) && self.player_alive() {
            let heal = if pact { 6.0 } else { 3.0 };
            self.player.hp = (self.player.hp + heal).min(self.player.max_hp);
            let pp = self.player.pos;
            self.push_text(pp, format!("+{}", heal as i32), "#ff4d6d", 14.0);
        }
    }

    // ------------------------------------------------------------ downed / revive
    pub fn go_down(&mut self) {
        self.lose_perks();
        self.downs += 1;
        let p = &mut self.player;
        p.hp = 0.0;
        p.downed = true;
        p.revive = 0.0;
        p.bleed = BLEED_TIME;
        p.dash_t = 0.0;
        self.banner = Some(Banner {
            title: "YOU'RE DOWN".into(),
            sub: "Stay alive! A teammate standing next to you revives you".into(),
            t: 0.0,
            dur: 3.0,
            color: "#ff6a6a",
        });
        self.audio.play(Sfx::Hurt);
    }

    pub fn update_revive(&mut self) {
        if self.player.dead || !self.player.downed {
            return;
        }
        let now = self.real_time;
        let me = self.player.pos;
        let helper = self.net.as_ref().and_then(|n| {
            n.remotes
                .iter()
                .find(|r| {
                    r.present && r.state.alive() && r.render_pos(now).dist(me) < REVIVE_RADIUS
                })
                .map(|r| (r.name.clone(), r.state.flags & PF_MEDIC != 0))
        });
        let medic = helper.as_ref().map(|h| h.1).unwrap_or(false);
        let helper = helper.map(|h| h.0);
        let p = &mut self.player;
        p.bleed -= DT;
        if helper.is_some() {
            p.revive += DT / REVIVE_TIME * if medic { 2.0 } else { 1.0 };
        } else {
            p.revive = (p.revive - DT * 0.5).max(0.0);
        }
        if p.revive >= 1.0 {
            p.downed = false;
            p.revive = 0.0;
            p.hp = p.max_hp * 0.35;
            p.hp_ghost = p.hp;
            p.iframes = 2.5;
            let pos = p.pos;
            self.toast = Some((format!("Revived by {}!", helper.unwrap_or_default()), 2.0));
            self.ring(pos, 120.0, 0.5, "#9cffb8", 6.0);
            self.push_sfx(Sfx::Heal, pos);
        } else if p.bleed <= 0.0 {
            p.dead = true;
            p.downed = false;
            self.banner = Some(Banner {
                title: "YOU BLED OUT".into(),
                sub: "Spectating your team…".into(),
                t: 0.0,
                dur: 3.0,
                color: "#ff6a6a",
            });
        }
    }

    /// Host: everyone down or dead ends the run for all.
    pub fn check_team_wipe(&mut self) {
        if !self.is_host()
            || self.phase != Phase::Playing
                && self.phase != Phase::LevelUp
                && self.phase != Phase::Paused
        {
            return;
        }
        let anyone_up = self.player_alive()
            || self
                .net
                .as_ref()
                .is_some_and(|n| n.remotes.iter().any(|r| r.present && r.state.alive()));
        if !anyone_up {
            if let Some(net) = &mut self.net {
                if !net.game_over_sent {
                    net.game_over_sent = true;
                    net.event(|w| w.u8(E_GAMEOVER));
                }
            }
            self.enter_dying();
        }
    }

    /// The run is lost for everyone (the Siege crystal shattered).
    pub fn check_team_wipe_forced(&mut self) {
        if let Some(net) = &mut self.net {
            if !net.game_over_sent {
                net.game_over_sent = true;
                net.event(|w| w.u8(E_GAMEOVER));
            }
        }
        self.enter_dying();
    }

    /// Camera target: ourselves, or a living teammate while spectating.
    pub fn camera_pos(&self, own: V2) -> V2 {
        if !self.player.dead {
            return own;
        }
        let now = self.real_time;
        self.net
            .as_ref()
            .and_then(|n| {
                n.remotes
                    .iter()
                    .find(|r| r.present && !r.state.dead())
                    .map(|r| r.render_pos(now))
            })
            .unwrap_or(own)
    }

    /// Client-side zombie interpolation factor between snapshots.
    pub fn za(&self, a: f32) -> f32 {
        match &self.net {
            Some(n) if n.role == Role::Client => {
                ((self.real_time - n.last_snap_at) / n.snap_interval).clamp(0.0, 1.0) as f32
            }
            _ => a,
        }
    }

    // ------------------------------------------------------------ client hits
    /// Client: show the hit instantly and report it to the host. Kills are
    /// predicted locally (gore now), credited when the host confirms.
    pub fn client_hit(&mut self, j: usize, dmg: f32, dir: V2, knock: f32, crit: bool) -> bool {
        let (uid, pos, dead, kind, r) = {
            let z = &mut self.zombies[j];
            if z.dead {
                return false;
            }
            z.hp -= dmg;
            z.flash = 1.0;
            (z.uid, z.pos, z.hp <= 0.0, z.kind, z.r)
        };
        if let Some(net) = &mut self.net {
            net.out_hits.push(Hit {
                uid,
                dmg,
                dir,
                knock,
                crit,
                fx: 0,
            });
        }
        self.damage_dealt += dmg as f64;
        let (col, size) = if crit {
            ("#ffcf3a", 20.0)
        } else {
            ("#ffffff", 14.0)
        };
        self.push_text(pos, format!("{}", dmg.round() as i32), col, size);
        self.push_sfx(if crit { Sfx::Crit } else { Sfx::Hit }, pos);
        if dead {
            self.zombies[j].dead = true;
            self.predicted.push((uid, self.real_time));
            self.add_corpse(j);
            self.kill_effects(pos, kind, r);
            if crit {
                self.headshot(pos);
            }
        }
        self.ob_on_hit(pos, dead, crit);
        dead
    }

    // ------------------------------------------------------------ io
    /// Called once per frame: apply received messages, send our updates.
    pub fn net_tick(&mut self) {
        if self.net.is_none() {
            return;
        }
        let inbox = std::mem::take(&mut self.net_in);
        for (from, data) in inbox {
            self.handle_msg(from, &data);
        }
        let now = self.real_time;
        let host = self.is_host();
        let me = self.my_slot();
        // remote shot visuals
        self.spawn_remote_shots();
        let mut out: Vec<(u8, bool, Vec<u8>)> = Vec::new();
        if host {
            let clients: Vec<u8> = self
                .net
                .as_ref()
                .map(|n| {
                    n.remotes
                        .iter()
                        .filter(|r| r.present)
                        .map(|r| r.state.slot)
                        .collect()
                })
                .unwrap_or_default();
            let (snap_due, gems_due) = {
                let n = self.net.as_mut().unwrap();
                let s = now - n.snap_t >= 1.0 / SNAP_HZ;
                let g = now - n.gems_t >= 1.0 / GEMS_HZ;
                if s {
                    n.snap_t = now;
                }
                if g {
                    n.gems_t = now;
                }
                (s, g)
            };
            if snap_due && !clients.is_empty() {
                let snap = self.encode_snapshot();
                for &c in &clients {
                    out.push((c, false, snap.clone()));
                }
            }
            if gems_due && !clients.is_empty() {
                let gems = self.encode_gems();
                for &c in &clients {
                    out.push((c, false, gems.clone()));
                }
            }
            if let Some(ev) = self.net.as_mut().unwrap().take_events() {
                for &c in &clients {
                    out.push((c, true, ev.clone()));
                }
            }
            let cp_due = {
                let n = self.net.as_mut().unwrap();
                let d = now - n.checkpoint_t >= 1.0 / CHECKPOINT_HZ;
                if d {
                    n.checkpoint_t = now;
                }
                d
            };
            if cp_due && !clients.is_empty() {
                let cp = self.encode_checkpoint();
                for &c in &clients {
                    out.push((c, true, cp.clone()));
                }
            }
        } else {
            let due = {
                let n = self.net.as_mut().unwrap();
                let d = now - n.state_t >= 1.0 / STATE_HZ;
                if d {
                    n.state_t = now;
                }
                d
            };
            if due {
                let mut w = W::new();
                w.u8(MSG_PSTATE);
                write_player(&mut w, &self.my_state());
                out.push((0, false, w.0));
            }
            let hits = std::mem::take(&mut self.net.as_mut().unwrap().out_hits);
            if !hits.is_empty() {
                let mut w = W::new();
                w.u8(MSG_HITS);
                w.u16(hits.len().min(u16::MAX as usize) as u16);
                for h in hits.iter().take(u16::MAX as usize) {
                    w.u32(h.uid);
                    w.f32(h.dmg);
                    w.angle(h.dir.angle());
                    w.u8((h.knock / 4.0).clamp(0.0, 255.0) as u8);
                    w.u8(h.crit as u8 | h.fx);
                }
                out.push((0, true, w.0));
            }
            // forget stale kill predictions
            self.predicted.retain(|(_, t)| now - *t < 1.5);
        }
        let _ = me;
        self.net_out.extend(out);
        self.check_team_wipe();
    }

    pub fn my_state(&self) -> NetPlayer {
        let p = &self.player;
        let mut flags = PF_CONNECTED;
        if p.downed {
            flags |= PF_DOWNED;
        }
        if p.dead {
            flags |= PF_DEAD;
        }
        if p.dash_t > 0.0 {
            flags |= PF_DASH;
        }
        if p.frenzy > 0.0 {
            flags |= PF_FRENZY;
        }
        if p.hurt_flash > 0.3 {
            flags |= PF_HURT;
        }
        if self.kit() == crate::content::Kit::HealPulse {
            flags |= PF_MEDIC;
        }
        if self.ability_t > 0.0 {
            flags |= PF_ABILITY;
        }
        NetPlayer {
            slot: self.my_slot(),
            pos: p.pos,
            vel: p.vel,
            aim: p.aim,
            hp: p.hp,
            max_hp: p.max_hp,
            flags,
            revive: p.revive,
            shots: p.shots,
            level: p.level.min(255) as u8,
            skin: self.skin as u8,
            hat: self.hat as u8,
            tracer: self.tracer as u8,
            gun: self.gun as u8,
            kills: self.kills,
            score: self.score.min(u32::MAX as u64) as u32,
            downs: self.downs.min(255) as u8,
            dmg: self.damage_dealt.min(u32::MAX as f64) as u32,
        }
    }

    fn set_remote(&mut self, p: NetPlayer) {
        let now = self.real_time;
        let me = self.my_slot();
        if p.slot == me {
            return;
        }
        if let Some(net) = &mut self.net {
            let r = &mut net.remotes[p.slot as usize];
            let first = r.recv_t == 0.0;
            r.prev_pos = if first { p.pos } else { r.render_pos(now) };
            r.interval = if first {
                0.05
            } else {
                (now - r.recv_t).clamp(0.02, 0.2)
            };
            r.recv_t = now;
            r.walk += p.vel.len() * r.interval as f32 * 0.06;
            r.present = true;
            r.state = p;
        }
    }

    /// Teammates' shots are cosmetic here (they deal damage on their own client).
    fn spawn_remote_shots(&mut self) {
        let now = self.real_time;
        let mut volleys: Vec<(V2, f32, usize, u8, u32)> = Vec::new();
        if let Some(net) = &mut self.net {
            for r in net.remotes.iter_mut() {
                if !r.present {
                    continue;
                }
                let n = r.state.shots.wrapping_sub(r.last_shots).min(4) as u32;
                r.last_shots = r.state.shots;
                r.recoil = (r.recoil - 0.1).max(0.0);
                if n > 0 && r.state.alive() {
                    r.recoil = 1.0;
                    volleys.push((
                        r.render_pos(now),
                        r.state.aim,
                        r.state.gun as usize,
                        r.state.tracer,
                        n,
                    ));
                }
            }
        }
        for (pos, aim, gun, tracer, n) in volleys {
            let g = GUNS[gun.min(GUNS.len() - 1)];
            for _ in 0..n {
                let pellets = g.pellets.max(1) as i32;
                for k in 0..pellets {
                    let f = if pellets > 1 {
                        k as f32 / (pellets - 1) as f32 - 0.5
                    } else {
                        0.0
                    };
                    let a = aim + f * g.spread + self.rng.range(-g.jitter, g.jitter + 1e-4);
                    let dir = V2::from_angle(a);
                    let start = pos + dir * g.len;
                    self.bullets.push(Bullet {
                        pos: start,
                        prev: start - dir * 6.0,
                        vel: dir * 920.0 * g.speed,
                        dmg: 0.0,
                        life: 0.62 * g.life,
                        pierce: 0,
                        bounces: 0,
                        r: g.bullet_r,
                        crit: false,
                        last_hit: 0,
                        dead: false,
                        blast: 0.0,
                        knock: 0.0,
                        cosmetic: true,
                        tracer,
                    });
                }
                self.push_sfx(if g.heavy { Sfx::ShootHeavy } else { Sfx::Shoot }, pos);
            }
        }
    }

    fn handle_msg(&mut self, from: u8, data: &[u8]) {
        let mut r = R::new(data);
        let Some(kind) = r.u8() else { return };
        let host = self.is_host();
        match kind {
            MSG_PSTATE if host => {
                if let Some(mut p) = read_player(&mut r) {
                    p.slot = from; // trust the channel, not the payload
                    self.set_remote(p);
                }
            }
            MSG_HITS if host => {
                let Some(n) = r.u16() else { return };
                for _ in 0..n {
                    let (Some(uid), Some(dmg), Some(a), Some(knock), Some(fl)) =
                        (r.u32(), r.f32(), r.angle(), r.u8(), r.u8())
                    else {
                        return;
                    };
                    let dmg = dmg.clamp(0.0, 1.0e6);
                    if let Some(j) = self.zombies.iter().position(|z| z.uid == uid && !z.dead) {
                        let power = self.elem_power();
                        for e in [Elem::Burn, Elem::Chill, Elem::Shock, Elem::Poison] {
                            if fl & e.bit() != 0 && !self.zombies[j].dead {
                                self.afflict(j, e, power);
                            }
                        }
                        if self.zombies[j].dead || dmg <= 0.0 {
                            continue;
                        }
                        self.hit_slot = from;
                        self.damage_zombie(
                            j,
                            dmg,
                            V2::from_angle(a),
                            knock as f32 * 4.0,
                            fl & 1 != 0,
                        );
                        self.hit_slot = self.my_slot();
                    }
                }
            }
            MSG_HELLO => {
                let theirs = r.u16().unwrap_or(0);
                if theirs != PROTOCOL {
                    self.ui_out
                        .push(("netversion", format!("{}:{}", PROTOCOL, theirs)));
                }
            }
            MSG_READY if host => self.mark_ready(from),
            MSG_PING if host => {
                let (Some(k), Some(p)) = (r.u8(), r.v2()) else { return };
                self.relay_ping(from, k, p);
            }
            MSG_BUILD if host => {
                let (Some(k), Some(p)) = (r.u8(), r.v2()) else { return };
                if let Some(kind) = crate::modes::siege::DefKind::from_u8(k) {
                    self.add_defense(kind, p, 1);
                }
            }
            MSG_DEFENSE if host => {
                let (Some(k), Some(uid)) = (r.u8(), r.u32()) else { return };
                self.siege_world_pub(k, uid);
            }
            MSG_WORLD if host => {
                let (Some(kind), Some(idx)) = (r.u8(), r.u8()) else { return };
                self.world_change(kind, idx);
            }
            MSG_CHECKPOINT if !host => self.apply_checkpoint(&mut r),
            MSG_SNAP if !host => self.apply_snapshot(&mut r),
            MSG_GEMS if !host => self.apply_gems(&mut r),
            MSG_EVENTS if !host => self.apply_events(&mut r),
            _ => {}
        }
    }

    // ------------------------------------------------------------ host encode
    fn encode_snapshot(&mut self) -> Vec<u8> {
        let mut w = W::new();
        w.u8(MSG_SNAP);
        let tick = {
            let n = self.net.as_mut().unwrap();
            n.tick = n.tick.wrapping_add(1);
            n.tick
        };
        w.u32(tick);
        w.u16(self.wave.n.min(u16::MAX as u32) as u16);
        w.u16(self.remaining().min(u16::MAX as u32) as u16);
        w.f32(self.wave.intermission.max(0.0));
        // players: host + every present remote
        let mut players = vec![self.my_state()];
        if let Some(n) = &self.net {
            players.extend(
                n.remotes
                    .iter()
                    .filter(|r| r.present)
                    .map(|r| r.state.clone()),
            );
        }
        w.u8(players.len() as u8);
        for p in &players {
            write_player(&mut w, p);
        }
        // the zombies nearest to any player, positions relative to an origin
        let origin = v2(self.player.pos.x.round(), self.player.pos.y.round());
        let targets = self.targets();
        let near = |z: &Zombie| targets.iter().map(|t| t.1.dist2(z.pos)).fold(f32::MAX, f32::min);
        let mut zs: Vec<&Zombie> = self.zombies.iter().filter(|z| !z.dead).collect();
        if zs.len() > 450 {
            zs.sort_by(|a, b| near(a).partial_cmp(&near(b)).unwrap_or(std::cmp::Ordering::Equal));
            zs.truncate(450);
        }
        w.v2(origin);
        w.u16(zs.len() as u16);
        for z in zs {
            w.u32(z.uid);
            w.u8(z.kind as u8);
            w.u8(z.boss_var as u8);
            w.pos16(z.pos, origin);
            w.angle(z.face);
            w.u8(((z.hp / z.max_hp.max(1.0)).clamp(0.0, 1.0) * 255.0) as u8);
            w.f32(z.max_hp);
            let mut fl = 0u8;
            if z.flash > 0.3 {
                fl |= 1;
            }
            fl |= (z.state.min(3)) << 2;
            fl |= z.st.bits();
            w.u8(fl);
            w.u8(z.affix as u8 | if z.shield > 0.0 { 0x80 } else { 0 });
            w.angle(z.charge_dir.angle());
            let aux = match z.kind {
                ZKind::Exploder => z.fuse.max(0.0) * 200.0,
                _ => z.timer.max(0.0) * 100.0,
            };
            w.u8(aux.clamp(0.0, 255.0) as u8);
        }
        let es: Vec<(V2, f32)> = self.eshots.iter().take(255).map(|s| (s.pos, s.r)).collect();
        w.u8(es.len() as u8);
        for (p, r) in es {
            w.v2(p);
            w.u8(r.clamp(0.0, 255.0) as u8);
        }
        let pk: Vec<(u8, V2, f32)> = self
            .pickups
            .iter()
            .take(255)
            .map(|p| (p.kind as u8, p.pos, p.t))
            .collect();
        w.u8(pk.len() as u8);
        for (k, p, t) in pk {
            w.u8(k);
            w.v2(p);
            w.f32(t);
        }
        w.0
    }

    fn encode_gems(&self) -> Vec<u8> {
        let mut w = W::new();
        w.u8(MSG_GEMS);
        let n = self.gems.len().min(1200);
        w.u16(n as u16);
        for g in self.gems.iter().take(n) {
            w.v2(g.pos);
            w.u16(g.value.min(u16::MAX as u32) as u16);
        }
        w.0
    }

    // ------------------------------------------------------------ client decode
    fn apply_snapshot(&mut self, r: &mut R) {
        let now = self.real_time;
        let parsed = (|| {
            let _tick = r.u32()?;
            let wave = r.u16()?;
            let remaining = r.u16()?;
            let inter = r.f32()?;
            let np = r.u8()?;
            let mut players = Vec::new();
            for _ in 0..np {
                players.push(read_player(r)?);
            }
            let origin = r.v2()?;
            let nz = r.u16()?;
            let mut zs = Vec::with_capacity(nz as usize);
            for _ in 0..nz {
                zs.push((
                    r.u32()?,
                    r.u8()?,
                    r.u8()?,
                    r.pos16(origin)?,
                    r.angle()?,
                    r.u8()? as f32 / 255.0,
                    r.f32()?,
                    r.u8()?,
                    r.u8()?,
                    r.angle()?,
                    r.u8()?,
                ));
            }
            let ne = r.u8()?;
            let mut es = Vec::new();
            for _ in 0..ne {
                es.push((r.v2()?, r.u8()? as f32));
            }
            let npk = r.u8()?;
            let mut pks = Vec::new();
            for _ in 0..npk {
                pks.push((r.u8()?, r.v2()?, r.f32()?));
            }
            Some((wave, remaining, inter, players, zs, es, pks))
        })();
        let Some((wave, remaining, inter, players, zs, es, pks)) = parsed else {
            return;
        };
        if let Some(n) = &mut self.net {
            n.snap_interval = if n.last_snap_at > 0.0 {
                (now - n.last_snap_at).clamp(0.02, 0.2)
            } else {
                0.05
            };
            n.last_snap_at = now;
        }
        self.wave.n = wave as u32;
        self.net_remaining = remaining as u32;
        self.wave.intermission = inter;
        for p in players {
            self.set_remote(p);
        }
        // zombies: keep existing replicas (for smooth interpolation), add new, drop missing
        let predicted: Vec<u32> = self.predicted.iter().map(|p| p.0).collect();
        let mut by_uid: std::collections::HashMap<u32, usize> = self
            .zombies
            .iter()
            .enumerate()
            .filter(|(_, z)| !z.dead)
            .map(|(i, z)| (z.uid, i))
            .collect();
        let mut next: Vec<Zombie> = Vec::with_capacity(zs.len());
        let dt = self
            .net
            .as_ref()
            .map(|n| n.snap_interval as f32)
            .unwrap_or(0.05);
        for (uid, kind, var, pos, face, hp, max_hp, fl, ax, charge, aux) in zs {
            if predicted.contains(&uid) {
                continue; // we already killed it locally; host confirmation is on its way
            }
            let kind = zkind_from(kind);
            let var = boss_from(var);
            let mut z = match by_uid.remove(&uid) {
                Some(i) => {
                    let mut z =
                        std::mem::replace(&mut self.zombies[i], replica(uid, kind, var, pos));
                    z.prev = z.prev.lerp(z.pos, self.za(1.0)); // continue from where it's drawn
                    z.vel = (pos - z.pos) * (1.0 / dt.max(0.01));
                    z.anim += (pos - z.pos).len() / 45.0;
                    z.pos = pos;
                    z
                }
                None => replica(uid, kind, var, pos),
            };
            z.face = face;
            z.max_hp = max_hp.max(1.0);
            z.hp = hp * z.max_hp; // sent as a fraction
            if fl & 1 != 0 {
                z.flash = z.flash.max(0.6);
            }
            z.st = crate::status::Status::from_bits(fl);
            z.affix = crate::elites::Affix::from_u8(ax & 0x7f);
            z.shield = if ax & 0x80 != 0 { 1.0 } else { 0.0 };
            z.state = (fl >> 2) & 3;
            z.hidden = kind == ZKind::Burrower && z.state < 2;
            z.charge_dir = V2::from_angle(charge);
            match kind {
                ZKind::Exploder => z.fuse = aux as f32 / 200.0,
                _ => z.timer = aux as f32 / 100.0,
            }
            next.push(z);
        }
        self.zombies = next;
        self.eshots = es
            .into_iter()
            .map(|(p, rad)| EShot {
                pos: p,
                prev: p,
                vel: V2::ZERO,
                life: 1.0,
                dmg: 0.0,
                dead: false,
                turn: 0.0,
                r: rad,
                grazed: false,
            })
            .collect();
        self.pickups = pks
            .into_iter()
            .map(|(k, p, t)| Pickup {
                pos: p,
                kind: match k {
                    1 => PickKind::Vacuum,
                    2 => PickKind::Frenzy,
                    3 => PickKind::Chest,
                    4 => PickKind::MaxAmmo,
                    5 => PickKind::InstaKill,
                    6 => PickKind::DoublePoints,
                    7 => PickKind::Nuke,
                    8 => PickKind::Carpenter,
                    _ => PickKind::Heart,
                },
                t,
                dead: false,
            })
            .collect();
    }

    fn apply_gems(&mut self, r: &mut R) {
        let Some(n) = r.u16() else { return };
        let mut gems = Vec::with_capacity(n as usize);
        for i in 0..n {
            let (Some(p), Some(v)) = (r.v2(), r.u16()) else {
                return;
            };
            let prev = self.gems.get(i as usize).map(|g| g.pos).unwrap_or(p);
            let prev = if prev.dist(p) < 80.0 { prev } else { p };
            gems.push(Gem {
                mana: false,
                pos: p,
                prev,
                vel: V2::ZERO,
                value: v as u32,
                t: 1.0,
                magnet: false,
                dead: false,
            });
        }
        self.gems = gems;
    }

    fn apply_events(&mut self, r: &mut R) {
        let Some(n) = r.u16() else { return };
        let me = self.my_slot();
        for _ in 0..n {
            let Some(e) = r.u8() else { return };
            match e {
                E_HURT => {
                    let (Some(slot), Some(dmg), Some(from)) = (r.u8(), r.f32(), r.v2()) else {
                        return;
                    };
                    if slot == me {
                        self.hurt_player(dmg, from);
                    }
                }
                E_KILL => {
                    let (Some(uid), Some(pos), Some(kind), Some(killer)) =
                        (r.u32(), r.v2(), r.u8(), r.u8())
                    else {
                        return;
                    };
                    let kind = zkind_from(kind);
                    let was_predicted = self.predicted.iter().any(|p| p.0 == uid);
                    if let Some(j) = self.zombies.iter().position(|z| z.uid == uid) {
                        let r_ = self.zombies[j].r;
                        self.zombies[j].dead = true;
                        if !was_predicted {
                            self.add_corpse(j);
                            self.kill_effects(pos, kind, r_);
                        }
                    } else if !was_predicted {
                        self.kill_effects(pos, kind, 15.0);
                    }
                    if killer == me {
                        self.credit_kill();
                    }
                    self.note_kill(killer, kind, pos);
                    if kind == ZKind::Boss {
                        let bonus = (50.0 * self.difficulty().coins) as u32;
                        self.coins += bonus;
                        self.banner = Some(Banner {
                            title: "BOSS SLAIN".into(),
                            sub: format!("+{} coins for the whole team", bonus),
                            t: 0.0,
                            dur: 2.2,
                            color: "#ff9cff",
                        });
                        self.add_shake(0.6);
                    }
                }
                E_XP => {
                    let Some(v) = r.f32() else { return };
                    self.player.xp += v.clamp(0.0, 1.0e5);
                    let p = self.player.pos;
                    self.push_sfx(Sfx::Pickup, p);
                }
                E_WAVE_START => {
                    let (Some(wn), Some(total), Some(boss)) = (r.u16(), r.u16(), r.u8()) else {
                        return;
                    };
                    self.wave.n = wn as u32;
                    self.banner = Some(Banner {
                        title: if boss != 0 {
                            format!("WAVE {} — BOSS", wn)
                        } else {
                            format!("WAVE {}", wn)
                        },
                        sub: format!("{} undead approach", total),
                        t: 0.0,
                        dur: 2.4,
                        color: if boss != 0 { "#ff6a8a" } else { "#e8f0ff" },
                    });
                    self.audio.play(Sfx::WaveStart);
                }
                E_WAVE_CLEAR => {
                    let (Some(wn), Some(coins)) = (r.u16(), r.u16()) else {
                        return;
                    };
                    self.coins += coins as u32;
                    if self.player_alive() {
                        let heal = self.player.max_hp * 0.1;
                        self.player.hp = (self.player.hp + heal).min(self.player.max_hp);
                    }
                    self.banner = Some(Banner {
                        title: format!("WAVE {} CLEARED", wn),
                        sub: format!("+{} coins · next wave incoming", coins),
                        t: 0.0,
                        dur: 2.6,
                        color: "#9cffb8",
                    });
                    self.audio.play(Sfx::WaveClear);
                    self.respawn_spectator();
                }
                E_HEAL => {
                    let (Some(slot), Some(amt)) = (r.u8(), r.f32()) else {
                        return;
                    };
                    if slot == me && self.player_alive() {
                        self.player.hp = (self.player.hp + amt).min(self.player.max_hp);
                        let p = self.player.pos;
                        self.push_text(p, format!("+{}", amt as i32), "#ff5c7a", 22.0);
                        self.push_sfx(Sfx::Heal, p);
                    }
                }
                E_FRENZY => {
                    let Some(slot) = r.u8() else { return };
                    if slot == me {
                        self.player.frenzy = 8.0;
                        self.toast = Some(("FRENZY! Double fire rate".into(), 1.6));
                    }
                }
                E_CHEST => {
                    let Some(slot) = r.u8() else { return };
                    if slot == me {
                        self.pending_chests += 1;
                    }
                }
                E_POWERUP => {
                    let Some(k) = r.u8() else { return };
                    let kind = match k {
                        4 => PickKind::MaxAmmo,
                        5 => PickKind::InstaKill,
                        6 => PickKind::DoublePoints,
                        7 => PickKind::Nuke,
                        8 => PickKind::Carpenter,
                        _ => return,
                    };
                    self.apply_powerup(kind, false);
                }
                E_BOARDS => {
                    let (Some(i), Some(b)) = (r.u8(), r.u8()) else { return };
                    if let Some(w) = self.ob.as_mut().and_then(|o| o.windows.get_mut(i as usize)) {
                        w.boards = b;
                    }
                }
                E_WORLD => {
                    let (Some(kind), Some(idx)) = (r.u8(), r.u8()) else { return };
                    self.apply_world(kind, idx);
                }
                E_MANA => {
                    let (Some(slot), Some(v)) = (r.u8(), r.u32()) else { return };
                    if slot == me {
                        self.gain_mana(v.min(10_000));
                    }
                }
                E_DEFENSE => {
                    let (Some(uid), Some(k), Some(p), Some(tier)) = (r.u32(), r.u8(), r.v2(), r.u8()) else { return };
                    if let (Some(kind), Some(s)) = (crate::modes::siege::DefKind::from_u8(k), self.siege.as_mut()) {
                        if !s.defenses.iter().any(|d| d.uid == uid) {
                            let hp = kind.base_hp() * (1.0 + 0.6 * (tier as f32 - 1.0));
                            s.defenses.push(crate::modes::siege::Defense { kind, pos: p, tier, hp, max_hp: hp, cd: 0.5, aim: 0.0, flash: 0.0, uid });
                        }
                    }
                }
                E_DEFCHANGE => {
                    let (Some(k), Some(uid)) = (r.u8(), r.u32()) else { return };
                    self.apply_defense_change(k, uid);
                }
                E_CRYSTAL => {
                    let Some(hp) = r.f32() else { return };
                    if let Some(s) = self.siege.as_mut() {
                        if hp < s.crystal_hp {
                            s.crystal_flash = 1.0;
                        }
                        s.crystal_hp = hp;
                    }
                }
                E_PING => {
                    let (Some(slot), Some(k), Some(p)) = (r.u8(), r.u8(), r.v2()) else { return };
                    if slot != me {
                        self.add_ping(slot, k, p);
                    }
                }
                E_GAMEOVER => self.enter_dying(),
                E_VICTORY => {
                    let Some(coins) = r.u16() else { return };
                    self.coins += coins as u32;
                    match self.mode {
                        Mode::Siege => self.siege_victory(),
                        _ => self.campaign_victory(),
                    }
                }
                E_BLAST => {
                    let (Some(p), Some(rad)) = (r.v2(), r.f32()) else {
                        return;
                    };
                    self.ring(p, rad, 0.35, "#ffb24a", 8.0);
                    self.push_sfx(Sfx::Explosion, p);
                    let d = self.player.pos.dist(p);
                    self.add_shake(if d < 400.0 { 0.3 } else { 0.08 });
                }
                E_PULL => {
                    let (Some(slot), Some(v)) = (r.u8(), r.v2()) else { return };
                    if slot == me && self.player.dash_t <= 0.0 {
                        self.player.vel = v.clamp_len(1500.0);
                    }
                }
                E_BOSS => {
                    let Some(v) = r.u8() else { return };
                    self.boss_intro(boss_from(v));
                    self.audio.play(Sfx::BossRoar);
                    self.add_shake(0.5);
                }
                _ => return,
            }
        }
    }
}

fn zkind_from(k: u8) -> ZKind {
    ZKind::from_u8(k)
}

fn boss_from(v: u8) -> BossVar {
    BossVar::from_u8(v)
}

/// A client-side zombie built from a snapshot entry.
fn replica(uid: u32, kind: ZKind, var: BossVar, pos: V2) -> Zombie {
    let def = kind.def();
    let (hp, speed, r, dmg, xp) = (def.hp, def.speed, def.r, def.dmg, def.xp);
    let r = if kind == ZKind::Boss {
        var.stats().1
    } else {
        r
    };
    Zombie {
        pos,
        prev: pos,
        vel: V2::ZERO,
        kind,
        hp,
        max_hp: hp,
        speed,
        r,
        dmg,
        xp,
        flash: 0.0,
        anim: (uid % 97) as f32 * 0.1,
        attack_cd: 0.0,
        st: crate::status::Status::default(),
        affix: crate::elites::Affix::None,
        shield: 0.0,
        rage_t: 0.0,
        win: -1,
        dummy: false,
        phase: 0,
        age: 0.0,
        hidden: false,
        timer: 1.0,
        state: 0,
        charge_dir: V2::ZERO,
        fuse: 0.0,
        blade_cd: 0.0,
        dash_mark: 0,
        dead: false,
        uid,
        face: 0.0,
        strafe: 1.0,
        tint: (uid % 4) as f32 * 0.25,
        boss_var: var,
        special_t: 0.0,
        last_hitter: 0,
    }
}

#[allow(dead_code)]
fn _unused() {
    let _ = INFINITE_BOSSES;
    let _ = content::DIFFICULTIES.len();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pump(
        host: &mut Game,
        client: &mut Game,
        t: &mut f64,
        frames: usize,
        mut each: impl FnMut(&mut Game, &mut Game, usize),
    ) {
        for f in 0..frames {
            each(host, client, f);
            *t += 16.7;
            host.frame(*t);
            client.frame(*t);
            // deliver host -> client (slot 1) and client -> host
            for (to, _rel, data) in std::mem::take(&mut host.net_out) {
                if to == 1 {
                    client.net_in.push((0, data));
                }
            }
            for (to, _rel, data) in std::mem::take(&mut client.net_out) {
                if to == 0 {
                    host.net_in.push((1, data));
                }
            }
        }
    }

    fn pair() -> (Game, Game) {
        let mut host = Game::new(1);
        let mut client = Game::new(2);
        for g in [&mut host, &mut client] {
            g.screen = v2(1280.0, 720.0);
            g.zoom = 1.0;
            g.spawn_r = 700.0;
        }
        host.start_coop("role=host;me=0;names=Host,Guest;present=1,1,0;diff=survivor");
        client.start_coop(
            "role=client;me=1;names=Host,Guest;present=1,1,0;diff=survivor;gun=shotgun",
        );
        (host, client)
    }

    #[test]
    fn host_and_client_share_a_world() {
        let (mut host, mut client) = pair();
        let mut t = 0.0;
        // let wave 1 spawn and approach
        pump(&mut host, &mut client, &mut t, 400, |h, c, _| {
            h.player.hp = 1.0e6;
            c.player.hp = 1.0e6;
        });
        assert!(host.wave.n >= 1, "host runs waves");
        assert!(
            !client.zombies.is_empty(),
            "client replicates zombies from snapshots"
        );
        let hz: Vec<u32> = host.zombies.iter().map(|z| z.uid).collect();
        assert!(client
            .zombies
            .iter()
            .all(|z| hz.contains(&z.uid) || client.predicted.iter().any(|p| p.0 == z.uid)));
        // host sees the client player where the client says it is
        let r = &host.net.as_ref().unwrap().remotes[1];
        assert!(r.present && r.state.pos.dist(client.player.pos) < 60.0);
        // the client's own shooting kills host zombies and is credited to the client
        pump(&mut host, &mut client, &mut t, 1500, |h, c, _| {
            h.player.hp = 1.0e6;
            c.player.hp = 1.0e6;
            h.player.fire_cd = 1.0e3; // host holds fire so kills come from the client
        });
        assert!(client.kills > 0, "client kills are confirmed by the host");
        assert!(
            client.player.xp > 0.0 || client.player.level > 1,
            "team xp reaches the client"
        );
    }

    #[test]
    fn dropped_player_rejoins_mid_match_synced_and_respawns_on_wave_clear() {
        let mut host = Game::new(1);
        let mut client = Game::new(2);
        for g in [&mut host, &mut client] {
            g.screen = v2(1280.0, 720.0);
            g.zoom = 1.0;
        }
        let o = "names=Host,Guest;present=1,1,0;diff=rookie;mode=siege";
        host.start_coop(&format!("role=host;me=0;{o}"));
        client.start_coop(&format!("role=client;me=1;{o}"));
        let mut t = 0.0;
        pump(&mut host, &mut client, &mut t, 30, |_, _, _| {});
        // the guest drops; the host builds a tower while they're gone
        host.peer_left(1);
        assert!(!host.net.as_ref().unwrap().remotes[1].present);
        let at = host.player.pos + v2(120.0, 0.0);
        host.add_defense(crate::modes::siege::DefKind::Tower, at, 2);
        assert_eq!(host.siege.as_ref().unwrap().defenses.len(), 1);
        // they come back in the same slot as a late joiner
        let mut back = Game::new(3);
        back.screen = v2(1280.0, 720.0);
        back.start_coop(&format!("role=client;me=1;{o};late=1"));
        assert!(back.player.dead && back.late_join, "late joiners spectate first");
        host.peer_joined(1, "Guest");
        assert!(host.net.as_ref().unwrap().remotes[1].present);
        pump(&mut host, &mut back, &mut t, 20, |_, _, _| {});
        let s = back.siege.as_ref().unwrap();
        assert_eq!(s.defenses.len(), 1, "the tower built while away is synced");
        assert_eq!(s.defenses[0].tier, 2);
        assert!(back.player.dead, "still spectating mid-wave");
        // the wave is cleared: the late joiner drops in next to the host
        host.wave.n = 1;
        host.wave_cleared();
        pump(&mut host, &mut back, &mut t, 10, |_, _, _| {});
        assert!(!back.player.dead && !back.late_join, "respawned on wave clear");
        assert!(back.player.hp > 0.0);
        // the host bled out earlier in a wave? they respawn on the next clear too
        host.player.dead = true;
        host.wave_cleared();
        assert!(!host.player.dead);
    }

    /// Routes messages through a star around `hub` (JS keys the host link as 0 on clients).
    fn pump_star(hub: &mut Game, leaves: &mut [&mut Game], t: &mut f64, frames: usize) {
        for _ in 0..frames {
            *t += 16.7;
            hub.frame(*t);
            for l in leaves.iter_mut() {
                l.frame(*t);
            }
            for (to, _rel, data) in std::mem::take(&mut hub.net_out) {
                if let Some(l) = leaves.iter_mut().find(|l| l.my_slot() == to) {
                    l.net_in.push((0, data));
                }
            }
            for l in leaves.iter_mut() {
                let from = l.my_slot();
                for (to, _rel, data) in std::mem::take(&mut l.net_out) {
                    if to == 0 {
                        hub.net_in.push((from, data));
                    }
                }
            }
        }
    }

    #[test]
    fn host_migration_lowest_slot_takes_over_and_the_match_continues() {
        let o = "names=Host,Ann,Ben;present=1,1,1;diff=rookie";
        let mut host = Game::new(1);
        let mut ann = Game::new(2);
        let mut ben = Game::new(3);
        for g in [&mut host, &mut ann, &mut ben] {
            g.screen = v2(1280.0, 720.0);
            g.zoom = 1.0;
            g.spawn_r = 700.0;
        }
        host.start_coop(&format!("role=host;me=0;{o}"));
        ann.start_coop(&format!("role=client;me=1;{o}"));
        ben.start_coop(&format!("role=client;me=2;{o}"));
        let mut t = 0.0;
        pump_star(&mut host, &mut [&mut ann, &mut ben], &mut t, 260);
        for g in [&mut host, &mut ann, &mut ben] {
            g.player.hp = 1.0e6;
            g.player.max_hp = 1.0e6;
        }
        pump_star(&mut host, &mut [&mut ann, &mut ben], &mut t, 60);
        assert!(host.wave.total > 0 && ann.wave.total == host.wave.total, "checkpoint mirrors the wave");
        assert!(!ann.zombies.is_empty(), "replicas exist");
        let wave = host.wave.n;
        // the host vanishes: Ann (lowest slot left) promotes, Ben follows her
        drop(host);
        ann.promote(0);
        ben.host_moved(0, 1);
        assert!(ann.is_host() && ann.net.as_ref().unwrap().host_slot == 1);
        // a spectator left alone respawns instead of wiping
        let mut solo = Game::new(9);
        solo.start_coop("role=client;me=1;names=Host,Ann;present=1,1,0;diff=rookie;late=1");
        solo.phase = Phase::Playing;
        assert!(solo.player.dead);
        solo.promote(0);
        assert!(!solo.player.dead, "a lone spectator who takes over respawns");
        let before: Vec<(u32, V2)> = ann.zombies.iter().map(|z| (z.uid, z.pos)).collect();
        pump_star(&mut ann, &mut [&mut ben], &mut t, 120);
        let moved = ann.zombies.iter().any(|z| before.iter().any(|b| b.0 == z.uid && b.1.dist(z.pos) > 5.0));
        assert!(moved || ann.zombies.is_empty(), "the new host simulates the horde");
        let bn = ben.net.as_ref().unwrap();
        assert!(bn.remotes[1].present && !bn.remotes[0].present, "Ben sees Ann as the host");
        assert!(bn.last_snap_at > 0.0, "Ben gets snapshots from the new host");
        // the wave still ends and the next one starts for both
        ann.wave.spawned = ann.wave.total;
        ann.wave.boss_pending = false;
        for z in ann.zombies.iter_mut() {
            z.dead = true;
        }
        let coins = ben.coins;
        pump_star(&mut ann, &mut [&mut ben], &mut t, 900);
        assert!(ann.wave.n > wave, "waves keep coming after the migration");
        assert!(ben.coins > coins, "Ben gets the wave-clear reward from the new host");
        assert_eq!(ben.wave.n, ann.wave.n);
    }

    #[test]
    fn campaign_coop_runs_the_hosts_chapter_and_shares_the_win() {
        let mut host = Game::new(1);
        let mut client = Game::new(2);
        for g in [&mut host, &mut client] {
            g.screen = v2(1280.0, 720.0);
            g.zoom = 1.0;
        }
        host.start_coop("role=host;me=0;names=Host,Guest;present=1,1,0;diff=rookie;chapter=2;mode=campaign");
        client.start_coop("role=client;me=1;names=Host,Guest;present=1,1,0;diff=rookie;mode=campaign;chapter=2");
        assert!(matches!(host.mode, Mode::Campaign(2)) && matches!(client.mode, Mode::Campaign(2)));
        let mut t = 0.0;
        pump(&mut host, &mut client, &mut t, 30, |_, _, _| {});
        // the host kills the last chapter boss: both players get the win
        host.wave.n = content::CHAPTERS[2].waves;
        host.wave.boss_pending = false;
        host.zombies.clear();
        let hooks = host.mode.hooks();
        hooks.on_kill(&mut host, ZKind::Boss, V2::ZERO);
        assert!(host.victory_t >= 0.0);
        let coins = client.coins;
        pump(&mut host, &mut client, &mut t, 10, |_, _, _| {});
        assert!(client.victory_t >= 0.0, "client replays the chapter win");
        assert!(client.coins > coins, "client gets the chapter bonus");
        // a client can't end the chapter on its own
        let mut solo = Game::new(3);
        solo.start_coop("role=client;me=1;names=Host,Guest;present=1,1,0;mode=campaign;chapter=0");
        solo.wave.n = 99;
        let hooks = solo.mode.hooks();
        hooks.on_kill(&mut solo, ZKind::Boss, V2::ZERO);
        assert!(solo.victory_t < 0.0);
    }

    #[test]
    fn downed_player_is_revived_by_teammate_and_wipe_ends_game() {
        let (mut host, mut client) = pair();
        let mut t = 0.0;
        pump(&mut host, &mut client, &mut t, 60, |_, _, _| {});
        // client goes down next to the host -> revived after ~3s
        client.player.iframes = 0.0;
        client.player.hp = 1.0;
        client.hurt_player(50.0, v2(0.0, 0.0));
        assert!(client.player.downed);
        pump(&mut host, &mut client, &mut t, 260, |h, c, _| {
            h.player.hp = 1.0e6;
            h.player.pos = c.player.pos + v2(20.0, 0.0);
        });
        assert!(
            !client.player.downed && !client.player.dead,
            "teammate revived the client"
        );
        // both down -> host ends the run for everyone
        for g in [&mut host, &mut client] {
            g.player.iframes = 0.0;
            g.player.hp = 1.0;
            g.hurt_player(50.0, v2(0.0, 0.0));
        }
        pump(&mut host, &mut client, &mut t, 30, |h, c, _| {
            h.player.pos = v2(-5000.0, 0.0);
            c.player.pos = v2(5000.0, 0.0);
        });
        assert!(matches!(host.phase, Phase::Dying | Phase::GameOver));
        assert!(
            matches!(client.phase, Phase::Dying | Phase::GameOver),
            "client got the game-over event"
        );
    }

    #[test]
    fn outbreak_coop_shares_power_and_powerups() {
        let mut host = Game::new(3);
        let mut client = Game::new(4);
        for g in [&mut host, &mut client] {
            g.screen = v2(1280.0, 720.0);
            g.zoom = 1.0;
            g.spawn_r = 700.0;
        }
        host.start_coop("role=host;me=0;names=Host,Guest;present=1,1,0;diff=rookie;mode=outbreak");
        client.start_coop("role=client;me=1;names=Host,Guest;present=1,1,0;diff=rookie;mode=outbreak");
        assert_eq!(host.mode, crate::modes::Mode::Outbreak);
        assert!(host.ob.is_some() && client.ob.is_some());
        let mut t = 0.0;
        // the client flips the power: the host hears about it
        client.world_change(crate::outbreak_interact::W_POWER, 0);
        pump(&mut host, &mut client, &mut t, 10, |_, _, _| {});
        assert!(host.ob.as_ref().unwrap().power, "host got the client's power switch");
        // a host power-up reaches the client
        host.apply_powerup(PickKind::DoublePoints, true);
        pump(&mut host, &mut client, &mut t, 10, |_, _, _| {});
        assert!(client.ob.as_ref().unwrap().double_t > 0.0);
    }

    #[test]
    fn pings_reach_the_squad() {
        let (mut host, mut client) = pair();
        let mut t = 0.0;
        pump(&mut host, &mut client, &mut t, 5, |_, _, _| {});
        client.input.mouse_seen = false;
        client.player.aim = 0.0;
        assert!(client.try_ping());
        pump(&mut host, &mut client, &mut t, 5, |_, _, _| {});
        assert!(host.pings.iter().any(|p| p.slot == 1), "host sees the client's ping");
        host.input.mouse_seen = false;
        host.try_ping();
        pump(&mut host, &mut client, &mut t, 5, |_, _, _| {});
        assert!(client.pings.iter().any(|p| p.slot == 0), "client sees the host's ping");
    }

    #[test]
    fn scoreboard_has_everyone() {
        let (mut host, mut client) = pair();
        let mut t = 0.0;
        client.kills = 42;
        client.score = 1234;
        pump(&mut host, &mut client, &mut t, 10, |_, _, _| {});
        let js = host.stats_json();
        assert!(js.contains("\"kills\":42") && js.contains("\"score\":1234"), "{js}");
        assert!(js.contains("\"me\":true"));
    }
}
