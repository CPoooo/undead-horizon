//! Kill feed, multi-kill callouts, crit-kill "headshots" and the score, which
//! grows faster the longer you keep a kill streak alive.

use crate::audio::Sfx;
use crate::game::{Game, ZKind};
use crate::math::V2;

pub struct FeedItem {
    pub text: String,
    pub color: &'static str,
    pub life: f32,
}

/// A big centered callout ("TRIPLE KILL").
pub struct Callout {
    pub text: &'static str,
    pub color: &'static str,
    pub t: f32,
}

/// Kills inside this window chain into a multi-kill.
const MULTI_WINDOW: f32 = 0.6;

const TIERS: [(u32, &str, &str); 6] = [
    (3, "TRIPLE KILL", "#ffd34a"),
    (6, "RAMPAGE", "#ffb03a"),
    (10, "MASSACRE", "#ff7a3a"),
    (15, "CARNAGE", "#ff4a5a"),
    (25, "ANNIHILATION", "#ff4df0"),
    (40, "APOCALYPSE", "#b48cff"),
];

impl Game {
    /// Streak multiplier applied to kill points: x1 up to x6 at a 100 streak.
    pub fn score_mult(&self) -> f32 {
        1.0 + self.combo.min(100) as f32 / 20.0
    }

    pub fn slot_name(&self, slot: u8) -> String {
        if slot == self.my_slot() {
            return self.player_name.clone();
        }
        self.net
            .as_ref()
            .and_then(|n| n.remotes.get(slot as usize))
            .map(|r| r.name.clone())
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| "Teammate".into())
    }

    pub fn feed(&mut self, text: String, color: &'static str) {
        if self.feed.len() >= 5 {
            self.feed.remove(0);
        }
        self.feed.push(FeedItem {
            text,
            color,
            life: 5.0,
        });
    }

    /// Every kill, on every peer: `killer` is the slot that got it (255 = none).
    pub fn note_kill(&mut self, killer: u8, kind: ZKind, _pos: V2) {
        let mine = killer == self.my_slot();
        if mine {
            if let Some(k) = self.kind_kills.get_mut(kind as usize) {
                *k += 1;
            }
            if kind == ZKind::Boss {
                self.bosses_killed += 1;
            }
            self.kill_fx(_pos);
            let pts = (kind.def().xp.max(1) * 10) as f32 * self.score_mult();
            self.score += pts as u64;
            self.multi_n += 1;
            self.multi_t = MULTI_WINDOW;
            if let Some(&(_, text, color)) = TIERS.iter().rev().find(|t| t.0 == self.multi_n) {
                self.callout = Some(Callout { text, color, t: 0.0 });
                self.score += (self.multi_n * 25) as u64;
                self.audio.play(Sfx::Stinger);
                self.say(&text.to_lowercase());
            }
        }
        if killer == 255 {
            return;
        }
        let notable = match kind {
            ZKind::Boss => Some("#ff9cff"),
            ZKind::Brute => Some("#c8d2e8"),
            _ => None,
        };
        if let Some(col) = notable {
            let who = self.slot_name(killer);
            let what = if kind == ZKind::Boss {
                self.zombies
                    .iter()
                    .find(|z| z.kind == ZKind::Boss)
                    .map(|z| z.boss_var.name())
                    .unwrap_or("the boss")
                    .to_string()
            } else {
                kind.def().name.to_string()
            };
            // solo: only bosses are worth a line
            if self.net.is_some() || kind == ZKind::Boss {
                self.feed(format!("{} ✖ {}", who, what), col);
            }
        }
    }

    /// A crit landed the killing blow on one of our own targets.
    pub fn headshot(&mut self, pos: V2) {
        self.score += 15;
        self.push_text(pos + V2 { x: 0.0, y: -18.0 }, "HEADSHOT".into(), "#ffcf3a", 15.0);
    }

    pub fn tick_callouts(&mut self, dt: f32) {
        if self.multi_t > 0.0 {
            self.multi_t -= dt;
            if self.multi_t <= 0.0 {
                self.multi_n = 0;
            }
        }
        if let Some(c) = &mut self.callout {
            c.t += dt;
            if c.t > 1.4 {
                self.callout = None;
            }
        }
        for f in self.feed.iter_mut() {
            f.life -= dt;
        }
        self.feed.retain(|f| f.life > 0.0);
    }
}
