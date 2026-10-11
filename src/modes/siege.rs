//! Siege: defend the crystal. Build phases (place and upgrade defenses with
//! mana) alternate with combat waves pouring out of portals. Enemies follow a
//! flow field to the crystal, around your barricades, and chew through them
//! if you wall the crystal off completely. Survive every wave to win, rated
//! in stars by how much of the crystal is left.

use super::ModeHooks;
use crate::audio::Sfx;
use crate::game::{Banner, Bullet, EShot, Game, Gem, ZKind, DT};
use crate::math::*;
use crate::nav::FlowField;
use crate::net;
use crate::status::Elem;
use crate::world;

pub struct Siege;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum DefKind {
    Tower = 0,
    Flame,
    Frost,
    Barricade,
    Mine,
    Totem,
}

pub const DEFS: [DefKind; 6] = [
    DefKind::Tower,
    DefKind::Flame,
    DefKind::Frost,
    DefKind::Barricade,
    DefKind::Mine,
    DefKind::Totem,
];

impl DefKind {
    pub fn from_u8(v: u8) -> Option<DefKind> {
        DEFS.get(v as usize).copied()
    }
    /// (name, icon, color, mana cost)
    pub fn def(self) -> (&'static str, &'static str, &'static str, u32) {
        match self {
            DefKind::Tower => ("Arrow Tower", "🏹", "#c8a060", 40),
            DefKind::Flame => ("Flame Trap", "🔥", "#ff7a1f", 30),
            DefKind::Frost => ("Frost Aura", "❄️", "#9ff3ff", 35),
            DefKind::Barricade => ("Barricade", "🧱", "#a07850", 20),
            DefKind::Mine => ("Proximity Mine", "💣", "#ff5050", 25),
            DefKind::Totem => ("Healing Totem", "💚", "#9cffb8", 50),
        }
    }
    /// Blocks movement (zombies must go around or break it).
    pub fn solid(self) -> bool {
        matches!(self, DefKind::Tower | DefKind::Barricade | DefKind::Totem)
    }
    pub fn radius(self) -> f32 {
        match self {
            DefKind::Barricade => 30.0,
            DefKind::Tower => 22.0,
            DefKind::Totem => 18.0,
            _ => 16.0,
        }
    }
    pub fn base_hp(self) -> f32 {
        match self {
            DefKind::Barricade => 500.0,
            DefKind::Tower => 260.0,
            DefKind::Totem => 220.0,
            _ => 120.0,
        }
    }
    /// effect range per tier (1..=3)
    pub fn range(self, tier: u8) -> f32 {
        let t = tier as f32;
        match self {
            DefKind::Tower => 320.0 + 40.0 * t,
            DefKind::Flame => 55.0 + 15.0 * t,
            DefKind::Frost => 120.0 + 30.0 * t,
            DefKind::Mine => 110.0 + 30.0 * t,
            DefKind::Totem => 140.0 + 20.0 * t,
            DefKind::Barricade => 0.0,
        }
    }
}

pub struct Defense {
    pub kind: DefKind,
    pub pos: V2,
    pub tier: u8,
    pub hp: f32,
    pub max_hp: f32,
    pub cd: f32,
    pub aim: f32,
    pub flash: f32,
    pub uid: u32,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SiegePhase {
    Build,
    Combat,
}

pub struct SiegeState {
    pub phase: SiegePhase,
    pub build_t: f32,
    pub mana: u32,
    pub crystal: V2,
    pub crystal_hp: f32,
    pub crystal_max: f32,
    pub crystal_flash: f32,
    pub portals: Vec<(V2, String)>,
    pub defenses: Vec<Defense>,
    /// defense being placed (build mode)
    pub build_sel: Option<DefKind>,
    /// enemies still to come this wave, per portal
    pub queue: Vec<Vec<ZKind>>,
    pub spawn_t: f32,
    pub final_wave: u32,
    pub next_uid: u32,
    /// all paths blocked: the flow ignores barricades so zombies break through
    pub walled_in: bool,
    /// "Glass Crystal" challenge: 40% crystal health, double coins
    pub glass: bool,
    /// co-op ready vote for ending the build phase (by slot)
    pub ready: [bool; 3],
}

pub const GRID: f32 = 40.0;
pub const BUILD_T: f32 = 35.0;

impl Game {
    fn init_siege(&mut self) {
        if self.map_id.is_empty() {
            self.map_id = "keep".into();
            let m = world::load_builtin(&self.map_id);
            if let Some(m) = &m {
                self.player.pos = m.spawn;
                self.player.prev = m.spawn;
            }
            world::use_map(m);
        }
        let Some(m) = world::map() else { return };
        let crystal = m
            .interact
            .iter()
            .find(|i| i.kind == "crystal")
            .map(|i| v2(i.pos[0], i.pos[1]))
            .unwrap_or(V2::ZERO);
        let portals = m
            .interact
            .iter()
            .filter(|i| i.kind == "portal")
            .map(|i| (v2(i.pos[0], i.pos[1]), i.id.clone()))
            .collect();
        self.flow = Some(FlowField::new(&m));
        let glass = self.variant == "glass";
        let hp = 1500.0 / self.difficulty().dmg.sqrt() * if glass { 0.4 } else { 1.0 };
        self.siege = Some(Box::new(SiegeState {
            phase: SiegePhase::Build,
            build_t: 45.0,
            mana: 140,
            crystal,
            crystal_hp: hp,
            crystal_max: hp,
            crystal_flash: 0.0,
            portals,
            defenses: Vec::new(),
            build_sel: None,
            queue: Vec::new(),
            spawn_t: 0.0,
            final_wave: 15,
            next_uid: 1,
            walled_in: false,
            glass,
            ready: [false; 3],
        }));
        self.wave.n = 0;
        self.wave.intermission = 0.0;
    }

