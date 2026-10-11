//! Outbreak: round-based survival on an authored map, in the spirit of classic
//! zombie modes (original names throughout). Zombies climb in through boarded
//! windows; you earn points for every hit and kill, and spend them on doors,
//! wall weapons, the mystery box, perk machines and the upgrade machine.
//! No XP or level-ups here: guns, perks and power-ups are the progression.

use super::ModeHooks;
use crate::audio::Sfx;
use crate::content::GUNS;
use crate::game::{Banner, Game, PickKind, Pickup, ZKind, DT};
use crate::math::*;
use crate::nav::FlowField;
use crate::net;
use crate::world;

pub struct Outbreak;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Perk {
    IronGut,
    Quickdraw,
    SecondWind,
    Fleetfoot,
}

impl Perk {
    pub fn from_id(id: &str) -> Option<Perk> {
        match id {
            "irongut" => Some(Perk::IronGut),
            "quickdraw" => Some(Perk::Quickdraw),
            "secondwind" => Some(Perk::SecondWind),
            "fleetfoot" => Some(Perk::Fleetfoot),
            _ => None,
        }
    }
    /// (name, icon, color, what it does)
    pub fn def(self) -> (&'static str, &'static str, &'static str, &'static str) {
        match self {
            Perk::IronGut => ("Iron Gut", "🛢️", "#ff4a4a", "+150 max health"),
            Perk::Quickdraw => ("Quickdraw Cola", "🥤", "#4ad06a", "Reload twice as fast, +15% fire rate"),
            Perk::SecondWind => ("Second Wind", "💨", "#4ac8ff", "Revive faster; solo: get back up (3x)"),
            Perk::Fleetfoot => ("Fleetfoot", "🦶", "#ffd84a", "+15% movement speed"),
        }
    }
}

#[derive(Clone, Copy)]
pub struct Weapon {
    pub gun: usize,
    pub mag: u32,
    pub reserve: u32,
    pub upgraded: bool,
}

impl Weapon {
    pub fn new(gun: usize) -> Weapon {
        Weapon {
            gun,
            mag: GUNS[gun].mag,
            reserve: GUNS[gun].reserve,
            upgraded: false,
        }
    }
    pub fn max_mag(&self) -> u32 {
        let m = GUNS[self.gun].mag;
        if self.upgraded {
            m + m / 2
        } else {
            m
        }
    }
    pub fn max_reserve(&self) -> u32 {
        let r = GUNS[self.gun].reserve;
        if self.upgraded {
            r * 2
        } else {
            r
        }
    }
    pub fn name(&self) -> &'static str {
        if self.upgraded {
            GUNS[self.gun].pap
        } else {
            GUNS[self.gun].name
        }
    }
}

pub struct Barricade {
    pub pos: V2,
    pub dir: V2,
    pub zone: usize,
    pub boards: u8,
    pub tear_t: f32,
}

pub const MAX_BOARDS: u8 = 6;

pub struct BoxSpin {
    pub t: f32,
    pub gun: usize,
    /// the box decided to leave
    pub hand: bool,
}

pub struct ObState {
    pub points: u32,
    pub weapons: Vec<Weapon>,
    pub cur: usize,
    pub reload_t: f32,
    pub windows: Vec<Barricade>,
    /// index into the map's interactables of the box currently in use
    pub box_at: usize,
    pub box_uses: u32,
    pub spin: Option<BoxSpin>,
    /// the box flew away; it lands somewhere else when this runs out
    pub box_away: f32,
    pub power: bool,
    pub perks: Vec<Perk>,
    pub insta_t: f32,
    pub double_t: f32,
    pub drops_round: u32,
    pub self_revives: u32,
    pub repair_t: f32,
    pub repair_points_round: u32,
    /// interact held this frame (repairs boards while held)
    pub use_held: f32,
    /// this round is a hound round; the next one comes at this round number
    pub hound_round: bool,
    pub next_hounds: u32,
}

pub const SPIN_T: f32 = 3.0;
pub const OFFER_T: f32 = 9.0;
pub const USE_R: f32 = 80.0;

impl ObState {
    pub fn weapon(&self) -> &Weapon {
        &self.weapons[self.cur.min(self.weapons.len() - 1)]
    }
    pub fn weapon_mut(&mut self) -> &mut Weapon {
        let i = self.cur.min(self.weapons.len() - 1);
        &mut self.weapons[i]
    }
    pub fn has(&self, p: Perk) -> bool {
        self.perks.contains(&p)
    }
}

