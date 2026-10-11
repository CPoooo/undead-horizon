//! Where HUD pieces go on screen. Desktop keeps the classic layout; phones
//! (touch, or a narrow/short screen) get the **compact** layout, where nothing
//! overlaps:
//!
//! - top-left: level badge + HP bar, ending above the HTML pause/chat row
//!   (CSS `top: 52px`, 40px tall)
//! - top-right: wave / remaining / stats / score in a tight column
//! - the **status stack**: boss and crystal bars (labels inside the bars) plus
//!   a phase line. Full width under the top rows in portrait; centred between
//!   the side panels in landscape
//! - under the stack: teammate cards and kill feed (left), minimap (right)
//! - bottom-right: the HTML thumb cluster (`#touch-ui`), which this layout keeps
//!   clear. The Siege build bar sits above it (portrait) or bottom-centre
//!   (landscape)

use crate::game::Game;

pub const ROW_H: f32 = 22.0; // a stack bar row: 16px bar + gap
pub const BAR_H: f32 = 16.0;
/// rough size of the HTML thumb cluster (`#touch-ui` in ui/style.css)
pub const CLUSTER_W: f32 = 236.0;
pub const CLUSTER_H: f32 = 168.0;

#[derive(Clone, Copy, Debug)]
pub struct HudLayout {
    pub compact: bool,
    /// compact and wide: the stack sits top-centre between the side panels
    pub landscape: bool,
    pub hp_w: f32,
    pub stack_x: f32,
    pub stack_y: f32,
    pub stack_w: f32,
    /// y below the stack's reserved rows (boss, crystal, phase line)
    pub stack_end: f32,
    /// where left-column extras (teammates, kill feed) start
    pub left_y: f32,
    pub minimap: (f32, f32, f32),
}

