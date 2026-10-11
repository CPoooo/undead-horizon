//! Photo mode: from the pause menu, a free camera over the frozen world with
//! the HUD hidden, plus vignette and letterbox. Filters and the capture
//! itself live in the UI (`ui/app.js`), which bakes the filter into the PNG.
//!
//! Commands: `photo|on`, `photo|off`, `photo|pan=dx,dy` (screen px),
//! `photo|zoom=k` (0.5..3), `photo|player=0|1`, `photo|vig=0..1`, `photo|bars=0|1`.

use crate::game::{Game, Phase};
use crate::math::*;
use web_sys::CanvasRenderingContext2d as Ctx;

#[derive(Clone, Debug)]
pub struct Photo {
    /// camera offset from the player (world units)
    pub off: V2,
    pub zoom: f32,
    pub hide_player: bool,
    pub vignette: f32,
    pub bars: bool,
}

/// How far the camera may wander from the player (keeps the world generated).
pub const MAX_OFF: f32 = 900.0;

impl Game {
    pub fn photo_cmd(&mut self, a: &str) {
        match a {
            "on" if self.phase == Phase::Paused => {
                self.photo = Some(Photo { off: V2::ZERO, zoom: 1.0, hide_player: false, vignette: 0.35, bars: false });
            }
            "off" => self.photo = None,
            _ => {}
        }
        let zoom_now = self.zoom;
        let Some(p) = self.photo.as_mut() else { return };
        let (k, v) = a.split_once('=').unwrap_or((a, ""));
        match k {
            "pan" => {
                let (x, y) = v.split_once(',').unwrap_or(("0", "0"));
                let dv = v2(x.parse().unwrap_or(0.0), y.parse().unwrap_or(0.0)) * (1.0 / zoom_now.max(0.1));
                p.off += dv;
                if p.off.len() > MAX_OFF {
                    p.off = p.off.norm() * MAX_OFF;
                }
            }
            "zoom" => p.zoom = v.parse::<f32>().unwrap_or(1.0).clamp(0.5, 3.0),
            "player" => p.hide_player = v == "0",
            "vig" => p.vignette = v.parse::<f32>().unwrap_or(0.0).clamp(0.0, 1.0),
            "bars" => p.bars = v == "1",
            _ => {}
        }
    }

    /// Screen-space photo overlays: vignette and cinematic bars.
    pub(crate) fn draw_photo_overlay(&self, c: &Ctx, w: f32, h: f32) {
        let Some(p) = &self.photo else { return };
        if p.vignette > 0.0 {
            let r = (w * w + h * h).sqrt() * 0.5;
            if let Ok(g) = c.create_radial_gradient(
                (w * 0.5) as f64,
                (h * 0.5) as f64,
                (r * 0.35) as f64,
                (w * 0.5) as f64,
                (h * 0.5) as f64,
                r as f64,
            ) {
                let _ = g.add_color_stop(0.0, "rgba(0,0,0,0)");
                let _ = g.add_color_stop(1.0, &format!("rgba(0,0,0,{:.2})", 0.85 * p.vignette));
                c.set_fill_style_canvas_gradient(&g);
                c.fill_rect(0.0, 0.0, w as f64, h as f64);
            }
        }
        if p.bars {
            // 2.39:1 letterbox
            let bar = ((h - w / 2.39) * 0.5).max(0.0);
            c.set_fill_style_str("#000");
            c.fill_rect(0.0, 0.0, w as f64, bar as f64);
            c.fill_rect(0.0, (h - bar) as f64, w as f64, bar as f64);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn photo_mode_only_from_pause_and_camera_is_leashed() {
        let mut g = Game::new(1);
        g.start("mode=infinite");
        g.phase = Phase::Playing;
        g.photo_cmd("on");
        assert!(g.photo.is_none(), "only from the pause menu");
        g.phase = Phase::Paused;
        g.photo_cmd("on");
        g.zoom = 1.0;
        g.photo_cmd("pan=5000,0");
        let p = g.photo.as_ref().unwrap();
        assert!((p.off.len() - MAX_OFF).abs() < 1.0);
        g.photo_cmd("zoom=9");
        g.photo_cmd("player=0");
        let p = g.photo.as_ref().unwrap();
        assert_eq!(p.zoom, 3.0);
        assert!(p.hide_player);
        g.photo_cmd("off");
        assert!(g.photo.is_none());
    }
}
