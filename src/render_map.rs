//! Drawing for authored maps: zone floors, 2.5D wall prisms, doors and the
//! extra prop kinds (crates, barrels, pillars). Walls are cut into short chunks
//! so they depth-sort correctly against the actors walking past them.

use crate::game::Game;
use crate::math::*;
use crate::world::{AuthoredMap, ObKind, Obstacle, Seg};
use std::f64::consts::TAU as TAU64;
use web_sys::CanvasRenderingContext2d as Ctx;

#[inline]
fn d(x: f32) -> f64 {
    x as f64
}

/// A piece of wall (or door) to draw, sorted by `key` with everything else.
#[derive(Clone, Copy)]
pub struct WallChunk {
    pub seg: Seg,
    pub key: f32,
    /// draw the end faces only where the real segment ends (no seams between chunks)
    pub cap_a: bool,
    pub cap_b: bool,
}

const CHUNK: f32 = 90.0;

fn poly(c: &Ctx, pts: &[V2]) {
    c.begin_path();
    for (i, p) in pts.iter().enumerate() {
        if i == 0 {
            c.move_to(d(p.x), d(p.y));
        } else {
            c.line_to(d(p.x), d(p.y));
        }
    }
    c.close_path();
}

impl Game {
    /// Floors per zone (locked zones are dimmed) and the void outside the map.
    pub fn draw_map_ground(&self, c: &Ctx, m: &AuthoredMap, vmin: V2, vmax: V2) {
        c.set_fill_style_str("#07080b");
        c.fill_rect(d(vmin.x), d(vmin.y), d(vmax.x - vmin.x), d(vmax.y - vmin.y));
        let [x0, y0, x1, y1] = m.bounds;
        c.set_fill_style_str("#1c1f24");
        c.fill_rect(d(x0), d(y0), d(x1 - x0), d(y1 - y0));
        for (zi, z) in m.zones.iter().enumerate() {
            let [zx0, zy0, zx1, zy1] = z.rect;
            if zx1 < vmin.x || zx0 > vmax.x || zy1 < vmin.y || zy0 > vmax.y {
                continue;
            }
            let (base, alt) = match z.floor.as_str() {
                "tile" => ("#2b3038", "#323844"),
                "wood" => ("#3a2b1e", "#33261a"),
                "metal" => ("#2a3036", "#343c44"),
                "dirt" => ("#2e2719", "#352d1d"),
                "grass" => ("#1f2b1b", "#263420"),
                "blood" => ("#2b1c1e", "#341f22"),
                _ => ("#2c2e33", "#313338"),
            };
            c.set_fill_style_str(base);
            c.fill_rect(d(zx0), d(zy0), d(zx1 - zx0), d(zy1 - zy0));
            // pattern, clipped to what's on screen
            let (px0, py0) = (zx0.max(vmin.x), zy0.max(vmin.y));
            let (px1, py1) = (zx1.min(vmax.x), zy1.min(vmax.y));
            c.set_fill_style_str(alt);
            c.begin_path();
            match z.floor.as_str() {
                "tile" | "metal" => {
                    let ts = if z.floor == "tile" { 48.0 } else { 64.0 };
                    let tx0 = ((px0 - zx0) / ts).floor() as i32;
                    let ty0 = ((py0 - zy0) / ts).floor() as i32;
                    let tx1 = ((px1 - zx0) / ts).ceil() as i32;
                    let ty1 = ((py1 - zy0) / ts).ceil() as i32;
                    for ty in ty0..ty1 {
                        for tx in tx0..tx1 {
                            if (tx + ty) % 2 == 0 {
                                let x = zx0 + tx as f32 * ts;
                                let y = zy0 + ty as f32 * ts;
                                c.rect(d(x), d(y), d(ts.min(zx1 - x)), d(ts.min(zy1 - y)));
                            }
                        }
                    }
                }
                "wood" => {
                    let ph = 22.0;
                    let r0 = ((py0 - zy0) / ph).floor() as i32;
                    let r1 = ((py1 - zy0) / ph).ceil() as i32;
                    for r in r0..r1 {
                        if r % 2 == 0 {
                            let y = zy0 + r as f32 * ph;
                            c.rect(d(zx0), d(y), d(zx1 - zx0), d(ph.min(zy1 - y)));
                        }
                    }
                }
                _ => {
                    let ts = 72.0;
                    let tx0 = (px0 / ts).floor() as i32;
                    let ty0 = (py0 / ts).floor() as i32;
                    let tx1 = (px1 / ts).ceil() as i32;
                    let ty1 = (py1 / ts).ceil() as i32;
                    for ty in ty0..ty1 {
                        for tx in tx0..tx1 {
                            if hashf(tx, ty, 41 + zi as u32) < 0.3 {
                                let x = (tx as f32 * ts).max(zx0);
                                let y = (ty as f32 * ts).max(zy0);
                                let w = (tx as f32 * ts + ts).min(zx1) - x;
                                let h = (ty as f32 * ts + ts).min(zy1) - y;
                                if w > 0.0 && h > 0.0 {
                                    c.rect(d(x), d(y), d(w), d(h));
                                }
                            }
                        }
                    }
                }
            }
            c.fill();
            if !m.zone_is_open(zi) {
                c.set_fill_style_str("rgba(0,0,0,0.42)");
                c.fill_rect(d(zx0), d(zy0), d(zx1 - zx0), d(zy1 - zy0));
            }
        }
    }

