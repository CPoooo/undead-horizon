//! The playfield. Two kinds of level share one interface:
//! - `ProceduralWorld`: infinite and deterministic. Nothing is stored: every
//!   obstacle and ground detail is derived from a hash of its cell coordinate,
//!   so the map is endless and costs zero memory.
//! - `AuthoredMap`: a hand-made map loaded from `assets/maps/*.json` (walls,
//!   doors, zones, spawn windows, props, interactables) for Outbreak and Siege.
//!
//! The active level is thread-local so the free functions below (`resolve`,
//! `blocking`, `for_obstacles_in`) work from anywhere in the sim.

use crate::math::{hash2, hashf, v2, V2};
use serde::Deserialize;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

pub const OBST_CELL: f32 = 230.0;
pub const SAFE_RADIUS: f32 = 340.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ObKind {
    Tree,
    Rock,
    Grave,
    Crate,
    Barrel,
    Pillar,
}

#[derive(Clone, Copy)]
pub struct Obstacle {
    pub pos: V2,
    pub r: f32,
    pub kind: ObKind,
    pub seed: u32,
}

/// Collision and steering queries every level answers.
pub trait Level {
    fn for_obstacles_in(&self, min: V2, max: V2, f: &mut dyn FnMut(&Obstacle));
    /// Push a circle out of solid geometry: (corrected position, push normal).
    fn resolve(&self, pos: V2, r: f32) -> (V2, V2);
    /// Nearest solid thing in the way of travel, as a circle to steer around.
    fn blocking(&self, pos: V2, dir: V2, look: f32, r: f32) -> Option<Obstacle>;
}

thread_local! {
    static LEVEL: RefCell<Rc<dyn Level>> = RefCell::new(Rc::new(ProceduralWorld));
}

/// Swap the active level (`None` = the infinite procedural world).
pub fn set_level(map: Option<Rc<AuthoredMap>>) {
    LEVEL.with(|l| {
        *l.borrow_mut() = match map {
            Some(m) => m,
            None => Rc::new(ProceduralWorld),
        }
    });
}

fn level() -> Rc<dyn Level> {
    LEVEL.with(|l| l.borrow().clone())
}

/// The authored map in play, if any.
pub fn map() -> Option<Rc<AuthoredMap>> {
    CURRENT_MAP.with(|m| m.borrow().clone())
}

thread_local! {
    static CURRENT_MAP: RefCell<Option<Rc<AuthoredMap>>> = const { RefCell::new(None) };
}

/// Load a map and make it the active level.
pub fn use_map(map: Option<Rc<AuthoredMap>>) {
    CURRENT_MAP.with(|m| *m.borrow_mut() = map.clone());
    set_level(map);
}

/// Visit every obstacle whose cell intersects the given AABB.
pub fn for_obstacles_in(min: V2, max: V2, mut f: impl FnMut(&Obstacle)) {
    level().for_obstacles_in(min, max, &mut f);
}

pub fn resolve(pos: V2, r: f32) -> (V2, V2) {
    level().resolve(pos, r)
}

pub fn blocking(pos: V2, dir: V2, look: f32, r: f32) -> Option<Obstacle> {
    level().blocking(pos, dir, look, r)
}

// ------------------------------------------------------------------ procedural

pub struct ProceduralWorld;

pub fn obstacle_at(cx: i32, cy: i32) -> Option<Obstacle> {
    let p = hashf(cx, cy, 11);
    // low-frequency "biome" density so there are forests and clearings
    let bx = (cx as f32 / 6.0).floor() as i32;
    let by = (cy as f32 / 6.0).floor() as i32;
    let density = 0.08 + hashf(bx, by, 99) * 0.28;
    if p > density {
        return None;
    }
    let seed = hash2(cx, cy, 7);
    let ox = 40.0 + hashf(cx, cy, 3) * (OBST_CELL - 80.0);
    let oy = 40.0 + hashf(cx, cy, 4) * (OBST_CELL - 80.0);
    let pos = v2(cx as f32 * OBST_CELL + ox, cy as f32 * OBST_CELL + oy);
    if pos.len() < SAFE_RADIUS {
        return None;
    }
    let k = hashf(cx, cy, 5);
    let (kind, r) = if k < 0.62 {
        (ObKind::Tree, 20.0 + hashf(cx, cy, 6) * 12.0)
    } else if k < 0.88 {
        (ObKind::Rock, 18.0 + hashf(cx, cy, 6) * 18.0)
    } else {
        (ObKind::Grave, 15.0)
    };
    Some(Obstacle { pos, r, kind, seed })
}

