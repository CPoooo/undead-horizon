//! Outbreak visuals: barricaded windows, the mystery box, wall-buys, perk
//! machines, the power switch and upgrade machine, the interact prompt, and
//! the HUD (points, gun + ammo, perks, power-up timers, reload bar).

use crate::content::{gun_index, GUNS};
use crate::game::Game;
use crate::math::*;
use crate::modes::outbreak::{Perk, MAX_BOARDS, SPIN_T};
use crate::world;
use std::f64::consts::TAU as TAU64;
use web_sys::CanvasRenderingContext2d as Ctx;

#[inline]
fn d(x: f32) -> f64 {
    x as f64
}

const FONT: &str = "'Inter','Segoe UI',system-ui,sans-serif";
const EMOJI: &str = "'Segoe UI Emoji','Apple Color Emoji','Noto Color Emoji',sans-serif";

fn rect(c: &Ctx, x: f32, y: f32, w: f32, h: f32, col: &str) {
    c.set_fill_style_str(col);
    c.fill_rect(d(x), d(y), d(w), d(h));
}

fn txt(c: &Ctx, s: &str, x: f32, y: f32) {
    let _ = c.fill_text(s, d(x), d(y));
}

fn outlined(c: &Ctx, s: &str, x: f32, y: f32, lw: f32) {
    c.set_line_width(d(lw));
    let _ = c.stroke_text(s, d(x), d(y));
    let _ = c.fill_text(s, d(x), d(y));
}

fn gun_icon(id: &str) -> &'static str {
    match id {
        "pistol" => "🔫",
        "smg" => "🐍",
        "shotgun" => "💥",
        "rifle" => "🎯",
        "minigun" => "🌪️",
        "launcher" => "🧨",
        "ray" => "🛸",
        "wave" => "🌊",
        _ => "🔫",
    }
}

