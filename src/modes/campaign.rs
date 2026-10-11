//! Campaign: a fixed number of waves per chapter, ending in the chapter boss.

use super::ModeHooks;
use crate::content::CHAPTERS;
use crate::game::{Banner, Game, ZKind};
use crate::math::V2;

pub struct Campaign;

impl ModeHooks for Campaign {
    fn on_start(&self, g: &mut Game) {
        let ch = g.chapter();
        g.banner = Some(Banner {
            title: format!("CHAPTER {}", ch + 1),
            sub: format!("{} · {}", CHAPTERS[ch].title, g.difficulty().name),
            t: 0.0,
            dur: 3.0,
            color: "#ffd36a",
        });
    }

    fn update_waves(&self, g: &mut Game) {
        g.horde_waves();
    }

    fn on_kill(&self, g: &mut Game, kind: ZKind, _pos: V2) {
        // the host decides the win; clients replay it from E_VICTORY
        if kind != ZKind::Boss || g.is_client() {
            return;
        }
        let bosses_left =
            g.wave.boss_pending || g.zombies.iter().any(|z| !z.dead && z.kind == ZKind::Boss);
        if g.wave.n >= CHAPTERS[g.chapter()].waves && !bosses_left {
            if let Some(net) = &mut g.net {
                net.event(|w| {
                    w.u8(crate::net::E_VICTORY);
                    w.u16(0);
                });
            }
            g.campaign_victory();
        }
    }

    fn hud(&self, g: &Game) -> String {
        format!("WAVE {}/{}", g.wave.n.max(1), CHAPTERS[g.chapter()].waves)
    }
}