impl Game {
    fn init_outbreak(&mut self) {
        if self.map_id.is_empty() {
            self.map_id = "asylum".into();
            let m = world::load_builtin(&self.map_id);
            if let Some(m) = &m {
                self.player.pos = m.spawn;
                self.player.prev = m.spawn;
            }
            world::use_map(m);
        }
        let Some(m) = world::map() else { return };
        let windows = m
            .windows
            .iter()
            .map(|w| Barricade {
                pos: v2(w.pos[0], w.pos[1]),
                dir: v2(w.dir[0], w.dir[1]).norm(),
                zone: m.zone_index(&w.zone).unwrap_or(0),
                boards: MAX_BOARDS,
                tear_t: 1.0,
            })
            .collect();
        let box_at = m
            .interact
            .iter()
            .position(|i| i.kind == "box" && i.data == "start")
            .or_else(|| m.interact.iter().position(|i| i.kind == "box"))
            .unwrap_or(0);
        self.flow = Some(FlowField::new(&m));
        self.init_secrets();
        let pistol = crate::content::gun_index("pistol");
        self.gun = pistol;
        self.ob = Some(Box::new(ObState {
            points: 500,
            weapons: vec![Weapon::new(pistol)],
            cur: 0,
            reload_t: 0.0,
            windows,
            box_at,
            box_uses: 0,
            spin: None,
            box_away: 0.0,
            power: false,
            perks: Vec::new(),
            insta_t: 0.0,
            double_t: 0.0,
            drops_round: 0,
            self_revives: 3,
            repair_t: 0.0,
            repair_points_round: 0,
            use_held: 0.0,
            hound_round: false,
            next_hounds: 5 + self.rng.below(2) as u32,
        }));
        self.wave.intermission = 3.0;
    }

    /// Points for the local player (doubled by Double Points).
    pub fn add_points(&mut self, n: u32, at: Option<V2>) {
        let Some(ob) = self.ob.as_mut() else { return };
        let n = if ob.double_t > 0.0 { n * 2 } else { n };
        ob.points += n;
        self.score += n as u64;
        if let Some(p) = at {
            self.push_text(p + v2(0.0, -20.0), format!("+{}", n), "#ffd36a", 12.0);
        }
    }

    pub fn spend(&mut self, n: u32) -> bool {
        match self.ob.as_mut() {
            Some(ob) if ob.points >= n => {
                ob.points -= n;
                true
            }
            _ => false,
        }
    }

    /// Our bullet hit a zombie (called on the shooter's own peer).
    pub fn ob_on_hit(&mut self, pos: V2, killed: bool, crit: bool) {
        if self.ob.is_none() {
            return;
        }
        let n = if killed {
            if crit {
                100
            } else {
                60
            }
        } else {
            10
        };
        self.add_points(n, if killed { Some(pos) } else { None });
    }

    /// Insta-kill turns every hit lethal.
    pub fn insta_kill(&self) -> bool {
        self.ob.as_ref().map(|o| o.insta_t > 0.0).unwrap_or(false)
    }

    // ------------------------------------------------------------- weapons

    /// Can the current weapon fire? Starts a reload when the magazine is empty.
    pub fn ob_can_fire(&mut self) -> bool {
        let Some(ob) = self.ob.as_mut() else { return true };
        if ob.reload_t > 0.0 {
            return false;
        }
        if ob.weapon().mag == 0 {
            if ob.weapon().reserve > 0 {
                self.start_reload();
            } else if (self.time * 3.0) as i32 != ((self.time - DT) * 3.0) as i32 {
                let p = self.player.pos;
                self.sfx(Sfx::UiMove, p); // dry click
            }
            return false;
        }
        true
    }

    pub fn ob_consume_round(&mut self) {
        if let Some(ob) = self.ob.as_mut() {
            let w = ob.weapon_mut();
            w.mag = w.mag.saturating_sub(1);
        }
    }

    pub fn start_reload(&mut self) {
        let Some(ob) = self.ob.as_mut() else { return };
        let w = *ob.weapon();
        if ob.reload_t > 0.0 || w.mag >= w.max_mag() || w.reserve == 0 {
            return;
        }
        let mut t = GUNS[w.gun].reload;
        if ob.has(Perk::Quickdraw) {
            t *= 0.5;
        }
        ob.reload_t = t;
        let p = self.player.pos;
        self.sfx(Sfx::Dash, p);
    }