impl Game {
    /// World-space Outbreak props (drawn under the depth-sorted actors).
    pub fn draw_outbreak_world(&self, c: &Ctx, t: f32) {
        let (Some(ob), Some(m)) = (self.ob.as_ref(), world::map()) else { return };
        c.set_text_align("center");
        c.set_text_baseline("middle");
        // windows: frame + boards across the opening
        for w in ob.windows.iter() {
            let side = w.dir.perp();
            let a = w.pos + side * 40.0;
            let b = w.pos - side * 40.0;
            c.set_stroke_style_str("#1a1410");
            c.set_line_width(10.0);
            c.begin_path();
            c.move_to(d(a.x), d(a.y));
            c.line_to(d(b.x), d(b.y));
            c.stroke();
            for k in 0..w.boards {
                let off = w.dir * (-8.0 + k as f32 * 3.0);
                let tilt = side * (((k * 7) % 5) as f32 - 2.0) * 0.8;
                let aa = a + off + tilt + w.dir * ((k % 2) as f32 * 4.0);
                let bb = b + off - tilt;
                c.set_stroke_style_str(if k % 2 == 0 { "#8a6034" } else { "#7a5228" });
                c.set_line_width(7.0);
                c.begin_path();
                c.move_to(d(aa.x), d(aa.y));
                c.line_to(d(bb.x), d(bb.y));
                c.stroke();
            }
            if w.boards < MAX_BOARDS && self.player.pos.dist(w.pos) < 260.0 {
                c.set_font(&format!("700 10px {}", FONT));
                c.set_fill_style_str("#ffb050");
                txt(c, &format!("{}/{}", w.boards, MAX_BOARDS), w.pos.x, w.pos.y + w.dir.y * 26.0 - 30.0);
            }
        }
        for (i, it) in m.interact.iter().enumerate() {
            let p = v2(it.pos[0], it.pos[1]);
            match it.kind.as_str() {
                "wall" => {
                    // chalk outline of the gun on the wall
                    let g = gun_index(&it.data);
                    c.set_stroke_style_str("rgba(240,240,220,0.75)");
                    c.set_line_width(2.0);
                    c.stroke_rect(d(p.x - 34.0), d(p.y - 16.0), 68.0, 32.0);
                    c.set_font(&format!("18px {}", EMOJI));
                    c.set_fill_style_str("#ffffff");
                    txt(c, gun_icon(GUNS[g].id), p.x, p.y - 2.0);
                    c.set_font(&format!("800 9px {}", FONT));
                    c.set_fill_style_str("#f0f0dc");
                    txt(c, &format!("{} · {}", GUNS[g].name, it.cost), p.x, p.y + 24.0);
                }
                "perk" => {
                    let Some(pk) = Perk::from_id(&it.id) else { continue };
                    let (name, icon, col, _) = pk.def();
                    let base = [p + v2(-20.0, -14.0), p + v2(20.0, -14.0), p + v2(20.0, 14.0), p + v2(-20.0, 14.0)];
                    let top: Vec<V2> = base.iter().map(|q| self.lift(*q, 80.0)).collect();
                    c.set_fill_style_str("#22262e");
                    c.begin_path();
                    c.move_to(d(base[2].x), d(base[2].y));
                    c.line_to(d(base[3].x), d(base[3].y));
                    c.line_to(d(top[3].x), d(top[3].y));
                    c.line_to(d(top[2].x), d(top[2].y));
                    c.close_path();
                    c.fill();
                    c.set_fill_style_str(col);
                    c.set_global_alpha(if ob.power { 1.0 } else { 0.45 });
                    c.begin_path();
                    for (k, q) in top.iter().enumerate() {
                        if k == 0 {
                            c.move_to(d(q.x), d(q.y));
                        } else {
                            c.line_to(d(q.x), d(q.y));
                        }
                    }
                    c.close_path();
                    c.fill();
                    let mid = top[0].lerp(top[2], 0.5);
                    c.set_global_alpha(1.0);
                    if ob.power {
                        let _ = c.set_global_composite_operation("lighter");
                        c.set_global_alpha(d(0.25 + 0.15 * (t * 3.0 + i as f32).sin()));
                        c.set_fill_style_str(col);
                        c.begin_path();
                        let _ = c.arc(d(mid.x), d(mid.y), 34.0, 0.0, TAU64);
                        c.fill();
                        let _ = c.set_global_composite_operation("source-over");
                        c.set_global_alpha(1.0);
                    }
                    c.set_font(&format!("20px {}", EMOJI));
                    c.set_fill_style_str("#ffffff");
                    txt(c, icon, mid.x, mid.y);
                    c.set_font(&format!("800 9px {}", FONT));
                    c.set_fill_style_str(col);
                    c.set_stroke_style_str("rgba(0,0,0,0.85)");
                    outlined(c, name, mid.x, mid.y - 24.0, 2.5);
                }
                "power" => {
                    rect(c, p.x - 16.0, p.y - 30.0, 32.0, 40.0, "#30343c");
                    let on = ob.power;
                    c.set_stroke_style_str(if on { "#7affb0" } else { "#ff5050" });
                    c.set_line_width(5.0);
                    c.begin_path();
                    c.move_to(d(p.x), d(p.y - 10.0));
                    c.line_to(d(p.x + if on { 0.0 } else { 0.0 }), d(p.y - if on { 34.0 } else { -4.0 }));
                    c.stroke();
                    c.set_font(&format!("16px {}", EMOJI));
                    c.set_fill_style_str("#ffffff");
                    txt(c, "⚡", p.x, p.y - 44.0);
                }
                "upgrade" => {
                    let base = [p + v2(-46.0, -24.0), p + v2(46.0, -24.0), p + v2(46.0, 24.0), p + v2(-46.0, 24.0)];
                    let top: Vec<V2> = base.iter().map(|q| self.lift(*q, 60.0)).collect();
                    c.set_fill_style_str("#1e1428");
                    c.begin_path();
                    c.move_to(d(base[2].x), d(base[2].y));
                    c.line_to(d(base[3].x), d(base[3].y));
                    c.line_to(d(top[3].x), d(top[3].y));
                    c.line_to(d(top[2].x), d(top[2].y));
                    c.close_path();
                    c.fill();
                    c.set_fill_style_str(if ob.power { "#6a3a9a" } else { "#3a2a4a" });
                    c.begin_path();
                    for (k, q) in top.iter().enumerate() {
                        if k == 0 {
                            c.move_to(d(q.x), d(q.y));
                        } else {
                            c.line_to(d(q.x), d(q.y));
                        }
                    }
                    c.close_path();
                    c.fill();
                    let mid = top[0].lerp(top[2], 0.5);
                    if ob.power {
                        let _ = c.set_global_composite_operation("lighter");
                        c.set_global_alpha(d(0.35 + 0.2 * (t * 5.0).sin()));
                        c.set_fill_style_str("#d58cff");
                        c.begin_path();
                        let _ = c.arc(d(mid.x), d(mid.y), 44.0, 0.0, TAU64);
                        c.fill();
                        let _ = c.set_global_composite_operation("source-over");
                        c.set_global_alpha(1.0);
                    }
                    c.set_font(&format!("22px {}", EMOJI));
                    c.set_fill_style_str("#ffffff");
                    txt(c, "⚙️", mid.x, mid.y);
                }
                "trap" => {
                    if let Some(tr) = self.trap_at(i) {
                        let col = if tr.fire { "#ff7a1f" } else { "#7ab8ff" };
                        c.set_global_alpha(if tr.active > 0.0 { 0.35 } else { 0.12 });
                        c.set_fill_style_str(col);
                        c.begin_path();
                        let _ = c.arc(d(p.x), d(p.y), d(tr.r), 0.0, TAU64);
                        c.fill();
                        c.set_global_alpha(1.0);
                        c.set_stroke_style_str(if tr.cooldown > 0.0 { "#555a66" } else { col });
                        c.set_line_width(3.0);
                        let _ = c.set_line_dash(&js_sys::Array::of2(&10.0.into(), &8.0.into()));
                        c.begin_path();
                        let _ = c.arc(d(p.x), d(p.y), d(tr.r), 0.0, TAU64);
                        c.stroke();
                        let _ = c.set_line_dash(&js_sys::Array::new());
                        c.set_font(&format!("20px {}", EMOJI));
                        c.set_fill_style_str("#ffffff");
                        txt(c, if tr.fire { "🔥" } else { "⚡" }, p.x, p.y);
                    }
                }
                "part" => {
                    let got = self.secrets.as_ref().map(|s| s.parts.contains(&i) || s.shield).unwrap_or(true);
                    if !got {
                        let glint = 0.5 + 0.5 * (t * 4.0 + i as f32).sin();
                        c.set_global_alpha(d(0.4 + 0.6 * glint));
                        c.set_font(&format!("16px {}", EMOJI));
                        c.set_fill_style_str("#ffffff");
                        txt(c, "⚙️", p.x, p.y);
                        c.set_global_alpha(1.0);
                    }
                }
                "bench" => {
                    rect(c, p.x - 34.0, p.y - 14.0, 68.0, 28.0, "#5a3d1f");
                    rect(c, p.x - 30.0, p.y - 10.0, 60.0, 20.0, "#8a6236");
                    let n = self.secrets.as_ref().map(|s| s.parts.len()).unwrap_or(0);
                    c.set_font(&format!("800 10px {}", FONT));
                    c.set_fill_style_str("#f0e0c0");
                    txt(c, &format!("🛡️ {}/3", n), p.x, p.y);
                }
                "candle" => {
                    let lit = self.secrets.as_ref().map(|s| s.candles.contains(&i)).unwrap_or(false);
                    rect(c, p.x - 4.0, p.y - 18.0, 8.0, 18.0, "#e8e0d0");
                    if lit {
                        let fl = (t * 13.0 + i as f32).sin() * 1.5;
                        c.set_fill_style_str("#ffb030");
                        c.begin_path();
                        let _ = c.ellipse(d(p.x + fl * 0.3), d(p.y - 24.0), 4.0, d(7.0 + fl), 0.0, 0.0, TAU64);
                        c.fill();
                    }
                }
                "box" if i == ob.box_at && ob.box_away <= 0.0 => self.draw_box(c, p, t),
                "box" => {
                    // empty box spot: a faint chalk "?"
                    c.set_font(&format!("800 26px {}", FONT));
                    c.set_fill_style_str("rgba(240,240,220,0.18)");
                    txt(c, "?", p.x, p.y);
                }
                _ => {}
            }
        }
    }

