//! Home Base: a walkable social hub between missions. Vendor NPCs replace the
//! menu shops (the UI opens their stall and asks the server for in-character
//! lines), the dispatcher's terminal launches missions, the memorial monument
//! shows the leaderboard, and the firing range has dummies with a DPS meter.
//! Party members in the hub walk around next to you.
//!
//! East through the gate lies **the Outskirts**, a free-roam zone: ambient
//! zombies, destructible crates, and **Supply Drop** public events (hold the
//! beacon for 30s against a surge; pays coins via the UI → `/hub/reward`).
//! Going down out there gets you pulled back to base, never a game over, and
//! the base turrets zap anything that follows you through the gate.

use super::ModeHooks;
use crate::game::{Banner, Game, ZKind, DT};
use crate::math::*;
use crate::world;

pub struct Hub;

/// A vendor NPC standing at their stall.
pub struct Npc {
    pub id: String,
    pub name: &'static str,
    pub pos: V2,
    pub aim: f32,
    pub walk: f32,
    pub skin: usize,
    pub hat: usize,
    pub gun: usize,
    pub line: String,
    pub line_t: f32,
}

/// Another player walking around the hub (from the social server).
pub struct HubMate {
    pub name: String,
    pub pos: V2,
    pub prev: V2,
    pub target: V2,
    pub aim: f32,
    pub walk: f32,
    pub skin: usize,
    pub hat: usize,
    pub gun: usize,
    pub bubble: String,
    pub bubble_t: f32,
}

pub struct HubState {
    pub npcs: Vec<Npc>,
    pub mates: Vec<HubMate>,
    pub dummies: Vec<V2>,
    /// damage dealt to dummies: (time, amount), for the DPS meter
    pub dps_log: Vec<(f32, f32)>,
    pub range: [f32; 4],
    /// vendor whose stall is open in the UI
    pub open: Option<String>,
    pub pos_t: f32,
    /// the Outskirts free-roam zone (x0, y0, x1, y1)
    pub outskirts: [f32; 4],
    pub amb_t: f32,
    /// time until the next Supply Drop (counts down while you're out there)
    pub drop_cd: f32,
    pub drop: Option<SupplyDrop>,
    pub drops_done: u32,
    /// base turret zaps (from, to, t) for the renderer
    pub zaps: Vec<(V2, V2, f32)>,
}

pub struct SupplyDrop {
    pub pos: V2,
    /// seconds held inside the ring
    pub held: f32,
    /// seconds since it landed (it leaves after DROP_MAX)
    pub t: f32,
}

pub const DROP_R: f32 = 170.0;
pub const DROP_HOLD: f32 = 30.0;
pub const DROP_MAX: f32 = 90.0;
const DROP_EVERY: f32 = 120.0;
const AMBIENT_CAP: usize = 12;
const GATE_X: f32 = 1000.0;
const TURRET: V2 = V2 { x: 940.0, y: -150.0 };

/// Nothing solid there (collision wouldn't push a body of radius `r`).
fn free_spot(c: V2, r: f32) -> bool {
    world::resolve(c, r).0.dist(c) < 0.5
}

pub const TALK_R: f32 = 110.0;

fn npc_look(id: &str) -> (&'static str, usize, usize, usize) {
    // (name, skin, hat, gun)
    match id {
        "gunsmith" => ("Rusty Vex", 3, 3, 3),
        "tailor" => ("Madame Lucienne", 4, 4, 0),
        "quartermaster" => ("Sgt. Dole", 2, 1, 1),
        _ => ("Ada", 5, 2, 0),
    }
}

