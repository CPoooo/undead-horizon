//! Game content tables: difficulties, guns, perks, cosmetics, boss variants and
//! the campaign. Ids are shared with the shop catalog in `index.html`.

// ------------------------------------------------------------------ difficulty

pub struct Difficulty {
    pub name: &'static str,
    pub hp: f32,
    pub dmg: f32,
    pub count: f32,
    pub speed: f32,
    pub coins: f32,
}

pub const DIFFICULTIES: [Difficulty; 4] = [
    Difficulty {
        name: "Rookie",
        hp: 0.7,
        dmg: 0.6,
        count: 0.8,
        speed: 0.95,
        coins: 0.75,
    },
    Difficulty {
        name: "Survivor",
        hp: 1.0,
        dmg: 1.0,
        count: 1.0,
        speed: 1.0,
        coins: 1.0,
    },
    Difficulty {
        name: "Veteran",
        hp: 1.35,
        dmg: 1.4,
        count: 1.2,
        speed: 1.07,
        coins: 1.5,
    },
    Difficulty {
        name: "Nightmare",
        hp: 1.8,
        dmg: 1.9,
        count: 1.4,
        speed: 1.14,
        coins: 2.25,
    },
];

pub fn difficulty_index(id: &str) -> usize {
    match id {
        "rookie" => 0,
        "veteran" => 2,
        "nightmare" => 3,
        _ => 1,
    }
}

// ------------------------------------------------------------------ enemies

/// Data for one zombie archetype, indexed by `ZKind as usize`. Behavior that
/// can't be expressed as numbers (spitting, fuses, boss specials) still lives
/// in `game.rs::update_zombies`, keyed on the kind.
pub struct EnemyDef {
    pub name: &'static str,
    pub hp: f32,
    pub speed: f32,
    pub r: f32,
    pub dmg: f32,
    pub xp: u32,
    /// multiplier on knockback taken (1 = full, 0 = immovable)
    pub knock: f32,
    /// first scaled wave it can spawn on; 0 = never spawns from the wave pool
    pub min_wave: u32,
    /// spawn weight: `base + per_wave * n`, capped at `cap`
    pub weight: (f32, f32, f32),
    /// clothing palette (one picked by tint), skin, dark outline
    pub cloth: &'static [&'static str],
    pub skin: &'static str,
    pub dark: &'static str,
    pub gore: &'static str,
    pub decal: &'static str,
    /// drops a gem shower, shakes the screen and gets a hitstop on death
    pub big: bool,
    /// draws an overhead health bar once damaged
    pub hp_bar: bool,
}