    fn draw_box(&self, c: &Ctx, p: V2, t: f32) {
        let Some(ob) = self.ob.as_ref() else { return };
        let base = [p + v2(-38.0, -16.0), p + v2(38.0, -16.0), p + v2(38.0, 16.0), p + v2(-38.0, 16.0)];
        let top: Vec<V2> = base.iter().map(|q| self.lift(*q, 26.0)).collect();
        c.set_fill_style_str("#3a2614");
        c.begin_path();
        c.move_to(d(base[2].x), d(base[2].y));
        c.line_to(d(base[3].x), d(base[3].y));
        c.line_to(d(top[3].x), d(top[3].y));
        c.line_to(d(top[2].x), d(top[2].y));
        c.close_path();
        c.fill();
        c.set_fill_style_str("#6a4626");
        c.begin_path();
        for (k, q) in top.iter().enumerate() {
            if k == 0 {
                c.move_to(d(q.x), d(q.y));
            } else {
                c.line_to(d(q.x), d(q.y));
            }
        }
        c.close_path();
        c.fill();
        c.set_stroke_style_str("#ffd36a");
        c.set_line_width(2.0);
        c.stroke();
        let mid = top[0].lerp(top[2], 0.5);
        // the beam of light that marks the box
        let _ = c.set_global_composite_operation("lighter");
        c.set_global_alpha(d(0.18 + 0.08 * (t * 2.0).sin()));
        c.set_fill_style_str("#7ad7ff");
        c.fill_rect(d(mid.x - 18.0), d(mid.y - 260.0), 36.0, 260.0);
        let _ = c.set_global_composite_operation("source-over");
        c.set_global_alpha(1.0);
        c.set_font(&format!("900 18px {}", FONT));
        c.set_fill_style_str("#ffd36a");
        txt(c, "?", mid.x, mid.y);
        if let Some(s) = &ob.spin {
            let float = mid + v2(0.0, -40.0 - (s.t / SPIN_T).min(1.0) * 20.0);
            c.set_font(&format!("26px {}", EMOJI));
            c.set_fill_style_str("#ffffff");
            if s.t < SPIN_T {
                let k = ((s.t * (6.0 + s.t * 4.0)) as usize) % GUNS.len();
                txt(c, gun_icon(GUNS[k].id), float.x, float.y);
            } else if s.hand {
                txt(c, "✋", float.x, float.y);
                c.set_font(&format!("800 11px {}", FONT));
                c.set_fill_style_str("#ff8a8a");
                txt(c, "Bye bye!", float.x, float.y - 24.0);
            } else {
                txt(c, gun_icon(GUNS[s.gun].id), float.x, float.y);
                c.set_font(&format!("800 11px {}", FONT));
                c.set_fill_style_str("#ffffff");
                c.set_stroke_style_str("rgba(0,0,0,0.85)");
                outlined(c, GUNS[s.gun].name, float.x, float.y - 24.0, 3.0);
            }
        }
    }