    pub fn reload_frac(&self) -> Option<f32> {
        let ob = self.ob.as_ref()?;
        if ob.reload_t <= 0.0 {
            return None;
        }
        let mut t = GUNS[ob.weapon().gun].reload;
        if ob.has(Perk::Quickdraw) {
            t *= 0.5;
        }
        Some(1.0 - ob.reload_t / t)
    }

    pub fn swap_weapon(&mut self, slot: Option<usize>) -> bool {
        let Some(ob) = self.ob.as_mut() else { return false };
        if ob.weapons.len() < 2 {
            return false;
        }
        let next = slot.unwrap_or((ob.cur + 1) % ob.weapons.len()).min(ob.weapons.len() - 1);
        if next == ob.cur {
            return true;
        }
        ob.cur = next;
        ob.reload_t = 0.0;
        self.gun = ob.weapon().gun;
        let p = self.player.pos;
        self.sfx(Sfx::UiMove, p);
        self.player.fire_cd = self.player.fire_cd.max(0.25);
        true
    }

    /// Give a weapon: refills it if owned, else fills an empty slot or replaces the held one.
    pub fn give_weapon(&mut self, gun: usize) {
        let Some(ob) = self.ob.as_mut() else { return };
        if let Some(i) = ob.weapons.iter().position(|w| w.gun == gun) {
            let w = &mut ob.weapons[i];
            w.mag = w.max_mag();
            w.reserve = w.max_reserve();
            ob.cur = i;
        } else if ob.weapons.len() < 2 {
            ob.weapons.push(Weapon::new(gun));
            ob.cur = ob.weapons.len() - 1;
        } else {
            let i = ob.cur;
            ob.weapons[i] = Weapon::new(gun);
        }
        ob.reload_t = 0.0;
        self.gun = gun;
    }

    // ------------------------------------------------------------- perks

    pub fn give_perk(&mut self, p: Perk) {
        let Some(ob) = self.ob.as_mut() else { return };
        if ob.has(p) {
            return;
        }
        ob.perks.push(p);
        if p == Perk::IronGut {
            self.player.max_hp += 150.0;
            self.player.hp = self.player.max_hp;
        }
        let (name, _, _, _) = p.def();
        self.toast = Some((format!("{} acquired", name), 2.0));
        self.audio.play(Sfx::ReadyUp);
        self.say(&name.to_lowercase());
    }

    pub fn lose_perks(&mut self) {
        let Some(ob) = self.ob.as_mut() else { return };
        if ob.perks.contains(&Perk::IronGut) {
            self.player.max_hp = (self.player.max_hp - 150.0).max(100.0);
            self.player.hp = self.player.hp.min(self.player.max_hp);
        }
        ob.perks.clear();
    }

    pub fn ob_speed_mult(&self) -> f32 {
        match self.ob.as_ref() {
            Some(o) if o.has(Perk::Fleetfoot) => 1.15,
            _ => 1.0,
        }
    }

    pub fn ob_fire_mult(&self) -> f32 {
        match self.ob.as_ref() {
            Some(o) if o.has(Perk::Quickdraw) => 1.15,
            _ => 1.0,
        }
    }

    /// Solo Second Wind: get back up instead of dying. Returns true if it saved you.
    pub fn second_wind(&mut self) -> bool {
        let Some(ob) = self.ob.as_mut() else { return false };
        if self.net.is_some() || !ob.has(Perk::SecondWind) || ob.self_revives == 0 {
            return false;
        }
        ob.self_revives -= 1;
        let left = ob.self_revives;
        self.lose_perks();
        self.player.hp = self.player.max_hp * 0.6;
        self.player.iframes = 3.0;
        let p = self.player.pos;
        self.ring(p, 300.0, 0.6, "#4ac8ff", 10.0);
        // shove everything back
        for z in self.zombies.iter_mut() {
            let d = z.pos - p;
            let l = d.len();
            if l < 300.0 && l > 0.1 {
                z.vel += d * (1.0 / l) * 700.0;
            }
        }
        self.banner = Some(Banner {
            title: "SECOND WIND".into(),
            sub: format!("Back on your feet · {} left", left),
            t: 0.0,
            dur: 2.0,
            color: "#4ac8ff",
        });
        self.audio.play(Sfx::Heal);
        true
    }

    // ------------------------------------------------------------- power-ups