    /// Ghost position for placing a defense: the cursor (snapped) or in front of you on touch.
    pub fn build_ghost(&self) -> V2 {
        let raw = if self.input.is_touch || !self.input.mouse_seen {
            self.player.pos + V2::from_angle(self.player.aim) * 70.0
        } else {
            self.cam + (self.input.mouse - self.screen * 0.5) * (1.0 / self.zoom.max(0.01))
        };
        v2((raw.x / GRID).round() * GRID, (raw.y / GRID).round() * GRID)
    }

    pub fn can_place(&self, kind: DefKind, at: V2) -> Result<(), &'static str> {
        let s = self.siege.as_ref().ok_or("no siege")?;
        if s.mana < kind.def().3 {
            return Err("Not enough mana");
        }
        if at.dist(self.player.pos) > 340.0 {
            return Err("Too far away");
        }
        if at.dist(s.crystal) < 90.0 {
            return Err("Too close to the crystal");
        }
        if s.portals.iter().any(|p| p.0.dist(at) < 120.0) {
            return Err("Too close to a portal");
        }
        if s.defenses.iter().any(|d| d.pos.dist(at) < d.kind.radius() + kind.radius() + 4.0) {
            return Err("Something is already there");
        }
        let (fixed, n) = world::resolve(at, kind.radius());
        if n.len2() > 0.0 || fixed.dist(at) > 1.0 {
            return Err("Blocked");
        }
        Ok(())
    }

    /// Place a defense (the host applies it; clients ask the host).
    pub fn place_defense(&mut self, kind: DefKind, at: V2) -> bool {
        if let Err(e) = self.can_place(kind, at) {
            self.toast = Some((e.into(), 1.2));
            self.audio.play(Sfx::UiMove);
            return false;
        }
        if let Some(s) = self.siege.as_mut() {
            s.mana -= kind.def().3;
        }
        if self.is_client() {
            let mut w = net::W::new();
            w.u8(net::MSG_BUILD);
            w.u8(kind as u8);
            w.v2(at);
            self.net_out.push((0, true, w.0));
        } else {
            self.add_defense(kind, at, 1);
        }
        self.audio.play(Sfx::UiSelect);
        self.burst(at, 14, 180.0, kind.def().2, 3.0, 0.4, false, true);
        true
    }

    pub fn add_defense(&mut self, kind: DefKind, at: V2, tier: u8) {
        let mult = self.defense_mult();
        let Some(s) = self.siege.as_mut() else { return };
        let uid = s.next_uid;
        s.next_uid += 1;
        let hp = kind.base_hp() * (1.0 + 0.6 * (tier as f32 - 1.0)) * mult;
        s.defenses.push(Defense { kind, pos: at, tier, hp, max_hp: hp, cd: 0.5, aim: 0.0, flash: 0.0, uid });
        if let Some(f) = self.flow.as_mut() {
            f.doors_sig = u64::MAX; // re-path
        }
        if self.is_host() {
            if let Some(n) = &mut self.net {
                n.event(|w| {
                    w.u8(net::E_DEFENSE);
                    w.u32(uid);
                    w.u8(kind as u8);
                    w.v2(at);
                    w.u8(tier);
                });
            }
        }
    }

    /// Engineers build stronger defenses; everything scales with your level.
    pub fn defense_mult(&self) -> f32 {
        let eng = if self.kit() == crate::content::Kit::Turret { 1.25 } else { 1.0 };
        eng * (1.0 + 0.03 * (self.player.level.saturating_sub(1)) as f32)
    }