impl Game {
    fn init_hub(&mut self) {
        self.map_id = "hub".into();
        let m = world::load_builtin("hub");
        if let Some(m) = &m {
            self.player.pos = m.spawn;
            self.player.prev = m.spawn;
        }
        world::use_map(m);
        let Some(m) = world::map() else { return };
        let npcs = m
            .interact
            .iter()
            .filter(|i| i.kind == "vendor")
            .map(|i| {
                let (name, skin, hat, gun) = npc_look(&i.id);
                Npc {
                    id: i.id.clone(),
                    name,
                    pos: v2(i.pos[0], i.pos[1]),
                    aim: std::f32::consts::FRAC_PI_2,
                    walk: 0.0,
                    skin,
                    hat,
                    gun,
                    line: String::new(),
                    line_t: 0.0,
                }
            })
            .collect();
        let dummies = m
            .interact
            .iter()
            .filter(|i| i.kind == "dummy")
            .map(|i| v2(i.pos[0], i.pos[1]))
            .collect();
        let range = m.zones.iter().find(|z| z.id == "range").map(|z| z.rect).unwrap_or([0.0; 4]);
        let outskirts = m.zones.iter().find(|z| z.id == "outskirts").map(|z| z.rect).unwrap_or([0.0; 4]);
        self.hub = Some(Box::new(HubState {
            npcs,
            mates: Vec::new(),
            dummies,
            dps_log: Vec::new(),
            range,
            open: None,
            pos_t: 0.0,
            outskirts,
            amb_t: 0.0,
            drop_cd: 40.0,
            drop: None,
            drops_done: 0,
            zaps: Vec::new(),
        }));
        self.wave.n = 0;
        self.wave.intermission = 0.0;
        self.audio.set_track(crate::audio::Track::Lobby);
    }

    /// Weapons are only live on the firing range and out in the Outskirts.
    pub fn in_range_zone(&self) -> bool {
        let Some(h) = self.hub.as_ref() else { return true };
        let p = self.player.pos;
        let inside = |[x0, y0, x1, y1]: [f32; 4]| p.x >= x0 && p.x <= x1 && p.y >= y0 && p.y <= y1;
        inside(h.range) || inside(h.outskirts)
    }

    pub fn in_outskirts(&self) -> bool {
        let Some(h) = self.hub.as_ref() else { return false };
        let p = self.player.pos;
        let [x0, y0, x1, y1] = h.outskirts;
        x1 > x0 && p.x >= x0 && p.x <= x1 && p.y >= y0 && p.y <= y1
    }

    /// Downed out in the Outskirts: the base medics drag you home.
    pub fn hub_rescue(&mut self) {
        let spawn = world::map().map(|m| m.spawn).unwrap_or(V2::ZERO);
        let p = &mut self.player;
        p.hp = p.max_hp;
        p.hp_ghost = p.hp;
        p.iframes = 2.5;
        p.pos = spawn;
        p.prev = spawn;
        p.vel = V2::ZERO;
        if let Some(h) = self.hub.as_mut() {
            h.drop = None;
        }
        self.banner = Some(Banner {
            title: "PULLED BACK TO BASE".into(),
            sub: "The medics patched you up. The Outskirts will still be there.".into(),
            t: 0.0,
            dur: 3.0,
            color: "#9cffb8",
        });
        self.audio.play(crate::audio::Sfx::Heal);
    }

