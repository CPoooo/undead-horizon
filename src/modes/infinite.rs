//! Infinite: endless horde waves, a boss every 5th, ranked on the leaderboard.

use super::ModeHooks;
use crate::content::GUNS;
use crate::game::{Banner, Game};

pub struct Infinite;

impl ModeHooks for Infinite {
    fn on_start(&self, g: &mut Game) {
        g.banner = Some(Banner {
            title: "SURVIVE".into(),
            sub: format!("Infinite · {} · {}", g.difficulty().name, GUNS[g.gun].name),
            t: 0.0,
            dur: 2.6,
            color: "#e8f0ff",
        });
    }

    fn update_waves(&self, g: &mut Game) {
        g.horde_waves();
    }

    fn hud(&self, g: &Game) -> String {
        format!("WAVE {}", g.wave.n.max(1))
    }

    fn is_over(&self, _g: &Game) -> bool {
        false
    }
}
