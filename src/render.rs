//! Canvas2D renderer. The "3D" look comes from perspective-lifting tall things
//! (tree canopies, rock tops, gravestones) away from the camera centre, so they
//! parallax as you move — plus soft shadows, depth sorting and dynamic lighting.

use crate::content::{GUNS, HATS, SKINS, TRACERS};
use crate::game::*;
use crate::math::*;
use crate::upgrades::ALL;
use crate::world::{self, ObKind, Obstacle};
use std::f32::consts::{PI, TAU};
use std::f64::consts::TAU as TAU64;
use web_sys::CanvasRenderingContext2d as Ctx;

const CAM_H: f32 = 900.0; // virtual camera height for perspective lift

#[inline]
fn d(x: f32) -> f64 {
    x as f64
}

fn circle(c: &Ctx, x: f32, y: f32, r: f32) {
    c.begin_path();
    let _ = c.arc(d(x), d(y), d(r.max(0.1)), 0.0, TAU64);
    c.fill();
}

fn path_circle(c: &Ctx, x: f32, y: f32, r: f32) {
    c.move_to(d(x + r), d(y));
    let _ = c.arc(d(x), d(y), d(r.max(0.1)), 0.0, TAU64);
}

fn path_circle_stroke(c: &Ctx, x: f32, y: f32, r: f32) {
    c.move_to(d(x + r), d(y));
    let _ = c.arc(d(x), d(y), d(r.max(0.1)), 0.0, TAU64);
}

fn round_rect(c: &Ctx, x: f32, y: f32, w: f32, h: f32, r: f32) {
    let r = r.min(w * 0.5).min(h * 0.5);
    c.begin_path();
    c.move_to(d(x + r), d(y));
    let _ = c.arc_to(d(x + w), d(y), d(x + w), d(y + h), d(r));
    let _ = c.arc_to(d(x + w), d(y + h), d(x), d(y + h), d(r));
    let _ = c.arc_to(d(x), d(y + h), d(x), d(y), d(r));
    let _ = c.arc_to(d(x), d(y), d(x + w), d(y), d(r));
    c.close_path();
}

fn text(c: &Ctx, s: &str, x: f32, y: f32) {
    let _ = c.fill_text(s, d(x), d(y));
}

fn outlined(c: &Ctx, s: &str, x: f32, y: f32, lw: f32) {
    c.set_line_width(d(lw));
    let _ = c.stroke_text(s, d(x), d(y));
    let _ = c.fill_text(s, d(x), d(y));
}

fn ease_out_back(t: f32) -> f32 {
    let c1 = 1.70158;
    let c3 = c1 + 1.0;
    1.0 + c3 * (t - 1.0).powi(3) + c1 * (t - 1.0).powi(2)
}

fn ease_out(t: f32) -> f32 {
    1.0 - (1.0 - t.clamp(0.0, 1.0)).powi(3)
}