pub const ENEMIES: [EnemyDef; 15] = [
    EnemyDef {
        name: "Walker",
        hp: 20.0,
        speed: 72.0,
        r: 15.0,
        dmg: 10.0,
        xp: 1,
        knock: 1.0,
        min_wave: 1,
        weight: (10.0, 0.0, 10.0),
        cloth: &["#4b5a7a", "#6a4b3a", "#5a5a5a", "#3e5f4a"],
        skin: "#7f9c5a",
        dark: "#2e3a22",
        gore: "#6f8f4e",
        decal: "#4a0a0e",
        big: false,
        hp_bar: false,
    },
    EnemyDef {
        name: "Runner",
        hp: 12.0,
        speed: 150.0,
        r: 13.0,
        dmg: 8.0,
        xp: 1,
        knock: 1.0,
        min_wave: 2,
        weight: (2.0, 0.4, 8.0),
        cloth: &["#8a3b3b"],
        skin: "#a3ad66",
        dark: "#3a2020",
        gore: "#9aa25a",
        decal: "#4a0a0e",
        big: false,
        hp_bar: false,
    },
    EnemyDef {
        name: "Brute",
        hp: 150.0,
        speed: 54.0,
        r: 26.0,
        dmg: 22.0,
        xp: 6,
        knock: 0.35,
        min_wave: 4,
        weight: (0.8, 0.12, 3.5),
        cloth: &["#3a3f4a"],
        skin: "#5f7f45",
        dark: "#1d2418",
        gore: "#4f6b3a",
        decal: "#4a0a0e",
        big: true,
        hp_bar: true,
    },
    EnemyDef {
        name: "Spitter",
        hp: 26.0,
        speed: 82.0,
        r: 15.0,
        dmg: 10.0,
        xp: 3,
        knock: 1.0,
        min_wave: 6,
        weight: (1.2, 0.1, 3.0),
        cloth: &["#4a6b2a"],
        skin: "#9fd04f",
        dark: "#2a3d14",
        gore: "#7fb04a",
        decal: "#3e6b1f",
        big: false,
        hp_bar: true,
    },
    EnemyDef {
        name: "Exploder",
        hp: 18.0,
        speed: 112.0,
        r: 16.0,
        dmg: 8.0,
        xp: 2,
        knock: 1.0,
        min_wave: 3,
        weight: (1.2, 0.15, 4.0),
        cloth: &["#a3501c"],
        skin: "#d0904a",
        dark: "#4a200a",
        gore: "#c9772f",
        decal: "#4a0a0e",
        big: false,
        hp_bar: false,
    },
    EnemyDef {
        name: "Boss",
        hp: 1700.0,
        speed: 74.0,
        r: 46.0,
        dmg: 30.0,
        xp: 60,
        knock: 0.06,
        min_wave: 0,
        weight: (0.0, 0.0, 0.0),
        cloth: &["#5a3b5e"],
        skin: "#5a3b5e",
        dark: "#2a1a2e",
        gore: "#5a3b5e",
        decal: "#4a0a0e",
        big: true,
        hp_bar: false,
    },
    EnemyDef {
        name: "Crawler",
        hp: 10.0,
        speed: 172.0,
        r: 11.0,
        dmg: 7.0,
        xp: 1,
        knock: 1.2,
        min_wave: 3,
        weight: (1.5, 0.2, 5.0),
        cloth: &["#5a4a3a"],
        skin: "#8fa86a",
        dark: "#2a2a1a",
        gore: "#8fa86a",
        decal: "#4a0a0e",
        big: false,
        hp_bar: false,
    },
    EnemyDef {
        name: "Screamer",
        hp: 34.0,
        speed: 78.0,
        r: 14.0,
        dmg: 6.0,
        xp: 4,
        knock: 1.0,
        min_wave: 8,
        weight: (0.6, 0.05, 1.6),
        cloth: &["#6a2a4a"],
        skin: "#c8b0c0",
        dark: "#2a1020",
        gore: "#b090a8",
        decal: "#4a0a0e",
        big: false,
        hp_bar: true,
    },
    EnemyDef {
        name: "Armored",
        hp: 70.0,
        speed: 64.0,
        r: 20.0,
        dmg: 16.0,
        xp: 5,
        knock: 0.5,
        min_wave: 10,
        weight: (0.7, 0.07, 2.5),
        cloth: &["#4a4f5a"],
        skin: "#7a8f5a",
        dark: "#1e2228",
        gore: "#6a7f4a",
        decal: "#4a0a0e",
        big: true,
        hp_bar: true,
    },
    EnemyDef {
        name: "Bat",
        hp: 6.0,
        speed: 195.0,
        r: 9.0,
        dmg: 5.0,
        xp: 1,
        knock: 1.5,
        min_wave: 7,
        weight: (0.5, 0.05, 1.5),
        cloth: &["#3a2a4a"],
        skin: "#5a3a6a",
        dark: "#1a1020",
        gore: "#5a3a6a",
        decal: "#2a0a1e",
        big: false,
        hp_bar: false,
    },
    EnemyDef {
        name: "Burrower",
        hp: 45.0,
        speed: 95.0,
        r: 17.0,
        dmg: 18.0,
        xp: 4,
        knock: 0.8,
        min_wave: 12,
        weight: (0.5, 0.05, 1.8),
        cloth: &["#5a3a1a"],
        skin: "#9a7a4a",
        dark: "#2a1a0a",
        gore: "#8a6a3a",
        decal: "#3a2a10",
        big: false,
        hp_bar: true,
    },
    EnemyDef {
        name: "Hellhound",
        hp: 16.0,
        speed: 210.0,
        r: 13.0,
        dmg: 9.0,
        xp: 2,
        knock: 1.3,
        min_wave: 0,
        weight: (0.0, 0.0, 0.0),
        cloth: &["#2a1410"],
        skin: "#5a2a1a",
        dark: "#140806",
        gore: "#ff6a2a",
        decal: "#3a1408",
        big: false,
        hp_bar: false,
    },
    EnemyDef {
        name: "Sapper",
        hp: 22.0,
        speed: 120.0,
        r: 13.0,
        dmg: 6.0,
        xp: 3,
        knock: 1.0,
        min_wave: 0,
        weight: (0.0, 0.0, 0.0),
        cloth: &["#6a5a20"],
        skin: "#9aa04a",
        dark: "#2a2410",
        gore: "#c9a02f",
        decal: "#4a0a0e",
        big: false,
        hp_bar: false,
    },
    EnemyDef {
        name: "Ogre",
        hp: 420.0,
        speed: 48.0,
        r: 34.0,
        dmg: 30.0,
        xp: 14,
        knock: 0.15,
        min_wave: 0,
        weight: (0.0, 0.0, 0.0),
        cloth: &["#4a3a2a"],
        skin: "#7a8a5a",
        dark: "#1a1a10",
        gore: "#6a7a4a",
        decal: "#4a0a0e",
        big: true,
        hp_bar: true,
    },
    EnemyDef {
        name: "Archer",
        hp: 24.0,
        speed: 80.0,
        r: 14.0,
        dmg: 9.0,
        xp: 3,
        knock: 1.0,
        min_wave: 0,
        weight: (0.0, 0.0, 0.0),
        cloth: &["#2a4a3a"],
        skin: "#8aa070",
        dark: "#10201a",
        gore: "#7a9060",
        decal: "#4a0a0e",
        big: false,
        hp_bar: true,
    },
];

