//! AI Director (Left 4 Dead style pacing) for the horde modes (Infinite and
//! Campaign). It tracks the squad's stress and paces spawns in cycles:
//!
//! - **BuildUp**: normal spawning while stress climbs.
//! - **Peak**: stress maxed out; hold the pressure for a few seconds.
//! - **Relax**: spawns slow right down so players can breathe, loot and heal,
//!   with a mercy health drop for whoever is hurting.
//!
//! If things stay too quiet during a build-up, it sends a **horde rush** from
//! one direction. Wave totals are unchanged: only their pacing moves.
//! Host/solo only (clients don't run `update_waves`).

use crate::audio::Sfx;
use crate::game::{Banner, Game, PickKind, Pickup, ZKind, DT};
use crate::math::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Tempo {
    #[default]
    BuildUp,
    Peak,
    Relax,
}

#[derive(Default)]
pub struct Director {
    pub tempo: Tempo,
    /// 0..1 squad stress
    pub intensity: f32,
    /// time in the current tempo
    pub t: f32,
    /// time stress has stayed low during a build-up
    pub calm_t: f32,
    pub rush_cd: f32,
    pub mercy_cd: f32,
    /// last frame's total squad hp (to notice damage)
    last_hp: f32,
    pub rushes: u32,
}

pub const PEAK_AT: f32 = 0.85;
const PEAK_HOLD: f32 = 4.0;
const RELAX_MIN: f32 = 8.0;
const RELAX_MAX: f32 = 20.0;
const RUSH_CALM: f32 = 12.0;

impl Director {
    /// Spawn-interval multiplier for the current tempo.
    pub fn spawn_mult(&self) -> f32 {
        match self.tempo {
            Tempo::BuildUp | Tempo::Peak => 1.0,
            Tempo::Relax => 3.5,
        }
    }
    /// Nudge for the music intensity.
    pub fn music_bias(&self) -> f32 {
        match self.tempo {
            Tempo::BuildUp => 0.0,
            Tempo::Peak => 0.25,
            Tempo::Relax => -0.25,
        }
    }
    fn enter(&mut self, t: Tempo) {
        self.tempo = t;
        self.t = 0.0;
        self.calm_t = 0.0;
    }
}

impl Game {
    /// (hp fraction, position) of every living player, plus the squad's hp total.
    fn squad_health(&self) -> (Vec<(f32, V2)>, f32) {
        let mut v = Vec::with_capacity(4);
        let mut total = 0.0;
        if self.player_alive() {
            v.push(((self.player.hp / self.player.max_hp.max(1.0)).clamp(0.0, 1.0), self.player.pos));
            total += self.player.hp.max(0.0);
        }
        if let Some(n) = &self.net {
            for r in n.remotes.iter().filter(|r| r.present && r.state.alive()) {
                v.push(((r.state.hp / r.state.max_hp.max(1.0)).clamp(0.0, 1.0), r.state.pos));
                total += r.state.hp.max(0.0);
            }
        }
        (v, total)
    }

    /// One director tick: update stress and tempo. Returns true when a horde
    /// rush should spawn now.
    pub(crate) fn director_tick(&mut self) -> bool {
        let (squad, hp_total) = self.squad_health();
        if squad.is_empty() {
            return false;
        }
        // stress inputs: crowding, low health, and damage taken
        let mut near = 0u32;
        for z in self.zombies.iter().filter(|z| !z.dead && !z.dummy) {
            if squad.iter().any(|s| s.1.dist2(z.pos) < 220.0 * 220.0) {
                near += 1;
            }
        }
        let lowest = squad.iter().map(|s| s.0).fold(1.0, f32::min);
        let d = &mut self.director;
        let hurt = (d.last_hp - hp_total).max(0.0);
        d.last_hp = hp_total;
        let squad_max = 100.0 * squad.len() as f32;
        d.intensity += hurt / squad_max * 1.6;
        d.intensity += DT * ((near as f32 / 12.0).min(1.5) * 0.12 + (1.0 - lowest) * 0.06);
        d.intensity -= DT * if d.tempo == Tempo::Relax { 0.09 } else { 0.03 };
        d.intensity = d.intensity.clamp(0.0, 1.0);
        d.t += DT;
        d.rush_cd -= DT;
        d.mercy_cd -= DT;
        let mut rush = false;
        match d.tempo {
            Tempo::BuildUp => {
                if d.intensity >= PEAK_AT {
                    d.enter(Tempo::Peak);
                } else if d.intensity < 0.15 {
                    d.calm_t += DT;
                    if d.calm_t > RUSH_CALM && d.rush_cd <= 0.0 {
                        d.calm_t = 0.0;
                        d.rush_cd = 45.0;
                        d.rushes += 1;
                        rush = true;
                    }
                } else {
                    d.calm_t = 0.0;
                }
            }
            Tempo::Peak => {
                if d.t >= PEAK_HOLD {
                    d.enter(Tempo::Relax);
                    // mercy: the most hurt player gets a heart nearby
                    if lowest < 0.35 && d.mercy_cd <= 0.0 {
                        d.mercy_cd = 60.0;
                        let at = squad.iter().fold(squad[0], |a, b| if b.0 < a.0 { *b } else { a }).1;
                        let p = at + self.rng.unit() * 90.0;
                        self.pickups.push(Pickup { pos: p, kind: PickKind::Heart, t: 0.0, dead: false });
                        self.ring(p, 60.0, 0.5, "#ff5c7a", 4.0);
                    }
                }
            }
            Tempo::Relax => {
                if (d.t >= RELAX_MIN && d.intensity < 0.3) || d.t >= RELAX_MAX {
                    d.enter(Tempo::BuildUp);
                }
            }
        }
        rush
    }

