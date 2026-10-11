//! Nemesis system (after Shadow of Mordor). The zombie that kills you in a
//! solo Infinite/Campaign run rises as your **Nemesis**: the UI names it,
//! remembers it (synced settings) and passes it back as
//! `nemesis=<kind>,<rank>,<name>` when a run starts. From wave 3 it may come
//! hunting you: bigger, tougher, with an elite affix, a name plate and a taunt.
//! Kill it for a chest; if it gets you again it ranks up (max 5).
//!
//! Game → UI: `nemesis` = "<kind u8>|<1 if the Nemesis itself killed you>" on
//! death, `nemesisslain` = "<rank>" when you put it down.

use crate::audio::Sfx;
use crate::game::{Banner, Game, PickKind, Pickup, ZKind};
use crate::math::*;
use crate::modes::Mode;
use web_sys::CanvasRenderingContext2d as Ctx;

pub const MAX_RANK: u32 = 5;

#[derive(Clone, Debug)]
pub struct Nemesis {
    pub name: String,
    pub kind: ZKind,
    pub rank: u32,
    /// uid of the living Nemesis this run
    pub uid: Option<u32>,
    pub spawned: bool,
    pub slain: bool,
}

const TAUNTS: [&str; 5] = [
    "You again? I remember the taste.",
    "I've been waiting for you, courier.",
    "Every scar you gave me, I'll give back.",
    "Run. It makes the hunt sweeter.",
    "This time there's no one to drag you home.",
];

impl Game {
    /// `nemesis=<kind>,<rank>,<name>` from the start options.
    pub(crate) fn set_nemesis_opt(&mut self, v: &str) {
        let mut it = v.splitn(3, ',');
        let (Some(k), Some(r), Some(n)) = (it.next(), it.next(), it.next()) else {
            self.nemesis = None;
            return;
        };
        let kind = ZKind::from_u8(k.parse().unwrap_or(0));
        if kind == ZKind::Boss || n.trim().is_empty() {
            self.nemesis = None;
            return;
        }
        self.nemesis = Some(Nemesis {
            name: n.chars().take(40).collect(),
            kind,
            rank: r.parse::<u32>().unwrap_or(1).clamp(1, MAX_RANK),
            uid: None,
            spawned: false,
            slain: false,
        });
    }

    fn nemesis_mode(&self) -> bool {
        matches!(self.mode, Mode::Infinite | Mode::Campaign(_)) && self.net.is_none() && self.tutorial.is_none()
    }

    /// Wave start: maybe the Nemesis comes hunting (once per run, from wave 3,
    /// never on a boss wave; certain by wave 8).
    pub(crate) fn nemesis_wave(&mut self, n: u32) {
        if !self.nemesis_mode() || n < 3 || self.wave.boss_pending {
            return;
        }
        let Some(ne) = self.nemesis.as_ref() else { return };
        if ne.spawned || !(n >= 8 || self.rng.chance(0.35)) {
            return;
        }
        let (kind, rank, name) = (ne.kind, ne.rank, ne.name.clone());
        let a = self.rng.angle();
        let p = self.player.pos + V2::from_angle(a) * (self.spawn_r - 60.0);
        self.spawn_zombie(kind, p);
        let affix = self.roll_affix();
        self.make_elite(affix);
        let uid = {
            let Some(z) = self.zombies.last_mut() else { return };
            let k = rank as f32;
            z.max_hp *= 8.0 + 2.5 * k;
            z.hp = z.max_hp;
            z.speed *= 1.1 + 0.03 * k;
            z.dmg *= 1.4 + 0.15 * k;
            z.r *= 1.35;
            z.xp = z.xp.max(40) * (2 + rank);
            z.uid
        };
        if let Some(ne) = self.nemesis.as_mut() {
            ne.spawned = true;
            ne.uid = Some(uid);
        }
        let taunt = TAUNTS[(rank as usize - 1).min(TAUNTS.len() - 1)];
        self.banner = Some(Banner {
            title: format!("NEMESIS: {}", name.to_uppercase()),
            sub: format!("\"{}\"", taunt),
            t: 0.0,
            dur: 3.6,
            color: "#ff4a5a",
        });
        self.say(taunt);
        self.sfx(Sfx::BossRoar, p);
        self.audio.play(Sfx::Stinger);
        self.zoom_punch = 0.06;
        self.add_shake(0.4);
    }

    /// A zombie died: was it the Nemesis?
    pub(crate) fn nemesis_check_kill(&mut self, uid: u32, pos: V2) {
        let Some(ne) = self.nemesis.as_mut() else { return };
        if ne.uid != Some(uid) {
            return;
        }
        ne.uid = None;
        ne.slain = true;
        let (rank, name) = (ne.rank, ne.name.clone());
        self.pickups.push(Pickup { pos, kind: PickKind::Chest, t: 0.0, dead: false });
        self.banner = Some(Banner {
            title: "NEMESIS SLAIN".into(),
            sub: format!("{} won't hunt you again · a chest dropped", name),
            t: 0.0,
            dur: 3.4,
            color: "#ffd36a",
        });
        self.ring(pos, 420.0, 1.0, "#ff4a5a", 10.0);
        self.burst(pos, 70, 600.0, "#ff4a5a", 4.0, 1.0, true, true);
        self.slowmo = self.slowmo.max(1.0);
        self.audio.play(Sfx::LevelUp);
        self.say("nemesis slain");
        self.ui_out.push(("nemesisslain", rank.to_string()));
    }