// ------------------------------------------------------------------ guns

#[derive(Clone, Copy)]
pub struct Gun {
    pub id: &'static str,
    pub name: &'static str,
    /// multiplier on the base fire interval (lower = faster)
    pub interval: f32,
    pub dmg: f32,
    pub pellets: u32,
    /// total fan angle for multi-pellet guns (radians)
    pub spread: f32,
    /// random angular jitter per bullet (radians)
    pub jitter: f32,
    pub speed: f32,
    pub life: f32,
    pub pierce: i32,
    /// built-in explosion radius (0 = none)
    pub blast: f32,
    pub knock: f32,
    pub move_mult: f32,
    pub heavy: bool,
    /// camera recoil per shot (world units, pushed against the aim)
    pub kick: f32,
    /// ejected shell color ("" = no casing)
    pub casing: &'static str,
    /// muzzle flash light radius
    pub flash: f32,
    /// Outbreak ammo: magazine, reserve, reload seconds
    pub mag: u32,
    pub reserve: u32,
    pub reload: f32,
    /// name after the upgrade machine
    pub pap: &'static str,
    // visuals
    pub len: f32,
    pub width: f32,
    pub color: &'static str,
    pub bullet_r: f32,
}

pub const GUNS: [Gun; 8] = [
    Gun {
        id: "pistol",
        name: "M9 Sidearm",
        interval: 1.0,
        dmg: 1.0,
        pellets: 1,
        spread: 0.0,
        jitter: 0.0,
        speed: 1.0,
        life: 1.0,
        pierce: 0,
        blast: 0.0,
        knock: 150.0,
        move_mult: 1.0,
        heavy: false,
        kick: 3.0,
        casing: "#d9b45a",
        flash: 110.0,
        mag: 8,
        reserve: 80,
        reload: 1.1,
        pap: "Hellbringer",
        len: 24.0,
        width: 7.0,
        color: "#2a2d34",
        bullet_r: 4.5,
    },
    Gun {
        id: "smg",
        name: "Viper SMG",
        interval: 0.48,
        dmg: 0.55,
        pellets: 1,
        spread: 0.0,
        jitter: 0.09,
        speed: 1.0,
        life: 0.85,
        pierce: 0,
        blast: 0.0,
        knock: 90.0,
        move_mult: 1.05,
        heavy: false,
        kick: 1.6,
        casing: "#d9b45a",
        flash: 90.0,
        mag: 32,
        reserve: 224,
        reload: 1.6,
        pap: "Viper's Kiss",
        len: 26.0,
        width: 7.0,
        color: "#1d2a22",
        bullet_r: 3.8,
    },
    Gun {
        id: "shotgun",
        name: "Boomstick",
        interval: 2.5,
        dmg: 0.62,
        pellets: 6,
        spread: 0.55,
        jitter: 0.04,
        speed: 0.9,
        life: 0.45,
        pierce: 0,
        blast: 0.0,
        knock: 340.0,
        move_mult: 1.0,
        heavy: true,
        kick: 9.0,
        casing: "#b8321e",
        flash: 170.0,
        mag: 6,
        reserve: 54,
        reload: 2.2,
        pap: "Doomsday Boomstick",
        len: 30.0,
        width: 9.0,
        color: "#4a3020",
        bullet_r: 4.0,
    },
    Gun {
        id: "rifle",
        name: "Marksman Rifle",
        interval: 2.1,
        dmg: 2.7,
        pellets: 1,
        spread: 0.0,
        jitter: 0.0,
        speed: 1.45,
        life: 1.2,
        pierce: 2,
        blast: 0.0,
        knock: 260.0,
        move_mult: 0.97,
        heavy: true,
        kick: 6.0,
        casing: "#e0c060",
        flash: 140.0,
        mag: 5,
        reserve: 50,
        reload: 2.0,
        pap: "Judgement",
        len: 40.0,
        width: 6.0,
        color: "#22262e",
        bullet_r: 5.0,
    },
    Gun {
        id: "minigun",
        name: "Shredder Minigun",
        interval: 0.3,
        dmg: 0.42,
        pellets: 1,
        spread: 0.0,
        jitter: 0.12,
        speed: 1.05,
        life: 0.9,
        pierce: 0,
        blast: 0.0,
        knock: 70.0,
        move_mult: 0.88,
        heavy: false,
        kick: 2.2,
        casing: "#d9b45a",
        flash: 120.0,
        mag: 100,
        reserve: 400,
        reload: 3.6,
        pap: "Death Machine",
        len: 34.0,
        width: 11.0,
        color: "#3a3f48",
        bullet_r: 3.6,
    },
    Gun {
        id: "launcher",
        name: "Thumper Launcher",
        interval: 3.0,
        dmg: 2.2,
        pellets: 1,
        spread: 0.0,
        jitter: 0.0,
        speed: 0.6,
        life: 1.1,
        pierce: 0,
        blast: 85.0,
        knock: 300.0,
        move_mult: 0.95,
        heavy: true,
        kick: 12.0,
        casing: "",
        flash: 190.0,
        mag: 4,
        reserve: 28,
        reload: 2.6,
        pap: "Thumper 9000",
        len: 30.0,
        width: 12.0,
        color: "#3d4a2a",
        bullet_r: 8.0,
    },
    // box-only wonder weapons (not sold in the shop)
    Gun {
        id: "ray",
        name: "Ray Rifle",
        interval: 0.9,
        dmg: 3.2,
        pellets: 1,
        spread: 0.0,
        jitter: 0.0,
        speed: 1.3,
        life: 1.0,
        pierce: 1,
        blast: 60.0,
        knock: 220.0,
        move_mult: 1.0,
        heavy: true,
        kick: 4.0,
        casing: "",
        flash: 170.0,
        mag: 20,
        reserve: 160,
        reload: 2.0,
        pap: "Ray Rifle Omega",
        len: 26.0,
        width: 10.0,
        color: "#2a8a3a",
        bullet_r: 7.0,
    },
    Gun {
        id: "wave",
        name: "Wave Gun",
        interval: 3.2,
        dmg: 2.4,
        pellets: 11,
        spread: 1.1,
        jitter: 0.05,
        speed: 0.8,
        life: 0.5,
        pierce: 6,
        blast: 0.0,
        knock: 900.0,
        move_mult: 1.0,
        heavy: true,
        kick: 14.0,
        casing: "",
        flash: 220.0,
        mag: 6,
        reserve: 36,
        reload: 2.4,
        pap: "Tsunami",
        len: 28.0,
        width: 14.0,
        color: "#3a5a9a",
        bullet_r: 9.0,
    },
];

