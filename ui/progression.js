// Career progression shared by the browser (window.UHProg) and the API server
// (bundled by esbuild): account levels, achievements, weapon mastery, the
// bestiary, permanent upgrades (meta) and relics. Pure functions over a
// plain `progress` object so both sides compute identical results.
(function (root, factory) {
  if (typeof module === 'object' && module.exports) module.exports = factory();
  else root.UHProg = factory();
})(typeof self !== 'undefined' ? self : this, function () {
  'use strict';

  // enemy kinds in the order of src/game.rs ZKind (stats JSON "kk")
  const KINDS = ['Walker', 'Runner', 'Brute', 'Spitter', 'Exploder', 'Boss', 'Crawler', 'Screamer', 'Armored', 'Bat', 'Burrower', 'Hellhound', 'Sapper', 'Ogre', 'Archer'];
  const BESTIARY = {
    Walker: 'The first to rise. Slow, relentless, everywhere.',
    Runner: 'Fresh turns that still remember how to sprint.',
    Brute: 'Bloated muscle. Shrugs off bullets, flattens fences.',
    Spitter: 'Its throat sac fills with Bloom acid. It aims for you.',
    Exploder: 'Spore-cooked until volatile. Do not let it hug you.',
    Boss: 'Something the Bloom grew on purpose.',
    Crawler: 'Low, fast and hard to hit. Watch your ankles.',
    Screamer: 'Its shriek drives the horde into a frenzy.',
    Armored: 'Riot gear fused to flesh. Flank it.',
    Bat: 'Spores took to the air. They come in flocks.',
    Burrower: 'Tunnels under the yard and comes up beneath you.',
    Hellhound: 'Lightning brings them. Fire follows.',
    Sapper: 'Carries a bomb and hates your towers more than you.',
    Ogre: 'Walls are suggestions.',
    Archer: 'It learned to throw. Then it learned to aim.',
  };

  // ------------------------------------------------------------ account level
  const xpToNext = (lvl) => Math.round(400 + 120 * lvl + 18 * lvl * lvl);
  function levelFromXp(xp) {
    let lvl = 1;
    let need = xpToNext(1);
    let rest = xp;
    while (rest >= need && lvl < 200) {
      rest -= need;
      lvl += 1;
      need = xpToNext(lvl);
    }
    return { level: lvl, into: rest, need };
  }
  function xpForRun(r) {
    const mult = { rookie: 0.8, survivor: 1, veteran: 1.3, nightmare: 1.7 }[r.diff] || 1;
    return Math.round((Math.min(r.kills || 0, 20000) + 25 * Math.min(r.wave || 1, 500) + 100 * Math.min(r.bosses || 0, 50) + (r.victory ? 300 : 0)) * mult);
  }
  /** coins and unlocks granted for reaching `level` */
  function levelReward(level) {
    const items = { 3: 'hat:cap', 5: 'tracer:ice', 8: 'skin:camo', 12: 'hat:headband', 15: 'tracer:toxic', 20: 'skin:neon', 30: 'hat:crown' };
    return { coins: 30 + 10 * level, item: items[level] || null };
  }

  // ------------------------------------------------------------ achievements
  // test(run, life): run = stats JSON of the run just finished, life = lifetime stats after it
  const A = (id, name, icon, desc, test, hidden) => ({ id, name, icon, desc, test, hidden: !!hidden });
  const kk = (r, k) => (r.kk || [])[KINDS.indexOf(k)] || 0;
  const ACHIEVEMENTS = [
    A('first_blood', 'First Blood', '🩸', 'Kill your first zombie', (r, l) => l.kills >= 1),
    A('kills_100', 'Pest Control', '🧹', '100 lifetime kills', (r, l) => l.kills >= 100),
    A('kills_1000', 'Exterminator', '☠️', '1,000 lifetime kills', (r, l) => l.kills >= 1000),
    A('kills_10000', 'Extinction Event', '🌋', '10,000 lifetime kills', (r, l) => l.kills >= 10000),
    A('kills_50000', 'Horizon Legend', '👑', '50,000 lifetime kills', (r, l) => l.kills >= 50000),
    A('wave_5', 'Still Breathing', '🫁', 'Reach wave 5 in Infinite', (r) => r.mode === 'infinite' && r.wave >= 5),
    A('wave_10', 'Seasoned', '🎖️', 'Reach wave 10 in Infinite', (r) => r.mode === 'infinite' && r.wave >= 10),
    A('wave_20', 'Veteran Survivor', '🏅', 'Reach wave 20 in Infinite', (r) => r.mode === 'infinite' && r.wave >= 20),
    A('wave_30', 'Unkillable', '💎', 'Reach wave 30 in Infinite', (r) => r.mode === 'infinite' && r.wave >= 30),
    A('wave_50', 'Myth', '🌌', 'Reach wave 50 in Infinite', (r) => r.mode === 'infinite' && r.wave >= 50),
    A('boss_1', 'Giant Slayer', '⚔️', 'Kill a boss', (r, l) => l.bosses >= 1),
    A('boss_10', 'Boss Hunter', '🗡️', 'Kill 10 bosses', (r, l) => l.bosses >= 10),
    A('boss_50', 'Apex', '🦴', 'Kill 50 bosses', (r, l) => l.bosses >= 50),
    A('runs_10', 'Regular', '🔁', 'Play 10 runs', (r, l) => l.runs >= 10),
    A('runs_100', 'Lifer', '♾️', 'Play 100 runs', (r, l) => l.runs >= 100),
    A('combo_50', 'Combo Starter', '🔥', 'A 50-kill streak', (r) => r.combo >= 50),
    A('combo_200', 'Unstoppable', '⚡', 'A 200-kill streak', (r) => r.combo >= 200),
    A('combo_500', 'Carnage Engine', '💥', 'A 500-kill streak', (r) => r.combo >= 500),
    A('level_20', 'Overpowered', '📈', 'Reach level 20 in one run', (r) => r.level >= 20),
    A('level_40', 'Ascended', '🚀', 'Reach level 40 in one run', (r) => r.level >= 40),
    A('evolve_1', 'Evolution', '🧬', 'Evolve a power', (r) => r.evos >= 1),
    A('evolve_3', 'Apex Predator', '🦖', 'Evolve 3 powers in one run', (r) => r.evos >= 3),
    A('chest_5', 'Treasure Hunter', '🎁', 'Open 5 chests in one run', (r) => r.chests >= 5),
    A('score_100k', 'Six Figures', '💯', 'Score 100,000 in one run', (r) => r.score >= 100000),
    A('score_1m', 'Millionaire', '💰', 'Score 1,000,000 in one run', (r) => r.score >= 1000000),
    A('nightmare_5', 'Bad Dreams', '😱', 'Reach wave 5 on Nightmare', (r) => r.diff === 'nightmare' && r.wave >= 5),
    A('nightmare_15', 'Living Nightmare', '👹', 'Reach wave 15 on Nightmare', (r) => r.diff === 'nightmare' && r.wave >= 15),
    A('campaign_1', 'The Long Night', '🌙', 'Clear chapter 1', (r) => r.mode === 'campaign' && r.victory && r.chapter >= 0),
    A('campaign_3', 'Ashfall', '🏭', 'Clear chapter 3', (r) => r.mode === 'campaign' && r.victory && r.chapter >= 2),
    A('campaign_5', 'Horizon', '🌅', 'Finish the campaign', (r) => r.mode === 'campaign' && r.victory && r.chapter >= 4),
    A('outbreak_5', 'Boarded Up', '🪵', 'Reach round 5 in Outbreak', (r) => r.mode === 'outbreak' && r.wave >= 5),
    A('outbreak_15', 'Asylum Resident', '🏚️', 'Reach round 15 in Outbreak', (r) => r.mode === 'outbreak' && r.wave >= 15),
    A('outbreak_30', 'Round Thirty', '🧟', 'Reach round 30 in Outbreak', (r) => r.mode === 'outbreak' && r.wave >= 30),
    A('siege_win', 'Keeper', '🏰', 'Win a Siege', (r) => r.mode === 'siege' && r.victory),
    A('siege_3star', 'Flawless Defense', '⭐', 'Win a Siege with 3 stars', (r) => r.mode === 'siege' && r.victory && r.stars >= 3),
    A('coop_1', 'Squad Up', '👥', 'Finish a co-op run', (r) => r.players > 1),
    A('coop_wave10', 'Brothers in Arms', '🤝', 'Reach wave 10 in co-op', (r) => r.players > 1 && r.wave >= 10),
    A('kill_brutes', 'Brute Force', '🦍', 'Kill 100 brutes', (r, l) => (l.kinds?.Brute || 0) >= 100),
    A('kill_exploders', 'Defuser', '🧨', 'Kill 200 exploders', (r, l) => (l.kinds?.Exploder || 0) >= 200),
    A('kill_bats', 'Bat Swatter', '🦇', 'Kill 300 bats', (r, l) => (l.kinds?.Bat || 0) >= 300),
    A('kill_hounds', 'Dog Catcher', '🐕', 'Kill 100 hellhounds', (r, l) => (l.kinds?.Hellhound || 0) >= 100),
    A('kill_ogres', 'Ogre Toppler', '🗿', 'Kill 25 ogres', (r, l) => (l.kinds?.Ogre || 0) >= 25),
    A('bestiary', 'Naturalist', '📚', 'Discover every enemy', (r, l) => KINDS.every((k) => (l.kinds?.[k] || 0) > 0)),
    A('mastery_1', 'Gun Nut', '🔫', 'Master a weapon (1,500 kills)', (r, l) => Object.values(l.gunKills || {}).some((n) => n >= 1500)),
    A('all_guns', 'Arsenal', '🧰', 'Get 100 kills with 6 different guns', (r, l) => Object.values(l.gunKills || {}).filter((n) => n >= 100).length >= 6),
    A('pacifist', 'Conscientious Objector', '🕊️', 'Survive to wave 3 with under 10 kills', (r) => r.mode === 'infinite' && r.wave >= 3 && r.kills < 10, true),
    A('speedrun', 'In a Hurry', '⏱️', 'Reach wave 10 in under 6 minutes', (r) => r.wave >= 10 && r.time < 360, true),
    A('asylum_egg', 'The Bloom Stirs', '🕯️', 'Complete the Chapel ritual', () => false, true),
    A('pure_20', 'Purist', '🧼', 'Reach wave 20 with no upgrades or relics', (r) => r.mode === 'infinite' && r.wave >= 20 && r.pure, true),
    A('relic_epic', 'Relic Hunter', '🏺', 'Find a legendary relic', () => false),
    A('level_10', 'Career Survivor', '🎓', 'Reach account level 10', (r, l) => levelFromXp(l.xp || 0).level >= 10),
    A('level_25', 'Lifer Elite', '🏆', 'Reach account level 25', (r, l) => levelFromXp(l.xp || 0).level >= 25),
  ];
  const EVENT_ACHIEVEMENTS = ['asylum_egg', 'relic_epic'];

  // characters unlock through achievements (C8c)
  const CHAR_UNLOCK = { rook: null, mara: 'wave_10', doc: 'boss_1', ash: 'kills_1000' };
  const charUnlocked = (id, progress) => !CHAR_UNLOCK[id] || (progress.achievements || []).includes(CHAR_UNLOCK[id]);

  // ------------------------------------------------------------ weapon mastery
  const CAMO_AT = [100, 500, 1500];
  const camoTier = (kills) => CAMO_AT.filter((n) => (kills || 0) >= n).length;

  // ------------------------------------------------------------ meta upgrades
  const META = [
    { id: 'dmg', name: 'Sharpened Rounds', icon: '💥', desc: '+2% damage per rank', max: 5, cost: 300 },
    { id: 'hp', name: 'Thick Skin', icon: '❤️', desc: '+5 max health per rank', max: 5, cost: 250 },
    { id: 'spd', name: 'Light Boots', icon: '👟', desc: '+2% move speed per rank', max: 5, cost: 300 },
    { id: 'magnet', name: 'Soul Pull', icon: '🧲', desc: '+6% pickup radius per rank', max: 5, cost: 200 },
    { id: 'reroll', name: 'Lucky Dice', icon: '🎲', desc: '+1 level-up reroll per rank', max: 3, cost: 800 },
    { id: 'banish', name: 'Exile', icon: '🚫', desc: '+1 banish per rank', max: 3, cost: 600 },
  ];
  const metaCost = (id, rank) => (META.find((m) => m.id === id)?.cost || 0) * (rank + 1);
  const metaSpec = (meta) => Object.entries(meta || {}).filter(([, v]) => v > 0).map(([k, v]) => `${k}:${v}`).join(',');

  // ------------------------------------------------------------ relics
  const RELIC_BASES = [
    { id: 'ember', name: 'Ember Idol', icon: '🔥', stat: 'damage' },
    { id: 'locket', name: 'Iron Locket', icon: '📿', stat: 'max health' },
    { id: 'charm', name: 'Quickdraw Charm', icon: '🪬', stat: 'fire rate' },
    { id: 'dice', name: 'Bone Dice', icon: '🎲', stat: 'crit chance' },
    { id: 'sigil', name: 'Wind Sigil', icon: '🌀', stat: 'move speed' },
    { id: 'lodestone', name: 'Lodestone', icon: '🧲', stat: 'pickup radius' },
  ];
  const AFFIXES = ['dmg', 'hp', 'rate', 'crit', 'speed', 'magnet'];
  const RARITY = ['', 'Common', 'Rare', 'Legendary'];
  function rollRelic(rand) {
    const base = RELIC_BASES[Math.floor(rand() * RELIC_BASES.length)];
    const r = rand();
    const rarity = r < 0.08 ? 3 : r < 0.35 ? 2 : 1;
    const affixes = [];
    for (let i = 0; i < rarity - 1; i++) affixes.push(AFFIXES[Math.floor(rand() * AFFIXES.length)]);
    return { uid: Math.floor(rand() * 1e9).toString(36), id: base.id, rarity, affixes };
  }
  const relicSpec = (r) => [r.id, r.rarity, ...(r.affixes || [])].join(':');
  function relicLabel(r) {
    const base = RELIC_BASES.find((b) => b.id === r.id) || RELIC_BASES[0];
    const main = `+${4 * r.rarity}% ${base.stat}`;
    const extra = (r.affixes || []).map((a) => `+${1.5 * r.rarity}% ${a}`).join(', ');
    return { name: `${RARITY[r.rarity]} ${base.name}`, icon: base.icon, desc: extra ? `${main}, ${extra}` : main };
  }

  // ------------------------------------------------------------ apply a run
  function fresh() {
    return { xp: 0, achievements: [], kills: 0, bosses: 0, runs: 0, kinds: {}, gunKills: {}, meta: {}, relics: [], equippedRelics: [] };
  }
  function normalize(p) {
    const f = fresh();
    p = Object.assign(f, p || {});
    p.kinds = p.kinds || {};
    p.gunKills = p.gunKills || {};
    p.meta = p.meta || {};
    p.relics = Array.isArray(p.relics) ? p.relics.slice(0, 40) : [];
    p.equippedRelics = (p.equippedRelics || []).filter((u) => p.relics.some((r) => r.uid === u)).slice(0, 3);
    p.achievements = Array.isArray(p.achievements) ? p.achievements : [];
    return p;
  }
  /**
   * Fold a finished run into progress. Returns what changed so the UI can celebrate:
   * { progress, xp, levelsGained:[n], coins, items:[], achievements:[], relics:[] }
   */
  function applyRun(progress, run, rand = Math.random) {
    const p = normalize(JSON.parse(JSON.stringify(progress || {})));
    const before = levelFromXp(p.xp).level;
    const xp = xpForRun(run);
    p.xp += xp;
    p.runs += 1;
    p.kills += Math.max(0, Math.min(run.kills || 0, 100000));
    p.bosses += Math.max(0, Math.min(run.bosses || 0, 100));
    (run.kk || []).forEach((n, i) => {
      if (KINDS[i] && n > 0) p.kinds[KINDS[i]] = (p.kinds[KINDS[i]] || 0) + Math.min(n, 100000);
    });
    if (run.gun) p.gunKills[run.gun] = (p.gunKills[run.gun] || 0) + Math.max(0, Math.min(run.kills || 0, 100000));
    const after = levelFromXp(p.xp).level;
    const levelsGained = [];
    let coins = 0;
    const items = [];
    for (let l = before + 1; l <= after; l++) {
      levelsGained.push(l);
      const rw = levelReward(l);
      coins += rw.coins;
      if (rw.item) items.push(rw.item);
    }
    // relics drop from bosses
    const relics = [];
    for (let i = 0; i < Math.min(run.bosses || 0, 5); i++) {
      if (rand() < 0.6 && p.relics.length < 40) {
        const r = rollRelic(rand);
        p.relics.push(r);
        relics.push(r);
        if (r.rarity === 3 && !p.achievements.includes('relic_epic')) p.achievements.push('relic_epic');
      }
    }
    const achievements = [];
    for (const a of ACHIEVEMENTS) {
      if (!p.achievements.includes(a.id) && a.test(run, p)) {
        p.achievements.push(a.id);
        achievements.push(a.id);
      }
    }
    if (relics.some((r) => r.rarity === 3)) achievements.push('relic_epic');
    return { progress: p, xp, levelsGained, coins, items, achievements, relics };
  }

  return {
    KINDS, BESTIARY, ACHIEVEMENTS, EVENT_ACHIEVEMENTS, CHAR_UNLOCK, CAMO_AT, META, RELIC_BASES,
    xpToNext, levelFromXp, xpForRun, levelReward, charUnlocked, camoTier, metaCost, metaSpec,
    rollRelic, relicSpec, relicLabel, fresh, normalize, applyRun,
  };
});
