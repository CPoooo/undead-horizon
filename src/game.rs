use crate::audio::{Audio, Sfx};
use crate::content::{self, BossVar, CHAPTERS, DIFFICULTIES, GUNS};
use crate::math::*;
use crate::upgrades::{Levels, Up};
use crate::world;
use std::f32::consts::{PI, TAU};

/// Simulation runs at a fixed 120 Hz; rendering interpolates between steps.
pub const DT: f32 = 1.0 / 120.0;
pub const PR: f32 = 16.0; // player radius

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Phase {
    Menu,
    Playing,
    LevelUp,
    Paused,
    Dying,
    GameOver,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ZKind {
    Walker,
    Runner,
    Brute,
    Spitter,
    Exploder,
    Boss,
}

impl ZKind {
    /// (hp, speed, radius, contact damage, xp)
    fn base(self) -> (f32, f32, f32, f32, u32) {
        match self {
            ZKind::Walker => (20.0, 72.0, 15.0, 10.0, 1),
            ZKind::Runner => (12.0, 150.0, 13.0, 8.0, 1),
            ZKind::Brute => (150.0, 54.0, 26.0, 22.0, 6),
            ZKind::Spitter => (26.0, 82.0, 15.0, 10.0, 3),
            ZKind::Exploder => (18.0, 112.0, 16.0, 8.0, 2),
            ZKind::Boss => (1700.0, 74.0, 46.0, 30.0, 60),
        }
    }
}

pub struct Player {
    pub pos: V2,
    pub prev: V2,
    pub vel: V2,
    pub aim: f32,
    pub hp: f32,
    pub hp_ghost: f32,
    pub max_hp: f32,
    pub iframes: f32,
    pub dash_t: f32,
    pub dash_cd: f32,
    pub dash_dir: V2,
    pub dash_buf: f32,
    pub dash_id: u32,
    pub fire_cd: f32,
    pub recoil: f32,
    pub walk: f32,
    pub hurt_flash: f32,
    pub level: u32,
    pub xp: f32,
    pub xp_next: f32,
    pub ult: f32,
    pub frenzy: f32,
    pub blade_angle: f32,
    pub nova_t: f32,
    pub bolt_t: f32,
    pub ghost_t: f32,
}

pub struct Zombie {
    pub pos: V2,
    pub prev: V2,
    pub vel: V2,
    pub kind: ZKind,
    pub hp: f32,
    pub max_hp: f32,
    pub speed: f32,
    pub r: f32,
    pub dmg: f32,
    pub xp: u32,
    pub flash: f32,
    pub anim: f32,
    pub attack_cd: f32,
    pub slow_t: f32,
    pub timer: f32,
    pub state: u8,
    pub charge_dir: V2,
    pub fuse: f32,
    pub blade_cd: f32,
    pub dash_mark: u32,
    pub dead: bool,
    pub uid: u32,
    pub face: f32,
    pub strafe: f32,
    pub tint: f32,
    pub boss_var: BossVar,
    pub special_t: f32,
}

pub struct Bullet {
    pub pos: V2,
    pub prev: V2,
    pub vel: V2,
    pub dmg: f32,
    pub life: f32,
    pub pierce: i32,
    pub bounces: i32,
    pub r: f32,
    pub crit: bool,
    pub last_hit: u32,
    pub dead: bool,
    pub blast: f32,
    pub knock: f32,
}

pub struct EShot {
    pub pos: V2,
    pub prev: V2,
    pub vel: V2,
    pub life: f32,
    pub dmg: f32,
    pub dead: bool,
}

pub struct Gem {
    pub pos: V2,
    pub prev: V2,
    pub vel: V2,
    pub value: u32,
    pub t: f32,
    pub magnet: bool,
    pub dead: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PickKind {
    Heart,
    Vacuum,
    Frenzy,
}

pub struct Pickup {
    pub pos: V2,
    pub kind: PickKind,
    pub t: f32,
    pub dead: bool,
}

pub struct Particle {
    pub pos: V2,
    pub vel: V2,
    pub z: f32,
    pub vz: f32,
    pub life: f32,
    pub max: f32,
    pub size: f32,
    pub color: &'static str,
    pub grav: bool,
    pub glow: bool,
    pub drag: f32,
}

pub struct Decal {
    pub pos: V2,
    pub r: f32,
    pub life: f32,
    pub color: &'static str,
    pub seed: u32,
}

pub struct FText {
    pub pos: V2,
    pub s: String,
    pub life: f32,
    pub max: f32,
    pub color: &'static str,
    pub size: f32,
    pub vy: f32,
}

pub struct Ring {
    pub pos: V2,
    pub r: f32,
    pub max_r: f32,
    pub life: f32,
    pub max: f32,
    pub color: &'static str,
    pub width: f32,
}

pub struct Bolt {
    pub pts: Vec<V2>,
    pub life: f32,
}

pub struct Ghost {
    pub pos: V2,
    pub life: f32,
}

pub struct Banner {
    pub title: String,
    pub sub: String,
    pub t: f32,
    pub dur: f32,
    pub color: &'static str,
}

pub struct Wave {
    pub n: u32,
    pub total: u32,
    pub spawned: u32,
    pub timer: f32,
    pub intermission: f32,
    pub boss_timer: f32,
    pub boss_pending: bool,
}

#[derive(Default)]
pub struct Input {
    pub up: bool,
    pub down: bool,
    pub left: bool,
    pub right: bool,
    pub mouse: V2,
    pub mouse_down: bool,
    pub mouse_seen: bool,
    pub pressed: Vec<String>,
    pub clicks: Vec<V2>,
    pub touch_move: V2,
    pub touch_active: bool,
    pub is_touch: bool,
    pub dash_req: bool,
    pub ult_req: bool,
    pub joy_id: Option<i32>,
    pub joy_origin: V2,
    pub joy_pos: V2,
}

/// Uniform spatial hash grid (counting sort) centred on the player; rebuilt each step.
pub struct Grid {
    cell: f32,
    n: i32,
    origin: V2,
    start: Vec<u32>,
    cursor: Vec<u32>,
    items: Vec<u32>,
}

impl Grid {
    fn new() -> Self {
        let n = 80;
        Grid {
            cell: 64.0,
            n,
            origin: V2::ZERO,
            start: vec![0; (n * n + 1) as usize],
            cursor: vec![0; (n * n) as usize],
            items: Vec::new(),
        }
    }
    fn cell_xy(&self, p: V2) -> (i32, i32) {
        let x = ((p.x - self.origin.x) / self.cell).floor() as i32;
        let y = ((p.y - self.origin.y) / self.cell).floor() as i32;
        (x.clamp(0, self.n - 1), y.clamp(0, self.n - 1))
    }
    fn build(&mut self, center: V2, zs: &[Zombie]) {
        let half = self.n as f32 * self.cell * 0.5;
        self.origin = center - v2(half, half);
        self.start.iter_mut().for_each(|s| *s = 0);
        let mut total = 0;
        for z in zs {
            if !z.dead {
                let (x, y) = self.cell_xy(z.pos);
                self.start[(y * self.n + x) as usize + 1] += 1;
                total += 1;
            }
        }
        for i in 1..self.start.len() {
            self.start[i] += self.start[i - 1];
        }
        let n = self.cursor.len();
        self.cursor.copy_from_slice(&self.start[..n]);
        self.items.resize(total, 0);
        for (i, z) in zs.iter().enumerate() {
            if !z.dead {
                let (x, y) = self.cell_xy(z.pos);
                let c = (y * self.n + x) as usize;
                self.items[self.cursor[c] as usize] = i as u32;
                self.cursor[c] += 1;
            }
        }
    }
    pub fn query(&self, p: V2, r: f32, mut f: impl FnMut(usize)) {
        let (x0, y0) = self.cell_xy(p - v2(r, r));
        let (x1, y1) = self.cell_xy(p + v2(r, r));
        for y in y0..=y1 {
            for x in x0..=x1 {
                let c = (y * self.n + x) as usize;
                for k in self.start[c]..self.start[c + 1] {
                    f(self.items[k as usize] as usize);
                }
            }
        }
    }
}

pub struct Game {
    pub phase: Phase,
    pub player: Player,
    pub zombies: Vec<Zombie>,
    pub bullets: Vec<Bullet>,
    pub eshots: Vec<EShot>,
    pub gems: Vec<Gem>,
    pub pickups: Vec<Pickup>,
    pub particles: Vec<Particle>,
    pub decals: Vec<Decal>,
    pub texts: Vec<FText>,
    pub rings: Vec<Ring>,
    pub bolts: Vec<Bolt>,
    pub ghosts: Vec<Ghost>,
    pub banner: Option<Banner>,
    pub toast: Option<(String, f32)>,
    pub wave: Wave,
    pub ups: Levels,
    pub offers: Vec<Up>,
    pub sel: usize,
    pub levelup_t: f32,
    pub pending_levels: u32,
    pub rng: Rng,
    pub grid: Grid,
    pub audio: Audio,
    pub input: Input,
    pub pending_sfx: Vec<(Sfx, V2)>,
    pub ui_out: Vec<(&'static str, String)>,
    // time
    pub time: f32,
    pub real_time: f64,
    pub last_ms: f64,
    pub acc: f32,
    pub alpha: f32,
    pub hitstop: f32,
    pub dying_t: f32,
    pub menu_t: f32,
    // camera / screen
    pub shake: f32,
    pub screen: V2,
    pub dpr: f32,
    pub zoom: f32,
    pub cam: V2,
    pub spawn_r: f32,
    pub mouse_world: V2,
    // stats
    pub kills: u32,
    pub combo: u32,
    pub combo_t: f32,
    pub best_combo: u32,
    pub damage_dealt: f64,
    pub next_uid: u32,
    pub ult_uses: u32,
    pub ult_flash: f32,
    pub ult_ready_played: bool,
    pub player_name: String,
    pub best_wave: u32,
    pub auto_aim: bool,
    pub show_fps: bool,
    pub show_minimap: bool,
    pub fps: f32,
    pub sep: Vec<V2>,
    pub groan_t: f32,
    // mode / loadout / cosmetics
    pub campaign: bool,
    pub chapter: usize,
    pub diff: usize,
    pub gun: usize,
    pub perk: String,
    pub skin: usize,
    pub hat: usize,
    pub tracer: usize,
    pub coins: u32,
    pub victory_t: f32,
}

fn xp_for(level: u32) -> f32 {
    let l = (level - 1) as f32;
    6.0 + l * 5.0 + l.powf(1.55) * 1.4
}

fn hp_mult(n: u32) -> f32 {
    let w = (n.max(1) - 1) as f32;
    1.0 + 0.18 * w + 0.012 * w * w
}

impl Game {
    pub fn new(seed: u64) -> Self {
        let mut g = Game {
            phase: Phase::Menu,
            player: Game::fresh_player(),
            zombies: Vec::with_capacity(512),
            bullets: Vec::with_capacity(512),
            eshots: Vec::new(),
            gems: Vec::with_capacity(512),
            pickups: Vec::new(),
            particles: Vec::with_capacity(2048),
            decals: Vec::with_capacity(300),
            texts: Vec::new(),
            rings: Vec::new(),
            bolts: Vec::new(),
            ghosts: Vec::new(),
            banner: None,
            toast: None,
            wave: Wave {
                n: 0,
                total: 0,
                spawned: 0,
                timer: 0.0,
                intermission: 0.0,
                boss_timer: 0.0,
                boss_pending: false,
            },
            ups: Levels::default(),
            offers: Vec::new(),
            sel: 0,
            levelup_t: 0.0,
            pending_levels: 0,
            rng: Rng::new(seed),
            grid: Grid::new(),
            audio: Audio::new(),
            input: Input::default(),
            pending_sfx: Vec::new(),
            ui_out: Vec::new(),
            time: 0.0,
            real_time: 0.0,
            last_ms: -1.0,
            acc: 0.0,
            alpha: 0.0,
            hitstop: 0.0,
            dying_t: 0.0,
            menu_t: 0.0,
            shake: 0.0,
            screen: v2(1280.0, 720.0),
            dpr: 1.0,
            zoom: 1.0,
            cam: V2::ZERO,
            spawn_r: 900.0,
            mouse_world: V2::ZERO,
            kills: 0,
            combo: 0,
            combo_t: 0.0,
            best_combo: 0,
            damage_dealt: 0.0,
            next_uid: 1,
            ult_uses: 0,
            ult_flash: 0.0,
            ult_ready_played: false,
            player_name: "Survivor".into(),
            best_wave: 0,
            auto_aim: false,
            show_fps: false,
            show_minimap: true,
            fps: 60.0,
            sep: Vec::new(),
            groan_t: 3.0,
            campaign: false,
            chapter: 0,
            diff: 1,
            gun: 0,
            perk: String::new(),
            skin: 0,
            hat: 0,
            tracer: 0,
            coins: 0,
            victory_t: -1.0,
        };
        g.player.pos = v2(0.0, 0.0);
        g
    }

    fn fresh_player() -> Player {
        Player {
            pos: V2::ZERO,
            prev: V2::ZERO,
            vel: V2::ZERO,
            aim: 0.0,
            hp: 100.0,
            hp_ghost: 100.0,
            max_hp: 100.0,
            iframes: 0.0,
            dash_t: 0.0,
            dash_cd: 0.0,
            dash_dir: v2(1.0, 0.0),
            dash_buf: 0.0,
            dash_id: 0,
            fire_cd: 0.3,
            recoil: 0.0,
            walk: 0.0,
            hurt_flash: 0.0,
            level: 1,
            xp: 0.0,
            xp_next: xp_for(1),
            ult: 0.0,
            frenzy: 0.0,
            blade_angle: 0.0,
            nova_t: 2.0,
            bolt_t: 1.0,
            ghost_t: 0.0,
        }
    }

    /// Apply `key=value;key=value` options: loadout, cosmetics, mode, difficulty.
    pub fn apply_opts(&mut self, opts: &str) {
        for kv in opts.split(';') {
            let Some((k, v)) = kv.split_once('=') else {
                continue;
            };
            match k {
                "name" => {
                    let n: String = v.trim().chars().take(16).collect();
                    self.player_name = if n.is_empty() { "Survivor".into() } else { n };
                }
                "best" => self.best_wave = v.parse().unwrap_or(0),
                "mode" => self.campaign = v == "campaign",
                "chapter" => self.chapter = v.parse::<usize>().unwrap_or(0).min(CHAPTERS.len() - 1),
                "diff" => self.diff = content::difficulty_index(v),
                "gun" => self.gun = content::gun_index(v),
                "perk" => self.perk = v.to_string(),
                "skin" => self.skin = content::skin_index(v),
                "hat" => self.hat = content::hat_index(v),
                "tracer" => self.tracer = content::tracer_index(v),
                _ => {}
            }
        }
    }

    pub fn difficulty(&self) -> &'static content::Difficulty {
        &DIFFICULTIES[self.diff]
    }

    /// Wave number used for scaling: campaign chapters start partway up the curve.
    pub fn scale_n(&self) -> u32 {
        let n = self.wave.n.max(1);
        if self.campaign {
            CHAPTERS[self.chapter].base + n - 1
        } else {
            n
        }
    }

    pub fn start(&mut self, opts: &str) {
        self.audio.unlock();
        self.audio.music_on = true;
        self.best_wave = 0;
        self.apply_opts(opts);
        self.player = Game::fresh_player();
        self.zombies.clear();
        self.bullets.clear();
        self.eshots.clear();
        self.gems.clear();
        self.pickups.clear();
        self.particles.clear();
        self.decals.clear();
        self.texts.clear();
        self.rings.clear();
        self.bolts.clear();
        self.ghosts.clear();
        self.ups = Levels::default();
        self.offers.clear();
        self.pending_levels = 0;
        self.kills = 0;
        self.combo = 0;
        self.best_combo = 0;
        self.damage_dealt = 0.0;
        self.time = 0.0;
        self.acc = 0.0;
        self.hitstop = 0.0;
        self.dying_t = 0.0;
        self.shake = 0.0;
        self.ult_uses = 0;
        self.ult_ready_played = false;
        self.coins = 0;
        self.victory_t = -1.0;
        self.wave = Wave {
            n: 0,
            total: 0,
            spawned: 0,
            timer: 0.0,
            intermission: 1.2,
            boss_timer: 0.0,
            boss_pending: false,
        };
        // starting perk from the loadout
        if let Some(u) = content::perk_upgrade(&self.perk) {
            self.ups.inc(u);
            if u == Up::Vitality {
                self.player.max_hp += 25.0;
                self.player.hp = self.player.max_hp;
                self.player.hp_ghost = self.player.hp;
            }
        }
        self.banner = Some(if self.campaign {
            Banner {
                title: format!("CHAPTER {}", self.chapter + 1),
                sub: format!(
                    "{} · {}",
                    CHAPTERS[self.chapter].title,
                    self.difficulty().name
                ),
                t: 0.0,
                dur: 3.0,
                color: "#ffd36a",
            }
        } else {
            Banner {
                title: "SURVIVE".into(),
                sub: format!(
                    "Infinite · {} · {}",
                    self.difficulty().name,
                    GUNS[self.gun].name
                ),
                t: 0.0,
                dur: 2.6,
                color: "#e8f0ff",
            }
        });
        self.phase = Phase::Playing;
        self.audio.play(Sfx::UiSelect);
    }

    // ---------------------------------------------------------------- stats
    pub fn dmg_mult(&self) -> f32 {
        (1.0 + 0.25 * self.ups.f(Up::Damage)) * (1.0 + 0.08 * self.ups.f(Up::Velocity))
    }
    fn fire_interval(&self) -> f32 {
        let base = 0.30 * GUNS[self.gun].interval / (1.0 + 0.16 * self.ups.f(Up::FireRate));
        if self.player.frenzy > 0.0 {
            base * 0.5
        } else {
            base
        }
    }
    fn move_speed(&self) -> f32 {
        255.0 * (1.0 + 0.09 * self.ups.f(Up::Speed)) * GUNS[self.gun].move_mult
    }
    pub fn magnet_r(&self) -> f32 {
        95.0 * (1.0 + 0.45 * self.ups.f(Up::Magnet))
    }
    fn crit_chance(&self) -> f32 {
        0.05 + 0.10 * self.ups.f(Up::Crit)
    }
    pub fn dash_cooldown(&self) -> f32 {
        1.1 * 0.8f32.powi(self.ups.get(Up::DashMaster) as i32)
    }
    pub fn blade_count(&self) -> u32 {
        let l = self.ups.get(Up::Blades) as u32;
        if l == 0 {
            0
        } else {
            l + 1
        }
    }
    pub fn blade_radius(&self) -> f32 {
        80.0 + 6.0 * self.ups.f(Up::Blades)
    }
    pub fn nova_radius(&self) -> f32 {
        150.0 + 25.0 * self.ups.f(Up::Nova)
    }
    pub fn alive(&self) -> u32 {
        self.zombies.iter().filter(|z| !z.dead).count() as u32
    }
    pub fn remaining(&self) -> u32 {
        self.wave.total.saturating_sub(self.wave.spawned)
            + self.alive()
            + if self.wave.boss_pending { 1 } else { 0 }
    }

    // ---------------------------------------------------------------- helpers
    fn sfx(&mut self, s: Sfx, at: V2) {
        if self.pending_sfx.len() < 48 {
            self.pending_sfx.push((s, at));
        }
    }

    pub fn flush_sfx(&mut self) {
        self.audio.begin_frame();
        let half_w = (self.screen.x / self.zoom.max(0.01)) * 0.5;
        let p = self.player.pos;
        for (s, at) in std::mem::take(&mut self.pending_sfx) {
            let d = at - p;
            let pan = (d.x / half_w.max(1.0)) * 0.7;
            let dist = d.len();
            let vol = if dist < 500.0 {
                1.0
            } else {
                (1.0 - (dist - 500.0) / 900.0).max(0.0)
            };
            self.audio.play_at(s, pan, vol);
        }
    }

    fn add_shake(&mut self, amt: f32) {
        self.shake = (self.shake + amt).min(1.0);
    }

    fn text(&mut self, pos: V2, s: String, color: &'static str, size: f32) {
        if self.texts.len() > 90 {
            self.texts.remove(0);
        }
        self.texts.push(FText {
            pos: pos + v2(self.rng.range(-8.0, 8.0), -10.0),
            s,
            life: 0.75,
            max: 0.75,
            color,
            size,
            vy: -70.0,
        });
    }

    #[allow(clippy::too_many_arguments)]
    fn burst(
        &mut self,
        pos: V2,
        n: usize,
        speed: f32,
        color: &'static str,
        size: f32,
        life: f32,
        grav: bool,
        glow: bool,
    ) {
        for _ in 0..n {
            if self.particles.len() >= 1800 {
                return;
            }
            let dir = self.rng.unit();
            let sp = speed * self.rng.range(0.3, 1.0);
            let l = life * self.rng.range(0.6, 1.2);
            self.particles.push(Particle {
                pos,
                vel: dir * sp,
                z: if grav { self.rng.range(4.0, 18.0) } else { 0.0 },
                vz: if grav {
                    self.rng.range(80.0, 340.0)
                } else {
                    0.0
                },
                life: l,
                max: l,
                size: size * self.rng.range(0.6, 1.3),
                color,
                grav,
                glow,
                drag: if grav { 1.5 } else { 4.0 },
            });
        }
    }

    fn ring(&mut self, pos: V2, max_r: f32, life: f32, color: &'static str, width: f32) {
        self.rings.push(Ring {
            pos,
            r: 4.0,
            max_r,
            life,
            max: life,
            color,
            width,
        });
    }

    fn nearest_zombie(&self, p: V2, max_d: f32, exclude: u32) -> Option<usize> {
        let mut best = None;
        let mut bd = max_d * max_d;
        self.grid.query(p, max_d, |j| {
            let z = &self.zombies[j];
            if !z.dead && z.uid != exclude {
                let d = z.pos.dist2(p);
                if d < bd {
                    bd = d;
                    best = Some(j);
                }
            }
        });
        best
    }

    // ---------------------------------------------------------------- frame
    /// Called once per animation frame with the rAF timestamp.
    pub fn frame(&mut self, now_ms: f64) {
        let real_dt = if self.last_ms < 0.0 {
            1.0 / 60.0
        } else {
            ((now_ms - self.last_ms) / 1000.0).clamp(0.0, 0.1) as f32
        };
        self.last_ms = now_ms;
        self.real_time += real_dt as f64;
        if real_dt > 0.0 {
            self.fps += (1.0 / real_dt - self.fps) * 0.05;
        }

        self.handle_input(real_dt);

        match self.phase {
            Phase::Menu => {
                self.menu_t += real_dt;
                let t = self.menu_t;
                self.player.prev = self.player.pos;
                self.player.pos = v2(t * 38.0, (t * 0.15).sin() * 260.0);
                if real_dt > 0.0 {
                    self.player.vel = (self.player.pos - self.player.prev) * (1.0 / real_dt);
                }
                self.player.walk += self.player.vel.len() * real_dt * 0.06;
                self.player.aim = if self.input.mouse_seen {
                    (self.input.mouse - self.screen * 0.5).angle()
                } else {
                    t * 0.8
                };
                self.alpha = 1.0;
            }
            Phase::Playing | Phase::Dying => {
                let scale = if self.phase == Phase::Dying { 0.3 } else { 1.0 };
                if self.hitstop > 0.0 {
                    self.hitstop -= real_dt;
                } else {
                    self.acc += real_dt * scale;
                    let mut steps = 0;
                    while self.acc >= DT && steps < 16 {
                        self.step();
                        self.acc -= DT;
                        steps += 1;
                    }
                    if steps == 16 {
                        self.acc = 0.0;
                    }
                }
                self.alpha = (self.acc / DT).clamp(0.0, 1.0);
                if self.victory_t >= 0.0 && self.phase == Phase::Playing {
                    self.victory_t += real_dt;
                    if self.victory_t > 3.2 {
                        self.phase = Phase::GameOver;
                        self.audio.music_on = false;
                        let json = self.stats_json();
                        self.ui_out.push(("victory", json));
                    }
                }
                if self.phase == Phase::Dying {
                    self.dying_t += real_dt;
                    if self.dying_t > 1.8 {
                        self.phase = Phase::GameOver;
                        self.audio.music_on = false;
                        let json = self.stats_json();
                        self.ui_out.push(("gameover", json));
                    }
                }
            }
            Phase::LevelUp => {
                self.levelup_t += real_dt;
            }
            Phase::Paused | Phase::GameOver => {}
        }

        if let Some(b) = &mut self.banner {
            if self.phase != Phase::Paused && self.phase != Phase::LevelUp {
                b.t += real_dt;
            }
            if b.t > b.dur {
                self.banner = None;
            }
        }
        if let Some(t) = &mut self.toast {
            t.1 -= real_dt;
            if t.1 <= 0.0 {
                self.toast = None;
            }
        }
        self.ult_flash = (self.ult_flash - real_dt * 2.5).max(0.0);

        // music intensity follows the action
        let alive = self.alive() as f32;
        let target = if matches!(self.phase, Phase::Playing | Phase::LevelUp) {
            ((alive / 120.0) + (self.wave.n as f32 / 30.0)).min(1.0)
        } else {
            0.0
        };
        self.audio.intensity += (target - self.audio.intensity) * damp(0.5, real_dt);
        self.flush_sfx();
        let _ = self.audio.update_music();
    }

    fn handle_input(&mut self, _dt: f32) {
        let pressed = std::mem::take(&mut self.input.pressed);
        let clicks = std::mem::take(&mut self.input.clicks);
        for code in pressed.iter() {
            let c = code.as_str();
            match self.phase {
                Phase::Playing => match c {
                    "Escape" | "KeyP" => self.pause(),
                    "Space" | "ShiftLeft" | "ShiftRight" => self.player.dash_buf = 0.18,
                    "KeyQ" | "KeyE" => self.try_ult(),
                    "KeyF" => {
                        self.auto_aim = !self.auto_aim;
                        let msg = if self.auto_aim {
                            "Auto-aim ON"
                        } else {
                            "Auto-aim OFF (mouse aim)"
                        };
                        self.toast = Some((msg.into(), 1.6));
                        self.audio.play(Sfx::UiMove);
                        self.ui_out.push((
                            "autoaim",
                            if self.auto_aim {
                                "1".into()
                            } else {
                                "0".into()
                            },
                        ));
                    }
                    "KeyM" => {
                        let v = if self.audio.music_volume > 0.0 {
                            0.0
                        } else {
                            0.5
                        };
                        self.audio.set_music_volume(v);
                        self.toast = Some((
                            if v > 0.0 {
                                "Music ON".into()
                            } else {
                                "Music OFF".into()
                            },
                            1.2,
                        ));
                    }
                    "Backquote" => self.show_fps = !self.show_fps,
                    "KeyN" => {
                        self.show_minimap = !self.show_minimap;
                        let msg = if self.show_minimap {
                            "Minimap ON"
                        } else {
                            "Minimap OFF"
                        };
                        self.toast = Some((msg.into(), 1.0));
                        self.audio.play(Sfx::UiMove);
                    }
                    _ => {}
                },
                Phase::Paused => {
                    if c == "Escape" || c == "KeyP" {
                        self.resume();
                    }
                }
                Phase::LevelUp => {
                    if self.levelup_t < 0.35 {
                        continue;
                    }
                    match c {
                        "Digit1" | "Numpad1" => self.choose(0),
                        "Digit2" | "Numpad2" => self.choose(1),
                        "Digit3" | "Numpad3" => self.choose(2),
                        "ArrowLeft" | "KeyA" => {
                            if self.sel > 0 {
                                self.sel -= 1;
                                self.audio.play(Sfx::UiMove);
                            }
                        }
                        "ArrowRight" | "KeyD" => {
                            if self.sel + 1 < self.offers.len() {
                                self.sel += 1;
                                self.audio.play(Sfx::UiMove);
                            }
                        }
                        "Enter" | "Space" => self.choose(self.sel),
                        _ => {}
                    }
                }
                _ => {}
            }
        }
        if self.input.dash_req {
            self.input.dash_req = false;
            if self.phase == Phase::Playing {
                self.player.dash_buf = 0.18;
            }
        }
        if self.input.ult_req {
            self.input.ult_req = false;
            if self.phase == Phase::Playing {
                self.try_ult();
            }
        }
        if self.phase == Phase::LevelUp {
            // hover selects, click picks
            let rects = self.card_rects();
            let m = self.input.mouse;
            for (i, r) in rects.iter().enumerate().take(self.offers.len()) {
                if self.input.mouse_seen
                    && m.x >= r.0
                    && m.x <= r.0 + r.2
                    && m.y >= r.1
                    && m.y <= r.1 + r.3
                    && self.sel != i
                {
                    self.sel = i;
                    self.audio.play(Sfx::UiMove);
                }
            }
            if self.levelup_t > 0.45 {
                for c in clicks {
                    for (i, r) in rects.iter().enumerate().take(self.offers.len()) {
                        if c.x >= r.0 && c.x <= r.0 + r.2 && c.y >= r.1 && c.y <= r.1 + r.3 {
                            self.choose(i);
                            return;
                        }
                    }
                }
            }
        }
    }

    pub fn pause(&mut self) {
        if self.phase == Phase::Playing {
            self.phase = Phase::Paused;
            let json = self.run_json();
            self.ui_out.push(("pause", json));
            self.audio.play(Sfx::UiMove);
        }
    }
    pub fn resume(&mut self) {
        if self.phase == Phase::Paused {
            self.phase = Phase::Playing;
            self.last_ms = -1.0;
            self.ui_out.push(("resume", String::new()));
            self.audio.play(Sfx::UiMove);
        }
    }
    pub fn to_menu(&mut self) {
        self.phase = Phase::Menu;
        self.player = Game::fresh_player();
        self.ups = Levels::default();
        self.victory_t = -1.0;
        self.texts.clear();
        self.particles.clear();
        self.rings.clear();
        self.audio.music_on = false;
        self.zombies.clear();
        self.bullets.clear();
        self.gems.clear();
        self.pickups.clear();
        self.eshots.clear();
        self.banner = None;
    }

    /// Layout of the three level-up cards in CSS pixels: (x, y, w, h).
    pub fn card_rects(&self) -> [(f32, f32, f32, f32); 3] {
        let w = (self.screen.x * 0.27).clamp(150.0, 260.0);
        let h = (w * 1.35).min(self.screen.y * 0.55);
        let gap = (self.screen.x * 0.025).clamp(10.0, 28.0);
        let total = w * 3.0 + gap * 2.0;
        let x0 = (self.screen.x - total) * 0.5;
        let y = self.screen.y * 0.5 - h * 0.42;
        [
            (x0, y, w, h),
            (x0 + w + gap, y, w, h),
            (x0 + 2.0 * (w + gap), y, w, h),
        ]
    }

    fn open_levelup(&mut self) {
        let luck = (self.wave.n as f32 * 0.01).min(0.25);
        self.offers = self.ups.roll(&mut self.rng, luck);
        if self.offers.is_empty() {
            // everything maxed: just heal
            self.player.hp = self.player.max_hp;
            self.pending_levels = 0;
            return;
        }
        self.sel = 0;
        self.levelup_t = 0.0;
        self.phase = Phase::LevelUp;
        self.audio.play(Sfx::LevelUp);
    }

    fn choose(&mut self, i: usize) {
        if i >= self.offers.len() || self.phase != Phase::LevelUp {
            return;
        }
        let u = self.offers[i];
        self.ups.inc(u);
        match u {
            Up::Vitality => {
                self.player.max_hp += 25.0;
                self.player.hp =
                    (self.player.hp + self.player.max_hp * 0.35).min(self.player.max_hp);
            }
            Up::Nova => self.player.nova_t = 0.4,
            Up::Lightning => self.player.bolt_t = 0.3,
            _ => {}
        }
        self.audio.play(Sfx::UiSelect);
        self.pending_levels = self.pending_levels.saturating_sub(1);
        if self.pending_levels > 0 {
            self.open_levelup();
            self.levelup_t = 0.2;
        } else {
            self.phase = Phase::Playing;
            self.last_ms = -1.0;
            // breathing room: shove nearby zombies away
            let p = self.player.pos;
            for z in self.zombies.iter_mut() {
                let d = z.pos - p;
                let l = d.len();
                if l < 240.0 && l > 0.1 {
                    let k = if z.kind == ZKind::Boss { 0.15 } else { 1.0 };
                    z.vel += d * (1.0 / l) * (520.0 * (1.0 - l / 240.0) + 120.0) * k;
                }
            }
            self.player.iframes = self.player.iframes.max(0.6);
            self.ring(p, 240.0, 0.45, "#bfe3ff", 6.0);
            self.burst(p, 26, 380.0, "#bfe3ff", 3.0, 0.5, false, true);
        }
    }

    fn try_ult(&mut self) {
        if self.player.ult < 1.0 {
            if self.player.ult < 0.999 {
                self.toast = Some((
                    format!("Ultimate charging… {}%", (self.player.ult * 100.0) as u32),
                    0.9,
                ));
            }
            return;
        }
        self.player.ult = 0.0;
        self.ult_uses += 1;
        self.ult_ready_played = false;
        let p = self.player.pos;
        let radius = 700.0;
        let dmg = (220.0 + 45.0 * self.wave.n as f32) * self.dmg_mult().sqrt();
        let mut hits = Vec::new();
        self.grid.query(p, radius, |j| hits.push(j));
        for j in hits {
            let z = &self.zombies[j];
            if z.dead {
                continue;
            }
            let d = z.pos - p;
            let l = d.len();
            if l < radius {
                let falloff = 1.0 - (l / radius) * 0.4;
                self.damage_zombie(j, dmg * falloff, d.norm(), 700.0, false);
            }
        }
        for s in self.eshots.iter_mut() {
            s.dead = true;
        }
        self.ring(p, radius, 0.7, "#ffe8a0", 18.0);
        self.ring(p, radius * 0.6, 0.5, "#ffffff", 10.0);
        self.burst(p, 90, 900.0, "#ffd36a", 4.0, 0.9, false, true);
        self.burst(p, 40, 500.0, "#ffffff", 3.0, 0.6, false, true);
        self.add_shake(0.9);
        self.hitstop = 0.09;
        self.ult_flash = 1.0;
        self.player.iframes = self.player.iframes.max(1.0);
        self.audio.play(Sfx::Ultimate);
    }

    pub fn stats_json(&self) -> String {
        let name = self.player_name.replace(['\\', '"'], "");
        let diff = ["rookie", "survivor", "veteran", "nightmare"][self.diff];
        format!(
            "{{\"name\":\"{}\",\"wave\":{},\"level\":{},\"kills\":{},\"time\":{:.1},\"combo\":{},\"coins\":{},\"mode\":\"{}\",\"chapter\":{},\"diff\":\"{}\",\"gun\":\"{}\",\"victory\":{}}}",
            name,
            self.wave.n.max(1),
            self.player.level,
            self.kills,
            self.time,
            self.best_combo,
            self.coins,
            if self.campaign { "campaign" } else { "infinite" },
            self.chapter,
            diff,
            GUNS[self.gun].id,
            self.victory_t >= 0.0
        )
    }

    /// Detailed snapshot for the pause screen's stats panel.
    pub fn run_json(&self) -> String {
        let ups: Vec<String> = crate::upgrades::ALL
            .iter()
            .filter(|u| self.ups.get(**u) > 0)
            .map(|u| {
                let d = u.def();
                format!(
                    "{{\"icon\":\"{}\",\"name\":\"{}\",\"lvl\":{},\"max\":{},\"color\":\"{}\"}}",
                    d.icon,
                    d.name,
                    self.ups.get(*u),
                    d.max,
                    d.color
                )
            })
            .collect();
        let shots_per_sec = 1.0 / self.fire_interval();
        format!(
            "{{\"run\":{},\"hp\":{:.0},\"maxHp\":{:.0},\"xp\":{:.0},\"xpNext\":{:.0},\"dmg\":{:.1},\"fireRate\":{:.2},\"speed\":{:.0},\"crit\":{:.0},\"magnet\":{:.0},\"ult\":{:.0},\"remaining\":{},\"gunName\":\"{}\",\"diffName\":\"{}\",\"upgrades\":[{}]}}",
            self.stats_json(),
            self.player.hp,
            self.player.max_hp,
            self.player.xp,
            self.player.xp_next,
            10.0 * GUNS[self.gun].dmg * self.dmg_mult(),
            shots_per_sec,
            self.move_speed(),
            self.crit_chance() * 100.0,
            self.magnet_r(),
            self.player.ult * 100.0,
            self.remaining(),
            GUNS[self.gun].name,
            self.difficulty().name,
            ups.join(",")
        )
    }

    /// Serialise the run so it can be resumed later (from the start of the current wave).
    pub fn save_string(&self) -> String {
        let wave = if self.wave.intermission > 0.0 {
            self.wave.n + 1
        } else {
            self.wave.n.max(1)
        };
        let ups: Vec<String> = self.ups.0.iter().map(|l| l.to_string()).collect();
        let diff = ["rookie", "survivor", "veteran", "nightmare"][self.diff];
        format!(
            "name={};mode={};chapter={};diff={};gun={};perk={};savewave={};level={};xp={:.1};hp={:.1};maxhp={:.1};ups={};kills={};coins={};time={:.1};ult={:.3};combo={};best={}",
            self.player_name.replace([';', '=', '|'], ""),
            if self.campaign { "campaign" } else { "infinite" },
            self.chapter,
            diff,
            GUNS[self.gun].id,
            self.perk,
            wave,
            self.player.level,
            self.player.xp,
            self.player.hp,
            self.player.max_hp,
            ups.join(","),
            self.kills,
            self.coins,
            self.time,
            self.player.ult,
            self.best_combo,
            self.best_wave
        )
    }

    pub fn load_save(&mut self, data: &str) {
        self.start(data);
        let mut wave = 1u32;
        for kv in data.split(';') {
            let Some((k, v)) = kv.split_once('=') else {
                continue;
            };
            match k {
                "savewave" => wave = v.parse().unwrap_or(1).max(1),
                "level" => self.player.level = v.parse().unwrap_or(1),
                "xp" => self.player.xp = v.parse().unwrap_or(0.0),
                "hp" => self.player.hp = v.parse().unwrap_or(100.0),
                "maxhp" => self.player.max_hp = v.parse().unwrap_or(100.0),
                "kills" => self.kills = v.parse().unwrap_or(0),
                "coins" => self.coins = v.parse().unwrap_or(0),
                "time" => self.time = v.parse().unwrap_or(0.0),
                "ult" => self.player.ult = v.parse().unwrap_or(0.0),
                "combo" => self.best_combo = v.parse().unwrap_or(0),
                "ups" => {
                    for (i, l) in v.split(',').enumerate().take(self.ups.0.len()) {
                        self.ups.0[i] = l.parse().unwrap_or(0);
                    }
                }
                _ => {}
            }
        }
        if self.campaign {
            wave = wave.min(CHAPTERS[self.chapter].waves);
        }
        self.player.xp_next = xp_for(self.player.level);
        self.player.hp = self.player.hp.clamp(1.0, self.player.max_hp);
        self.player.hp_ghost = self.player.hp;
        self.ult_ready_played = self.player.ult >= 1.0;
        self.wave.n = wave - 1;
        self.wave.intermission = 2.0;
        self.banner = Some(Banner {
            title: "RUN RESUMED".into(),
            sub: format!(
                "Wave {} · Level {} · {}",
                wave,
                self.player.level,
                self.difficulty().name
            ),
            t: 0.0,
            dur: 2.4,
            color: "#7ad7ff",
        });
    }

    // ---------------------------------------------------------------- simulation
    fn step(&mut self) {
        self.time += DT;
        let alive_play = self.phase == Phase::Playing;

        self.grid.build(self.player.pos, &self.zombies);
        if alive_play {
            self.update_player();
            self.update_weapons();
        }
        self.update_zombies();
        self.grid.build(self.player.pos, &self.zombies);
        self.separate();
        self.update_bullets();
        self.update_eshots();
        if alive_play {
            self.update_pickups();
        }
        self.update_effects();
        if alive_play {
            self.update_waves();
        }

        self.zombies.retain(|z| !z.dead);
        self.gems.retain(|g| !g.dead);
        self.pickups.retain(|p| !p.dead);

        if alive_play {
            while self.player.xp >= self.player.xp_next {
                self.player.xp -= self.player.xp_next;
                self.player.level += 1;
                self.player.xp_next = xp_for(self.player.level);
                self.pending_levels += 1;
            }
            if self.pending_levels > 0 && self.phase == Phase::Playing {
                self.open_levelup();
            }
        }
    }

    fn move_dir(&self) -> V2 {
        let i = &self.input;
        if i.touch_active {
            return i.touch_move.clamp_len(1.0);
        }
        let mut d = V2::ZERO;
        if i.up {
            d.y -= 1.0;
        }
        if i.down {
            d.y += 1.0;
        }
        if i.left {
            d.x -= 1.0;
        }
        if i.right {
            d.x += 1.0;
        }
        d.norm()
    }

    fn update_player(&mut self) {
        let mdir = self.move_dir();
        let speed = self.move_speed();
        let dash_cd = self.dash_cooldown();
        let p = &mut self.player;
        p.prev = p.pos;
        p.iframes -= DT;
        p.dash_cd -= DT;
        p.dash_buf -= DT;
        p.recoil = (p.recoil - DT * 9.0).max(0.0);
        p.hurt_flash = (p.hurt_flash - DT * 2.5).max(0.0);
        p.frenzy = (p.frenzy - DT).max(0.0);
        p.hp_ghost = if p.hp_ghost > p.hp {
            (p.hp_ghost - DT * 40.0).max(p.hp)
        } else {
            p.hp
        };
        let regen = 0.8 * self.ups.f(Up::Regen);
        p.hp = (p.hp + regen * DT).min(p.max_hp);

        let mut started_dash = false;
        if p.dash_buf > 0.0 && p.dash_cd <= 0.0 && p.dash_t <= 0.0 {
            p.dash_dir = if mdir.len2() > 0.01 {
                mdir
            } else {
                V2::from_angle(p.aim)
            };
            p.dash_t = 0.17;
            p.dash_cd = dash_cd;
            p.dash_buf = 0.0;
            p.iframes = p.iframes.max(0.24);
            p.dash_id += 1;
            started_dash = true;
        }
        if p.dash_t > 0.0 {
            p.dash_t -= DT;
            p.vel = p.dash_dir * speed * 3.5;
            p.ghost_t -= DT;
            if p.ghost_t <= 0.0 {
                p.ghost_t = 0.025;
                self.ghosts.push(Ghost {
                    pos: p.pos,
                    life: 0.25,
                });
            }
            if p.dash_t <= 0.0 {
                p.vel = p.dash_dir * speed;
            }
        } else {
            let target = mdir * speed;
            // fast accel, slightly slower decel => snappy yet weighty
            let rate = if mdir.len2() > 0.0 { 18.0 } else { 14.0 };
            p.vel = p.vel.lerp(target, damp(rate, DT));
        }
        p.pos += p.vel * DT;
        let (np, _) = world::resolve(p.pos, PR);
        p.pos = np;
        p.walk += p.vel.len() * DT * 0.06;
        let ppos = p.pos;
        if started_dash {
            self.sfx(Sfx::Dash, ppos);
            self.burst(ppos, 10, 160.0, "#cfd8ff", 3.0, 0.35, false, false);
        }

        // aim
        self.mouse_world =
            self.player.pos + (self.input.mouse - self.screen * 0.5) * (1.0 / self.zoom.max(0.01));
        let auto = self.auto_aim || self.input.is_touch || !self.input.mouse_seen;
        let target_angle = if auto {
            self.nearest_zombie(ppos, 650.0, 0).map(|j| {
                let z = &self.zombies[j];
                // lead the target a little
                let lead = z.pos + z.vel * (z.pos.dist(ppos) / 1100.0);
                (lead - ppos).angle()
            })
        } else {
            Some(self.assisted_aim(ppos, (self.mouse_world - ppos).angle()))
        };
        if let Some(a) = target_angle {
            let p = &mut self.player;
            if auto {
                let mut d = a - p.aim;
                while d > PI {
                    d -= TAU;
                }
                while d < -PI {
                    d += TAU;
                }
                p.aim += d * damp(30.0, DT);
            } else {
                p.aim = a;
            }
        }

        // dash damage (Shadow Step)
        let dm = self.ups.get(Up::DashMaster);
        if dm > 0 && self.player.dash_t > 0.0 {
            let id = self.player.dash_id;
            let dmg = (18.0 + 14.0 * dm as f32) * self.dmg_mult();
            let mut hits = Vec::new();
            self.grid.query(ppos, PR + 50.0, |j| hits.push(j));
            for j in hits {
                let z = &mut self.zombies[j];
                if !z.dead && z.dash_mark != id && z.pos.dist(ppos) < z.r + PR + 10.0 {
                    z.dash_mark = id;
                    let dir = (z.pos - ppos).norm();
                    self.damage_zombie(j, dmg, dir, 380.0, false);
                }
            }
        }
    }

    /// Slight aim magnetism for mouse aiming: if a zombie sits inside a narrow
    /// cone around the cursor direction, bend the shot partway toward it
    /// (leading its movement). Strongest when nearly on target, fading to zero
    /// at the cone's edge, so it never fights the player's intent.
    fn assisted_aim(&self, ppos: V2, raw: f32) -> f32 {
        const RANGE: f32 = 620.0;
        const BASE_CONE: f32 = 0.14; // ~8 degrees
        const STRENGTH: f32 = 0.5;
        let mut best: Option<(f32, f32, f32)> = None; // (score, angle delta, falloff)
        self.grid.query(ppos, RANGE, |j| {
            let z = &self.zombies[j];
            if z.dead {
                return;
            }
            let dist = z.pos.dist(ppos);
            if !(1.0..=RANGE).contains(&dist) {
                return;
            }
            let lead = z.pos + z.vel * (dist / 1100.0);
            let delta = wrap_angle((lead - ppos).angle() - raw);
            // cone widens by the target's angular size so big zombies are easier to snap
            let cone = BASE_CONE + (z.r / dist).min(0.2);
            if delta.abs() < cone {
                let score = delta.abs() / cone + dist / RANGE * 0.35;
                if best.map_or(true, |b| score < b.0) {
                    best = Some((score, delta, 1.0 - delta.abs() / cone));
                }
            }
        });
        match best {
            Some((_, delta, falloff)) => raw + delta * STRENGTH * falloff.sqrt(),
            None => raw,
        }
    }

    fn update_weapons(&mut self) {
        let ppos = self.player.pos;
        // ---- gun
        self.player.fire_cd -= DT;
        let enemies_near = self.nearest_zombie(ppos, self.spawn_r * 0.95, 0).is_some();
        let auto = self.auto_aim || self.input.is_touch || !self.input.mouse_seen;
        let want_fire = if auto {
            self.nearest_zombie(ppos, 650.0, 0).is_some()
        } else {
            enemies_near || self.input.mouse_down
        };
        if want_fire && self.player.fire_cd <= 0.0 {
            self.player.fire_cd += self.fire_interval();
            if self.player.fire_cd < 0.0 {
                self.player.fire_cd = 0.0;
            }
            self.fire();
        }
        if self.player.fire_cd < -0.2 {
            self.player.fire_cd = 0.0;
        }

        // ---- orbiting blades
        let nb = self.blade_count();
        if nb > 0 {
            self.player.blade_angle += DT * (3.0 + 0.15 * nb as f32);
            let br = self.blade_radius();
            let dmg = 13.0 * (1.0 + 0.2 * (self.ups.f(Up::Blades) - 1.0)) * self.dmg_mult() * 0.75;
            for k in 0..nb {
                let a = self.player.blade_angle + k as f32 * TAU / nb as f32;
                let bp = ppos + V2::from_angle(a) * br;
                let mut hits = Vec::new();
                self.grid.query(bp, 70.0, |j| hits.push(j));
                for j in hits {
                    let z = &mut self.zombies[j];
                    if !z.dead && z.blade_cd <= 0.0 && z.pos.dist(bp) < z.r + 16.0 {
                        z.blade_cd = 0.4;
                        let dir = (z.pos - ppos).norm();
                        self.damage_zombie(j, dmg, dir, 220.0, false);
                        self.sfx(Sfx::Blade, bp);
                    }
                }
            }
        }

        // ---- frost nova
        let nl = self.ups.get(Up::Nova);
        if nl > 0 {
            self.player.nova_t -= DT;
            if self.player.nova_t <= 0.0 {
                self.player.nova_t = (3.8 - 0.35 * nl as f32).max(1.6);
                let r = self.nova_radius();
                let dmg = (10.0 + 7.0 * nl as f32) * self.dmg_mult();
                let mut hits = Vec::new();
                self.grid.query(ppos, r, |j| hits.push(j));
                for j in hits {
                    let z = &mut self.zombies[j];
                    if !z.dead && z.pos.dist(ppos) < r + z.r {
                        z.slow_t = 2.2;
                        let dir = (z.pos - ppos).norm();
                        self.damage_zombie(j, dmg, dir, 260.0, false);
                    }
                }
                self.ring(ppos, r, 0.5, "#a8f4ff", 10.0);
                self.burst(ppos, 30, r * 2.2, "#d8fbff", 3.0, 0.45, false, true);
                self.sfx(Sfx::Nova, ppos);
            }
        }

        // ---- chain lightning
        let ll = self.ups.get(Up::Lightning);
        if ll > 0 {
            self.player.bolt_t -= DT;
            if self.player.bolt_t <= 0.0 {
                self.player.bolt_t = (2.3 - 0.22 * ll as f32).max(0.8);
                let chains = 2 + ll as usize;
                let dmg = (16.0 + 9.0 * ll as f32) * self.dmg_mult();
                // first target: random-ish zombie near the player
                let mut cands = Vec::new();
                self.grid.query(ppos, 480.0, |j| {
                    if !self.zombies[j].dead && self.zombies[j].pos.dist(ppos) < 480.0 {
                        cands.push(j)
                    }
                });
                if !cands.is_empty() {
                    let mut cur = cands[self.rng.below(cands.len())];
                    let mut pts = vec![ppos + v2(0.0, -20.0)];
                    let mut visited: Vec<u32> = Vec::new();
                    for _ in 0..chains {
                        let zp = self.zombies[cur].pos;
                        let uid = self.zombies[cur].uid;
                        visited.push(uid);
                        pts.push(zp);
                        let dir = (zp - *pts.first().unwrap()).norm();
                        self.damage_zombie(cur, dmg, dir, 120.0, false);
                        self.burst(zp, 6, 220.0, "#cfe0ff", 2.5, 0.25, false, true);
                        let mut next = None;
                        let mut bd = 240.0f32 * 240.0;
                        self.grid.query(zp, 240.0, |j| {
                            let z = &self.zombies[j];
                            if !z.dead && !visited.contains(&z.uid) {
                                let d = z.pos.dist2(zp);
                                if d < bd {
                                    bd = d;
                                    next = Some(j);
                                }
                            }
                        });
                        match next {
                            Some(n) => cur = n,
                            None => break,
                        }
                    }
                    self.bolts.push(Bolt { pts, life: 0.22 });
                    self.sfx(Sfx::Lightning, ppos);
                }
            }
        }
    }

    fn fire(&mut self) {
        let gun = GUNS[self.gun];
        let ppos = self.player.pos;
        let aim = self.player.aim;
        let extra = self.ups.get(Up::Multishot) as i32;
        let speed = 920.0 * gun.speed * (1.0 + 0.2 * self.ups.f(Up::Velocity));
        let life = 0.62 * gun.life * (1.0 + 0.15 * self.ups.f(Up::Velocity));
        let dmg = 10.0 * gun.dmg * self.dmg_mult();
        let pierce = self.ups.get(Up::Pierce) as i32 + gun.pierce;
        let bounces = self.ups.get(Up::Ricochet) as i32;
        let crit_c = self.crit_chance();
        let shoot = |g: &mut Game, a: f32| {
            let a = a + if gun.jitter > 0.0 {
                g.rng.range(-gun.jitter, gun.jitter)
            } else {
                0.0
            };
            let dir = V2::from_angle(a);
            let crit = g.rng.chance(crit_c);
            let start = ppos + dir * gun.len;
            g.bullets.push(Bullet {
                pos: start,
                prev: start - dir * 6.0,
                vel: dir * speed * g.rng.range(0.97, 1.03),
                dmg: if crit { dmg * 2.5 } else { dmg },
                life: life
                    * if gun.pellets > 1 {
                        g.rng.range(0.85, 1.1)
                    } else {
                        1.0
                    },
                pierce,
                bounces,
                r: gun.bullet_r * if crit { 1.35 } else { 1.0 },
                crit,
                last_hit: 0,
                dead: false,
                blast: gun.blast,
                knock: gun.knock,
            });
        };
        let n = gun.pellets as i32 + extra;
        // multi-pellet guns fan across their spread; others add 0.1 rad per extra shot
        let fan = if gun.pellets > 1 {
            gun.spread + 0.06 * extra as f32
        } else {
            0.10 * (n - 1) as f32
        };
        for k in 0..n {
            let f = if n > 1 {
                k as f32 / (n - 1) as f32 - 0.5
            } else {
                0.0
            };
            shoot(self, aim + f * fan);
        }
        let back = self.ups.get(Up::Backshot) as i32;
        for k in 0..back {
            let a = aim + PI + (k as f32 - (back - 1) as f32 * 0.5) * 0.22;
            shoot(self, a);
        }
        self.player.recoil = 1.0;
        let muzzle = ppos + V2::from_angle(aim) * (gun.len + 4.0);
        let flash_n = if gun.heavy { 7 } else { 3 };
        for _ in 0..flash_n {
            if self.particles.len() < 1800 {
                let a = aim + self.rng.range(-0.4, 0.4);
                self.particles.push(Particle {
                    pos: muzzle,
                    vel: V2::from_angle(a) * self.rng.range(120.0, 380.0) + self.player.vel,
                    z: 0.0,
                    vz: 0.0,
                    life: 0.08,
                    max: 0.08,
                    size: 3.0,
                    color: "#ffe9a8",
                    grav: false,
                    glow: true,
                    drag: 6.0,
                });
            }
        }
        if gun.heavy {
            self.player.vel -= V2::from_angle(aim) * 60.0;
            self.add_shake(0.06);
            self.sfx(Sfx::ShootHeavy, ppos);
        } else {
            self.sfx(Sfx::Shoot, ppos);
        }
    }

    pub fn damage_zombie(&mut self, j: usize, dmg: f32, dir: V2, knock: f32, crit: bool) -> bool {
        let z = &mut self.zombies[j];
        if z.dead {
            return false;
        }
        z.hp -= dmg;
        z.flash = 1.0;
        let kres = match z.kind {
            ZKind::Brute => 0.35,
            ZKind::Boss => 0.06,
            _ => 1.0,
        };
        z.vel += dir * knock * kres;
        let pos = z.pos;
        let dead = z.hp <= 0.0;
        self.damage_dealt += dmg as f64;
        let (col, size) = if crit {
            ("#ffcf3a", 20.0)
        } else {
            ("#ffffff", 14.0)
        };
        let s = format!("{}", dmg.round() as i32);
        self.text(pos, s, col, size);
        // blood spray along the hit direction
        for _ in 0..3 {
            if self.particles.len() < 1800 {
                let a = dir.angle() + self.rng.range(-0.7, 0.7);
                let l = self.rng.range(0.25, 0.5);
                self.particles.push(Particle {
                    pos,
                    vel: V2::from_angle(a) * self.rng.range(80.0, 260.0),
                    z: 12.0,
                    vz: self.rng.range(40.0, 180.0),
                    life: l,
                    max: l,
                    size: self.rng.range(1.5, 3.5),
                    color: "#7a0f14",
                    grav: true,
                    glow: false,
                    drag: 2.0,
                });
            }
        }
        if crit {
            self.sfx(Sfx::Crit, pos);
        } else {
            self.sfx(Sfx::Hit, pos);
        }
        if dead {
            self.kill_zombie(j);
        }
        dead
    }

    fn kill_zombie(&mut self, j: usize) {
        let (pos, kind, xp, r, fuse_done) = {
            let z = &mut self.zombies[j];
            z.dead = true;
            (z.pos, z.kind, z.xp, z.r, z.fuse < -0.5)
        };
        self.kills += 1;
        self.combo += 1;
        self.combo_t = 2.5;
        self.best_combo = self.best_combo.max(self.combo);
        let was = self.player.ult;
        self.player.ult = (self.player.ult + 1.0 / (70.0 + 15.0 * self.ult_uses as f32)).min(1.0);
        if was < 1.0 && self.player.ult >= 1.0 && !self.ult_ready_played {
            self.ult_ready_played = true;
            self.sfx(Sfx::UltReady, self.player.pos);
            self.toast = Some(("ULTIMATE READY — press Q".into(), 2.2));
        }

        // gore
        let big = matches!(kind, ZKind::Brute | ZKind::Boss);
        let skin = match kind {
            ZKind::Walker => "#6f8f4e",
            ZKind::Runner => "#9aa25a",
            ZKind::Brute => "#4f6b3a",
            ZKind::Spitter => "#7fb04a",
            ZKind::Exploder => "#c9772f",
            ZKind::Boss => "#5a3b5e",
        };
        let n = (6.0 + r * 0.5) as usize;
        self.burst(pos, n, 260.0 + r * 4.0, "#8a1018", 3.0, 0.7, true, false);
        self.burst(pos, n / 2, 220.0, skin, 4.0, 0.8, true, false);
        if self.decals.len() >= 260 {
            self.decals.remove(0);
        }
        let seed = self.rng.next_u32();
        self.decals.push(Decal {
            pos,
            r: r * self.rng.range(1.1, 1.7),
            life: 22.0,
            color: if kind == ZKind::Spitter {
                "#3e6b1f"
            } else {
                "#4a0a0e"
            },
            seed,
        });

        // xp
        let bonus = self.wave.n / 4;
        let mut drops = if big { 4 + (r / 10.0) as u32 } else { 1 };
        if kind == ZKind::Boss {
            drops = 14;
        }
        let per = ((xp + bonus) as f32 / drops as f32).ceil() as u32;
        for _ in 0..drops {
            self.spawn_gem(pos, per.max(1));
        }
        // pickups
        let roll = self.rng.f();
        let pk = if kind == ZKind::Boss {
            Some(PickKind::Heart)
        } else if roll < 0.010 {
            Some(PickKind::Heart)
        } else if roll < 0.014 {
            Some(PickKind::Vacuum)
        } else if roll < 0.018 {
            Some(PickKind::Frenzy)
        } else {
            None
        };
        if let Some(k) = pk {
            self.pickups.push(Pickup {
                pos: pos + self.rng.unit() * 10.0,
                kind: k,
                t: 0.0,
                dead: false,
            });
        }
        if kind == ZKind::Boss {
            self.pickups.push(Pickup {
                pos: pos + v2(30.0, 0.0),
                kind: PickKind::Vacuum,
                t: 0.0,
                dead: false,
            });
        }

        // vampirism
        let vl = self.ups.get(Up::Vampire);
        if vl > 0 && self.rng.chance(0.04 * vl as f32) {
            self.player.hp = (self.player.hp + 3.0).min(self.player.max_hp);
            let pp = self.player.pos;
            self.text(pp, "+3".into(), "#ff4d6d", 14.0);
        }

        match kind {
            ZKind::Boss => {
                self.sfx(Sfx::BigDie, pos);
                self.sfx(Sfx::Explosion, pos);
                self.add_shake(0.8);
                self.hitstop = 0.12;
                self.ring(pos, 260.0, 0.6, "#ffb0ff", 12.0);
                self.burst(pos, 60, 700.0, "#ff6aff", 4.0, 0.8, false, true);
                let bonus = (50.0 * self.difficulty().coins) as u32;
                self.coins += bonus;
                let name = self.zombies[j].boss_var.name();
                self.banner = Some(Banner {
                    title: "BOSS SLAIN".into(),
                    sub: format!("{} defeated · +{} coins", name, bonus),
                    t: 0.0,
                    dur: 2.2,
                    color: "#ff9cff",
                });
                let bosses_left = self.wave.boss_pending
                    || self
                        .zombies
                        .iter()
                        .any(|z| !z.dead && z.kind == ZKind::Boss);
                if self.campaign && self.wave.n >= CHAPTERS[self.chapter].waves && !bosses_left {
                    self.campaign_victory();
                }
            }
            ZKind::Brute => {
                self.sfx(Sfx::BigDie, pos);
                self.add_shake(0.25);
                self.hitstop = self.hitstop.max(0.035);
            }
            ZKind::Exploder => {
                self.explode(pos, 115.0, 45.0 * hp_mult(self.wave.n).sqrt(), fuse_done);
            }
            _ => {
                self.sfx(Sfx::ZombieDie, pos);
            }
        }
    }

    fn spawn_gem(&mut self, pos: V2, value: u32) {
        if self.gems.len() > 420 {
            // merge into a random existing gem to bound the count
            let i = self.rng.below(self.gems.len());
            self.gems[i].value += value;
            return;
        }
        let v = self.rng.unit() * self.rng.range(40.0, 160.0);
        self.gems.push(Gem {
            pos,
            prev: pos,
            vel: v,
            value,
            t: 0.0,
            magnet: false,
            dead: false,
        });
    }

    /// `hurts_player`: true for exploder fuses that went off; player-caused kills are safe.
    fn explode(&mut self, pos: V2, radius: f32, dmg: f32, hurts_player: bool) {
        let mut hits = Vec::new();
        self.grid.query(pos, radius + 50.0, |j| hits.push(j));
        for j in hits {
            let z = &self.zombies[j];
            if !z.dead && z.pos.dist(pos) < radius + z.r {
                let dir = (z.pos - pos).norm();
                self.damage_zombie(j, dmg * 1.5, dir, 420.0, false);
            }
        }
        if hurts_player && self.player.pos.dist(pos) < radius + PR {
            self.hurt_player(dmg * 0.6, pos);
        }
        self.ring(pos, radius, 0.35, "#ffb24a", 8.0);
        self.burst(pos, 26, 520.0, "#ffb24a", 4.0, 0.45, false, true);
        self.burst(pos, 14, 300.0, "#5a5a5a", 6.0, 0.9, true, false);
        self.sfx(Sfx::Explosion, pos);
        let d = self.player.pos.dist(pos);
        self.add_shake(if d < 400.0 { 0.35 } else { 0.1 });
    }

    fn hurt_player(&mut self, dmg: f32, from: V2) {
        if self.phase != Phase::Playing {
            return;
        }
        if self.victory_t >= 0.0 {
            return;
        }
        let p = &mut self.player;
        if p.iframes > 0.0 || p.dash_t > 0.0 {
            return;
        }
        p.hp -= dmg;
        p.iframes = 0.65;
        p.hurt_flash = 1.0;
        let away = (p.pos - from).norm();
        p.vel += away * 340.0;
        let pos = p.pos;
        self.add_shake(0.45);
        self.hitstop = self.hitstop.max(0.05);
        self.burst(pos, 14, 260.0, "#c0101c", 3.0, 0.6, true, false);
        self.sfx(Sfx::Hurt, pos);
        self.combo = 0;
        if self.player.hp <= 0.0 {
            self.player.hp = 0.0;
            self.phase = Phase::Dying;
            self.dying_t = 0.0;
            self.audio.play(Sfx::GameOver);
            self.burst(pos, 70, 520.0, "#b0101a", 4.0, 1.4, true, false);
            self.add_shake(1.0);
            self.hitstop = 0.15;
        }
    }

    fn update_zombies(&mut self) {
        let ppos = self.player.pos;
        let wave_n = self.scale_n();
        let mut contacts: Vec<usize> = Vec::new();
        let mut fuses: Vec<usize> = Vec::new();
        let mut boss_summons: Vec<(V2, ZKind, u32)> = Vec::new();
        let mut blasts: Vec<V2> = Vec::new();
        let relocate_r = self.spawn_r * 1.7;
        let pvel = self.player.vel;
        let playing = self.phase == Phase::Playing;
        for (i, z) in self.zombies.iter_mut().enumerate() {
            if z.dead {
                continue;
            }
            z.prev = z.pos;
            z.flash = (z.flash - DT * 7.0).max(0.0);
            z.attack_cd -= DT;
            z.slow_t -= DT;
            z.blade_cd -= DT;
            let to = ppos - z.pos;
            let dist = to.len();
            let dir = to.norm();

            // stragglers that fall too far behind get recycled ahead of the player
            if dist > relocate_r && z.kind != ZKind::Boss {
                let a = if pvel.len2() > 400.0 {
                    pvel.angle() + self.rng.range(-0.9, 0.9)
                } else {
                    self.rng.angle()
                };
                z.pos = ppos + V2::from_angle(a) * (self.spawn_r + 40.0);
                z.prev = z.pos;
                z.vel = V2::ZERO;
                continue;
            }

            let mut spd = z.speed * if z.slow_t > 0.0 { 0.45 } else { 1.0 };
            let mut want = if playing {
                dir
            } else {
                V2::from_angle(z.face + 0.5)
            };
            let mut accel = 7.0;

            match z.kind {
                ZKind::Spitter => {
                    if dist < 250.0 {
                        want = -dir;
                    } else if dist < 400.0 {
                        want = (dir.perp() * z.strafe + dir * 0.15).norm();
                        spd *= 0.6;
                    }
                    z.timer -= DT;
                    if z.timer <= 0.0 && dist < 560.0 && playing {
                        z.timer = 2.4 + self.rng.range(-0.3, 0.3);
                        let lead = ppos + pvel * (dist / 340.0) * 0.6;
                        let sdir = (lead - z.pos).norm();
                        self.eshots.push(EShot {
                            pos: z.pos + sdir * z.r,
                            prev: z.pos,
                            vel: sdir * 340.0,
                            life: 2.2,
                            dmg: z.dmg,
                            dead: false,
                        });
                        self.pending_sfx.push((Sfx::Spit, z.pos));
                    }
                }
                ZKind::Exploder => {
                    if z.fuse > 0.0 {
                        z.fuse -= DT;
                        spd *= 0.2;
                        if z.fuse <= 0.0 {
                            z.fuse = -1.0;
                            fuses.push(i);
                        }
                    } else if dist < z.r + PR + 34.0 && playing {
                        z.fuse = 0.55;
                    }
                }
                ZKind::Boss => {
                    z.timer -= DT;
                    z.special_t -= DT;
                    let enraged = z.hp < z.max_hp * 0.5;
                    let var = z.boss_var;
                    // signature special attacks
                    if playing && z.special_t <= 0.0 {
                        let shot_dmg = z.dmg * 0.55;
                        let mut ring =
                            |count: usize, offset: f32, spread: Option<f32>, speed: f32| {
                                for k in 0..count {
                                    let a = match spread {
                                        Some(sp) => {
                                            dir.angle()
                                                + (k as f32 / (count - 1).max(1) as f32 - 0.5) * sp
                                        }
                                        None => offset + k as f32 * TAU / count as f32,
                                    };
                                    let sd = V2::from_angle(a);
                                    self.eshots.push(EShot {
                                        pos: z.pos + sd * z.r,
                                        prev: z.pos,
                                        vel: sd * speed,
                                        life: 2.6,
                                        dmg: shot_dmg,
                                        dead: false,
                                    });
                                }
                            };
                        match var {
                            BossVar::Abomination => z.special_t = 99.0,
                            BossVar::Gravedigger => {
                                z.special_t = if enraged { 3.5 } else { 5.0 };
                                boss_summons.push((
                                    z.pos,
                                    ZKind::Walker,
                                    if enraged { 7 } else { 5 },
                                ));
                                self.pending_sfx.push((Sfx::Groan, z.pos));
                            }
                            BossVar::Matriarch => {
                                z.special_t = if enraged { 1.5 } else { 2.3 };
                                ring(if enraged { 7 } else { 5 }, 0.0, Some(0.7), 320.0);
                                self.pending_sfx.push((Sfx::Spit, z.pos));
                                if self.rng.chance(0.3) {
                                    boss_summons.push((z.pos, ZKind::Spitter, 2));
                                }
                            }
                            BossVar::CinderKing => {
                                z.special_t = 5.5;
                                boss_summons.push((
                                    z.pos,
                                    ZKind::Exploder,
                                    if enraged { 4 } else { 3 },
                                ));
                            }
                            BossVar::Wraith => {
                                z.special_t = if enraged { 2.4 } else { 3.4 };
                                ring(8, self.rng.angle(), None, 260.0);
                                self.pending_sfx.push((Sfx::Nova, z.pos));
                            }
                            BossVar::BloomHeart => {
                                z.special_t = if enraged { 2.0 } else { 3.0 };
                                ring(if enraged { 18 } else { 12 }, self.rng.angle(), None, 240.0);
                                boss_summons.push((z.pos, ZKind::Runner, 3));
                                self.pending_sfx.push((Sfx::Spit, z.pos));
                            }
                        }
                    }
                    if var == BossVar::Matriarch {
                        // keeps her distance and strafes
                        if dist < 300.0 {
                            want = -dir;
                        } else if dist < 460.0 {
                            want = (dir.perp() * z.strafe + dir * 0.1).norm();
                            spd *= 0.7;
                        }
                    }
                    let charges = match var {
                        BossVar::Matriarch => false,
                        BossVar::BloomHeart => enraged,
                        _ => true,
                    };
                    let (wind, charge_speed) = match var {
                        BossVar::Wraith => (0.55, 760.0),
                        BossVar::BloomHeart => (0.9, 520.0),
                        _ => (0.75, 640.0),
                    };
                    match z.state {
                        0 => {
                            if z.timer <= 0.0 && playing && charges {
                                z.state = 1;
                                z.timer = wind;
                                z.charge_dir = dir;
                            }
                        }
                        1 => {
                            // wind-up: telegraph, track slowly
                            spd = 0.0;
                            z.charge_dir = z.charge_dir.lerp(dir, damp(3.0, DT)).norm();
                            if z.timer <= 0.0 {
                                z.state = 2;
                                z.timer = 0.6;
                                self.pending_sfx.push((Sfx::BossRoar, z.pos));
                            }
                        }
                        _ => {
                            want = z.charge_dir;
                            spd = charge_speed;
                            accel = 20.0;
                            if z.timer <= 0.0 {
                                z.state = 0;
                                z.timer = 3.2 + self.rng.range(0.0, 1.0);
                                if var == BossVar::CinderKing {
                                    blasts.push(z.pos);
                                }
                                if var == BossVar::Abomination && wave_n >= 10 {
                                    boss_summons.push((z.pos, ZKind::Runner, 4));
                                }
                            }
                        }
                    }
                }
                _ => {}
            }

            if z.kind != ZKind::Boss || z.state == 0 {
                if let Some(o) = world::blocking(z.pos, want, 46.0, z.r) {
                    let to_o = o.pos - z.pos;
                    let cross = want.x * to_o.y - want.y * to_o.x;
                    let side = if cross > 0.0 { -1.0 } else { 1.0 };
                    want = (want + want.perp() * (1.6 * side)).norm();
                }
            }

            let target_v = want * spd;
            z.vel += (target_v - z.vel) * damp(accel, DT);
            z.pos += z.vel * DT;
            z.anim += DT * (0.6 + z.vel.len() / 45.0);
            let mut fd = dir.angle() - z.face;
            while fd > PI {
                fd -= TAU;
            }
            while fd < -PI {
                fd += TAU;
            }
            z.face += fd * damp(6.0, DT);

            // contact with player
            let rr = z.r + PR;
            if dist < rr && playing {
                if z.attack_cd <= 0.0 {
                    contacts.push(i);
                    z.attack_cd = 0.8;
                }
                if dist > 0.01 {
                    z.pos = ppos - dir * rr;
                }
            }
        }

        for i in contacts {
            let (dmg, zp) = (self.zombies[i].dmg, self.zombies[i].pos);
            let was_iframe = self.player.iframes > 0.0 || self.player.dash_t > 0.0;
            self.hurt_player(dmg, zp);
            let tl = self.ups.get(Up::Thorns);
            if tl > 0 && !was_iframe {
                let d = (zp - self.player.pos).norm();
                let td = (25.0 + 30.0 * tl as f32) * self.dmg_mult();
                self.damage_zombie(i, td, d, 500.0, false);
            }
        }
        for i in fuses {
            if !self.zombies[i].dead {
                self.kill_zombie(i);
            }
        }
        for (at, kind, count) in boss_summons {
            for _ in 0..count {
                let p = at + self.rng.unit() * self.rng.range(60.0, 120.0);
                self.spawn_zombie(kind, p);
                self.burst(p, 6, 120.0, "#4a3a2a", 4.0, 0.5, true, false);
            }
        }
        for at in blasts {
            let dmg = 28.0 * self.difficulty().dmg;
            self.explode(at, 150.0, dmg, true);
        }

        // periodic ambient groans
        self.groan_t -= DT;
        if self.groan_t <= 0.0 {
            self.groan_t = self.rng.range(1.5, 4.0);
            if !self.zombies.is_empty() {
                let i = self.rng.below(self.zombies.len());
                let p = self.zombies[i].pos;
                self.sfx(Sfx::Groan, p);
            }
        }
    }

    /// Soft separation so hordes flow around each other instead of stacking.
    fn separate(&mut self) {
        let n = self.zombies.len();
        self.sep.clear();
        self.sep.resize(n, V2::ZERO);
        let zs = &self.zombies;
        let sep = &mut self.sep;
        for i in 0..n {
            let a = &zs[i];
            if a.dead {
                continue;
            }
            self.grid.query(a.pos, a.r + 50.0, |j| {
                if j <= i {
                    return;
                }
                let b = &zs[j];
                if b.dead {
                    return;
                }
                let d = b.pos - a.pos;
                let rr = a.r + b.r;
                let l2 = d.len2();
                if l2 < rr * rr {
                    let l = l2.sqrt();
                    let nrm = if l > 0.001 {
                        d * (1.0 / l)
                    } else {
                        v2(1.0, 0.0)
                    };
                    let overlap = rr - l;
                    let ma = a.r * a.r;
                    let mb = b.r * b.r;
                    let ta = mb / (ma + mb);
                    sep[i] -= nrm * (overlap * ta);
                    sep[j] += nrm * (overlap * (1.0 - ta));
                }
            });
        }
        for (z, s) in self.zombies.iter_mut().zip(self.sep.iter()) {
            if z.dead {
                continue;
            }
            z.pos += s.clamp_len(6.0) * 0.6;
            let (np, nrm) = world::resolve(z.pos, z.r);
            if nrm.len2() > 0.0 {
                z.pos = np;
                // slide: remove velocity into the obstacle
                let nn = nrm.norm();
                let into = z.vel.dot(nn);
                if into < 0.0 {
                    z.vel -= nn * into;
                }
            }
        }
    }

    fn update_bullets(&mut self) {
        let mut bullets = std::mem::take(&mut self.bullets);
        let homing = self.ups.get(Up::Homing);
        let expl = self.ups.get(Up::Explosive);
        for b in bullets.iter_mut() {
            if b.dead {
                continue;
            }
            b.prev = b.pos;
            b.life -= DT;
            if b.life <= 0.0 {
                b.dead = true;
                continue;
            }
            if homing > 0 {
                if let Some(t) =
                    self.nearest_zombie(b.pos, 220.0 + 40.0 * homing as f32, b.last_hit)
                {
                    let sp = b.vel.len();
                    let want = (self.zombies[t].pos - b.pos).norm() * sp;
                    b.vel = (b.vel.lerp(want, damp(3.0 + 3.0 * homing as f32, DT))).norm() * sp;
                }
            }
            b.pos += b.vel * DT;

            let mut hit = None;
            let zs = &self.zombies;
            let (bp, br, last) = (b.pos, b.r, b.last_hit);
            self.grid.query(bp, br + 50.0, |j| {
                if hit.is_some() {
                    return;
                }
                let z = &zs[j];
                if !z.dead && z.uid != last {
                    let rr = z.r + br;
                    if z.pos.dist2(bp) < rr * rr {
                        hit = Some(j);
                    }
                }
            });
            if let Some(j) = hit {
                let uid = self.zombies[j].uid;
                let dir = b.vel.norm();
                self.damage_zombie(j, b.dmg, dir, b.knock, b.crit);
                b.last_hit = uid;
                // impact sparks
                let col = if b.crit { "#ffd34a" } else { "#fff1c0" };
                self.burst(b.pos, 3, 200.0, col, 2.0, 0.15, false, true);
                if b.blast > 0.0 {
                    let r = b.blast * (1.0 + 0.3 * expl as f32);
                    self.small_blast(b.pos, r, b.dmg * 0.7, uid);
                    self.add_shake(0.08);
                } else if expl > 0 {
                    let r = 55.0 * (1.0 + 0.3 * (expl as f32 - 1.0));
                    let d = b.dmg * 0.45 * (1.0 + 0.3 * (expl as f32 - 1.0));
                    self.small_blast(b.pos, r, d, uid);
                }
                if b.bounces > 0 {
                    b.bounces -= 1;
                    if let Some(t) = self.nearest_zombie(b.pos, 320.0, uid) {
                        let sp = b.vel.len();
                        b.vel = (self.zombies[t].pos - b.pos).norm() * sp;
                        b.life = b.life.max(0.35);
                    } else if b.pierce > 0 {
                        b.pierce -= 1;
                    } else {
                        b.dead = true;
                    }
                } else if b.pierce > 0 {
                    b.pierce -= 1;
                } else {
                    b.dead = true;
                }
            }
        }
        bullets.retain(|b| !b.dead);
        bullets.append(&mut self.bullets);
        self.bullets = bullets;
    }

    fn small_blast(&mut self, pos: V2, r: f32, dmg: f32, skip: u32) {
        let mut hits = Vec::new();
        self.grid.query(pos, r + 50.0, |j| hits.push(j));
        for j in hits {
            let z = &self.zombies[j];
            if !z.dead && z.uid != skip && z.pos.dist(pos) < r + z.r {
                let dir = (z.pos - pos).norm();
                self.damage_zombie(j, dmg, dir, 160.0, false);
            }
        }
        self.ring(pos, r, 0.22, "#ff9a3a", 4.0);
        self.burst(pos, 6, 260.0, "#ffa040", 3.0, 0.25, false, true);
        self.sfx(Sfx::Explosion, pos);
        self.add_shake(0.04);
    }

    fn update_eshots(&mut self) {
        let ppos = self.player.pos;
        let mut hits = Vec::new();
        for (i, s) in self.eshots.iter_mut().enumerate() {
            s.prev = s.pos;
            s.pos += s.vel * DT;
            s.life -= DT;
            if s.life <= 0.0 {
                s.dead = true;
            } else if s.pos.dist(ppos) < PR + 7.0 {
                s.dead = true;
                hits.push(i);
            }
        }
        for i in hits {
            let (d, p) = (self.eshots[i].dmg, self.eshots[i].pos);
            self.hurt_player(d, p);
            self.burst(p, 10, 200.0, "#9be04a", 3.0, 0.4, true, false);
        }
        self.eshots.retain(|s| !s.dead);
    }

    fn update_pickups(&mut self) {
        let ppos = self.player.pos;
        let mr = self.magnet_r();
        let mut gained = 0.0;
        let mut picked = 0;
        for g in self.gems.iter_mut() {
            g.prev = g.pos;
            g.t += DT;
            let d = ppos - g.pos;
            let l = d.len();
            if !g.magnet && l < mr && g.t > 0.15 {
                g.magnet = true;
            }
            if g.magnet {
                let pull = 2400.0 + g.t * 400.0;
                g.vel += d * (1.0 / l.max(1.0)) * pull * DT;
                g.vel *= 1.0 - damp(4.0, DT);
            } else {
                g.vel *= 1.0 - damp(5.0, DT);
            }
            g.pos += g.vel * DT;
            if l < PR + 10.0 {
                g.dead = true;
                gained += g.value as f32;
                picked += 1;
            }
        }
        if picked > 0 {
            self.player.xp += gained;
            self.sfx(Sfx::Pickup, ppos);
        }

        let mut collected = Vec::new();
        for (i, pk) in self.pickups.iter_mut().enumerate() {
            pk.t += DT;
            let d = ppos - pk.pos;
            let l = d.len();
            if l < 60.0 {
                pk.pos += d.norm() * 300.0 * DT;
            }
            if l < PR + 14.0 {
                pk.dead = true;
                collected.push(i);
            }
            if pk.t > 30.0 {
                pk.dead = true;
            }
        }
        for i in collected {
            let k = self.pickups[i].kind;
            match k {
                PickKind::Heart => {
                    let heal = 30.0 + self.player.max_hp * 0.1;
                    self.player.hp = (self.player.hp + heal).min(self.player.max_hp);
                    self.text(ppos, format!("+{}", heal as i32), "#ff5c7a", 22.0);
                    self.burst(ppos, 16, 200.0, "#ff6a8a", 3.0, 0.6, false, true);
                    self.sfx(Sfx::Heal, ppos);
                }
                PickKind::Vacuum => {
                    for g in self.gems.iter_mut() {
                        g.magnet = true;
                    }
                    self.ring(ppos, 600.0, 0.6, "#b48cff", 6.0);
                    self.toast = Some(("Soul Vacuum!".into(), 1.2));
                    self.sfx(Sfx::Nova, ppos);
                }
                PickKind::Frenzy => {
                    self.player.frenzy = 8.0;
                    self.toast = Some(("FRENZY! Double fire rate".into(), 1.6));
                    self.burst(ppos, 20, 300.0, "#ffd84a", 3.0, 0.5, false, true);
                    self.sfx(Sfx::UltReady, ppos);
                }
            }
        }
    }

    fn update_effects(&mut self) {
        for p in self.particles.iter_mut() {
            p.life -= DT;
            p.pos += p.vel * DT;
            p.vel *= 1.0 - damp(p.drag, DT);
            if p.grav {
                p.vz -= 900.0 * DT;
                p.z += p.vz * DT;
                if p.z < 0.0 {
                    p.z = 0.0;
                    p.vz = -p.vz * 0.3;
                    p.vel *= 0.5;
                }
            }
        }
        self.particles.retain(|p| p.life > 0.0);
        for d in self.decals.iter_mut() {
            d.life -= DT;
        }
        self.decals.retain(|d| d.life > 0.0);
        for t in self.texts.iter_mut() {
            t.life -= DT;
            t.pos.y += t.vy * DT;
            t.vy *= 1.0 - damp(4.0, DT);
        }
        self.texts.retain(|t| t.life > 0.0);
        for r in self.rings.iter_mut() {
            r.life -= DT;
            let k = 1.0 - (r.life / r.max).max(0.0);
            let e = 1.0 - (1.0 - k).powi(3);
            r.r = r.max_r * e;
        }
        self.rings.retain(|r| r.life > 0.0);
        for b in self.bolts.iter_mut() {
            b.life -= DT;
        }
        self.bolts.retain(|b| b.life > 0.0);
        for g in self.ghosts.iter_mut() {
            g.life -= DT;
        }
        self.ghosts.retain(|g| g.life > 0.0);
        self.shake = (self.shake - DT * 1.6).max(0.0);
        self.combo_t -= DT;
        if self.combo_t <= 0.0 {
            self.combo = 0;
        }
    }

    // ---------------------------------------------------------------- waves
    fn is_boss_wave(&self, n: u32) -> bool {
        if self.campaign {
            n == CHAPTERS[self.chapter].waves
        } else {
            n % 5 == 0
        }
    }

    fn start_wave(&mut self, n: u32) {
        self.wave.n = n;
        let lvl = self.scale_n();
        let w = (lvl - 1) as f32;
        let base_total = 12.0 + 7.0 * w + w.powf(1.45);
        let camp = if self.campaign { 0.85 } else { 1.0 };
        self.wave.total = (base_total * self.difficulty().count * camp) as u32;
        self.wave.spawned = 0;
        self.wave.timer = 0.6;
        let boss = self.is_boss_wave(n);
        self.wave.boss_pending = boss;
        self.wave.boss_timer = 2.5;
        let new_record = !self.campaign && self.best_wave > 0 && n > self.best_wave;
        let (title, sub) = if self.campaign {
            let ch = &CHAPTERS[self.chapter];
            let t = if boss {
                format!("FINAL WAVE {}/{}", n, ch.waves)
            } else {
                format!("WAVE {}/{}", n, ch.waves)
            };
            let line = ch.radio.get((n - 1) as usize).copied().unwrap_or("");
            (t, line.to_string())
        } else {
            (
                if boss {
                    format!("WAVE {} — BOSS", n)
                } else {
                    format!("WAVE {}", n)
                },
                if new_record {
                    "NEW PERSONAL BEST!".into()
                } else {
                    format!(
                        "{} undead approach",
                        self.wave.total + if boss { 1 } else { 0 }
                    )
                },
            )
        };
        self.banner = Some(Banner {
            title,
            sub,
            t: 0.0,
            dur: if self.campaign { 3.6 } else { 2.4 },
            color: if boss { "#ff6a8a" } else { "#e8f0ff" },
        });
        self.sfx(Sfx::WaveStart, self.player.pos);
    }

    fn spawn_boss(&mut self, var: BossVar, pos: V2) {
        self.spawn_zombie(ZKind::Boss, pos);
        let (hm, r, sm) = var.stats();
        if let Some(z) = self.zombies.last_mut() {
            z.boss_var = var;
            z.hp *= hm;
            z.max_hp = z.hp;
            z.r = r;
            z.speed *= sm;
            z.special_t = 2.5;
            if var == BossVar::BloomHeart {
                z.xp *= 3;
            }
        }
    }

    fn campaign_victory(&mut self) {
        if self.victory_t >= 0.0 {
            return;
        }
        self.victory_t = 0.0;
        let bonus = (100.0 * (self.chapter + 1) as f32 * self.difficulty().coins) as u32;
        self.coins += bonus;
        // the horde collapses with its master
        for z in self.zombies.iter_mut() {
            z.dead = true;
        }
        for g in self.gems.iter_mut() {
            g.magnet = true;
        }
        self.eshots.clear();
        let p = self.player.pos;
        self.ring(p, 900.0, 1.2, "#ffd36a", 14.0);
        self.burst(p, 60, 700.0, "#ffd36a", 4.0, 1.0, false, true);
        self.banner = Some(Banner {
            title: "CHAPTER COMPLETE".into(),
            sub: format!("{} · +{} coins", CHAPTERS[self.chapter].title, bonus),
            t: 0.0,
            dur: 3.2,
            color: "#ffd36a",
        });
        self.audio.play(Sfx::WaveClear);
        self.audio.play(Sfx::LevelUp);
    }

    fn pick_kind(&mut self) -> ZKind {
        let n = self.scale_n() as f32;
        let mut opts: Vec<(ZKind, f32)> = vec![(ZKind::Walker, 10.0)];
        if n >= 2.0 {
            opts.push((ZKind::Runner, (2.0 + n * 0.4).min(8.0)));
        }
        if n >= 3.0 {
            opts.push((ZKind::Exploder, (1.2 + n * 0.15).min(4.0)));
        }
        if n >= 4.0 {
            opts.push((ZKind::Brute, (0.8 + n * 0.12).min(3.5)));
        }
        if n >= 6.0 {
            opts.push((ZKind::Spitter, (1.2 + n * 0.1).min(3.0)));
        }
        let total: f32 = opts.iter().map(|o| o.1).sum();
        let mut r = self.rng.f() * total;
        for (k, w) in opts {
            r -= w;
            if r <= 0.0 {
                return k;
            }
        }
        ZKind::Walker
    }

    fn spawn_zombie(&mut self, kind: ZKind, pos: V2) {
        let (hp, speed, r, dmg, xp) = kind.base();
        let n = self.scale_n();
        let df = self.difficulty();
        let (dhp, dspeed, ddmg) = (df.hp, df.speed, df.dmg);
        let hm = hp_mult(n)
            * if kind == ZKind::Boss {
                1.0 + (n / 5) as f32 * 0.35
            } else {
                1.0
            };
        let sm = 1.0 + (0.018 * (n - 1) as f32).min(0.35);
        let (p, _) = world::resolve(pos, r);
        let uid = self.next_uid;
        self.next_uid += 1;
        let face = (self.player.pos - p).angle();
        self.zombies.push(Zombie {
            pos: p,
            prev: p,
            vel: V2::ZERO,
            kind,
            hp: hp * hm * dhp,
            max_hp: hp * hm * dhp,
            speed: speed
                * sm
                * dspeed
                * if kind == ZKind::Boss {
                    1.0
                } else {
                    self.rng.range(0.88, 1.12)
                },
            r,
            dmg: dmg * (1.0 + 0.04 * (n - 1) as f32) * ddmg,
            xp,
            flash: 0.0,
            anim: self.rng.range(0.0, 10.0),
            attack_cd: 0.0,
            slow_t: 0.0,
            timer: if kind == ZKind::Boss {
                3.0
            } else {
                self.rng.range(0.5, 2.0)
            },
            state: 0,
            charge_dir: V2::ZERO,
            fuse: 0.0,
            blade_cd: 0.0,
            dash_mark: 0,
            dead: false,
            uid,
            face,
            strafe: if self.rng.chance(0.5) { 1.0 } else { -1.0 },
            tint: self.rng.f(),
            boss_var: BossVar::Abomination,
            special_t: 2.0,
        });
    }

    fn update_waves(&mut self) {
        if self.victory_t >= 0.0 {
            return;
        }
        if self.wave.intermission > 0.0 {
            self.wave.intermission -= DT;
            if self.wave.intermission <= 0.0 {
                let n = self.wave.n + 1;
                self.start_wave(n);
            }
            return;
        }
        let n = self.wave.n;
        let lvl = self.scale_n();
        let max_alive = (150 + 8 * lvl).min(340);
        let alive = self.alive();
        self.wave.timer -= DT;
        if self.wave.spawned < self.wave.total && self.wave.timer <= 0.0 && alive < max_alive {
            let interval = (1.1 - lvl as f32 * 0.045).max(0.22) / self.difficulty().count;
            self.wave.timer = interval * self.rng.range(0.7, 1.3);
            let gmax = 3 + lvl / 2;
            let g =
                (2 + self.rng.below(gmax as usize) as u32).min(self.wave.total - self.wave.spawned);
            let ppos = self.player.pos;
            let pv = self.player.vel;
            let a = if pv.len2() > 900.0 && self.rng.chance(0.4) {
                pv.angle() + self.rng.range(-0.7, 0.7)
            } else {
                self.rng.angle()
            };
            let base = ppos + V2::from_angle(a) * self.spawn_r;
            for _ in 0..g {
                let k = self.pick_kind();
                let p = base + self.rng.unit() * self.rng.range(0.0, 70.0);
                self.spawn_zombie(k, p);
            }
            self.wave.spawned += g;
        }
        if self.wave.boss_pending {
            self.wave.boss_timer -= DT;
            if self.wave.boss_timer <= 0.0 {
                self.wave.boss_pending = false;
                let (var, twin) = if self.campaign {
                    let ch = &CHAPTERS[self.chapter];
                    (ch.boss, ch.twin)
                } else {
                    let i = ((n / 5).max(1) - 1) as usize % content::INFINITE_BOSSES.len();
                    (content::INFINITE_BOSSES[i], false)
                };
                let a = self.rng.angle();
                let p = self.player.pos + V2::from_angle(a) * (self.spawn_r - 40.0);
                self.spawn_boss(var, p);
                if twin {
                    let p2 = self.player.pos + V2::from_angle(a + PI) * (self.spawn_r - 40.0);
                    self.spawn_boss(var, p2);
                }
                self.toast = Some((var.name().to_string(), 2.5));
                self.sfx(Sfx::BossRoar, self.player.pos);
                self.add_shake(0.5);
            }
        }
        if self.wave.spawned >= self.wave.total && !self.wave.boss_pending && alive == 0 && n > 0 {
            self.wave_cleared();
        } else if n == 0 {
            self.wave.intermission = 0.01;
        }
    }

    fn wave_cleared(&mut self) {
        self.wave.intermission = 3.5;
        for g in self.gems.iter_mut() {
            g.magnet = true;
        }
        let heal = self.player.max_hp * 0.1;
        self.player.hp = (self.player.hp + heal).min(self.player.max_hp);
        let earned = ((10 + 5 * self.scale_n()) as f32 * self.difficulty().coins) as u32;
        self.coins += earned;
        self.banner = Some(Banner {
            title: format!("WAVE {} CLEARED", self.wave.n),
            sub: format!(
                "+{} coins · +{} HP · next wave incoming",
                earned, heal as i32
            ),
            t: 0.0,
            dur: 2.6,
            color: "#9cffb8",
        });
        self.sfx(Sfx::WaveClear, self.player.pos);
        self.ui_out.push(("wave", format!("{}", self.wave.n)));
    }
}