impl Game {
    pub fn hud_layout(&self) -> HudLayout {
        let (w, h) = (self.screen.x, self.screen.y);
        let compact = self.input.is_touch || w < 640.0 || h < 500.0;
        if !compact {
            let r = 76.0;
            return HudLayout {
                compact,
                landscape: false,
                hp_w: 250.0,
                stack_x: w * 0.5 - (w * 0.4).min(440.0) * 0.5,
                stack_y: 34.0,
                stack_w: (w * 0.4).min(440.0),
                stack_end: 120.0,
                left_y: 92.0,
                minimap: (w - r - 20.0, h - r - 30.0, r),
            };
        }
        let landscape = w > h && w >= 600.0;
        let hp_w = if landscape { 170.0 } else { (w * 0.4).min(170.0) };
        let r = 46.0;
        if landscape {
            // between the HP panel (left) and the wave column (right)
            let x0 = 50.0 + hp_w + 18.0;
            let x1 = w - 190.0;
            let stack_w = (x1 - x0).clamp(160.0, 480.0);
            let stack_x = (x0 + x1) * 0.5 - stack_w * 0.5;
            let stack_end = 12.0 + ROW_H * 2.0 + 18.0;
            HudLayout {
                compact,
                landscape,
                hp_w,
                stack_x,
                stack_y: 12.0,
                stack_w,
                stack_end,
                left_y: 100.0,
                minimap: (w - r - 14.0, 112.0 + r, r),
            }
        } else {
            let stack_y = 112.0;
            let stack_end = stack_y + ROW_H * 2.0 + 18.0;
            HudLayout {
                compact,
                landscape,
                hp_w,
                stack_x: 12.0,
                stack_y,
                stack_w: w - 24.0,
                stack_end,
                left_y: stack_end + 8.0,
                minimap: (w - r - 12.0, stack_end + 8.0 + r, r),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::game::Game;
    use crate::math::v2;

    /// Nothing in the compact layout overlaps, on common phone sizes.
    #[test]
    fn compact_layout_has_no_overlaps_on_phones() {
        for (w, h) in [(390.0, 844.0), (360.0, 640.0), (844.0, 390.0), (667.0, 375.0), (430.0, 932.0), (932.0, 430.0)] {
            let mut g = Game::new(1);
            g.screen = v2(w, h);
            g.input.is_touch = true;
            let l = g.hud_layout();
            assert!(l.compact);
            // the stack stays on screen and clear of the HP panel / pause row
            assert!(l.stack_x >= 0.0 && l.stack_x + l.stack_w <= w, "{w}x{h}");
            if l.landscape {
                assert!(l.stack_x > 50.0 + l.hp_w, "stack clears the HP panel at {w}x{h}");
                assert!(l.stack_x + l.stack_w < w - 150.0, "stack clears the wave column at {w}x{h}");
            } else {
                assert!(l.stack_y >= 52.0 + 40.0, "stack below the pause row at {w}x{h}");
            }
            // minimap clear of the stack and of the thumb cluster
            let (cx, cy, r) = l.minimap;
            assert!(cy - r >= l.stack_end || cx - r > l.stack_x + l.stack_w, "minimap vs stack at {w}x{h}");
            assert!(cy + r < h - super::CLUSTER_H, "minimap vs thumb cluster at {w}x{h}");
            // the build bar never sits under the thumb cluster
            g.siege = None;
            let bar = g.build_bar();
            for b in bar.iter() {
                let under_cluster = b.0 + b.2 > w - super::CLUSTER_W && b.1 + b.3 > h - super::CLUSTER_H;
                assert!(!under_cluster, "build slot under the buttons at {w}x{h}: {b:?}");
                assert!(b.0 >= 0.0 && b.0 + b.2 <= w && b.1 > l.stack_end, "build slot on screen at {w}x{h}");
            }
        }
    }

    /// The level-up tray and the chest strip leave the player (screen centre)
    /// visible and stay on screen, on phones and desktop.
    #[test]
    fn levelup_tray_and_chest_strip_stay_clear() {
        let sizes = [(390.0, 844.0, true), (360.0, 640.0, true), (844.0, 390.0, true), (667.0, 375.0, true),
            (430.0, 932.0, true), (932.0, 430.0, true), (1280.0, 720.0, false), (1920.0, 1080.0, false)];
        for (w, h, touch) in sizes {
            let mut g = Game::new(1);
            g.screen = v2(w, h);
            g.input.is_touch = touch;
            let l = g.hud_layout();
            let cards = g.card_rects();
            let btns = g.levelup_buttons();
            let (_, title_y, _) = g.levelup_title();
            let top = cards[0].1.min(btns[0].1).min(title_y - 16.0);
            // the title and the tray stay off the player
            assert!(top > h * 0.5 + 24.0, "level-up tray covers the player at {w}x{h} (top {top})");
            for r in cards.iter().chain(btns.iter()) {
                assert!(r.0 >= 0.0 && r.0 + r.2 <= w && r.1 + r.3 <= h, "tray piece off screen at {w}x{h}: {r:?}");
            }
            // cards and buttons don't overlap each other
            for (i, a) in cards.iter().chain(btns.iter()).enumerate() {
                for b in cards.iter().chain(btns.iter()).skip(i + 1) {
                    let hit = a.0 < b.0 + b.2 && b.0 < a.0 + a.2 && a.1 < b.1 + b.3 && b.1 < a.1 + a.3;
                    assert!(!hit, "tray pieces overlap at {w}x{h}: {a:?} {b:?}");
                }
            }
            for n in [1, 3, 5] {
                let (x, y, cw, ch) = g.chest_rect(n);
                assert!(x >= 0.0 && x + cw <= w, "chest strip off screen at {w}x{h}");
                assert!(y + ch < h * 0.5 - 24.0, "chest strip covers the player at {w}x{h}");
                // fits its tiles
                let tile = Game::chest_tile(l.compact);
                assert!(cw >= n as f32 * tile + (n - 1) as f32 * 6.0, "chest tiles don't fit at {w}x{h} n={n}");
                if l.compact {
                    let (mx, my, mr) = l.minimap;
                    let clear_map = x + cw <= mx - mr || y >= my + mr || y + ch <= my - mr;
                    assert!(clear_map, "chest strip over the minimap at {w}x{h}");
                    assert!(y >= 92.0 || l.landscape, "chest strip under the pause row at {w}x{h}");
                    if l.landscape {
                        assert!(x > 50.0 + l.hp_w && x + cw < w - 150.0, "chest strip vs side panels at {w}x{h}");
                    }
                }
            }
        }
    }
}