pub fn gun_index(id: &str) -> usize {
    GUNS.iter().position(|g| g.id == id).unwrap_or(0)
}

// ------------------------------------------------------------------ cosmetics

pub struct Skin {
    pub id: &'static str,
    pub dark: &'static str,
    pub light: &'static str,
    pub feet: &'static str,
}

pub const SKINS: [Skin; 8] = [
    Skin {
        id: "default",
        dark: "#1e4fa8",
        light: "#3b7bea",
        feet: "#1d2433",
    },
    Skin {
        id: "crimson",
        dark: "#7a1020",
        light: "#d63a4a",
        feet: "#2a1014",
    },
    Skin {
        id: "camo",
        dark: "#2e4a22",
        light: "#5f7f3c",
        feet: "#1e2618",
    },
    Skin {
        id: "shadow",
        dark: "#121218",
        light: "#34343f",
        feet: "#08080c",
    },
    Skin {
        id: "royal",
        dark: "#3a1a6a",
        light: "#7a4ad8",
        feet: "#1c1030",
    },
    Skin {
        id: "neon",
        dark: "#067a6c",
        light: "#25e8c8",
        feet: "#06302c",
    },
    Skin {
        id: "hazmat",
        dark: "#a08a10",
        light: "#f0d838",
        feet: "#3a3410",
    },
    Skin {
        id: "gold",
        dark: "#9a6a10",
        light: "#ffd04a",
        feet: "#4a3208",
    },
];