fn wrap(c: &Ctx, s: &str, max_w: f32) -> Vec<String> {
    let mut lines = Vec::new();
    let mut cur = String::new();
    for w in s.split_whitespace() {
        let cand = if cur.is_empty() {
            w.to_string()
        } else {
            format!("{} {}", cur, w)
        };
        let width = c
            .measure_text(&cand)
            .map(|m| m.width() as f32)
            .unwrap_or(0.0);
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

const REVIVE_R: f32 = 75.0;

/// Everything needed to draw a survivor (local, teammate or lobby showcase).
pub struct PView {
    pub aim: f32,
    pub vel: V2,
    pub walk: f32,
    pub recoil: f32,
    pub blink: bool,
    pub frenzy: bool,
    /// class ability active: glow in this color
    pub ability: Option<&'static str>,
    pub hurt: f32,
    pub skin: usize,
    pub gun: usize,
    pub hat: usize,
    pub downed: bool,
    pub revive: f32,
}

enum Draw {
    Remote(usize),
    Player,
    Zombie(usize),
    Ob(Obstacle),
    Wall(crate::render_map::WallChunk),
    Npc(usize),
    Mate(usize),
}

impl Game {
    fn local_view(&self) -> PView {
        let pl = &self.player;
        PView {
            aim: pl.aim,
            vel: pl.vel,
            walk: pl.walk,
            recoil: pl.recoil,
            blink: pl.iframes > 0.0 && pl.dash_t <= 0.0 && !pl.downed,
            frenzy: pl.frenzy > 0.0,
            ability: if self.ability_t > 0.0 {
                Some(crate::content::CHARACTERS[self.character].color)
            } else {
                None
            },
            hurt: pl.hurt_flash,
            skin: self.skin,
            gun: self.gun,
            hat: self.hat,
            downed: pl.downed,
            revive: pl.revive,
        }
    }

    fn remote_view(r: &crate::net::Remote) -> PView {
        let s = &r.state;
        PView {
            aim: s.aim,
            vel: s.vel,
            walk: r.walk,
            recoil: r.recoil,
            blink: false,
            frenzy: s.flags & crate::net::PF_FRENZY != 0,
            ability: if s.flags & crate::net::PF_ABILITY != 0 { Some("#ffb03a") } else { None },
            hurt: if s.flags & crate::net::PF_HURT != 0 {
                0.8
            } else {
                0.0
            },
            skin: s.skin as usize,
            gun: s.gun as usize,
            hat: s.hat as usize,
            downed: s.downed(),
            revive: s.revive,
        }
    }

    pub fn lift(&self, base: V2, h: f32) -> V2 {
        base + (base - self.cam) * (h / (CAM_H - h))
    }

    pub fn render(&mut self, c: &Ctx, w: f32, h: f32, dpr: f32) {
        self.screen = v2(w, h);
        self.dpr = dpr;
        // constant visible area regardless of window size
        let base_zoom = ((w * h).sqrt() / 960.0).clamp(0.45, 2.2);
        let portrait_k = if h > w * 1.25 { 0.85 } else { 1.0 };
        self.spawn_r = (v2(w, h) * (0.5 / (base_zoom * portrait_k))).len() + 70.0;
        // phones held upright see a narrow strip: pull the camera back a little
        let portrait = if h > w * 1.25 { 0.85 } else { 1.0 };
        self.zoom = base_zoom * portrait * (1.0 + self.zoom_punch);
        let half = v2(w, h) * (0.5 / self.zoom);

        let a = self.alpha;
        let ppos = self.player.prev.lerp(self.player.pos, a);
        let t = self.real_time as f32;
        let sh = self.shake * self.shake * 16.0;
        let shake = v2(
            ((t * 61.0).sin() + (t * 23.0).sin() * 0.5) * sh,
            ((t * 53.0).cos() + (t * 31.0).cos() * 0.5) * sh,
        );
        self.cam = self.camera_pos(ppos) + self.cam_kick + self.cam_look;

        let _ = c.set_transform(d(dpr), 0.0, 0.0, d(dpr), 0.0, 0.0);
        c.set_image_smoothing_enabled(true);
        let _ = c.set_global_composite_operation("source-over");
        c.set_global_alpha(1.0);

        // ---------------- world space
        c.save();
        // phones: the lobby panels cover the lower screen, so lift the showcase into view
        let cy = if self.phase == Phase::Menu && w < 760.0 { (h * 0.2 + 40.0).min(h * 0.5) } else { h * 0.5 };
        let _ = c.translate(d(w * 0.5 + shake.x), d(cy + shake.y));
        let _ = c.scale(d(self.zoom), d(self.zoom));
        let _ = c.translate(d(-self.cam.x), d(-self.cam.y));
        let vmin = self.cam - half - v2(80.0, 80.0);
        let vmax = self.cam + half + v2(80.0, 80.0);

        let map = world::map();
        match &map {
            Some(m) => self.draw_map_ground(c, m, vmin, vmax),
            None => self.draw_ground(c, vmin, vmax),
        }
        self.draw_decals(c, vmin, vmax);
        self.draw_corpses(c);

        let mut obs: Vec<Obstacle> = Vec::new();
        world::for_obstacles_in(vmin - v2(150.0, 150.0), vmax + v2(150.0, 150.0), |o| {
            obs.push(*o)
        });

        self.draw_shadows(c, a, &obs);
        self.draw_outbreak_world(c, t);
        self.draw_siege_world(c, t);
        self.draw_gems(c, a, t);
        self.draw_telegraphs(c, a, t);
        self.draw_rings(c);

        // depth-sorted bodies
        let mut list: Vec<(f32, Draw)> = Vec::with_capacity(self.zombies.len() + obs.len() + 1);
        if !matches!(self.phase, Phase::Menu | Phase::Dying | Phase::GameOver) && !self.player.dead
        {
            list.push((ppos.y, Draw::Player));
        }
        if let Some(n) = &self.net {
            for (i, r) in n.remotes.iter().enumerate() {
                if r.present && !r.state.dead() {
                    list.push((r.render_pos(self.real_time).y, Draw::Remote(i)));
                }
            }
        }
        let mut visible = 0;
        for (i, z) in self.zombies.iter().enumerate() {
            let zp = z.prev.lerp(z.pos, self.za(a));
            if zp.x > vmin.x && zp.x < vmax.x && zp.y > vmin.y && zp.y < vmax.y {
                list.push((zp.y, Draw::Zombie(i)));
                visible += 1;
            }
        }
        // big hordes: cheap zombies (no composite-mode switches, fewer paths)
        self.lod = visible > 220;
        for o in obs.iter() {
            list.push((o.pos.y, Draw::Ob(*o)));
        }
        if let Some(hb) = &self.hub {
            for (i, n) in hb.npcs.iter().enumerate() {
                list.push((n.pos.y, Draw::Npc(i)));
            }
            for (i, m) in hb.mates.iter().enumerate() {
                list.push((m.pos.y, Draw::Mate(i)));
            }
        }
        if let Some(m) = &map {
            let mut chunks = Vec::new();
            self.map_wall_chunks(m, vmin, vmax, &mut chunks);
            for w in chunks {
                list.push((w.key, Draw::Wall(w)));
            }
        }
        list.sort_unstable_by(|x, y| x.0.partial_cmp(&y.0).unwrap_or(std::cmp::Ordering::Equal));

        self.draw_ghosts(c);
        for (_, dr) in list.iter() {
            match dr {
                Draw::Player => self.draw_player(c, ppos, t, &self.local_view()),
                Draw::Remote(i) => {
                    if let Some(r) = self.net.as_ref().map(|n| &n.remotes[*i]) {
                        let rp = r.render_pos(self.real_time);
                        self.draw_player(c, rp, t, &Game::remote_view(r));
                    }
                }
                Draw::Zombie(i) => self.draw_zombie(c, *i, a, t),
                Draw::Ob(o) => self.draw_obstacle_base(c, o),
                Draw::Npc(i) => {
                    if let Some(hb) = &self.hub {
                        self.draw_player(c, hb.npcs[*i].pos, t, &self.npc_view(*i, t));
                    }
                }
                Draw::Mate(i) => {
                    if let Some(hb) = &self.hub {
                        self.draw_player(c, hb.mates[*i].pos, t, &self.mate_view(*i));
                    }
                }
                Draw::Wall(w) => {
                    if let Some(m) = &map {
                        self.draw_wall_chunk(c, m, w);
                    }
                }
            }
        }

        self.draw_blades(c, ppos, t);
        self.draw_turrets(c, t);
        self.draw_riot_shield(c, ppos);
        self.draw_bullets(c, a);
        self.draw_particles(c);
        self.draw_lights(c);
        self.draw_canopies(c, &obs, ppos);
        if self.phase == Phase::Menu {
            // character showcase: the player model, scaled up on a glowing pedestal
            let s = (3.2 / self.zoom.max(0.5)).clamp(2.0, 4.5);
            c.save();
            let _ = c.translate(d(ppos.x), d(ppos.y));
            let _ = c.scale(d(s), d(s));
            let _ = c.translate(d(-ppos.x), d(-ppos.y));
            let pulse = 0.5 + 0.5 * (t * 2.0).sin();
            let _ = c.set_global_composite_operation("lighter");
            c.set_global_alpha(d(0.18 + 0.1 * pulse));
            c.set_fill_style_str("#7ad7ff");
            c.begin_path();
            let _ = c.ellipse(d(ppos.x), d(ppos.y + 6.0), 30.0, 15.0, 0.0, 0.0, TAU64);
            c.fill();
            c.set_global_alpha(0.6);
            c.set_stroke_style_str("#7ad7ff");
            c.set_line_width(1.2);
            c.begin_path();
            let _ = c.ellipse(
                d(ppos.x),
                d(ppos.y + 6.0),
                d(24.0 + 4.0 * pulse),
                d(12.0 + 2.0 * pulse),
                0.0,
                0.0,
                TAU64,
            );
            c.stroke();
            let _ = c.set_global_composite_operation("source-over");
            c.set_global_alpha(0.35);
            c.set_fill_style_str("#000");
            c.begin_path();
            let _ = c.ellipse(
                d(ppos.x + 3.0),
                d(ppos.y + 8.0),
                d(PR * 1.1),
                d(PR * 0.5),
                0.0,
                0.0,
                TAU64,
            );
            c.fill();
            c.set_global_alpha(1.0);
            self.draw_player(c, ppos, t, &self.local_view());
            // party members stand beside us
            let font = "'Inter','Segoe UI',system-ui,sans-serif";
            for (k, (name, skin, hat, gun)) in self.lobby_mates.iter().enumerate() {
                let side = if k == 0 { -1.0 } else { 1.0 };
                let mp = ppos + v2(side * 62.0, 10.0);
                c.set_global_alpha(0.25);
                c.set_fill_style_str("#7ad7ff");
                c.begin_path();
                let _ = c.ellipse(d(mp.x), d(mp.y + 6.0), 24.0, 12.0, 0.0, 0.0, TAU64);
                c.fill();
                c.set_global_alpha(1.0);
                let view = PView {
                    aim: self.player.aim + side * 0.4,
                    vel: self.player.vel,
                    walk: self.player.walk + k as f32,
                    recoil: 0.0,
                    blink: false,
                    frenzy: false,
                    ability: None,
                    hurt: 0.0,
                    skin: *skin,
                    gun: *gun,
                    hat: *hat,
                    downed: false,
                    revive: 0.0,
                };
                self.draw_player(c, mp, t, &view);
                c.set_text_align("center");
                c.set_text_baseline("middle");
                c.set_font(&format!("800 6px {}", font));
                c.set_fill_style_str("#9cffb8");
                c.set_stroke_style_str("rgba(0,0,0,0.85)");
                outlined(c, name, mp.x, mp.y - 30.0, 1.5);
            }
            c.restore();
        }
        self.draw_bolts(c, t);
        self.draw_hub_overlays(c, t);
        self.draw_texts(c);
        self.draw_teammate_tags(c, t);
        c.restore();

        // ---------------- screen space
        self.draw_lighting(c, w, h, ppos, t);
        self.draw_weather(c, w, h, t);
        if !matches!(self.phase, Phase::Menu) {
            self.draw_offscreen(c, w, h, a, t);
            self.draw_hud(c, w, h, t);
            self.draw_minimap(c, w, h, a, t);
        }
        if self.coop() && !matches!(self.phase, Phase::Menu) {
            self.draw_coop_hud(c, w, h, t);
        }
        if self.hub.is_some() && self.phase == Phase::Playing {
            self.draw_hub_prompt(c, w, cy);
        }
        if self.siege.is_some() && matches!(self.phase, Phase::Playing | Phase::Paused) {
            self.draw_siege_hud(c, w, h, cy, t);
        }
        if self.ob.is_some() && matches!(self.phase, Phase::Playing | Phase::Paused) {
            self.draw_outbreak_hud(c, w, h, t);
            self.draw_outbreak_prompt(c, w, h, cy);
        }
        // the touch USE button follows the prompt
        let show_use = self.phase == Phase::Playing
            && ((self.ob.is_some() && self.ob_prompt().is_some())
                || (self.siege.is_some() && self.defense_in_reach().is_some())
                || (self.hub.is_some() && self.hub_target().is_some()));
        let show_ready = self.phase == Phase::Playing
            && self.siege.as_ref().is_some_and(|s| s.phase == crate::modes::siege::SiegePhase::Build);
        if show_ready != self.ready_shown {
            self.ready_shown = show_ready;
            self.ui_out.push(("readybtn", if show_ready { "1".into() } else { "0".into() }));
        }
        if show_use != self.use_shown {
            self.use_shown = show_use;
            self.ui_out.push(("usebtn", if show_use { "1".into() } else { "0".into() }));
        }
        if let Some(tu) = &self.tutorial {
            if self.phase == Phase::Playing {
                let (title, how) = crate::tutorial::STEPS[tu.step.min(crate::tutorial::STEPS.len() - 1)];
                let font = "'Inter','Segoe UI',system-ui,sans-serif";
                let bw = (w * 0.9).min(560.0);
                let x = w * 0.5 - bw * 0.5;
                let y = if w < 640.0 { 150.0 } else { 96.0 };
                round_rect(c, x, y, bw, 54.0, 12.0);
                c.set_fill_style_str("rgba(8,12,22,0.85)");
                c.fill();
                c.set_stroke_style_str("#7ad7ff");
                c.set_line_width(2.0);
                c.stroke();
                c.set_text_align("center");
                c.set_text_baseline("middle");
                c.set_font(&format!("800 14px {}", font));
                c.set_fill_style_str("#7ad7ff");
                text(c, &format!("TUTORIAL {}/{} · {}", tu.step + 1, crate::tutorial::STEPS.len(), title), w * 0.5, y + 17.0);
                c.set_font(&format!("600 {}px {}", if w < 640.0 { 11 } else { 13 }, font));
                c.set_fill_style_str("#e8f0ff");
                text(c, how, w * 0.5, y + 37.0);
            }
        }
        if let Some(bi) = &self.boss_intro {
            self.draw_boss_intro(c, w, h, bi.var, bi.t);
        }
        if self.phase == Phase::LevelUp {
            if self.chest.is_some() {
                self.draw_chest(c, w, h, t);
            } else {
                self.draw_levelup(c, w, h, t);
            }
        }
        if self.show_fps {
            c.set_font("12px monospace");
            c.set_text_align("left");
            c.set_fill_style_str("#9f9");
            text(
                c,
                &format!(
                    "{:.0} fps · {} z · {} p",
                    self.fps,
                    self.zombies.len(),
                    self.particles.len()
                ),
                10.0,
                h - 10.0,
            );
        }
    }

    // ------------------------------------------------------------- ground
    fn draw_ground(&self, c: &Ctx, vmin: V2, vmax: V2) {
        c.set_fill_style_str("#1f2b1b");
        c.fill_rect(d(vmin.x), d(vmin.y), d(vmax.x - vmin.x), d(vmax.y - vmin.y));

        // large soft biome patches
        let big = 320.0;
        let bx0 = (vmin.x / big).floor() as i32 - 1;
        let by0 = (vmin.y / big).floor() as i32 - 1;
        let bx1 = (vmax.x / big).floor() as i32 + 1;
        let by1 = (vmax.y / big).floor() as i32 + 1;
        for (col, salt, thresh) in [
            ("#263420", 21u32, 0.55f32),
            ("#2b2a1f", 22, 0.78),
            ("#1b2618", 23, 0.6),
        ] {
            c.set_fill_style_str(col);
            c.begin_path();
            for cy in by0..=by1 {
                for cx in bx0..=bx1 {
                    let hv = hashf(cx, cy, salt);
                    if hv > thresh {
                        let px = cx as f32 * big + hashf(cx, cy, salt + 1) * big;
                        let py = cy as f32 * big + hashf(cx, cy, salt + 2) * big;
                        let r = 90.0 + hashf(cx, cy, salt + 3) * 140.0;
                        c.move_to(d(px + r), d(py));
                        let _ = c.ellipse(d(px), d(py), d(r), d(r * 0.75), d(hv * 3.0), 0.0, TAU64);
                    }
                }
            }
            c.fill();
        }

        // fine tile variation
        let ts = 72.0;
        let x0 = (vmin.x / ts).floor() as i32;
        let y0 = (vmin.y / ts).floor() as i32;
        let x1 = (vmax.x / ts).floor() as i32;
        let y1 = (vmax.y / ts).floor() as i32;
        c.set_fill_style_str("#222e1d");
        c.begin_path();
        for ty in y0..=y1 {
            for tx in x0..=x1 {
                if hashf(tx, ty, 1) < 0.35 {
                    c.rect(d(tx as f32 * ts), d(ty as f32 * ts), d(ts), d(ts));
                }
            }
        }
        c.fill();

        // grass tufts
        c.set_stroke_style_str("#3b5530");
        c.set_line_width(2.0);
        c.set_line_cap("round");
        c.begin_path();
        let gs = 48.0;
        let gx0 = (vmin.x / gs).floor() as i32;
        let gy0 = (vmin.y / gs).floor() as i32;
        let gx1 = (vmax.x / gs).floor() as i32;
        let gy1 = (vmax.y / gs).floor() as i32;
        let mut flowers: Vec<(V2, u32)> = Vec::new();
        let mut pebbles: Vec<V2> = Vec::new();
        for gy in gy0..=gy1 {
            for gx in gx0..=gx1 {
                let hv = hash2(gx, gy, 31);
                let f = (hv >> 8) as f32 / 16777216.0;
                let px = gx as f32 * gs + ((hv & 31) as f32 / 31.0) * gs;
                let py = gy as f32 * gs + (((hv >> 5) & 31) as f32 / 31.0) * gs;
                if f < 0.42 {
                    for k in 0..3 {
                        let ox = (k as f32 - 1.0) * 3.5;
                        c.move_to(d(px + ox), d(py));
                        c.line_to(d(px + ox * 1.8 + 1.0), d(py - 7.0 - (k as f32 % 2.0) * 3.0));
                    }
                } else if f < 0.47 {
                    flowers.push((v2(px, py), hv));
                } else if f < 0.55 {
                    pebbles.push(v2(px, py));
                }
            }
        }
        c.stroke();
        c.set_fill_style_str("#4a4d44");
        c.begin_path();
        for p in pebbles.iter() {
            path_circle(c, p.x, p.y, 2.5);
        }
        c.fill();
        for (col, m) in [("#c9b3ff", 0u32), ("#ffe38a", 1), ("#ff9fb3", 2)] {
            c.set_fill_style_str(col);
            c.begin_path();
            for (p, hv) in flowers.iter() {
                if hv % 3 == m {
                    path_circle(c, p.x, p.y, 2.2);
                    path_circle(c, p.x + 4.0, p.y + 2.0, 1.8);
                }
            }
            c.fill();
        }
    }

    fn draw_decals(&self, c: &Ctx, vmin: V2, vmax: V2) {
        for dc in self.decals.iter() {
            if dc.pos.x < vmin.x - 60.0
                || dc.pos.x > vmax.x + 60.0
                || dc.pos.y < vmin.y - 60.0
                || dc.pos.y > vmax.y + 60.0
            {
                continue;
            }
            c.set_global_alpha(d((dc.life / 4.0).min(1.0) * 0.75));
            c.set_fill_style_str(dc.color);
            c.begin_path();
            path_circle(c, dc.pos.x, dc.pos.y, dc.r * 0.7);
            for k in 0..5u32 {
                let hv = dc.seed.wrapping_mul(2654435761).wrapping_add(k * 97);
                let a = (hv % 628) as f32 / 100.0;
                let dist = dc.r * (0.5 + ((hv >> 10) % 100) as f32 / 100.0);
                let rr = dc.r * (0.15 + ((hv >> 17) % 30) as f32 / 100.0);
                path_circle(c, dc.pos.x + a.cos() * dist, dc.pos.y + a.sin() * dist, rr);
            }
            c.fill();
        }
        c.set_global_alpha(1.0);
    }

    fn draw_shadows(&self, c: &Ctx, a: f32, obs: &[Obstacle]) {
        c.set_fill_style_str("#000000");
        // tree canopy shadows (big & soft)
        c.set_global_alpha(0.22);
        c.begin_path();
        for o in obs.iter() {
            if o.kind == ObKind::Tree {
                let p = o.pos + v2(22.0, 16.0);
                let r = o.r * 2.3;
                c.move_to(d(p.x + r), d(p.y));
                let _ = c.ellipse(d(p.x), d(p.y), d(r), d(r * 0.8), 0.0, 0.0, TAU64);
            }
        }
        c.fill();
        c.set_global_alpha(0.3);
        c.begin_path();
        for o in obs.iter() {
            if o.kind != ObKind::Tree {
                let p = o.pos + v2(5.0, 6.0);
                c.move_to(d(p.x + o.r * 1.1), d(p.y));
                let _ = c.ellipse(d(p.x), d(p.y), d(o.r * 1.1), d(o.r * 0.85), 0.0, 0.0, TAU64);
            }
        }
        for z in self.zombies.iter().filter(|z| !z.hidden) {
            let p = z.prev.lerp(z.pos, self.za(a)) + v2(3.0, z.r * 0.55);
            c.move_to(d(p.x + z.r * 1.05), d(p.y));
            let _ = c.ellipse(d(p.x), d(p.y), d(z.r * 1.05), d(z.r * 0.5), 0.0, 0.0, TAU64);
        }
        if !matches!(self.phase, Phase::Menu | Phase::Dying | Phase::GameOver) {
            let p = self.player.prev.lerp(self.player.pos, a) + v2(3.0, PR * 0.6);
            c.move_to(d(p.x + PR * 1.1), d(p.y));
            let _ = c.ellipse(d(p.x), d(p.y), d(PR * 1.1), d(PR * 0.5), 0.0, 0.0, TAU64);
        }
        c.fill();
        c.set_global_alpha(1.0);
    }

    fn draw_gems(&self, c: &Ctx, a: f32, t: f32) {
        let groups: [(&str, u32, u32); 4] = [
            ("#5ee7ff", 0, 2),
            ("#6dff8a", 3, 9),
            ("#c77dff", 10, 29),
            ("#ffd34a", 30, u32::MAX),
        ];
        // glow pass
        let _ = c.set_global_composite_operation("lighter");
        c.set_global_alpha(0.22);
        for (col, lo, hi) in groups.iter() {
            c.set_fill_style_str(col);
            c.begin_path();
            for g in self.gems.iter() {
                if g.value >= *lo && g.value <= *hi {
                    let p = g.prev.lerp(g.pos, a);
                    let s = 5.0 + (g.value as f32).sqrt() * 1.2;
                    path_circle(c, p.x, p.y, s * 2.0);
                }
            }
            c.fill();
        }
        let _ = c.set_global_composite_operation("source-over");
        c.set_global_alpha(1.0);
        for (col, lo, hi) in groups.iter() {
            c.set_fill_style_str(col);
            c.begin_path();
            for g in self.gems.iter() {
                if g.value >= *lo && g.value <= *hi {
                    let p = g.prev.lerp(g.pos, a);
                    let s = 4.0 + (g.value as f32).sqrt() * 1.1;
                    let bob = (t * 4.0 + p.x * 0.05).sin() * 2.0;
                    let y = p.y - 4.0 + bob;
                    c.move_to(d(p.x), d(y - s * 1.3));
                    c.line_to(d(p.x + s), d(y));
                    c.line_to(d(p.x), d(y + s * 1.3));
                    c.line_to(d(p.x - s), d(y));
                    c.close_path();
                }
            }
            c.fill();
        }
        // pickups
        c.set_text_align("center");
        c.set_text_baseline("middle");
        c.set_font("22px 'Segoe UI Emoji','Apple Color Emoji','Noto Color Emoji',sans-serif");
        for p in self.pickups.iter() {
            let bob = (t * 3.0 + p.pos.x).sin() * 3.0;
            let pulse = 0.5 + 0.5 * (t * 5.0).sin();
            let col = match p.kind {
                PickKind::Heart => "#ff4d6d",
                PickKind::Vacuum => "#b48cff",
                PickKind::Frenzy => "#ffd84a",
                PickKind::Chest => "#ffb830",
                _ => "#7affb0",
            };
            let blink = p.t > 24.0 && ((t * 10.0) as i32 % 2 == 0);
            if blink {
                continue;
            }
            c.set_global_alpha(d(0.35 + 0.25 * pulse));
            c.set_stroke_style_str(col);
            c.set_line_width(2.0);
            c.begin_path();
            let _ = c.arc(d(p.pos.x), d(p.pos.y), d(16.0 + pulse * 4.0), 0.0, TAU64);
            c.stroke();
            c.set_global_alpha(1.0);
            let icon = match p.kind {
                PickKind::Heart => "❤️",
                PickKind::Vacuum => "🧲",
                PickKind::Frenzy => "⚡",
                PickKind::Chest => "🎁",
                PickKind::MaxAmmo => "🎒",
                PickKind::InstaKill => "💀",
                PickKind::DoublePoints => "✖️",
                PickKind::Nuke => "☢️",
                PickKind::Carpenter => "🔨",
            };
            text(c, icon, p.pos.x, p.pos.y - 4.0 + bob);
        }
    }

    fn draw_telegraphs(&self, c: &Ctx, a: f32, t: f32) {
        for sl in self.slams.iter() {
            let k = (sl.t / sl.dur).clamp(0.0, 1.0);
            let col = self.access.color(if sl.fire { "#ff7a1f" } else { "#ff2040" });
            c.set_global_alpha(d(0.12 + 0.2 * k));
            c.set_fill_style_str(col);
            circle(c, sl.pos.x, sl.pos.y, sl.r);
            c.set_global_alpha(0.55);
            circle(c, sl.pos.x, sl.pos.y, sl.r * k);
            c.set_global_alpha(d(0.6 + 0.4 * (t * 25.0).sin().abs()));
            c.set_stroke_style_str(col);
            c.set_line_width(3.0);
            if self.access.colorblind > 0 {
                // shape-coded: dashed ring for "area attack incoming"
                let _ = c.set_line_dash(&js_sys::Array::of2(&14.0.into(), &8.0.into()));
            }
            c.begin_path();
            let _ = c.arc(d(sl.pos.x), d(sl.pos.y), d(sl.r), 0.0, TAU64);
            c.stroke();
            let _ = c.set_line_dash(&js_sys::Array::new());
            c.set_global_alpha(1.0);
        }
        for z in self.zombies.iter() {
            let p = z.prev.lerp(z.pos, self.za(a));
            if z.kind == ZKind::Boss && z.state == 3 && z.boss_var == crate::content::BossVar::Butcher {
                // hook line: a dashed chain that fills in before the throw
                let k = 1.0 - (z.timer / 0.8).clamp(0.0, 1.0);
                let end = p + z.charge_dir * 460.0;
                c.set_stroke_style_str("#ff4060");
                c.set_global_alpha(d(0.35 + 0.5 * k));
                c.set_line_width(d(6.0 + 6.0 * k));
                let _ = c.set_line_dash(&js_sys::Array::of2(&12.0.into(), &10.0.into()));
                c.begin_path();
                c.move_to(d(p.x), d(p.y));
                c.line_to(d(end.x), d(end.y));
                c.stroke();
                let _ = c.set_line_dash(&js_sys::Array::new());
                c.set_global_alpha(1.0);
            }
            if z.kind == ZKind::Boss && z.state == 1 {
                let k = 1.0 - (z.timer / 0.75).clamp(0.0, 1.0);
                c.save();
                let _ = c.translate(d(p.x), d(p.y));
                let _ = c.rotate(d(z.charge_dir.angle()));
                c.set_global_alpha(d(0.15 + 0.25 * k));
                c.set_fill_style_str("#ff2040");
                c.fill_rect(0.0, d(-z.r), d(400.0), d(z.r * 2.0));
                c.set_global_alpha(d(0.6));
                c.fill_rect(0.0, d(-z.r), d(400.0 * k), d(z.r * 2.0));
                c.restore();
                c.set_global_alpha(1.0);
            }
            if z.kind == ZKind::Spitter && z.timer < 0.35 && !self.player.dead {
                let k = 1.0 - z.timer / 0.35;
                let tgt = self.player.prev.lerp(self.player.pos, a);
                if tgt.dist(p) < 560.0 {
                    c.set_stroke_style_str("#9be04a");
                    c.set_global_alpha(d(0.15 + 0.35 * k));
                    c.set_line_width(2.0);
                    c.begin_path();
                    c.move_to(d(p.x), d(p.y));
                    c.line_to(d(tgt.x), d(tgt.y));
                    c.stroke();
                    c.set_global_alpha(1.0);
                }
            }
            if z.kind == ZKind::Brute && z.state == 1 {
                // rock held overhead, landing line ahead
                let k = 1.0 - (z.timer / 0.65).clamp(0.0, 1.0);
                let hand = p + v2(0.0, -z.r * 2.0 - 8.0 * k);
                c.set_fill_style_str("#6b6e74");
                circle(c, hand.x, hand.y, 13.0);
                c.set_stroke_style_str("#ff6040");
                c.set_global_alpha(d(0.2 + 0.4 * k));
                c.set_line_width(3.0);
                c.begin_path();
                c.move_to(d(p.x), d(p.y));
                let e = p + z.charge_dir * 360.0;
                c.line_to(d(e.x), d(e.y));
                c.stroke();
                c.set_global_alpha(1.0);
            }
            if z.kind == ZKind::Exploder && z.fuse > 0.0 {
                let k = 1.0 - z.fuse / 0.55;
                c.set_global_alpha(d(0.15 + 0.3 * k));
                c.set_fill_style_str("#ff5a1f");
                circle(c, p.x, p.y, 115.0 * (0.6 + 0.4 * k));
                c.set_global_alpha(d(0.5 + 0.5 * (t * 30.0).sin().abs()));
                c.set_stroke_style_str("#ffb070");
                c.set_line_width(2.0);
                c.begin_path();
                let _ = c.arc(d(p.x), d(p.y), 115.0, 0.0, TAU64);
                c.stroke();
                c.set_global_alpha(1.0);
            }
        }
    }

    fn draw_rings(&self, c: &Ctx) {
        let _ = c.set_global_composite_operation("lighter");
        for r in self.rings.iter() {
            let k = (r.life / r.max).clamp(0.0, 1.0);
            c.set_global_alpha(d(k * 0.85));
            c.set_stroke_style_str(r.color);
            c.set_line_width(d(r.width * (0.4 + k)));
            c.begin_path();
            let _ = c.arc(d(r.pos.x), d(r.pos.y), d(r.r.max(1.0)), 0.0, TAU64);
            c.stroke();
            c.set_global_alpha(d(k * 0.12));
            c.set_fill_style_str(r.color);
            c.fill();
        }
        let _ = c.set_global_composite_operation("source-over");
        c.set_global_alpha(1.0);
    }

    fn draw_ghosts(&self, c: &Ctx) {
        for g in self.ghosts.iter() {
            c.set_global_alpha(d(g.life / 0.25 * 0.45));
            c.set_fill_style_str("#7fa7ff");
            circle(c, g.pos.x, g.pos.y, PR);
        }
        c.set_global_alpha(1.0);
    }

    fn draw_player(&self, c: &Ctx, p: V2, t: f32, v: &PView) {
        let blink = v.blink && ((t * 24.0) as i32 % 2 == 0);
        if v.downed {
            let pulse = 0.5 + 0.5 * (t * 5.0).sin();
            c.set_global_alpha(d(0.35 + 0.3 * pulse));
            c.set_stroke_style_str("#ff4a4a");
            c.set_line_width(3.0);
            c.begin_path();
            let _ = c.arc(d(p.x), d(p.y), d(REVIVE_R), 0.0, TAU64);
            c.stroke();
            if v.revive > 0.0 {
                c.set_global_alpha(1.0);
                c.set_stroke_style_str("#9cffb8");
                c.set_line_width(5.0);
                c.begin_path();
                let _ = c.arc(
                    d(p.x),
                    d(p.y),
                    d(REVIVE_R),
                    -PI as f64 / 2.0,
                    -PI as f64 / 2.0 + TAU64 * v.revive as f64,
                );
                c.stroke();
            }
            c.set_global_alpha(0.55);
        }
        if blink {
            c.set_global_alpha(0.45);
        }
        let aim = v.aim;
        let fwd = V2::from_angle(aim);
        let side = fwd.perp();
        let moving = v.vel.len() > 30.0;
        let step = if moving { (v.walk * 2.2).sin() } else { 0.0 };

        let skin = &SKINS[v.skin.min(SKINS.len() - 1)];
        let gun = &GUNS[v.gun.min(GUNS.len() - 1)];
        // feet
        c.set_fill_style_str(skin.feet);
        let mv = if moving { v.vel.norm() } else { fwd };
        let f1 = p + side * 7.0 + mv * (step * 6.0);
        let f2 = p - side * 7.0 - mv * (step * 6.0);
        circle(c, f1.x, f1.y + 4.0, 5.0);
        circle(c, f2.x, f2.y + 4.0, 5.0);

        // backpack
        let bp = p - fwd * 11.0;
        c.set_fill_style_str("#5a4630");
        circle(c, bp.x, bp.y - 2.0, 8.5);

        // gun
        let rec = v.recoil * 4.0;
        let g0 = p + fwd * (6.0 - rec) + side * 5.0;
        let g1 = p + fwd * (gun.len + 6.0 - rec) + side * 5.0;
        let camo = if v.gun == self.gun && !crate::content::CAMOS[self.camo].is_empty() {
            crate::content::CAMOS[self.camo]
        } else {
            gun.color
        };
        c.set_stroke_style_str(camo);
        c.set_line_width(d(gun.width));
        c.set_line_cap("round");
        c.begin_path();
        c.move_to(d(g0.x), d(g0.y - 4.0));
        c.line_to(d(g1.x), d(g1.y - 4.0));
        c.stroke();
        if gun.pellets > 1 {
            // double barrel
            let g2 = g1 - side * 3.0;
            c.set_stroke_style_str("#1a1a1e");
            c.set_line_width(3.0);
            c.begin_path();
            c.move_to(d(g0.x - side.x * 3.0), d(g0.y - side.y * 3.0 - 4.0));
            c.line_to(d(g2.x), d(g2.y - 4.0));
            c.stroke();
        }
        c.set_stroke_style_str("#6b7280");
        c.set_line_width((gun.width * 0.4).max(2.0) as f64);
        c.begin_path();
        c.move_to(d(g0.x), d(g0.y - 5.0));
        c.line_to(d(g1.x), d(g1.y - 5.0));
        c.stroke();

        // body (two-tone for volume)
        let body = p + v2(0.0, -4.0);
        c.set_fill_style_str(skin.dark);
        circle(c, body.x, body.y, PR);
        c.set_fill_style_str(skin.light);
        circle(c, body.x - 2.0, body.y - 3.0, PR * 0.78);
        if v.frenzy {
            let _ = c.set_global_composite_operation("lighter");
            c.set_global_alpha(d(0.25 + 0.15 * (t * 20.0).sin()));
            c.set_fill_style_str("#ffd84a");
            circle(c, body.x, body.y, PR + 8.0);
            let _ = c.set_global_composite_operation("source-over");
            c.set_global_alpha(if blink { 0.45 } else { 1.0 });
        }
        if let Some(col) = v.ability {
            let _ = c.set_global_composite_operation("lighter");
            c.set_global_alpha(d(0.3 + 0.2 * (t * 14.0).sin()));
            c.set_fill_style_str(col);
            circle(c, body.x, body.y, PR + 11.0);
            let _ = c.set_global_composite_operation("source-over");
            c.set_global_alpha(if blink { 0.45 } else { 1.0 });
        }
        // hands on the gun
        c.set_fill_style_str("#e8b48a");
        let h1 = p + fwd * (16.0 - rec) + side * 7.0;
        circle(c, h1.x, h1.y - 4.0, 4.0);
        // head
        let head = p + fwd * 3.0 + v2(0.0, -9.0);
        c.set_fill_style_str("#e8b48a");
        circle(c, head.x, head.y, 9.0);
        // hair / helmet
        c.set_fill_style_str("#3a2a1e");
        c.begin_path();
        let _ = c.arc(
            d(head.x),
            d(head.y),
            9.5,
            d(aim + PI * 0.55),
            d(aim + PI * 1.45),
        );
        c.fill();
        // eyes
        c.set_fill_style_str("#111");
        let e1 = head + fwd * 5.0 + side * 3.0;
        let e2 = head + fwd * 5.0 - side * 3.0;
        circle(c, e1.x, e1.y, 1.4);
        circle(c, e2.x, e2.y, 1.4);
        self.draw_hat(c, head, aim, t, v.hat);
        if v.hurt > 0.0 {
            c.set_global_alpha(d(v.hurt * 0.7));
            c.set_fill_style_str("#ff2a2a");
            circle(c, body.x, body.y, PR + 2.0);
        }
        c.set_global_alpha(1.0);

        // muzzle flash
        if v.recoil > 0.6 {
            let m = p + fwd * (gun.len + 10.0) + side * 5.0 + v2(0.0, -4.0);
            let _ = c.set_global_composite_operation("lighter");
            c.set_fill_style_str("#ffcf6a");
            c.set_global_alpha(d((v.recoil - 0.6) * 2.2));
            circle(c, m.x, m.y, 9.0);
            c.set_fill_style_str("#ffffff");
            circle(c, m.x, m.y, 4.0);
            let _ = c.set_global_composite_operation("source-over");
            c.set_global_alpha(1.0);
        }
    }

    fn draw_hat(&self, c: &Ctx, head: V2, aim: f32, t: f32, hat: usize) {
        let fwd = V2::from_angle(aim);
        let side = fwd.perp();
        match HATS[hat.min(HATS.len() - 1)] {
            "cap" => {
                c.set_fill_style_str("#c0283a");
                circle(c, head.x - fwd.x, head.y - fwd.y - 1.0, 8.5);
                let b = head + fwd * 8.0;
                c.begin_path();
                let _ = c.ellipse(d(b.x), d(b.y - 1.0), 6.0, 3.5, d(aim), 0.0, TAU64);
                c.fill();
            }
            "headband" => {
                c.set_stroke_style_str("#e02020");
                c.set_line_width(3.0);
                c.begin_path();
                let _ = c.arc(d(head.x), d(head.y), 8.5, d(aim - 1.3), d(aim + 1.3));
                c.stroke();
                let back = head - fwd * 9.0;
                let sway = (t * 8.0).sin() * 3.0;
                c.begin_path();
                c.move_to(d(back.x), d(back.y));
                c.line_to(
                    d(back.x - fwd.x * 9.0 + side.x * sway),
                    d(back.y - fwd.y * 9.0 + side.y * sway),
                );
                c.move_to(d(back.x), d(back.y));
                c.line_to(
                    d(back.x - fwd.x * 8.0 - side.x * (4.0 + sway)),
                    d(back.y - fwd.y * 8.0 - side.y * (4.0 + sway)),
                );
                c.stroke();
            }
            "cowboy" => {
                c.set_fill_style_str("#6a4424");
                c.begin_path();
                let _ = c.ellipse(d(head.x), d(head.y), 15.0, 12.0, d(aim), 0.0, TAU64);
                c.fill();
                c.set_fill_style_str("#8a5a30");
                circle(c, head.x, head.y - 1.0, 8.0);
                c.set_fill_style_str("#3a2414");
                c.fill_rect(d(head.x - 8.0), d(head.y + 1.0), 16.0, 2.0);
            }
            "tophat" => {
                c.set_fill_style_str("#111116");
                circle(c, head.x, head.y, 11.0);
                c.set_fill_style_str("#1c1c24");
                circle(c, head.x, head.y - 3.0, 7.5);
                c.set_stroke_style_str("#a0203a");
                c.set_line_width(2.0);
                c.begin_path();
                let _ = c.arc(d(head.x), d(head.y - 3.0), 7.5, 0.0, TAU64);
                c.stroke();
            }
            "horns" => {
                c.set_fill_style_str("#ece0c8");
                for sgn in [-1.0f32, 1.0] {
                    let base = head + side * (7.0 * sgn) - fwd * 1.0;
                    let tip = head + side * (15.0 * sgn) + fwd * 3.0 + v2(0.0, -7.0);
                    let w = fwd * 3.0;
                    c.begin_path();
                    c.move_to(d(base.x + w.x), d(base.y + w.y));
                    c.line_to(d(tip.x), d(tip.y));
                    c.line_to(d(base.x - w.x), d(base.y - w.y));
                    c.close_path();
                    c.fill();
                }
                c.set_fill_style_str("#8a8a90");
                circle(c, head.x, head.y - 1.0, 8.0);
            }
            "crown" => {
                c.set_fill_style_str("#ffcf3a");
                c.begin_path();
                for k in 0..10 {
                    let an = k as f32 * TAU / 10.0;
                    let rr = if k % 2 == 0 { 10.5 } else { 6.5 };
                    let q = head + V2::from_angle(an) * rr + v2(0.0, -2.0);
                    if k == 0 {
                        c.move_to(d(q.x), d(q.y));
                    } else {
                        c.line_to(d(q.x), d(q.y));
                    }
                }
                c.close_path();
                c.fill();
                c.set_fill_style_str("#e0203a");
                circle(c, head.x, head.y - 2.0, 2.5);
            }
            "halo" => {
                let _ = c.set_global_composite_operation("lighter");
                c.set_stroke_style_str("#ffe68a");
                c.set_line_width(3.0);
                c.begin_path();
                let bob = (t * 3.0).sin() * 1.5;
                let _ = c.ellipse(
                    d(head.x),
                    d(head.y - 12.0 + bob),
                    10.0,
                    4.0,
                    0.0,
                    0.0,
                    TAU64,
                );
                c.stroke();
                let _ = c.set_global_composite_operation("source-over");
            }
            _ => {}
        }
    }

    /// Squash & stretch on hit, and a lean in the direction it's being knocked.
    fn draw_zombie(&self, c: &Ctx, i: usize, a: f32, t: f32) {
        let z = &self.zombies[i];
        let p = z.prev.lerp(z.pos, self.za(a));
        let f = if self.access.reduce_motion { 0.0 } else { z.flash.clamp(0.0, 1.0) };
        let lean = if self.access.reduce_motion { 0.0 } else { (z.vel.x / 900.0).clamp(-0.22, 0.22) } * if z.kind == ZKind::Boss { 0.3 } else { 1.0 };
        if f < 0.01 && lean.abs() < 0.01 {
            return self.draw_zombie_body(c, i, a, t);
        }
        c.save();
        let _ = c.translate(d(p.x), d(p.y));
        let _ = c.rotate(d(lean));
        let _ = c.scale(d(1.0 + 0.16 * f), d(1.0 - 0.13 * f));
        let _ = c.translate(d(-p.x), d(-p.y));
        self.draw_zombie_body(c, i, a, t);
        c.restore();
    }

    fn draw_corpses(&self, c: &Ctx) {
        for co in self.corpses.iter() {
            let k = (co.t / crate::corpses::CORPSE_T).clamp(0.0, 1.0);
            let fall = (co.t / 0.25).min(1.0);
            let (cloth, skin) = if co.kind == ZKind::Boss {
                let (cl, sk, _) = co.boss_var.colors();
                (cl, sk)
            } else {
                let def = co.kind.def();
                let n = def.cloth.len();
                (def.cloth[((co.tint * n as f32) as usize).min(n - 1)], def.skin)
            };
            let frozen = co.elem & crate::status::FROZEN_BIT != 0;
            let burnt = co.elem & crate::status::Elem::Burn.bit() != 0;
            c.save();
            let _ = c.translate(d(co.pos.x), d(co.pos.y));
            let _ = c.rotate(d(co.face + fall * 1.2));
            let _ = c.scale(1.0, d(1.0 - 0.45 * fall));
            c.set_global_alpha(d((1.0 - k) * 0.95));
            c.set_fill_style_str(if burnt { "#2a2a2a" } else if frozen { "#bff4ff" } else { cloth });
            circle(c, 0.0, 0.0, co.r * (1.0 - 0.25 * k));
            c.set_fill_style_str(if burnt { "#1a1a1a" } else if frozen { "#e0fbff" } else { skin });
            circle(c, co.r * 0.7, 0.0, co.r * 0.5 * (1.0 - 0.25 * k));
            c.restore();
        }
        c.set_global_alpha(1.0);
    }

    /// Level-of-detail zombie for huge hordes: shadowless flat shapes only.
    fn draw_zombie_lod(&self, c: &Ctx, i: usize, a: f32) {
        let z = &self.zombies[i];
        let p = z.prev.lerp(z.pos, self.za(a));
        let r = z.r;
        let def = z.kind.def();
        let (cloth, skin) = if z.kind == ZKind::Boss {
            let (cl, sk, _) = z.boss_var.colors();
            (cl, sk)
        } else {
            (def.cloth[0], def.skin)
        };
        let fwd = V2::from_angle(z.face);
        c.set_fill_style_str(if z.flash > 0.3 { "#ffffff" } else { cloth });
        circle(c, p.x, p.y - r * 0.3, r);
        c.set_fill_style_str(skin);
        let head = p + fwd * (r * 0.32) + v2(0.0, -r * 0.6);
        circle(c, head.x, head.y, r * 0.55);
        if z.st.frozen() {
            c.set_fill_style_str("rgba(191,244,255,0.5)");
            circle(c, p.x, p.y - r * 0.3, r * 1.2);
        }
    }

    fn draw_zombie_body(&self, c: &Ctx, i: usize, a: f32, t: f32) {
        if self.lod && self.zombies[i].kind != ZKind::Boss && self.zombies[i].affix == crate::elites::Affix::None {
            return self.draw_zombie_lod(c, i, a);
        }
        let z = &self.zombies[i];
        let p = z.prev.lerp(z.pos, self.za(a));
        let r = z.r;
        let face = z.face;
        let fwd = V2::from_angle(face);
        let side = fwd.perp();
        let bob = (z.anim * 2.0).sin() * r * 0.08;
        let sway = (z.anim * 1.3).sin() * 0.25;

        let def = z.kind.def();
        if z.kind == ZKind::Burrower && z.state < 2 {
            // a travelling dirt mound; it shakes and cracks before erupting
            let shake = if z.state == 1 { (t * 50.0).sin() * 3.0 } else { 0.0 };
            c.set_fill_style_str("#3a2a14");
            c.begin_path();
            let _ = c.ellipse(d(p.x + shake), d(p.y), d(r * 1.2), d(r * 0.7), 0.0, 0.0, TAU64);
            c.fill();
            c.set_fill_style_str("#5a4020");
            c.begin_path();
            let _ = c.ellipse(d(p.x + shake), d(p.y - 4.0), d(r * 0.9), d(r * 0.5), 0.0, 0.0, TAU64);
            c.fill();
            if z.state == 1 {
                c.set_stroke_style_str("#ffb050");
                c.set_line_width(2.0);
                c.set_global_alpha(d(0.4 + 0.4 * (t * 20.0).sin().abs()));
                c.begin_path();
                let _ = c.arc(d(p.x), d(p.y), d(80.0), 0.0, TAU64);
                c.stroke();
                c.set_global_alpha(1.0);
            }
            return;
        }
        if z.kind == ZKind::Hound {
            // a lean four-legged shape wreathed in flame
            let body = p + v2(0.0, -6.0);
            let stride = (z.anim * 3.5).sin();
            c.set_stroke_style_str(def.skin);
            c.set_line_width(4.0);
            c.begin_path();
            for (k, s2) in [(0.6f32, 1.0f32), (0.6, -1.0), (-0.6, 1.0), (-0.6, -1.0)] {
                let hip = body + fwd * (r * k) + side * (r * 0.4 * s2);
                let foot = hip + side * (r * 0.5 * s2) + fwd * (r * 0.5 * stride * s2 * k.signum());
                c.move_to(d(hip.x), d(hip.y));
                c.line_to(d(foot.x), d(foot.y));
            }
            c.stroke();
            c.set_fill_style_str(def.dark);
            c.begin_path();
            let _ = c.ellipse(d(body.x), d(body.y), d(r * 1.2), d(r * 0.7), d(face), 0.0, TAU64);
            c.fill();
            let head = body + fwd * (r * 1.1);
            c.set_fill_style_str(def.skin);
            circle(c, head.x, head.y, r * 0.55);
            let _ = c.set_global_composite_operation("lighter");
            c.set_fill_style_str("#ff3020");
            for s2 in [-1.0f32, 1.0] {
                let e = head + fwd * (r * 0.3) + side * (r * 0.22 * s2);
                circle(c, e.x, e.y, 2.0);
            }
            for k in 0..3 {
                let ph = t * 8.0 + k as f32 * 2.0 + z.tint * 6.0;
                let fp = body + side * ((ph * 1.3).sin() * r * 0.5) + v2(0.0, -(ph.fract()) * r * 1.4);
                c.set_global_alpha(d(0.6 * (1.0 - ph.fract())));
                c.set_fill_style_str(if k % 2 == 0 { "#ff7a1f" } else { "#ffd23a" });
                circle(c, fp.x, fp.y, r * 0.3);
            }
            c.set_global_alpha(1.0);
            let _ = c.set_global_composite_operation("source-over");
            if z.flash > 0.0 {
                c.set_global_alpha(d(z.flash * 0.85));
                c.set_fill_style_str("#ffffff");
                circle(c, body.x, body.y, r);
                c.set_global_alpha(1.0);
            }
            return;
        }
        if z.kind == ZKind::Bat {
            let flap = (z.anim * 4.0).sin();
            let lifted = p + v2(0.0, -26.0);
            c.set_fill_style_str(def.skin);
            for s in [-1.0f32, 1.0] {
                c.begin_path();
                c.move_to(d(lifted.x), d(lifted.y));
                c.line_to(d(lifted.x + s * r * 2.2), d(lifted.y - r * (0.6 + flap * 0.9)));
                c.line_to(d(lifted.x + s * r * 1.4), d(lifted.y + r * 0.3));
                c.close_path();
                c.fill();
            }
            c.set_fill_style_str(def.dark);
            circle(c, lifted.x, lifted.y, r * 0.75);
            let _ = c.set_global_composite_operation("lighter");
            c.set_fill_style_str("#ff3020");
            circle(c, lifted.x - 2.5, lifted.y - 1.5, 1.6);
            circle(c, lifted.x + 2.5, lifted.y - 1.5, 1.6);
            let _ = c.set_global_composite_operation("source-over");
            if z.flash > 0.0 {
                c.set_global_alpha(d(z.flash * 0.85));
                c.set_fill_style_str("#ffffff");
                circle(c, lifted.x, lifted.y, r);
                c.set_global_alpha(1.0);
            }
            return;
        }
        let (cloth, skin, dark) = if z.kind == ZKind::Boss {
            z.boss_var.colors()
        } else {
            let n = def.cloth.len();
            (def.cloth[((z.tint * n as f32) as usize).min(n - 1)], def.skin, def.dark)
        };

        // elite aura under the body
        let elite = z.affix != crate::elites::Affix::None;
        if elite {
            let (_, _, col) = z.affix.def();
            let pulse = 0.5 + 0.5 * (t * 4.0 + z.tint * 6.0).sin();
            let _ = c.set_global_composite_operation("lighter");
            c.set_global_alpha(d(0.25 + 0.2 * pulse));
            c.set_fill_style_str(col);
            c.begin_path();
            let _ = c.ellipse(d(p.x), d(p.y + r * 0.3), d(r * 1.5), d(r * 0.8), 0.0, 0.0, TAU64);
            c.fill();
            c.set_global_alpha(1.0);
            let _ = c.set_global_composite_operation("source-over");
        }

        // arms reaching toward the player
        let arm_w = r * 0.42;
        c.set_stroke_style_str(skin);
        c.set_line_width(d(arm_w));
        c.set_line_cap("round");
        c.begin_path();
        for s in [-1.0f32, 1.0] {
            let sh = p + side * (r * 0.75 * s) + v2(0.0, -r * 0.35 + bob);
            let reach = r * (1.05 + 0.15 * (z.anim * 2.0 + s).sin());
            let hand = sh + V2::from_angle(face + sway * s * 0.4) * reach;
            c.move_to(d(sh.x), d(sh.y));
            c.line_to(d(hand.x), d(hand.y));
        }
        c.stroke();

        // body
        let body = p + v2(0.0, -r * 0.3 + bob);
        c.set_fill_style_str(dark);
        circle(c, body.x, body.y, r);
        c.set_fill_style_str(cloth);
        circle(c, body.x - r * 0.1, body.y - r * 0.12, r * 0.84);

        // special features
        match z.kind {
            ZKind::Spitter => {
                let swell = if z.timer < 0.4 {
                    1.0 + (0.4 - z.timer) * 1.2
                } else {
                    1.0
                };
                c.set_fill_style_str("#b6f05a");
                let sac = body - fwd * (r * 0.3);
                circle(c, sac.x, sac.y, r * 0.5 * swell);
            }
            ZKind::Exploder => {
                let pulse = if z.fuse > 0.0 {
                    (t * 40.0).sin().abs()
                } else {
                    0.4 + 0.3 * (t * 6.0 + z.tint * 9.0).sin()
                };
                let _ = c.set_global_composite_operation("lighter");
                c.set_global_alpha(d(0.3 + 0.5 * pulse));
                c.set_fill_style_str("#ff7a1f");
                circle(c, body.x, body.y, r * 0.75);
                let _ = c.set_global_composite_operation("source-over");
                c.set_global_alpha(1.0);
            }
            ZKind::Crawler => {
                // splayed legs
                c.set_stroke_style_str(skin);
                c.set_line_width(d(r * 0.25));
                c.begin_path();
                for k in 0..4 {
                    let side_s = if k < 2 { -1.0 } else { 1.0 };
                    let along = if k % 2 == 0 { 0.5 } else { -0.4 };
                    let swing = (z.anim * 3.0 + k as f32).sin() * 0.4;
                    let base = body + fwd * (r * along);
                    let foot = base + side * (r * 1.3 * side_s) + fwd * (r * swing);
                    c.move_to(d(base.x), d(base.y));
                    c.line_to(d(foot.x), d(foot.y));
                }
                c.stroke();
            }
            ZKind::Screamer => {
                let open = if z.timer < 0.5 { 1.0 + (0.5 - z.timer) * 2.0 } else { 0.6 };
                let mouth = body + fwd * (r * 0.75) + v2(0.0, -r * 0.2);
                c.set_fill_style_str("#1a0008");
                circle(c, mouth.x, mouth.y, r * 0.32 * open);
                if z.rage_t > 0.0 || z.timer < 0.5 {
                    c.set_stroke_style_str("#ff8ac0");
                    c.set_line_width(1.5);
                    c.set_global_alpha(0.6);
                    for k in 1..3 {
                        c.begin_path();
                        let rr = r * (0.6 + k as f32 * 0.5 + (t * 3.0).fract() * 0.5);
                        let _ = c.arc(d(mouth.x), d(mouth.y), d(rr), d(face - 0.6), d(face + 0.6));
                        c.stroke();
                    }
                    c.set_global_alpha(1.0);
                }
            }
            ZKind::Armored => {
                // riveted plate across the front
                c.set_stroke_style_str("#9aa3b0");
                c.set_line_width(d(r * 0.38));
                c.set_line_cap("butt");
                c.begin_path();
                let _ = c.arc(d(body.x), d(body.y), d(r * 0.95), d(face - 1.0), d(face + 1.0));
                c.stroke();
                c.set_line_cap("round");
                c.set_fill_style_str("#d0d6e0");
                for k in [-0.7f32, 0.0, 0.7] {
                    let rv = body + V2::from_angle(face + k) * (r * 0.95);
                    circle(c, rv.x, rv.y, 1.8);
                }
            }
            ZKind::Brute => {
                c.set_fill_style_str("#262a31");
                for s in [-1.0f32, 1.0] {
                    let sp = body + side * (r * 0.7 * s);
                    circle(c, sp.x, sp.y, r * 0.38);
                }
            }
            ZKind::Boss => {
                // glowing weak point on the back
                let wp = body - fwd * (r * 0.75);
                let pulse = 0.6 + 0.4 * (t * 6.0).sin();
                let _ = c.set_global_composite_operation("lighter");
                c.set_global_alpha(d(0.5 * pulse));
                c.set_fill_style_str("#ffe14a");
                circle(c, wp.x, wp.y, r * 0.42);
                c.set_global_alpha(1.0);
                c.set_fill_style_str("#fff6c0");
                circle(c, wp.x, wp.y, r * 0.18);
                let _ = c.set_global_composite_operation("source-over");
                match z.boss_var {
                    crate::content::BossVar::Butcher => {
                        // a cleaver in the right hand
                        let hand = body + side * (r * 0.95) + fwd * (r * 0.6);
                        c.set_fill_style_str("#c8ccd4");
                        c.save();
                        let _ = c.translate(d(hand.x), d(hand.y));
                        let _ = c.rotate(d(face + 0.4 * (z.anim * 2.0).sin()));
                        c.fill_rect(0.0, d(-r * 0.25), d(r * 0.9), d(r * 0.5));
                        c.set_fill_style_str("#4a3020");
                        c.fill_rect(d(-r * 0.4), d(-r * 0.08), d(r * 0.4), d(r * 0.16));
                        c.restore();
                    }
                    crate::content::BossVar::HiveQueen => {
                        let flap = (z.anim * 3.0).sin();
                        c.set_global_alpha(0.75);
                        c.set_fill_style_str("#b08ad0");
                        for s2 in [-1.0f32, 1.0] {
                            c.begin_path();
                            c.move_to(d(body.x), d(body.y));
                            c.line_to(d(body.x + s2 * r * 2.2), d(body.y - r * (1.2 + flap * 0.6)));
                            c.line_to(d(body.x + s2 * r * 1.6), d(body.y + r * 0.4));
                            c.close_path();
                            c.fill();
                        }
                        c.set_global_alpha(1.0);
                    }
                    _ => {}
                }
                // bony spikes along the back
                c.set_fill_style_str("#d8cce0");
                for k in 0..5 {
                    let an = face + PI + (k as f32 - 2.0) * 0.38;
                    let b0 = body + V2::from_angle(an) * (r * 0.8);
                    let tip = body + V2::from_angle(an) * (r * 1.3);
                    let s2 = V2::from_angle(an).perp() * (r * 0.12);
                    c.begin_path();
                    c.move_to(d(b0.x + s2.x), d(b0.y + s2.y));
                    c.line_to(d(tip.x), d(tip.y));
                    c.line_to(d(b0.x - s2.x), d(b0.y - s2.y));
                    c.fill();
                }
            }
            _ => {}
        }

        // head
        let head = body + fwd * (r * 0.32) + v2(0.0, -r * 0.3);
        let hr = r * if z.kind == ZKind::Boss { 0.5 } else { 0.6 } * if self.mutators.bighead { 1.8 } else { 1.0 };
        c.set_fill_style_str(skin);
        circle(c, head.x, head.y, hr);
        // glowing eyes
        let _ = c.set_global_composite_operation("lighter");
        let eye_col = if z.kind == ZKind::Boss {
            "#ff40ff"
        } else if z.st.chill > 0.05 || z.st.frozen() {
            "#80e0ff"
        } else {
            "#ff3020"
        };
        c.set_fill_style_str(eye_col);
        for s in [-1.0f32, 1.0] {
            let e = head + fwd * (hr * 0.55) + side * (hr * 0.38 * s);
            circle(c, e.x, e.y, hr * 0.2);
        }
        let _ = c.set_global_composite_operation("source-over");

        // status overlays
        let st = &z.st;
        if st.frozen() {
            // an ice shell with a highlight
            c.set_global_alpha(0.55);
            c.set_fill_style_str("#bff4ff");
            circle(c, body.x, body.y - r * 0.15, r * 1.25);
            c.set_global_alpha(0.8);
            c.set_fill_style_str("#ffffff");
            circle(c, body.x - r * 0.45, body.y - r * 0.6, r * 0.22);
            c.set_global_alpha(1.0);
        } else if st.chill > 0.05 {
            c.set_global_alpha(d(0.2 + 0.3 * st.chill.min(1.0)));
            c.set_fill_style_str("#8fe8ff");
            circle(c, body.x, body.y, r * 1.02);
            c.set_global_alpha(1.0);
        }
        if st.poison > 0 {
            c.set_global_alpha(0.3);
            c.set_fill_style_str(self.access.color("#7dff3a"));
            circle(c, body.x, body.y, r * 0.95);
            c.set_global_alpha(0.85);
            c.set_fill_style_str("#b6ff5a");
            for k in 0..3 {
                let ph = t * 1.6 + k as f32 * 2.1 + z.tint * 7.0;
                let rise = (ph.fract()) * r * 1.4;
                let bx = body.x + (ph * 3.0).sin() * r * 0.5;
                circle(c, bx, body.y - r * 0.4 - rise, 2.2 + k as f32 * 0.6);
            }
            c.set_global_alpha(1.0);
        }
        if st.burn_t > 0.0 {
            let _ = c.set_global_composite_operation("lighter");
            for k in 0..4 {
                let ph = t * 9.0 + k as f32 * 1.7 + z.tint * 5.0;
                let fx = body.x + (ph * 0.7).sin() * r * 0.6;
                let fy = body.y - r * 0.5 - (ph.fract()) * r * 1.1;
                c.set_global_alpha(d(0.7 * (1.0 - ph.fract())));
                c.set_fill_style_str(if k % 2 == 0 { "#ff7a1f" } else { "#ffd23a" });
                circle(c, fx, fy, r * 0.28 * (1.0 - ph.fract() * 0.6));
            }
            c.set_global_alpha(1.0);
            let _ = c.set_global_composite_operation("source-over");
        }
        if st.shock_t > 0.0 && ((t * 20.0 + z.tint * 10.0) as i32) % 3 == 0 {
            c.set_stroke_style_str("#cfe0ff");
            c.set_line_width(1.6);
            c.begin_path();
            let a0 = t * 13.0 + z.tint * 9.0;
            let mut q = body + V2::from_angle(a0) * r;
            c.move_to(d(q.x), d(q.y));
            for k in 1..4 {
                q = body + V2::from_angle(a0 + k as f32 * 0.9) * (r * (0.8 + 0.4 * ((k * 7) % 3) as f32 / 2.0));
                c.line_to(d(q.x), d(q.y));
            }
            c.stroke();
        }
        if z.rage_t > 0.0 {
            c.set_global_alpha(0.22);
            c.set_fill_style_str("#ff3a6a");
            circle(c, body.x, body.y, r * 1.05);
            c.set_global_alpha(1.0);
        }
        if z.flash > 0.0 {
            c.set_global_alpha(d(z.flash * 0.85));
            c.set_fill_style_str("#ffffff");
            circle(c, body.x, body.y, r);
            circle(c, head.x, head.y, hr);
            c.set_global_alpha(1.0);
        }
        if elite {
            let (name, icon, col) = z.affix.def();
            if z.shield > 0.0 {
                c.set_global_alpha(0.35);
                c.set_stroke_style_str("#9fdcff");
                c.set_line_width(3.0);
                c.begin_path();
                let _ = c.arc(d(body.x), d(body.y - r * 0.1), d(r * 1.35), 0.0, TAU64);
                c.stroke();
                c.set_global_alpha(0.12);
                c.set_fill_style_str("#6cc8ff");
                circle(c, body.x, body.y - r * 0.1, r * 1.35);
                c.set_global_alpha(1.0);
            }
            if z.kind != ZKind::Boss {
                c.set_text_align("center");
                c.set_text_baseline("middle");
                c.set_font("700 9px 'Inter','Segoe UI',system-ui,sans-serif");
                c.set_fill_style_str(col);
                c.set_stroke_style_str("rgba(0,0,0,0.85)");
                outlined(c, &format!("{} {}", name, def.name), p.x, p.y - r * 2.0 - 12.0, 2.5);
                c.set_font("12px 'Segoe UI Emoji','Apple Color Emoji','Noto Color Emoji',sans-serif");
                text(c, icon, p.x, p.y - r * 2.0 - 25.0);
            }
        }
        if z.hp < z.max_hp && (def.hp_bar || elite) {
            let bw = r * 2.0;
            let k = (z.hp / z.max_hp).max(0.0);
            c.set_fill_style_str("rgba(0,0,0,0.6)");
            c.fill_rect(d(p.x - bw * 0.5), d(p.y - r * 2.0), d(bw), 4.0);
            c.set_fill_style_str("#ff5050");
            c.fill_rect(d(p.x - bw * 0.5), d(p.y - r * 2.0), d(bw * k), 4.0);
        }
    }

    fn draw_obstacle_base(&self, c: &Ctx, o: &Obstacle) {
        if self.draw_map_prop(c, o) {
            return;
        }
        match o.kind {
            ObKind::Tree => {
                // trunk: a tapered quad from ground up to the canopy (perspective-lifted)
                let top = self.lift(o.pos, 70.0);
                let dir = (top - o.pos).norm();
                let side = if dir.len2() > 0.0 {
                    dir.perp()
                } else {
                    v2(1.0, 0.0)
                };
                let w0 = o.r * 0.42;
                let w1 = o.r * 0.28;
                c.set_fill_style_str("#3d2b1e");
                c.begin_path();
                c.move_to(d(o.pos.x + side.x * w0), d(o.pos.y + side.y * w0));
                c.line_to(d(top.x + side.x * w1), d(top.y + side.y * w1));
                c.line_to(d(top.x - side.x * w1), d(top.y - side.y * w1));
                c.line_to(d(o.pos.x - side.x * w0), d(o.pos.y - side.y * w0));
                c.close_path();
                c.fill();
                c.set_fill_style_str("#2c1f15");
                circle(c, o.pos.x, o.pos.y, w0);
            }
            ObKind::Rock => {
                let n = 7;
                let top = self.lift(o.pos, 18.0);
                let mut pts = Vec::with_capacity(n);
                for k in 0..n {
                    let an = k as f32 / n as f32 * TAU + (o.seed % 100) as f32 * 0.01;
                    let jr = 0.8 + ((o.seed >> (k * 3)) & 7) as f32 / 7.0 * 0.3;
                    pts.push(V2::from_angle(an) * (o.r * jr));
                }
                // sides: base polygon + top polygon hull approximated by drawing base then joins
                c.set_fill_style_str("#3a3c40");
                c.begin_path();
                for (k, q) in pts.iter().enumerate() {
                    let b = o.pos + *q;
                    if k == 0 {
                        c.move_to(d(b.x), d(b.y));
                    } else {
                        c.line_to(d(b.x), d(b.y));
                    }
                }
                c.close_path();
                c.fill();
                for k in 0..n {
                    let q0 = pts[k];
                    let q1 = pts[(k + 1) % n];
                    c.begin_path();
                    c.move_to(d(o.pos.x + q0.x), d(o.pos.y + q0.y));
                    c.line_to(d(o.pos.x + q1.x), d(o.pos.y + q1.y));
                    c.line_to(d(top.x + q1.x * 0.8), d(top.y + q1.y * 0.8));
                    c.line_to(d(top.x + q0.x * 0.8), d(top.y + q0.y * 0.8));
                    c.close_path();
                    c.fill();
                }
                c.set_fill_style_str("#6b6e74");
                c.begin_path();
                for (k, q) in pts.iter().enumerate() {
                    let b = top + *q * 0.8;
                    if k == 0 {
                        c.move_to(d(b.x), d(b.y));
                    } else {
                        c.line_to(d(b.x), d(b.y));
                    }
                }
                c.close_path();
                c.fill();
                c.set_fill_style_str("#858890");
                circle(c, top.x - o.r * 0.25, top.y - o.r * 0.25, o.r * 0.25);
            }
            ObKind::Grave => {
                // a standing slab extruded toward the viewer
                let w = 15.0;
                let base_l = o.pos + v2(-w, 0.0);
                let base_r = o.pos + v2(w, 0.0);
                let tl = self.lift(base_l, 34.0);
                let tr = self.lift(base_r, 34.0);
                c.set_fill_style_str("#2b2b30");
                c.begin_path();
                c.move_to(d(base_l.x), d(base_l.y + 5.0));
                c.line_to(d(base_r.x), d(base_r.y + 5.0));
                c.line_to(d(tr.x), d(tr.y + 5.0));
                c.line_to(d(tl.x), d(tl.y + 5.0));
                c.close_path();
                c.fill();
                c.set_fill_style_str("#7d7f88");
                c.begin_path();
                c.move_to(d(base_l.x), d(base_l.y));
                c.line_to(d(base_r.x), d(base_r.y));
                c.line_to(d(tr.x), d(tr.y));
                c.line_to(d(tl.x), d(tl.y));
                c.close_path();
                c.fill();
                let mid = tl.lerp(tr, 0.5);
                c.set_fill_style_str("#7d7f88");
                c.begin_path();
                let _ = c.arc(d(mid.x), d(mid.y), d(w), PI as f64, TAU64);
                c.fill();
                // engraved cross
                let cc = o.pos.lerp(mid, 0.6);
                c.set_stroke_style_str("#4a4b52");
                c.set_line_width(2.5);
                c.begin_path();
                c.move_to(d(cc.x), d(cc.y - 9.0));
                c.line_to(d(cc.x), d(cc.y + 6.0));
                c.move_to(d(cc.x - 5.0), d(cc.y - 4.0));
                c.line_to(d(cc.x + 5.0), d(cc.y - 4.0));
                c.stroke();
            }
            _ => {}
        }
    }

    fn draw_canopies(&self, c: &Ctx, obs: &[Obstacle], ppos: V2) {
        for o in obs.iter() {
            if o.kind != ObKind::Tree {
                continue;
            }
            let c1 = self.lift(o.pos, 70.0);
            let c2 = self.lift(o.pos, 92.0);
            let c3 = self.lift(o.pos, 112.0);
            let hue = (o.seed % 3) as usize;
            let (d1, d2, d3) = [
                ("#1d3a1f", "#2b5a2c", "#3f7a3a"),
                ("#223a1a", "#365a24", "#527a34"),
                ("#1a332a", "#27503d", "#3a7055"),
            ][hue];
            // fade canopy when the player (or nearby action) is beneath it
            let under = ppos.dist(c2) < o.r * 2.4;
            let al = if under { 0.4 } else { 1.0 };
            c.set_global_alpha(d(al));
            c.set_fill_style_str(d1);
            c.begin_path();
            path_circle(c, c1.x, c1.y, o.r * 2.1);
            for k in 0..5u32 {
                let an = k as f32 * 1.256 + (o.seed % 7) as f32;
                let q = c1 + V2::from_angle(an) * (o.r * 1.5);
                path_circle(c, q.x, q.y, o.r * 0.9);
            }
            c.fill();
            c.set_fill_style_str(d2);
            c.begin_path();
            path_circle(c, c2.x, c2.y, o.r * 1.65);
            for k in 0..4u32 {
                let an = k as f32 * 1.57 + (o.seed % 5) as f32;
                let q = c2 + V2::from_angle(an) * (o.r * 1.05);
                path_circle(c, q.x, q.y, o.r * 0.75);
            }
            c.fill();
            c.set_fill_style_str(d3);
            circle(c, c3.x - o.r * 0.35, c3.y - o.r * 0.35, o.r * 0.95);
        }
        c.set_global_alpha(1.0);
    }

    fn draw_blades(&self, c: &Ctx, ppos: V2, t: f32) {
        let nb = self.blade_count();
        if nb == 0 || matches!(self.phase, Phase::Menu | Phase::Dying | Phase::GameOver) {
            return;
        }
        let br = self.blade_radius();
        let base = self.player.blade_angle;
        let _ = c.set_global_composite_operation("lighter");
        // trail arc
        c.set_stroke_style_str("#7fe8ff");
        c.set_line_width(5.0);
        c.set_global_alpha(0.18);
        for k in 0..nb {
            let an = base + k as f32 * TAU / nb as f32;
            c.begin_path();
            let _ = c.arc(d(ppos.x), d(ppos.y), d(br), d(an - 0.7), d(an));
            c.stroke();
        }
        c.set_global_alpha(1.0);
        for k in 0..nb {
            let an = base + k as f32 * TAU / nb as f32;
            let bp = ppos + V2::from_angle(an) * br;
            c.save();
            let _ = c.translate(d(bp.x), d(bp.y));
            let _ = c.rotate(d(an + t * 14.0));
            c.set_fill_style_str("#bff6ff");
            c.begin_path();
            c.move_to(0.0, -14.0);
            c.line_to(4.0, 0.0);
            c.line_to(0.0, 14.0);
            c.line_to(-4.0, 0.0);
            c.close_path();
            c.fill();
            c.begin_path();
            c.move_to(-14.0, 0.0);
            c.line_to(0.0, 4.0);
            c.line_to(14.0, 0.0);
            c.line_to(0.0, -4.0);
            c.close_path();
            c.fill();
            c.restore();
        }
        let _ = c.set_global_composite_operation("source-over");
    }

    fn draw_bullets(&self, c: &Ctx, a: f32) {
        let _ = c.set_global_composite_operation("lighter");
        c.set_line_cap("round");
        let rainbow = [
            "#ff4a4a", "#ffb84a", "#fff04a", "#4aff7a", "#4ac8ff", "#c04aff",
        ];
        // one batch per tracer style in use (yours + teammates'), crits on top
        let mut styles: Vec<u8> = self
            .bullets
            .iter()
            .filter(|b| !b.crit)
            .map(|b| b.tracer)
            .collect();
        styles.sort_unstable();
        styles.dedup();
        let mut passes: Vec<(Option<u8>, &str, &str, f32)> = styles
            .into_iter()
            .map(|ti| {
                let tr = TRACERS[(ti as usize).min(TRACERS.len() - 1)];
                let (trail, core) = if tr.0 == "rainbow" {
                    let k = (self.real_time * 6.0) as usize % rainbow.len();
                    (rainbow[k], "#ffffff")
                } else {
                    (tr.1, tr.2)
                };
                (Some(ti), trail, core, 5.0f32)
            })
            .collect();
        passes.push((None, "#ff7a2a", "#fff2a0", 8.0));
        for (style, col, core, w) in passes {
            let crit = style.is_none();
            let pick =
                |b: &&crate::game::Bullet| b.crit == crit && (crit || Some(b.tracer) == style);
            c.set_stroke_style_str(col);
            c.set_line_width(d(w));
            c.set_global_alpha(0.55);
            c.begin_path();
            for b in self.bullets.iter().filter(pick) {
                let p = b.prev.lerp(b.pos, a);
                let tail = p - b.vel * 0.022;
                c.move_to(d(tail.x), d(tail.y));
                c.line_to(d(p.x), d(p.y));
            }
            c.stroke();
            c.set_global_alpha(1.0);
            c.set_fill_style_str(core);
            c.begin_path();
            for b in self.bullets.iter().filter(pick) {
                let p = b.prev.lerp(b.pos, a);
                path_circle(c, p.x, p.y, b.r * 0.75);
            }
            c.fill();
        }
        // enemy acid (rocks are drawn solid below)
        c.set_fill_style_str(self.access.color("#7dff3a"));
        c.set_global_alpha(0.35);
        c.begin_path();
        for s in self.eshots.iter().filter(|s| s.r < 10.0) {
            let p = s.prev.lerp(s.pos, a);
            path_circle(c, p.x, p.y, s.r * 2.0);
        }
        c.fill();
        let _ = c.set_global_composite_operation("source-over");
        c.set_global_alpha(1.0);
        c.set_fill_style_str(self.access.color("#c4ff6a"));
        c.begin_path();
        for s in self.eshots.iter().filter(|s| s.r < 10.0) {
            let p = s.prev.lerp(s.pos, a);
            path_circle(c, p.x, p.y, s.r);
        }
        c.fill();
        for s in self.eshots.iter().filter(|s| s.r >= 10.0) {
            let p = s.prev.lerp(s.pos, a);
            let top = self.lift(p, 18.0);
            c.set_fill_style_str("rgba(0,0,0,0.3)");
            circle(c, p.x + 4.0, p.y + 6.0, s.r);
            c.set_fill_style_str("#6b6e74");
            circle(c, top.x, top.y, s.r);
            c.set_fill_style_str("#8a8e96");
            circle(c, top.x - s.r * 0.3, top.y - s.r * 0.3, s.r * 0.4);
        }
        // danger: shots on a collision course with you get a red outline
        if self.player_alive() {
            let me = self.player.prev.lerp(self.player.pos, a);
            let pulse = 0.55 + 0.45 * (self.real_time as f32 * 14.0).sin().abs();
            c.set_stroke_style_str(self.access.color("#ff3040"));
            c.set_line_width(2.0);
            c.set_global_alpha(d(pulse));
            c.begin_path();
            for s in self.eshots.iter() {
                let p = s.prev.lerp(s.pos, a);
                let rel = me - p;
                let v2_ = s.vel.len2().max(1.0);
                let tc = (rel.dot(s.vel) / v2_).clamp(0.0, 0.7);
                if (p + s.vel * tc).dist(me) < PR + s.r + 6.0 {
                    path_circle_stroke(c, p.x, p.y, s.r + 5.0);
                }
            }
            c.stroke();
            c.set_global_alpha(1.0);
        }
    }

    fn draw_turrets(&self, c: &Ctx, t: f32) {
        for tu in self.turrets.iter() {
            let p = tu.pos;
            c.set_fill_style_str("rgba(0,0,0,0.3)");
            c.begin_path();
            let _ = c.ellipse(d(p.x + 3.0), d(p.y + 8.0), 16.0, 8.0, 0.0, 0.0, TAU64);
            c.fill();
            // tripod
            c.set_stroke_style_str("#3a4250");
            c.set_line_width(3.0);
            c.begin_path();
            for k in 0..3 {
                let a = k as f32 * TAU / 3.0 + 0.5;
                c.move_to(d(p.x), d(p.y - 6.0));
                c.line_to(d(p.x + a.cos() * 14.0), d(p.y + a.sin() * 8.0 + 4.0));
            }
            c.stroke();
            let head = p + v2(0.0, -10.0);
            c.set_fill_style_str("#5a6a80");
            circle(c, head.x, head.y, 9.0);
            let barrel = head + V2::from_angle(tu.aim) * 16.0;
            c.set_stroke_style_str("#2a303a");
            c.set_line_width(4.0);
            c.begin_path();
            c.move_to(d(head.x), d(head.y));
            c.line_to(d(barrel.x), d(barrel.y));
            c.stroke();
            let blink = if tu.life < 3.0 { (t * 10.0).sin() > 0.0 } else { true };
            if blink {
                let _ = c.set_global_composite_operation("lighter");
                c.set_fill_style_str("#7ad7ff");
                circle(c, head.x, head.y, 3.0);
                let _ = c.set_global_composite_operation("source-over");
            }
        }
    }

    /// Additive glow pools for transient lights (muzzle flashes, blasts).
    fn draw_lights(&self, c: &Ctx) {
        if self.lights.is_empty() {
            return;
        }
        let _ = c.set_global_composite_operation("lighter");
        for l in self.lights.iter() {
            let k = (l.life / l.max).clamp(0.0, 1.0);
            if let Ok(g) =
                c.create_radial_gradient(d(l.pos.x), d(l.pos.y), 0.0, d(l.pos.x), d(l.pos.y), d(l.r))
            {
                let _ = g.add_color_stop(0.0, l.color);
                let _ = g.add_color_stop(1.0, "rgba(0,0,0,0)");
                c.set_global_alpha(d(0.35 * k));
                c.set_fill_style_canvas_gradient(&g);
                c.fill_rect(d(l.pos.x - l.r), d(l.pos.y - l.r), d(l.r * 2.0), d(l.r * 2.0));
            }
        }
        c.set_global_alpha(1.0);
        let _ = c.set_global_composite_operation("source-over");
    }

    fn draw_particles(&self, c: &Ctx) {
        // normal (blood, gore, dust)
        for p in self.particles.iter().filter(|p| !p.glow) {
            let k = (p.life / p.max).clamp(0.0, 1.0);
            c.set_global_alpha(d(k.min(0.5) * 2.0));
            c.set_fill_style_str(p.color);
            let s = p.size;
            c.fill_rect(d(p.pos.x - s * 0.5), d(p.pos.y - p.z - s * 0.5), d(s), d(s));
        }
        let _ = c.set_global_composite_operation("lighter");
        for p in self.particles.iter().filter(|p| p.glow) {
            let k = (p.life / p.max).clamp(0.0, 1.0);
            c.set_global_alpha(d(k));
            c.set_fill_style_str(p.color);
            let s = p.size * (0.5 + k);
            c.fill_rect(d(p.pos.x - s * 0.5), d(p.pos.y - p.z - s * 0.5), d(s), d(s));
        }
        let _ = c.set_global_composite_operation("source-over");
        c.set_global_alpha(1.0);
    }

    fn draw_bolts(&self, c: &Ctx, t: f32) {
        if self.bolts.is_empty() {
            return;
        }
        let _ = c.set_global_composite_operation("lighter");
        c.set_line_join("round");
        for b in self.bolts.iter() {
            let k = (b.life / 0.22).clamp(0.0, 1.0);
            let mut pts = Vec::new();
            let seed = (t * 30.0) as i32;
            for w in b.pts.windows(2) {
                let (p0, p1) = (w[0], w[1]);
                let n = (p1 - p0).perp().norm();
                for s in 0..6 {
                    let f = s as f32 / 6.0;
                    let j = if s == 0 {
                        0.0
                    } else {
                        (hashf(seed, s + (p0.x as i32), 3) - 0.5) * 28.0
                    };
                    pts.push(p0.lerp(p1, f) + n * j);
                }
            }
            if let Some(l) = b.pts.last() {
                pts.push(*l);
            }
            for (col, wdt, al) in [
                ("#5a7dff", 9.0f32, 0.35f32),
                ("#cfe0ff", 3.5, 0.9),
                ("#ffffff", 1.5, 1.0),
            ] {
                c.set_stroke_style_str(col);
                c.set_line_width(d(wdt));
                c.set_global_alpha(d(al * k));
                c.begin_path();
                for (i, p) in pts.iter().enumerate() {
                    if i == 0 {
                        c.move_to(d(p.x), d(p.y));
                    } else {
                        c.line_to(d(p.x), d(p.y));
                    }
                }
                c.stroke();
            }
        }
        let _ = c.set_global_composite_operation("source-over");
        c.set_global_alpha(1.0);
    }

    fn draw_texts(&self, c: &Ctx) {
        c.set_text_align("center");
        c.set_text_baseline("middle");
        c.set_stroke_style_str("rgba(0,0,0,0.8)");
        c.set_line_join("round");
        let mut cur = 0.0f32;
        for tx in self.texts.iter() {
            let k = tx.life / tx.max;
            let pop = if k > 0.8 { 1.0 + (k - 0.8) * 2.5 } else { 1.0 };
            let size = (tx.size * pop / self.zoom.max(0.6)).round();
            if (size - cur).abs() > 0.5 {
                c.set_font(&format!(
                    "800 {}px 'Inter','Segoe UI',system-ui,sans-serif",
                    size
                ));
                cur = size;
            }
            c.set_global_alpha(d((k * 2.0).min(1.0)));
            c.set_fill_style_str(tx.color);
            outlined(c, &tx.s, tx.pos.x, tx.pos.y, 3.0);
        }
        c.set_global_alpha(1.0);
    }

    // ------------------------------------------------------------- lighting & screen fx
    fn draw_lighting(&self, c: &Ctx, w: f32, h: f32, ppos: V2, t: f32) {
        let cy = if self.phase == Phase::Menu && w < 760.0 { (h * 0.2 + 40.0).min(h * 0.5) } else { h * 0.5 };
        let sp = v2(w * 0.5, cy) + (ppos - self.cam) * self.zoom;
        let outer = (w.max(h)) * 0.75;
        let inner = 140.0 * self.zoom;
        let dark = if self.phase == Phase::Menu {
            0.55
        } else {
            ((0.42 + self.wave.n as f32 * 0.012).min(0.72) + self.sky_darkness()).clamp(0.2, 0.86)
        };
        if !self.draw_light_mask(c, w, h, ppos, dark, cy) {
            if let Ok(g) =
                c.create_radial_gradient(d(sp.x), d(sp.y), d(inner), d(sp.x), d(sp.y), d(outer))
            {
                let _ = g.add_color_stop(0.0, "rgba(4,6,18,0)");
                let _ = g.add_color_stop(0.55, &format!("rgba(4,6,18,{:.3})", dark * 0.6));
                let _ = g.add_color_stop(1.0, &format!("rgba(2,3,10,{:.3})", dark + 0.15));
                c.set_fill_style_canvas_gradient(&g);
                c.fill_rect(0.0, 0.0, d(w), d(h));
            }
        }
        // cool moonlight tint
        c.set_global_alpha(0.07);
        c.set_fill_style_str("#3050a0");
        c.fill_rect(0.0, 0.0, d(w), d(h));
        c.set_global_alpha(1.0);

        // damage / low-health vignette
        let low = if self.player.hp < self.player.max_hp * 0.3
            && matches!(self.phase, Phase::Playing | Phase::LevelUp)
        {
            0.25 + 0.2 * (t * 5.0).sin()
        } else {
            0.0
        };
        let hurt = (self.player.hurt_flash * 0.55).max(low) * self.access.flash;
        if hurt > 0.01 {
            if let Ok(g) = c.create_radial_gradient(
                d(w * 0.5),
                d(h * 0.5),
                d(h * 0.3),
                d(w * 0.5),
                d(h * 0.5),
                d(w.max(h) * 0.7),
            ) {
                let _ = g.add_color_stop(0.0, "rgba(180,0,10,0)");
                let _ = g.add_color_stop(1.0, &format!("rgba(180,0,10,{:.3})", hurt));
                c.set_fill_style_canvas_gradient(&g);
                c.fill_rect(0.0, 0.0, d(w), d(h));
            }
        }
        if self.ult_flash > 0.0 && self.access.flash > 0.0 {
            c.set_global_alpha(d(self.ult_flash * 0.6 * self.access.flash));
            c.set_fill_style_str("#fff6d8");
            c.fill_rect(0.0, 0.0, d(w), d(h));
            c.set_global_alpha(1.0);
        }
        if self.phase == Phase::Dying || self.phase == Phase::GameOver {
            let k = (self.dying_t / 1.8).min(1.0);
            c.set_global_alpha(d(k * 0.55));
            c.set_fill_style_str("#300000");
            c.fill_rect(0.0, 0.0, d(w), d(h));
            c.set_global_alpha(1.0);
        }
    }

    fn draw_offscreen(&self, c: &Ctx, w: f32, h: f32, a: f32, t: f32) {
        if !matches!(self.phase, Phase::Playing | Phase::LevelUp | Phase::Paused) {
            return;
        }
        let alive = self.zombies.len();
        let few = alive <= 6 && self.wave.spawned >= self.wave.total;
        let center = v2(w * 0.5, h * 0.5);
        let margin = 34.0;
        for z in self.zombies.iter() {
            if !(few || z.kind == ZKind::Boss) || z.dummy {
                continue;
            }
            let zp = z.prev.lerp(z.pos, self.za(a));
            let s = center + (zp - self.cam) * self.zoom;
            if s.x > 0.0 && s.x < w && s.y > 0.0 && s.y < h {
                continue;
            }
            let dir = (s - center).norm();
            let kx = if dir.x.abs() > 1e-4 {
                (w * 0.5 - margin) / dir.x.abs()
            } else {
                f32::MAX
            };
            let ky = if dir.y.abs() > 1e-4 {
                (h * 0.5 - margin) / dir.y.abs()
            } else {
                f32::MAX
            };
            let e = center + dir * kx.min(ky);
            let boss = z.kind == ZKind::Boss;
            let col = if boss { "#ff40ff" } else { "#ff5040" };
            let pulse = 1.0 + 0.15 * (t * 6.0).sin();
            c.save();
            let _ = c.translate(d(e.x), d(e.y));
            let _ = c.rotate(d(dir.angle()));
            let _ = c.scale(
                d(pulse * if boss { 1.4 } else { 1.0 }),
                d(pulse * if boss { 1.4 } else { 1.0 }),
            );
            c.set_fill_style_str(col);
            c.set_global_alpha(0.9);
            c.begin_path();
            c.move_to(12.0, 0.0);
            c.line_to(-8.0, -9.0);
            c.line_to(-4.0, 0.0);
            c.line_to(-8.0, 9.0);
            c.close_path();
            c.fill();
            c.restore();
        }
        c.set_global_alpha(1.0);
    }

    fn draw_hud(&self, c: &Ctx, w: f32, h: f32, t: f32) {
        let pl = &self.player;
        let small = w < 640.0;
        let font = "'Inter','Segoe UI',system-ui,sans-serif";
        c.set_stroke_style_str("rgba(0,0,0,0.75)");
        c.set_line_join("round");

        // XP bar (very top)
        let xk = (pl.xp / pl.xp_next).clamp(0.0, 1.0);
        c.set_fill_style_str("rgba(0,0,0,0.55)");
        c.fill_rect(0.0, 0.0, d(w), 8.0);
        {
            let g = c.create_linear_gradient(0.0, 0.0, d(w), 0.0);
            let _ = g.add_color_stop(0.0, "#38d6ff");
            let _ = g.add_color_stop(1.0, "#b46cff");
            c.set_fill_style_canvas_gradient(&g);
            c.fill_rect(0.0, 0.0, d(w * xk), 8.0);
        }
        c.set_global_alpha(0.5);
        c.set_fill_style_str("#ffffff");
        c.fill_rect(0.0, 0.0, d(w * xk), 2.0);
        c.set_global_alpha(1.0);

        // HP panel
        let px = 16.0;
        let py = 20.0;
        let bw = if small { 150.0 } else { 250.0 };
        // level badge
        c.set_fill_style_str("#1a1f2e");
        circle(c, px + 20.0, py + 20.0, 20.0);
        c.set_stroke_style_str("#7ad7ff");
        c.set_line_width(3.0);
        c.begin_path();
        let _ = c.arc(
            d(px + 20.0),
            d(py + 20.0),
            20.0,
            -PI as f64 / 2.0,
            -PI as f64 / 2.0 + TAU64 * xk as f64,
        );
        c.stroke();
        c.set_fill_style_str("#e8f6ff");
        c.set_text_align("center");
        c.set_text_baseline("middle");
        c.set_font(&format!("800 15px {}", font));
        text(c, &format!("{}", pl.level), px + 20.0, py + 21.0);
        c.set_font(&format!("700 8px {}", font));
        c.set_fill_style_str("#7ad7ff");
        text(c, "LV", px + 20.0, py + 8.0);

        let hx = px + 48.0;
        let hy = py + 8.0;
        round_rect(c, hx, hy, bw, 18.0, 5.0);
        c.set_fill_style_str("rgba(10,10,16,0.75)");
        c.fill();
        let gk = (pl.hp_ghost / pl.max_hp).clamp(0.0, 1.0);
        let hk = (pl.hp / pl.max_hp).clamp(0.0, 1.0);
        if gk > 0.0 {
            round_rect(c, hx + 2.0, hy + 2.0, (bw - 4.0) * gk, 14.0, 4.0);
            c.set_fill_style_str("#ffe0e0");
            c.fill();
        }
        if hk > 0.0 {
            round_rect(c, hx + 2.0, hy + 2.0, (bw - 4.0) * hk, 14.0, 4.0);
            {
                let g = c.create_linear_gradient(0.0, d(hy), 0.0, d(hy + 18.0));
                let _ = g.add_color_stop(0.0, "#ff6b7d");
                let _ = g.add_color_stop(1.0, "#c3122b");
                c.set_fill_style_canvas_gradient(&g);
            }
            c.fill();
        }
        c.set_fill_style_str("#ffffff");
        c.set_stroke_style_str("rgba(0,0,0,0.75)");
        c.set_font(&format!("700 11px {}", font));
        outlined(
            c,
            &format!("{} / {}", pl.hp.ceil() as i32, pl.max_hp as i32),
            hx + bw * 0.5,
            hy + 9.5,
            3.0,
        );
        // dash pips under HP
        let dash_k = 1.0 - (pl.dash_cd / self.dash_cooldown()).clamp(0.0, 1.0);
        round_rect(c, hx, hy + 22.0, bw * 0.5, 5.0, 2.5);
        c.set_fill_style_str("rgba(10,10,16,0.75)");
        c.fill();
        round_rect(c, hx, hy + 22.0, bw * 0.5 * dash_k, 5.0, 2.5);
        c.set_fill_style_str(if dash_k >= 1.0 { "#9fc0ff" } else { "#4a5a80" });
        c.fill();

        // wave info (top-right)
        c.set_text_align("right");
        c.set_text_baseline("alphabetic");
        c.set_fill_style_str("#ffffff");
        c.set_font(&format!("900 {}px {}", if small { 22 } else { 30 }, font));
        outlined(
            c,
            &self.mode.hooks().hud(self),
            w - 18.0,
            44.0,
            5.0,
        );
        c.set_font(&format!("600 13px {}", font));
        c.set_fill_style_str("#c8d2e8");
        let info = if self.hub.is_some() {
            format!("🪙 {} · E to talk · Esc to leave", self.coins)
        } else if self.wave.intermission > 0.0 && self.wave.n > 0 {
            format!("Next wave in {:.1}s", self.wave.intermission.max(0.0))
        } else {
            format!("{} undead remain", self.remaining())
        };
        outlined(c, &info, w - 18.0, 64.0, 3.0);
        let secs = self.time as u32;
        c.set_fill_style_str("#9aa4bb");
        outlined(
            c,
            &format!(
                "🪙 {}   ☠ {}   ⏱ {}:{:02}",
                self.coins,
                self.kills,
                secs / 60,
                secs % 60
            ),
            w - 18.0,
            82.0,
            3.0,
        );
        if self.best_wave > 0 {
            c.set_fill_style_str("#ffd36a");
            c.set_font(&format!("600 11px {}", font));
            outlined(
                c,
                &format!("Best: wave {}", self.best_wave),
                w - 18.0,
                98.0,
                3.0,
            );
        }
        if self.combo >= 5 {
            let pulse = 1.0 + 0.08 * (t * 12.0).sin();
            c.set_font(&format!("900 {}px {}", (22.0 * pulse).round(), font));
            let col = if self.combo >= 100 {
                "#ff4df0"
            } else if self.combo >= 50 {
                "#ff7a3a"
            } else {
                "#ffd34a"
            };
            c.set_fill_style_str(col);
            outlined(c, &format!("{}× STREAK", self.combo), w - 18.0, 126.0, 4.0);
        }
        // score + streak multiplier
        c.set_font(&format!("800 {}px {}", if small { 12 } else { 14 }, font));
        c.set_fill_style_str("#e8f0ff");
        let mult = self.score_mult();
        let score_line = if mult > 1.04 {
            format!("{} pts · x{:.1}", self.score, mult)
        } else {
            format!("{} pts", self.score)
        };
        outlined(c, &score_line, w - 18.0, if self.combo >= 5 { 146.0 } else { 118.0 }, 3.0);

        // multi-kill callout
        if let Some(co) = &self.callout {
            let k = co.t;
            let pop = if k < 0.12 { 0.6 + k / 0.12 * 0.55 } else { 1.15 - ((k - 0.12) * 0.4).min(0.15) };
            let alpha = if k > 1.0 { (1.4 - k) / 0.4 } else { 1.0 };
            c.set_global_alpha(d(alpha.clamp(0.0, 1.0)));
            c.set_text_align("center");
            c.set_font(&format!("italic 900 {}px {}", ((if small { 30.0 } else { 44.0 }) * pop).round(), font));
            c.set_fill_style_str(co.color);
            outlined(c, co.text, w * 0.5, h * 0.3, 7.0);
            c.set_global_alpha(1.0);
            c.set_text_align("right");
        }

        // kill feed (top-left, under the health bar)
        if !self.feed.is_empty() {
            c.set_text_align("left");
            c.set_font(&format!("700 {}px {}", if small { 11 } else { 13 }, font));
            for (i, f) in self.feed.iter().enumerate() {
                c.set_global_alpha(d(f.life.min(1.0)));
                c.set_fill_style_str(f.color);
                outlined(c, &f.text, 18.0, 84.0 + i as f32 * 18.0, 3.0);
            }
            c.set_global_alpha(1.0);
            c.set_text_align("right");
        }

        // boss bar
        if let Some(b) = self.zombies.iter().find(|z| z.kind == ZKind::Boss) {
            let bw2 = (w * 0.45).min(520.0);
            let bx = (w - bw2) * 0.5;
            let by = 40.0;
            c.set_text_align("center");
            c.set_font(&format!("800 13px {}", font));
            c.set_fill_style_str("#ff9cff");
            let bosses = self
                .zombies
                .iter()
                .filter(|z| z.kind == ZKind::Boss)
                .count();
            let label = if bosses > 1 {
                format!("{} ×{}", b.boss_var.name(), bosses)
            } else {
                b.boss_var.name().to_string()
            };
            outlined(c, &label, w * 0.5, by - 6.0, 3.0);
            round_rect(c, bx, by, bw2, 12.0, 4.0);
            c.set_fill_style_str("rgba(10,0,16,0.8)");
            c.fill();
            let k = (b.hp / b.max_hp).clamp(0.0, 1.0);
            if k > 0.0 {
                round_rect(c, bx + 2.0, by + 2.0, (bw2 - 4.0) * k, 8.0, 3.0);
                c.set_fill_style_str(if b.rage_t > 1000.0 { "#ff3a3a" } else { "#d03aff" });
                c.fill();
            }
            // phase thresholds
            c.set_fill_style_str("rgba(255,255,255,0.7)");
            for f in [0.33f32, 0.66] {
                c.fill_rect(d(bx + bw2 * f), d(by + 1.0), 2.0, 10.0);
            }
            if b.shield > 0.0 {
                c.set_stroke_style_str("#9fdcff");
                c.set_line_width(2.0);
                round_rect(c, bx - 2.0, by - 2.0, bw2 + 4.0, 16.0, 5.0);
                c.stroke();
            }
            c.set_font(&format!("700 11px {}", font));
            c.set_fill_style_str(if b.rage_t > 1000.0 { "#ff6a6a" } else { "#e0c0ff" });
            let phase_txt = if b.rage_t > 1000.0 {
                format!("PHASE {} · ENRAGED", b.phase.max(1))
            } else {
                let left = (crate::boss::ENRAGE_T - b.age).max(0.0);
                format!("PHASE {} · enrage in {}s · hit its back for crits", b.phase.max(1), left as i32)
            };
            outlined(c, &phase_txt, w * 0.5, by + 26.0, 3.0);
        }

        // abilities (bottom centre)
        let cy = h - 52.0;
        let cx = w * 0.5;
        // ultimate
        let uk = pl.ult.clamp(0.0, 1.0);
        let ready = uk >= 1.0;
        c.set_fill_style_str("rgba(10,12,20,0.75)");
        circle(c, cx, cy, 28.0);
        if ready {
            let _ = c.set_global_composite_operation("lighter");
            c.set_global_alpha(d(0.35 + 0.25 * (t * 6.0).sin()));
            c.set_fill_style_str("#ffcf4a");
            circle(c, cx, cy, 34.0);
            let _ = c.set_global_composite_operation("source-over");
            c.set_global_alpha(1.0);
        }
        c.set_stroke_style_str(if ready { "#ffe08a" } else { "#b08a3a" });
        c.set_line_width(5.0);
        c.begin_path();
        let _ = c.arc(
            d(cx),
            d(cy),
            26.0,
            -PI as f64 / 2.0,
            -PI as f64 / 2.0 + TAU64 * uk as f64,
        );
        c.stroke();
        c.set_text_align("center");
        c.set_text_baseline("middle");
        c.set_font("22px 'Segoe UI Emoji','Apple Color Emoji','Noto Color Emoji',sans-serif");
        c.set_global_alpha(if ready { 1.0 } else { 0.5 });
        text(c, "☢️", cx, cy);
        c.set_global_alpha(1.0);
        if !self.input.is_touch {
            c.set_font(&format!("800 10px {}", font));
            c.set_fill_style_str("#ffffff");
            c.set_stroke_style_str("rgba(0,0,0,0.8)");
            outlined(c, "R", cx, cy + 38.0, 3.0);
        }

        // class ability (right of the ultimate)
        let ch = &crate::content::CHARACTERS[self.character];
        let ax = cx + 64.0;
        let ay = cy + 6.0;
        let ak = 1.0 - (self.ability_cd / ch.cd).clamp(0.0, 1.0);
        let aready = self.ability_cd <= 0.0;
        c.set_fill_style_str("rgba(10,12,20,0.75)");
        circle(c, ax, ay, 21.0);
        if self.ability_t > 0.0 {
            let _ = c.set_global_composite_operation("lighter");
            c.set_global_alpha(d(0.4 + 0.3 * (t * 10.0).sin()));
            c.set_fill_style_str(ch.color);
            circle(c, ax, ay, 26.0);
            let _ = c.set_global_composite_operation("source-over");
            c.set_global_alpha(1.0);
        }
        c.set_stroke_style_str(if aready { ch.color } else { "#4a5a70" });
        c.set_line_width(4.0);
        c.begin_path();
        let _ = c.arc(d(ax), d(ay), 19.0, -PI as f64 / 2.0, -PI as f64 / 2.0 + TAU64 * ak as f64);
        c.stroke();
        c.set_font("17px 'Segoe UI Emoji','Apple Color Emoji','Noto Color Emoji',sans-serif");
        c.set_fill_style_str("#ffffff");
        c.set_global_alpha(if aready { 1.0 } else { 0.45 });
        text(c, ch.ability_icon, ax, ay);
        c.set_global_alpha(1.0);
        if !aready {
            c.set_font(&format!("800 11px {}", font));
            c.set_stroke_style_str("rgba(0,0,0,0.85)");
            outlined(c, &format!("{}", self.ability_cd.ceil() as i32), ax, ay + 1.0, 3.0);
        }
        if !self.input.is_touch {
            c.set_font(&format!("800 10px {}", font));
            c.set_fill_style_str("#ffffff");
            c.set_stroke_style_str("rgba(0,0,0,0.8)");
            outlined(c, "Q", ax, ay + 32.0, 3.0);
        }

        // upgrade tray (bottom-left)
        let mut ix = 16.0;
        let iy = h - 34.0;
        let mut owned = 0;
        for u in ALL.iter() {
            let l = self.ups.get(*u);
            if l == 0 {
                continue;
            }
            owned += 1;
            let def = u.def();
            let evo = self.evolved.iter().find(|e| e.def().base == *u).map(|e| e.def());
            round_rect(c, ix, iy - 16.0, 32.0, 32.0, 7.0);
            c.set_fill_style_str(if evo.is_some() { "rgba(60,40,10,0.85)" } else { "rgba(10,12,20,0.7)" });
            c.fill();
            c.set_stroke_style_str(if evo.is_some() { "#ffd36a" } else { def.color });
            c.set_line_width(if evo.is_some() { 2.5 } else { 1.5 });
            c.stroke();
            c.set_font("16px 'Segoe UI Emoji','Apple Color Emoji','Noto Color Emoji',sans-serif");
            c.set_fill_style_str("#ffffff");
            text(c, evo.map(|e| e.icon).unwrap_or(def.icon), ix + 16.0, iy - 2.0);
            // pips
            for k in 0..def.max {
                c.set_fill_style_str(if k < l {
                    def.color
                } else {
                    "rgba(255,255,255,0.15)"
                });
                let pw = 28.0 / def.max as f32;
                c.fill_rect(
                    d(ix + 2.0 + k as f32 * pw),
                    d(iy + 11.0),
                    d((pw - 1.0).max(1.0)),
                    3.0,
                );
            }
            ix += 36.0;
            if ix > w * 0.5 - 60.0 && owned % 2 == 0 && small {
                break;
            }
            if ix > w * 0.5 - 50.0 {
                break;
            }
        }

        // frenzy timer
        if pl.frenzy > 0.0 {
            c.set_font(&format!("800 12px {}", font));
            c.set_fill_style_str("#ffd84a");
            outlined(c, &format!("FRENZY {:.1}s", pl.frenzy), cx, cy - 46.0, 3.0);
        }

        // toast
        if let Some((s, life)) = &self.toast {
            c.set_global_alpha(d(life.min(0.3) / 0.3));
            c.set_font(&format!("700 15px {}", font));
            c.set_fill_style_str("#ffffff");
            outlined(c, s, cx, h - 112.0, 4.0);
            c.set_global_alpha(1.0);
        }

        // banner
        if let Some(b) = &self.banner {
            let k_in = ease_out_back((b.t / 0.35).min(1.0));
            let k_out = if b.t > b.dur - 0.5 {
                ((b.dur - b.t) / 0.5).max(0.0)
            } else {
                1.0
            };
            let sz = if small { 34.0 } else { 58.0 };
            c.save();
            // on touch screens keep banners clear of the minimap (top-right)
            let by = if self.input.is_touch { (h * 0.3).max(300.0).min(h * 0.45) } else { h * 0.3 };
            let _ = c.translate(d(w * 0.5), d(by));
            let _ = c.scale(d(0.6 + 0.4 * k_in), d(0.6 + 0.4 * k_in));
            c.set_global_alpha(d(k_out * (b.t / 0.15).min(1.0)));
            c.set_text_align("center");
            c.set_text_baseline("middle");
            c.set_font(&format!("900 {}px {}", sz, font));
            c.set_fill_style_str(b.color);
            c.set_stroke_style_str("rgba(0,0,0,0.85)");
            outlined(c, &b.title, 0.0, 0.0, 8.0);
            c.set_font(&format!("600 {}px {}", if small { 13 } else { 17 }, font));
            c.set_fill_style_str("#d8e0f0");
            outlined(c, &b.sub, 0.0, sz * 0.75, 4.0);
            c.restore();
            c.set_global_alpha(1.0);
        }

        // crosshair
        let manual = !self.auto_aim && !self.input.is_touch && self.input.mouse_seen;
        if manual && matches!(self.phase, Phase::Playing) {
            let m = self.input.mouse;
            let gap = 6.0 + pl.recoil * 5.0;
            c.set_stroke_style_str("rgba(0,0,0,0.7)");
            c.set_line_width(4.0);
            for pass in 0..2 {
                if pass == 1 {
                    c.set_stroke_style_str("#ffffff");
                    c.set_line_width(2.0);
                }
                c.begin_path();
                c.move_to(d(m.x - gap - 7.0), d(m.y));
                c.line_to(d(m.x - gap), d(m.y));
                c.move_to(d(m.x + gap), d(m.y));
                c.line_to(d(m.x + gap + 7.0), d(m.y));
                c.move_to(d(m.x), d(m.y - gap - 7.0));
                c.line_to(d(m.x), d(m.y - gap));
                c.move_to(d(m.x), d(m.y + gap));
                c.line_to(d(m.x), d(m.y + gap + 7.0));
                c.stroke();
            }
            c.set_fill_style_str("#ff5a5a");
            circle(c, m.x, m.y, 1.8);
        }
    }

    /// Circular radar: terrain, zombies, pickups and the visible area around the
    /// player, plus red rim arcs showing which directions hordes are coming from
    /// (including zombies beyond radar range).
    fn draw_minimap(&self, c: &Ctx, w: f32, h: f32, a: f32, t: f32) {
        if !self.show_minimap
            || !matches!(self.phase, Phase::Playing | Phase::LevelUp | Phase::Paused)
        {
            return;
        }
        let small = w < 640.0;
        let r = if small { 52.0 } else { 76.0 };
        // touch: top-right under the wave info (bottom-right holds the touch buttons)
        let (cx, cy) = if self.input.is_touch {
            (w - r - 16.0, 140.0 + r)
        } else {
            (w - r - 20.0, h - r - 30.0)
        };
        let centre = v2(cx, cy);
        let range = self.spawn_r * 1.5;
        let k = r / range;
        let cam = self.cam;
        let to_map = |p: V2| centre + (p - cam) * k;

        // backdrop
        c.set_global_alpha(1.0);
        c.set_fill_style_str("rgba(8,12,22,0.78)");
        circle(c, cx, cy, r + 3.0);

        c.save();
        c.begin_path();
        let _ = c.arc(d(cx), d(cy), d(r), 0.0, TAU64);
        c.clip();

        // range rings
        c.set_stroke_style_str("rgba(150,170,220,0.12)");
        c.set_line_width(1.0);
        c.begin_path();
        path_circle_stroke(c, cx, cy, r * 0.33);
        path_circle_stroke(c, cx, cy, r * 0.66);
        c.move_to(d(cx - r), d(cy));
        c.line_to(d(cx + r), d(cy));
        c.move_to(d(cx), d(cy - r));
        c.line_to(d(cx), d(cy + r));
        c.stroke();

        // terrain
        let ext = v2(range, range);
        let mut trees = Vec::new();
        let mut rocks = Vec::new();
        world::for_obstacles_in(cam - ext, cam + ext, |o| {
            if o.kind == ObKind::Tree {
                trees.push(*o)
            } else {
                rocks.push(*o)
            }
        });
        c.set_fill_style_str("#2f5a2c");
        c.begin_path();
        for o in trees.iter() {
            let m = to_map(o.pos);
            path_circle(c, m.x, m.y, (o.r * 2.0 * k).max(1.6));
        }
        c.fill();
        c.set_fill_style_str("#555a66");
        c.begin_path();
        for o in rocks.iter() {
            let m = to_map(o.pos);
            path_circle(c, m.x, m.y, (o.r * k).max(1.1));
        }
        c.fill();

        if let Some(m) = world::map() {
            self.draw_map_minimap(c, &m, &to_map, k);
        }

        // visible screen area
        let half = self.screen * (0.5 / self.zoom.max(0.01));
        let tl = to_map(cam - half);
        c.set_stroke_style_str("rgba(255,255,255,0.28)");
        c.set_line_width(1.0);
        c.stroke_rect(d(tl.x), d(tl.y), d(half.x * 2.0 * k), d(half.y * 2.0 * k));

        // starting point
        let home = to_map(V2::ZERO);
        if home.dist(centre) < r - 4.0 {
            c.set_fill_style_str("#7ad7ff");
            c.begin_path();
            c.move_to(d(home.x), d(home.y - 4.0));
            c.line_to(d(home.x + 4.0), d(home.y));
            c.line_to(d(home.x), d(home.y + 4.0));
            c.line_to(d(home.x - 4.0), d(home.y));
            c.close_path();
            c.fill();
        }

        // pickups
        for p in self.pickups.iter() {
            let col = match p.kind {
                PickKind::Heart => "#ff5c7a",
                PickKind::Vacuum => "#b48cff",
                PickKind::Frenzy => "#ffd84a",
                PickKind::Chest => "#ffb830",
                _ => "#7affb0",
            };
            c.set_fill_style_str(col);
            let m = to_map(p.pos);
            circle(c, m.x, m.y, 2.6);
        }

        // zombies, batched by colour
        let groups: [(&str, &[ZKind], f32); 6] = [
            ("#ff4a3a", &[ZKind::Walker, ZKind::Runner, ZKind::Crawler], 1.7),
            ("#ff9a3a", &[ZKind::Exploder], 2.0),
            ("#9be04a", &[ZKind::Spitter], 2.0),
            ("#ff6a5a", &[ZKind::Brute, ZKind::Armored], 2.8),
            ("#ff8ac0", &[ZKind::Screamer], 2.2),
            ("#b48cff", &[ZKind::Bat], 1.3),
        ];
        for (col, kinds, size) in groups {
            c.set_fill_style_str(col);
            c.begin_path();
            for z in self.zombies.iter() {
                if kinds.contains(&z.kind) && !z.hidden {
                    let m = to_map(z.prev.lerp(z.pos, self.za(a)));
                    path_circle(c, m.x, m.y, size);
                }
            }
            c.fill();
        }
        c.restore();

        // threat arcs: which directions zombies are coming from (off-screen ones)
        const SECTORS: usize = 24;
        let mut threat = [0f32; SECTORS];
        let mut boss_dir = None;
        for z in self.zombies.iter().filter(|z| !z.dummy) {
            let d0 = z.pos - cam;
            let off_screen = d0.x.abs() > half.x || d0.y.abs() > half.y;
            if z.kind == ZKind::Boss {
                boss_dir = Some(d0);
            }
            if !off_screen {
                continue;
            }
            let ang = d0.angle().rem_euclid(TAU);
            let s = ((ang / TAU) * SECTORS as f32) as usize % SECTORS;
            let wgt = match z.kind {
                ZKind::Boss => 8.0,
                ZKind::Brute => 2.0,
                _ => 1.0,
            };
            threat[s] += wgt;
        }
        c.set_stroke_style_str("#ff3b3b");
        c.set_line_cap("butt");
        let seg = TAU / SECTORS as f32;
        for (i, v) in threat.iter().enumerate() {
            if *v <= 0.0 {
                continue;
            }
            let intensity = (v / 10.0).min(1.0);
            c.set_global_alpha(d(0.3 + 0.6 * intensity));
            c.set_line_width(d(2.5 + 3.5 * intensity));
            c.begin_path();
            let a0 = i as f32 * seg + 0.03;
            let _ = c.arc(d(cx), d(cy), d(r + 1.5), d(a0), d(a0 + seg - 0.06));
            c.stroke();
        }
        c.set_global_alpha(1.0);

        // rim
        c.set_stroke_style_str("rgba(160,180,255,0.4)");
        c.set_line_width(1.5);
        c.begin_path();
        let _ = c.arc(d(cx), d(cy), d(r + 3.0), 0.0, TAU64);
        c.stroke();

        // boss: pulsing marker, pinned to the rim when out of range
        if let Some(bd) = boss_dir {
            let mut m = bd * k;
            if m.len() > r - 5.0 {
                m = m.norm() * (r - 5.0);
            }
            let p = centre + m;
            let pulse = 4.0 + 1.5 * (t * 6.0).sin();
            c.set_fill_style_str("#ff40ff");
            circle(c, p.x, p.y, pulse);
            c.set_stroke_style_str("#ffffff");
            c.set_line_width(1.0);
            c.begin_path();
            let _ = c.arc(d(p.x), d(p.y), d(pulse + 1.5), 0.0, TAU64);
            c.stroke();
        }

        // teammates
        if let Some(n) = &self.net {
            for tm in n.remotes.iter().filter(|tm| tm.present && !tm.state.dead()) {
                let mut m = (tm.render_pos(self.real_time) - cam) * k;
                if m.len() > r - 5.0 {
                    m = m.norm() * (r - 5.0);
                }
                let p = centre + m;
                c.set_fill_style_str(if tm.state.downed() {
                    "#ff7a7a"
                } else {
                    "#4ade80"
                });
                circle(c, p.x, p.y, 4.0);
                c.set_stroke_style_str("#ffffff");
                c.set_line_width(1.0);
                c.begin_path();
                let _ = c.arc(d(p.x), d(p.y), 4.5, 0.0, TAU64);
                c.stroke();
            }
        }

        // player arrow (points along aim)
        c.save();
        let _ = c.translate(d(cx), d(cy));
        let _ = c.rotate(d(self.player.aim));
        c.set_fill_style_str("#ffffff");
        c.set_stroke_style_str("#1e4fa8");
        c.set_line_width(1.5);
        c.begin_path();
        c.move_to(7.0, 0.0);
        c.line_to(-5.0, -5.0);
        c.line_to(-2.5, 0.0);
        c.line_to(-5.0, 5.0);
        c.close_path();
        c.stroke();
        c.fill();
        c.restore();

        // position readout (1 m = 32 world units)
        let font = "'Inter','Segoe UI',system-ui,sans-serif";
        c.set_font(&format!("700 10px {}", font));
        c.set_text_align("center");
        c.set_text_baseline("middle");
        c.set_fill_style_str("#c8d2e8");
        c.set_stroke_style_str("rgba(0,0,0,0.8)");
        let ty = if self.input.is_touch {
            cy + r + 14.0
        } else {
            cy - r - 12.0
        };
        let dist = cam.len() / 32.0;
        let label = if self.input.is_touch {
            format!(
                "{:.0}, {:.0} · {:.0}m from start",
                cam.x / 32.0,
                cam.y / 32.0,
                dist
            )
        } else {
            format!(
                "{:.0}, {:.0} · {:.0}m from start · [N]",
                cam.x / 32.0,
                cam.y / 32.0,
                dist
            )
        };
        outlined(c, &label, cx, ty, 3.0);
    }

    /// Names + health bars above teammates (world space).
    fn draw_teammate_tags(&self, c: &Ctx, t: f32) {
        let Some(n) = &self.net else { return };
        let font = "'Inter','Segoe UI',system-ui,sans-serif";
        let s = 1.0 / self.zoom.max(0.5);
        c.set_text_align("center");
        c.set_text_baseline("middle");
        c.set_line_join("round");
        for r in n.remotes.iter().filter(|r| r.present && !r.state.dead()) {
            let p = r.render_pos(self.real_time) + v2(0.0, -42.0);
            c.set_font(&format!("800 {}px {}", (13.0 * s).round(), font));
            c.set_stroke_style_str("rgba(0,0,0,0.85)");
            c.set_fill_style_str(if r.state.downed() {
                "#ff7a7a"
            } else {
                "#9cffb8"
            });
            outlined(c, &r.name, p.x, p.y, 3.0 * s);
            let bw = 44.0 * s;
            let k = (r.state.hp / r.state.max_hp).clamp(0.0, 1.0);
            c.set_fill_style_str("rgba(0,0,0,0.6)");
            c.fill_rect(d(p.x - bw * 0.5), d(p.y + 9.0 * s), d(bw), d(5.0 * s));
            c.set_fill_style_str("#4ade80");
            c.fill_rect(d(p.x - bw * 0.5), d(p.y + 9.0 * s), d(bw * k), d(5.0 * s));
            if r.state.downed() {
                let pulse = 0.6 + 0.4 * (t * 6.0).sin();
                c.set_global_alpha(d(pulse));
                c.set_font(&format!("900 {}px {}", (12.0 * s).round(), font));
                c.set_fill_style_str("#ffdddd");
                outlined(c, "DOWN · REVIVE ME", p.x, p.y - 16.0 * s, 3.0 * s);
                c.set_global_alpha(1.0);
            }
        }
    }

    /// Screen-space co-op HUD: teammate list, downed/spectating state, arrows.
    fn draw_coop_hud(&self, c: &Ctx, w: f32, h: f32, t: f32) {
        let Some(n) = &self.net else { return };
        let font = "'Inter','Segoe UI',system-ui,sans-serif";
        // teammate cards under the HP bar
        let mut y = 92.0;
        c.set_text_baseline("middle");
        for r in n.remotes.iter().filter(|r| r.present) {
            let s = &r.state;
            round_rect(c, 16.0, y, 180.0, 30.0, 8.0);
            c.set_fill_style_str("rgba(10,12,20,0.7)");
            c.fill();
            c.set_text_align("left");
            c.set_font(&format!("800 12px {}", font));
            let (status, col) = if s.dead() {
                ("☠", "#8a8a8a")
            } else if s.downed() {
                ("⬇", "#ff7a7a")
            } else {
                ("", "#e8eefc")
            };
            c.set_fill_style_str(col);
            text(
                c,
                &format!("{} {}", status, r.name).trim().to_string(),
                26.0,
                y + 10.0,
            );
            c.set_text_align("right");
            c.set_fill_style_str("#94a0bd");
            text(c, &format!("Lv {}", s.level), 188.0, y + 10.0);
            let k = if s.dead() {
                0.0
            } else {
                (s.hp / s.max_hp).clamp(0.0, 1.0)
            };
            c.set_fill_style_str("rgba(255,255,255,0.1)");
            c.fill_rect(26.0, d(y + 20.0), 160.0, 4.0);
            c.set_fill_style_str(if s.downed() { "#ff7a7a" } else { "#4ade80" });
            c.fill_rect(
                26.0,
                d(y + 20.0),
                d(160.0 * if s.downed() { s.revive } else { k }),
                4.0,
            );
            y += 36.0;
        }
        // arrows to teammates who are off-screen
        let center = v2(w * 0.5, h * 0.5);
        for r in n.remotes.iter().filter(|r| r.present && !r.state.dead()) {
            let sp = center + (r.render_pos(self.real_time) - self.cam) * self.zoom;
            if sp.x > 0.0 && sp.x < w && sp.y > 0.0 && sp.y < h {
                continue;
            }
            let dir = (sp - center).norm();
            let kx = if dir.x.abs() > 1e-4 {
                (w * 0.5 - 40.0) / dir.x.abs()
            } else {
                f32::MAX
            };
            let ky = if dir.y.abs() > 1e-4 {
                (h * 0.5 - 40.0) / dir.y.abs()
            } else {
                f32::MAX
            };
            let e = center + dir * kx.min(ky);
            c.save();
            let _ = c.translate(d(e.x), d(e.y));
            let _ = c.rotate(d(dir.angle()));
            c.set_fill_style_str(if r.state.downed() {
                "#ff7a7a"
            } else {
                "#4ade80"
            });
            c.begin_path();
            c.move_to(14.0, 0.0);
            c.line_to(-8.0, -10.0);
            c.line_to(-8.0, 10.0);
            c.close_path();
            c.fill();
            c.restore();
            c.set_text_align("center");
            c.set_font(&format!("800 11px {}", font));
            c.set_fill_style_str("#ffffff");
            c.set_stroke_style_str("rgba(0,0,0,0.8)");
            outlined(c, &r.name, e.x - dir.x * 22.0, e.y - dir.y * 22.0, 3.0);
        }
        // our own state
        let p = &self.player;
        c.set_text_align("center");
        if p.downed {
            let pulse = 0.7 + 0.3 * (t * 4.0).sin();
            c.set_global_alpha(d(pulse));
            c.set_font(&format!("900 26px {}", font));
            c.set_fill_style_str("#ff6a6a");
            c.set_stroke_style_str("rgba(0,0,0,0.85)");
            outlined(
                c,
                &format!("DOWN · bleeding out in {:.0}s", p.bleed.max(0.0)),
                w * 0.5,
                h * 0.72,
                6.0,
            );
            c.set_global_alpha(1.0);
            c.set_font(&format!("700 14px {}", font));
            c.set_fill_style_str("#ffd0d0");
            outlined(
                c,
                "Crawl toward a teammate: they revive you by standing close",
                w * 0.5,
                h * 0.72 + 28.0,
                4.0,
            );
        } else if p.dead {
            c.set_font(&format!("900 22px {}", font));
            c.set_fill_style_str("#c8d2e8");
            c.set_stroke_style_str("rgba(0,0,0,0.85)");
            outlined(
                c,
                "SPECTATING · you'll be back next run",
                w * 0.5,
                h - 140.0,
                5.0,
            );
        }
    }

    /// Cinematic boss card: letterbox bars, the name slams in, then the taunt types out.
    fn draw_boss_intro(&self, c: &Ctx, w: f32, h: f32, var: crate::content::BossVar, it: f32) {
        let font = "'Inter','Segoe UI',system-ui,sans-serif";
        let total = crate::boss::INTRO_T;
        let inn = ease_out((it / 0.35).min(1.0));
        let out = ease_out(((total - it) / 0.4).clamp(0.0, 1.0));
        let k = inn.min(out);
        let bar = (h * 0.11).min(80.0) * k;
        c.set_fill_style_str("#000000");
        c.fill_rect(0.0, 0.0, d(w), d(bar));
        c.fill_rect(0.0, d(h - bar), d(w), d(bar));
        let small = w < 640.0;
        let y = h * 0.34;
        c.set_global_alpha(d(k));
        c.set_text_align("center");
        c.set_text_baseline("middle");
        c.set_stroke_style_str("rgba(0,0,0,0.9)");
        c.set_line_join("round");
        let slam = 1.0 + 0.6 * (1.0 - ((it - 0.15) / 0.2).clamp(0.0, 1.0));
        c.set_font(&format!("900 {}px {}", ((if small { 30.0 } else { 54.0 }) * slam).round(), font));
        c.set_fill_style_str("#ff9cff");
        outlined(c, var.name(), w * 0.5, y, 8.0);
        let taunt = var.taunt();
        let shown = ((it - 0.5).max(0.0) * 40.0) as usize;
        let line: String = taunt.chars().take(shown).collect();
        c.set_font(&format!("italic 600 {}px {}", if small { 14 } else { 19 }, font));
        c.set_fill_style_str("#f0e0ff");
        outlined(c, &format!("\u{201c}{}\u{201d}", line), w * 0.5, y + if small { 34.0 } else { 50.0 }, 4.0);
        c.set_global_alpha(1.0);
    }

    /// Slot-machine chest reveal: one reel per pick, stopping left to right.
    fn draw_chest(&self, c: &Ctx, w: f32, h: f32, t: f32) {
        let Some(ch) = &self.chest else { return };
        let font = "'Inter','Segoe UI',system-ui,sans-serif";
        let emoji = "'Segoe UI Emoji','Apple Color Emoji','Noto Color Emoji',sans-serif";
        let k = ease_out((ch.t / 0.3).min(1.0));
        c.set_global_alpha(d(0.75 * k));
        c.set_fill_style_str("#05060c");
        c.fill_rect(0.0, 0.0, d(w), d(h));
        c.set_global_alpha(d(k));
        // golden god-rays
        let _ = c.set_global_composite_operation("lighter");
        c.set_fill_style_str("rgba(255,200,80,0.06)");
        for i in 0..12 {
            let a = t * 0.3 + i as f32 * TAU / 12.0;
            c.begin_path();
            c.move_to(d(w * 0.5), d(h * 0.45));
            let r = w.max(h);
            c.line_to(d(w * 0.5 + a.cos() * r), d(h * 0.45 + a.sin() * r));
            c.line_to(d(w * 0.5 + (a + 0.12).cos() * r), d(h * 0.45 + (a + 0.12).sin() * r));
            c.fill();
        }
        let _ = c.set_global_composite_operation("source-over");
        c.set_text_align("center");
        c.set_text_baseline("middle");
        c.set_stroke_style_str("rgba(0,0,0,0.85)");
        c.set_line_join("round");
        let small = w < 640.0;
        let n = ch.picks.len();
        c.set_font(&format!("900 {}px {}", if small { 30 } else { 46 }, font));
        c.set_fill_style_str("#ffd36a");
        let title = match n {
            5 => "LEGENDARY CHEST!",
            3 => "TREASURE CHEST!",
            _ => "CHEST",
        };
        outlined(c, title, w * 0.5, h * 0.2, 7.0);
        let size = if small { (w - 40.0) / n.max(1) as f32 - 8.0 } else { 104.0f32 }.min(104.0);
        let gap = 10.0;
        let total = n as f32 * size + (n.saturating_sub(1)) as f32 * gap;
        let x0 = w * 0.5 - total * 0.5;
        let y = h * 0.45 - size * 0.5;
        for (i, u) in ch.picks.iter().enumerate() {
            let x = x0 + i as f32 * (size + gap);
            let stop = 0.6 + crate::chest::REEL_T * (i + 1) as f32;
            let done = ch.t >= stop;
            round_rect(c, x, y, size, size, 14.0);
            c.set_fill_style_str(if done { "#2a1f10" } else { "#14161e" });
            c.fill();
            let evo = matches!(u, crate::chest::ChestPick::Evo(_));
            c.set_stroke_style_str(if done { u.color() } else { "#5a4a2a" });
            c.set_line_width(if done { 3.0 } else { 2.0 });
            c.stroke();
            c.save();
            round_rect(c, x, y, size, size, 14.0);
            c.clip();
            c.set_font(&format!("{}px {}", (size * 0.5).round(), emoji));
            c.set_fill_style_str("#ffffff");
            if done {
                let pop = 1.0 + 0.25 * (1.0 - ((ch.t - stop) / 0.2).min(1.0));
                c.set_font(&format!("{}px {}", (size * 0.5 * pop).round(), emoji));
                text(c, u.icon(), x + size * 0.5, y + size * 0.5);
            } else {
                // spinning strip of icons
                let spin = ch.t * 14.0 + i as f32 * 1.7;
                for j in 0..3 {
                    let f = spin.fract();
                    let idx = (spin.floor() as usize + j) % ALL.len();
                    let yy = y + size * (0.5 + (j as f32 - 1.0 - f + 1.0) * 0.8) - size * 0.3;
                    text(c, ALL[idx].def().icon, x + size * 0.5, yy);
                }
            }
            c.restore();
            if done {
                c.set_font(&format!("800 {}px {}", if small { 10 } else { 12 }, font));
                c.set_fill_style_str("#ffffff");
                if evo {
                    c.set_fill_style_str("#ffd36a");
                    text(c, "EVOLVED!", x + size * 0.5, y - 12.0);
                    c.set_fill_style_str("#ffffff");
                }
                let lines = wrap(c, u.name(), size + 6.0);
                for (li, l) in lines.iter().take(2).enumerate() {
                    text(c, l, x + size * 0.5, y + size + 14.0 + li as f32 * 14.0);
                }
            }
        }
        if ch.revealed() {
            c.set_font(&format!("800 {}px {}", if small { 15 } else { 18 }, font));
            c.set_fill_style_str("#ffd36a");
            outlined(c, &format!("+{} coins", ch.coins), w * 0.5, y + size + 56.0, 4.0);
            c.set_font(&format!("600 14px {}", font));
            c.set_fill_style_str("#c8d2e8");
            let hint = if self.input.is_touch { "Tap to continue" } else { "Click or press Enter to continue" };
            c.set_global_alpha(d(0.6 + 0.4 * (t * 4.0).sin().abs()));
            outlined(c, hint, w * 0.5, y + size + 84.0, 3.0);
        }
        c.set_global_alpha(1.0);
    }

    fn draw_levelup(&self, c: &Ctx, w: f32, h: f32, t: f32) {
        let font = "'Inter','Segoe UI',system-ui,sans-serif";
        let k = ease_out((self.levelup_t / 0.25).min(1.0));
        c.set_global_alpha(d(0.68 * k));
        c.set_fill_style_str("#05060c");
        c.fill_rect(0.0, 0.0, d(w), d(h));
        c.set_global_alpha(1.0);

        let rects = self.card_rects();
        c.set_text_align("center");
        c.set_text_baseline("middle");
        c.set_stroke_style_str("rgba(0,0,0,0.85)");
        c.set_line_join("round");
        let ty = rects[0].1 - if self.pending_levels > 1 { 70.0 } else { 54.0 };
        let small = w < 640.0;
        c.set_font(&format!("900 {}px {}", if small { 28 } else { 44 }, font));
        c.set_fill_style_str("#ffe27a");
        c.set_global_alpha(d(k));
        outlined(c, "LEVEL UP!", w * 0.5, ty, 7.0);
        c.set_font(&format!("600 14px {}", font));
        c.set_fill_style_str("#c8d2e8");
        let hint = if self.input.is_touch {
            "Tap a power"
        } else {
            "Choose a power — 1 · 2 · 3, arrows + Enter, or click"
        };
        outlined(c, hint, w * 0.5, ty + 32.0, 3.0);
        if self.pending_levels > 1 {
            c.set_fill_style_str("#7ad7ff");
            outlined(
                c,
                &format!("+{} more level-ups queued", self.pending_levels - 1),
                w * 0.5,
                ty + 52.0,
                3.0,
            );
        }
        c.set_global_alpha(1.0);

        for (i, u) in self.offers.iter().enumerate() {
            let (x, y, cw, ch) = rects[i];
            let delay = i as f32 * 0.06;
            let ck = ease_out_back(((self.levelup_t - delay) / 0.3).clamp(0.0, 1.0));
            let sel = i == self.sel;
            let lift = if sel {
                -10.0 - (t * 4.0).sin() * 2.0
            } else {
                0.0
            };
            let yy = y + (1.0 - ck) * 120.0 + lift;
            let def = u.def();
            let lvl = self.ups.get(*u);
            c.set_global_alpha(d(ck.clamp(0.0, 1.0)));

            // glow
            if sel {
                let _ = c.set_global_composite_operation("lighter");
                c.set_global_alpha(d(0.25 * ck.clamp(0.0, 1.0)));
                round_rect(c, x - 8.0, yy - 8.0, cw + 16.0, ch + 16.0, 18.0);
                c.set_fill_style_str(def.color);
                c.fill();
                let _ = c.set_global_composite_operation("source-over");
                c.set_global_alpha(d(ck.clamp(0.0, 1.0)));
            }
            round_rect(c, x, yy, cw, ch, 14.0);
            {
                let g = c.create_linear_gradient(0.0, d(yy), 0.0, d(yy + ch));
                let (a1, a2) = match def.rarity {
                    2 => ("#2a1640", "#120a1e"),
                    1 => ("#132a40", "#0a1420"),
                    _ => ("#1c2230", "#0e1118"),
                };
                let _ = g.add_color_stop(0.0, a1);
                let _ = g.add_color_stop(1.0, a2);
                c.set_fill_style_canvas_gradient(&g);
            }
            c.fill();
            let banish = self.banish_mode;
            c.set_stroke_style_str(if banish {
                if sel { "#ff8a8a" } else { "#ff4a5a" }
            } else if sel {
                "#ffffff"
            } else {
                def.color
            });
            c.set_line_width(if sel { 3.0 } else { 2.0 });
            c.stroke();
            if banish {
                c.set_global_alpha(d(0.18 * ck.clamp(0.0, 1.0)));
                c.set_fill_style_str("#ff2030");
                round_rect(c, x, yy, cw, ch, 14.0);
                c.fill();
                c.set_global_alpha(d(ck.clamp(0.0, 1.0)));
            }

            let (rl, rc) = match def.rarity {
                2 => ("SUPERPOWER", "#d58cff"),
                1 => ("RARE", "#6cc8ff"),
                _ => ("COMMON", "#a0a8b8"),
            };
            if cw > ch * 1.6 {
                // compact horizontal card (phones): icon left, text right
                let bob = if sel { (t * 5.0).sin() * 2.0 } else { 0.0 };
                c.set_text_align("center");
                c.set_font(&format!(
                    "{}px 'Segoe UI Emoji','Apple Color Emoji','Noto Color Emoji',sans-serif",
                    (ch * 0.36).min(44.0).round()
                ));
                // solid fill: Safari draws text under a gradient fillStyle as a
                // flat silhouette, which hid the emoji against the dark card
                c.set_fill_style_str("#ffffff");
                text(c, def.icon, x + ch * 0.42, yy + ch * 0.5 + bob);
                let tx = x + ch * 0.82;
                c.set_text_align("left");
                c.set_font(&format!("800 10px {}", font));
                c.set_fill_style_str(rc);
                text(c, rl, tx, yy + 16.0);
                c.set_text_align("right");
                c.set_font(&format!("800 12px {}", font));
                if lvl == 0 {
                    c.set_fill_style_str("#7dffa0");
                    text(c, "NEW!", x + cw - 14.0, yy + 16.0);
                } else {
                    c.set_fill_style_str("#ffd36a");
                    text(c, &format!("Lv {} → {}", lvl, lvl + 1), x + cw - 14.0, yy + 16.0);
                }
                c.set_text_align("left");
                c.set_font(&format!("800 17px {}", font));
                c.set_fill_style_str("#ffffff");
                text(c, def.name, tx, yy + 38.0);
                c.set_font(&format!("500 13px {}", font));
                c.set_fill_style_str("#c8d2e8");
                let lines = wrap(c, &u.desc(lvl + 1), cw - (tx - x) - 14.0);
                for (li, l) in lines.iter().take(((ch - 58.0) / 16.0).max(1.0) as usize).enumerate() {
                    text(c, l, tx, yy + 60.0 + li as f32 * 16.0);
                }
                c.set_text_align("center");
                continue;
            }
            c.set_font(&format!("800 10px {}", font));
            c.set_fill_style_str(rc);
            text(c, rl, x + cw * 0.5, yy + 18.0);

            let icon_s = (cw * 0.26).min(54.0);
            c.set_font(&format!(
                "{}px 'Segoe UI Emoji','Apple Color Emoji','Noto Color Emoji',sans-serif",
                icon_s.round()
            ));
            let bob = if sel { (t * 5.0).sin() * 3.0 } else { 0.0 };
            text(c, def.icon, x + cw * 0.5, yy + ch * 0.27 + bob);

            c.set_font(&format!(
                "800 {}px {}",
                if cw < 190.0 { 14 } else { 17 },
                font
            ));
            c.set_fill_style_str("#ffffff");
            text(c, def.name, x + cw * 0.5, yy + ch * 0.47);

            c.set_font(&format!("700 12px {}", font));
            if lvl == 0 {
                c.set_fill_style_str("#7dffa0");
                text(c, "NEW!", x + cw * 0.5, yy + ch * 0.56);
            } else {
                c.set_fill_style_str("#ffd36a");
                text(
                    c,
                    &format!("Lv {} → {}", lvl, lvl + 1),
                    x + cw * 0.5,
                    yy + ch * 0.56,
                );
            }

            c.set_font(&format!(
                "500 {}px {}",
                if cw < 190.0 { 12 } else { 13 },
                font
            ));
            c.set_fill_style_str("#c8d2e8");
            let lines = wrap(c, &u.desc(lvl + 1), cw - 28.0);
            for (li, l) in lines.iter().enumerate() {
                text(c, l, x + cw * 0.5, yy + ch * 0.67 + li as f32 * 17.0);
            }

            if !self.input.is_touch {
                round_rect(c, x + cw * 0.5 - 13.0, yy + ch - 34.0, 26.0, 22.0, 6.0);
                c.set_fill_style_str(if sel {
                    "#ffffff"
                } else {
                    "rgba(255,255,255,0.12)"
                });
                c.fill();
                c.set_font(&format!("800 12px {}", font));
                c.set_fill_style_str(if sel { "#111" } else { "#ddd" });
                text(c, &format!("{}", i + 1), x + cw * 0.5, yy + ch - 23.0);
            }
            if self.banish_mode {
                c.set_font(&format!("900 13px {}", font));
                c.set_fill_style_str("#ff6a7a");
                c.set_text_align("center");
                text(c, "🚫 BANISH", x + cw * 0.5, yy - 10.0);
            }
        }
        c.set_global_alpha(1.0);

        // Reroll / Banish / Skip
        let kb = !self.input.is_touch;
        let labels = [
            ("🎲 Reroll", self.rerolls, "R", false),
            (if self.banish_mode { "✖ Cancel" } else { "🚫 Banish" }, self.banishes, "B", self.banish_mode),
            ("⏭ Skip +HP", self.skips, "X", false),
        ];
        let bk = ease_out(((self.levelup_t - 0.25) / 0.3).clamp(0.0, 1.0));
        c.set_global_alpha(d(bk));
        for (r, (label, n, key, active)) in self.levelup_buttons().iter().zip(labels) {
            round_rect(c, r.0, r.1, r.2, r.3, 10.0);
            c.set_fill_style_str(if active {
                "rgba(255,60,80,0.35)"
            } else if n > 0 {
                "rgba(255,255,255,0.10)"
            } else {
                "rgba(255,255,255,0.03)"
            });
            c.fill();
            c.set_stroke_style_str(if n > 0 { "rgba(255,255,255,0.35)" } else { "rgba(255,255,255,0.1)" });
            c.set_line_width(1.5);
            c.stroke();
            c.set_text_align("center");
            c.set_font(&format!("800 {}px {}", if r.2 < 110.0 { 12 } else { 14 }, font));
            c.set_fill_style_str(if n > 0 { "#ffffff" } else { "#6a7080" });
            let cap = if kb { format!("{} ({}) · {}", label, n, key) } else { format!("{} ({})", label, n) };
            text(c, &cap, r.0 + r.2 * 0.5, r.1 + r.3 * 0.5 + 1.0);
        }
        c.set_global_alpha(1.0);
    }
}