    /// Nearest defense within reach of the player (for upgrade/sell prompts).
    pub fn defense_in_reach(&self) -> Option<usize> {
        let s = self.siege.as_ref()?;
        let p = self.player.pos;
        s.defenses
            .iter()
            .enumerate()
            .filter(|(_, d)| d.pos.dist(p) < 80.0)
            .min_by(|a, b| a.1.pos.dist(p).partial_cmp(&b.1.pos.dist(p)).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(i, _)| i)
    }

    pub fn upgrade_cost(d: &Defense) -> u32 {
        d.kind.def().3 * d.tier as u32
    }

    /// Interact near a defense: upgrade it (or repair when maxed).
    pub fn siege_interact(&mut self) -> bool {
        let Some(i) = self.defense_in_reach() else {
            return false;
        };
        let s = self.siege.as_mut().unwrap();
        let d = &s.defenses[i];
        if d.tier >= 3 {
            // repair to full for a little mana
            if d.hp < d.max_hp && s.mana >= 10 {
                s.mana -= 10;
                let d = &mut s.defenses[i];
                d.hp = d.max_hp;
                self.audio.play(Sfx::Heal);
            } else {
                self.audio.play(Sfx::UiMove);
            }
            return true;
        }
        let cost = Game::upgrade_cost(d);
        if s.mana < cost {
            self.toast = Some(("Not enough mana".into(), 1.0));
            self.audio.play(Sfx::UiMove);
            return true;
        }
        s.mana -= cost;
        let uid = s.defenses[i].uid;
        self.siege_world(net::B_UPGRADE, uid);
        true
    }

    /// Sell the defense in reach for 60% of what went into it.
    pub fn sell_defense(&mut self) -> bool {
        let Some(i) = self.defense_in_reach() else { return false };
        let s = self.siege.as_mut().unwrap();
        let d = &s.defenses[i];
        let spent: u32 = (1..=d.tier as u32).map(|t| d.kind.def().3 * t.max(1)).sum::<u32>();
        s.mana += spent * 6 / 10;
        let uid = d.uid;
        self.siege_world(net::B_SELL, uid);
        self.audio.play(Sfx::Pickup);
        true
    }

    pub fn siege_world_pub(&mut self, kind: u8, uid: u32) {
        self.siege_world(kind, uid);
    }

    fn siege_world(&mut self, kind: u8, uid: u32) {
        if self.is_client() {
            let mut w = net::W::new();
            w.u8(net::MSG_DEFENSE);
            w.u8(kind);
            w.u32(uid);
            self.net_out.push((0, true, w.0));
        }
        self.apply_defense_change(kind, uid);
        if self.is_host() {
            if let Some(n) = &mut self.net {
                n.event(|w| {
                    w.u8(net::E_DEFCHANGE);
                    w.u8(kind);
                    w.u32(uid);
                });
            }
        }
    }

    pub fn apply_defense_change(&mut self, kind: u8, uid: u32) {
        let Some(s) = self.siege.as_mut() else { return };
        let Some(i) = s.defenses.iter().position(|d| d.uid == uid) else { return };
        match kind {
            net::B_UPGRADE => {
                let d = &mut s.defenses[i];
                if d.tier < 3 {
                    d.tier += 1;
                    let hp = d.kind.base_hp() * (1.0 + 0.6 * (d.tier as f32 - 1.0));
                    d.max_hp = hp;
                    d.hp = hp;
                    let (p, col) = (d.pos, d.kind.def().2);
                    self.ring(p, 60.0, 0.4, col, 5.0);
                    self.audio.play(Sfx::LevelUp);
                }
            }
            net::B_SELL | net::B_DESTROY => {
                let d = s.defenses.remove(i);
                self.burst(d.pos, 18, 220.0, "#8a7a6a", 4.0, 0.6, true, false);
                if kind == net::B_DESTROY {
                    self.sfx(Sfx::BigDie, d.pos);
                }
                if let Some(f) = self.flow.as_mut() {
                    f.doors_sig = u64::MAX;
                }
            }
            _ => {}
        }
    }

    /// Build bar under the HUD: one button per defense (screen px).
    pub fn build_bar(&self) -> [(f32, f32, f32, f32); 6] {
        let bw = if self.screen.x < 640.0 { 50.0 } else { 58.0 };
        let gap = 6.0;
        let total = 6.0 * bw + 5.0 * gap;
        let x0 = self.screen.x * 0.5 - total * 0.5;
        let y = self.screen.y - if self.input.is_touch { 200.0 } else { 160.0 };
        let mut out = [(0.0, 0.0, 0.0, 0.0); 6];
        for (k, o) in out.iter_mut().enumerate() {
            *o = (x0 + k as f32 * (bw + gap), y, bw, bw);
        }
        out
    }

