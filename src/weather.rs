//! Day/night and weather for the open world (Infinite / Campaign). A full day
//! lasts six minutes; nights are darker and bring more runners. Rain and fog
//! roll in and out, cutting visibility.

use crate::game::{Game, Phase};
use crate::math::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Sky {
    Clear,
    Rain,
    Fog,
}

pub struct Weather {
    pub sky: Sky,
    /// 0..1 how strongly the current weather shows (fades in/out)
    pub k: f32,
    /// seconds until the weather changes
    pub next: f32,
    /// 0..1 position in the day (0 = dawn, 0.25 noon, 0.5 dusk, 0.75 midnight)
    pub clock: f32,
}

pub const DAY_LEN: f32 = 360.0;

impl Weather {
    pub fn new() -> Weather {
        Weather { sky: Sky::Clear, k: 0.0, next: 90.0, clock: 0.2 }
    }
    /// 0 at noon, 1 at midnight
    pub fn night(&self) -> f32 {
        let a = (self.clock - 0.75) * std::f32::consts::TAU;
        (0.5 + 0.5 * a.cos()).clamp(0.0, 1.0)
    }
    pub fn rain(&self) -> f32 {
        if self.sky == Sky::Rain { self.k } else { 0.0 }
    }
    pub fn fog(&self) -> f32 {
        if self.sky == Sky::Fog { self.k } else { 0.0 }
    }
}

impl Game {
    pub fn world_has_sky(&self) -> bool {
        crate::world::map().is_none() && self.hub.is_none()
    }

    pub fn tick_weather(&mut self, dt: f32) {
        if !matches!(self.phase, Phase::Playing | Phase::LevelUp) || !self.world_has_sky() {
            return;
        }
        let w = &mut self.weather;
        w.clock = (w.clock + dt / DAY_LEN).fract();
        w.next -= dt;
        if w.next <= 0.0 {
            // fade out, then pick the next sky
            w.k -= dt * 0.25;
            if w.k <= 0.0 {
                w.k = 0.0;
                let r = self.rng.f();
                let sky = if r < 0.45 { Sky::Clear } else if r < 0.75 { Sky::Rain } else { Sky::Fog };
                self.weather.sky = sky;
                self.weather.next = self.rng.range(70.0, 150.0);
                let msg = match sky {
                    Sky::Rain => Some("Rain rolls in…"),
                    Sky::Fog => Some("A fog creeps over the horizon…"),
                    Sky::Clear => None,
                };
                if let Some(m) = msg {
                    self.toast = Some((m.into(), 2.4));
                }
            }
        } else if w.sky != Sky::Clear {
            w.k = (w.k + dt * 0.2).min(1.0);
        }
    }

    /// Extra darkness from the hour and the weather (added to the light mask).
    pub fn sky_darkness(&self) -> f32 {
        if !self.world_has_sky() {
            return 0.0;
        }
        let w = &self.weather;
        0.28 * w.night() + 0.1 * w.rain() + 0.12 * w.fog() - 0.12 * (1.0 - w.night())
    }

    /// Screen-space rain streaks and fog haze.
    pub fn draw_weather(&self, c: &web_sys::CanvasRenderingContext2d, w: f32, h: f32, t: f32) {
        if !self.world_has_sky() || self.phase == Phase::Menu {
            return;
        }
        let rain = self.weather.rain();
        if rain > 0.01 {
            c.set_stroke_style_str("rgba(170,190,230,0.35)");
            c.set_line_width(1.2);
            c.set_global_alpha((rain * 0.9) as f64);
            c.begin_path();
            let n = (160.0 * rain) as i32;
            for k in 0..n {
                let seed = hashf(k, 7, 31);
                let x = (seed * w * 1.3 + t * 260.0 + (self.cam.x * self.zoom) * -0.2).rem_euclid(w * 1.3) - w * 0.15;
                let y = (hashf(k, 9, 37) * h + t * 900.0 * (0.8 + seed * 0.4)).rem_euclid(h + 40.0) - 20.0;
                c.move_to(x as f64, y as f64);
                c.line_to((x - 6.0) as f64, (y + 18.0) as f64);
            }
            c.stroke();
            c.set_global_alpha(1.0);
        }
        let fog = self.weather.fog();
        if fog > 0.01 {
            if let Ok(g) = c.create_radial_gradient((w * 0.5) as f64, (h * 0.5) as f64, (h * 0.15) as f64, (w * 0.5) as f64, (h * 0.5) as f64, (w.max(h) * 0.7) as f64) {
                let _ = g.add_color_stop(0.0, "rgba(160,170,180,0)");
                let _ = g.add_color_stop(1.0, &format!("rgba(150,160,175,{:.3})", 0.55 * fog));
                c.set_fill_style_canvas_gradient(&g);
                c.fill_rect(0.0, 0.0, w as f64, h as f64);
            }
        }
    }
}
