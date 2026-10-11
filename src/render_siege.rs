//! Siege visuals: the crystal, portals, defenses, the build ghost, and the HUD
//! (phase timer, mana, crystal health, build bar, wave preview per portal).

use crate::game::{Game, ZKind};
use crate::math::*;
use crate::modes::siege::{DefKind, SiegePhase, DEFS};
use std::f64::consts::TAU as TAU64;
use web_sys::CanvasRenderingContext2d as Ctx;

#[inline]
fn d(x: f32) -> f64 {
    x as f64
}
const FONT: &str = "'Inter','Segoe UI',system-ui,sans-serif";
const EMOJI: &str = "'Segoe UI Emoji','Apple Color Emoji','Noto Color Emoji',sans-serif";

fn circ(c: &Ctx, p: V2, r: f32) {
    c.begin_path();
    let _ = c.arc(d(p.x), d(p.y), d(r.max(0.1)), 0.0, TAU64);
}
fn outlined(c: &Ctx, s: &str, x: f32, y: f32, lw: f32) {
    c.set_line_width(d(lw));
    let _ = c.stroke_text(s, d(x), d(y));
    let _ = c.fill_text(s, d(x), d(y));
}

impl Game {
    pub fn draw_siege_world(&self, c: &Ctx, t: f32) {
        let Some(s) = self.siege.as_ref() else { return };
        c.set_text_align("center");
        c.set_text_baseline("middle");
        // portals: swirling violet rings
        for (p, _) in s.portals.iter() {
            for k in 0..3 {
                let r = 30.0 + k as f32 * 12.0 + (t * 2.0 + k as f32).sin() * 4.0;
                c.set_stroke_style_str("#b48cff");
                c.set_global_alpha(d(0.6 - k as f32 * 0.15));
                c.set_line_width(4.0);
                c.begin_path();
                let a0 = t * (1.5 + k as f32);
                let _ = c.arc(d(p.x), d(p.y), d(r), d(a0), d(a0 + 4.5));
                c.stroke();
            }
            c.set_global_alpha(1.0);
        }
        // the crystal: a tall faceted gem lifted toward the camera
        let p = s.crystal;
        let k = (s.crystal_hp / s.crystal_max).clamp(0.0, 1.0);
        let top = self.lift(p, 110.0);
        let col = if s.crystal_flash > 0.0 { "#ffffff" } else if k > 0.4 { "#7ad7ff" } else { "#ff8a8a" };
        c.set_fill_style_str("rgba(0,0,0,0.3)");
        c.begin_path();
        let _ = c.ellipse(d(p.x + 6.0), d(p.y + 10.0), 44.0, 20.0, 0.0, 0.0, TAU64);
        c.fill();
        c.set_fill_style_str("#2a3a5a");
        circ(c, p, 40.0);
        c.fill();
        let side = (top - p).norm().perp();
        let side = if side.len2() > 0.0 { side } else { v2(1.0, 0.0) };
        for (a, b, shade) in [(1.0f32, 0.0f32, 0.75f32), (0.0, -1.0, 1.0)] {
            c.set_global_alpha(d(shade));
            c.set_fill_style_str(col);
            c.begin_path();
            c.move_to(d(p.x + side.x * 26.0 * a), d(p.y + side.y * 26.0 * a));
            c.line_to(d(top.x), d(top.y));
            c.line_to(d(p.x + side.x * 26.0 * b), d(p.y + side.y * 26.0 * b));
            c.close_path();
            c.fill();
        }
        c.set_global_alpha(1.0);
        let _ = c.set_global_composite_operation("lighter");
        c.set_global_alpha(d(0.25 + 0.15 * (t * 2.0).sin()));
        c.set_fill_style_str(col);
        circ(c, p.lerp(top, 0.5), 60.0);
        c.fill();
        let _ = c.set_global_composite_operation("source-over");
        c.set_global_alpha(1.0);
        // defenses
        for df in s.defenses.iter() {
            self.draw_defense(c, df.kind, df.pos, df.tier, df.aim, df.flash, t, 1.0);
            if df.hp < df.max_hp {
                let w = 40.0;
                c.set_fill_style_str("rgba(0,0,0,0.6)");
                c.fill_rect(d(df.pos.x - w * 0.5), d(df.pos.y + 26.0), d(w), 4.0);
                c.set_fill_style_str("#9cffb8");
                c.fill_rect(d(df.pos.x - w * 0.5), d(df.pos.y + 26.0), d(w * (df.hp / df.max_hp).max(0.0)), 4.0);
            }
        }
        // build ghost + range
        if let Some(kind) = s.build_sel {
            let g = self.build_ghost();
            let ok = self.can_place(kind, g).is_ok();
            let r = kind.range(1);
            if r > 0.0 {
                c.set_stroke_style_str(if ok { "rgba(122,255,176,0.6)" } else { "rgba(255,90,90,0.6)" });
                c.set_line_width(2.0);
                let _ = c.set_line_dash(&js_sys::Array::of2(&8.0.into(), &6.0.into()));
                circ(c, g, r);
                c.stroke();
                let _ = c.set_line_dash(&js_sys::Array::new());
            }
            c.set_global_alpha(0.6);
            self.draw_defense(c, kind, g, 1, self.player.aim, 0.0, t, 1.0);
            c.set_global_alpha(1.0);
            if !ok {
                c.set_fill_style_str("rgba(255,60,60,0.35)");
                circ(c, g, kind.radius() + 6.0);
                c.fill();
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_defense(&self, c: &Ctx, kind: DefKind, p: V2, tier: u8, aim: f32, flash: f32, t: f32, _a: f32) {
        let (_, icon, col, _) = kind.def();
        match kind {
            DefKind::Tower => {
                let top = self.lift(p, 50.0 + 8.0 * tier as f32);
                c.set_stroke_style_str("#5a4630");
                c.set_line_width(14.0);
                c.begin_path();
                c.move_to(d(p.x), d(p.y));
                c.line_to(d(top.x), d(top.y));
                c.stroke();
                c.set_fill_style_str(col);
                circ(c, top, 15.0);
                c.fill();
                let tip = top + V2::from_angle(aim) * 20.0;
                c.set_stroke_style_str("#3a2a1a");
                c.set_line_width(4.0);
                c.begin_path();
                c.move_to(d(top.x), d(top.y));
                c.line_to(d(tip.x), d(tip.y));
                c.stroke();
            }
            DefKind::Barricade => {
                let top = self.lift(p, 34.0);
                c.set_fill_style_str("#5a4026");
                c.fill_rect(d(p.x - 30.0), d(p.y - 10.0), 60.0, 20.0);
                c.set_fill_style_str(col);
                c.fill_rect(d(top.x - 30.0), d(top.y - 10.0), 60.0, 20.0);
                c.set_stroke_style_str("#3a2a1a");
                c.set_line_width(2.0);
                c.stroke_rect(d(top.x - 30.0), d(top.y - 10.0), 60.0, 20.0);
            }
            DefKind::Flame | DefKind::Frost => {
                c.set_fill_style_str("#30343c");
                circ(c, p, 18.0);
                c.fill();
                let pulse = 0.5 + 0.5 * (t * 4.0).sin();
                c.set_global_alpha(d(0.25 + 0.25 * pulse));
                c.set_fill_style_str(col);
                circ(c, p, kind.range(tier));
                c.fill();
                c.set_global_alpha(1.0);
            }
            DefKind::Mine => {
                c.set_fill_style_str("#3a3a3a");
                circ(c, p, 12.0);
                c.fill();
                c.set_fill_style_str(if (t * 3.0).sin() > 0.0 { "#ff4040" } else { "#601010" });
                circ(c, p, 4.0);
                c.fill();
            }
            DefKind::Totem => {
                let top = self.lift(p, 60.0);
                c.set_stroke_style_str("#6a5a40");
                c.set_line_width(10.0);
                c.begin_path();
                c.move_to(d(p.x), d(p.y));
                c.line_to(d(top.x), d(top.y));
                c.stroke();
                let _ = c.set_global_composite_operation("lighter");
                c.set_global_alpha(d(0.4 + 0.2 * (t * 3.0).sin()));
                c.set_fill_style_str(col);
                circ(c, top, 20.0);
                c.fill();
                let _ = c.set_global_composite_operation("source-over");
                c.set_global_alpha(1.0);
            }
        }
        if flash > 0.0 {
            c.set_global_alpha(d(flash * 0.6));
            c.set_fill_style_str("#ff4040");
            circ(c, p, kind.radius() + 4.0);
            c.fill();
            c.set_global_alpha(1.0);
        }
        // tier pips + icon
        c.set_font(&format!("14px {}", EMOJI));
        c.set_fill_style_str("#ffffff");
        let _ = c.fill_text(icon, d(p.x), d(p.y - 4.0));
        for k in 0..tier {
            c.set_fill_style_str("#ffd36a");
            circ(c, p + v2(-8.0 + k as f32 * 8.0, 14.0), 2.5);
            c.fill();
        }
    }

    pub fn draw_siege_hud(&self, c: &Ctx, w: f32, h: f32, cy: f32, t: f32) {
        let Some(s) = self.siege.as_ref() else { return };
        let small = w < 640.0;
        c.set_text_align("center");
        c.set_text_baseline("alphabetic");
        c.set_stroke_style_str("rgba(0,0,0,0.85)");
        c.set_line_join("round");
        // crystal bar (top centre)
        let bw = (w * 0.4).min(440.0);
        let bx = w * 0.5 - bw * 0.5;
        let by = if self.zombies.iter().any(|z| z.kind == ZKind::Boss) { 86.0 } else { 34.0 };
        c.set_font(&format!("800 12px {}", FONT));
        c.set_fill_style_str("#7ad7ff");
        outlined(c, &format!("💎 CRYSTAL {} / {}", s.crystal_hp.max(0.0) as i32, s.crystal_max as i32), w * 0.5, by - 6.0, 3.0);
        c.set_fill_style_str("rgba(0,10,20,0.8)");
        c.fill_rect(d(bx), d(by), d(bw), 10.0);
        let k = (s.crystal_hp / s.crystal_max).clamp(0.0, 1.0);
        c.set_fill_style_str(if k > 0.4 { "#4ab8ff" } else { "#ff5a5a" });
        c.fill_rect(d(bx + 1.0), d(by + 1.0), d((bw - 2.0) * k), 8.0);
        // phase line
        c.set_font(&format!("800 {}px {}", if small { 13 } else { 15 }, FONT));
        let line = match s.phase {
            SiegePhase::Build => {
                let key = if self.input.is_touch { "tap READY" } else { "Enter" };
                format!("BUILD PHASE {}s · {} when ready", s.build_t.ceil() as i32, key)
            }
            SiegePhase::Combat => format!("{} enemies left", s.queue.iter().map(|q| q.len()).sum::<usize>() + self.zombies.len()),
        };
        c.set_fill_style_str(if s.phase == SiegePhase::Build { "#9cffb8" } else { "#ffb070" });
        outlined(c, &line, w * 0.5, by + 30.0, 3.0);
        if s.walled_in {
            c.set_fill_style_str("#ff8a8a");
            c.set_font(&format!("700 12px {}", FONT));
            outlined(c, "Every path is walled off: they'll smash through your barricades!", w * 0.5, by + 48.0, 3.0);
        }
        // mana + build bar
        let bar = self.build_bar();
        c.set_font(&format!("900 {}px {}", if small { 16 } else { 20 }, FONT));
        c.set_fill_style_str("#7ab8ff");
        c.set_text_align("left");
        outlined(c, &format!("🔷 {}", s.mana), bar[0].0, bar[0].1 - 10.0, 4.0);
        c.set_text_align("center");
        c.set_text_baseline("middle");
        for (i, r) in bar.iter().enumerate() {
            let kind = DEFS[i];
            let (name, icon, col, cost) = kind.def();
            let sel = s.build_sel == Some(kind);
            let afford = s.mana >= cost;
            c.set_fill_style_str(if sel { "rgba(122,255,176,0.35)" } else { "rgba(10,12,20,0.75)" });
            c.fill_rect(d(r.0), d(r.1), d(r.2), d(r.3));
            c.set_stroke_style_str(if sel { "#7affb0" } else { col });
            c.set_line_width(if sel { 3.0 } else { 1.5 });
            c.stroke_rect(d(r.0), d(r.1), d(r.2), d(r.3));
            c.set_global_alpha(if afford { 1.0 } else { 0.4 });
            c.set_font(&format!("{}px {}", (r.2 * 0.4).round(), EMOJI));
            c.set_fill_style_str("#ffffff");
            let _ = c.fill_text(icon, d(r.0 + r.2 * 0.5), d(r.1 + r.3 * 0.42));
            c.set_font(&format!("800 10px {}", FONT));
            c.set_fill_style_str("#7ab8ff");
            let _ = c.fill_text(&format!("{}", cost), d(r.0 + r.2 * 0.5), d(r.1 + r.3 * 0.82));
            c.set_global_alpha(1.0);
            if !self.input.is_touch {
                c.set_font(&format!("700 9px {}", FONT));
                c.set_fill_style_str("#c8d2e8");
                let _ = c.fill_text(&format!("{}", i + 1), d(r.0 + 7.0), d(r.1 + 8.0));
            }
            if sel {
                c.set_font(&format!("700 11px {}", FONT));
                c.set_fill_style_str("#ffffff");
                c.set_stroke_style_str("rgba(0,0,0,0.85)");
                outlined(c, name, r.0 + r.2 * 0.5, r.1 - 12.0, 3.0);
            }
        }
        // upgrade / sell prompt near a defense
        if let Some(i) = self.defense_in_reach() {
            let df = &s.defenses[i];
            let sp = v2(w * 0.5, cy) + (df.pos - self.cam) * self.zoom;
            let key = if self.input.is_touch { "USE" } else { "[E]" };
            let txt = if df.tier >= 3 {
                format!("{} Repair — 10 · [X] Sell", key)
            } else {
                format!("{} Upgrade to tier {} — {} · [X] Sell", key, df.tier + 1, Game::upgrade_cost(df))
            };
            c.set_font(&format!("800 13px {}", FONT));
            c.set_fill_style_str("rgba(8,10,18,0.78)");
            let tw = c.measure_text(&txt).map(|m| m.width() as f32).unwrap_or(200.0);
            c.fill_rect(d(sp.x - tw * 0.5 - 10.0), d(sp.y - 62.0), d(tw + 20.0), 24.0);
            c.set_fill_style_str("#ffffff");
            let _ = c.fill_text(&txt, d(sp.x), d(sp.y - 50.0));
        }
        // wave preview at each portal during the build phase
        if s.phase == SiegePhase::Build {
            let next = self.wave.n + 1;
            let open = if next >= 5 { s.portals.len() } else if next >= 3 { s.portals.len().min(2) } else { 1 };
            for (k, (p, _)) in s.portals.iter().enumerate() {
                let sp = v2(w * 0.5, cy) + (*p - self.cam) * self.zoom;
                let sp = v2(sp.x.clamp(90.0, w - 90.0), sp.y.clamp(160.0, h - 240.0));
                c.set_font(&format!("800 12px {}", FONT));
                c.set_stroke_style_str("rgba(0,0,0,0.85)");
                if k < open {
                    c.set_fill_style_str("#d0b0ff");
                    outlined(c, "⚠ enemies incoming", sp.x, sp.y, 3.0);
                } else {
                    c.set_fill_style_str("#6a6a80");
                    outlined(c, "quiet… for now", sp.x, sp.y, 3.0);
                }
            }
        }
        let _ = t;
    }
}
