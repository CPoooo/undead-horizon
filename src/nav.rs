//! Flow-field pathfinding for authored maps. A coarse grid covers the map;
//! cells blocked by walls, closed doors or props are marked solid, and a
//! multi-source BFS from the targets (players, or the Siege crystal) gives
//! every open cell a distance. Zombies walk downhill. Rebuilt when doors open
//! and a few times per second as targets move. Bats ignore it.

use crate::math::*;
use crate::world::AuthoredMap;
use std::collections::VecDeque;

pub const NAV_CELL: f32 = 40.0;

pub struct FlowField {
    pub origin: V2,
    pub w: i32,
    pub h: i32,
    pub solid: Vec<bool>,
    pub dist: Vec<u16>,
    /// rebuild timer
    pub t: f32,
    /// bumped when doors change so `solid` gets rebuilt
    pub doors_sig: u64,
}

const FAR: u16 = u16::MAX;

impl FlowField {
    pub fn new(m: &AuthoredMap) -> FlowField {
        let [x0, y0, x1, y1] = m.bounds;
        let w = ((x1 - x0) / NAV_CELL).ceil() as i32;
        let h = ((y1 - y0) / NAV_CELL).ceil() as i32;
        let mut f = FlowField {
            origin: v2(x0, y0),
            w,
            h,
            solid: vec![false; (w * h) as usize],
            dist: vec![FAR; (w * h) as usize],
            t: 0.0,
            doors_sig: u64::MAX,
        };
        f.rebuild_solid(m);
        f
    }

    pub fn cell(&self, p: V2) -> Option<(i32, i32)> {
        let x = ((p.x - self.origin.x) / NAV_CELL).floor() as i32;
        let y = ((p.y - self.origin.y) / NAV_CELL).floor() as i32;
        if x < 0 || y < 0 || x >= self.w || y >= self.h {
            None
        } else {
            Some((x, y))
        }
    }

    pub fn center(&self, x: i32, y: i32) -> V2 {
        self.origin + v2((x as f32 + 0.5) * NAV_CELL, (y as f32 + 0.5) * NAV_CELL)
    }

    fn idx(&self, x: i32, y: i32) -> usize {
        (y * self.w + x) as usize
    }

    pub fn doors_signature(m: &AuthoredMap) -> u64 {
        m.doors
            .iter()
            .enumerate()
            .fold(0u64, |acc, (i, d)| acc | ((d.open.get() as u64) << (i % 64)))
    }

    /// Mark cells whose centre sits inside a wall, closed door or prop.
    pub fn rebuild_solid(&mut self, m: &AuthoredMap) {
        self.doors_sig = FlowField::doors_signature(m);
        let margin = 12.0; // keep paths off the wall faces
        for y in 0..self.h {
            for x in 0..self.w {
                let c = self.center(x, y);
                let mut solid = false;
                for s in &m.segs {
                    if let Some(d) = s.door {
                        if m.doors[d].open.get() {
                            continue;
                        }
                    }
                    if s.closest(c).dist(c) < s.hr + margin {
                        solid = true;
                        break;
                    }
                }
                if !solid {
                    solid = m.props.iter().any(|o| o.pos.dist(c) < o.r + margin);
                }
                let i = self.idx(x, y);
                self.solid[i] = solid;
            }
        }
    }

    /// Distances from the nearest target, through open cells (8-connected).
    pub fn compute(&mut self, targets: &[V2]) {
        self.dist.iter_mut().for_each(|d| *d = FAR);
        let mut q = VecDeque::new();
        for t in targets {
            if let Some((x, y)) = self.cell(*t) {
                let i = self.idx(x, y);
                self.dist[i] = 0;
                q.push_back((x, y));
            }
        }
        while let Some((x, y)) = q.pop_front() {
            let d = self.dist[self.idx(x, y)];
            for (dx, dy, cost) in [
                (1, 0, 2u16),
                (-1, 0, 2),
                (0, 1, 2),
                (0, -1, 2),
                (1, 1, 3),
                (1, -1, 3),
                (-1, 1, 3),
                (-1, -1, 3),
            ] {
                let (nx, ny) = (x + dx, y + dy);
                if nx < 0 || ny < 0 || nx >= self.w || ny >= self.h {
                    continue;
                }
                let ni = self.idx(nx, ny);
                if self.solid[ni] {
                    continue;
                }
                // no corner cutting through walls
                if dx != 0 && dy != 0 && (self.solid[self.idx(x + dx, y)] || self.solid[self.idx(x, y + dy)]) {
                    continue;
                }
                let nd = d.saturating_add(cost);
                if nd < self.dist[ni] {
                    self.dist[ni] = nd;
                    q.push_back((nx, ny));
                }
            }
        }
    }

    /// Direction to walk from `p` (unit vector), or None when unreachable.
    pub fn dir(&self, p: V2) -> Option<V2> {
        let (x, y) = self.cell(p)?;
        let here = self.dist[self.idx(x, y)];
        let mut best = (here, None);
        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1)] {
            let (nx, ny) = (x + dx, y + dy);
            if nx < 0 || ny < 0 || nx >= self.w || ny >= self.h {
                continue;
            }
            if dx != 0 && dy != 0 && (self.solid[self.idx(x + dx, y)] || self.solid[self.idx(x, y + dy)]) {
                continue;
            }
            let d = self.dist[self.idx(nx, ny)];
            if d < best.0 {
                best = (d, Some((nx, ny)));
            }
        }
        // a zombie standing in a solid cell (hugging a wall): step to any open neighbour
        if here == FAR && best.1.is_none() {
            return None;
        }
        best.1.map(|(nx, ny)| (self.center(nx, ny) - p).norm())
    }

    #[cfg(test)]
    pub fn reachable(&self, p: V2) -> bool {
        self.cell(p).map(|(x, y)| self.dist[self.idx(x, y)] != FAR).unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::load_builtin;

    #[test]
    fn flow_goes_around_walls_and_through_open_doors() {
        let m = load_builtin("testbox").unwrap();
        let mut f = FlowField::new(&m);
        // target in the west room; a zombie in the east room can't reach it
        f.compute(&[v2(-200.0, 0.0)]);
        assert!(!f.reachable(v2(200.0, 0.0)), "door closed");
        assert!(f.reachable(v2(-300.0, -100.0)));
        // open the door: now there's a path, and it leads toward the doorway
        m.open_door(m.door_index("east").unwrap());
        f.rebuild_solid(&m);
        f.compute(&[v2(-200.0, 0.0)]);
        assert!(f.reachable(v2(200.0, 0.0)));
        let mut p = v2(250.0, -300.0);
        for _ in 0..400 {
            if let Some(d) = f.dir(p) {
                p += d * 8.0;
            }
        }
        assert!(p.x < 0.0, "walked through the door to the west room, at {:?}", (p.x, p.y));
    }
}
