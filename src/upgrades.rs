use crate::math::Rng;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Up {
    Damage,
    FireRate,
    Multishot,
    Pierce,
    Velocity,
    Speed,
    Vitality,
    Regen,
    Magnet,
    Crit,
    Blades,
    Lightning,
    Nova,
    Explosive,
    Ricochet,
    Homing,
    Vampire,
    DashMaster,
    Backshot,
    Thorns,
}

pub const ALL: [Up; 20] = [
    Up::Damage,
    Up::FireRate,
    Up::Multishot,
    Up::Pierce,
    Up::Velocity,
    Up::Speed,
    Up::Vitality,
    Up::Regen,
    Up::Magnet,
    Up::Crit,
    Up::Blades,
    Up::Lightning,
    Up::Nova,
    Up::Explosive,
    Up::Ricochet,
    Up::Homing,
    Up::Vampire,
    Up::DashMaster,
    Up::Backshot,
    Up::Thorns,
];

/// 0 = common, 1 = rare, 2 = epic (superpower)
pub struct Def {
    pub name: &'static str,
    pub icon: &'static str,
    pub max: u8,
    pub rarity: u8,
    pub color: &'static str,
}

impl Up {
    pub fn idx(self) -> usize {
        self as usize
    }

    pub fn def(self) -> Def {
        let (name, icon, max, rarity, color) = match self {
            Up::Damage => ("Hollow Points", "💥", 8, 0, "#ff6b4a"),
            Up::FireRate => ("Hair Trigger", "⚡", 8, 0, "#ffd84a"),
            Up::Multishot => ("Split Shot", "🔱", 5, 1, "#7ad7ff"),
            Up::Pierce => ("Piercing Rounds", "🗡️", 5, 0, "#c9d6ff"),
            Up::Velocity => ("Rail Barrel", "🎯", 5, 0, "#9be8c0"),
            Up::Speed => ("Swift Feet", "👟", 5, 0, "#8cffb0"),
            Up::Vitality => ("Iron Heart", "❤️", 6, 0, "#ff5a7a"),
            Up::Regen => ("Regeneration", "🌿", 5, 0, "#6cf08a"),
            Up::Magnet => ("Soul Magnet", "🧲", 5, 0, "#b48cff"),
            Up::Crit => ("Deadeye", "👁️", 5, 1, "#ff9c3a"),
            Up::Blades => ("Orbiting Blades", "🌀", 6, 2, "#a8f0ff"),
            Up::Lightning => ("Chain Lightning", "🌩️", 6, 2, "#9fb8ff"),
            Up::Nova => ("Frost Nova", "❄️", 6, 2, "#9ff3ff"),
            Up::Explosive => ("Explosive Rounds", "🔥", 5, 2, "#ff8a3a"),
            Up::Ricochet => ("Ricochet", "🪃", 4, 1, "#f0e68c"),
            Up::Homing => ("Seeker Rounds", "🛰️", 3, 1, "#d0a0ff"),
            Up::Vampire => ("Vampirism", "🩸", 4, 1, "#d0203a"),
            Up::DashMaster => ("Shadow Step", "💨", 4, 1, "#c0c8ff"),
            Up::Backshot => ("Rear Guard", "↩️", 3, 0, "#e0c090"),
            Up::Thorns => ("Thorn Armor", "🌵", 4, 0, "#9ccf5a"),
        };
        Def {
            name,
            icon,
            max,
            rarity,
            color,
        }
    }

    /// Description of what the *next* level (1-based `lvl`) grants.
    pub fn desc(self, lvl: u8) -> String {
        match self {
            Up::Damage => "+25% bullet damage".into(),
            Up::FireRate => "+16% fire rate".into(),
            Up::Multishot => "+1 projectile per shot".into(),
            Up::Pierce => "Bullets pierce +1 zombie".into(),
            Up::Velocity => "+20% bullet speed & range, +8% damage".into(),
            Up::Speed => "+9% movement speed".into(),
            Up::Vitality => "+25 max HP and heal 35%".into(),
            Up::Regen => "Regenerate +0.8 HP per second".into(),
            Up::Magnet => "+45% pickup radius".into(),
            Up::Crit => "+10% chance to crit for 2.5x".into(),
            Up::Blades => {
                if lvl == 1 {
                    "Summon 2 spectral blades that orbit you".into()
                } else {
                    "+1 blade, +20% blade damage".into()
                }
            }
            Up::Lightning => {
                if lvl == 1 {
                    "Lightning strikes & chains between zombies".into()
                } else {
                    "+1 chain, faster strikes, +damage".into()
                }
            }
            Up::Nova => {
                if lvl == 1 {
                    "Pulse a freezing nova that slows & damages".into()
                } else {
                    "Bigger, stronger, more frequent novas".into()
                }
            }
            Up::Explosive => {
                if lvl == 1 {
                    "Bullets explode on impact".into()
                } else {
                    "+30% explosion radius & damage".into()
                }
            }
            Up::Ricochet => "Bullets bounce to another zombie +1".into(),
            Up::Homing => "Bullets curve toward zombies".into(),
            Up::Vampire => "4% chance per kill to heal 3 HP".into(),
            Up::DashMaster => {
                if lvl == 1 {
                    "-20% dash cooldown, dashing damages zombies".into()
                } else {
                    "-20% dash cooldown, +dash damage".into()
                }
            }
            Up::Backshot => "Also fire a shot behind you".into(),
            Up::Thorns => "Zombies that hit you take heavy damage".into(),
        }
    }
}

#[derive(Clone, Default)]
pub struct Levels(pub [u8; 20]);

impl Levels {
    pub fn get(&self, u: Up) -> u8 {
        self.0[u.idx()]
    }
    pub fn f(&self, u: Up) -> f32 {
        self.0[u.idx()] as f32
    }
    pub fn inc(&mut self, u: Up) {
        self.0[u.idx()] = (self.0[u.idx()] + 1).min(u.def().max);
    }
    pub fn maxed(&self, u: Up) -> bool {
        self.get(u) >= u.def().max
    }

    /// Pick up to 3 distinct offers, weighted so superpowers stay exciting but rarer.
    pub fn roll(&self, rng: &mut Rng, luck: f32) -> Vec<Up> {
        let mut pool: Vec<(Up, f32)> = ALL
            .iter()
            .filter(|u| !self.maxed(**u))
            .map(|u| {
                let d = u.def();
                let mut w = match d.rarity {
                    0 => 1.0,
                    1 => 0.65,
                    _ => 0.45 + luck,
                };
                // Make unlocked superpowers more likely to appear again (build synergy).
                if self.get(*u) > 0 && d.rarity == 2 {
                    w *= 1.5;
                }
                (*u, w)
            })
            .collect();
        let mut out = Vec::new();
        while out.len() < 3 && !pool.is_empty() {
            let total: f32 = pool.iter().map(|p| p.1).sum();
            let mut r = rng.f() * total;
            let mut pick = 0;
            for (i, p) in pool.iter().enumerate() {
                r -= p.1;
                if r <= 0.0 {
                    pick = i;
                    break;
                }
            }
            out.push(pool.remove(pick).0);
        }
        out
    }
}
