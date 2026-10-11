//! Mode-specific save data for Outbreak and Siege, appended to the common
//! save string (`Game::save_string`). Like the horde modes, a save resumes at
//! the start of the current round/wave.
//!
//! Outbreak:
//! - `ob=points,cur,box_at,box_uses,power,self_revives,next_hounds`
//! - `obw=gun:mag:reserve:upgraded/...` (weapons)
//! - `obp=perk,...` (perk indices)
//! - `obd=door,...` (opened door indices)
//! - `obs=parts.../shield/shield_hp/candles.../egg_done` (secrets)
//!
//! Siege:
//! - `sg=mana,crystal_hp`
//! - `sgd=kind:x:y:tier:hp/...` (defenses)
//!
//! Values never contain `;` or `=` (the save string's separators).

use crate::modes::outbreak::{Perk, Weapon};
use crate::modes::siege::{DefKind, Defense, SiegePhase};
use crate::world;

const PERKS: [Perk; 4] = [Perk::IronGut, Perk::Quickdraw, Perk::SecondWind, Perk::Fleetfoot];

fn nums<T: std::str::FromStr>(v: &str, sep: char) -> Vec<T> {
    v.split(sep).filter_map(|x| x.parse().ok()).collect()
}

impl crate::game::Game {
    /// The round/wave a save resumes at (the current one, or the next if between them).
    pub(crate) fn save_wave(&self) -> u32 {
        if let Some(s) = &self.siege {
            // the build phase sits before wave n+1
            return if s.phase == SiegePhase::Build { self.wave.n + 1 } else { self.wave.n.max(1) };
        }
        if self.wave.intermission > 0.0 {
            self.wave.n + 1
        } else {
            self.wave.n.max(1)
        }
    }

    /// Extra `;key=value` pairs for the current mode (empty for the horde modes).
    pub(crate) fn mode_save_fields(&self) -> String {
        let mut out = String::new();
        if let Some(ob) = &self.ob {
            out += &format!(
                ";ob={},{},{},{},{},{},{}",
                ob.points, ob.cur, ob.box_at, ob.box_uses, ob.power as u8, ob.self_revives, ob.next_hounds
            );
            let ws: Vec<String> = ob.weapons.iter().map(|w| format!("{}:{}:{}:{}", w.gun, w.mag, w.reserve, w.upgraded as u8)).collect();
            out += &format!(";obw={}", ws.join("/"));
            let ps: Vec<String> = ob.perks.iter().filter_map(|p| PERKS.iter().position(|x| x == p)).map(|i| i.to_string()).collect();
            out += &format!(";obp={}", ps.join(","));
            if let Some(m) = world::map() {
                let ds: Vec<String> = m.doors.iter().enumerate().filter(|(_, d)| d.open.get()).map(|(i, _)| i.to_string()).collect();
                out += &format!(";obd={}", ds.join(","));
            }
            if let Some(se) = &self.secrets {
                let join = |v: &Vec<usize>| v.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(".");
                out += &format!(";obs={}/{}/{:.0}/{}/{}", join(&se.parts), se.shield as u8, se.shield_hp, join(&se.candles), se.egg_done as u8);
            }
        }
        if let Some(s) = &self.siege {
            out += &format!(";sg={},{:.1}", s.mana, s.crystal_hp);
            let ds: Vec<String> = s.defenses.iter().map(|d| format!("{}:{:.0}:{:.0}:{}:{:.0}", d.kind as u8, d.pos.x, d.pos.y, d.tier, d.hp)).collect();
            out += &format!(";sgd={}", ds.join("/"));
        }
        out
    }

