//! Treasure chests: bosses (and the odd brute) drop one. Opening it spins a
//! slot machine that reveals 1, 3 or 5 upgrades. The first chest of a run is
//! always generous. Upgrades are applied the moment it opens; the reels are
//! presentation only: a small loot strip near the top of the screen that never
//! pauses the game or waits for input, and fades out on its own.

use crate::audio::Sfx;
use crate::game::Game;
use crate::upgrades::{Evo, Up, ALL};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ChestPick {
    Up(Up),
    Evo(Evo),
}

impl ChestPick {
    pub fn icon(self) -> &'static str {
        match self {
            ChestPick::Up(u) => u.def().icon,
            ChestPick::Evo(e) => e.def().icon,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            ChestPick::Up(u) => u.def().name,
            ChestPick::Evo(e) => e.def().name,
        }
    }
    pub fn color(self) -> &'static str {
        match self {
            ChestPick::Up(u) => u.def().color,
            ChestPick::Evo(e) => e.def().color,
        }
    }
}

pub struct Chest {
    pub picks: Vec<ChestPick>,
    pub coins: u32,
    pub t: f32,
}

/// Seconds per reel before it stops (reels stop one after another).
pub const REEL_T: f32 = 0.4;
/// Before the first reel stops.
pub const LEAD_T: f32 = 0.35;
/// How long the result stays up once revealed, then how long it fades.
pub const HOLD_T: f32 = 2.6;
pub const FADE_T: f32 = 0.45;

impl Chest {
    /// When reel `k` (0-based) lands.
    pub fn stop_t(k: usize) -> f32 {
        LEAD_T + REEL_T * (k + 1) as f32
    }
    pub fn revealed(&self) -> bool {
        self.t >= Self::stop_t(self.picks.len().max(1) - 1)
    }
    /// Strip opacity: slides in, holds, fades out.
    pub fn alpha(&self) -> f32 {
        let end = Self::stop_t(self.picks.len().max(1) - 1) + HOLD_T;
        ((self.t / 0.2).min(1.0) * (1.0 - (self.t - end) / FADE_T)).clamp(0.0, 1.0)
    }
    pub fn finished(&self) -> bool {
        self.t >= Self::stop_t(self.picks.len().max(1) - 1) + HOLD_T + FADE_T
    }
}

impl Game {
    fn chest_count(&mut self) -> usize {
        self.chests_opened += 1;
        if self.chests_opened == 1 {
            return 3;
        }
        let luck = (self.wave.n as f32 * 0.003).min(0.08);
        let r = self.rng.f();
        if r < 0.05 + luck * 0.5 {
            5
        } else if r < 0.32 + luck {
            3
        } else {
            1
        }
    }

    /// Open a chest for the local player.
    pub fn open_chest(&mut self) {
        let n = self.chest_count();
        let mut picks = Vec::new();
        // an evolution, when one is ready, takes the first reel
        if let Some(e) = self.evo_ready() {
            self.evolved.push(e);
            picks.push(ChestPick::Evo(e));
            self.say("evolution");
            self.banner = Some(crate::game::Banner {
                title: "EVOLUTION!".into(),
                sub: format!("{}: {}", e.def().name, e.def().desc),
                t: 0.0,
                dur: 3.2,
                color: e.def().color,
            });
        }
        while picks.len() < n {
            // favor powers you already have (evolving a build), then anything open
            let owned: Vec<Up> = ALL
                .iter()
                .copied()
                .filter(|u| self.ups.get(*u) > 0 && !self.ups.maxed(*u) && !self.banished.contains(u))
                .collect();
            let pick = if !owned.is_empty() && self.rng.chance(0.65) {
                owned[self.rng.below(owned.len())]
            } else {
                let offers = self.ups.roll(&mut self.rng, 0.15, &self.banished);
                match offers.first() {
                    Some(u) => *u,
                    None => break,
                }
            };
            self.apply_upgrade(pick);
            picks.push(ChestPick::Up(pick));
        }
        let coins = ((15 + 10 * n as u32) as f32 * self.difficulty().coins) as u32;
        self.coins += coins;
        self.chest = Some(Chest { picks, coins, t: 0.0 });
        self.haptic(120, 0.5);
        self.audio.play(Sfx::UltReady);
    }

    /// Advance the loot strip; it removes itself once it has faded.
    pub fn tick_chest(&mut self, dt: f32) {
        let Some(c) = &mut self.chest else { return };
        let before = c.t;
        c.t += dt;
        // a clack as each reel lands
        let n = c.picks.len();
        let finished = c.finished();
        for k in 0..n {
            let stop = Chest::stop_t(k);
            if before < stop && c.t >= stop {
                self.audio.play(if k + 1 == n { Sfx::LevelUp } else { Sfx::UiSelect });
            }
        }
        if finished {
            self.chest = None;
        }
    }
}
