//! Co-op glue for `Game`: network I/O, snapshots, replicas, downed/revive.
//! See `net.rs` for the authority model and wire format.

use crate::audio::Sfx;
use crate::content::{self, BossVar, GUNS, INFINITE_BOSSES};
use crate::game::*;
use crate::math::*;
use crate::net::*;
use crate::upgrades::Up;

const SNAP_HZ: f64 = 20.0;
const STATE_HZ: f64 = 30.0;
const GEMS_HZ: f64 = 12.0;
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
        self.campaign = false;
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
        let n = net.players();
        self.net = Some(net);
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

    pub fn peer_left(&mut self, slot: u8) {
        let host_left = slot == 0 && self.is_client();
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
            self.toast = Some(("ULTIMATE READY — press Q".into(), 2.2));
        }
        let vl = self.ups.get(Up::Vampire);
        if vl > 0 && self.rng.chance(0.04 * vl as f32) && self.player_alive() {
            self.player.hp = (self.player.hp + 3.0).min(self.player.max_hp);
            let pp = self.player.pos;
            self.push_text(pp, "+3".into(), "#ff4d6d", 14.0);
        }
    }

    // ------------------------------------------------------------ downed / revive
    pub fn go_down(&mut self) {
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
                .map(|r| r.name.clone())
        });
        let p = &mut self.player;
        p.bleed -= DT;
        if helper.is_some() {
            p.revive += DT / REVIVE_TIME;
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
        let slow = false;
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
                slow,
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
            self.kill_effects(pos, kind, r);
        }
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
                    w.u8(h.crit as u8 | (h.slow as u8) << 1);
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
        let zs: Vec<&Zombie> = self.zombies.iter().filter(|z| !z.dead).take(600).collect();
        w.u16(zs.len() as u16);
        for z in zs {
            w.u32(z.uid);
            w.u8(z.kind as u8);
            w.u8(z.boss_var as u8);
            w.v2(z.pos);
            w.angle(z.face);
            w.f32(z.hp);
            w.f32(z.max_hp);
            let mut fl = 0u8;
            if z.flash > 0.3 {
                fl |= 1;
            }
            if z.slow_t > 0.0 {
                fl |= 2;
            }
            fl |= (z.state.min(3)) << 2;
            w.u8(fl);
            w.angle(z.charge_dir.angle());
            let aux = match z.kind {
                ZKind::Exploder => z.fuse.max(0.0) * 200.0,
                _ => z.timer.max(0.0) * 100.0,
            };
            w.u8(aux.clamp(0.0, 255.0) as u8);
        }
        let es: Vec<V2> = self.eshots.iter().take(255).map(|s| s.pos).collect();
        w.u8(es.len() as u8);
        for p in es {
            w.v2(p);
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
            let nz = r.u16()?;
            let mut zs = Vec::with_capacity(nz as usize);
            for _ in 0..nz {
                zs.push((
                    r.u32()?,
                    r.u8()?,
                    r.u8()?,
                    r.v2()?,
                    r.angle()?,
                    r.f32()?,
                    r.f32()?,
                    r.u8()?,
                    r.angle()?,
                    r.u8()?,
                ));
            }
            let ne = r.u8()?;
            let mut es = Vec::new();
            for _ in 0..ne {
                es.push(r.v2()?);
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
        for (uid, kind, var, pos, face, hp, max_hp, fl, charge, aux) in zs {
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
            z.hp = hp;
            z.max_hp = max_hp.max(1.0);
            if fl & 1 != 0 {
                z.flash = z.flash.max(0.6);
            }
            z.slow_t = if fl & 2 != 0 { 1.0 } else { 0.0 };
            z.state = (fl >> 2) & 3;
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
            .map(|p| EShot {
                pos: p,
                prev: p,
                vel: V2::ZERO,
                life: 1.0,
                dmg: 0.0,
                dead: false,
            })
            .collect();
        self.pickups = pks
            .into_iter()
            .map(|(k, p, t)| Pickup {
                pos: p,
                kind: match k {
                    1 => PickKind::Vacuum,
                    2 => PickKind::Frenzy,
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
                            self.kill_effects(pos, kind, r_);
                        }
                    } else if !was_predicted {
                        self.kill_effects(pos, kind, 15.0);
                    }
                    if killer == me {
                        self.credit_kill();
                    }
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
                E_GAMEOVER => self.enter_dying(),
                E_BLAST => {
                    let (Some(p), Some(rad)) = (r.v2(), r.f32()) else {
                        return;
                    };
                    self.ring(p, rad, 0.35, "#ffb24a", 8.0);
                    self.push_sfx(Sfx::Explosion, p);
                    let d = self.player.pos.dist(p);
                    self.add_shake(if d < 400.0 { 0.3 } else { 0.08 });
                }
                E_BOSS => {
                    let Some(v) = r.u8() else { return };
                    self.toast = Some((boss_from(v).name().to_string(), 2.5));
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
    match v {
        1 => BossVar::Gravedigger,
        2 => BossVar::Matriarch,
        3 => BossVar::CinderKing,
        4 => BossVar::Wraith,
        5 => BossVar::BloomHeart,
        _ => BossVar::Abomination,
    }
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
        slow_t: 0.0,
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
}