fn push_circle(p: &mut V2, n: &mut V2, r: f32, c: V2, cr: f32) {
    let d = *p - c;
    let rr = r + cr;
    let l2 = d.len2();
    if l2 < rr * rr {
        let l = l2.sqrt().max(0.001);
        let push = d * (1.0 / l);
        *p = c + push * rr;
        *n += push;
    }
}

fn steer_candidate(best: &mut Option<(f32, Obstacle)>, pos: V2, dir: V2, look: f32, r: f32, o: Obstacle) {
    let to = o.pos - pos;
    let along = to.dot(dir);
    if along <= 0.0 || along > look + o.r {
        return;
    }
    let lateral = (to - dir * along).len();
    if lateral < o.r + r + 4.0 && best.map_or(true, |b| along < b.0) {
        *best = Some((along, o));
    }
}

impl Level for ProceduralWorld {
    fn for_obstacles_in(&self, min: V2, max: V2, f: &mut dyn FnMut(&Obstacle)) {
        let x0 = (min.x / OBST_CELL).floor() as i32 - 1;
        let y0 = (min.y / OBST_CELL).floor() as i32 - 1;
        let x1 = (max.x / OBST_CELL).floor() as i32 + 1;
        let y1 = (max.y / OBST_CELL).floor() as i32 + 1;
        for cy in y0..=y1 {
            for cx in x0..=x1 {
                if let Some(o) = obstacle_at(cx, cy) {
                    f(&o);
                }
            }
        }
    }

    fn resolve(&self, pos: V2, r: f32) -> (V2, V2) {
        let mut p = pos;
        let mut n = V2::ZERO;
        let cx = (p.x / OBST_CELL).floor() as i32;
        let cy = (p.y / OBST_CELL).floor() as i32;
        for dy in -1..=1 {
            for dx in -1..=1 {
                if let Some(o) = obstacle_at(cx + dx, cy + dy) {
                    push_circle(&mut p, &mut n, r, o.pos, o.r);
                }
            }
        }
        (p, n)
    }

    fn blocking(&self, pos: V2, dir: V2, look: f32, r: f32) -> Option<Obstacle> {
        let cx = (pos.x / OBST_CELL).floor() as i32;
        let cy = (pos.y / OBST_CELL).floor() as i32;
        let mut best = None;
        for dy in -1..=1 {
            for dx in -1..=1 {
                if let Some(o) = obstacle_at(cx + dx, cy + dy) {
                    steer_candidate(&mut best, pos, dir, look, r, o);
                }
            }
        }
        best.map(|b| b.1)
    }
}

// ------------------------------------------------------------------ authored maps
// (map data is read by the Outbreak and Siege modes)

#[derive(Deserialize)]
pub struct MapDef {
    pub name: String,
    /// x0, y0, x1, y1: the playable area, walled in automatically
    pub bounds: [f32; 4],
    pub spawn: [f32; 2],
    #[serde(default = "default_wall_h")]
    pub wall_h: f32,
    #[serde(default)]
    pub zones: Vec<ZoneDef>,
    #[serde(default)]
    pub walls: Vec<WallDef>,
    #[serde(default)]
    pub doors: Vec<DoorDef>,
    #[serde(default)]
    pub windows: Vec<WindowDef>,
    #[serde(default)]
    pub props: Vec<PropDef>,
    #[serde(default)]
    pub interact: Vec<InteractDef>,
}