pub fn skin_index(id: &str) -> usize {
    SKINS.iter().position(|s| s.id == id).unwrap_or(0)
}

pub const HATS: [&str; 8] = [
    "none", "cap", "headband", "cowboy", "tophat", "horns", "crown", "halo",
];

pub fn hat_index(id: &str) -> usize {
    HATS.iter().position(|h| *h == id).unwrap_or(0)
}

/// (id, trail colour, core colour). "rainbow" is animated at draw time.
pub const TRACERS: [(&str, &str, &str); 6] = [
    ("default", "#ffbe5a", "#fff6dc"),
    ("fire", "#ff4a12", "#ffd0a0"),
    ("ice", "#4ac8ff", "#e6faff"),
    ("toxic", "#62ff2a", "#e8ffd0"),
    ("plasma", "#c04aff", "#f5dcff"),
    ("rainbow", "#ffffff", "#ffffff"),
];

pub fn tracer_index(id: &str) -> usize {
    TRACERS.iter().position(|t| t.0 == id).unwrap_or(0)
}

// ------------------------------------------------------------------ perks (start-of-run upgrades)

pub fn perk_upgrade(id: &str) -> Option<crate::upgrades::Up> {
    use crate::upgrades::Up;
    Some(match id {
        "medic" => Up::Vitality,
        "sprinter" => Up::Speed,
        "scavenger" => Up::Magnet,
        "gunsmith" => Up::Damage,
        "lucky" => Up::Crit,
        "blademaster" => Up::Blades,
        "stormcaller" => Up::Lightning,
        _ => return None,
    })
}

// ------------------------------------------------------------------ bosses

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BossVar {
    Abomination,
    Gravedigger,
    Matriarch,
    CinderKing,
    Wraith,
    BloomHeart,
    Butcher,
    HiveQueen,
}

impl BossVar {
    pub fn name(self) -> &'static str {
        match self {
            BossVar::Abomination => "THE ABOMINATION",
            BossVar::Gravedigger => "GRAVEDIGGER MOSS",
            BossVar::Matriarch => "THE SPITTING MATRIARCH",
            BossVar::CinderKing => "THE CINDER KING",
            BossVar::Wraith => "HOLLOW WRAITH",
            BossVar::BloomHeart => "THE BLOOM HEART",
            BossVar::Butcher => "THE BUTCHER",
            BossVar::HiveQueen => "THE HIVE QUEEN",
        }
    }
    /// Line shown on the boss intro card.
    pub fn taunt(self) -> &'static str {
        match self {
            BossVar::Abomination => "It hungers. It grows. It has seen you.",
            BossVar::Gravedigger => "Every grave I dig has your name on it.",
            BossVar::Matriarch => "My children are hungry. Feed them.",
            BossVar::CinderKing => "Kneel and burn warm. Run and burn anyway.",
            BossVar::Wraith => "You cannot shoot what is already gone.",
            BossVar::BloomHeart => "I am the root of every corpse on this horizon.",
            BossVar::Butcher => "Fresh meat! Come closer... closer...",
            BossVar::HiveQueen => "The swarm is my voice. Listen.",
        }
    }
    pub fn from_u8(v: u8) -> BossVar {
        [
            BossVar::Abomination,
            BossVar::Gravedigger,
            BossVar::Matriarch,
            BossVar::CinderKing,
            BossVar::Wraith,
            BossVar::BloomHeart,
            BossVar::Butcher,
            BossVar::HiveQueen,
        ]
        .get(v as usize)
        .copied()
        .unwrap_or(BossVar::Abomination)
    }
    /// (hp mult, radius, speed mult)
    pub fn stats(self) -> (f32, f32, f32) {
        match self {
            BossVar::Abomination => (1.0, 46.0, 1.0),
            BossVar::Gravedigger => (1.1, 44.0, 0.85),
            BossVar::Matriarch => (0.9, 40.0, 1.0),
            BossVar::CinderKing => (1.15, 46.0, 1.0),
            BossVar::Wraith => (0.6, 36.0, 1.35),
            BossVar::BloomHeart => (2.6, 64.0, 0.7),
            BossVar::Butcher => (1.3, 48.0, 0.95),
            BossVar::HiveQueen => (1.0, 42.0, 0.9),
        }
    }
    /// (body/cloth, skin, dark)
    pub fn colors(self) -> (&'static str, &'static str, &'static str) {
        match self {
            BossVar::Abomination => ("#3a2040", "#7a5a8a", "#1a0a1e"),
            BossVar::Gravedigger => ("#3a3226", "#7d8a5a", "#18140c"),
            BossVar::Matriarch => ("#3d6a1a", "#a6e05a", "#1a300a"),
            BossVar::CinderKing => ("#5a1a0a", "#e0702a", "#200804"),
            BossVar::Wraith => ("#1a2a40", "#9ab8e0", "#0a1020"),
            BossVar::BloomHeart => ("#5a0a3a", "#ff4aa8", "#20041a"),
            BossVar::Butcher => ("#6a5a50", "#c09080", "#2a1a14"),
            BossVar::HiveQueen => ("#3a2a4a", "#b08ad0", "#140a1e"),
        }
    }
}

