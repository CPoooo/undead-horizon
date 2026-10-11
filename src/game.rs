use crate::audio::{Audio, Sfx, Track};
use crate::content::{self, BossVar, CHAPTERS, DIFFICULTIES, GUNS};
use crate::math::*;
use crate::input::{Action, Bindings};
use crate::modes::Mode;
use crate::status::Elem;
use crate::net::{self, Net};
use crate::upgrades::{Evo, Levels, Up, EVOS};
use crate::world;
use std::f32::consts::{PI, TAU};

/// Simulation runs at a fixed 120 Hz; rendering interpolates between steps.
pub const DT: f32 = 1.0 / 120.0;
pub const PR: f32 = 16.0; // player radius
/// Save-string format version (`v=`). Older saves (no `v`) still load.
pub const SAVE_VERSION: u32 = 2;

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
    Crawler,
    Screamer,
    Armored,
    Bat,
    Burrower,
    Hound,
    Sapper,
    Ogre,
    Archer,
}

impl ZKind {
    pub const ALL: [ZKind; 15] = [
        ZKind::Walker,
        ZKind::Runner,
        ZKind::Brute,
        ZKind::Spitter,
        ZKind::Exploder,
        ZKind::Boss,
        ZKind::Crawler,
        ZKind::Screamer,
        ZKind::Armored,
        ZKind::Bat,
        ZKind::Burrower,
        ZKind::Hound,
        ZKind::Sapper,
        ZKind::Ogre,
        ZKind::Archer,
    ];

    pub fn def(self) -> &'static content::EnemyDef {
        &content::ENEMIES[self as usize]
    }

    /// Wire / save id back to a kind (unknown ids become walkers).
    pub fn from_u8(k: u8) -> ZKind {
        ZKind::ALL.get(k as usize).copied().unwrap_or(ZKind::Walker)
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
    // co-op
    pub downed: bool,
    pub dead: bool,
    pub revive: f32,
    pub bleed: f32,
    pub shots: u8,
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
    pub st: crate::status::Status,
    pub affix: crate::elites::Affix,
    pub shield: f32,
    /// screamer enrage: faster while > 0
    pub rage_t: f32,
    /// Outbreak: index of the window it is breaking in through (-1 = inside)
    pub win: i8,
    /// Home Base firing-range dummy: never moves, never dies, never hurts
    pub dummy: bool,
    /// boss phase (1..3) and lifetime, for phases and the enrage timer
    pub phase: u8,
    pub age: f32,
    /// underground (burrowers): not in the spatial grid, so nothing can hit it
    pub hidden: bool,
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
    /// slot of the player who last damaged it (kill credit in co-op)
    pub last_hitter: u8,
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
    /// a teammate's shot: drawn locally, damage is dealt on their client
    pub cosmetic: bool,
    pub tracer: u8,
}

pub struct EShot {
    pub pos: V2,
    pub prev: V2,
    pub vel: V2,
    pub life: f32,
    pub dmg: f32,
    pub dead: bool,
    /// homing turn rate (0 = flies straight)
    pub turn: f32,
    /// hit radius (spit 6.5, thrown rocks bigger)
    pub r: f32,
    /// already counted as a near miss
    pub grazed: bool,
}

pub struct Gem {
    pub pos: V2,
    pub prev: V2,
    pub vel: V2,
    pub value: u32,
    pub t: f32,
    pub magnet: bool,
    pub dead: bool,
    /// Siege mana orb instead of XP
    pub mana: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PickKind {
    Heart,
    Vacuum,
    Frenzy,
    Chest,
    // Outbreak power-ups
    MaxAmmo,
    InstaKill,
    DoublePoints,
    Nuke,
    Carpenter,
}

impl PickKind {
    pub fn is_powerup(self) -> bool {
        matches!(
            self,
            PickKind::MaxAmmo | PickKind::InstaKill | PickKind::DoublePoints | PickKind::Nuke | PickKind::Carpenter
        )
    }
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

/// A short-lived point light (muzzle flashes, explosions, novas).
pub struct Light {
    pub pos: V2,
    pub r: f32,
    pub life: f32,
    pub max: f32,
    pub color: &'static str,
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
    /// key codes pressed this frame (mapped through `Game::bindings`)
    pub pressed: Vec<String>,
    /// actions pushed directly (gamepad, touch buttons, tests)
    pub actions: Vec<Action>,
    pub clicks: Vec<V2>,
    pub touch_move: V2,
    pub touch_active: bool,
    pub is_touch: bool,
    pub dash_req: bool,
    pub ult_req: bool,
    pub joy_id: Option<i32>,
    pub joy_origin: V2,
    pub joy_pos: V2,
    /// interact held (rebuilding barricades)
    pub use_down: bool,
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
    pub(crate) fn new() -> Self {
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
    pub(crate) fn cell_xy(&self, p: V2) -> (i32, i32) {
        let x = ((p.x - self.origin.x) / self.cell).floor() as i32;
        let y = ((p.y - self.origin.y) / self.cell).floor() as i32;
        (x.clamp(0, self.n - 1), y.clamp(0, self.n - 1))
    }
    pub(crate) fn build(&mut self, center: V2, zs: &[Zombie]) {
        let half = self.n as f32 * self.cell * 0.5;
        self.origin = center - v2(half, half);
        self.start.iter_mut().for_each(|s| *s = 0);
        let mut total = 0;
        for z in zs {
            if !z.dead && !z.hidden {
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
            if !z.dead && !z.hidden {
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
    pub lights: Vec<Light>,
    pub banner: Option<Banner>,
    pub feed: Vec<crate::callouts::FeedItem>,
    pub callout: Option<crate::callouts::Callout>,
    pub multi_n: u32,
    pub multi_t: f32,
    pub score: u64,
    pub toast: Option<(String, f32)>,
    pub wave: Wave,
    pub ups: Levels,
    pub offers: Vec<Up>,
    pub sel: usize,
    pub levelup_t: f32,
    pub pending_levels: u32,
    /// level-up charges for this run
    pub rerolls: u32,
    pub banishes: u32,
    pub skips: u32,
    pub banished: Vec<Up>,
    /// the next card picked is banished instead of taken
    pub banish_mode: bool,
    /// an open treasure chest (drawn over the level-up phase)
    pub chest: Option<crate::chest::Chest>,
    pub chests_opened: u32,
    pub evolved: Vec<Evo>,
    /// volatile elite blasts waiting to go off: (pos, fuse, damage)
    pub volatile: Vec<(V2, f32, f32)>,
    pub slams: Vec<crate::boss::Slam>,
    pub corpses: Vec<crate::corpses::Corpse>,
    /// Outbreak state (points, guns, perks, windows, box...)
    pub ob: Option<Box<crate::modes::outbreak::ObState>>,
    /// pathfinding on authored maps
    pub flow: Option<crate::nav::FlowField>,
    pub nuke_pending: bool,
    /// Outbreak traps, buildables and the easter egg
    pub secrets: Option<Box<crate::secrets::Secrets>>,
    pub siege: Option<Box<crate::modes::siege::SiegeState>>,
    pub hub: Option<Box<crate::modes::hub::HubState>>,
    /// the touch USE button is visible
    pub use_shown: bool,
    pub ready_shown: bool,
    pub boss_intro: Option<crate::boss::BossIntro>,
    /// chests picked up while another overlay was open
    pub pending_chests: u32,
    pub rng: Rng,
    pub grid: Grid,
    pub audio: Audio,
    pub input: Input,
    pub bindings: Bindings,
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
    /// recoil camera offset, decays back to zero
    pub cam_kick: V2,
    /// aim lookahead offset (smoothed) and its strength setting (0 = off)
    pub cam_look: V2,
    pub lookahead: f32,
    /// brief zoom-in (ult, boss spawn), decays to 0
    pub zoom_punch: f32,
    /// real seconds of slow motion left (boss kills)
    pub slowmo: f32,
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
    pub mode: Mode,
    /// `chapter=` may arrive before `mode=` in the options string
    pub pending_chapter: usize,
    pub diff: usize,
    pub gun: usize,
    pub perk: String,
    pub skin: usize,
    pub hat: usize,
    pub tracer: usize,
    /// permanent upgrades, relic bonuses and gun camo from the profile
    pub meta: content::Meta,
    pub mutators: content::Mutators,
    /// daily run: everyone gets the same seed
    pub daily: String,
    pub relics: content::RelicBonus,
    pub camo: usize,
    /// kills per enemy kind and bosses felled this run (achievements, bestiary)
    pub kind_kills: [u32; 15],
    pub bosses_killed: u32,
    /// character (content::CHARACTERS) and their ability state
    pub character: usize,
    pub ability_cd: f32,
    pub ability_t: f32,
    pub turrets: Vec<crate::abilities::Turret>,
    pub coins: u32,
    /// authored map id (`start|map=asylum`); empty = the procedural world
    pub map_id: String,
    /// challenge variant (Siege: "glass")
    pub variant: String,
    pub victory_t: f32,
    pub ult_queued: bool,
    // co-op networking (None in single player)
    pub net: Option<Net>,
    pub net_in: Vec<(u8, Vec<u8>)>,
    pub net_out: Vec<(u8, bool, Vec<u8>)>,
    pub net_remaining: u32,
    pub predicted: Vec<(u32, f64)>,
    pub hit_slot: u8,
    /// party members shown next to us in the lobby: (name, skin, hat, gun)
    pub lobby_mates: Vec<(String, usize, usize, usize)>,
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
            lights: Vec::new(),
            banner: None,
            feed: Vec::new(),
            callout: None,
            multi_n: 0,
            multi_t: 0.0,
            score: 0,
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
            rerolls: 0,
            banishes: 0,
            skips: 0,
            banished: Vec::new(),
            banish_mode: false,
            chest: None,
            chests_opened: 0,
            evolved: Vec::new(),
            volatile: Vec::new(),
            slams: Vec::new(),
            corpses: Vec::new(),
            ob: None,
            flow: None,
            nuke_pending: false,
            secrets: None,
            siege: None,
            hub: None,
            use_shown: false,
            ready_shown: false,
            boss_intro: None,
            pending_chests: 0,
            rng: Rng::new(seed),
            grid: Grid::new(),
            audio: Audio::new(),
            input: Input::default(),
            bindings: Bindings::default(),
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
            cam_kick: V2::ZERO,
            cam_look: V2::ZERO,
            lookahead: 1.0,
            zoom_punch: 0.0,
            slowmo: 0.0,
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
            mode: Mode::Infinite,
            pending_chapter: 0,
            diff: 1,
            gun: 0,
            perk: String::new(),
            skin: 0,
            hat: 0,
            tracer: 0,
            meta: content::Meta::default(),
            mutators: content::Mutators::default(),
            daily: String::new(),
            relics: content::RelicBonus::default(),
            camo: 0,
            kind_kills: [0; 15],
            bosses_killed: 0,
            character: 0,
            ability_cd: 0.0,
            ability_t: 0.0,
            turrets: Vec::new(),
            coins: 0,
            map_id: String::new(),
            variant: String::new(),
            victory_t: -1.0,
            ult_queued: false,
            net: None,
            net_in: Vec::new(),
            net_out: Vec::new(),
            net_remaining: 0,
            predicted: Vec::new(),
            hit_slot: 0,
            lobby_mates: Vec::new(),
        };
        g.player.pos = v2(0.0, 0.0);
        g
    }

    pub(crate) fn fresh_player() -> Player {
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
            downed: false,
            dead: false,
            revive: 0.0,
            bleed: 0.0,
            shots: 0,
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
                "mode" => {
                    self.mode = match v {
                        "campaign" => Mode::Campaign(self.chapter()),
                        "outbreak" => Mode::Outbreak,
                        "siege" => Mode::Siege,
                        "hub" => Mode::Hub,
                        _ => Mode::Infinite,
                    }
                }
                "chapter" => {
                    let ch = v.parse::<usize>().unwrap_or(0).min(CHAPTERS.len() - 1);
                    if let Mode::Campaign(c) = &mut self.mode {
                        *c = ch;
                    }
                    self.pending_chapter = ch;
                }
                "diff" => self.diff = content::difficulty_index(v),
                "gun" => self.gun = content::gun_index(v),
                "perk" => self.perk = v.to_string(),
                "skin" => self.skin = content::skin_index(v),
                "hat" => self.hat = content::hat_index(v),
                "tracer" => self.tracer = content::tracer_index(v),
                "char" => self.character = content::character_index(v),
                "meta" => self.meta = content::Meta::parse(v),
                "mut" => self.mutators = content::Mutators::parse(v),
                "seed" => {
                    if let Ok(n) = v.parse::<u64>() {
                        self.rng = Rng::new(n);
                    }
                }
                "daily" => self.daily = v.chars().take(10).collect(),
                "relics" => self.relics = content::relic_bonus(v),
                "camo" => self.camo = v.parse::<usize>().unwrap_or(0).min(content::CAMOS.len() - 1),
                "map" => self.map_id = v.to_string(),
                "variant" => self.variant = v.to_string(),
                _ => {}
            }
        }
    }

    pub fn difficulty(&self) -> &'static content::Difficulty {
        &DIFFICULTIES[self.diff]
    }

    pub fn is_campaign(&self) -> bool {
        matches!(self.mode, Mode::Campaign(_))
    }

    /// Current campaign chapter (0 outside the campaign).
    pub fn chapter(&self) -> usize {
        match self.mode {
            Mode::Campaign(c) => c,
            _ => self.pending_chapter,
        }
    }