fn default_wall_h() -> f32 {
    64.0
}
fn default_t() -> f32 {
    24.0
}

#[derive(Deserialize, Clone)]
#[allow(dead_code)]
pub struct ZoneDef {
    pub id: String,
    pub name: String,
    pub rect: [f32; 4],
    #[serde(default)]
    pub floor: String,
    /// open from the start (the spawn room)
    #[serde(default)]
    pub start: bool,
}

#[derive(Deserialize)]
pub struct WallDef {
    pub pts: Vec<[f32; 2]>,
    #[serde(default = "default_t")]
    pub t: f32,
}

#[derive(Deserialize, Clone)]
#[allow(dead_code)]
pub struct DoorDef {
    pub id: String,
    pub a: [f32; 2],
    pub b: [f32; 2],
    #[serde(default)]
    pub cost: u32,
    /// zones this door connects; opening it opens them
    #[serde(default)]
    pub zones: Vec<String>,
    #[serde(default)]
    pub label: String,
}

#[derive(Deserialize, Clone)]
#[allow(dead_code)]
pub struct WindowDef {
    pub pos: [f32; 2],
    pub zone: String,
    /// direction zombies come in from (outside -> inside)
    #[serde(default)]
    pub dir: [f32; 2],
}

#[derive(Deserialize, Clone)]
pub struct PropDef {
    pub kind: String,
    pub pos: [f32; 2],
    #[serde(default)]
    pub r: f32,
}

#[derive(Deserialize, Clone)]
#[allow(dead_code)]
pub struct InteractDef {
    pub kind: String,
    pub pos: [f32; 2],
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub zone: String,
    #[serde(default)]
    pub cost: u32,
    #[serde(default)]
    pub data: String,
}

/// A solid wall segment: a capsule from `a` to `b` with half-thickness `hr`.
#[derive(Clone, Copy)]
pub struct Seg {
    pub a: V2,
    pub b: V2,
    pub hr: f32,
    /// index into `doors` when this segment is a door
    pub door: Option<usize>,
}

impl Seg {
    pub fn closest(&self, p: V2) -> V2 {
        let ab = self.b - self.a;
        let l2 = ab.len2();
        if l2 < 1e-6 {
            return self.a;
        }
        let t = ((p - self.a).dot(ab) / l2).clamp(0.0, 1.0);
        self.a + ab * t
    }
}

#[allow(dead_code)]
pub struct Door {
    pub def: DoorDef,
    pub open: Cell<bool>,
}

#[allow(dead_code)]
pub struct AuthoredMap {
    pub name: String,
    pub bounds: [f32; 4],
    pub spawn: V2,
    pub wall_h: f32,
    pub zones: Vec<ZoneDef>,
    pub segs: Vec<Seg>,
    pub doors: Vec<Door>,
    pub windows: Vec<WindowDef>,
    pub props: Vec<Obstacle>,
    pub interact: Vec<InteractDef>,
    /// zones the players have opened (by index)
    pub zone_open: Vec<Cell<bool>>,
    // spatial buckets of segment and prop indices
    cell: f32,
    gw: i32,
    gh: i32,
    seg_cells: Vec<Vec<u16>>,
    prop_cells: Vec<Vec<u16>>,
}

pub const MAP_CELL: f32 = 160.0;

#[allow(dead_code)]
impl AuthoredMap {
    pub fn parse(json: &str) -> Result<AuthoredMap, String> {
        let def: MapDef = serde_json::from_str(json).map_err(|e| e.to_string())?;
        Ok(AuthoredMap::build(def))
    }