    pub fn select_build(&mut self, k: usize) {
        let Some(s) = self.siege.as_mut() else { return };
        let kind = DEFS.get(k).copied();
        s.build_sel = if s.build_sel == kind { None } else { kind };
        self.audio.play(Sfx::UiMove);
    }

    /// A click/tap while in build mode: pick from the bar, or place.
    pub fn siege_click(&mut self, at: V2) -> bool {
        if self.siege.is_none() {
            return false;
        }
        for (k, r) in self.build_bar().iter().enumerate() {
            if at.x >= r.0 && at.x <= r.0 + r.2 && at.y >= r.1 && at.y <= r.1 + r.3 {
                self.select_build(k);
                return true;
            }
        }
        let Some(kind) = self.siege.as_ref().and_then(|s| s.build_sel) else { return false };
        let ghost = self.build_ghost();
        self.place_defense(kind, ghost);
        true
    }

    /// Ready to fight. Solo: start now. Co-op: vote; the host starts when everyone is ready.
    pub fn ready_up(&mut self) -> bool {
        let me = self.my_slot();
        let solo = self.net.is_none();
        let Some(s) = self.siege.as_mut() else { return false };
        if s.phase != SiegePhase::Build {
            return false;
        }
        if solo {
            s.build_t = s.build_t.min(0.5);
            return true;
        }
        s.ready[me as usize % 3] = true;
        if self.is_client() {
            let mut w = net::W::new();
            w.u8(net::MSG_READY);
            self.net_out.push((0, true, w.0));
        } else {
            self.check_ready_vote();
        }
        self.toast = Some(("Ready! Waiting for your team…".into(), 1.5));
        true
    }

    pub fn mark_ready(&mut self, slot: u8) {
        if let Some(s) = self.siege.as_mut() {
            s.ready[slot as usize % 3] = true;
        }
        self.check_ready_vote();
    }

    fn check_ready_vote(&mut self) {
        let present: Vec<u8> = self.targets().iter().filter(|t| t.0 != 255).map(|t| t.0).collect();
        let Some(s) = self.siege.as_mut() else { return };
        if present.iter().all(|p| s.ready[*p as usize % 3]) {
            s.build_t = s.build_t.min(0.5);
        }
    }

    /// The enemies of wave `n`, per portal.
    fn wave_plan(&mut self, n: u32) -> Vec<Vec<ZKind>> {
        let portals = self.siege.as_ref().map(|s| s.portals.len()).unwrap_or(1);
        let open = if n >= 5 { portals } else if n >= 3 { portals.min(2) } else { 1 };
        let players = self.net.as_ref().map(|n| n.players()).unwrap_or(1) as f32;
        let total = ((10.0 + 5.0 * n as f32 + n as f32 * n as f32 * 0.25) * (1.0 + 0.5 * (players - 1.0)) * self.difficulty().count) as usize;
        let mut plan = vec![Vec::new(); portals];
        for k in 0..total {
            let lane = k % open;
            let r = self.rng.f();
            let kind = if n >= 4 && r < 0.12 {
                ZKind::Sapper
            } else if n >= 6 && r < 0.24 {
                ZKind::Archer
            } else if r < 0.5 && n >= 2 {
                ZKind::Runner
            } else if n >= 5 && r < 0.6 {
                ZKind::Brute
            } else {
                ZKind::Walker
            };
            plan[lane].push(kind);
        }
        if n >= 8 && n % 3 == 2 {
            for lane in plan.iter_mut().take(open) {
                lane.push(ZKind::Ogre);
            }
        }
        plan
    }

    fn start_siege_wave(&mut self, n: u32) {
        self.wave.n = n;
        let plan = self.wave_plan(n);
        let total: usize = plan.iter().map(|l| l.len()).sum();
        let last = self.siege.as_ref().map(|s| s.final_wave).unwrap_or(15);
        self.wave.boss_pending = n == last;
        self.wave.boss_timer = 8.0;
        if let Some(s) = self.siege.as_mut() {
            s.phase = SiegePhase::Combat;
            s.ready = [false; 3];
            s.queue = plan;
            s.spawn_t = 0.5;
            s.build_sel = None;
        }
        self.wave.total = total as u32;
        self.wave.spawned = 0;
        self.banner = Some(Banner {
            title: if n == last { format!("FINAL WAVE {}", n) } else { format!("WAVE {}/{}", n, last) },
            sub: format!("{} enemies · protect the crystal", total),
            t: 0.0,
            dur: 2.6,
            color: "#7ad7ff",
        });
        self.sfx(Sfx::WaveStart, self.player.pos);
        self.say(&format!("wave {}", n));
        if let Some(net) = &mut self.net {
            net.event(|w| {
                w.u8(net::E_WAVE_START);
                w.u16(n as u16);
                w.u16(total.min(u16::MAX as usize) as u16);
                w.u8(0);
            });
        }
    }

