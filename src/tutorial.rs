//! First-run tutorial: a short checklist on the player's first Infinite run.
//! Each step completes from what the player actually does.

use crate::game::{Game, Phase};
use crate::math::*;

pub struct Tutorial {
    pub step: usize,
    pub t: f32,
    pub start_pos: V2,
    pub start_kills: u32,
    pub start_level: u32,
    pub dashes: u32,
}

pub const STEPS: [(&str, &str); 5] = [
    ("Move", "WASD / left stick / drag on the left side"),
    ("Dash", "Space / A / tap the right side: you're invulnerable mid-dash"),
    ("Fight", "Your gun fires on its own. Aim with the mouse or right stick. Kill 5 zombies"),
    ("Level up", "Grab the blue gems, then pick a power"),
    ("Ultimate", "When the ☢️ ring fills, press R / RB / tap ☢️"),
];

impl Game {
    pub fn start_tutorial(&mut self) {
        self.tutorial = Some(Tutorial {
            step: 0,
            t: 0.0,
            start_pos: self.player.pos,
            start_kills: self.kills,
            start_level: self.player.level,
            dashes: self.player.dash_id,
        });
    }

    pub fn tick_tutorial(&mut self, dt: f32) {
        if self.phase == Phase::Paused {
            return;
        }
        let Some(tu) = self.tutorial.as_mut() else { return };
        tu.t += dt;
        let done = match tu.step {
            0 => self.player.pos.dist(tu.start_pos) > 260.0,
            1 => self.player.dash_id > tu.dashes,
            2 => self.kills >= tu.start_kills + 5,
            3 => self.player.level > tu.start_level && self.phase == Phase::Playing,
            4 => self.ult_uses > 0 || tu.t > 90.0,
            _ => false,
        };
        if done {
            tu.step += 1;
            tu.t = 0.0;
            self.audio.play(crate::audio::Sfx::UiSelect);
            if tu.step >= STEPS.len() {
                self.tutorial = None;
                self.toast = Some(("Tutorial complete. Good luck out there, courier!".into(), 3.0));
                self.ui_out.push(("tutorial", "done".into()));
            } else if tu.step == 4 {
                // let them feel the ultimate
                self.player.ult = 1.0;
            }
        }
    }
}