    pub fn build(def: MapDef) -> AuthoredMap {
        let [x0, y0, x1, y1] = def.bounds;
        let mut segs = Vec::new();
        let border = [
            [v2(x0, y0), v2(x1, y0)],
            [v2(x1, y0), v2(x1, y1)],
            [v2(x1, y1), v2(x0, y1)],
            [v2(x0, y1), v2(x0, y0)],
        ];
        for [a, b] in border {
            segs.push(Seg { a, b, hr: 16.0, door: None });
        }
        for w in &def.walls {
            for p in w.pts.windows(2) {
                segs.push(Seg {
                    a: v2(p[0][0], p[0][1]),
                    b: v2(p[1][0], p[1][1]),
                    hr: w.t * 0.5,
                    door: None,
                });
            }
        }
        let mut doors = Vec::new();
        for (i, d) in def.doors.iter().enumerate() {
            segs.push(Seg {
                a: v2(d.a[0], d.a[1]),
                b: v2(d.b[0], d.b[1]),
                hr: 10.0,
                door: Some(i),
            });
            doors.push(Door {
                def: d.clone(),
                open: Cell::new(d.cost == 0 && d.zones.is_empty()),
            });
        }
        let props = def
            .props
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let (kind, r) = match p.kind.as_str() {
                    "tree" => (ObKind::Tree, 26.0),
                    "rock" => (ObKind::Rock, 26.0),
                    "grave" => (ObKind::Grave, 15.0),
                    "barrel" => (ObKind::Barrel, 16.0),
                    "pillar" => (ObKind::Pillar, 22.0),
                    _ => (ObKind::Crate, 20.0),
                };
                Obstacle {
                    pos: v2(p.pos[0], p.pos[1]),
                    r: if p.r > 0.0 { p.r } else { r },
                    kind,
                    seed: hash2(i as i32, 3, 77),
                }
            })
            .collect::<Vec<_>>();
        let zone_open = def.zones.iter().map(|z| Cell::new(z.start)).collect();
        let cell = MAP_CELL;
        let gw = ((x1 - x0) / cell).ceil() as i32 + 2;
        let gh = ((y1 - y0) / cell).ceil() as i32 + 2;
        let mut m = AuthoredMap {
            name: def.name,
            bounds: def.bounds,
            spawn: v2(def.spawn[0], def.spawn[1]),
            wall_h: def.wall_h,
            zones: def.zones,
            segs,
            doors,
            windows: def.windows,
            props,
            interact: def.interact,
            zone_open,
            cell,
            gw,
            gh,
            seg_cells: vec![Vec::new(); (gw * gh) as usize],
            prop_cells: vec![Vec::new(); (gw * gh) as usize],
        };
        for (i, s) in m.segs.iter().enumerate() {
            let pad = s.hr + 1.0;
            let (lo, hi) = (
                v2(s.a.x.min(s.b.x) - pad, s.a.y.min(s.b.y) - pad),
                v2(s.a.x.max(s.b.x) + pad, s.a.y.max(s.b.y) + pad),
            );
            let (cx0, cy0) = m.cell_of(lo);
            let (cx1, cy1) = m.cell_of(hi);
            for cy in cy0..=cy1 {
                for cx in cx0..=cx1 {
                    m.seg_cells[(cy * m.gw + cx) as usize].push(i as u16);
                }
            }
        }
        for (i, o) in m.props.iter().enumerate() {
            let (cx0, cy0) = m.cell_of(o.pos - v2(o.r, o.r));
            let (cx1, cy1) = m.cell_of(o.pos + v2(o.r, o.r));
            for cy in cy0..=cy1 {
                for cx in cx0..=cx1 {
                    m.prop_cells[(cy * m.gw + cx) as usize].push(i as u16);
                }
            }
        }
        m
    }

    fn cell_of(&self, p: V2) -> (i32, i32) {
        let x = ((p.x - self.bounds[0]) / self.cell).floor() as i32 + 1;
        let y = ((p.y - self.bounds[1]) / self.cell).floor() as i32 + 1;
        (x.clamp(0, self.gw - 1), y.clamp(0, self.gh - 1))
    }

    /// Visit solid segments and props near `p` (within `r`).
    fn near(&self, p: V2, r: f32, mut seg: impl FnMut(&Seg), mut prop: impl FnMut(&Obstacle)) {
        let (cx0, cy0) = self.cell_of(p - v2(r, r));
        let (cx1, cy1) = self.cell_of(p + v2(r, r));
        // segments can span several cells: dedupe with a tiny seen-list
        let mut seen: [u16; 32] = [u16::MAX; 32];
        let mut ns = 0;
        let mut seen_p: [u16; 32] = [u16::MAX; 32];
        let mut np = 0;
        for cy in cy0..=cy1 {
            for cx in cx0..=cx1 {
                let c = (cy * self.gw + cx) as usize;
                for &i in &self.seg_cells[c] {
                    if seen[..ns].contains(&i) {
                        continue;
                    }
                    if ns < 32 {
                        seen[ns] = i;
                        ns += 1;
                    }
                    let s = &self.segs[i as usize];
                    if let Some(d) = s.door {
                        if self.doors[d].open.get() {
                            continue;
                        }
                    }
                    seg(s);
                }
                for &i in &self.prop_cells[c] {
                    if seen_p[..np].contains(&i) {
                        continue;
                    }
                    if np < 32 {
                        seen_p[np] = i;
                        np += 1;
                    }
                    prop(&self.props[i as usize]);
                }
            }
        }
    }

    pub fn zone_index(&self, id: &str) -> Option<usize> {
        self.zones.iter().position(|z| z.id == id)
    }

    /// Zone containing a point (first match), if any.
    pub fn zone_at(&self, p: V2) -> Option<usize> {
        self.zones.iter().position(|z| {
            let [x0, y0, x1, y1] = z.rect;
            p.x >= x0 && p.x <= x1 && p.y >= y0 && p.y <= y1
        })
    }

    pub fn zone_is_open(&self, i: usize) -> bool {
        self.zone_open.get(i).map(|c| c.get()).unwrap_or(false)
    }

    /// Open a door and the zones behind it. Returns false if it was already open.
    pub fn open_door(&self, i: usize) -> bool {
        let Some(d) = self.doors.get(i) else {
            return false;
        };
        if d.open.get() {
            return false;
        }
        d.open.set(true);
        for z in &d.def.zones {
            if let Some(zi) = self.zone_index(z) {
                self.zone_open[zi].set(true);
            }
        }
        true
    }

    pub fn door_index(&self, id: &str) -> Option<usize> {
        self.doors.iter().position(|d| d.def.id == id)
    }

    pub fn door_center(&self, i: usize) -> V2 {
        let d = &self.doors[i].def;
        v2((d.a[0] + d.b[0]) * 0.5, (d.a[1] + d.b[1]) * 0.5)
    }

    pub fn inside(&self, p: V2) -> bool {
        let [x0, y0, x1, y1] = self.bounds;
        p.x > x0 && p.x < x1 && p.y > y0 && p.y < y1
    }

    /// True when the straight line a->b crosses no solid wall (for line of sight).
    pub fn clear_line(&self, a: V2, b: V2) -> bool {
        let steps = ((b - a).len() / 40.0).ceil().max(1.0) as i32;
        for k in 1..steps {
            let p = a.lerp(b, k as f32 / steps as f32);
            let mut hit = false;
            self.near(
                p,
                2.0,
                |s| {
                    if s.closest(p).dist2(p) < s.hr * s.hr {
                        hit = true;
                    }
                },
                |_| {},
            );
            if hit {
                return false;
            }
        }
        true
    }
}