/// Infinite mode cycles bosses every 5 waves.
pub const INFINITE_BOSSES: [BossVar; 7] = [
    BossVar::Abomination,
    BossVar::Gravedigger,
    BossVar::Butcher,
    BossVar::Matriarch,
    BossVar::CinderKing,
    BossVar::HiveQueen,
    BossVar::BloomHeart,
];

// ------------------------------------------------------------------ campaign

pub struct Chapter {
    pub title: &'static str,
    pub waves: u32,
    /// difficulty offset: local wave 1 plays like infinite wave `base`
    pub base: u32,
    pub boss: BossVar,
    pub twin: bool,
    /// radio chatter shown under the wave banner, one per wave
    pub radio: &'static [&'static str],
}

pub const CHAPTERS: [Chapter; 5] = [
    Chapter {
        title: "The Long Night",
        waves: 3,
        base: 1,
        boss: BossVar::Gravedigger,
        twin: false,
        radio: &[
            "Radio: \"...anyone out there? Head east, to the lighthouse...\"",
            "Rook: \"Hollowbrook used to be quiet. Used to.\"",
            "Radio: \"Something big is digging up the graveyard. Run, courier!\"",
        ],
    },
    Chapter {
        title: "Whispering Pines",
        waves: 4,
        base: 4,
        boss: BossVar::Matriarch,
        twin: false,
        radio: &[
            "Mara (radio): \"I'm Mara. I run the lighthouse. Keep moving, Rook.\"",
            "Mara: \"The spores are thicker in the pines. Don't breathe deep.\"",
            "Rook: \"Something's spitting at me from the trees!\"",
            "Mara: \"That's the Matriarch. She's protecting her brood. Burn it down.\"",
        ],
    },
    Chapter {
        title: "Ashfall",
        waves: 4,
        base: 8,
        boss: BossVar::CinderKing,
        twin: false,
        radio: &[
            "Mara: \"The old refinery is still burning. You'll have to cut through.\"",
            "Rook: \"These ones explode. Of course they explode.\"",
            "Mara: \"The heat is coming from one source. Something... crowned.\"",
            "Mara: \"The Cinder King. Keep your distance when he charges!\"",
        ],
    },
    Chapter {
        title: "The Hollow Choir",
        waves: 5,
        base: 12,
        boss: BossVar::Wraith,
        twin: true,
        radio: &[
            "Mara: \"You hear that singing? That's the Choir. Don't listen.\"",
            "Rook: \"They're everywhere. The whole town turned.\"",
            "Mara: \"The Bloom sent its wardens. Two of them, always together.\"",
            "Rook: \"I can see the lighthouse beam from here!\"",
            "Mara: \"The Wraiths are on you! Break them apart!\"",
        ],
    },
    Chapter {
        title: "Horizon",
        waves: 6,
        base: 16,
        boss: BossVar::BloomHeart,
        twin: false,
        radio: &[
            "Mara: \"The lighthouse signal is what draws them. It always was.\"",
            "Rook: \"You've been luring them here?\"",
            "Mara: \"Luring them to the Heart. If it dies, the Bloom dies.\"",
            "Rook: \"Then let's end it.\"",
            "Mara: \"The ground is splitting. It's waking up!\"",
            "Mara: \"THE BLOOM HEART! Everything you've got, Rook!\"",
        ],
    },
];