    /// The riot shield, held in front.
    pub fn draw_riot_shield(&self, c: &Ctx, ppos: V2) {
        let has = self.secrets.as_ref().map(|s| s.shield).unwrap_or(false);
        if !has || self.player.dead {
            return;
        }
        let aim = self.player.aim;
        let k = self.secrets.as_ref().map(|s| s.shield_hp / crate::secrets::SHIELD_HP).unwrap_or(1.0);
        c.set_stroke_style_str(if k > 0.35 { "#9fdcff" } else { "#ff8a8a" });
        c.set_line_width(6.0);
        c.set_global_alpha(0.85);
        c.begin_path();
        let _ = c.arc(d(ppos.x), d(ppos.y - 4.0), 26.0, d(aim - 0.75), d(aim + 0.75));
        c.stroke();
        c.set_global_alpha(1.0);
    }

    /// Prompt over the interactable in reach (screen space).
    pub fn draw_outbreak_prompt(&self, c: &Ctx, w: f32, h: f32, cy: f32) {
        let Some(pr) = self.ob_prompt() else { return };
        let sp = v2(w * 0.5, cy) + (pr.pos - self.cam) * self.zoom;
        let y = (sp.y - 56.0).max(80.0).min((h - 140.0).max(80.0));
        c.set_text_align("center");
        c.set_text_baseline("middle");
        c.set_font(&format!("800 {}px {}", if w < 640.0 { 13 } else { 15 }, FONT));
        let tw = c.measure_text(&pr.text).map(|m| m.width() as f32).unwrap_or(200.0);
        c.set_fill_style_str("rgba(8,10,18,0.78)");
        c.fill_rect(d(sp.x - tw * 0.5 - 12.0), d(y - 14.0), d(tw + 24.0), 28.0);
        c.set_fill_style_str(if pr.ok { "#ffffff" } else { "#ff7a7a" });
        txt(c, &pr.text, sp.x, y);
    }

