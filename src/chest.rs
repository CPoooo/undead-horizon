//! Treasure chests: bosses (and the odd brute) drop one. Opening it spins a
//! slot machine that reveals 1, 3 or 5 upgrades. The first chest of a run is
//! always generous. Upgrades are applied the moment it opens; the reels are
//! presentation, and the overlay reuses the level-up phase (so co-op keeps
//! running underneath).

use crate::audio::Sfx;
use crate::game::{Game, Phase};
use crate::upgrades::{Up, ALL};

pub struct Chest {
    pub picks: Vec<Up>,
    pub coins: u32,
    pub t: f32,
}

/// Seconds per reel before it stops (reels stop one after another).
pub const REEL_T: f32 = 0.55;

impl Chest {
    pub fn revealed(&self) -> bool {
        self.t > 0.6 + REEL_T * self.picks.len() as f32
    }
}

impl Game {
    fn chest_count(&mut self) -> usize {
        self.chests_opened += 1;
        if self.chests_opened == 1 {
            return 3;
        }
        let luck = (self.wave.n as f32 * 0.004).min(0.1);
        let r = self.rng.f();
        if r < 0.10 + luck {
            5
        } else if r < 0.45 + luck {
            3
        } else {
            1
        }
    }

    /// Open a chest for the local player.
    pub fn open_chest(&mut self) {
        let n = self.chest_count();
        let mut picks = Vec::new();
        for _ in 0..n {
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
            picks.push(pick);
        }
        let coins = ((15 + 10 * n as u32) as f32 * self.difficulty().coins) as u32;
        self.coins += coins;
        self.chest = Some(Chest { picks, coins, t: 0.0 });
        self.levelup_t = 0.0;
        self.phase = Phase::LevelUp;
        self.audio.play(Sfx::UltReady);
    }

    pub fn tick_chest(&mut self, dt: f32) {
        let Some(c) = &mut self.chest else { return };
        let before = c.t;
        c.t += dt;
        // a clack as each reel lands
        let n = c.picks.len();
        for k in 0..n {
            let stop = 0.6 + REEL_T * (k + 1) as f32;
            if before < stop && c.t >= stop {
                self.audio.play(if k + 1 == n { Sfx::LevelUp } else { Sfx::UiSelect });
            }
        }
    }

    /// Close the chest once everything is revealed (any key, click or tap).
    pub fn close_chest(&mut self) -> bool {
        match &self.chest {
            Some(c) if c.revealed() => {
                self.chest = None;
                self.phase = Phase::Playing;
                self.last_ms = -1.0;
                self.player.iframes = self.player.iframes.max(0.8);
                true
            }
            Some(c) => {
                // impatient: skip the spin
                let full = 0.61 + REEL_T * c.picks.len() as f32;
                if let Some(c) = &mut self.chest {
                    c.t = c.t.max(full);
                }
                true
            }
            None => false,
        }
    }
}