// ------------------------------------------------------------------ characters

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kit {
    HeatSink,
    Turret,
    HealPulse,
    FlameCone,
}

pub struct Character {
    pub id: &'static str,
    pub name: &'static str,
    pub class: &'static str,
    pub icon: &'static str,
    pub color: &'static str,
    /// gun id that gets +15% damage in their hands
    pub signature: &'static str,
    pub kit: Kit,
    pub ability: &'static str,
    pub ability_icon: &'static str,
    /// ability cooldown (s)
    pub cd: f32,
    pub passive: &'static str,
}

pub const CHARACTERS: [Character; 4] = [
    Character {
        id: "rook",
        name: "Rook",
        class: "Gunner",
        icon: "🎯",
        color: "#ffd36a",
        signature: "pistol",
        kit: Kit::HeatSink,
        ability: "Heat Sink",
        ability_icon: "🔥",
        cd: 16.0,
        passive: "+10% fire rate",
    },
    Character {
        id: "mara",
        name: "Mara",
        class: "Engineer",
        icon: "🔧",
        color: "#7ad7ff",
        signature: "smg",
        kit: Kit::Turret,
        ability: "Sentry Turret",
        ability_icon: "🛰️",
        cd: 22.0,
        passive: "+30% pickup radius",
    },
    Character {
        id: "doc",
        name: "Doc Hale",
        class: "Medic",
        icon: "⛑️",
        color: "#9cffb8",
        signature: "rifle",
        kit: Kit::HealPulse,
        ability: "Heal Pulse",
        ability_icon: "💚",
        cd: 18.0,
        passive: "Regenerates, revives twice as fast",
    },
    Character {
        id: "ash",
        name: "Ash",
        class: "Pyro",
        icon: "🔥",
        color: "#ff8a3a",
        signature: "shotgun",
        kit: Kit::FlameCone,
        ability: "Flame Cone",
        ability_icon: "🌋",
        cd: 14.0,
        passive: "Hits can set zombies ablaze",
    },
];

pub fn character_index(id: &str) -> usize {
    CHARACTERS.iter().position(|c| c.id == id).unwrap_or(0)
}

// ------------------------------------------------------------------ meta progression

/// Permanent upgrade ranks bought with coins (P2). Each rank is a small edge.
#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub struct Meta {
    pub dmg: u8,
    pub hp: u8,
    pub spd: u8,
    pub magnet: u8,
    pub reroll: u8,
    pub banish: u8,
}

impl Meta {
    pub fn parse(spec: &str) -> Meta {
        let mut m = Meta::default();
        for kv in spec.split(',') {
            let Some((k, v)) = kv.split_once(':') else { continue };
            let n = v.parse::<u8>().unwrap_or(0).min(5);
            match k {
                "dmg" => m.dmg = n,
                "hp" => m.hp = n,
                "spd" => m.spd = n,
                "magnet" => m.magnet = n,
                "reroll" => m.reroll = n.min(3),
                "banish" => m.banish = n.min(3),
                _ => {}
            }
        }
        m
    }
    pub fn is_pure(&self) -> bool {
        *self == Meta::default()
    }
}

/// Stat bonuses from equipped relics (P3), in percent unless noted.
#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub struct RelicBonus {
    pub dmg: f32,
    pub hp: f32,
    pub rate: f32,
    pub crit: f32,
    pub speed: f32,
    pub magnet: f32,
}

/// Relic stats: `id` names the base (its main stat), then rarity (1-3), then
/// up to two affix stats. Spec: `ember:3:crit:speed,locket:1,...`
pub fn relic_bonus(spec: &str) -> RelicBonus {
    let mut b = RelicBonus::default();
    let add = |b: &mut RelicBonus, stat: &str, v: f32| match stat {
        "ember" | "dmg" => b.dmg += v,
        "locket" | "hp" => b.hp += v,
        "charm" | "rate" => b.rate += v,
        "dice" | "crit" => b.crit += v,
        "sigil" | "speed" => b.speed += v,
        "lodestone" | "magnet" => b.magnet += v,
        _ => {}
    };
    for r in spec.split(',').filter(|r| !r.is_empty()).take(3) {
        let f: Vec<&str> = r.split(':').collect();
        let rarity = f.get(1).and_then(|x| x.parse::<f32>().ok()).unwrap_or(1.0).clamp(1.0, 3.0);
        add(&mut b, f[0], 4.0 * rarity);
        for a in f.iter().skip(2).take(2) {
            add(&mut b, a, 1.5 * rarity);
        }
    }
    b
}