impl Level for AuthoredMap {
    fn for_obstacles_in(&self, min: V2, max: V2, f: &mut dyn FnMut(&Obstacle)) {
        for o in &self.props {
            if o.pos.x + o.r > min.x && o.pos.x - o.r < max.x && o.pos.y + o.r > min.y && o.pos.y - o.r < max.y {
                f(o);
            }
        }
    }

    fn resolve(&self, pos: V2, r: f32) -> (V2, V2) {
        let mut p = pos;
        let mut n = V2::ZERO;
        // two passes settle corners where two walls meet
        for _ in 0..2 {
            let q = p;
            let mut segs: [Option<Seg>; 16] = [None; 16];
            let mut k = 0;
            let mut props: [Option<Obstacle>; 16] = [None; 16];
            let mut kp = 0;
            self.near(
                q,
                r + 40.0,
                |s| {
                    if k < 16 {
                        segs[k] = Some(*s);
                        k += 1;
                    }
                },
                |o| {
                    if kp < 16 {
                        props[kp] = Some(*o);
                        kp += 1;
                    }
                },
            );
            for s in segs.iter().flatten() {
                let c = s.closest(p);
                push_circle(&mut p, &mut n, r, c, s.hr);
            }
            for o in props.iter().flatten() {
                push_circle(&mut p, &mut n, r, o.pos, o.r);
            }
        }
        // never leave the map
        let [x0, y0, x1, y1] = self.bounds;
        p.x = p.x.clamp(x0 + r, x1 - r);
        p.y = p.y.clamp(y0 + r, y1 - r);
        (p, n)
    }