    /// Points, weapon + ammo, perks, power-up timers.
    pub fn draw_outbreak_hud(&self, c: &Ctx, w: f32, h: f32, t: f32) {
        let Some(ob) = self.ob.as_ref() else { return };
        // red chalk tally marks for the first five rounds
        let n = self.wave.n.max(1);
        let lay = self.hud_layout();
        // phones: the round is in the wave title; the tally would sit on it
        if n <= 5 && !lay.compact {
            let col = if ob.hound_round { "#ff7a1f" } else { "#c81e1e" };
            c.set_stroke_style_str(col);
            c.set_line_cap("round");
            c.set_line_width(5.0);
            let x0 = w - 24.0 - 5.0 * 14.0;
            c.begin_path();
            for k in 0..n.min(4) {
                let x = x0 + k as f32 * 14.0;
                let wob = ((k * 7 + 3) % 5) as f32 - 2.0;
                c.move_to(d(x + wob), d(16.0));
                c.line_to(d(x - wob * 0.5), d(50.0));
            }
            if n == 5 {
                c.move_to(d(x0 - 8.0), d(44.0));
                c.line_to(d(x0 + 52.0), d(20.0));
            }
            c.stroke();
            c.set_line_cap("butt");
        }
        let small = w < 640.0;
        // above the minimap (desktop); above the thumb cluster (phones, portrait);
        // left of the thumb cluster (phones, landscape)
        let (right, bottom) = if !lay.compact {
            (w - 18.0, h - 300.0)
        } else if lay.landscape {
            (w - crate::hud_layout::CLUSTER_W - 20.0, h - 86.0)
        } else {
            (w - 14.0, h - crate::hud_layout::CLUSTER_H - 96.0)
        };
        c.set_text_baseline("alphabetic");
        c.set_stroke_style_str("rgba(0,0,0,0.85)");
        c.set_line_join("round");
        // points
        c.set_text_align("right");
        c.set_font(&format!("900 {}px {}", if small { 22 } else { 30 }, FONT));
        c.set_fill_style_str("#ffd36a");
        outlined(c, &format!("{}", ob.points), right, bottom, 5.0);
        c.set_font(&format!("700 11px {}", FONT));
        c.set_fill_style_str("#c8b070");
        outlined(c, "POINTS", right, bottom + 14.0, 3.0);
        // weapon + ammo
        let wpn = ob.weapon();
        c.set_font(&format!("800 {}px {}", if small { 12 } else { 14 }, FONT));
        c.set_fill_style_str(if wpn.upgraded { "#d58cff" } else { "#ffffff" });
        outlined(c, wpn.name(), right, bottom + 38.0, 3.0);
        c.set_font(&format!("900 {}px {}", if small { 18 } else { 22 }, FONT));
        let low = wpn.mag * 4 <= wpn.max_mag();
        c.set_fill_style_str(if wpn.mag == 0 { "#ff4a4a" } else if low { "#ffb050" } else { "#ffffff" });
        outlined(c, &format!("{} / {}", wpn.mag, wpn.reserve), right, bottom + 62.0, 4.0);
        if ob.weapons.len() > 1 {
            let other = &ob.weapons[(ob.cur + 1) % ob.weapons.len()];
            c.set_font(&format!("600 11px {}", FONT));
            c.set_fill_style_str("#9aa3b8");
            let hint = if self.input.is_touch { "" } else { " · Tab" };
            outlined(c, &format!("{}{}", other.name(), hint), right, bottom + 78.0, 3.0);
        }
        // perks (under the health bar)
        c.set_text_align("center");
        c.set_text_baseline("middle");
        for (k, pk) in ob.perks.iter().enumerate() {
            let (_, icon, col, _) = pk.def();
            // phones: a row right-aligned above the points (the HP corner holds the pause row)
            let (x, y) = if lay.compact { (right - 14.0 - k as f32 * 32.0, bottom - 40.0) } else { (30.0 + k as f32 * 34.0, 74.0) };
            c.set_fill_style_str(col);
            c.begin_path();
            let _ = c.arc(d(x), d(y), 14.0, 0.0, TAU64);
            c.fill();
            c.set_font(&format!("15px {}", EMOJI));
            c.set_fill_style_str("#ffffff");
            txt(c, icon, x, y);
        }
        // power-up timers (bottom centre, above the abilities)
        let mut timers: Vec<(&str, f32)> = Vec::new();
        if ob.insta_t > 0.0 {
            timers.push(("💀", ob.insta_t));
        }
        if ob.double_t > 0.0 {
            timers.push(("✖️", ob.double_t));
        }
        for (k, (icon, left)) in timers.iter().enumerate() {
            let mid = if lay.compact { lay.stack_x + lay.stack_w * 0.5 } else { w * 0.5 };
            let x = mid + (k as f32 - (timers.len() as f32 - 1.0) * 0.5) * 52.0;
            let y = if lay.compact { lay.stack_end + 22.0 } else { h - 130.0 };
            let blink = *left > 5.0 || (t * 6.0).sin() > 0.0;
            if !blink {
                continue;
            }
            c.set_font(&format!("28px {}", EMOJI));
            c.set_fill_style_str("#ffffff");
            txt(c, icon, x, y);
            c.set_font(&format!("800 11px {}", FONT));
            c.set_fill_style_str("#7affb0");
            outlined(c, &format!("{}", left.ceil() as i32), x, y + 22.0, 3.0);
        }
        // reload bar under the player
        if let Some(k) = self.reload_frac() {
            let p = v2(w * 0.5, h * 0.5) + (self.player.pos - self.cam) * self.zoom + v2(0.0, 30.0);
            rect(c, p.x - 26.0, p.y, 52.0, 6.0, "rgba(0,0,0,0.6)");
            rect(c, p.x - 25.0, p.y + 1.0, 50.0 * k, 4.0, "#ffd36a");
            c.set_font(&format!("800 10px {}", FONT));
            c.set_fill_style_str("#ffd36a");
            outlined(c, "RELOADING", p.x, p.y + 16.0, 3.0);
        }
    }
}