    fn maybe_drop_powerup(&mut self, pos: V2) {
        let Some(ob) = self.ob.as_mut() else { return };
        if ob.drops_round >= 4 || !self.rng.chance(0.025) {
            return;
        }
        ob.drops_round += 1;
        let kind = [
            PickKind::MaxAmmo,
            PickKind::InstaKill,
            PickKind::DoublePoints,
            PickKind::Nuke,
            PickKind::Carpenter,
        ][self.rng.below(5)];
        self.pickups.push(Pickup {
            pos,
            kind,
            t: 0.0,
            dead: false,
        });
    }

    /// A power-up was grabbed (by anyone): everyone gets it.
    pub fn apply_powerup(&mut self, kind: PickKind, broadcast: bool) {
        if broadcast {
            if let Some(n) = &mut self.net {
                n.event(|w| {
                    w.u8(net::E_POWERUP);
                    w.u8(kind as u8);
                });
            }
        }
        let p = self.player.pos;
        let (title, line) = match kind {
            PickKind::MaxAmmo => {
                if let Some(ob) = self.ob.as_mut() {
                    for w in ob.weapons.iter_mut() {
                        w.reserve = w.max_reserve();
                        w.mag = w.max_mag();
                    }
                    ob.reload_t = 0.0;
                }
                ("MAX AMMO", "max ammo")
            }
            PickKind::InstaKill => {
                if let Some(ob) = self.ob.as_mut() {
                    ob.insta_t = 30.0;
                }
                ("INSTA-KILL", "insta kill")
            }
            PickKind::DoublePoints => {
                if let Some(ob) = self.ob.as_mut() {
                    ob.double_t = 30.0;
                }
                ("DOUBLE POINTS", "double points")
            }
            PickKind::Nuke => {
                if !self.is_client() {
                    for z in self.zombies.iter_mut() {
                        if z.kind != ZKind::Boss {
                            z.hp = 0.0;
                            z.flash = 1.0;
                        }
                    }
                    self.nuke_pending = true;
                }
                self.add_points(400, None);
                self.ult_flash = 1.0;
                self.add_shake(0.8);
                ("KA-BOOM", "kaboom")
            }
            PickKind::Carpenter => {
                if let Some(ob) = self.ob.as_mut() {
                    for w in ob.windows.iter_mut() {
                        w.boards = MAX_BOARDS;
                    }
                }
                self.add_points(200, None);
                ("CARPENTER", "carpenter")
            }
            _ => return,
        };
        self.callout = Some(crate::callouts::Callout {
            text: title,
            color: "#7affb0",
            t: 0.0,
        });
        self.say(line);
        self.audio.play(Sfx::Stinger);
        self.ring(p, 400.0, 0.5, "#7affb0", 8.0);
    }

    /// Kill everything flagged by a nuke (host), outside the zombie loop.
    pub fn resolve_nuke(&mut self) {
        if !self.nuke_pending {
            return;
        }
        self.nuke_pending = false;
        self.grid.build(self.player.pos, &self.zombies);
        for j in 0..self.zombies.len() {
            if !self.zombies[j].dead && self.zombies[j].hp <= 0.0 {
                self.zombies[j].last_hitter = 255;
                self.kill_zombie(j);
            }
        }
    }

    // ------------------------------------------------------------- per step

    pub fn ob_tick(&mut self) {
        let Some(ob) = self.ob.as_mut() else { return };
        ob.insta_t = (ob.insta_t - DT).max(0.0);
        ob.double_t = (ob.double_t - DT).max(0.0);
        ob.repair_t -= DT;
        ob.use_held -= DT;
        if ob.reload_t > 0.0 {
            ob.reload_t -= DT;
            if ob.reload_t <= 0.0 {
                ob.reload_t = 0.0;
                let w = ob.weapon_mut();
                let need = w.max_mag().saturating_sub(w.mag).min(w.reserve);
                w.mag += need;
                w.reserve -= need;
            }
        }
        if let Some(s) = ob.spin.as_mut() {
            s.t += DT;
            if s.t > SPIN_T + OFFER_T || (s.hand && s.t > SPIN_T + 1.2) {
                let hand = s.hand;
                ob.spin = None;
                if hand {
                    ob.box_away = 6.0;
                }
            }
        }
        if ob.box_away > 0.0 {
            ob.box_away -= DT;
            if ob.box_away <= 0.0 {
                if let Some(m) = world::map() {
                    let spots: Vec<usize> = m
                        .interact
                        .iter()
                        .enumerate()
                        .filter(|(i, it)| it.kind == "box" && *i != ob.box_at)
                        .map(|(i, _)| i)
                        .collect();
                    if !spots.is_empty() {
                        let to = spots[self.rng.below(spots.len())];
                        self.world_change(crate::outbreak_interact::W_BOXMOVE, to as u8);
                    }
                }
                self.toast = Some(("The mystery box has moved…".into(), 2.5));
            }
        }
        // the shooter's own magazine: auto-reload when empty
        let empty = self.ob.as_ref().map(|o| o.weapon().mag == 0 && o.weapon().reserve > 0).unwrap_or(false);
        if empty {
            self.start_reload();
        }
        // repairing: hold interact next to a damaged window
        let holding = self.ob.as_ref().map(|o| o.use_held > 0.0).unwrap_or(false);
        if holding {
            self.repair_nearby();
        }
    }