    /// Wave number used for scaling: campaign chapters start partway up the curve.
    pub fn scale_n(&self) -> u32 {
        let n = self.wave.n.max(1);
        if self.is_campaign() {
            CHAPTERS[self.chapter()].base + n - 1
        } else {
            n
        }
    }

    pub fn start(&mut self, opts: &str) {
        self.audio.unlock();
        self.audio.music_on = true;
        self.audio.set_track(Track::Gameplay);
        self.best_wave = 0;
        self.map_id.clear();
        self.variant.clear();
        self.mutators = content::Mutators::default();
        self.daily.clear();
        self.mode = Mode::Infinite;
        self.pending_chapter = 0;
        self.apply_opts(opts);
        self.player = Game::fresh_player();
        let map = world::load_builtin(&self.map_id);
        if let Some(m) = &map {
            self.player.pos = m.spawn;
            self.player.prev = m.spawn;
        }
        world::use_map(map);
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
        self.rerolls = 2 + self.meta.reroll as u32;
        self.banishes = 2 + self.meta.banish as u32;
        self.skips = 3;
        self.banished.clear();
        self.banish_mode = false;
        self.chest = None;
        self.chests_opened = 0;
        self.ability_cd = 3.0;
        self.ability_t = 0.0;
        self.turrets.clear();
        self.evolved.clear();
        self.volatile.clear();
        self.slams.clear();
        self.corpses.clear();
        self.ob = None;
        self.flow = None;
        self.nuke_pending = false;
        self.secrets = None;
        self.siege = None;
        self.hub = None;
        self.boss_intro = None;
        self.pending_chests = 0;
        self.kills = 0;
        self.kind_kills = [0; 15];
        self.bosses_killed = 0;
        self.score = 0;
        self.feed.clear();
        self.callout = None;
        self.multi_n = 0;
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
        self.ult_queued = false;
        self.net = None;
        self.net_in.clear();
        self.net_out.clear();
        self.predicted.clear();
        self.hit_slot = 0;
        self.wave = Wave {
            n: 0,
            total: 0,
            spawned: 0,
            timer: 0.0,
            intermission: 1.2,
            boss_timer: 0.0,
            boss_pending: false,
        };
        // permanent upgrades and relics
        let bonus_hp = 5.0 * self.meta.hp as f32 + self.relics.hp;
        self.player.max_hp += bonus_hp;
        if self.mutators.glass {
            self.player.max_hp *= 0.5;
        }
        self.player.hp = self.player.max_hp;
        self.player.hp_ghost = self.player.hp;
        // starting perk from the loadout
        if let Some(u) = content::perk_upgrade(&self.perk) {
            self.ups.inc(u);
            if u == Up::Vitality {
                self.player.max_hp += 25.0;
                self.player.hp = self.player.max_hp;
                self.player.hp_ghost = self.player.hp;
            }
        }
        self.phase = Phase::Playing;
        self.mode.hooks().on_start(self);
        self.audio.play(Sfx::UiSelect);
    }

    // ---------------------------------------------------------------- stats
    /// Armored zombies shrug off 75% of damage from the front (crits punch through).
    pub fn armor_mult(&self, j: usize, dir: V2, crit: bool) -> f32 {
        let z = &self.zombies[j];
        if z.kind != ZKind::Armored || crit || dir.len2() < 0.01 {
            return 1.0;
        }
        // `dir` points from the shooter into the zombie; a frontal hit opposes its facing
        if dir.dot(V2::from_angle(z.face)) < -0.4 {
            0.25
        } else {
            1.0
        }
    }

    /// A shot whizzed past: a little score and ultimate charge.
    fn graze(&mut self) {
        if self.player.iframes > 0.0 {
            return; // dashing through doesn't count
        }
        self.score += 10;
        self.player.ult = (self.player.ult + 0.01).min(1.0);
        let p = self.player.pos;
        self.text(p + v2(0.0, -30.0), "GRAZE".into(), "#9fe0ff", 12.0);
        self.sfx(Sfx::UiMove, p);
    }

    pub fn has_evo(&self, e: Evo) -> bool {
        self.evolved.contains(&e)
    }

    /// The next evolution a chest would grant, if any.
    pub fn evo_ready(&self) -> Option<Evo> {
        EVOS.iter()
            .copied()
            .find(|e| !self.evolved.contains(e) && e.ready(&self.ups))
    }

    pub fn dmg_mult(&self) -> f32 {
        (1.0 + 0.25 * self.ups.f(Up::Damage))
            * (1.0 + 0.08 * self.ups.f(Up::Velocity))
            * (1.0 + 0.02 * self.meta.dmg as f32 + self.relics.dmg / 100.0)
            * if self.mutators.glass { 2.0 } else { 1.0 }
    }
    pub(crate) fn fire_interval(&self) -> f32 {
        let class = if self.kit() == content::Kit::HeatSink { 1.1 } else { 1.0 };
        let heat = if self.kit() == content::Kit::HeatSink && self.ability_t > 0.0 { 2.0 } else { 1.0 };
        let base = 0.30 * GUNS[self.gun].interval
            / ((1.0 + 0.16 * self.ups.f(Up::FireRate)) * class * heat * self.ob_fire_mult()
                * (1.0 + self.relics.rate / 100.0));
        if self.player.frenzy > 0.0 {
            base * 0.5
        } else {
            base
        }
    }
    pub(crate) fn move_speed(&self) -> f32 {
        255.0 * (1.0 + 0.09 * self.ups.f(Up::Speed)) * GUNS[self.gun].move_mult * self.ob_speed_mult()
            * (1.0 + 0.02 * self.meta.spd as f32 + self.relics.speed / 100.0)
    }
    pub fn magnet_r(&self) -> f32 {
        let eng = if self.kit() == content::Kit::Turret { 1.3 } else { 1.0 };
        95.0 * (1.0 + 0.45 * self.ups.f(Up::Magnet)) * eng
            * (1.0 + 0.06 * self.meta.magnet as f32 + self.relics.magnet / 100.0)
    }
    pub(crate) fn crit_chance(&self) -> f32 {
        0.05 + 0.10 * self.ups.f(Up::Crit) + self.relics.crit / 100.0
    }
    pub fn dash_cooldown(&self) -> f32 {
        1.1 * 0.8f32.powi(self.ups.get(Up::DashMaster) as i32)
    }
    pub fn blade_count(&self) -> u32 {
        let l = self.ups.get(Up::Blades) as u32;
        if l == 0 {
            0
        } else {
            l + 1 + if self.has_evo(Evo::ReaperRing) { 2 } else { 0 }
        }
    }
    pub fn blade_radius(&self) -> f32 {
        (80.0 + 6.0 * self.ups.f(Up::Blades)) * if self.has_evo(Evo::ReaperRing) { 1.3 } else { 1.0 }
    }
    pub fn nova_radius(&self) -> f32 {
        (150.0 + 25.0 * self.ups.f(Up::Nova)) * if self.has_evo(Evo::AbsoluteZero) { 1.4 } else { 1.0 }
    }
    pub fn alive(&self) -> u32 {
        self.zombies.iter().filter(|z| !z.dead).count() as u32
    }
    pub fn remaining(&self) -> u32 {
        if self.is_client() {
            return self.net_remaining;
        }
        self.wave.total.saturating_sub(self.wave.spawned)
            + self.alive()
            + if self.wave.boss_pending { 1 } else { 0 }
    }

    // ---------------------------------------------------------------- helpers
    pub fn push_sfx(&mut self, s: Sfx, at: V2) {
        self.sfx(s, at);
    }
    pub fn push_text(&mut self, pos: V2, s: String, color: &'static str, size: f32) {
        self.text(pos, s, color, size);
    }