    fn blocking(&self, pos: V2, dir: V2, look: f32, r: f32) -> Option<Obstacle> {
        let mut best = None;
        let ahead = pos + dir * look * 0.5;
        let mut walls: Vec<Obstacle> = Vec::new();
        let mut props: Vec<Obstacle> = Vec::new();
        self.near(
            ahead,
            look + r,
            |s| {
                walls.push(Obstacle {
                    pos: s.closest(ahead),
                    r: s.hr,
                    kind: ObKind::Pillar,
                    seed: 0,
                });
            },
            |o| props.push(*o),
        );
        for o in walls.into_iter().chain(props) {
            steer_candidate(&mut best, pos, dir, look, r, o);
        }
        best.map(|b| b.1)
    }
}

/// Maps shipped with the game, embedded at compile time.
pub fn builtin_map(id: &str) -> Option<&'static str> {
    match id {
        "testbox" => Some(include_str!("../assets/maps/testbox.json")),
        _ => None,
    }
}

pub fn load_builtin(id: &str) -> Option<Rc<AuthoredMap>> {
    builtin_map(id).and_then(|j| AuthoredMap::parse(j).ok()).map(Rc::new)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_map_loads_and_walls_collide() {
        let m = load_builtin("testbox").expect("testbox parses");
        assert_eq!(m.name, "Test Box");
        assert!(m.segs.len() > 4 && !m.doors.is_empty());
        use_map(Some(m.clone()));
        // walking into the inner wall at x=0 from the left stops short of it
        let mut p = v2(-200.0, -150.0);
        for _ in 0..400 {
            p = resolve(p + v2(3.0, 0.0), 16.0).0;
        }
        assert!(p.x < -10.0, "blocked by the wall, got x={}", p.x);
        // the closed door blocks too; once opened, you walk through
        let mut q = v2(-200.0, 150.0);
        for _ in 0..400 {
            q = resolve(q + v2(3.0, 0.0), 16.0).0;
        }
        assert!(q.x < -10.0, "closed door blocks, got x={}", q.x);
        let d = m.door_index("east").unwrap();
        assert!(m.open_door(d));
        assert!(m.zone_is_open(m.zone_index("east").unwrap()));
        for _ in 0..400 {
            q = resolve(q + v2(3.0, 0.0), 16.0).0;
        }
        assert!(q.x > 100.0, "open door lets you through, got x={}", q.x);
        // bounds hold
        let (b, _) = resolve(v2(99999.0, 0.0), 16.0);
        assert!(b.x <= m.bounds[2]);
        // props collide
        let prop = m.props[0];
        let (pp, nrm) = resolve(prop.pos + v2(1.0, 0.0), 16.0);
        assert!(nrm.len2() > 0.0 && pp.dist(prop.pos) >= prop.r + 15.9);
        use_map(None);
        assert!(map().is_none());
    }
}