    fn repair_nearby(&mut self) {
        let p = self.player.pos;
        let Some(ob) = self.ob.as_mut() else { return };
        if ob.repair_t > 0.0 {
            return;
        }
        let Some(i) = ob
            .windows
            .iter()
            .position(|w| w.boards < MAX_BOARDS && w.pos.dist(p) < USE_R + 20.0)
        else {
            return;
        };
        ob.windows[i].boards += 1;
        ob.repair_t = 0.55;
        let paid = ob.repair_points_round < 50;
        if paid {
            ob.repair_points_round += 10;
        }
        let (wp, boards) = (ob.windows[i].pos, ob.windows[i].boards);
        if paid {
            self.add_points(10, Some(wp));
        }
        self.sfx(Sfx::Blade, wp);
        self.board_event(i, boards);
    }

    pub fn board_event(&mut self, i: usize, boards: u8) {
        if let Some(n) = &mut self.net {
            if n.role == net::Role::Host {
                n.event(|w| {
                    w.u8(net::E_BOARDS);
                    w.u8(i as u8);
                    w.u8(boards);
                });
            }
        }
    }

    /// Zombies held at windows tear boards off, then climb in (host).
    pub fn update_windows(&mut self) {
        let Some(ob) = self.ob.as_mut() else { return };
        let mut changed: Vec<(usize, u8)> = Vec::new();
        let mut tear_sfx: Vec<V2> = Vec::new();
        for z in self.zombies.iter_mut() {
            if z.dead || z.win < 0 {
                continue;
            }
            let wi = z.win as usize;
            let Some(w) = ob.windows.get_mut(wi) else {
                z.win = -1;
                continue;
            };
            z.prev = z.pos;
            z.pos = w.pos - w.dir * 6.0;
            z.vel = V2::ZERO;
            z.face = w.dir.angle();
            z.anim += DT * 2.0;
            if w.boards > 0 {
                w.tear_t -= DT;
                if w.tear_t <= 0.0 {
                    w.boards -= 1;
                    w.tear_t = 1.2;
                    changed.push((wi, w.boards));
                    tear_sfx.push(w.pos);
                }
            } else {
                // climb through
                z.win = -1;
                z.pos = w.pos + w.dir * 36.0;
                z.prev = z.pos;
                z.vel = w.dir * 120.0;
            }
        }
        for p in tear_sfx {
            self.sfx(Sfx::Hit, p);
        }
        for (i, b) in changed {
            self.board_event(i, b);
        }
    }

    // ------------------------------------------------------------- rounds

    fn round_size(&self, n: u32) -> u32 {
        let players = self.net.as_ref().map(|n| n.players()).unwrap_or(1) as f32;
        let base = 6.0 + 3.0 * n as f32 + (n * n) as f32 / 6.0;
        (base * (1.0 + 0.5 * (players - 1.0)) * self.difficulty().count).min(220.0) as u32
    }