    pub(crate) fn sfx(&mut self, s: Sfx, at: V2) {
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

    /// Announcer voice line (Web Speech in the UI, behind a setting).
    pub fn say(&mut self, line: &str) {
        if !self.ui_out.iter().any(|e| e.0 == "say") {
            self.ui_out.push(("say", line.to_string()));
        }
    }

    /// Rumble the phone / gamepad (ms, 0..1 strength); the UI scales it by the setting.
    pub fn haptic(&mut self, ms: u32, strength: f32) {
        if self.ui_out.iter().filter(|e| e.0 == "haptic").count() < 2 {
            self.ui_out.push(("haptic", format!("{}:{:.2}", ms, strength)));
        }
    }

    pub fn add_shake(&mut self, amt: f32) {
        self.shake = (self.shake + amt).min(1.0);
    }

    pub(crate) fn text(&mut self, pos: V2, s: String, color: &'static str, size: f32) {
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
    pub(crate) fn burst(
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

    pub fn ring(&mut self, pos: V2, max_r: f32, life: f32, color: &'static str, width: f32) {
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

    pub fn light(&mut self, pos: V2, r: f32, life: f32, color: &'static str) {
        if self.lights.len() < 64 {
            self.lights.push(Light {
                pos,
                r,
                life,
                max: life,
                color,
            });
        }
    }

    pub(crate) fn nearest_zombie(&self, p: V2, max_d: f32, exclude: u32) -> Option<usize> {
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
            Phase::Playing | Phase::Dying => self.simulate(real_dt),
            Phase::LevelUp | Phase::Paused if self.net.is_some() => {
                if self.phase == Phase::LevelUp {
                    self.levelup_t += real_dt;
                    self.tick_chest(real_dt);
                }
                self.simulate(real_dt);
            }
            Phase::LevelUp => {
                self.levelup_t += real_dt;
                self.tick_chest(real_dt);
            }
            Phase::Paused | Phase::GameOver => {}
        }
        self.net_tick();

        if let Some(b) = &mut self.banner {
            if self.phase != Phase::Paused && (self.phase != Phase::LevelUp || self.net.is_some()) {
                b.t += real_dt;
            }
            if b.t > b.dur {
                self.banner = None;
            }
        }
        self.frame_tail(real_dt);
    }

    pub(crate) fn simulate(&mut self, real_dt: f32) {
        {
            {
                let mut scale = if self.phase == Phase::Dying { 0.3 } else { 1.0 };
                if self.slowmo > 0.0 {
                    self.slowmo -= real_dt;
                    // ease back to full speed over the last third
                    scale *= 0.3 + 0.7 * (1.0 - (self.slowmo / 0.5).min(1.0));
                }
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
                if self.mode.hooks().is_over(self) && self.phase == Phase::Playing {
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
        }
    }

    pub(crate) fn frame_tail(&mut self, real_dt: f32) {
        if let Some(t) = &mut self.toast {
            t.1 -= real_dt;
            if t.1 <= 0.0 {
                self.toast = None;
            }
        }
        self.ult_flash = (self.ult_flash - real_dt * 2.5).max(0.0);
        self.cam_kick = self.cam_kick * (1.0 - damp(16.0, real_dt));
        self.zoom_punch = (self.zoom_punch - real_dt * 0.6).max(0.0);
        self.tick_callouts(real_dt);
        self.tick_boss_intro(real_dt);
        if self.phase != Phase::Paused {
            self.update_corpses(real_dt);
        }
        let look = if matches!(self.phase, Phase::Playing) && !self.player.downed {
            V2::from_angle(self.player.aim) * (55.0 * self.lookahead)
        } else {
            V2::ZERO
        };
        self.cam_look = self.cam_look.lerp(look, damp(3.0, real_dt));
        for l in self.lights.iter_mut() {
            l.life -= real_dt;
        }
        self.lights.retain(|l| l.life > 0.0);

        // music intensity follows the action
        let alive = self.alive() as f32;
        let target = if matches!(self.phase, Phase::Playing | Phase::LevelUp) {
            ((alive / 120.0) + (self.wave.n as f32 / 30.0)).min(1.0)
        } else {
            0.0
        };
        self.audio.intensity += (target - self.audio.intensity) * damp(0.5, real_dt);
        let low = if matches!(self.phase, Phase::Playing | Phase::LevelUp) && !self.player.dead {
            (1.0 - self.player.hp / (self.player.max_hp * 0.3).max(1.0)).clamp(0.0, 1.0)
        } else {
            0.0
        };
        self.audio.low_hp += (low - self.audio.low_hp) * damp(4.0, real_dt);
        self.flush_sfx();
        let _ = self.audio.update_music();
    }

    pub(crate) fn handle_input(&mut self, _dt: f32) {
        let mut pressed = std::mem::take(&mut self.input.pressed);
        let mut clicks = std::mem::take(&mut self.input.clicks);
        // Siege build hotkeys 1-6 and build clicks
        if self.siege.is_some() && self.phase == Phase::Playing {
            pressed.retain(|c| match c.strip_prefix("Digit").and_then(|d| d.parse::<usize>().ok()) {
                Some(d) if (1..=6).contains(&d) => {
                    self.select_build(d - 1);
                    false
                }
                _ => true,
            });
            for c in std::mem::take(&mut clicks) {
                self.siege_click(c);
            }
        }
        let direct = std::mem::take(&mut self.input.actions);
        // each key's actions in priority order: the first one that applies wins
        // in co-op the match runs during level-up, so movement keys keep moving
        let coop_pick = self.net.is_some() && self.phase == Phase::LevelUp;
        let mut groups: Vec<Vec<Action>> = pressed
            .iter()
            .map(|c| {
                let acts = self.bindings.actions(c);
                if coop_pick && acts.iter().any(|a| a.is_held()) {
                    Vec::new()
                } else {
                    acts.to_vec()
                }
            })
            .collect();
        groups.extend(direct.into_iter().map(|a| vec![a]));
        for group in groups {
            for a in group {
                if self.on_action(a) {
                    break;
                }
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
        if self.phase == Phase::LevelUp && self.chest.is_some() {
            if !clicks.is_empty() {
                self.close_chest();
            }
        } else if self.phase == Phase::LevelUp {
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
                let buttons = self.levelup_buttons();
                for c in clicks.iter() {
                    for (k, r) in buttons.iter().enumerate() {
                        if c.x >= r.0 && c.x <= r.0 + r.2 && c.y >= r.1 && c.y <= r.1 + r.3 {
                            match k {
                                0 => self.reroll(),
                                1 => self.toggle_banish(),
                                _ => self.skip_levelup(),
                            }
                            return;
                        }
                    }
                }
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

    /// Handle one pressed action; returns true if it did something.
    pub(crate) fn on_action(&mut self, a: Action) -> bool {
        match self.phase {
            Phase::Playing => match a {
                Action::Pause if self.siege.as_ref().is_some_and(|s| s.build_sel.is_some()) => {
                    if let Some(s) = self.siege.as_mut() {
                        s.build_sel = None;
                    }
                }
                Action::Pause if self.hub.is_some() => self.ui_out.push(("hubmenu", String::new())),
                Action::Pause => self.pause(),
                Action::Dash => self.player.dash_buf = 0.18,
                Action::Interact => return self.try_interact(),
                Action::Ult => self.try_ult(),
                Action::Ability => return self.try_ability(),
                Action::Reload if self.ob.is_some() => self.start_reload(),
                Action::Confirm if self.siege.is_some() => return self.ready_up(),
                Action::Swap if self.siege.is_some() => return self.sell_defense(),
                Action::Swap if self.ob.is_some() => return self.swap_weapon(None),
                Action::Pick1 if self.ob.is_some() => return self.swap_weapon(Some(0)),
                Action::Pick2 if self.ob.is_some() => return self.swap_weapon(Some(1)),
                Action::AutoAim => {
                    self.auto_aim = !self.auto_aim;
                    let msg = if self.auto_aim {
                        "Auto-aim ON"
                    } else {
                        "Auto-aim OFF (mouse aim)"
                    };
                    self.toast = Some((msg.into(), 1.6));
                    self.audio.play(Sfx::UiMove);
                    self.ui_out
                        .push(("autoaim", if self.auto_aim { "1" } else { "0" }.into()));
                }
                Action::Music => {
                    let v = if self.audio.music_volume > 0.0 { 0.0 } else { 0.5 };
                    self.audio.set_music_volume(v);
                    let msg = if v > 0.0 { "Music ON" } else { "Music OFF" };
                    self.toast = Some((msg.into(), 1.2));
                }
                Action::Fps => self.show_fps = !self.show_fps,
                Action::Minimap => {
                    self.show_minimap = !self.show_minimap;
                    let msg = if self.show_minimap { "Minimap ON" } else { "Minimap OFF" };
                    self.toast = Some((msg.into(), 1.0));
                    self.audio.play(Sfx::UiMove);
                }
                _ => return false,
            },
            Phase::Paused => match a {
                Action::Pause => self.resume(),
                _ => return false,
            },
            Phase::LevelUp => {
                if self.chest.is_some() {
                    return match a {
                        Action::Confirm | Action::Pick1 | Action::Pick2 | Action::Pick3
                        | Action::Dash | Action::Interact | Action::Pause => self.close_chest(),
                        _ => false,
                    };
                }
                if self.levelup_t < 0.35 {
                    return true;
                }
                match a {
                    Action::Pick1 => self.choose(0),
                    Action::Pick2 => self.choose(1),
                    Action::Pick3 => self.choose(2),
                    Action::Confirm => self.choose(self.sel),
                    Action::Reroll => self.reroll(),
                    Action::Banish => self.toggle_banish(),
                    Action::Skip => self.skip_levelup(),
                    Action::Prev => {
                        if self.sel > 0 {
                            self.sel -= 1;
                            self.audio.play(Sfx::UiMove);
                        }
                    }
                    Action::Next => {
                        if self.sel + 1 < self.offers.len() {
                            self.sel += 1;
                            self.audio.play(Sfx::UiMove);
                        }
                    }
                    _ => return false,
                }
            }
            _ => return false,
        }
        true
    }

    /// Use whatever interactable is in reach (doors, machines...). Modes with
    /// authored maps fill this in; returns false when there is nothing to use.
    pub(crate) fn try_interact(&mut self) -> bool {
        if self.ob.is_some() {
            return self.ob_interact();
        }
        if self.siege.is_some() {
            return self.siege_interact();
        }
        if self.hub.is_some() {
            return self.hub_interact();
        }
        false
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
        world::use_map(None);
        self.ob = None;
        self.flow = None;
        self.siege = None;
        self.hub = None;
        self.audio.set_track(Track::Lobby);
        self.audio.music_on = true;
        self.net = None;
        self.player = Game::fresh_player();
        self.ups = Levels::default();
        self.victory_t = -1.0;
        self.texts.clear();
        self.particles.clear();
        self.rings.clear();
        self.zombies.clear();
        self.bullets.clear();
        self.gems.clear();
        self.pickups.clear();
        self.eshots.clear();
        self.banner = None;
    }

    /// Layout of the three level-up cards in CSS pixels: (x, y, w, h).
    pub fn card_rects(&self) -> [(f32, f32, f32, f32); 3] {
        // phones in portrait: three wide cards stacked vertically
        if self.screen.x < 600.0 {
            let cw = (self.screen.x - 28.0).min(440.0);
            let gap = 12.0;
            // fit between the title and the touch buttons at the bottom
            let ch = ((self.screen.y * 0.5 - 2.0 * gap) / 3.0).clamp(84.0, 118.0);
            let x = (self.screen.x - cw) * 0.5;
            let y0 = (self.screen.y * 0.22).max(150.0);
            return [
                (x, y0, cw, ch),
                (x, y0 + ch + gap, cw, ch),
                (x, y0 + 2.0 * (ch + gap), cw, ch),
            ];
        }
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

    pub(crate) fn open_levelup(&mut self) {
        self.roll_offers();
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

    fn roll_offers(&mut self) {
        let luck = (self.wave.n as f32 * 0.01).min(0.25);
        self.offers = self.ups.roll(&mut self.rng, luck, &self.banished);
    }

    /// Level-up buttons under the cards: (x, y, w, h) for Reroll, Banish, Skip.
    pub fn levelup_buttons(&self) -> [(f32, f32, f32, f32); 3] {
        let rects = self.card_rects();
        let last = rects[2];
        let first = rects[0];
        let phone = self.screen.x < 600.0;
        let y = if phone {
            last.1 + last.3 + 14.0
        } else {
            first.1 + first.3 + 22.0
        };
        let total_w = if phone { last.2 } else { (last.0 + last.2 - first.0).min(560.0) };
        let x0 = if phone { last.0 } else { self.screen.x * 0.5 - total_w * 0.5 };
        let gap = 10.0;
        let bw = (total_w - gap * 2.0) / 3.0;
        let bh = if phone { 40.0 } else { 36.0 };
        [
            (x0, y, bw, bh),
            (x0 + bw + gap, y, bw, bh),
            (x0 + 2.0 * (bw + gap), y, bw, bh),
        ]
    }

    pub(crate) fn reroll(&mut self) {
        if self.rerolls == 0 || self.phase != Phase::LevelUp {
            self.audio.play(Sfx::UiMove);
            return;
        }
        self.rerolls -= 1;
        self.banish_mode = false;
        self.roll_offers();
        self.sel = 0;
        self.levelup_t = 0.2;
        self.audio.play(Sfx::UiSelect);
    }

    pub(crate) fn toggle_banish(&mut self) {
        if self.banishes == 0 || self.phase != Phase::LevelUp {
            self.audio.play(Sfx::UiMove);
            return;
        }
        self.banish_mode = !self.banish_mode;
        self.audio.play(Sfx::UiMove);
    }

    pub(crate) fn skip_levelup(&mut self) {
        if self.skips == 0 || self.phase != Phase::LevelUp {
            self.audio.play(Sfx::UiMove);
            return;
        }
        self.skips -= 1;
        self.banish_mode = false;
        let heal = self.player.max_hp * 0.15;
        self.player.hp = (self.player.hp + heal).min(self.player.max_hp);
        let p = self.player.pos;
        self.text(p, format!("+{}", heal as i32), "#ff5c7a", 18.0);
        self.audio.play(Sfx::UiSelect);
        self.finish_pick();
    }

    pub(crate) fn choose(&mut self, i: usize) {
        if i >= self.offers.len() || self.phase != Phase::LevelUp {
            return;
        }
        let u = self.offers[i];
        if self.banish_mode {
            // gone for the rest of the run; deal a fresh hand
            self.banish_mode = false;
            self.banishes = self.banishes.saturating_sub(1);
            self.banished.push(u);
            self.roll_offers();
            self.sel = 0;
            self.levelup_t = 0.2;
            self.audio.play(Sfx::Hurt);
            if self.offers.is_empty() {
                self.finish_pick();
            }
            return;
        }
        self.apply_upgrade(u);
        self.audio.play(Sfx::UiSelect);
        self.finish_pick();
    }

    /// Gain one level of an upgrade, with its immediate side effects.
    pub fn apply_upgrade(&mut self, u: Up) {
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
    }

    /// Close this level-up (or open the next queued one).
    fn finish_pick(&mut self) {
        self.pending_levels = self.pending_levels.saturating_sub(1);
        if self.pending_levels > 0 {
            self.open_levelup();
            self.levelup_t = 0.2;
        } else {
            self.phase = Phase::Playing;
            self.last_ms = -1.0;
            // breathing room: shove nearby zombies away (host/solo own the zombies)
            let p = self.player.pos;
            let owns_zombies = !self.is_client();
            for z in self.zombies.iter_mut().filter(|_| owns_zombies) {
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

    pub(crate) fn try_ult(&mut self) {
        if self.hub.is_some() {
            return;
        }
        if self.player.ult < 1.0 {
            if self.player.ult < 0.999 {
                self.toast = Some((
                    format!("Ultimate charging… {}%", (self.player.ult * 100.0) as u32),
                    0.9,
                ));
            }
            return;
        }
        // Fire inside the next simulation step, once the spatial grid matches
        // the zombie list again (between steps the grid holds stale indices).
        self.ult_queued = true;
    }

    pub(crate) fn fire_ult(&mut self) {
        if self.player.ult < 1.0 {
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
        self.zoom_punch = 0.12;
        self.haptic(220, 1.0);
        self.player.iframes = self.player.iframes.max(1.0);
        self.audio.play(Sfx::Ultimate);
    }

    pub fn stats_json(&self) -> String {
        let name = self.player_name.replace(['\\', '"'], "");
        let diff = ["rookie", "survivor", "veteran", "nightmare"][self.diff];
        format!(
            "{{\"players\":{},\"name\":\"{}\",\"wave\":{},\"level\":{},\"kills\":{},\"time\":{:.1},\"combo\":{},\"coins\":{},\"mode\":\"{}\",\"chapter\":{},\"diff\":\"{}\",\"gun\":\"{}\",\"victory\":{},\"score\":{},\"map\":\"{}\",\"kk\":[{}],\"bosses\":{},\"char\":\"{}\",\"evos\":{},\"chests\":{},\"pure\":{},\"stars\":{},\"mutated\":{},\"daily\":\"{}\"}}",
            self.net.as_ref().map(|n| n.players()).unwrap_or(1),
            name,
            self.wave.n.max(1),
            self.player.level,
            self.kills,
            self.time,
            self.best_combo,
            self.coins,
            self.mode.id(),
            self.chapter(),
            diff,
            GUNS[self.gun].id,
            self.victory_t >= 0.0,
            self.score,
            self.map_id,
            self.kind_kills.iter().map(|k| k.to_string()).collect::<Vec<_>>().join(","),
            self.bosses_killed,
            content::CHARACTERS[self.character].id,
            self.evolved.len(),
            self.chests_opened,
            self.meta.is_pure() && self.relics == content::RelicBonus::default(),
            if self.victory_t >= 0.0 && self.siege.is_some() { self.siege_stars() } else { 0 },
            self.mutators.any(),
            self.daily
        )
    }

    /// Detailed snapshot for the pause screen's stats panel.
    pub fn run_json(&self) -> String {
        let evos = self.evolved.iter().map(|e| {
            let d = e.def();
            format!(
                "{{\"icon\":\"{}\",\"name\":\"{} (evolved)\",\"lvl\":1,\"max\":1,\"color\":\"{}\"}}",
                d.icon, d.name, d.color
            )
        });
        let ups: Vec<String> = evos.chain(crate::upgrades::ALL
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
            }))
            .collect();
        let shots_per_sec = 1.0 / self.fire_interval();
        format!(
            "{{\"run\":{},\"hp\":{:.0},\"maxHp\":{:.0},\"xp\":{:.0},\"xpNext\":{:.0},\"dmg\":{:.1},\"fireRate\":{:.2},\"speed\":{:.0},\"crit\":{:.0},\"magnet\":{:.0},\"ult\":{:.0},\"remaining\":{},\"gunName\":\"{}\",\"diffName\":\"{}\",\"char\":{{\"name\":\"{}\",\"class\":\"{}\",\"icon\":\"{}\",\"color\":\"{}\",\"ability\":\"{}\",\"passive\":\"{}\"}},\"upgrades\":[{}]}}",
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
            content::CHARACTERS[self.character].name,
            content::CHARACTERS[self.character].class,
            content::CHARACTERS[self.character].icon,
            content::CHARACTERS[self.character].color,
            content::CHARACTERS[self.character].ability,
            content::CHARACTERS[self.character].passive,
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
            "v={};name={};mode={};chapter={};diff={};gun={};perk={};savewave={};level={};xp={:.1};hp={:.1};maxhp={:.1};ups={};kills={};coins={};time={:.1};ult={:.3};combo={};best={};score={};charges={},{},{};ban={};evo={};char={}",
            SAVE_VERSION,
            self.player_name.replace([';', '=', '|'], ""),
            self.mode.id(),
            self.chapter(),
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
            self.best_wave,
            self.score,
            self.rerolls,
            self.banishes,
            self.skips,
            self.banished
                .iter()
                .map(|u| (*u as u8).to_string())
                .collect::<Vec<_>>()
                .join(","),
            self.evolved
                .iter()
                .map(|e| (*e as u8).to_string())
                .collect::<Vec<_>>()
                .join(","),
            content::CHARACTERS[self.character].id
        )
    }

    /// Version stamped into a save string (0 = pre-versioning).
    pub fn save_version(data: &str) -> u32 {
        data.split(';')
            .find_map(|kv| kv.strip_prefix("v="))
            .and_then(|v| v.parse().ok())
            .unwrap_or(0)
    }

    pub fn load_save(&mut self, data: &str) {
        self.start(data);
        if Game::save_version(data) > SAVE_VERSION {
            // best effort: unknown keys are skipped, but say why things may look off
            self.toast = Some(("Save is from a newer version. Refresh to update.".into(), 4.0));
        }
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
                "score" => self.score = v.parse().unwrap_or(0),
                "charges" => {
                    let c: Vec<u32> = v.split(',').filter_map(|x| x.parse().ok()).collect();
                    if c.len() == 3 {
                        (self.rerolls, self.banishes, self.skips) = (c[0], c[1], c[2]);
                    }
                }
                "evo" => {
                    self.evolved = v
                        .split(',')
                        .filter_map(|x| x.parse::<usize>().ok())
                        .filter_map(|i| EVOS.get(i).copied())
                        .collect()
                }
                "ban" => {
                    self.banished = v
                        .split(',')
                        .filter_map(|x| x.parse::<usize>().ok())
                        .filter_map(|i| crate::upgrades::ALL.get(i).copied())
                        .collect()
                }
                "ups" => {
                    for (i, l) in v.split(',').enumerate().take(self.ups.0.len()) {
                        self.ups.0[i] = l.parse().unwrap_or(0);
                    }
                }
                _ => {}
            }
        }
        if self.is_campaign() {
            wave = wave.min(CHAPTERS[self.chapter()].waves);
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
    pub(crate) fn step(&mut self) {
        self.time += DT;
        let coop = self.net.is_some();
        let client = self.is_client();
        let alive_play = self.phase == Phase::Playing
            || (coop && matches!(self.phase, Phase::LevelUp | Phase::Paused));

        self.grid.build(self.player.pos, &self.zombies);
        if alive_play && self.ult_queued {
            self.ult_queued = false;
            self.fire_ult();
            // the blast may have killed zombies: rebuild so indices stay valid
            self.zombies.retain(|z| !z.dead);
            self.grid.build(self.player.pos, &self.zombies);
        }
        if self.ob.is_some() && alive_play {
            if self.input.use_down {
                if let Some(o) = self.ob.as_mut() {
                    o.use_held = 0.1;
                }
            }
            self.ob_tick();
        }
        if alive_play && !self.player.dead {
            self.update_abilities();
            self.update_player();
            if self.player_alive() {
                self.update_weapons();
            }
        }
        if client {
            // replicas: only cosmetic timers tick locally; positions come from snapshots
            for z in self.zombies.iter_mut() {
                z.flash = (z.flash - DT * 7.0).max(0.0);
                z.blade_cd -= DT;
                z.anim += DT * 0.6;
            }
        } else {
            if self.ob.is_some() {
                self.update_flow();
                self.update_windows();
                self.update_traps();
            }
            if self.siege.is_some() {
                self.update_siege_flow();
            }
            self.update_zombies();
            if self.siege.is_some() {
                self.update_defenses();
            }
            self.update_status();
            self.update_elites();
            self.update_volatile();
            self.update_bosses();
        }
        self.grid.build(self.player.pos, &self.zombies);
        if !client {
            self.separate();
        }
        self.update_bullets();
        if !client {
            self.update_eshots();
        }
        if alive_play && !client {
            self.update_pickups();
        }
        self.update_effects();
        if alive_play && !client {
            self.mode.hooks().update_waves(self);
        }
        if coop && alive_play {
            self.update_revive();
        }

        if !client {
            self.resolve_nuke();
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
            if self.pending_chests > 0 && self.phase == Phase::Playing && self.player_alive() {
                self.pending_chests -= 1;
                self.open_chest();
            }
            if self.pending_levels > 0
                && self.phase == Phase::Playing
                && self.chest.is_none()
                && self.player_alive()
            {
                self.open_levelup();
            }
        }
    }

    pub(crate) fn move_dir(&self) -> V2 {
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

    pub(crate) fn update_player(&mut self) {
        let mdir = self.move_dir();
        let speed = self.move_speed() * if self.player.downed { 0.25 } else { 1.0 };
        if self.player.downed {
            self.player.dash_buf = 0.0;
        }
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
        // the camera leads the aim and recoils, so map the cursor through it
        let cam = if self.cam.dist2(self.player.pos) < 400.0 * 400.0 {
            self.cam
        } else {
            self.player.pos
        };
        self.mouse_world = cam + (self.input.mouse - self.screen * 0.5) * (1.0 / self.zoom.max(0.01));
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
    pub(crate) fn assisted_aim(&self, ppos: V2, raw: f32) -> f32 {
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

    pub(crate) fn update_weapons(&mut self) {
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
        let want_fire = want_fire && (self.hub.is_none() || self.in_range_zone());
        if want_fire && self.player.fire_cd <= 0.0 && self.ob_can_fire() {
            self.player.fire_cd += self.fire_interval();
            if self.player.fire_cd < 0.0 {
                self.player.fire_cd = 0.0;
            }
            self.fire();
            self.ob_consume_round();
        }
        if self.player.fire_cd < -0.2 {
            self.player.fire_cd = 0.0;
        }

        // ---- orbiting blades
        let nb = self.blade_count();
        if nb > 0 {
            self.player.blade_angle += DT * (3.0 + 0.15 * nb as f32);
            let br = self.blade_radius();
            let reap = if self.has_evo(Evo::ReaperRing) { 2.0 } else { 1.0 };
            let dmg = 13.0 * (1.0 + 0.2 * (self.ups.f(Up::Blades) - 1.0)) * self.dmg_mult() * 0.75 * reap;
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
                    let z = &self.zombies[j];
                    if !z.dead && z.pos.dist(ppos) < r + z.r {
                        let dir = (z.pos - ppos).norm();
                        let power = self.elem_power();
                        self.afflict(j, Elem::Chill, power);
                        if self.has_evo(Evo::AbsoluteZero) && !self.zombies[j].dead {
                            self.afflict(j, Elem::Chill, power);
                        }
                        if !self.zombies[j].dead {
                            self.damage_zombie(j, dmg, dir, 260.0, false);
                        }
                    }
                }
                self.ring(ppos, r, 0.5, "#a8f4ff", 10.0);
                self.light(ppos, r * 1.6, 0.4, "#a8f4ff");
                self.burst(ppos, 30, r * 2.2, "#d8fbff", 3.0, 0.45, false, true);
                self.sfx(Sfx::Nova, ppos);
            }
        }

        // ---- chain lightning
        let ll = self.ups.get(Up::Lightning);
        if ll > 0 {
            self.player.bolt_t -= DT;
            if self.player.bolt_t <= 0.0 {
                let crown = self.has_evo(Evo::StormCrown);
                self.player.bolt_t = (2.3 - 0.22 * ll as f32).max(0.8) * if crown { 0.5 } else { 1.0 };
                let chains = 2 + ll as usize + if crown { 3 } else { 0 };
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
                        let power = self.elem_power();
                        self.afflict(cur, Elem::Shock, power);
                        if !self.zombies[cur].dead {
                            let crit = crown && self.rng.chance(self.crit_chance());
                            let d = if crit { dmg * 2.5 } else { dmg };
                            self.damage_zombie(cur, d, dir, 120.0, crit);
                        }
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

    pub(crate) fn fire(&mut self) {
        let gun = GUNS[self.gun];
        let ppos = self.player.pos;
        let aim = self.player.aim;
        let hail = self.has_evo(Evo::Hailstorm);
        let extra = self.ups.get(Up::Multishot) as i32 + if hail { 2 } else { 0 };
        let speed = 920.0 * gun.speed * (1.0 + 0.2 * self.ups.f(Up::Velocity));
        let life = 0.62 * gun.life * (1.0 + 0.15 * self.ups.f(Up::Velocity));
        let upgraded = self.ob.as_ref().map(|o| o.weapon().upgraded).unwrap_or(false);
        let dmg = 10.0 * gun.dmg * self.dmg_mult() * self.signature_mult() * if upgraded { 2.2 } else { 1.0 };
        let pierce = self.ups.get(Up::Pierce) as i32 + gun.pierce + if hail { 4 } else { 0 };
        let heat_sink = self.kit() == content::Kit::HeatSink && self.ability_t > 0.0;
        let bounces = self.ups.get(Up::Ricochet) as i32 + if heat_sink { 2 } else { 0 };
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
                cosmetic: false,
                tracer: g.tracer as u8,
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
        self.player.shots = self.player.shots.wrapping_add(1);
        let muzzle = ppos + V2::from_angle(aim) * (gun.len + 4.0);
        self.cam_kick -= V2::from_angle(aim) * gun.kick;
        self.light(muzzle, gun.flash, 0.06, "#ffd890");
        if !gun.casing.is_empty() && self.particles.len() < 1800 {
            // brass flies out the right side of the gun and bounces
            let side = V2::from_angle(aim).perp();
            let l = self.rng.range(0.7, 1.1);
            self.particles.push(Particle {
                pos: ppos + V2::from_angle(aim) * (gun.len * 0.4),
                vel: side * self.rng.range(90.0, 170.0) - V2::from_angle(aim) * 30.0
                    + self.player.vel * 0.5,
                z: 14.0,
                vz: self.rng.range(120.0, 220.0),
                life: l,
                max: l,
                size: if gun.pellets > 1 { 2.6 } else { 1.8 },
                color: gun.casing,
                grav: true,
                glow: false,
                drag: 1.2,
            });
        }
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
        if self.is_client() {
            return self.client_hit(j, dmg, dir, knock, crit);
        }
        let hit_slot = self.hit_slot;
        let dmg = dmg * self.status_damage_mult(j);
        let dmg = self.shield_absorb(j, dmg);
        let dmg = self.armor_mult(j, dir, crit) * dmg;
        let weak = !crit && self.weak_point(j, dir);
        let (dmg, crit) = if weak { (dmg * 2.0, true) } else { (dmg, crit) };
        let dmg = if self.insta_kill() && self.zombies[j].kind != ZKind::Boss {
            dmg.max(self.zombies[j].hp + self.zombies[j].shield + 1.0)
        } else {
            dmg
        };
        let z = &mut self.zombies[j];
        if z.dead {
            return false;
        }
        z.last_hitter = hit_slot;
        let dummy = z.dummy;
        z.hp -= dmg;
        z.flash = 1.0;
        z.vel += dir * knock * z.kind.def().knock * if self.mutators.floaty { 2.5 } else { 1.0 };
        let pos = z.pos;
        let dead = z.hp <= 0.0;
        if dead && crit && hit_slot == self.my_slot() {
            self.headshot(pos);
        }
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
        if dummy {
            self.log_dummy_damage(dmg);
        }
        if hit_slot == self.my_slot() {
            self.ob_on_hit(pos, dead, crit);
        }
        if dead {
            self.kill_zombie(j);
        }
        dead
    }

    pub(crate) fn kill_zombie(&mut self, j: usize) {
        let shocked = self.zombies[j].st.shock_t > 0.0;
        let affix = self.zombies[j].affix;
        let (pos, kind, xp, r, fuse_done, uid, killer) = {
            let z = &mut self.zombies[j];
            z.dead = true;
            (
                z.pos,
                z.kind,
                z.xp,
                z.r,
                z.fuse < -0.5,
                z.uid,
                z.last_hitter,
            )
        };
        // a fuse that went off on its own isn't anyone's kill
        if !fuse_done && killer == self.my_slot() {
            self.credit_kill();
        }
        if let Some(net) = &mut self.net {
            let killer = if fuse_done { 255 } else { killer };
            net.event(|w| {
                w.u8(net::E_KILL);
                w.u32(uid);
                w.v2(pos);
                w.u8(kind as u8);
                w.u8(killer);
            });
        }
        self.note_kill(if fuse_done { 255 } else { killer }, kind, pos);
        if shocked {
            self.shock_discharge(pos, uid);
        }
        if affix != crate::elites::Affix::None {
            self.elite_death(pos, kind, affix, r);
        }
        self.add_corpse(j);
        self.kill_effects(pos, kind, r);
        let big = kind.def().big;
        self.drop_loot(pos, kind, xp, r, big);
        self.kill_aftermath(j, pos, kind, fuse_done);
    }

    /// Gore, decal and death sound (also used for client-side predicted kills).
    pub fn kill_effects(&mut self, pos: V2, kind: ZKind, r: f32) {
        let def = kind.def();
        let skin = def.gore;
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
            color: def.decal,
            seed,
        });
        match kind {
            ZKind::Boss => {
                self.sfx(Sfx::BigDie, pos);
                self.sfx(Sfx::Explosion, pos);
                self.ring(pos, 260.0, 0.6, "#ffb0ff", 12.0);
                self.burst(pos, 60, 700.0, "#ff6aff", 4.0, 0.8, false, true);
            }
            ZKind::Brute => self.sfx(Sfx::BigDie, pos),
            ZKind::Exploder => {}
            _ => self.sfx(Sfx::ZombieDie, pos),
        }
    }

    pub(crate) fn drop_loot(&mut self, pos: V2, kind: ZKind, xp: u32, r: f32, big: bool) {
        if self.ob.is_some() {
            self.ob_drop(pos);
            if kind == ZKind::Boss {
                self.pickups.push(Pickup { pos, kind: PickKind::MaxAmmo, t: 0.0, dead: false });
            }
            return;
        }
        self.siege_drop(pos, xp);
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
        let chest = kind == ZKind::Boss || (kind == ZKind::Brute && self.rng.chance(0.08));
        if chest {
            self.pickups.push(Pickup {
                pos: pos + v2(-30.0, 10.0),
                kind: PickKind::Chest,
                t: 0.0,
                dead: false,
            });
        }
    }

    pub(crate) fn kill_aftermath(&mut self, j: usize, pos: V2, kind: ZKind, fuse_done: bool) {
        match kind {
            ZKind::Boss => {
                self.add_shake(0.8);
                self.hitstop = 0.12;
                self.slowmo = 1.4;
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
            }
            ZKind::Brute => {
                self.add_shake(0.25);
                self.hitstop = self.hitstop.max(0.035);
            }
            ZKind::Exploder => {
                self.explode(pos, 115.0, 45.0 * hp_mult(self.wave.n).sqrt(), fuse_done);
            }
            _ => {}
        }
        self.mode.hooks().on_kill(self, kind, pos);
    }

    pub(crate) fn spawn_gem(&mut self, pos: V2, value: u32) {
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
            mana: false,
        });
    }

    /// `hurts_player`: true for exploder fuses that went off; player-caused kills are safe.
    pub(crate) fn explode(&mut self, pos: V2, radius: f32, dmg: f32, hurts_player: bool) {
        let mut hits = Vec::new();
        self.grid.query(pos, radius + 50.0, |j| hits.push(j));
        for j in hits {
            let z = &self.zombies[j];
            if !z.dead && z.pos.dist(pos) < radius + z.r {
                let dir = (z.pos - pos).norm();
                self.damage_zombie(j, dmg * 1.5, dir, 420.0, false);
            }
        }
        if hurts_player {
            self.hurt_players_in(pos, radius, dmg * 0.6);
        }
        if let Some(net) = &mut self.net {
            net.event(|w| {
                w.u8(net::E_BLAST);
                w.v2(pos);
                w.f32(radius);
            });
        }
        self.ring(pos, radius, 0.35, "#ffb24a", 8.0);
        self.light(pos, radius * 2.4, 0.35, "#ffb060");
        self.burst(pos, 26, 520.0, "#ffb24a", 4.0, 0.45, false, true);
        self.burst(pos, 14, 300.0, "#5a5a5a", 6.0, 0.9, true, false);
        self.sfx(Sfx::Explosion, pos);
        let d = self.player.pos.dist(pos);
        self.add_shake(if d < 400.0 { 0.35 } else { 0.1 });
    }

    pub fn hurt_player(&mut self, dmg: f32, from: V2) {
        let coop_live = self.net.is_some() && matches!(self.phase, Phase::LevelUp | Phase::Paused);
        if self.phase != Phase::Playing && !coop_live {
            return;
        }
        if self.victory_t >= 0.0 {
            return;
        }
        if self.player.iframes > 0.0 || self.player.dash_t > 0.0 || self.player.downed || self.player.dead {
            return;
        }
        let dmg = self.shield_block(dmg, from);
        let p = &mut self.player;
        p.hp -= if self.mutators.onehit { p.hp + 1.0 } else { dmg };
        p.iframes = 0.65;
        p.hurt_flash = 1.0;
        let away = (p.pos - from).norm();
        p.vel += away * 340.0;
        let pos = p.pos;
        self.add_shake(0.45);
        self.hitstop = self.hitstop.max(0.05);
        self.burst(pos, 14, 260.0, "#c0101c", 3.0, 0.6, true, false);
        self.sfx(Sfx::Hurt, pos);
        self.haptic(70, 0.6);
        self.combo = 0;
        if self.player.hp <= 0.0 && self.net.is_some() {
            self.go_down();
        } else if self.player.hp <= 0.0 && self.second_wind() {
            // got back up
        } else if self.player.hp <= 0.0 {
            self.player.hp = 0.0;
            self.phase = Phase::Dying;
            self.dying_t = 0.0;
            self.audio.play(Sfx::GameOver);
            self.burst(pos, 70, 520.0, "#b0101a", 4.0, 1.4, true, false);
            self.add_shake(1.0);
            self.hitstop = 0.15;
        }
    }

    pub(crate) fn update_zombies(&mut self) {
        let targets = self.targets();
        let wave_n = self.scale_n();
        let mut contacts: Vec<(usize, u8)> = Vec::new();
        let mut fuses: Vec<usize> = Vec::new();
        let mut boss_summons: Vec<(V2, ZKind, u32)> = Vec::new();
        let mut blasts: Vec<V2> = Vec::new();
        let mut screams: Vec<V2> = Vec::new();
        let siege_on = self.siege.is_some();
        let siege_defs: Vec<V2> = self
            .siege
            .as_ref()
            .map(|s| s.defenses.iter().map(|d| d.pos).collect())
            .unwrap_or_default();
        let mut volleys: Vec<(V2, f32)> = Vec::new();
        let mut eruptions: Vec<(V2, f32)> = Vec::new();
        let relocate_r = self.spawn_r * 1.7;
        let playing = self.phase == Phase::Playing
            || (self.net.is_some() && matches!(self.phase, Phase::LevelUp | Phase::Paused));
        for (i, z) in self.zombies.iter_mut().enumerate() {
            if z.dead {
                continue;
            }
            if z.win >= 0 || z.dummy {
                continue; // breaking in through a window (outbreak.rs) / range dummy (hub.rs)
            }
            let (tslot, ppos, pvel) = Game::nearest_target(&targets, z.pos);
            z.prev = z.pos;
            z.flash = (z.flash - DT * 7.0).max(0.0);
            z.attack_cd -= DT;
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

            z.rage_t -= DT;
            let mut spd = z.speed * z.st.speed_mult() * if z.rage_t > 0.0 { 1.35 } else { 1.0 };
            let mut want = if playing {
                dir
            } else {
                V2::from_angle(z.face + 0.5)
            };
            let mut accel = 7.0;
            let aggro = if siege_on { 220.0 } else { 90.0 };
            if let Some(flow) = &self.flow {
                if dist > aggro && z.kind != ZKind::Bat && playing {
                    if let Some(fd) = flow.dir(z.pos) {
                        want = fd;
                    }
                }
            }
            if siege_on && playing {
                match z.kind {
                    ZKind::Sapper => {
                        if let Some(dp) = siege_defs
                            .iter()
                            .filter(|d| d.dist(z.pos) < 700.0)
                            .min_by(|a, b| a.dist2(z.pos).partial_cmp(&b.dist2(z.pos)).unwrap_or(std::cmp::Ordering::Equal))
                        {
                            want = (*dp - z.pos).norm();
                        }
                    }
                    ZKind::Archer => {
                        let near = siege_defs.iter().any(|d| d.dist(z.pos) < 380.0) || dist < 380.0;
                        if near {
                            spd *= 0.15;
                            z.timer -= DT;
                            if z.timer <= 0.0 {
                                z.timer = 1.8 + self.rng.range(0.0, 0.6);
                                volleys.push((z.pos, z.dmg));
                            }
                        }
                    }
                    _ => {}
                }
            }

            match z.kind {
                ZKind::Brute if wave_n >= 12 => {
                    z.special_t -= DT;
                    if z.state == 0 && z.special_t <= 0.0 && (220.0..520.0).contains(&dist) && playing {
                        z.state = 1;
                        z.timer = 0.65;
                        z.charge_dir = dir;
                    }
                    if z.state == 1 {
                        spd = 0.0;
                        z.timer -= DT;
                        if z.timer <= 0.0 {
                            z.state = 0;
                            z.special_t = 5.0 + self.rng.range(0.0, 2.0);
                            let lead = ppos + pvel * (dist / 260.0) * 0.5;
                            let sdir = (lead - z.pos).norm();
                            self.eshots.push(EShot {
                                pos: z.pos + sdir * z.r,
                                prev: z.pos,
                                vel: sdir * 260.0,
                                life: 2.6,
                                dmg: z.dmg * 0.8,
                                dead: false,
                                turn: 0.0,
                                r: 13.0,
                                grazed: false,
                            });
                            self.pending_sfx.push((Sfx::Dash, z.pos));
                        }
                    }
                }
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
                        let aim = (lead - z.pos).angle();
                        let spread: &[f32] = if wave_n >= 15 { &[-0.22, 0.0, 0.22] } else { &[0.0] };
                        for off in spread {
                            let sdir = V2::from_angle(aim + off);
                            self.eshots.push(EShot {
                                pos: z.pos + sdir * z.r,
                                prev: z.pos,
                                vel: sdir * 340.0,
                                life: 2.2,
                                dmg: z.dmg,
                                dead: false,
                                turn: 0.0,
                                r: 6.5,
                                grazed: false,
                            });
                        }
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
                ZKind::Crawler => {
                    // scuttles in a zigzag
                    let wob = (z.anim * 0.9 + z.tint * 9.0).sin() * 0.7;
                    want = (want + want.perp() * wob).norm();
                }
                ZKind::Screamer => {
                    if dist < 280.0 {
                        want = -dir;
                    } else if dist < 380.0 {
                        want = (dir.perp() * z.strafe).norm();
                        spd *= 0.5;
                    }
                    z.timer -= DT;
                    if z.timer <= 0.0 && playing && dist < 700.0 {
                        z.timer = 4.5;
                        screams.push(z.pos);
                    }
                }
                ZKind::Bat => {
                    // fluttering approach that overshoots and circles back
                    let wob = (z.anim * 1.7 + z.tint * 11.0).sin();
                    want = (want + want.perp() * (wob * 1.1)).norm();
                    accel = 4.0;
                }
                ZKind::Burrower => {
                    match z.state {
                        0 => {
                            // tunnelling: surface when close
                            spd *= 1.3;
                            if dist < 120.0 && playing {
                                z.state = 1;
                                z.timer = 0.8;
                            }
                        }
                        1 => {
                            spd = 0.0;
                            z.timer -= DT;
                            if z.timer <= 0.0 {
                                z.state = 2;
                                z.hidden = false;
                                eruptions.push((z.pos, z.dmg));
                            }
                        }
                        _ => {}
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
                                        turn: 0.0,
                                        r: 6.5,
                                        grazed: false,
                                    });
                                }
                            };
                        match var {
                            BossVar::Abomination => z.special_t = 99.0,
                            // signature moves live in boss.rs
                            BossVar::Butcher | BossVar::HiveQueen => {}
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
                    if matches!(var, BossVar::Matriarch | BossVar::HiveQueen) {
                        // keeps her distance and strafes
                        if dist < 300.0 {
                            want = -dir;
                        } else if dist < 460.0 {
                            want = (dir.perp() * z.strafe + dir * 0.1).norm();
                            spd *= 0.7;
                        }
                    }
                    let charges = match var {
                        BossVar::Matriarch | BossVar::HiveQueen => false,
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
                        3 => spd = 0.0,
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

            if (z.kind != ZKind::Boss || z.state == 0) && z.kind != ZKind::Bat && !z.hidden {
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
            if dist < rr && playing && tslot != 255 && !z.hidden {
                if z.attack_cd <= 0.0 && !z.st.frozen() {
                    contacts.push((i, tslot));
                    z.attack_cd = 0.8;
                }
                if dist > 0.01 {
                    z.pos = ppos - dir * rr;
                }
            }
        }

        for (i, slot) in contacts {
            let (dmg, zp) = (self.zombies[i].dmg, self.zombies[i].pos);
            if slot != self.my_slot() {
                self.hurt_slot(slot, dmg, zp);
                continue;
            }
            let was_iframe = self.player.iframes > 0.0 || self.player.dash_t > 0.0;
            self.hurt_player(dmg, zp);
            if !was_iframe {
                self.elite_bite(i, dmg);
            }
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
        for (at, dmg) in volleys {
            if self.archer_volley(at, dmg) {
                self.sfx(Sfx::Blade, at);
            }
        }
        for at in screams {
            let mut n = 0;
            for z in self.zombies.iter_mut() {
                if !z.dead && z.kind != ZKind::Screamer && z.pos.dist2(at) < 240.0 * 240.0 {
                    z.rage_t = 3.5;
                    n += 1;
                }
            }
            self.ring(at, 240.0, 0.5, "#ff6ab0", 5.0);
            self.sfx(Sfx::BossRoar, at);
            if n > 0 {
                self.text(at + v2(0.0, -30.0), "SCREECH!".into(), "#ff8ac0", 13.0);
            }
        }
        for (at, dmg) in eruptions {
            self.ring(at, 90.0, 0.35, "#9a7a4a", 8.0);
            self.burst(at, 24, 300.0, "#6a4a2a", 5.0, 0.8, true, false);
            self.sfx(Sfx::Explosion, at);
            self.add_shake(0.25);
            self.hurt_players_in(at, 80.0, dmg);
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
    pub(crate) fn separate(&mut self) {
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
            if z.kind == ZKind::Bat {
                continue;
            }
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

    pub(crate) fn update_bullets(&mut self) {
        let mut bullets = std::mem::take(&mut self.bullets);
        let homing = self.ups.get(Up::Homing) + if self.has_evo(Evo::SmartMissiles) { 3 } else { 0 };
        let missiles = self.has_evo(Evo::SmartMissiles);
        let plague = self.has_evo(Evo::PlagueCarrier);
        let expl = self.ups.get(Up::Explosive);
        let venom = self.ups.get(Up::Venom);
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
            if b.cosmetic {
                continue;
            }

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
                if venom > 0 && (plague || self.rng.chance(0.22 + 0.13 * venom as f32)) {
                    let power = self.elem_power();
                    self.afflict(j, Elem::Poison, power);
                    if plague && b.bounces < self.ups.get(Up::Ricochet) as i32 && !self.zombies[j].dead {
                        // a bounced round: spread two more stacks
                        self.afflict(j, Elem::Poison, power);
                        if !self.zombies[j].dead {
                            self.afflict(j, Elem::Poison, power);
                        }
                    }
                }
                if self.zombies[j].dead {
                    b.dead = true;
                    continue;
                }
                self.damage_zombie(j, b.dmg, dir, b.knock, b.crit);
                if !self.zombies[j].dead {
                    let from = self.player.pos;
                    self.reflect_shot(j, from);
                    self.pyro_ignite(j);
                }
                b.last_hit = uid;
                // impact sparks
                let col = if b.crit { "#ffd34a" } else { "#fff1c0" };
                self.burst(b.pos, 3, 200.0, col, 2.0, 0.15, false, true);
                if b.blast > 0.0 {
                    let r = b.blast * (1.0 + 0.3 * expl as f32);
                    self.small_blast(b.pos, r, b.dmg * 0.7, uid);
                    self.add_shake(0.08);
                } else if missiles && expl == 0 {
                    self.small_blast(b.pos, 40.0, b.dmg * 0.35, uid);
                } else if expl > 0 {
                    let r = 55.0 * (1.0 + 0.3 * (expl as f32 - 1.0)) * if self.has_evo(Evo::Napalm) { 1.5 } else { 1.0 };
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

    pub(crate) fn small_blast(&mut self, pos: V2, r: f32, dmg: f32, skip: u32) {
        let mut hits = Vec::new();
        self.grid.query(pos, r + 50.0, |j| hits.push(j));
        let power = self.elem_power() * if self.has_evo(Evo::Napalm) { 2.0 } else { 1.0 };
        for j in hits {
            let z = &self.zombies[j];
            if !z.dead && z.uid != skip && z.pos.dist(pos) < r + z.r {
                let dir = (z.pos - pos).norm();
                self.afflict(j, Elem::Burn, power);
                if !self.zombies[j].dead {
                    self.damage_zombie(j, dmg, dir, 160.0, false);
                }
            }
        }
        self.ring(pos, r, 0.22, "#ff9a3a", 4.0);
        self.light(pos, r * 2.2, 0.18, "#ffa050");
        self.burst(pos, 6, 260.0, "#ffa040", 3.0, 0.25, false, true);
        self.sfx(Sfx::Explosion, pos);
        self.add_shake(0.04);
    }

    pub(crate) fn update_eshots(&mut self) {
        let targets = self.targets();
        let mut hits = Vec::new();
        for (i, s) in self.eshots.iter_mut().enumerate() {
            if s.turn > 0.0 {
                let (_, tp, _) = Game::nearest_target(&targets, s.pos);
                let sp = s.vel.len();
                let want = (tp - s.pos).norm() * sp;
                s.vel = s.vel.lerp(want, damp(s.turn, DT)).norm() * sp;
            }
            s.prev = s.pos;
            s.pos += s.vel * DT;
            s.life -= DT;
            if s.life <= 0.0 {
                s.dead = true;
            } else if let Some(t) = targets
                .iter()
                .find(|t| t.0 != 255 && s.pos.dist(t.1) < PR + s.r * 0.6 + 3.0)
            {
                s.dead = true;
                hits.push((i, t.0));
            }
        }
        // near misses on the local player: reward the dodge
        let me = self.player.pos;
        let mut grazes = 0;
        if self.player_alive() {
            for s in self.eshots.iter_mut() {
                if !s.dead && !s.grazed && s.pos.dist(me) < PR + s.r + 26.0 {
                    s.grazed = true;
                    grazes += 1;
                }
            }
        }
        for _ in 0..grazes {
            self.graze();
        }
        for (i, slot) in hits {
            let (d, p) = (self.eshots[i].dmg, self.eshots[i].pos);
            self.hurt_slot(slot, d, p);
            self.burst(p, 10, 200.0, "#9be04a", 3.0, 0.4, true, false);
        }
        self.eshots.retain(|s| !s.dead);
    }

    pub(crate) fn update_pickups(&mut self) {
        let targets = self.targets();
        let mr = self.magnet_r();
        let mut gained = 0.0;
        let mut picked = 0;
        let mut mana: Vec<(u8, u32)> = Vec::new();
        for g in self.gems.iter_mut() {
            g.prev = g.pos;
            g.t += DT;
            let (gslot, ppos, _) = Game::nearest_target(&targets, g.pos);
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
                if g.mana {
                    mana.push((gslot, g.value));
                } else {
                    gained += g.value as f32;
                    picked += 1;
                }
            }
        }
        // mana is per player: the collector gets it
        for (slot, v) in mana {
            if slot == self.my_slot() {
                self.gain_mana(v);
            } else if let Some(net) = &mut self.net {
                net.event(|w| {
                    w.u8(net::E_MANA);
                    w.u8(slot);
                    w.u32(v);
                });
            }
        }
        let ppos = self.player.pos;
        if picked > 0 {
            // team xp: everybody levels from every gem
            self.player.xp += gained;
            self.sfx(Sfx::Pickup, ppos);
            if let Some(net) = &mut self.net {
                net.event(|w| {
                    w.u8(net::E_XP);
                    w.f32(gained);
                });
            }
        }

        let mut collected = Vec::new();
        for (i, pk) in self.pickups.iter_mut().enumerate() {
            pk.t += DT;
            let (tslot, tpos, _) = Game::nearest_target(&targets, pk.pos);
            let d = tpos - pk.pos;
            let l = d.len();
            if l < 60.0 {
                pk.pos += d.norm() * 300.0 * DT;
            }
            if l < PR + 14.0 && tslot != 255 {
                pk.dead = true;
                collected.push((i, tslot));
            }
            if pk.t > 30.0 && pk.kind != PickKind::Chest {
                pk.dead = true;
            }
        }
        for (i, slot) in collected {
            let k = self.pickups[i].kind;
            if k.is_powerup() {
                self.apply_powerup(k, true);
                continue;
            }
            if slot != self.my_slot() {
                // a teammate grabbed it
                let heal = 30.0 + self.player.max_hp * 0.1;
                if let Some(net) = &mut self.net {
                    match k {
                        PickKind::Heart => net.event(|w| {
                            w.u8(net::E_HEAL);
                            w.u8(slot);
                            w.f32(heal);
                        }),
                        PickKind::Frenzy => net.event(|w| {
                            w.u8(net::E_FRENZY);
                            w.u8(slot);
                        }),
                        PickKind::Chest => net.event(|w| {
                            w.u8(net::E_CHEST);
                            w.u8(slot);
                        }),
                        _ => {}
                    }
                }
                if k == PickKind::Vacuum {
                    for g in self.gems.iter_mut() {
                        g.magnet = true;
                    }
                }
                continue;
            }
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
                PickKind::Chest => {
                    self.burst(ppos, 30, 320.0, "#ffd36a", 3.5, 0.7, false, true);
                    self.pending_chests += 1;
                }
                PickKind::Frenzy => {
                    self.player.frenzy = 8.0;
                    self.toast = Some(("FRENZY! Double fire rate".into(), 1.6));
                    self.burst(ppos, 20, 300.0, "#ffd84a", 3.0, 0.5, false, true);
                    self.sfx(Sfx::UltReady, ppos);
                }
                _ => {}
            }
        }
    }

    pub(crate) fn update_effects(&mut self) {
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





    pub(crate) fn pick_kind(&mut self) -> ZKind {
        let n = self.scale_n() as f32;
        let weight = |k: ZKind| {
            let d = k.def();
            if d.min_wave == 0 || n < d.min_wave as f32 {
                0.0
            } else {
                (d.weight.0 + d.weight.1 * n).min(d.weight.2)
            }
        };
        let total: f32 = ZKind::ALL.iter().map(|k| weight(*k)).sum();
        let mut r = self.rng.f() * total;
        for k in ZKind::ALL {
            r -= weight(k);
            if r <= 0.0 && weight(k) > 0.0 {
                return k;
            }
        }
        ZKind::Walker
    }

    pub(crate) fn spawn_zombie(&mut self, kind: ZKind, pos: V2) {
        let def = kind.def();
        let (hp, speed, r, dmg, xp) = (def.hp, def.speed, def.r, def.dmg, def.xp);
        let n = self.scale_n();
        let df = self.difficulty();
        let (dhp, dspeed, ddmg) = (df.hp, df.speed, df.dmg);
        let hm = hp_mult(n)
            * if kind == ZKind::Boss {
                1.0 + (n / 5) as f32 * 0.35
            } else {
                1.0
            };
        let sm = (1.0 + (0.018 * (n - 1) as f32).min(0.35)) * if self.mutators.fast { 1.3 } else { 1.0 };
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
            st: crate::status::Status::default(),
            affix: crate::elites::Affix::None,
            shield: 0.0,
            rage_t: 0.0,
            win: -1,
            dummy: false,
            phase: 0,
            age: 0.0,
            hidden: kind == ZKind::Burrower,
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
            last_hitter: 0,
        });
    }


}

#[cfg(test)]
mod tests {
    use super::*;

    /// Regression: the ultimate used to run between simulation steps with a
    /// spatial grid built before dead zombies were removed, so after bullets
    /// killed zombies its indices could run past the end of `zombies` (panic,
    /// seen in the browser as `RuntimeError: unreachable`).
    #[test]
    pub(crate) fn ultimate_while_bullets_are_killing_never_uses_stale_grid() {
        for seed in 1..=20u64 {
            let mut g = Game::new(seed);
            g.phase = Phase::Playing;
            g.wave.intermission = 0.01;
            g.screen = v2(1280.0, 720.0);
            g.zoom = 1.2;
            g.spawn_r = 700.0;
            g.auto_aim = true;
            g.ups.0[Up::Multishot.idx()] = 5;
            g.ups.0[Up::Pierce.idx()] = 5;
            g.ups.0[Up::FireRate.idx()] = 8;
            let mut t = 0.0;
            for frame in 0..2000 {
                g.player.hp = 1.0e9;
                g.player.max_hp = 1.0e9;
                if frame % 9 == 0 {
                    g.player.ult = 1.0;
                    g.input.actions.push(Action::Ult);
                }
                if g.phase == Phase::LevelUp {
                    g.input.actions.push(Action::Pick1);
                    g.levelup_t = 1.0;
                }
                // a steady horde: some close (bullet kills), some far (survivors)
                for k in 0..4 {
                    let a = (frame * 4 + k) as f32 * 0.77;
                    let r = if k % 2 == 0 { 120.0 } else { 900.0 };
                    let p = g.player.pos + V2::from_angle(a) * r;
                    g.spawn_zombie(ZKind::Walker, p);
                }
                t += 16.7;
                g.frame(t);
            }
            assert!(
                g.kills > 500,
                "seed {seed}: expected lots of kills, got {}",
                g.kills
            );
        }
    }

    #[test]
    pub(crate) fn enemy_table_drives_spawns() {
        let mut g = Game::new(7);
        g.phase = Phase::Playing;
        // wave 1 only spawns walkers; later waves unlock kinds by `min_wave`
        g.wave.n = 1;
        for _ in 0..200 {
            assert_eq!(g.pick_kind(), ZKind::Walker);
        }
        g.wave.n = 12;
        let mut seen = [false; ZKind::ALL.len()];
        for _ in 0..8000 {
            seen[g.pick_kind() as usize] = true;
        }
        for k in ZKind::ALL {
            // bosses and hounds only come from their own events
            if k.def().min_wave > 0 {
                assert!(seen[k as usize], "{:?} spawns by wave 12", k);
            } else {
                assert!(!seen[k as usize], "{:?} never comes from the pool", k);
            }
        }
        // stats come from the table
        for k in ZKind::ALL {
            g.zombies.clear();
            g.spawn_zombie(k, v2(500.0, 0.0));
            assert_eq!(g.zombies[0].r, k.def().r);
            assert_eq!(ZKind::from_u8(k as u8), k);
        }
    }

    #[test]
    fn modes_parse_and_dispatch() {
        let mut g = Game::new(3);
        g.start("mode=campaign;chapter=2;diff=rookie");
        assert_eq!(g.mode, Mode::Campaign(2));
        g.start("chapter=3;mode=campaign");
        assert_eq!(g.mode, Mode::Campaign(3));
        assert_eq!(g.chapter(), 3);
        g.wave.n = 4;
        assert_eq!(g.mode.hooks().hud(&g), format!("WAVE 4/{}", CHAPTERS[3].waves));
        g.start("diff=rookie");
        assert_eq!(g.mode, Mode::Infinite);
        assert_eq!(g.mode.hooks().hud(&g), "WAVE 1");
        // campaign clears on the final boss
        g.start("mode=campaign;chapter=0");
        let last = CHAPTERS[0].waves;
        g.wave.n = last;
        g.wave.boss_pending = false;
        g.spawn_boss(CHAPTERS[0].boss, v2(300.0, 0.0));
        g.grid.build(g.player.pos, &g.zombies);
        g.damage_zombie(0, 1.0e9, v2(1.0, 0.0), 0.0, false);
        assert!(g.victory_t >= 0.0, "final boss kill wins the chapter");
    }

    #[test]
    fn save_strings_are_versioned_and_roundtrip() {
        let mut g = Game::new(5);
        g.start("mode=campaign;chapter=1;diff=veteran;gun=smg");
        g.wave.n = 3;
        g.player.level = 4;
        g.kills = 77;
        let s = g.save_string();
        assert_eq!(Game::save_version(&s), SAVE_VERSION);
        assert_eq!(Game::save_version("name=old;savewave=2"), 0);
        let mut h = Game::new(6);
        h.load_save(&s);
        assert_eq!(h.mode, Mode::Campaign(1));
        assert_eq!(h.player.level, 4);
        assert_eq!(h.kills, 77);
        assert_eq!(h.diff, 2);
    }

    #[test]
    fn firing_kicks_camera_and_ejects_brass() {
        let mut g = Game::new(9);
        g.start("gun=shotgun");
        g.player.aim = 0.0;
        g.particles.clear();
        g.fire();
        assert!(g.cam_kick.x < 0.0, "camera recoils against the aim");
        assert_eq!(g.lights.len(), 1);
        assert!(g.particles.iter().any(|p| p.color == GUNS[g.gun].casing));
    }

    #[test]
    fn boss_kill_slows_time_then_recovers() {
        let mut g = Game::new(11);
        g.start("diff=rookie");
        g.spawn_boss(BossVar::Gravedigger, v2(300.0, 0.0));
        g.grid.build(g.player.pos, &g.zombies);
        g.damage_zombie(0, 1.0e9, v2(1.0, 0.0), 0.0, false);
        assert!(g.slowmo > 1.0);
        let mut t = 0.0;
        for _ in 0..200 {
            t += 16.7;
            g.frame(t);
        }
        assert!(g.slowmo <= 0.0, "slow motion wears off");
    }

    #[test]
    fn multikills_streaks_and_score() {
        let mut g = Game::new(12);
        g.start("diff=rookie");
        for k in 0..6 {
            g.spawn_zombie(ZKind::Walker, v2(100.0 + k as f32 * 40.0, 0.0));
        }
        g.grid.build(g.player.pos, &g.zombies);
        for j in 0..3 {
            g.damage_zombie(j, 1.0e6, v2(1.0, 0.0), 0.0, false);
        }
        assert_eq!(g.callout.as_ref().map(|c| c.text), Some("TRIPLE KILL"));
        let after3 = g.score;
        assert!(after3 > 30, "kills plus the multi-kill bonus");
        // streak multiplier grows the per-kill value
        g.combo = 60;
        let before = g.score;
        g.damage_zombie(3, 1.0e6, v2(1.0, 0.0), 0.0, false);
        assert!(g.score - before >= 40, "x4 multiplier at a 60 streak");
        // the chain expires
        g.tick_callouts(1.0);
        assert_eq!(g.multi_n, 0);
        assert!(g.save_string().contains(&format!("score={}", g.score)));
    }

    #[test]
    fn levelup_reroll_banish_skip() {
        let mut g = Game::new(13);
        g.start("diff=rookie");
        g.pending_levels = 2;
        g.open_levelup();
        assert_eq!(g.phase, Phase::LevelUp);
        g.reroll();
        assert_eq!(g.rerolls, 1);
        let victim = g.offers[0];
        g.toggle_banish();
        g.choose(0);
        assert_eq!(g.banishes, 1);
        assert!(g.banished.contains(&victim));
        assert_eq!(g.ups.get(victim), 0, "banishing doesn't take it");
        for _ in 0..50 {
            g.roll_offers();
            assert!(!g.offers.contains(&victim), "banished never comes back");
        }
        g.skip_levelup();
        assert_eq!(g.skips, 2);
        assert_eq!(g.phase, Phase::LevelUp, "second queued level-up opens");
        g.skip_levelup();
        assert_eq!(g.phase, Phase::Playing);
        // charges and bans survive a save
        let s = g.save_string();
        let mut h = Game::new(14);
        h.load_save(&s);
        assert_eq!((h.rerolls, h.banishes, h.skips), (1, 1, 1));
        assert_eq!(h.banished, vec![victim]);
    }

    #[test]
    fn chests_reveal_upgrades_and_queue_behind_overlays() {
        let mut g = Game::new(15);
        g.start("diff=rookie");
        g.open_chest();
        let c = g.chest.as_ref().unwrap();
        assert_eq!(c.picks.len(), 3, "first chest is generous");
        let total: u32 = crate::upgrades::ALL.iter().map(|u| g.ups.get(*u) as u32).sum();
        assert_eq!(total, 3, "picks are applied on open");
        assert_eq!(g.phase, Phase::LevelUp);
        // first press skips the spin, second closes
        assert!(g.close_chest());
        assert!(g.chest.is_some());
        assert!(g.close_chest());
        assert!(g.chest.is_none());
        assert_eq!(g.phase, Phase::Playing);
        // a chest picked up during a level-up waits its turn
        g.pending_levels = 1;
        g.open_levelup();
        g.pending_chests = 1;
        g.choose(0);
        let mut t = 0.0;
        for _ in 0..4 {
            t += 16.7;
            g.frame(t);
        }
        assert!(g.chest.is_some(), "queued chest opens after the level-up");
    }

    #[test]
    fn elements_tick_freeze_and_combo() {
        let mut g = Game::new(16);
        g.start("diff=rookie");
        g.wave.n = 5;
        g.spawn_zombie(ZKind::Brute, v2(300.0, 0.0));
        g.spawn_zombie(ZKind::Brute, v2(330.0, 40.0));
        g.grid.build(g.player.pos, &g.zombies);
        let hp0 = g.zombies[0].hp;
        // burn ticks damage over time
        g.afflict(0, Elem::Burn, 10.0);
        for _ in 0..240 {
            g.update_status();
        }
        assert!(g.zombies[0].hp < hp0, "burn hurts");
        // two chills freeze; a frozen zombie doesn't move
        g.afflict(1, Elem::Chill, 1.0);
        g.afflict(1, Elem::Chill, 1.0);
        assert!(g.zombies[1].st.frozen());
        assert_eq!(g.zombies[1].st.speed_mult(), 0.0);
        // shock on a frozen zombie shatters it for a chunk of its health
        let hp1 = g.zombies[1].hp;
        g.afflict(1, Elem::Shock, 10.0);
        assert!(!g.zombies[1].st.frozen());
        assert!(g.zombies[1].hp < hp1 - g.zombies[1].max_hp * 0.3 || g.zombies[1].dead);
        // poison stacks cap at 10; with burn it blights
        for _ in 0..12 {
            g.afflict(0, Elem::Poison, 1.0);
        }
        assert!(g.zombies[0].st.poison <= 10);
        // snapshot bits round-trip the visible statuses
        let b = g.zombies[0].st.bits();
        assert_eq!(crate::status::Status::from_bits(b).poison > 0, g.zombies[0].st.poison > 0);
    }

    #[test]
    fn evolutions_need_max_plus_partner_and_come_from_chests() {
        let mut g = Game::new(17);
        g.start("diff=rookie");
        g.chests_opened = 5;
        for _ in 0..6 {
            g.apply_upgrade(Up::Blades);
        }
        assert!(g.evo_ready().is_none(), "needs the partner passive");
        g.apply_upgrade(Up::Speed);
        assert_eq!(g.evo_ready(), Some(Evo::ReaperRing));
        let before = g.blade_count();
        g.open_chest();
        assert!(g.has_evo(Evo::ReaperRing));
        assert_eq!(g.blade_count(), before + 2);
        assert_eq!(
            g.chest.as_ref().unwrap().picks[0],
            crate::chest::ChestPick::Evo(Evo::ReaperRing)
        );
        assert!(g.evo_ready().is_none(), "evolves once");
        let mut h = Game::new(18);
        h.load_save(&g.save_string());
        assert!(h.has_evo(Evo::ReaperRing));
    }

    #[test]
    fn elite_affixes() {
        use crate::elites::Affix;
        let mut g = Game::new(19);
        g.start("diff=rookie");
        g.wave.n = 8;
        g.spawn_zombie(ZKind::Walker, v2(200.0, 0.0));
        let base_hp = g.zombies[0].max_hp;
        g.make_elite(Affix::Shielded);
        assert!(g.zombies[0].max_hp > base_hp * 2.5 && g.zombies[0].shield > 0.0);
        g.grid.build(g.player.pos, &g.zombies);
        let hp = g.zombies[0].hp;
        g.damage_zombie(0, 1.0, v2(1.0, 0.0), 0.0, false);
        assert_eq!(g.zombies[0].hp, hp, "the shield soaks it");
        // splitting elites burst into three on death
        g.zombies.clear();
        g.spawn_zombie(ZKind::Brute, v2(200.0, 0.0));
        g.make_elite(Affix::Splitting);
        g.grid.build(g.player.pos, &g.zombies);
        g.damage_zombie(0, 1.0e9, v2(1.0, 0.0), 0.0, false);
        assert_eq!(g.zombies.iter().filter(|z| !z.dead).count(), 3);
        // volatile ones blow up after a fuse
        g.zombies.clear();
        g.spawn_zombie(ZKind::Walker, v2(60.0, 0.0));
        g.make_elite(Affix::Volatile);
        g.grid.build(g.player.pos, &g.zombies);
        g.damage_zombie(0, 1.0e9, v2(1.0, 0.0), 0.0, false);
        assert_eq!(g.volatile.len(), 1);
        g.player.iframes = 0.0;
        let hp0 = g.player.hp;
        for _ in 0..120 {
            g.update_volatile();
        }
        assert!(g.volatile.is_empty() && g.player.hp < hp0, "telegraphed blast hurts");
    }

    #[test]
    fn new_archetypes_behave() {
        let mut g = Game::new(20);
        g.start("diff=rookie");
        g.wave.n = 14;
        // armored: frontal hits glance off, flanking works
        g.spawn_zombie(ZKind::Armored, v2(200.0, 0.0));
        g.zombies[0].face = PI; // facing the player at the origin
        g.grid.build(g.player.pos, &g.zombies);
        let hp = g.zombies[0].hp;
        g.damage_zombie(0, 10.0, v2(1.0, 0.0), 0.0, false);
        let front = hp - g.zombies[0].hp;
        let hp = g.zombies[0].hp;
        g.damage_zombie(0, 10.0, v2(-1.0, 0.0), 0.0, false);
        let back = hp - g.zombies[0].hp;
        assert!(front < back * 0.5, "front {front} vs back {back}");
        // burrower: untargetable until it erupts next to you
        g.zombies.clear();
        g.spawn_zombie(ZKind::Burrower, v2(400.0, 0.0));
        g.grid.build(g.player.pos, &g.zombies);
        assert!(g.nearest_zombie(g.player.pos, 2000.0, 0).is_none(), "hidden from the grid");
        g.player.hp = 1.0e9;
        g.player.max_hp = 1.0e9;
        let mut t = 0.0;
        for _ in 0..600 {
            t += 16.7;
            g.player.pos = V2::ZERO;
            g.frame(t);
            if !g.zombies.iter().any(|z| z.hidden) {
                break;
            }
        }
        assert!(g.zombies.iter().any(|z| z.kind == ZKind::Burrower && !z.hidden), "it surfaces");
        // screamer: enrages its neighbours
        g.zombies.clear();
        g.spawn_zombie(ZKind::Screamer, v2(500.0, 0.0));
        g.spawn_zombie(ZKind::Walker, v2(540.0, 0.0));
        g.zombies[0].timer = 0.0;
        g.grid.build(g.player.pos, &g.zombies);
        g.update_zombies();
        assert!(g.zombies[1].rage_t > 0.0);
    }

    #[test]
    fn boss_phases_weak_point_enrage_and_hook() {
        let mut g = Game::new(21);
        g.start("diff=rookie");
        g.spawn_boss(BossVar::Butcher, v2(300.0, 0.0));
        g.zombies[0].face = PI; // facing the player
        g.grid.build(g.player.pos, &g.zombies);
        // weak point: a hit travelling with its facing (from behind) doubles
        let hp = g.zombies[0].hp;
        g.damage_zombie(0, 10.0, v2(1.0, 0.0), 0.0, false); // front
        let front = hp - g.zombies[0].hp;
        let hp = g.zombies[0].hp;
        g.damage_zombie(0, 10.0, v2(-1.0, 0.0), 0.0, false); // back
        let back = hp - g.zombies[0].hp;
        assert!(back > front * 1.9, "front {front} back {back}");
        // dropping below 66% starts phase 2 with adds and a shield
        let max = g.zombies[0].max_hp;
        g.zombies[0].hp = max * 0.6;
        let before = g.zombies.len();
        g.update_bosses();
        assert_eq!(g.zombies[0].phase, 2);
        assert!(g.zombies.len() > before && g.zombies[0].shield > 0.0);
        // enrage after the timer
        g.zombies[0].age = crate::boss::ENRAGE_T + 1.0;
        g.update_bosses();
        assert!(g.zombies[0].rage_t > 1000.0);
        // slams land after their telegraph
        g.zombies.clear();
        g.slams.clear();
        g.player.iframes = 0.0;
        let php = g.player.hp;
        g.slams.push(crate::boss::Slam { pos: g.player.pos, r: 100.0, t: 0.0, dur: 0.2, dmg: 10.0, fire: false });
        for _ in 0..40 {
            g.update_bosses();
        }
        assert!(g.slams.is_empty() && g.player.hp < php);
        // the hook drags you in
        g.player.iframes = 0.0;
        g.player.vel = V2::ZERO;
        g.butcher_hook(v2(300.0, 0.0), v2(-1.0, 0.0), 5.0);
        assert!(g.player.vel.x > 500.0, "pulled toward the butcher");
    }

    #[test]
    fn rocks_spreads_and_grazes() {
        let mut g = Game::new(22);
        g.start("diff=rookie");
        g.wave.n = 16;
        // high-wave spitters fire a 3-way spread
        g.spawn_zombie(ZKind::Spitter, v2(450.0, 0.0));
        g.zombies[0].timer = 0.0;
        g.grid.build(g.player.pos, &g.zombies);
        g.update_zombies();
        assert_eq!(g.eshots.len(), 3);
        // a near miss grazes once
        g.eshots.clear();
        g.player.iframes = 0.0;
        g.eshots.push(EShot { pos: v2(30.0, 30.0), prev: v2(30.0, 30.0), vel: v2(0.0, 0.0), life: 1.0, dmg: 5.0, dead: false, turn: 0.0, r: 6.5, grazed: false });
        let s0 = g.score;
        g.update_eshots();
        g.update_eshots();
        assert_eq!(g.score, s0 + 10, "grazed exactly once");
        // brutes wind up and throw rocks
        g.eshots.clear();
        g.zombies.clear();
        g.spawn_zombie(ZKind::Brute, v2(400.0, 0.0));
        g.zombies[0].special_t = 0.0;
        g.grid.build(g.player.pos, &g.zombies);
        for _ in 0..120 {
            g.update_zombies();
        }
        assert!(g.eshots.iter().any(|s| s.r > 10.0), "a rock is in the air");
    }

    #[test]
    fn character_kits() {
        let mut g = Game::new(23);
        g.start("char=mara;gun=smg");
        assert_eq!(g.kit(), content::Kit::Turret);
        assert!(g.signature_mult() > 1.1, "Mara's signature SMG");
        g.ability_cd = 0.0;
        assert!(g.try_ability());
        assert_eq!(g.turrets.len(), 1);
        assert!(g.ability_cd > 0.0);
        // the turret shoots at zombies on its own
        g.spawn_zombie(ZKind::Walker, g.turrets[0].pos + v2(200.0, 0.0));
        g.grid.build(g.player.pos, &g.zombies);
        let n = g.bullets.len();
        for _ in 0..60 {
            g.update_abilities();
        }
        assert!(g.bullets.len() > n, "turret fired");
        // heat sink doubles fire rate
        let mut h = Game::new(24);
        h.start("char=rook");
        let slow = h.fire_interval();
        h.ability_cd = 0.0;
        h.try_ability();
        assert!(h.fire_interval() < slow * 0.6);
        // saves keep the survivor
        let mut k = Game::new(25);
        k.load_save(&g.save_string());
        assert_eq!(content::CHARACTERS[k.character].id, "mara");
    }

    #[test]
    fn corpses_fade_and_elemental_deaths() {
        let mut g = Game::new(26);
        g.start("diff=rookie");
        g.spawn_zombie(ZKind::Walker, v2(200.0, 0.0));
        g.grid.build(g.player.pos, &g.zombies);
        g.afflict(0, Elem::Chill, 1.0);
        g.afflict(0, Elem::Chill, 1.0);
        let parts = g.particles.len();
        g.damage_zombie(0, 1.0e9, v2(1.0, 0.0), 0.0, false);
        assert_eq!(g.corpses.len(), 1);
        assert!(g.corpses[0].elem & crate::status::FROZEN_BIT != 0);
        assert!(g.particles.iter().skip(parts).any(|p| p.color == "#d8fbff"), "ice shards");
        g.update_corpses(1.0);
        assert_eq!(g.corpses.len(), 1);
        g.update_corpses(1.0);
        assert!(g.corpses.is_empty(), "faded away");
    }

    #[test]
    fn outbreak_round_loop() {
        let mut g = Game::new(27);
        g.start("mode=outbreak;diff=rookie");
        assert_eq!(g.mode, Mode::Outbreak);
        assert!(g.ob.is_some() && g.flow.is_some());
        assert_eq!(g.ob.as_ref().unwrap().points, 500);
        g.player.hp = 1.0e9;
        g.player.max_hp = 1.0e9;
        let mut t = 0.0;
        // round 1 starts and zombies come in through windows
        let mut saw_window = false;
        for _ in 0..1200 {
            t += 16.7;
            g.frame(t);
            if g.zombies.iter().any(|z| z.win >= 0) {
                saw_window = true;
            }
        }
        assert_eq!(g.wave.n, 1);
        assert!(saw_window, "zombies break in through windows");
        let boards: u32 = g.ob.as_ref().unwrap().windows.iter().map(|w| w.boards as u32).sum();
        assert!(boards < 15 * 6, "boards get torn off");
        // shooting earns points; the pistol runs dry and reloads
        assert!(g.ob.as_ref().unwrap().points > 500, "points from hits");
        // buying the lobby door opens it
        let m = world::map().unwrap();
        let d = m.door_index("lobby").unwrap();
        g.ob.as_mut().unwrap().points = 5000;
        g.player.pos = m.door_center(d) + v2(0.0, 60.0);
        assert!(g.ob_interact());
        assert!(m.doors[d].open.get());
        assert_eq!(g.ob.as_ref().unwrap().points, 5000 - 750);
        // power-ups apply
        g.apply_powerup(PickKind::InstaKill, false);
        assert!(g.insta_kill());
    }

    #[test]
    fn hound_rounds() {
        let mut g = Game::new(28);
        g.start("mode=outbreak;diff=rookie");
        let n = g.ob.as_ref().unwrap().next_hounds;
        g.wave.n = n - 1;
        g.wave.intermission = DT * 0.5;
        g.outbreak_waves_pub();
        assert!(g.ob.as_ref().unwrap().hound_round);
        g.wave.timer = 0.0;
        g.outbreak_waves_pub();
        assert!(g.zombies.iter().any(|z| z.kind == ZKind::Hound && z.win < 0), "hounds strike in from lightning");
    }

    #[test]
    fn traps_shield_and_easter_egg() {
        let mut g = Game::new(29);
        g.start("mode=outbreak;diff=rookie");
        let m = world::map().unwrap();
        // trap: pay, it runs, then cools down
        let ti = m.interact.iter().position(|i| i.kind == "trap").unwrap();
        g.ob.as_mut().unwrap().points = 5000;
        assert!(g.activate_trap(ti));
        assert!(!g.activate_trap(ti), "can't stack a running trap");
        let tp = g.trap_at(ti).unwrap().pos;
        g.spawn_zombie(ZKind::Walker, tp);
        g.grid.build(g.player.pos, &g.zombies);
        for _ in 0..60 {
            g.update_traps();
        }
        assert!(g.zombies[0].dead || g.zombies[0].hp < g.zombies[0].max_hp);
        // shield: three parts, build, block frontal damage
        let parts: Vec<usize> = m.interact.iter().enumerate().filter(|(_, i)| i.kind == "part").map(|(i, _)| i).collect();
        assert!(!g.build_shield());
        for p in &parts {
            assert!(g.pick_part(*p));
        }
        assert!(g.build_shield());
        g.player.aim = 0.0;
        let through = g.shield_block(10.0, g.player.pos + v2(100.0, 0.0));
        assert!(through < 3.0);
        let behind = g.shield_block(10.0, g.player.pos - v2(100.0, 0.0));
        assert_eq!(behind, 10.0);
        // easter egg: candles need power, then survive the round
        let candles: Vec<usize> = m.interact.iter().enumerate().filter(|(_, i)| i.kind == "candle").map(|(i, _)| i).collect();
        g.light_candle(candles[0]);
        assert!(g.secrets.as_ref().unwrap().candles.is_empty(), "no power, no flame");
        g.ob.as_mut().unwrap().power = true;
        for c in &candles {
            g.light_candle(*c);
        }
        g.wave.n += 1;
        g.check_egg();
        assert!(g.secrets.as_ref().unwrap().egg_done);
        assert_eq!(g.ob.as_ref().unwrap().perks.len(), 4);
    }

    #[test]
    fn siege_loop() {
        use crate::modes::siege::{DefKind, SiegePhase};
        let mut g = Game::new(30);
        g.start("mode=siege;diff=rookie");
        assert_eq!(g.mode, Mode::Siege);
        let s = g.siege.as_ref().unwrap();
        assert_eq!(s.phase, SiegePhase::Build);
        let crystal = s.crystal;
        // placement rules
        assert!(g.can_place(DefKind::Tower, crystal).is_err(), "not on the crystal");
        let spot = g.player.pos + v2(-120.0, 80.0);
        let spot = v2((spot.x / 40.0).round() * 40.0, (spot.y / 40.0).round() * 40.0);
        assert!(g.place_defense(DefKind::Tower, spot));
        assert_eq!(g.siege.as_ref().unwrap().defenses.len(), 1);
        assert!(!g.place_defense(DefKind::Tower, spot), "occupied");
        // ready up: the wave pours out of the portal and heads for the crystal
        g.ready_up();
        g.player.hp = 1.0e9;
        g.player.max_hp = 1.0e9;
        g.player.pos = v2(0.0, 900.0); // stay out of the way
        let mut t = 0.0;
        let mut closest = f32::MAX;
        for _ in 0..1500 {
            t += 16.7;
            g.player.pos = v2(0.0, 900.0);
            g.frame(t);
            for z in g.zombies.iter() {
                closest = closest.min(z.pos.dist(crystal));
            }
        }
        assert!(g.wave.n >= 1, "the first wave started");
        assert!(closest < 600.0, "enemies path toward the crystal, got {closest}");
        assert!(g.kills > 0, "the tower and the player fight back");
        // the crystal can fall
        g.siege.as_mut().unwrap().crystal_hp = 0.1;
        g.spawn_zombie(ZKind::Walker, crystal + v2(40.0, 0.0));
        g.zombies.last_mut().unwrap().attack_cd = 0.0;
        g.grid.build(g.player.pos, &g.zombies);
        g.update_defenses();
        assert_eq!(g.phase, Phase::Dying);
    }

    #[test]
    fn home_base() {
        let mut g = Game::new(31);
        g.start("mode=hub");
        assert_eq!(g.mode, Mode::Hub);
        let h = g.hub.as_ref().unwrap();
        assert_eq!(h.npcs.len(), 4);
        let smith = h.npcs.iter().find(|n| n.id == "gunsmith").unwrap().pos;
        // no shooting in the plaza
        let mut t = 0.0;
        for _ in 0..120 {
            t += 16.7;
            g.frame(t);
        }
        assert!(g.zombies.iter().all(|z| z.dummy), "only range dummies");
        assert!(g.bullets.is_empty(), "no shooting outside the range");
        // walk up to the gunsmith and talk
        g.player.pos = smith + v2(0.0, 60.0);
        assert!(g.hub_interact());
        assert!(g.ui_out.iter().any(|e| e.0 == "hubopen" && e.1 == "gunsmith"));
        g.npc_say("gunsmith", "Keep your voice down.");
        assert!(g.hub.as_ref().unwrap().npcs.iter().any(|n| n.line_t > 0.0));
        // walking away closes the stall
        g.ui_out.clear();
        g.player.pos = v2(0.0, 300.0);
        g.frame(t + 16.7);
        assert!(g.ui_out.iter().any(|e| e.0 == "hubleave"));
        // the firing range: shooting dummies registers DPS, dummies never die
        let range = g.hub.as_ref().unwrap().range;
        g.player.pos = v2((range[0] + range[2]) * 0.5, range[1] + 120.0);
        g.auto_aim = true;
        for _ in 0..240 {
            t += 16.7;
            g.frame(t);
        }
        assert!(g.dps() > 0.0, "dummies take hits");
        assert_eq!(g.zombies.iter().filter(|z| z.dummy && !z.dead).count(), 3);
        // party members show up
        g.set_hub_mates("Mara~10~20~0.5~crimson~cap~smg|Doc~-50~40~1~default~none~pistol");
        assert_eq!(g.hub.as_ref().unwrap().mates.len(), 2);
    }

    #[test]
    fn daily_seed_is_deterministic_and_mutators_apply() {
        let run = |seed: u64| {
            let mut g = Game::new(seed + 1000); // different construction seeds...
            g.start(&format!("seed={};mut=glass,fast;daily=2026-10-11;diff=rookie", 4242));
            let mut t = 0.0;
            for _ in 0..400 {
                t += 16.7;
                g.player.hp = g.player.max_hp;
                g.frame(t);
            }
            (g.zombies.iter().map(|z| (z.pos.x as i32, z.pos.y as i32)).collect::<Vec<_>>(), g.player.max_hp, g.mutators)
        };
        let (a, hp, m) = run(1);
        let (b, _, _) = run(2);
        assert_eq!(a, b, "...but the same daily seed gives the same run");
        assert!(m.glass && m.fast);
        assert!(hp <= 50.0, "glass cannon halves health");
    }
}
