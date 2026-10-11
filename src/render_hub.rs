//! Home Base visuals: vendor NPCs and other players (drawn as survivors), name
//! plates, speech bubbles, the talk prompt, the monument and the DPS meter.

use crate::game::Game;
use crate::math::*;
use crate::render::PView;
use web_sys::CanvasRenderingContext2d as Ctx;

#[inline]
fn d(x: f32) -> f64 {
    x as f64
}
const FONT: &str = "'Inter','Segoe UI',system-ui,sans-serif";

fn outlined(c: &Ctx, s: &str, x: f32, y: f32, lw: f32) {
    c.set_line_width(d(lw));
    let _ = c.stroke_text(s, d(x), d(y));
    let _ = c.fill_text(s, d(x), d(y));
}

fn wrap(c: &Ctx, s: &str, max_w: f32) -> Vec<String> {
    let mut lines = Vec::new();
    let mut cur = String::new();
    for w in s.split_whitespace() {
        let cand = if cur.is_empty() { w.to_string() } else { format!("{} {}", cur, w) };
        let width = c.measure_text(&cand).map(|m| m.width() as f32).unwrap_or(0.0);
        if width > max_w && !cur.is_empty() {
            lines.push(std::mem::replace(&mut cur, w.to_string()));
        } else {
            cur = cand;
        }
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    lines
}

/// A rounded speech bubble above `p` (world space).
fn bubble(c: &Ctx, p: V2, text: &str, color: &str) {
    c.set_font(&format!("600 11px {}", FONT));
    let lines = wrap(c, text, 200.0);
    let w = lines
        .iter()
        .map(|l| c.measure_text(l).map(|m| m.width() as f32).unwrap_or(0.0))
        .fold(0.0, f32::max)
        + 18.0;
    let h = lines.len() as f32 * 14.0 + 12.0;
    let x = p.x - w * 0.5;
    let y = p.y - 62.0 - h;
    c.set_fill_style_str("rgba(250,248,240,0.95)");
    c.begin_path();
    c.move_to(d(x + 8.0), d(y));
    let _ = c.arc_to(d(x + w), d(y), d(x + w), d(y + h), 8.0);
    let _ = c.arc_to(d(x + w), d(y + h), d(x), d(y + h), 8.0);
    c.line_to(d(p.x + 6.0), d(y + h));
    c.line_to(d(p.x), d(y + h + 8.0));
    c.line_to(d(p.x - 6.0), d(y + h));
    let _ = c.arc_to(d(x), d(y + h), d(x), d(y), 8.0);
    let _ = c.arc_to(d(x), d(y), d(x + w), d(y), 8.0);
    c.close_path();
    c.fill();
    c.set_stroke_style_str(color);
    c.set_line_width(2.0);
    c.stroke();
    c.set_fill_style_str("#1a1c24");
    c.set_text_align("center");
    c.set_text_baseline("middle");
    for (i, l) in lines.iter().enumerate() {
        let _ = c.fill_text(l, d(p.x), d(y + 13.0 + i as f32 * 14.0));
    }
}

impl Game {
    pub fn npc_view(&self, i: usize, t: f32) -> PView {
        let n = &self.hub.as_ref().unwrap().npcs[i];
        PView {
            aim: n.aim + (t * 0.7 + i as f32).sin() * 0.08,
            vel: V2::ZERO,
            walk: n.walk,
            recoil: 0.0,
            blink: false,
            frenzy: false,
            ability: None,
            hurt: 0.0,
            skin: n.skin,
            gun: n.gun,
            hat: n.hat,
            downed: false,
            revive: 0.0,
            pack: 0,
            reload: false,
        }
    }

    pub fn mate_view(&self, i: usize) -> PView {
        let m = &self.hub.as_ref().unwrap().mates[i];
        PView {
            aim: m.aim,
            vel: (m.pos - m.prev) * 120.0,
            walk: m.walk,
            recoil: 0.0,
            blink: false,
            frenzy: false,
            ability: None,
            hurt: 0.0,
            skin: m.skin,
            gun: m.gun,
            hat: m.hat,
            downed: false,
            revive: 0.0,
            pack: 0,
            reload: false,
        }
    }

    /// Name plates, bubbles, monument label and DPS meter (world space, over actors).
    pub fn draw_hub_overlays(&self, c: &Ctx, t: f32) {
        let Some(h) = self.hub.as_ref() else { return };
        c.set_text_align("center");
        c.set_text_baseline("middle");
        c.set_stroke_style_str("rgba(0,0,0,0.85)");
        c.set_line_join("round");
        for n in h.npcs.iter() {
            c.set_font(&format!("800 11px {}", FONT));
            c.set_fill_style_str("#ffd36a");
            outlined(c, n.name, n.pos.x, n.pos.y - 40.0, 3.0);
            if n.line_t > 0.0 && !n.line.is_empty() {
                c.set_global_alpha(d(n.line_t.min(1.0)));
                bubble(c, n.pos, &n.line, "#ffd36a");
                c.set_global_alpha(1.0);
            }
        }
        for m in h.mates.iter() {
            c.set_font(&format!("800 11px {}", FONT));
            c.set_fill_style_str("#9cffb8");
            c.set_stroke_style_str("rgba(0,0,0,0.85)");
            outlined(c, &m.name, m.pos.x, m.pos.y - 40.0, 3.0);
            if m.bubble_t > 0.0 {
                c.set_global_alpha(d(m.bubble_t.min(1.0)));
                bubble(c, m.pos, &m.bubble, "#9cffb8");
                c.set_global_alpha(1.0);
            }
        }
        // the memorial: names of the best runs live in the UI panel
        c.set_font(&format!("900 13px {}", FONT));
        c.set_fill_style_str("#e8f0ff");
        c.set_stroke_style_str("rgba(0,0,0,0.85)");
        outlined(c, "🏆 Hall of Survivors", 0.0, -60.0, 3.0);
        // firing range
        let [x0, y0, x1, _] = h.range;
        let dps = self.dps();
        c.set_font(&format!("800 13px {}", FONT));
        c.set_fill_style_str("#ffb070");
        outlined(c, "FIRING RANGE", (x0 + x1) * 0.5, y0 + 30.0, 3.0);
        if dps > 0.5 {
            c.set_font(&format!("900 {}px {}", 18.0 + (t * 8.0).sin().abs() * 2.0, FONT));
            c.set_fill_style_str("#ffffff");
            outlined(c, &format!("{:.0} DPS", dps), (x0 + x1) * 0.5, y0 + 56.0, 4.0);
        }
    }

    /// "[E] Talk to ..." prompt (screen space).
    pub fn draw_hub_prompt(&self, c: &Ctx, w: f32, cy: f32) {
        let Some(h) = self.hub.as_ref() else { return };
        if h.open.is_some() {
            return;
        }
        let Some((id, pos)) = self.hub_target() else { return };
        let label = if id == "monument" {
            "the Hall of Survivors".to_string()
        } else {
            h.npcs.iter().find(|n| n.id == id).map(|n| n.name.to_string()).unwrap_or_default()
        };
        let key = if self.input.is_touch { "Tap USE" } else { "[E]" };
        let verb = if id == "monument" { "Read" } else if id == "dispatcher" { "Missions with" } else { "Talk to" };
        let text = format!("{} {} {}", key, verb, label);
        let sp = v2(w * 0.5, cy) + (pos - self.cam) * self.zoom;
        c.set_text_align("center");
        c.set_text_baseline("middle");
        c.set_font(&format!("800 14px {}", FONT));
        let tw = c.measure_text(&text).map(|m| m.width() as f32).unwrap_or(200.0);
        c.set_fill_style_str("rgba(8,10,18,0.8)");
        c.fill_rect(d(sp.x - tw * 0.5 - 12.0), d(sp.y + 34.0), d(tw + 24.0), 28.0);
        c.set_fill_style_str("#ffffff");
        let _ = c.fill_text(&text, d(sp.x), d(sp.y + 48.0));
    }
}