    /// A horde rush: a big group from one direction, telegraphed.
    pub(crate) fn horde_rush(&mut self) {
        let left = self.wave.total.saturating_sub(self.wave.spawned);
        let n = (10 + self.scale_n() * 2).min(left).min(40);
        if n < 6 {
            return;
        }
        let targets = self.targets();
        let (_, ppos, _) = targets[self.rng.below(targets.len())];
        let a = self.rng.angle();
        let base = ppos + V2::from_angle(a) * self.spawn_r;
        for i in 0..n {
            let k = if i % 5 == 4 { ZKind::Runner } else { self.pick_kind() };
            let p = base + self.rng.unit() * self.rng.range(0.0, 140.0);
            self.spawn_zombie(k, p);
        }
        self.wave.spawned += n;
        self.banner = Some(Banner {
            title: "HORDE INCOMING".into(),
            sub: "They're coming from one side, get ready".into(),
            t: 0.0,
            dur: 2.2,
            color: "#ff9a5a",
        });
        self.say("here they come");
        self.sfx(Sfx::BossRoar, base);
        self.add_shake(0.25);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::Phase;

    fn game() -> Game {
        let mut g = Game::new(7);
        g.screen = v2(1280.0, 720.0);
        g.zoom = 1.0;
        g.start("mode=infinite;diff=survivor");
        g.phase = Phase::Playing;
        g
    }

    #[test]
    fn damage_drives_a_peak_then_a_relax_with_a_mercy_heart() {
        let mut g = game();
        g.director_tick();
        assert_eq!(g.director.tempo, Tempo::BuildUp);
        // a beating: the squad's stress peaks
        g.player.hp = 20.0;
        g.director_tick();
        g.player.hp = 12.0;
        for _ in 0..(120 * 3) {
            g.director.intensity = (g.director.intensity + 0.01).min(1.0);
            g.director_tick();
            if g.director.tempo != Tempo::BuildUp {
                break;
            }
        }
        assert_eq!(g.director.tempo, Tempo::Peak);
        let hearts = g.pickups.len();
        for _ in 0..(120.0 * (PEAK_HOLD + 0.1)) as usize {
            g.director_tick();
        }
        assert_eq!(g.director.tempo, Tempo::Relax);
        assert_eq!(g.pickups.len(), hearts + 1, "mercy heart for a hurt player");
        assert!(g.director.spawn_mult() > 1.0, "spawns slow down while relaxing");
        // calm returns to a build-up within the relax window
        g.player.hp = g.player.max_hp;
        for _ in 0..(120.0 * (RELAX_MAX + 0.1)) as usize {
            g.director_tick();
        }
        assert_eq!(g.director.tempo, Tempo::BuildUp);
    }

    #[test]
    fn a_long_lull_sends_a_horde_rush() {
        let mut g = game();
        g.wave.n = 4;
        g.wave.total = 200;
        g.wave.spawned = 0;
        g.zombies.clear();
        let mut rushed = false;
        for _ in 0..(120.0 * (RUSH_CALM + 1.0)) as usize {
            if g.director_tick() {
                rushed = true;
                break;
            }
        }
        assert!(rushed, "quiet build-ups get a rush");
        g.horde_rush();
        assert!(g.zombies.len() >= 10 && g.wave.spawned == g.zombies.len() as u32);
    }
}