    /// Ambient zombies, the gate turret and Supply Drop events.
    fn outskirts_tick(&mut self) {
        let out = self.in_outskirts();
        let p = self.player.pos;
        // base turret: anything that follows you through the gate gets zapped
        let mut zapped = Vec::new();
        for z in self.zombies.iter_mut().filter(|z| !z.dead && !z.dummy) {
            if z.pos.x < GATE_X - 30.0 {
                z.dead = true;
                zapped.push(z.pos);
            }
        }
        for at in zapped {
            if let Some(h) = self.hub.as_mut() {
                h.zaps.push((TURRET, at, 0.25));
            }
            self.burst(at, 16, 260.0, "#7ad7ff", 3.0, 0.5, true, true);
            self.sfx(crate::audio::Sfx::Lightning, at);
        }
        let Some(h) = self.hub.as_mut() else { return };
        for z in h.zaps.iter_mut() {
            z.2 -= DT;
        }
        h.zaps.retain(|z| z.2 > 0.0);
        if out {
            h.drop_cd -= DT;
        }
        // Supply Drop: land, hold the ring, get paid
        let mut landed = None;
        let mut done = false;
        let mut gone = false;
        if let Some(dr) = h.drop.as_mut() {
            dr.t += DT;
            if p.dist(dr.pos) < DROP_R && out {
                dr.held += DT;
            }
            if dr.held >= DROP_HOLD {
                done = true;
            } else if dr.t >= DROP_MAX {
                gone = true;
            }
        } else if out && h.drop_cd <= 0.0 {
            let [x0, y0, x1, y1] = h.outskirts;
            let mut at = v2((x0 + x1) * 0.5, (y0 + y1) * 0.5);
            for _ in 0..12 {
                let c = v2(self.rng.range(x0 + 250.0, x1 - 200.0), self.rng.range(y0 + 200.0, y1 - 200.0));
                if c.dist(p) > 350.0 && free_spot(c, 60.0) {
                    at = c;
                    break;
                }
            }
            h.drop = Some(SupplyDrop { pos: at, held: 0.0, t: 0.0 });
            landed = Some(at);
        }
        let surge = h.drop.is_some();
        if done || gone {
            h.drop = None;
            h.drop_cd = DROP_EVERY;
            if done {
                h.drops_done += 1;
            }
        }
        let rect = h.outskirts;
        if let Some(at) = landed {
            self.banner = Some(Banner {
                title: "SUPPLY DROP".into(),
                sub: "Hold the beacon for 30s. They'll come for it.".into(),
                t: 0.0,
                dur: 3.0,
                color: "#ffd36a",
            });
            self.ring(at, 260.0, 0.8, "#ffd36a", 8.0);
            self.burst(at, 40, 420.0, "#ffd36a", 4.0, 0.9, false, true);
            self.add_shake(0.35);
            self.sfx(crate::audio::Sfx::Explosion, at);
            self.say("supply drop");
        }
        if done {
            self.ui_out.push(("hubreward", "supply".into()));
            self.banner = Some(Banner {
                title: "SUPPLIES SECURED".into(),
                sub: "Ada's sending a reward your way".into(),
                t: 0.0,
                dur: 3.0,
                color: "#9cffb8",
            });
            self.audio.play(crate::audio::Sfx::LevelUp);
            // the surge breaks
            for z in self.zombies.iter_mut().filter(|z| !z.dummy && z.pos.dist(p) > 300.0) {
                z.dead = true;
            }
        }
        if gone {
            self.toast = Some(("The supply drop was overrun".into(), 2.5));
        }
        // ambient horde while you're out there (more during a Supply Drop)
        if !out {
            return;
        }
        let amb = self.zombies.iter().filter(|z| !z.dead && !z.dummy).count();
        let cap = if surge { AMBIENT_CAP + 14 } else { AMBIENT_CAP };
        let Some(h) = self.hub.as_mut() else { return };
        h.amb_t -= DT;
        if amb >= cap || h.amb_t > 0.0 {
            return;
        }
        h.amb_t = if surge { 0.55 } else { 1.6 };
        let [x0, y0, x1, y1] = rect;
        for _ in 0..10 {
            let c = v2(self.rng.range(x0 + 60.0, x1 - 40.0), self.rng.range(y0 + 40.0, y1 - 40.0));
            if c.dist(p) > 480.0 && c.x > GATE_X + 150.0 && free_spot(c, 20.0) {
                let k = match self.rng.below(10) {
                    0..=5 => ZKind::Walker,
                    6..=7 => ZKind::Runner,
                    8 => ZKind::Crawler,
                    _ => ZKind::Brute,
                };
                self.spawn_zombie(k, c);
                if let Some(z) = self.zombies.last_mut() {
                    z.xp = 0; // no level-up screens in the hub
                }
                break;
            }
        }
    }

    /// The vendor (or monument) in talking distance.
    pub fn hub_target(&self) -> Option<(String, V2)> {
        let h = self.hub.as_ref()?;
        let p = self.player.pos;
        let mut best: Option<(f32, String, V2)> = None;
        for n in h.npcs.iter() {
            let d = n.pos.dist(p);
            if d < TALK_R && best.as_ref().map_or(true, |b| d < b.0) {
                best = Some((d, n.id.clone(), n.pos));
            }
        }
        if best.is_none() && p.dist(V2::ZERO) < 120.0 {
            return Some(("monument".into(), V2::ZERO));
        }
        best.map(|b| (b.1, b.2))
    }

    pub fn hub_interact(&mut self) -> bool {
        let Some((id, _)) = self.hub_target() else { return false };
        if let Some(h) = self.hub.as_mut() {
            h.open = Some(id.clone());
        }
        self.ui_out.push(("hubopen", id));
        self.audio.play(crate::audio::Sfx::UiSelect);
        true
    }