    fn siege_wave_cleared(&mut self) {
        let n = self.wave.n;
        let last = self.siege.as_ref().map(|s| s.final_wave).unwrap_or(15);
        let glass = self.siege.as_ref().map(|s| s.glass).unwrap_or(false);
        let earned = ((10 + 5 * n) as f32 * self.difficulty().coins * if glass { 2.0 } else { 1.0 }) as u32;
        self.coins += earned;
        self.score += 100 * n as u64;
        if n >= last {
            if let Some(net) = &mut self.net {
                net.event(|w| {
                    w.u8(net::E_VICTORY);
                    w.u16(earned.min(u16::MAX as u32) as u16);
                });
            }
            self.siege_victory();
            return;
        }
        if let Some(s) = self.siege.as_mut() {
            s.phase = SiegePhase::Build;
            s.build_t = BUILD_T;
            s.mana += 30 + 5 * n;
        }
        self.banner = Some(Banner {
            title: format!("WAVE {} HELD", n),
            sub: format!("+{} coins · build phase: place and upgrade defenses", earned),
            t: 0.0,
            dur: 2.6,
            color: "#9cffb8",
        });
        self.sfx(Sfx::WaveClear, self.player.pos);
        self.ui_out.push(("wave", format!("{}", n)));
        if let Some(net) = &mut self.net {
            net.event(|w| {
                w.u8(net::E_WAVE_CLEAR);
                w.u16(n as u16);
                w.u16(earned.min(u16::MAX as u32) as u16);
            });
        }
    }

    /// The last wave is held: stars from the crystal's health (clients run this
    /// from `E_VICTORY` with the replicated crystal hp).
    pub(crate) fn siege_victory(&mut self) {
        if self.victory_t >= 0.0 {
            return;
        }
        let glass = self.siege.as_ref().map(|s| s.glass).unwrap_or(false);
        let stars = self.siege_stars();
        let bonus = (60 * stars) as u32;
        self.coins += bonus;
        self.victory_t = 0.0;
        self.ui_out.push(("siegestars", format!("{}:{}:{}", self.map_id, stars, glass as u8)));
        self.banner = Some(Banner {
            title: format!("KEEP DEFENDED {}", "★".repeat(stars as usize)),
            sub: format!("+{} bonus coins", bonus),
            t: 0.0,
            dur: 3.2,
            color: "#ffd36a",
        });
        self.audio.play(Sfx::WaveClear);
        self.audio.play(Sfx::LevelUp);
    }

    /// 3 stars above 80% crystal health, 2 above 40%, else 1.
    pub fn siege_stars(&self) -> u32 {
        let k = self.siege.as_ref().map(|s| s.crystal_hp / s.crystal_max).unwrap_or(0.0);
        if k > 0.8 {
            3
        } else if k > 0.4 {
            2
        } else {
            1
        }
    }

    fn siege_waves(&mut self) {
        let Some(s) = self.siege.as_mut() else { return };
        match s.phase {
            SiegePhase::Build => {
                s.build_t -= DT;
                if s.build_t <= 0.0 {
                    let n = self.wave.n + 1;
                    self.start_siege_wave(n);
                }
            }
            SiegePhase::Combat => {
                s.spawn_t -= DT;
                let alive = self.zombies.iter().filter(|z| !z.dead).count() as u32;
                if s.spawn_t <= 0.0 && alive < 90 {
                    s.spawn_t = (1.0 - self.wave.n as f32 * 0.03).max(0.35);
                    let mut spawns: Vec<(V2, ZKind)> = Vec::new();
                    for (lane, q) in s.queue.iter_mut().enumerate() {
                        if let Some(k) = q.pop() {
                            spawns.push((s.portals[lane].0, k));
                        }
                    }
                    for (p, k) in spawns {
                        let at = p + self.rng.unit() * 30.0;
                        self.spawn_zombie(k, at);
                        self.ring(p, 50.0, 0.3, "#b48cff", 4.0);
                        self.wave.spawned += 1;
                    }
                }
                if self.wave.boss_pending {
                    self.wave.boss_timer -= DT;
                    if self.wave.boss_timer <= 0.0 {
                        self.wave.boss_pending = false;
                        let p = self.siege.as_ref().unwrap().portals[0].0;
                        let var = crate::content::BossVar::Butcher;
                        self.spawn_boss(var, p);
                        self.boss_intro(var);
                    }
                }
                let queued: usize = self.siege.as_ref().unwrap().queue.iter().map(|q| q.len()).sum();
                let alive = self.zombies.iter().filter(|z| !z.dead).count();
                if queued == 0 && alive == 0 && !self.wave.boss_pending && self.wave.n > 0 {
                    self.siege_wave_cleared();
                }
            }
        }
    }

