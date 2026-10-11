//! Home Base visuals: vendor NPCs and other players (drawn as survivors), name
//! plates, speech bubbles, the talk prompt, the monument and the DPS meter.

use crate::game::Game;
use crate::math::*;
use crate::render::PView;
use crate::modes::hub::{DROP_HOLD, DROP_R};
use std::f64::consts::TAU as TAU64;
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
            c.set_fill_style_str(if m.party { "#9cffb8" } else { "#cfe3ff" });
            c.set_stroke_style_str("rgba(0,0,0,0.85)");
            outlined(c, &m.name, m.pos.x, m.pos.y - 40.0, 3.0);
            if m.bubble_t > 0.0 {
                c.set_global_alpha(d(m.bubble_t.min(1.0)));
                bubble(c, m.pos, &m.bubble, "#9cffb8");
                c.set_global_alpha(1.0);
            } else if m.emote_t > 0.0 {
                let pop = 1.0 + 0.3 * ((2.5 - m.emote_t) * 8.0).min(1.0).sin();
                c.set_global_alpha(d(m.emote_t.min(1.0)));
                c.set_font(&format!("{}px 'Segoe UI Emoji','Apple Color Emoji','Noto Color Emoji',sans-serif", (26.0 * pop).round()));
                c.set_fill_style_str("#ffffff");
                let _ = c.fill_text(crate::content::EMOTES[m.emote.min(crate::content::EMOTES.len() - 1)].1, d(m.pos.x), d(m.pos.y - 60.0));
                c.set_global_alpha(1.0);
            }
        }
        // the Outskirts gate, the base turret and its zaps
        if h.outskirts[2] > h.outskirts[0] {
            c.set_font(&format!("900 14px {}", FONT));
            c.set_fill_style_str("#ff9a5a");
            outlined(c, "THE OUTSKIRTS →", 1110.0, -150.0, 3.0);
            c.set_fill_style_str("#9cffb8");
            outlined(c, "← HOME BASE", 890.0, 150.0, 3.0);
            c.set_font(&format!("700 10px {}", FONT));
            c.set_fill_style_str("#c8d2e8");
            outlined(c, "weapons free · supply drops · no game over", 1110.0, -130.0, 2.5);
            let tp = v2(940.0, -150.0);
            c.set_fill_style_str("#2a3242");
            c.begin_path();
            let _ = c.arc(d(tp.x), d(tp.y), 14.0, 0.0, TAU64);
            c.fill();
            c.set_fill_style_str("#7ad7ff");
            c.begin_path();
            let _ = c.arc(d(tp.x), d(tp.y), d(5.0 + (t * 4.0).sin().abs() * 2.0), 0.0, TAU64);
            c.fill();
            for (a, b, k) in h.zaps.iter() {
                c.set_global_alpha(d((k / 0.25).clamp(0.0, 1.0)));
                c.set_stroke_style_str("#bfefff");
                c.set_line_width(3.0);
                c.begin_path();
                c.move_to(d(a.x), d(a.y));
                let mid = (*a + *b) * 0.5 + v2((t * 91.0).sin() * 14.0, (t * 77.0).cos() * 14.0);
                c.line_to(d(mid.x), d(mid.y));
                c.line_to(d(b.x), d(b.y));
                c.stroke();
                c.set_global_alpha(1.0);
            }
            c.set_stroke_style_str("rgba(0,0,0,0.85)");
        }
        if let Some(dr) = &h.drop {
            let pulse = 0.5 + 0.5 * (t * 3.0).sin();
            // light pillar
            let _ = c.set_global_composite_operation("lighter");
            c.set_global_alpha(d(0.22 + 0.12 * pulse));
            c.set_fill_style_str("#ffd36a");
            c.fill_rect(d(dr.pos.x - 10.0), d(dr.pos.y - 220.0), 20.0, 220.0);
            c.set_global_alpha(0.12);
            c.begin_path();
            let _ = c.arc(d(dr.pos.x), d(dr.pos.y), d(DROP_R), 0.0, TAU64);
            c.fill();
            let _ = c.set_global_composite_operation("source-over");
            c.set_global_alpha(1.0);
            // the crate
            c.set_fill_style_str("#6b4a22");
            c.fill_rect(d(dr.pos.x - 16.0), d(dr.pos.y - 16.0), 32.0, 26.0);
            c.set_stroke_style_str("#ffd36a");
            c.set_line_width(2.0);
            c.stroke_rect(d(dr.pos.x - 16.0), d(dr.pos.y - 16.0), 32.0, 26.0);
            // hold ring
            c.set_line_width(4.0);
            c.set_stroke_style_str("rgba(255,211,106,0.25)");
            c.begin_path();
            let _ = c.arc(d(dr.pos.x), d(dr.pos.y), d(DROP_R), 0.0, TAU64);
            c.stroke();
            c.set_stroke_style_str("#ffd36a");
            c.begin_path();
            let k = (dr.held / DROP_HOLD).clamp(0.0, 1.0) as f64;
            let _ = c.arc(d(dr.pos.x), d(dr.pos.y), d(DROP_R), -std::f64::consts::FRAC_PI_2, -std::f64::consts::FRAC_PI_2 + TAU64 * k);
            c.stroke();
            c.set_font(&format!("900 13px {}", FONT));
            c.set_fill_style_str("#ffd36a");
            c.set_stroke_style_str("rgba(0,0,0,0.85)");
            outlined(c, &format!("SUPPLY DROP · {:.0}s", (DROP_HOLD - dr.held).max(0.0)), dr.pos.x, dr.pos.y - 240.0, 3.0);
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

    /// "[E] Talk to ..." prompt (screen space), plus an edge arrow to an
    /// off-screen Supply Drop.
    pub fn draw_hub_prompt(&self, c: &Ctx, w: f32, cy: f32) {
        let Some(h) = self.hub.as_ref() else { return };
        if let Some(dr) = &h.drop {
            let sp = v2(w * 0.5, cy) + (dr.pos - self.cam) * self.zoom;
            let hgt = cy * 2.0;
            if sp.x < 0.0 || sp.y < 0.0 || sp.x > w || sp.y > hgt {
                let dir = (sp - v2(w * 0.5, cy)).norm();
                let edge = v2(w * 0.5, cy) + dir * ((w * 0.5 - 40.0).min(cy - 40.0));
                c.save();
                let _ = c.translate(d(edge.x), d(edge.y));
                let _ = c.rotate(d(dir.angle()));
                c.set_fill_style_str("#ffd36a");
                c.begin_path();
                c.move_to(16.0, 0.0);
                c.line_to(-10.0, -10.0);
                c.line_to(-10.0, 10.0);
                c.close_path();
                c.fill();
                c.restore();
                c.set_text_align("center");
                c.set_font(&format!("800 11px {}", FONT));
                c.set_stroke_style_str("rgba(0,0,0,0.85)");
                outlined(c, &format!("📦 {:.0}m", dr.pos.dist(self.player.pos) / 40.0), edge.x - dir.x * 30.0, edge.y - dir.y * 30.0, 3.0);
            }
        }
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
