//! Volleys: telegraphed patterns of enemy shots that close in on a player
//! from every side. They start a few waves in (sooner on harder
//! difficulties), then come more often and with more shots as the waves
//! climb, so dodging gets genuinely hard deep into a run.
//!
//! Each shot hangs for a moment (glowing, with a warning ring) and then
//! launches at the nearest player, so standing still is the one wrong answer.
//! Host/solo only, in Infinite and Campaign; clients see the shots through
//! snapshots like any other enemy fire.

use crate::audio::Sfx;
use crate::game::*;
use crate::math::*;
use std::f32::consts::TAU;

/// Distance from the target at which volley shots appear.
pub const VOLLEY_R: f32 = 400.0;
/// How long the shots hang before launching.
pub const VOLLEY_WARN: f32 = 0.9;

impl Game {
    /// First wave with volleys: 8 on Survivor, 4 on Nightmare, 13 on Rookie.
    pub fn volley_first_wave(&self) -> u32 {
        (8.0 / self.difficulty().shots).round().max(3.0) as u32
    }

    /// Shots in one volley at wave `n`.
    pub fn volley_count(&self, n: u32) -> usize {
        let k = n.saturating_sub(self.volley_first_wave()) as f32;
        ((6.0 + k * 0.9) * self.difficulty().shots.sqrt()).min(34.0) as usize
    }

    /// Seconds between volleys at wave `n`.
    pub fn volley_gap(&self, n: u32) -> f32 {
        let k = n.saturating_sub(self.volley_first_wave()) as f32;
        (15.0 - 0.4 * k).max(5.0) / self.difficulty().shots.sqrt()
    }

    pub(crate) fn update_volleys(&mut self) {
        let n = self.wave.n;
        if n < self.volley_first_wave() || self.wave.intermission > 0.0 || self.victory_t >= 0.0 {
            return;
        }
        self.volley_t -= DT;
        if self.volley_t > 0.0 {
            return;
        }
        self.volley_t = self.volley_gap(n) * self.rng.range(0.85, 1.15);
        let targets = self.targets();
        if targets.is_empty() {
            return;
        }
        let (_, at, _) = targets[self.rng.below(targets.len())];
        self.fire_volley(at, n);
    }

    /// Lay out one volley around `at`: a ring with a gap to slip through, an
    /// arc from one side, or a spiral that launches one shot after another.
    pub(crate) fn fire_volley(&mut self, at: V2, n: u32) {
        let count = self.volley_count(n);
        let k = n.saturating_sub(self.volley_first_wave()) as f32;
        let speed = 210.0 * (1.0 + 0.012 * k).min(1.45);
        let dmg = 9.0 * self.difficulty().dmg * (1.0 + 0.04 * n as f32);
        let a0 = self.rng.angle();
        let mut shots: Vec<(f32, f32)> = Vec::with_capacity(count); // (angle, extra delay)
        match self.rng.below(3) {
            0 => {
                // full ring, minus a gap (narrower later on)
                let gap = (1.1 - 0.02 * k).max(0.55);
                let m = count + 2;
                for i in 0..m {
                    let a = a0 + i as f32 * TAU / m as f32;
                    let off = (a - a0).rem_euclid(TAU);
                    if off > gap * 0.5 && off < TAU - gap * 0.5 {
                        shots.push((a, 0.0));
                    }
                }
            }
            1 => {
                // an arc from one side, in two staggered rows
                let spread = 1.9;
                for i in 0..count {
                    let f = i as f32 / (count.max(2) - 1) as f32 - 0.5;
                    shots.push((a0 + f * spread, if i % 2 == 0 { 0.0 } else { 0.35 }));
                }
            }
            _ => {
                // spiral: shots launch one after another around the circle
                for i in 0..count {
                    shots.push((a0 + i as f32 * TAU / count as f32, i as f32 * 0.07));
                }
            }
        }
        for (a, extra) in shots {
            let p = at + V2::from_angle(a) * VOLLEY_R;
            let p = crate::world::resolve(p, 7.0).0;
            self.eshots.push(EShot {
                pos: p,
                prev: p,
                vel: (at - p).norm() * speed,
                life: 3.4,
                dmg,
                dead: false,
                turn: 0.0,
                r: 7.0,
                grazed: false,
                delay: VOLLEY_WARN + extra,
            });
        }
        self.ring(at, VOLLEY_R * 0.35, 0.6, "#ff4a5a", 3.0);
        self.sfx(Sfx::Spit, at);
        if !self.volley_seen {
            self.volley_seen = true;
            let p = at + v2(0.0, -50.0);
            self.push_text(p, "VOLLEY! Keep moving".into(), "#ff6a7a", 20.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn volleys_ramp_with_waves_and_difficulty() {
        let mut g = Game::new(3);
        g.start("diff=survivor");
        assert_eq!(g.volley_first_wave(), 8);
        assert!(g.volley_count(20) > g.volley_count(8), "more shots later");
        assert!(g.volley_gap(25) < g.volley_gap(8), "more often later");
        let mut n = Game::new(3);
        n.start("diff=nightmare");
        assert!(n.volley_first_wave() < 8);
        assert!(n.volley_count(20) > g.volley_count(20));
        let mut r = Game::new(3);
        r.start("diff=rookie");
        assert!(r.volley_first_wave() > 8);

        // shots hang, then fly at the player and can hit
        g.player.pos = v2(5000.0, 5000.0);
        g.player.prev = g.player.pos;
        g.player.iframes = 0.0;
        g.eshots.clear();
        let at = g.player.pos;
        g.fire_volley(at, 12);
        assert!(!g.eshots.is_empty());
        assert!(g.eshots.iter().all(|s| s.delay > 0.0));
        let before: Vec<V2> = g.eshots.iter().map(|s| s.pos).collect();
        for _ in 0..((VOLLEY_WARN / DT) as usize - 2) {
            g.update_eshots();
        }
        assert!(g.eshots.iter().zip(before.iter()).all(|(s, b)| s.pos == *b), "they hang first");
        let hp = g.player.hp;
        for _ in 0..((3.0 / DT) as usize) {
            g.update_eshots();
            g.eshots.retain(|s| !s.dead);
        }
        assert!(g.player.hp < hp, "a volley you don't dodge hurts");
    }
}