    /// A vendor speaks (text from the UI / server).
    pub fn npc_say(&mut self, id: &str, line: &str) {
        let Some(h) = self.hub.as_mut() else { return };
        if let Some(n) = h.npcs.iter_mut().find(|n| n.id == id) {
            n.line = line.chars().take(180).collect();
            n.line_t = 4.0 + line.len() as f32 * 0.04;
        }
    }

    /// Hub mates from the social server: name~x~y~aim~skin~hat~gun|...
    pub fn set_hub_mates(&mut self, spec: &str) {
        let Some(h) = self.hub.as_mut() else { return };
        let mut seen = Vec::new();
        for m in spec.split('|').filter(|s| !s.is_empty()).take(8) {
            let f: Vec<&str> = m.split('~').collect();
            if f.len() < 7 {
                continue;
            }
            let name: String = f[0].chars().take(16).collect();
            let pos = v2(f[1].parse().unwrap_or(0.0), f[2].parse().unwrap_or(0.0));
            let aim = f[3].parse().unwrap_or(0.0);
            let (skin, hat, gun) = (
                crate::content::skin_index(f[4]),
                crate::content::hat_index(f[5]),
                crate::content::gun_index(f[6]),
            );
            seen.push(name.clone());
            match h.mates.iter_mut().find(|x| x.name == name) {
                Some(x) => {
                    x.target = pos;
                    x.aim = aim;
                    x.skin = skin;
                    x.hat = hat;
                    x.gun = gun;
                }
                None => h.mates.push(HubMate {
                    name,
                    pos,
                    prev: pos,
                    target: pos,
                    aim,
                    walk: 0.0,
                    skin,
                    hat,
                    gun,
                    bubble: String::new(),
                    bubble_t: 0.0,
                }),
            }
        }
        h.mates.retain(|x| seen.contains(&x.name));
    }

    pub fn hub_bubble(&mut self, name: &str, text: &str) {
        let Some(h) = self.hub.as_mut() else { return };
        if let Some(x) = h.mates.iter_mut().find(|x| x.name == name) {
            x.bubble = text.chars().take(120).collect();
            x.bubble_t = 5.0;
        }
    }

    pub fn dps(&self) -> f32 {
        let Some(h) = self.hub.as_ref() else { return 0.0 };
        let window = 3.0;
        let sum: f32 = h.dps_log.iter().filter(|e| self.time - e.0 < window).map(|e| e.1).sum();
        sum / window
    }

    pub fn log_dummy_damage(&mut self, dmg: f32) {
        let t = self.time;
        if let Some(h) = self.hub.as_mut() {
            h.dps_log.push((t, dmg));
            if h.dps_log.len() > 400 {
                h.dps_log.drain(0..100);
            }
        }
    }

    fn hub_tick(&mut self) {
        self.outskirts_tick();
        let p = self.player.pos;
        // dummies: always standing, always full health
        let dummies = self.hub.as_ref().map(|h| h.dummies.clone()).unwrap_or_default();
        for d in dummies {
            match self.zombies.iter_mut().find(|z| z.dummy && z.pos.dist(d) < 4.0) {
                Some(z) => {
                    z.hp = z.max_hp;
                    z.pos = d;
                    z.prev = d;
                    z.vel = V2::ZERO;
                }
                None => {
                    self.spawn_zombie(ZKind::Walker, d);
                    if let Some(z) = self.zombies.last_mut() {
                        z.dummy = true;
                        z.hp = 1.0e9;
                        z.max_hp = 1.0e9;
                        z.speed = 0.0;
                        z.dmg = 0.0;
                        z.xp = 0;
                        z.pos = d;
                        z.prev = d;
                        z.face = -std::f32::consts::FRAC_PI_2;
                    }
                }
            }
        }
        // walking away from an open stall closes it
        let target = self.hub_target().map(|t| t.0);
        if let Some(h) = self.hub.as_mut() {
            if let Some(open) = h.open.clone() {
                let far = h.npcs.iter().find(|n| n.id == open).map(|n| n.pos.dist(p) > TALK_R + 80.0).unwrap_or(open == "monument" && target.is_none());
                if far {
                    h.open = None;
                    self.ui_out.push(("hubleave", open));
                }
            }
        }
        let Some(h) = self.hub.as_mut() else { return };
        for n in h.npcs.iter_mut() {
            n.line_t = (n.line_t - DT).max(0.0);
            // turn to face a nearby courier, idle-sway otherwise
            if n.pos.dist(p) < 260.0 {
                let want = (p - n.pos).angle();
                n.aim += wrap_angle(want - n.aim) * damp(5.0, DT);
            }
        }
        for m in h.mates.iter_mut() {
            m.prev = m.pos;
            let step = m.target - m.pos;
            if step.len() > 400.0 {
                m.pos = m.target;
            } else {
                m.pos += step * damp(8.0, DT);
            }
            m.walk += (m.pos - m.prev).len() * 0.06;
            m.bubble_t = (m.bubble_t - DT).max(0.0);
        }
        // tell the UI where we are (for other players in the hub), ~4x a second
        h.pos_t -= DT;
        if h.pos_t <= 0.0 {
            h.pos_t = 0.25;
            let s = format!("{:.0}~{:.0}~{:.2}", p.x, p.y, self.player.aim);
            self.ui_out.push(("hubpos", s));
        }
    }
}