    /// We died (solo): the closest zombie to the blow rises as (or ranks up) the Nemesis.
    pub(crate) fn nemesis_on_death(&mut self, from: V2) {
        if !self.nemesis_mode() {
            return;
        }
        let killer = self
            .zombies
            .iter()
            // melee: the zombie at `from`; a projectile: whoever is closest to where it hit
            .filter(|z| !z.dead && z.kind != ZKind::Boss && z.pos.dist(from) < 900.0)
            .min_by(|a, b| a.pos.dist2(from).partial_cmp(&b.pos.dist2(from)).unwrap_or(std::cmp::Ordering::Equal))
            .map(|z| (z.kind, z.uid));
        let Some((kind, uid)) = killer else { return };
        // kinds that can't carry a grudge (they blow up or flutter off) rise as something sturdier
        let kind = match kind {
            ZKind::Exploder => ZKind::Brute,
            ZKind::Bat => ZKind::Runner,
            k => k,
        };
        let again = self.nemesis.as_ref().is_some_and(|n| n.uid == Some(uid));
        self.ui_out.push(("nemesis", format!("{}|{}", kind as u8, again as u8)));
    }

    /// Red aura and name plate over the living Nemesis.
    pub(crate) fn draw_nemesis(&self, c: &Ctx, t: f32) {
        let Some(ne) = &self.nemesis else { return };
        let Some(uid) = ne.uid else { return };
        let Some(z) = self.zombies.iter().find(|z| z.uid == uid && !z.dead) else { return };
        let p = z.prev.lerp(z.pos, self.alpha);
        let pulse = 0.5 + 0.5 * (t * 4.0).sin();
        let d = |x: f32| x as f64;
        c.set_stroke_style_str("#ff4a5a");
        c.set_line_width(3.0);
        c.set_global_alpha(d(0.45 + 0.35 * pulse));
        c.begin_path();
        let _ = c.arc(d(p.x), d(p.y), d(z.r + 10.0 + 4.0 * pulse), 0.0, std::f64::consts::TAU);
        c.stroke();
        c.set_global_alpha(1.0);
        c.set_text_align("center");
        c.set_text_baseline("middle");
        c.set_font("900 12px 'Inter','Segoe UI',system-ui,sans-serif");
        c.set_line_join("round");
        c.set_stroke_style_str("rgba(0,0,0,0.9)");
        c.set_line_width(3.5);
        let label = format!("☠ {} · R{}", ne.name, ne.rank);
        let _ = c.stroke_text(&label, d(p.x), d(p.y - z.r - 40.0));
        c.set_fill_style_str("#ff6a7a");
        let _ = c.fill_text(&label, d(p.x), d(p.y - z.r - 40.0));
        // health bar
        let w = 60.0;
        let k = (z.hp / z.max_hp.max(1.0)).clamp(0.0, 1.0);
        c.set_fill_style_str("rgba(0,0,0,0.7)");
        c.fill_rect(d(p.x - w * 0.5), d(p.y - z.r - 16.0), d(w), 5.0);
        c.set_fill_style_str("#ff4a5a");
        c.fill_rect(d(p.x - w * 0.5), d(p.y - z.r - 16.0), d(w * k), 5.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::Phase;

    fn run(opts: &str) -> Game {
        let mut g = Game::new(11);
        g.screen = v2(1280.0, 720.0);
        g.zoom = 1.0;
        g.spawn_r = 700.0;
        g.start(opts);
        g.phase = Phase::Playing;
        g
    }

    #[test]
    fn the_nemesis_hunts_from_wave_three_and_drops_a_chest() {
        let mut g = run("mode=infinite;nemesis=1,3,Gnasher the Unbroken");
        let ne = g.nemesis.as_ref().expect("parsed");
        assert_eq!((ne.rank, ne.name.as_str()), (3, "Gnasher the Unbroken"));
        g.nemesis_wave(2);
        assert!(!g.nemesis.as_ref().unwrap().spawned, "not before wave 3");
        g.wave.boss_pending = false;
        g.nemesis_wave(8);
        let uid = g.nemesis.as_ref().unwrap().uid.expect("certain by wave 8");
        let z = g.zombies.iter().find(|z| z.uid == uid).unwrap();
        assert!(z.max_hp > z.kind.def().hp * 8.0, "much tougher than its kind");
        let pos = z.pos;
        let count = g.zombies.len();
        g.nemesis_wave(9);
        assert_eq!(g.zombies.len(), count, "only once per run");
        let chests = g.pickups.iter().filter(|p| p.kind == PickKind::Chest).count();
        g.ui_out.clear();
        g.nemesis_check_kill(uid, pos);
        assert!(g.pickups.iter().filter(|p| p.kind == PickKind::Chest).count() > chests);
        assert!(g.ui_out.iter().any(|e| e.0 == "nemesisslain" && e.1 == "3"));
    }

    #[test]
    fn dying_names_the_killer_and_coop_has_no_nemesis() {
        let mut g = run("mode=infinite");
        g.spawn_zombie(ZKind::Runner, g.player.pos + v2(20.0, 0.0));
        let from = g.zombies.last().unwrap().pos;
        g.ui_out.clear();
        g.nemesis_on_death(from);
        assert!(g.ui_out.iter().any(|e| e.0 == "nemesis" && e.1 == format!("{}|0", ZKind::Runner as u8)));
        let mut co = Game::new(3);
        co.start_coop("role=host;me=0;names=A,B;present=1,1,0;nemesis=1,2,Bob");
        co.phase = Phase::Playing;
        co.nemesis_wave(8);
        assert!(co.nemesis.as_ref().is_none_or(|n| !n.spawned), "solo only");
    }
}