    fn start_round(&mut self, n: u32) {
        self.wave.n = n;
        let hounds = self.ob.as_ref().map(|o| n == o.next_hounds).unwrap_or(false);
        if let Some(ob) = self.ob.as_mut() {
            ob.hound_round = hounds;
            if hounds {
                ob.next_hounds = n + 4 + self.rng.below(3) as u32;
            }
        }
        self.wave.total = if hounds {
            let players = self.net.as_ref().map(|n| n.players()).unwrap_or(1) as u32;
            (6 + 2 * n).min(48) * players.max(1)
        } else {
            self.round_size(n)
        };
        self.wave.spawned = 0;
        self.wave.timer = 1.0;
        self.wave.boss_pending = n % 10 == 0;
        self.wave.boss_timer = 6.0;
        if let Some(ob) = self.ob.as_mut() {
            ob.drops_round = 0;
            ob.repair_points_round = 0;
        }
        self.banner = Some(Banner {
            title: if hounds { "HOUND ROUND".into() } else { format!("ROUND {}", n) },
            sub: if hounds {
                "Something is hunting you. Watch for the lightning.".into()
            } else if n == 1 {
                "Hold the courtyard. Buy doors, guns and perks with points.".into()
            } else {
                format!("{} undead", self.wave.total)
            },
            t: 0.0,
            dur: 3.0,
            color: if hounds { "#ff7a1f" } else { "#ff4040" },
        });
        self.sfx(Sfx::WaveStart, self.player.pos);
        self.sfx(Sfx::BossRoar, self.player.pos);
        if hounds {
            self.say("fetch me their souls");
        } else {
            self.say(&format!("round {}", n));
        }
        if let Some(net) = &mut self.net {
            let total = self.wave.total;
            net.event(|w| {
                w.u8(net::E_WAVE_START);
                w.u16(n.min(u16::MAX as u32) as u16);
                w.u16(total.min(u16::MAX as u32) as u16);
                w.u8(0);
            });
        }
    }

    fn round_cleared(&mut self) {
        self.wave.intermission = 9.0;
        let hounds = self.ob.as_ref().map(|o| o.hound_round).unwrap_or(false);
        if hounds {
            // the last hound always leaves a gift
            let p = self.player.pos + v2(0.0, -60.0);
            self.pickups.push(Pickup { pos: p, kind: PickKind::MaxAmmo, t: 0.0, dead: false });
        }
        let earned = ((10 + 5 * self.wave.n) as f32 * self.difficulty().coins) as u32;
        self.coins += earned;
        self.score += 100 * self.wave.n as u64;
        self.banner = Some(Banner {
            title: format!("ROUND {} SURVIVED", self.wave.n),
            sub: format!("+{} coins · the next round is coming", earned),
            t: 0.0,
            dur: 3.0,
            color: "#9cffb8",
        });
        self.sfx(Sfx::WaveClear, self.player.pos);
        self.ui_out.push(("wave", format!("{}", self.wave.n)));
        if let Some(net) = &mut self.net {
            let n = self.wave.n;
            net.event(|w| {
                w.u8(net::E_WAVE_CLEAR);
                w.u16(n.min(u16::MAX as u32) as u16);
                w.u16(earned.min(u16::MAX as u32) as u16);
            });
        }
    }

    /// Pick a window in an open zone, near a player.
    fn spawn_window(&mut self) -> Option<usize> {
        let m = world::map()?;
        let ob = self.ob.as_ref()?;
        let targets = self.targets();
        let mut cands: Vec<(usize, f32)> = Vec::new();
        for (i, w) in ob.windows.iter().enumerate() {
            if !m.zone_is_open(w.zone) {
                continue;
            }
            let d = targets
                .iter()
                .filter(|t| t.0 != 255)
                .map(|t| t.1.dist(w.pos))
                .fold(f32::MAX, f32::min);
            let weight = if d < 1500.0 { 1.0 + (1500.0 - d) / 500.0 } else { 0.3 };
            cands.push((i, weight));
        }
        let total: f32 = cands.iter().map(|c| c.1).sum();
        if total <= 0.0 {
            return None;
        }
        let mut r = self.rng.f() * total;
        for (i, w) in cands {
            r -= w;
            if r <= 0.0 {
                return Some(i);
            }
        }
        None
    }

    #[cfg(test)]
    pub fn outbreak_waves_pub(&mut self) {
        self.outbreak_waves();
    }

