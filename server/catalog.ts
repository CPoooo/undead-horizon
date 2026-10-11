// Server-authoritative prices and reward rules.
// Keep in sync with ui/app.js (CATALOG) and src/content.rs / src/game.rs (coin formulas).

export const CATEGORIES = ["gun", "perk", "skin", "hat", "tracer", "char"];

export const PRICES: Record<string, number> = {
  "gun:pistol": 0, "gun:smg": 400, "gun:shotgun": 650, "gun:rifle": 900, "gun:minigun": 1500, "gun:launcher": 2000,
  "perk:none": 0, "perk:scavenger": 200, "perk:medic": 250, "perk:sprinter": 250, "perk:gunsmith": 400,
  "perk:lucky": 400, "perk:blademaster": 1200, "perk:stormcaller": 1200,
  "skin:default": 0, "skin:crimson": 150, "skin:camo": 200, "skin:shadow": 300, "skin:royal": 400,
  "skin:neon": 500, "skin:hazmat": 600, "skin:gold": 1500,
  "hat:none": 0, "hat:cap": 100, "hat:headband": 150, "hat:cowboy": 250, "hat:tophat": 350,
  "hat:horns": 500, "hat:crown": 1200, "hat:halo": 2000,
  "tracer:default": 0, "tracer:fire": 200, "tracer:ice": 200, "tracer:toxic": 250, "tracer:plasma": 400,
  "tracer:rainbow": 1000,
  // survivors are free (gating comes from achievements)
  "char:rook": 0, "char:mara": 0, "char:doc": 0, "char:ash": 0,
};

export const DIFFS = ["rookie", "survivor", "veteran", "nightmare"];
export const DIFF_COINS: Record<string, number> = { rookie: 0.75, survivor: 1, veteran: 1.5, nightmare: 2.25 };

/** waves per chapter, the infinite-wave level the chapter starts at, and boss count */
export const CHAPTERS = [
  { waves: 3, base: 1, bosses: 1 },
  { waves: 4, base: 4, bosses: 1 },
  { waves: 4, base: 8, bosses: 1 },
  { waves: 5, base: 12, bosses: 2 },
  { waves: 6, base: 16, bosses: 1 },
];

/** Upper bound on coins a run with this progress could have earned (mirrors game.rs). */
export function waveCoins(r: { mode: string; chapter: number; diff: string; wave: number; victory: boolean }): number {
  const m = DIFF_COINS[r.diff] ?? 1;
  let total = 0;
  // treasure chests (bosses, elites, brutes) add coins on top of wave rewards
  total += 65 * m * (Math.floor(r.wave / 5) + Math.ceil(r.wave / 2) + 1);
  if (r.mode === "campaign") {
    const ch = CHAPTERS[r.chapter];
    const cleared = Math.min(r.wave, ch.waves);
    for (let n = 1; n <= cleared; n++) total += (10 + 5 * (ch.base + n - 1)) * m;
    if (r.wave >= ch.waves) total += 50 * m * ch.bosses;
    if (r.victory) total += 100 * (r.chapter + 1) * m;
  } else {
    // the current wave may already be cleared (intermission), so count it
    for (let n = 1; n <= r.wave; n++) total += (10 + 5 * n) * m;
    total += Math.floor(r.wave / 5) * 50 * m;
  }
  return Math.ceil(total) + 5;
}