    /// Flow toward the crystal, around solid defenses (host).
    pub fn update_siege_flow(&mut self) {
        let Some(m) = world::map() else { return };
        let Some(s) = self.siege.as_ref() else { return };
        let crystal = s.crystal;
        let solids: Vec<(V2, f32)> = s
            .defenses
            .iter()
            .filter(|d| d.kind.solid())
            .map(|d| (d.pos, d.kind.radius()))
            .collect();
        let portals: Vec<V2> = s.portals.iter().map(|p| p.0).collect();
        let Some(f) = self.flow.as_mut() else { return };
        f.t -= DT;
        if f.doors_sig != FlowField::doors_signature(&m) ^ (solids.len() as u64).wrapping_mul(0x9E37) || f.t <= -10.0 {
            f.rebuild_solid(&m);
            f.add_solids(&solids);
            f.compute(&[crystal]);
            let walled = portals.iter().any(|p| !f.reachable_from(*p));
            if walled {
                // every path is blocked: path straight through, they'll break the barricades
                f.rebuild_solid(&m);
                f.compute(&[crystal]);
            }
            f.doors_sig = FlowField::doors_signature(&m) ^ (solids.len() as u64).wrapping_mul(0x9E37);
            f.t = 0.0;
            if let Some(s) = self.siege.as_mut() {
                s.walled_in = walled;
            }
        }
    }

    /// Defenses act, enemies hit defenses and the crystal (host).
    pub fn update_defenses(&mut self) {
        let dmult = self.defense_mult();
        let Some(s) = self.siege.as_mut() else { return };
        s.crystal_flash = (s.crystal_flash - DT * 3.0).max(0.0);
        let wave = self.wave.n as f32;
        let mut shots: Vec<(V2, V2, f32)> = Vec::new();
        let mut auras: Vec<(V2, f32, DefKind, u8)> = Vec::new();
        let mut mines: Vec<(usize, V2, f32, u8)> = Vec::new();
        let mut heal: Vec<(V2, f32, u8)> = Vec::new();
        let zs = &self.zombies;
        for (i, d) in s.defenses.iter_mut().enumerate() {
            d.cd -= DT;
            d.flash = (d.flash - DT * 4.0).max(0.0);
            let range = d.kind.range(d.tier);
            match d.kind {
                DefKind::Tower => {
                    if d.cd <= 0.0 {
                        let mut best: Option<(f32, V2, V2)> = None;
                        for z in zs.iter().filter(|z| !z.dead && !z.hidden) {
                            let dd = z.pos.dist2(d.pos);
                            if dd < range * range && best.map_or(true, |b| dd < b.0) {
                                best = Some((dd, z.pos, z.vel));
                            }
                        }
                        if let Some((dd, zp, zv)) = best {
                            let lead = zp + zv * (dd.sqrt() / 900.0);
                            d.aim = (lead - d.pos).angle();
                            d.cd = [0.75, 0.55, 0.38][(d.tier - 1) as usize];
                            let dmg = [14.0, 22.0, 34.0][(d.tier - 1) as usize] * (1.0 + 0.08 * wave) * dmult;
                            shots.push((d.pos, V2::from_angle(d.aim), dmg));
                        }
                    }
                }
                DefKind::Flame | DefKind::Frost => {
                    if d.cd <= 0.0 {
                        d.cd = if d.kind == DefKind::Flame { 0.5 } else { 1.2 };
                        auras.push((d.pos, range, d.kind, d.tier));
                    }
                }
                DefKind::Mine => {
                    if zs.iter().any(|z| !z.dead && !z.hidden && z.pos.dist(d.pos) < 40.0 + z.r) {
                        let dmg = [120.0, 220.0, 360.0][(d.tier - 1) as usize] * (1.0 + 0.08 * wave);
                        mines.push((i, d.pos, dmg, d.tier));
                    }
                }
                DefKind::Totem => {
                    if d.cd <= 0.0 {
                        d.cd = 0.5;
                        heal.push((d.pos, range, d.tier));
                    }
                }
                DefKind::Barricade => {}
            }
        }
        for (from, dir, dmg) in shots {
            let start = from + dir * 22.0 + v2(0.0, -20.0);
            self.bullets.push(Bullet {
                pos: start,
                prev: start,
                vel: dir * 1000.0,
                dmg,
                life: 0.5,
                pierce: 0,
                bounces: 0,
                r: 4.0,
                crit: false,
                last_hit: 0,
                dead: false,
                blast: 0.0,
                knock: 90.0,
                cosmetic: false,
                tracer: 0,
            });
            self.sfx(Sfx::Shoot, from);
        }
        for (p, r, kind, tier) in auras {
            let mut hits = Vec::new();
            self.grid.query(p, r + 30.0, |j| hits.push(j));
            let power = self.elem_power() * (0.7 + 0.3 * tier as f32);
            for j in hits {
                if self.zombies[j].dead || self.zombies[j].pos.dist(p) > r + self.zombies[j].r {
                    continue;
                }
                let e = if kind == DefKind::Flame { Elem::Burn } else { Elem::Chill };
                self.afflict(j, e, power);
                if kind == DefKind::Flame && !self.zombies[j].dead {
                    let dir = (self.zombies[j].pos - p).norm();
                    self.damage_zombie(j, 4.0 * tier as f32 * (1.0 + 0.06 * wave), dir, 20.0, false);
                }
            }
            if kind == DefKind::Frost {
                self.ring(p, r, 0.4, "#9ff3ff", 3.0);
            } else {
                self.burst(p, 6, r * 2.0, "#ff7a1f", 3.0, 0.3, false, true);
            }
        }
        for (i, p, dmg, _) in mines.into_iter().rev() {
            self.explode(p, 110.0, dmg / 1.5, false);
            if let Some(s) = self.siege.as_ref() {
                if let Some(d) = s.defenses.get(i) {
                    let uid = d.uid;
                    self.siege_world(net::B_DESTROY, uid);
                }
            }
        }
        for (p, r, tier) in heal {
            let amt = [2.0, 3.5, 5.5][(tier - 1) as usize];
            if self.player_alive() && self.player.pos.dist(p) < r {
                self.player.hp = (self.player.hp + amt).min(self.player.max_hp);
            }
            if let Some(s) = self.siege.as_mut() {
                if s.crystal.dist(p) < r + 60.0 {
                    s.crystal_hp = (s.crystal_hp + amt * 0.6).min(s.crystal_max);
                }
            }
        }
        self.enemy_vs_structures();
    }