    fn outbreak_waves(&mut self) {
        if self.wave.intermission > 0.0 {
            self.wave.intermission -= DT;
            if self.wave.intermission <= 0.0 {
                let n = self.wave.n + 1;
                self.start_round(n);
                self.check_egg();
            }
            return;
        }
        let n = self.wave.n;
        let players = self.net.as_ref().map(|n| n.players()).unwrap_or(1) as u32;
        let max_alive = 24 + 6 * players;
        let alive = self.alive();
        self.wave.timer -= DT;
        if self.wave.spawned < self.wave.total && self.wave.timer <= 0.0 && alive < max_alive {
            self.wave.timer = (2.2 - n as f32 * 0.09).max(0.35) * self.rng.range(0.7, 1.3);
            let hounds = self.ob.as_ref().map(|o| o.hound_round).unwrap_or(false);
            if hounds {
                let targets = self.targets();
                let alive_t: Vec<V2> = targets.iter().filter(|t| t.0 != 255).map(|t| t.1).collect();
                if let Some(&tp) = alive_t.get(self.rng.below(alive_t.len().max(1)).min(alive_t.len().saturating_sub(1))) {
                    let a = self.rng.angle();
                    let at = tp + V2::from_angle(a) * self.rng.range(260.0, 420.0);
                    let (at, _) = world::resolve(at, 14.0);
                    self.spawn_zombie(ZKind::Hound, at);
                    self.bolts.push(crate::game::Bolt { pts: vec![at + v2(0.0, -500.0), at + v2(18.0, -250.0), at], life: 0.25 });
                    self.light(at, 260.0, 0.4, "#ffb060");
                    self.ring(at, 60.0, 0.3, "#ff7a1f", 5.0);
                    self.sfx(Sfx::Lightning, at);
                    self.wave.spawned += 1;
                    self.wave.timer *= 0.6;
                }
            } else if let Some(wi) = self.spawn_window() {
                let (pos, _) = {
                    let w = &self.ob.as_ref().unwrap().windows[wi];
                    (w.pos, w.dir)
                };
                let mut k = self.pick_kind();
                if matches!(k, ZKind::Bat | ZKind::Burrower) {
                    k = ZKind::Runner;
                }
                self.spawn_zombie(k, pos);
                if let Some(z) = self.zombies.last_mut() {
                    z.win = wi as i8;
                    z.pos = pos;
                    z.prev = pos;
                }
                if n >= 6 && self.rng.chance(self.elite_chance() * 0.6) {
                    let a = self.roll_affix();
                    self.make_elite(a);
                }
                self.wave.spawned += 1;
            }
        }
        if self.wave.boss_pending {
            self.wave.boss_timer -= DT;
            if self.wave.boss_timer <= 0.0 {
                self.wave.boss_pending = false;
                if let Some(wi) = self.spawn_window() {
                    let pos = self.ob.as_ref().unwrap().windows[wi].pos;
                    let var = crate::content::INFINITE_BOSSES
                        [((n / 10) as usize).saturating_sub(1) % crate::content::INFINITE_BOSSES.len()];
                    self.spawn_boss(var, pos);
                    self.boss_intro(var);
                }
            }
        }
        if self.wave.spawned >= self.wave.total && !self.wave.boss_pending && alive == 0 && n > 0 {
            self.round_cleared();
        }
    }

    /// Keep the flow field current (host): doors change it, players move.
    pub fn update_flow(&mut self) {
        let Some(m) = world::map() else { return };
        let targets: Vec<V2> = self
            .targets()
            .iter()
            .filter(|t| t.0 != 255)
            .map(|t| t.1)
            .collect();
        let Some(f) = self.flow.as_mut() else { return };
        if FlowField::doors_signature(&m) != f.doors_sig {
            f.rebuild_solid(&m);
            f.t = 0.0;
        }
        f.t -= DT;
        if f.t <= 0.0 {
            f.t = 0.25;
            f.compute(&targets);
        }
    }

    /// Outbreak kills: points come from hits; drops are power-ups, not XP.
    pub fn ob_drop(&mut self, pos: V2) {
        self.maybe_drop_powerup(pos);
    }
}

impl ModeHooks for Outbreak {
    fn on_start(&self, g: &mut Game) {
        g.init_outbreak();
        let name = world::map().map(|m| m.name.clone()).unwrap_or_default();
        g.banner = Some(Banner {
            title: "OUTBREAK".into(),
            sub: format!("{} · {}", name, g.difficulty().name),
            t: 0.0,
            dur: 3.0,
            color: "#ff4040",
        });
    }

    fn update_waves(&self, g: &mut Game) {
        g.outbreak_waves();
    }

    fn hud(&self, g: &Game) -> String {
        // rounds 1-5 are drawn as chalk tally marks (render_outbreak.rs)
        if g.wave.n <= 5 {
            String::new()
        } else {
            format!("ROUND {}", g.wave.n)
        }
    }

    fn is_over(&self, _g: &Game) -> bool {
        false
    }
}