impl ModeHooks for Hub {
    fn on_start(&self, g: &mut Game) {
        g.init_hub();
        g.banner = Some(Banner {
            title: "HOME BASE".into(),
            sub: "Talk to the vendors (E). Missions at Ada's terminal.".into(),
            t: 0.0,
            dur: 3.0,
            color: "#9cffb8",
        });
    }

    fn update_waves(&self, g: &mut Game) {
        g.hub_tick();
    }

    fn hud(&self, _g: &Game) -> String {
        "HOME BASE".into()
    }

    fn is_over(&self, _g: &Game) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::Phase;

    fn hub() -> Game {
        let mut g = Game::new(5);
        g.screen = v2(1280.0, 720.0);
        g.zoom = 1.0;
        g.start("mode=hub");
        g.phase = Phase::Playing;
        g
    }

    fn tick(g: &mut Game, secs: f32) {
        for _ in 0..(secs / DT) as usize {
            g.mode.hooks().update_waves(g);
            g.zombies.retain(|z| !z.dead);
        }
    }

    #[test]
    fn outskirts_spawn_zombies_and_the_gate_turret_guards_the_plaza() {
        let mut g = hub();
        assert!(!g.in_outskirts());
        tick(&mut g, 3.0);
        assert!(g.zombies.iter().all(|z| z.dummy), "nothing ambient in the plaza");
        g.player.pos = v2(1300.0, 0.0);
        assert!(g.in_outskirts() && g.in_range_zone(), "weapons free out there");
        tick(&mut g, 6.0);
        let amb: Vec<_> = g.zombies.iter().filter(|z| !z.dummy).collect();
        assert!(!amb.is_empty() && amb.iter().all(|z| z.xp == 0 && z.pos.x > GATE_X));
        g.spawn_gem_pub(v2(1300.0, 0.0), 50);
        assert!(g.gems.is_empty(), "no XP gems in the hub");
        // one follows us home: zapped at the gate
        g.spawn_zombie(ZKind::Walker, v2(600.0, 0.0));
        tick(&mut g, 0.1);
        assert!(g.zombies.iter().all(|z| z.dummy || z.pos.x > GATE_X));
    }

    #[test]
    fn going_down_out_there_is_a_rescue_not_a_game_over() {
        let mut g = hub();
        g.player.pos = v2(1500.0, 0.0);
        g.player.iframes = 0.0;
        g.player.hp = 5.0;
        g.hurt_player(50.0, v2(1520.0, 0.0));
        assert_eq!(g.phase, Phase::Playing);
        assert!(g.player.hp >= g.player.max_hp && g.player.pos.x < GATE_X, "back at base, patched up");
    }

    #[test]
    fn holding_a_supply_drop_pays_out() {
        let mut g = hub();
        g.player.pos = v2(1300.0, 0.0);
        g.hub.as_mut().unwrap().drop_cd = 0.0;
        tick(&mut g, 0.1);
        let at = g.hub.as_ref().unwrap().drop.as_ref().expect("a drop landed").pos;
        g.player.max_hp = 1.0e7;
        g.player.hp = 1.0e7;
        g.player.pos = at + v2(30.0, 0.0);
        g.ui_out.clear();
        tick(&mut g, DROP_HOLD + 0.5);
        let h = g.hub.as_ref().unwrap();
        assert!(h.drop.is_none() && h.drops_done == 1);
        assert!(g.ui_out.iter().any(|e| e.0 == "hubreward"));
    }
}