    /// Zombies attack the crystal and any solid defense they touch; arrows hit defenses.
    fn enemy_vs_structures(&mut self) {
        let Some(s) = self.siege.as_mut() else { return };
        let mut destroyed: Vec<u32> = Vec::new();
        let mut crystal_hits = 0.0;
        let mut blasts: Vec<V2> = Vec::new();
        for z in self.zombies.iter_mut() {
            if z.dead || z.hidden {
                continue;
            }
            // crystal
            if z.pos.dist(s.crystal) < z.r + 48.0 {
                if z.attack_cd <= 0.0 {
                    z.attack_cd = 1.0;
                    crystal_hits += z.dmg * if z.kind == ZKind::Ogre { 2.0 } else { 1.0 };
                }
                continue;
            }
            for d in s.defenses.iter_mut() {
                let rr = z.r + d.kind.radius() + 4.0;
                let touching = z.pos.dist(d.pos) < rr;
                if !touching || !(d.kind.solid() || z.kind == ZKind::Sapper) {
                    continue;
                }
                if z.kind == ZKind::Sapper {
                    z.hp = 0.0;
                    z.flash = 1.0;
                    d.hp -= 180.0 + 12.0 * self.wave.n as f32;
                    blasts.push(z.pos);
                } else if z.attack_cd <= 0.0 {
                    z.attack_cd = 0.9;
                    d.hp -= z.dmg * if z.kind == ZKind::Ogre { 4.0 } else { 1.5 };
                    d.flash = 1.0;
                }
                if d.hp <= 0.0 && !destroyed.contains(&d.uid) {
                    destroyed.push(d.uid);
                }
                // and it can't walk through
                let away = (z.pos - d.pos).norm();
                z.pos = d.pos + away * rr;
                break;
            }
        }
        if crystal_hits > 0.0 {
            s.crystal_hp -= crystal_hits;
            s.crystal_flash = 1.0;
            let hp = s.crystal_hp;
            if let Some(n) = &mut self.net {
                n.event(|w| {
                    w.u8(net::E_CRYSTAL);
                    w.f32(hp.max(0.0));
                });
            }
        }
        let crystal_hit = if crystal_hits > 0.0 { Some(s.crystal) } else { None };
        // enemy arrows and spit hit defenses too
        for e in self.eshots.iter_mut() {
            if let Some(d) = s.defenses.iter_mut().find(|d| d.kind.solid() && d.pos.dist(e.pos) < d.kind.radius() + 6.0) {
                d.hp -= e.dmg;
                d.flash = 1.0;
                e.dead = true;
                if d.hp <= 0.0 && !destroyed.contains(&d.uid) {
                    destroyed.push(d.uid);
                }
            }
        }
        let lost = s.crystal_hp <= 0.0;
        if let Some(p) = crystal_hit {
            self.sfx(Sfx::Hurt, p);
            self.add_shake(0.15);
        }
        for p in blasts {
            self.ring(p, 80.0, 0.3, "#ffb24a", 6.0);
            self.sfx(Sfx::Explosion, p);
        }
        // sappers that blew themselves up
        for j in 0..self.zombies.len() {
            if !self.zombies[j].dead && self.zombies[j].kind == ZKind::Sapper && self.zombies[j].hp <= 0.0 {
                self.zombies[j].last_hitter = 255;
                self.kill_zombie(j);
            }
        }
        for uid in destroyed {
            self.siege_world(net::B_DESTROY, uid);
        }
        if lost && self.phase == crate::game::Phase::Playing {
            self.crystal_lost();
        }
    }