    /// Wall pieces near the view, ready for depth sorting.
    pub fn map_wall_chunks(&self, m: &AuthoredMap, vmin: V2, vmax: V2, out: &mut Vec<WallChunk>) {
        let pad = 120.0;
        for s in &m.segs {
            if let Some(di) = s.door {
                if m.doors[di].open.get() {
                    continue;
                }
            }
            let lo = v2(s.a.x.min(s.b.x), s.a.y.min(s.b.y));
            let hi = v2(s.a.x.max(s.b.x), s.a.y.max(s.b.y));
            if hi.x < vmin.x - pad || lo.x > vmax.x + pad || hi.y < vmin.y - pad || lo.y > vmax.y + pad {
                continue;
            }
            let len = s.a.dist(s.b);
            let n = (len / CHUNK).ceil().max(1.0) as usize;
            for k in 0..n {
                let a = s.a.lerp(s.b, k as f32 / n as f32);
                let b = s.a.lerp(s.b, (k + 1) as f32 / n as f32);
                // sort by the chunk's front edge so actors behind it are hidden
                let key = a.y.max(b.y) + s.hr * 0.5;
                out.push(WallChunk {
                    seg: Seg { a, b, ..*s },
                    key,
                    cap_a: k == 0,
                    cap_b: k + 1 == n,
                });
            }
        }
    }

    pub fn draw_wall_chunk(&self, c: &Ctx, m: &AuthoredMap, w: &WallChunk) {
        let s = &w.seg;
        let dir = (s.b - s.a).norm();
        let n = if dir.len2() > 0.0 { dir.perp() } else { v2(0.0, 1.0) };
        let hr = s.hr;
        let base = [s.a + n * hr, s.b + n * hr, s.b - n * hr, s.a - n * hr];
        let door = s.door.is_some();
        let h = if door { m.wall_h * 0.8 } else { m.wall_h };
        let top: Vec<V2> = base.iter().map(|p| self.lift(*p, h)).collect();
        let (side, side2, cap) = if door {
            ("#5a3a20", "#4a2f19", "#8a6034")
        } else {
            ("#3b3f4a", "#30343d", "#5d6372")
        };
        // side faces, far ones first
        let mut faces: Vec<(f32, usize)> = (0..4)
            .map(|i| {
                let mid = base[i].lerp(base[(i + 1) % 4], 0.5);
                (-(mid - self.cam).len2(), i)
            })
            .collect();
        faces.sort_by(|x, y| x.0.partial_cmp(&y.0).unwrap_or(std::cmp::Ordering::Equal));
        for (_, i) in faces {
            // faces 1 and 3 are the ends at b and a
            if (i == 1 && !w.cap_b) || (i == 3 && !w.cap_a) {
                continue;
            }
            let j = (i + 1) % 4;
            c.set_fill_style_str(if i % 2 == 0 { side } else { side2 });
            poly(c, &[base[i], base[j], top[j], top[i]]);
            c.fill();
        }
        c.set_fill_style_str(cap);
        poly(c, &top);
        c.fill();
        if door {
            // boards across the door
            c.set_stroke_style_str("#2a1a0c");
            c.set_line_width(3.0);
            c.begin_path();
            let t0 = top[0].lerp(top[3], 0.5);
            let t1 = top[1].lerp(top[2], 0.5);
            for k in 0..3 {
                let f = 0.2 + k as f32 * 0.3;
                let p = t0.lerp(t1, f);
                c.move_to(d(p.x - n.x * hr), d(p.y - n.y * hr));
                c.line_to(d(p.x + n.x * hr), d(p.y + n.y * hr));
            }
            c.stroke();
        } else {
            c.set_stroke_style_str("rgba(255,255,255,0.08)");
            c.set_line_width(1.5);
            poly(c, &top);
            c.stroke();
        }
    }

