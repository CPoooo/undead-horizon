//! Home Base: a walkable social hub between missions. Vendor NPCs replace the
//! menu shops (the UI opens their stall and asks the server for in-character
//! lines), the dispatcher's terminal launches missions, the memorial monument
//! shows the leaderboard, and the firing range has dummies with a DPS meter.
//! Party members in the hub walk around next to you.

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
        self.hub = Some(Box::new(HubState {
            npcs,
            mates: Vec::new(),
            dummies,
            dps_log: Vec::new(),
            range,
            open: None,
            pos_t: 0.0,
        }));
        self.wave.n = 0;
        self.wave.intermission = 0.0;
        self.audio.set_track(crate::audio::Track::Lobby);
    }

    pub fn in_range_zone(&self) -> bool {
        let Some(h) = self.hub.as_ref() else { return true };
        let p = self.player.pos;
        let [x0, y0, x1, y1] = h.range;
        p.x >= x0 && p.x <= x1 && p.y >= y0 && p.y <= y1
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