/// Weapon mastery camo tints (P4), unlocked by kills with that gun.
pub const CAMOS: [&str; 4] = ["", "#4a6a3a", "#3a5a8a", "#d4af37"];

// ------------------------------------------------------------------ mutators

/// Run modifiers (weekly mutator, Custom mode, the daily run). Mutated runs are
/// not ranked on the main board.
#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub struct Mutators {
    pub bighead: bool,
    pub glass: bool,
    pub fast: bool,
    pub onehit: bool,
    pub floaty: bool,
}

impl Mutators {
    pub fn parse(spec: &str) -> Mutators {
        let mut m = Mutators::default();
        for id in spec.split(',') {
            match id {
                "bighead" => m.bighead = true,
                "glass" => m.glass = true,
                "fast" => m.fast = true,
                "onehit" => m.onehit = true,
                "floaty" => m.floaty = true,
                _ => {}
            }
        }
        m
    }
    pub fn any(&self) -> bool {
        *self != Mutators::default()
    }
    /// coin multiplier for the extra risk
    pub fn coin_mult(&self) -> f32 {
        let mut k = 1.0;
        if self.glass {
            k += 0.25;
        }
        if self.fast {
            k += 0.25;
        }
        if self.onehit {
            k += 1.0;
        }
        k
    }
}

// ------------------------------------------------------------------ accessibility

/// Player comfort settings (A2). Set from the UI with `access|k=v;...`.
#[derive(Clone, Copy, Debug)]
pub struct Access {
    /// 0..1 screen-shake intensity
    pub shake: f32,
    /// 0..1 full-screen flash intensity (ultimate, hurt, low health)
    pub flash: f32,
    /// no zoom punches, slow motion, lookahead or hit squash
    pub reduce_motion: bool,
    /// 0..1 mouse/stick aim assist strength
    pub aim_assist: f32,
    /// 0 off, 1 deuteranopia/protanopia, 2 tritanopia
    pub colorblind: u8,
    /// tap interact to toggle instead of holding
    pub toggle_use: bool,
}

impl Default for Access {
    fn default() -> Self {
        Access { shake: 1.0, flash: 1.0, reduce_motion: false, aim_assist: 0.5, colorblind: 0, toggle_use: false }
    }
}

impl Access {
    pub fn apply(&mut self, spec: &str) {
        for kv in spec.split(';') {
            let Some((k, v)) = kv.split_once('=') else { continue };
            let f = v.parse::<f32>().unwrap_or(0.0);
            match k {
                "shake" => self.shake = f.clamp(0.0, 1.0),
                "flash" => self.flash = f.clamp(0.0, 1.0),
                "motion" => self.reduce_motion = v == "1",
                "assist" => self.aim_assist = f.clamp(0.0, 1.0),
                "cb" => self.colorblind = (f as u8).min(2),
                "toggleuse" => self.toggle_use = v == "1",
                _ => {}
            }
        }
    }
    /// Swap the colors that carry meaning (enemy acid, danger red) for
    /// colorblind-safe ones.
    pub fn color(&self, c: &'static str) -> &'static str {
        match (self.colorblind, c) {
            (1, "#7dff3a") => "#ffe14a",
            (1, "#c4ff6a") => "#fff08a",
            (1, "#9be04a") => "#ffe14a",
            (1, "#ff2040") => "#ff8a00",
            (1, "#ff3040") => "#ff8a00",
            (2, "#7dff3a") => "#ff5ab4",
            (2, "#c4ff6a") => "#ff9ad0",
            (2, "#9be04a") => "#ff5ab4",
            (2, "#7ad7ff") => "#e8e8e8",
            _ => c,
        }
    }
}

// ------------------------------------------------------------------ extra cosmetic slots (V4)

pub const PACKS: [&str; 5] = ["pack", "jet", "shell", "wings", "none"];
pub const KILLFX: [&str; 5] = ["default", "confetti", "flames", "sparkle", "ghost"];
pub const DEATHFX: [&str; 4] = ["default", "ghost", "fireworks", "kaboom"];
pub const EMOTES: [(&str, &str); 6] = [
    ("wave", "👋"),
    ("laugh", "😂"),
    ("skull", "💀"),
    ("flex", "💪"),
    ("heart", "❤️"),
    ("zzz", "😴"),
];

pub fn idx_of(list: &[&str], id: &str) -> usize {
    list.iter().position(|x| *x == id).unwrap_or(0)
}