    /// Restore Outbreak/Siege state after `load_save` has started the mode.
    pub(crate) fn restore_mode_save(&mut self, data: &str) {
        for kv in data.split(';') {
            let Some((k, v)) = kv.split_once('=') else { continue };
            match k {
                "ob" => {
                    let n: Vec<u32> = nums(v, ',');
                    if let (Some(ob), true) = (self.ob.as_mut(), n.len() == 7) {
                        ob.points = n[0];
                        ob.cur = n[1] as usize;
                        ob.box_at = n[2] as usize;
                        ob.box_uses = n[3];
                        ob.power = n[4] != 0;
                        ob.self_revives = n[5];
                        ob.next_hounds = n[6];
                    }
                }
                "obw" => {
                    let ws: Vec<Weapon> = v
                        .split('/')
                        .filter_map(|w| {
                            let f: Vec<u32> = nums(w, ':');
                            (f.len() == 4 && (f[0] as usize) < crate::content::GUNS.len()).then(|| Weapon {
                                gun: f[0] as usize,
                                mag: f[1],
                                reserve: f[2],
                                upgraded: f[3] != 0,
                            })
                        })
                        .collect();
                    if let (Some(ob), false) = (self.ob.as_mut(), ws.is_empty()) {
                        ob.weapons = ws;
                        ob.cur = ob.cur.min(ob.weapons.len() - 1);
                        self.gun = ob.weapons[ob.cur].gun;
                    }
                }
                "obp" => {
                    // saved max HP already includes Iron Gut, so perks are restored directly
                    if let Some(ob) = self.ob.as_mut() {
                        ob.perks = nums::<usize>(v, ',').into_iter().filter_map(|i| PERKS.get(i).copied()).collect();
                    }
                }
                "obd" => {
                    if let Some(m) = world::map() {
                        for i in nums::<usize>(v, ',') {
                            m.open_door(i);
                        }
                    }
                    if let Some(f) = self.flow.as_mut() {
                        f.doors_sig = u64::MAX;
                    }
                }
                "obs" => {
                    let f: Vec<&str> = v.split('/').collect();
                    if let (Some(se), 5) = (self.secrets.as_mut(), f.len()) {
                        se.parts = nums(f[0], '.');
                        se.shield = f[1] == "1";
                        se.shield_hp = f[2].parse().unwrap_or(0.0);
                        se.candles = nums(f[3], '.');
                        se.egg_done = f[4] == "1";
                    }
                }
                "sg" => {
                    let f: Vec<&str> = v.split(',').collect();
                    if let (Some(s), 2) = (self.siege.as_mut(), f.len()) {
                        s.mana = f[0].parse().unwrap_or(s.mana);
                        s.crystal_hp = f[1].parse::<f32>().unwrap_or(s.crystal_hp).clamp(1.0, s.crystal_max);
                    }
                }
                "sgd" => {
                    let Some(s) = self.siege.as_mut() else { continue };
                    for d in v.split('/') {
                        let f: Vec<f32> = nums(d, ':');
                        let Some(kind) = (f.len() == 5).then(|| DefKind::from_u8(f[0] as u8)).flatten() else { continue };
                        let tier = (f[3] as u8).clamp(1, 3);
                        let max_hp = kind.base_hp() * (1.0 + 0.6 * (tier as f32 - 1.0));
                        let uid = s.next_uid;
                        s.next_uid += 1;
                        s.defenses.push(Defense {
                            kind,
                            pos: crate::math::v2(f[1], f[2]),
                            tier,
                            hp: f[4].clamp(1.0, max_hp),
                            max_hp,
                            cd: 0.5,
                            aim: 0.0,
                            flash: 0.0,
                            uid,
                        });
                    }
                    s.phase = SiegePhase::Build;
                    s.build_t = 30.0;
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::game::{Game, Phase};
    use crate::math::v2;
    use crate::modes::siege::DefKind;

    #[test]
    fn outbreak_saves_points_guns_perks_doors_and_resumes_the_round() {
        let mut g = Game::new(1);
        g.start("mode=outbreak;map=refinery;diff=veteran");
        g.phase = Phase::Playing;
        g.wave.n = 7;
        g.wave.intermission = 0.0;
        {
            let ob = g.ob.as_mut().unwrap();
            ob.points = 4321;
            ob.power = true;
            ob.weapons.push(crate::modes::outbreak::Weapon::new(crate::content::gun_index("smg")));
            ob.weapons[1].upgraded = true;
            ob.weapons[1].mag = 7;
            ob.cur = 1;
        }
        g.give_perk(crate::modes::outbreak::Perk::IronGut);
        g.give_perk(crate::modes::outbreak::Perk::Fleetfoot);
        world_open_first_door();
        let s = g.save_string();
        assert!(s.contains("map=refinery") && s.contains("mode=outbreak"));
        let mut h = Game::new(2);
        h.load_save(&s);
        let ob = h.ob.as_ref().expect("outbreak restored");
        assert_eq!(h.map_id, "refinery");
        assert_eq!((ob.points, ob.power, ob.weapons.len(), ob.cur), (4321, true, 2, 1));
        assert!(ob.weapons[1].upgraded && ob.weapons[1].mag == 7);
        assert_eq!(ob.perks.len(), 2);
        assert!(h.player.max_hp >= 250.0, "Iron Gut's max HP came back once, not twice: {}", h.player.max_hp);
        assert!(h.player.max_hp < 400.0);
        assert_eq!(h.wave.n, 6, "resumes at the start of round 7");
        assert!(crate::world::map().unwrap().doors[0].open.get());
    }

    fn world_open_first_door() {
        crate::world::map().unwrap().open_door(0);
    }

    #[test]
    fn siege_saves_map_crystal_mana_and_defenses() {
        let mut g = Game::new(3);
        g.start("mode=siege;map=bridge;variant=glass;diff=rookie");
        g.phase = Phase::Playing;
        g.wave.n = 4;
        let at = g.siege.as_ref().unwrap().crystal + v2(120.0, 0.0);
        g.add_defense(DefKind::Tower, at, 3);
        g.add_defense(DefKind::Frost, at + v2(0.0, 120.0), 1);
        {
            let s = g.siege.as_mut().unwrap();
            s.mana = 333;
            s.crystal_hp = s.crystal_max * 0.5;
        }
        let saved_hp = g.siege.as_ref().unwrap().crystal_hp;
        let s = g.save_string();
        let mut h = Game::new(4);
        h.load_save(&s);
        let sg = h.siege.as_ref().expect("siege restored");
        assert_eq!(h.map_id, "bridge");
        assert!(sg.glass, "Glass Crystal variant kept");
        assert_eq!(sg.mana, 333);
        assert!((sg.crystal_hp - saved_hp).abs() < 1.0);
        assert_eq!(sg.defenses.len(), 2);
        assert_eq!(sg.defenses.iter().find(|d| d.kind == DefKind::Tower).unwrap().tier, 3);
        // build phase before wave 5 (wave 4 was cleared: build phase saves resume at n+1)
        assert_eq!(sg.phase, crate::modes::siege::SiegePhase::Build);
        assert_eq!(h.wave.n, 4);
    }
}
