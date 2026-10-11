//! Infinite: endless horde waves, a boss every 5th, ranked on the leaderboard.

use super::ModeHooks;
use crate::content::GUNS;
use crate::game::{Banner, Game};

pub struct Infinite;

impl ModeHooks for Infinite {
    fn on_start(&self, g: &mut Game) {
        let m = g.mutators;
        let muts: Vec<&str> = [
            (m.bighead, "Big Heads"),
            (m.glass, "Glass Cannon"),
            (m.fast, "Fast Horde"),
            (m.onehit, "One Hit"),
            (m.floaty, "Low Gravity"),
        ]
        .iter()
        .filter(|x| x.0)
        .map(|x| x.1)
        .collect();
        let title = if !g.daily.is_empty() { "DAILY RUN" } else if m.any() { "CUSTOM RUN" } else { "SURVIVE" };
        let extra = if muts.is_empty() { String::new() } else { format!(" · {}", muts.join(", ")) };
        g.banner = Some(Banner {
            title: title.into(),
            sub: format!("Infinite · {} · {}{}", g.difficulty().name, GUNS[g.gun].name, extra),
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