    /// Crates, barrels and pillars (map props); returns false for kinds drawn elsewhere.
    pub fn draw_map_prop(&self, c: &Ctx, o: &Obstacle) -> bool {
        match o.kind {
            ObKind::Crate => {
                let r = o.r * 0.9;
                let base = [
                    o.pos + v2(-r, -r),
                    o.pos + v2(r, -r),
                    o.pos + v2(r, r),
                    o.pos + v2(-r, r),
                ];
                let top: Vec<V2> = base.iter().map(|p| self.lift(*p, 30.0)).collect();
                c.set_fill_style_str("#4a3420");
                for i in 0..4 {
                    let j = (i + 1) % 4;
                    poly(c, &[base[i], base[j], top[j], top[i]]);
                    c.fill();
                }
                c.set_fill_style_str("#8a6236");
                poly(c, &top);
                c.fill();
                c.set_stroke_style_str("#5a3d1f");
                c.set_line_width(2.5);
                c.begin_path();
                c.move_to(d(top[0].x), d(top[0].y));
                c.line_to(d(top[2].x), d(top[2].y));
                c.move_to(d(top[1].x), d(top[1].y));
                c.line_to(d(top[3].x), d(top[3].y));
                c.stroke();
                true
            }
            ObKind::Barrel => {
                let top = self.lift(o.pos, 34.0);
                c.set_fill_style_str("#7a1e14");
                let dir = (top - o.pos).norm();
                let side = if dir.len2() > 0.0 { dir.perp() } else { v2(1.0, 0.0) };
                poly(
                    c,
                    &[
                        o.pos + side * o.r,
                        top + side * o.r,
                        top - side * o.r,
                        o.pos - side * o.r,
                    ],
                );
                c.fill();
                c.set_fill_style_str("#b8321e");
                c.begin_path();
                let _ = c.arc(d(top.x), d(top.y), d(o.r), 0.0, TAU64);
                c.fill();
                c.set_fill_style_str("#ffd23a");
                c.begin_path();
                let _ = c.arc(d(top.x), d(top.y), d(o.r * 0.35), 0.0, TAU64);
                c.fill();
                true
            }
            ObKind::Pillar => {
                let top = self.lift(o.pos, 90.0);
                let dir = (top - o.pos).norm();
                let side = if dir.len2() > 0.0 { dir.perp() } else { v2(1.0, 0.0) };
                c.set_fill_style_str("#555a66");
                poly(
                    c,
                    &[
                        o.pos + side * o.r,
                        top + side * o.r,
                        top - side * o.r,
                        o.pos - side * o.r,
                    ],
                );
                c.fill();
                c.set_fill_style_str("#7d8290");
                c.begin_path();
                let _ = c.arc(d(top.x), d(top.y), d(o.r), 0.0, TAU64);
                c.fill();
                true
            }
            _ => false,
        }
    }

    /// Walls and doors on the minimap.
    pub fn draw_map_minimap(&self, c: &Ctx, m: &AuthoredMap, to_map: &dyn Fn(V2) -> V2, k: f32) {
        c.set_line_cap("round");
        for (closed_door, col) in [(false, "#8a90a0"), (true, "#c08040")] {
            c.set_stroke_style_str(col);
            c.begin_path();
            for s in &m.segs {
                let is_door = s.door.is_some();
                if is_door != closed_door {
                    continue;
                }
                if let Some(di) = s.door {
                    if m.doors[di].open.get() {
                        continue;
                    }
                }
                let a = to_map(s.a);
                let b = to_map(s.b);
                c.move_to(d(a.x), d(a.y));
                c.line_to(d(b.x), d(b.y));
            }
            c.set_line_width(d((m.segs.first().map(|s| s.hr).unwrap_or(10.0) * 2.0 * k).max(1.5)));
            c.stroke();
        }
    }
}
