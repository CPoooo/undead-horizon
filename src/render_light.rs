//! Light mask: darkness is painted into a half-resolution offscreen canvas,
//! lights are cut out of it with `destination-out`, and the result is drawn
//! over the world. The player carries a soft glow and a flashlight cone that
//! follows the aim; muzzle flashes, explosions and novas light up the night.

use crate::game::{Game, Phase};
use crate::math::*;
use std::cell::RefCell;
use wasm_bindgen::JsCast;
use web_sys::{CanvasRenderingContext2d as Ctx, HtmlCanvasElement};

thread_local! {
    static MASK: RefCell<Option<(HtmlCanvasElement, Ctx)>> = const { RefCell::new(None) };
}

const SCALE: f32 = 0.5;

#[inline]
fn d(x: f32) -> f64 {
    x as f64
}

fn mask() -> Option<(HtmlCanvasElement, Ctx)> {
    MASK.with(|m| {
        if m.borrow().is_none() {
            let doc = web_sys::window()?.document()?;
            let cv: HtmlCanvasElement = doc.create_element("canvas").ok()?.dyn_into().ok()?;
            let cx: Ctx = cv.get_context("2d").ok()??.dyn_into().ok()?;
            *m.borrow_mut() = Some((cv, cx));
        }
        m.borrow().clone()
    })
}

impl Game {
    /// Paint darkness with light cut out of it. Returns false if unavailable.
    pub fn draw_light_mask(&self, c: &Ctx, w: f32, h: f32, ppos: V2, dark: f32, cy: f32) -> bool {
        let Some((cv, m)) = mask() else { return false };
        let mw = (w * SCALE).ceil().max(1.0) as u32;
        let mh = (h * SCALE).ceil().max(1.0) as u32;
        if cv.width() != mw || cv.height() != mh {
            cv.set_width(mw);
            cv.set_height(mh);
        }
        let _ = m.set_transform(d(SCALE), 0.0, 0.0, d(SCALE), 0.0, 0.0);
        let _ = m.set_global_composite_operation("source-over");
        m.set_global_alpha(1.0);
        m.clear_rect(0.0, 0.0, d(w), d(h));
        m.set_fill_style_str(&format!("rgba(3,5,14,{:.3})", (dark + 0.12).min(0.92)));
        m.fill_rect(0.0, 0.0, d(w), d(h));
        let _ = m.set_global_composite_operation("destination-out");
        let z = self.zoom;
        let to_screen = |p: V2| v2(w * 0.5, cy) + (p - self.cam) * z;
        let glow = |pos: V2, r: f32, a: f32| {
            let sp = to_screen(pos);
            let rr = r * z;
            if let Ok(g) = m.create_radial_gradient(d(sp.x), d(sp.y), 0.0, d(sp.x), d(sp.y), d(rr)) {
                let _ = g.add_color_stop(0.0, &format!("rgba(0,0,0,{:.3})", a));
                let _ = g.add_color_stop(0.6, &format!("rgba(0,0,0,{:.3})", a * 0.6));
                let _ = g.add_color_stop(1.0, "rgba(0,0,0,0)");
                m.set_fill_style_canvas_gradient(&g);
                m.fill_rect(d(sp.x - rr), d(sp.y - rr), d(rr * 2.0), d(rr * 2.0));
            }
        };
        let cone = |pos: V2, aim: f32, len: f32, half: f32, a: f32| {
            let sp = to_screen(pos);
            let l = len * z;
            if let Ok(g) = m.create_radial_gradient(d(sp.x), d(sp.y), d(10.0 * z), d(sp.x), d(sp.y), d(l)) {
                let _ = g.add_color_stop(0.0, &format!("rgba(0,0,0,{:.3})", a));
                let _ = g.add_color_stop(0.7, &format!("rgba(0,0,0,{:.3})", a * 0.55));
                let _ = g.add_color_stop(1.0, "rgba(0,0,0,0)");
                m.set_fill_style_canvas_gradient(&g);
                m.begin_path();
                m.move_to(d(sp.x), d(sp.y));
                let _ = m.arc(d(sp.x), d(sp.y), d(l), d(aim - half), d(aim + half));
                m.close_path();
                m.fill();
            }
        };
        let alive = !matches!(self.phase, Phase::Dying | Phase::GameOver) && !self.player.dead;
        if alive {
            glow(ppos, 230.0, 1.0);
            if self.phase != Phase::Menu {
                cone(ppos, self.player.aim, 560.0, 0.42, 0.9);
            }
        }
        if let Some(n) = &self.net {
            for r in n.remotes.iter().filter(|r| r.present && !r.state.dead()) {
                let rp = r.render_pos(self.real_time);
                glow(rp, 190.0, 0.85);
                cone(rp, r.state.aim, 480.0, 0.42, 0.75);
            }
        }
        for l in self.lights.iter() {
            let k = (l.life / l.max).clamp(0.0, 1.0);
            glow(l.pos, l.r * 1.6, k);
        }
        // the lobby pedestal glows
        if self.phase == Phase::Menu {
            glow(ppos, 320.0, 1.0);
        }
        // burning zombies and lit barrels are little light sources too
        let mut n = 0;
        for zb in self.zombies.iter() {
            if zb.st.burn_t > 0.0 && n < 24 {
                glow(zb.pos, 90.0, 0.6);
                n += 1;
            }
        }
        m.set_global_alpha(1.0);
        let _ = m.set_global_composite_operation("source-over");
        let _ = c.draw_image_with_html_canvas_element_and_dw_and_dh(&cv, 0.0, 0.0, d(w), d(h));
        true
    }
}