    fn crystal_lost(&mut self) {
        if let Some(s) = self.siege.as_mut() {
            s.crystal_hp = 0.0;
        }
        let p = self.siege.as_ref().map(|s| s.crystal).unwrap_or(V2::ZERO);
        self.ring(p, 500.0, 0.8, "#7ad7ff", 14.0);
        self.burst(p, 80, 700.0, "#bfe8ff", 4.0, 1.2, false, true);
        self.banner = Some(Banner {
            title: "THE CRYSTAL SHATTERED".into(),
            sub: format!("Held until wave {}", self.wave.n),
            t: 0.0,
            dur: 3.0,
            color: "#ff6a6a",
        });
        if self.net.is_some() {
            self.check_team_wipe_forced();
        } else {
            self.phase = crate::game::Phase::Dying;
            self.dying_t = 0.0;
            self.audio.play(Sfx::GameOver);
        }
    }

    /// Siege kills drop mana orbs as well as XP.
    pub fn siege_drop(&mut self, pos: V2, xp: u32) {
        if self.siege.is_none() {
            return;
        }
        let v = 2 + xp * 2;
        let vel = self.rng.unit() * 90.0;
        self.gems.push(Gem {
            pos,
            prev: pos,
            vel,
            value: v,
            t: 0.0,
            magnet: false,
            dead: false,
            mana: true,
        });
    }

    pub fn gain_mana(&mut self, v: u32) {
        if let Some(s) = self.siege.as_mut() {
            s.mana += v;
        }
    }


    /// Archers shoot at the nearest defense or player in range (host).
    pub fn archer_volley(&mut self, from: V2, dmg: f32) -> bool {
        let s = match self.siege.as_ref() {
            Some(s) => s,
            None => return false,
        };
        let mut best: Option<(f32, V2)> = None;
        for d in s.defenses.iter().filter(|d| d.kind.solid()) {
            let dd = d.pos.dist(from);
            if dd < 480.0 && best.map_or(true, |b| dd < b.0) {
                best = Some((dd, d.pos));
            }
        }
        if self.player_alive() {
            let dd = self.player.pos.dist(from);
            if dd < 480.0 && best.map_or(true, |b| dd < b.0) {
                best = Some((dd, self.player.pos));
            }
        }
        let Some((_, at)) = best else { return false };
        let dir = (at - from).norm();
        self.eshots.push(EShot {
            pos: from + dir * 18.0,
            prev: from,
            vel: dir * 420.0,
            life: 1.4,
            dmg,
            dead: false,
            turn: 0.0,
            r: 5.0,
            grazed: false,
        });
        true
    }


}

impl ModeHooks for Siege {
    fn on_start(&self, g: &mut Game) {
        g.init_siege();
        let name = world::map().map(|m| m.name.clone()).unwrap_or_default();
        g.banner = Some(Banner {
            title: "SIEGE".into(),
            sub: format!("{} · build defenses, then press Enter to start", name),
            t: 0.0,
            dur: 3.5,
            color: "#7ad7ff",
        });
    }

    fn update_waves(&self, g: &mut Game) {
        g.siege_waves();
    }

    fn hud(&self, g: &Game) -> String {
        let last = g.siege.as_ref().map(|s| s.final_wave).unwrap_or(15);
        format!("WAVE {}/{}", g.wave.n.max(1), last)
    }
}
