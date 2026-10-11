//! Game modes. Each mode supplies hooks the sim calls at fixed points, so new
//! modes (Outbreak, Siege) live in their own module instead of as more
//! branches in `game.rs`.

pub mod campaign;
pub mod horde;
pub mod infinite;
pub mod outbreak;
pub mod siege;

use crate::game::{Game, ZKind};
use crate::math::V2;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Infinite,
    /// chapter index
    Campaign(usize),
    /// CoD-style rounds on an authored map (`Game::map_id`)
    Outbreak,
    /// defend the crystal on an authored map (`Game::map_id`)
    Siege,
}

impl Mode {
    pub fn id(self) -> &'static str {
        match self {
            Mode::Infinite => "infinite",
            Mode::Campaign(_) => "campaign",
            Mode::Outbreak => "outbreak",
            Mode::Siege => "siege",
        }
    }

    pub fn hooks(self) -> &'static dyn ModeHooks {
        match self {
            Mode::Campaign(_) => &campaign::Campaign,
            Mode::Outbreak => &outbreak::Outbreak,
            Mode::Siege => &siege::Siege,
            _ => &infinite::Infinite,
        }
    }
}

pub trait ModeHooks {
    /// Run setup after the player and world are reset (banner, map, economy).
    fn on_start(&self, g: &mut Game);
    /// Advance waves / rounds / phases. Host and solo only.
    fn update_waves(&self, g: &mut Game);
    /// A zombie died (host and solo; after loot and effects).
    fn on_kill(&self, _g: &mut Game, _kind: ZKind, _pos: V2) {}
    /// Wave line for the HUD's top-right corner.
    fn hud(&self, g: &Game) -> String;
    /// The run ended in victory (campaign chapter cleared, siege survived).
    fn is_over(&self, g: &Game) -> bool {
        g.victory_t >= 0.0
    }
}
