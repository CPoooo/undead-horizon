// Undead Horizon UI layer: menus, shop, locker, campaign story, pause/save,
// leaderboard. The game itself (Rust/WASM) talks to this file through
// window.uhPoll (commands JS -> Rust) and window.uhEvent (events Rust -> JS).
(() => {
  'use strict';
  const $ = (id) => document.getElementById(id);
  // Bind without throwing if the element is missing (e.g. a stale cached page),
  // so one mismatch can't take down the whole UI and the game bridge.
  const on = (id, ev, fn, opts) => {
    const el = $(id);
    if (el) el.addEventListener(ev, fn, opts);
    else console.warn(`[ui] #${id} not found`);
  };
  const esc = (s) => String(s).replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));
  const fmtTime = (t) => `${Math.floor(t / 60)}:${String(Math.floor(t % 60)).padStart(2, '0')}`;

  // ------------------------------------------------------------------ catalog
  // ids must match src/content.rs
  const CATALOG = {
    gun: [
      { id: 'pistol', name: 'M9 Sidearm', icon: '🔫', price: 0, desc: 'Reliable all-rounder.' },
      { id: 'smg', name: 'Viper SMG', icon: '🐍', price: 400, desc: 'Twice the fire rate, lighter hits, a little spray.' },
      { id: 'shotgun', name: 'Boomstick', icon: '💥', price: 650, desc: '6-pellet blast with huge knockback. Get close.' },
      { id: 'rifle', name: 'Marksman Rifle', icon: '🎯', price: 900, desc: 'Slow, devastating shots that pierce 2 zombies.' },
      { id: 'minigun', name: 'Shredder Minigun', icon: '🌪️', price: 1500, desc: 'A wall of lead. You move a bit slower.' },
      { id: 'launcher', name: 'Thumper Launcher', icon: '🧨', price: 2000, desc: 'Explosive grenades that splash whole hordes.' },
    ],
    perk: [
      { id: 'none', name: 'No perk', icon: '➖', price: 0, desc: 'Start with nothing extra.' },
      { id: 'scavenger', name: 'Scavenger', icon: '🧲', price: 200, desc: 'Start with Soul Magnet Lv1.' },
      { id: 'medic', name: 'Field Medic', icon: '❤️', price: 250, desc: 'Start with Iron Heart Lv1 (+25 HP).' },
      { id: 'sprinter', name: 'Sprinter', icon: '👟', price: 250, desc: 'Start with Swift Feet Lv1.' },
      { id: 'gunsmith', name: 'Gunsmith', icon: '🛠️', price: 400, desc: 'Start with Hollow Points Lv1.' },
      { id: 'lucky', name: 'Lucky Charm', icon: '🍀', price: 400, desc: 'Start with Deadeye Lv1.' },
      { id: 'blademaster', name: 'Blademaster', icon: '🌀', price: 1200, rarity: 'epic', desc: 'Start with Orbiting Blades.' },
      { id: 'stormcaller', name: 'Stormcaller', icon: '🌩️', price: 1200, rarity: 'epic', desc: 'Start with Chain Lightning.' },
    ],
    skin: [
      { id: 'default', name: 'Courier Blue', color: '#3b7bea', price: 0 },
      { id: 'crimson', name: 'Crimson', color: '#d63a4a', price: 150 },
      { id: 'camo', name: 'Woodland Camo', color: '#5f7f3c', price: 200 },
      { id: 'shadow', name: 'Shadow Ops', color: '#34343f', price: 300 },
      { id: 'royal', name: 'Royal Violet', color: '#7a4ad8', price: 400 },
      { id: 'neon', name: 'Neon Tide', color: '#25e8c8', price: 500 },
      { id: 'hazmat', name: 'Hazmat', color: '#f0d838', price: 600 },
      { id: 'gold', name: '24K Gold', color: '#ffd04a', price: 1500, rarity: 'epic' },
    ],
    hat: [
      { id: 'none', name: 'Bare head', icon: '🙂', price: 0 },
      { id: 'cap', name: 'Ball Cap', icon: '🧢', price: 100 },
      { id: 'headband', name: 'Headband', icon: '🎽', price: 150 },
      { id: 'cowboy', name: 'Stetson', icon: '🤠', price: 250 },
      { id: 'tophat', name: 'Top Hat', icon: '🎩', price: 350 },
      { id: 'horns', name: 'Viking Helm', icon: '🪖', price: 500 },
      { id: 'crown', name: 'Crown', icon: '👑', price: 1200, rarity: 'epic' },
      { id: 'halo', name: 'Halo', icon: '😇', price: 2000, rarity: 'epic' },
    ],
    tracer: [
      { id: 'default', name: 'Brass', color: '#ffbe5a', price: 0 },
      { id: 'fire', name: 'Inferno', color: '#ff4a12', price: 200 },
      { id: 'ice', name: 'Cryo', color: '#4ac8ff', price: 200 },
      { id: 'toxic', name: 'Toxic', color: '#62ff2a', price: 250 },
      { id: 'plasma', name: 'Plasma', color: '#c04aff', price: 400 },
      { id: 'rainbow', name: 'Prism', color: 'conic-gradient(#f44,#fb4,#ff4,#4f7,#4cf,#c4f,#f44)', price: 1000, rarity: 'epic' },
    ],
  };
  // V4 cosmetic slots (ids must match content::PACKS / KILLFX / DEATHFX / EMOTES)
  CATALOG.pack = [
    { id: 'pack', name: 'Courier Pack', icon: '🎒', price: 0 },
    { id: 'jet', name: 'Jetpack', icon: '🚀', price: 900, desc: 'Flames when you run. Purely for style.' },
    { id: 'shell', name: 'Turtle Shell', icon: '🐢', price: 400 },
    { id: 'wings', name: 'Bat Wings', icon: '🦇', price: 1200, rarity: 'epic' },
    { id: 'none', name: 'Travel Light', icon: '🚫', price: 50 },
  ];
  CATALOG.killfx = [
    { id: 'default', name: 'Classic Gore', icon: '🩸', price: 0 },
    { id: 'confetti', name: 'Party Time', icon: '🎉', price: 600 },
    { id: 'flames', name: 'Hellfire', icon: '🔥', price: 500 },
    { id: 'sparkle', name: 'Stardust', icon: '✨', price: 500 },
    { id: 'ghost', name: 'Spirit Release', icon: '👻', price: 800, rarity: 'epic' },
  ];
  CATALOG.deathfx = [
    { id: 'default', name: 'Messy', icon: '🩸', price: 0 },
    { id: 'ghost', name: 'Ascension', icon: '👻', price: 500 },
    { id: 'fireworks', name: 'Going Out With a Bang', icon: '🎆', price: 900 },
    { id: 'kaboom', name: 'Kaboom', icon: '💥', price: 700 },
  ];
  CATALOG.emote = [
    { id: 'wave', name: 'Wave', icon: '👋', price: 0 },
    { id: 'laugh', name: 'LOL', icon: '😂', price: 150 },
    { id: 'skull', name: 'Dead Inside', icon: '💀', price: 200 },
    { id: 'flex', name: 'Flex', icon: '💪', price: 250 },
    { id: 'heart', name: 'Wholesome', icon: '❤️', price: 150 },
    { id: 'zzz', name: 'Bored', icon: '😴', price: 200 },
  ];
  // Survivors (ids must match content::CHARACTERS). Free for now.
  CATALOG.char = [
    { id: 'rook', name: 'Rook · Gunner', icon: '🎯', price: 0, desc: 'Q: Heat Sink (double fire rate, ricochets). Passive: +10% fire rate. Signature: M9.' },
    { id: 'mara', name: 'Mara · Engineer', icon: '🔧', price: 0, desc: 'Q: Sentry Turret. Passive: +30% pickup radius. Signature: Viper SMG.' },
    { id: 'doc', name: 'Doc Hale · Medic', icon: '⛑️', price: 0, desc: 'Q: Heal Pulse (heals, shoves zombies). Passive: regen, 2× revive speed. Signature: Marksman.' },
    { id: 'ash', name: 'Ash · Pyro', icon: '🔥', price: 0, desc: 'Q: Flame Cone. Passive: hits can ignite. Signature: Boomstick.' },
  ];
  const CAT_LABEL_EXTRA = { pack: '🎒 Backpacks', killfx: '💫 Kill effects', deathfx: '☠️ Death effects', emote: '😀 Emotes (Y)' };
  const CAT_LABEL = { gun: '🔫 Guns', perk: '✨ Perks', skin: '👕 Skins', hat: '🎩 Hats', tracer: '☄️ Tracers' };
  Object.assign(CAT_LABEL, CAT_LABEL_EXTRA);
  const find = (cat, id) => CATALOG[cat].find((i) => i.id === id) || CATALOG[cat][0];

  const DIFF = {
    rookie: { name: 'Rookie', hint: 'Weaker, slower undead. 0.75× coins.' },
    survivor: { name: 'Survivor', hint: 'The intended experience. 1× coins.' },
    veteran: { name: 'Veteran', hint: 'Tougher, hungrier hordes. 1.5× coins.' },
    nightmare: { name: 'Nightmare', hint: 'Good luck. 2.25× coins.' },
  };

  // ------------------------------------------------------------------ story
  const STORY = [
    {
      title: 'The Long Night',
      intro: 'Three days after the meteor shower, the dead got up.\n\nThey called it the Bloom: grey spores drifting from the craters, settling on the graves of Hollowbrook. You\'re Rook, a courier with a busted truck, a pistol, and a radio that crackles one message on loop:\n\n<span class="speaker">"...the lighthouse at the horizon... we have power... come east..."</span>\n\nThe graveyard between you and the road is stirring. Get through it.',
      outro: 'The thing that called itself the Gravedigger crumbles into the soil it crawled from.\n\nThe radio clears for a moment. A woman\'s voice:\n\n<span class="speaker">"Courier? You\'re still breathing? Good. My name is Mara. Follow the pines east. Don\'t stop."</span>',
    },
    {
      title: 'Whispering Pines',
      intro: 'The forest is too quiet. Spores hang in the air like snow that refuses to fall.\n\n<span class="speaker">Mara: "Something nests in those trees. The infected there don\'t just bite. They spit. And their mother is the worst of them."</span>\n\nKeep your head down and your trigger finger busy.',
      outro: 'The Matriarch\'s brood scatters as she falls, hissing into the dirt.\n\n<span class="speaker">Mara: "You\'re faster than the last one who tried. Past the pines is the old refinery. It\'s been burning since the Bloom."</span>',
    },
    {
      title: 'Ashfall',
      intro: 'Ash falls like grey rain over the refinery. Every few seconds, something in the distance detonates.\n\n<span class="speaker">Rook: "Mara, the zombies here are... ticking."</span>\n<span class="speaker">Mara: "The heat cooked the spores into something volatile. Don\'t let them get close. And whatever wears the crown in there, it\'s the source."</span>',
      outro: 'The Cinder King\'s crown clatters to the ground, still glowing.\n\n<span class="speaker">Mara: "I can see your flare from the tower. You\'re close now. But the town between us... it sings at night. Be careful, Rook."</span>',
    },
    {
      title: 'The Hollow Choir',
      intro: 'Coldwater was a fishing town. Now its people stand in the streets humming one long, terrible note: the Hollow Choir.\n\n<span class="speaker">Mara: "The Bloom has wardens. Two of them, bound together. Hurt one and the other comes running. They\'re guarding the road to me."</span>\n\nThe lighthouse beam sweeps over the rooftops. Almost there.',
      outro: 'The Wraiths dissolve into mist, and the singing finally stops.\n\nAt the lighthouse door, Mara is waiting with a rifle and a tired smile.\n\n<span class="speaker">Mara: "There\'s something I haven\'t told you. The signal you followed... I built it to bring them here. All of them."</span>',
    },
    {
      title: 'Horizon',
      intro: '<span class="speaker">Mara: "The Bloom has a heart, buried under this cliff. The signal draws every infected thing toward it, and toward us. One last stand, courier. If the Heart dies, the Bloom dies with it."</span>\n\n<span class="speaker">Rook: "And if we die?"</span>\n<span class="speaker">Mara: "Then at least we\'ll have taken a lot of them with us."</span>\n\nThe ground splits open. Something enormous is waking up.',
      outro: 'The Bloom Heart bursts in a storm of pink light, and across the whole horizon, the dead lie down.\n\nThe sun comes up for the first time in weeks.\n\n<span class="speaker">Mara: "Not bad, courier. Not bad at all."</span>\n\n<b>THE END</b>, but the horde in Infinite Mode never sleeps.',
    },
  ];

  // ------------------------------------------------------------------ error reporting
  // Crashes and uncaught errors go to POST /errors (deduped, max 3 per page load).
  const reported = new Set();
  function reportError(kind, message, stack) {
    const base = window.UH_CONFIG?.apiBase;
    const key = kind + String(message).slice(0, 120);
    if (!base || reported.has(key) || reported.size >= 3) return;
    reported.add(key);
    const build = (document.querySelector('script[type="module"]')?.src || '').split('/').pop().slice(0, 40);
    Promise.resolve(window.UHCloud?.token?.().catch(() => null))
      .then((tok) => fetch(base + '/errors', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json', ...(tok ? { Authorization: 'Bearer ' + tok } : {}) },
        body: JSON.stringify({ kind, message: String(message), stack: String(stack || ''), url: location.href, build }),
        keepalive: true,
      }))
      .catch(() => {});
  }
  window.addEventListener('error', (e) => reportError('error', e.message, e.error?.stack || `${e.filename}:${e.lineno}`));
  window.addEventListener('unhandledrejection', (e) => reportError('promise', e.reason?.message || e.reason, e.reason?.stack));
  window.UHReportError = reportError;

  // ------------------------------------------------------------------ bridge
  const queue = [];
  window.uhPoll = () => (queue.length ? queue.shift() : undefined);
  const send = (cmd) => queue.push(cmd);

  // ------------------------------------------------------------------ data layer
  const store = {
    get(k, d) { try { const v = localStorage.getItem(k); return v ? JSON.parse(v) : d; } catch { return d; } },
    set(k, v) { try { localStorage.setItem(k, JSON.stringify(v)); } catch {} },
  };
  const LB_KEY = 'undeadHorizon.leaderboard.v1';
  const PROFILE_KEY = 'undeadHorizon.profile.v1';
  const SET_KEY = 'undeadHorizon.settings.v1';

  const freshProfile = () => ({
    coins: 0,
    owned: ['gun:pistol', 'perk:none', 'skin:default', 'hat:none', 'tracer:default', 'char:rook', 'char:mara', 'char:doc', 'char:ash',
      'pack:pack', 'killfx:default', 'deathfx:default', 'emote:wave'],
    equipped: { gun: 'pistol', perk: 'none', skin: 'default', hat: 'none', tracer: 'default', char: 'rook', pack: 'pack', killfx: 'default', deathfx: 'default', emote: 'wave' },
    campaign: { unlocked: 0, cleared: [] },
    saves: { infinite: null, campaign: null },
    stats: { runs: 0, kills: 0, bestWave: 0 },
    progress: window.UHProg.fresh(),
  });
  const normalizeProfile = (p) => {
    const f = freshProfile();
    p = Object.assign(f, p || {});
    p.equipped = Object.assign(freshProfile().equipped, p.equipped || {});
    p.campaign = Object.assign({ unlocked: 0, cleared: [] }, p.campaign || {});
    p.stats = Object.assign({ runs: 0, kills: 0, bestWave: 0 }, p.stats || {});
    p.progress = window.UHProg.normalize(p.progress);
    // older builds had a single save slot
    p.saves = Object.assign({ infinite: null, campaign: null }, p.saves || {});
    if (p.save) {
      p.saves[p.save.summary?.mode === 'campaign' ? 'campaign' : 'infinite'] = p.save;
      delete p.save;
    }
    for (const o of freshProfile().owned) if (!p.owned.includes(o)) p.owned.push(o);
    return p;
  };

  const cmpScore = (a, b) => b.wave - a.wave || b.level - a.level || b.kills - a.kills || a.time - b.time;
  const localProfile = () => normalizeProfile(store.get(PROFILE_KEY, null));
  const OB_KEY = 'undeadHorizon.outbreakBoard.v1';
  const DAILY_KEY = 'undeadHorizon.dailyBoard.v1';
  const localBoard = (filter) => {
    if (filter && filter.startsWith('ob:')) return store.get(OB_KEY, []).filter((e) => e.map === filter.slice(3)).slice(0, 10);
    if (filter && filter.startsWith('daily:')) return store.get(DAILY_KEY, []).filter((e) => e.daily === filter.slice(6)).slice(0, 10);
    let lb = store.get(LB_KEY, []);
    if (filter === 'week') return lb.filter((e) => Date.now() - new Date(e.date || 0).getTime() < 7 * 86400000).slice(0, 10);
    if (filter === 'solo') return lb.filter((e) => (e.players || 1) === 1).slice(0, 10);
    if (filter === 'coop') return lb.filter((e) => (e.players || 1) > 1).slice(0, 10);
    if (filter === 'pure') return lb.filter((e) => e.pure).slice(0, 10);
    if (filter === 'friends') return lb.slice(0, 10);
    if (filter && filter !== 'all') lb = lb.filter((e) => (e.diff || 'survivor') === filter);
    return lb.slice(0, 10);
  };

  // Data layer. Signed out it uses browser storage. Signed in (window.UHCloud,
  // see ui/cloud-src.js), every mutation goes through the server, which checks
  // prices, ownership and coin rewards.
  const Data = {
    profile: null,
    get cloud() { return window.UHCloud && window.UHCloud.user ? window.UHCloud : null; },
    get globalBoard() { return !!window.UHCloud; },
    async init() {
      if (window.UHCloud) await window.UHCloud.restore();
      await this.reload();
      // welcome back: a gentle bonus after 3+ days away (never a lost streak)
      const last = settings.lastSeen || 0;
      settings.lastSeen = Date.now(); saveSettings();
      try {
        if (this.cloud) {
          const r = await this.cloud.api('/welcome', { method: 'POST' });
          if (r.bonus) { this.applyCloud(r.profile); setTimeout(() => toast(`👋 Welcome back, survivor! +🪙 ${r.bonus}`), 1500); }
        } else if (last && Date.now() - last > 3 * 86400000) {
          this.profile.coins += 150; this.persistLocal();
          setTimeout(() => toast('👋 Welcome back, survivor! +🪙 150'), 1500);
        }
      } catch {}
    },
    async reload() {
      if (this.cloud) {
        try {
          let p = await this.cloud.api('/me');
          if (!p.imported) {
            // first sign-in on this account: carry over offline progress once
            const local = localProfile();
            p = await this.cloud.api('/me/import', { method: 'POST', body: { coins: local.coins, owned: local.owned, campaign: local.campaign } });
          }
          this.applyCloud(p);
          return;
        } catch (e) {
          console.warn('cloud profile unavailable, using local', e);
          toast('Cloud unavailable. Using this browser\'s profile.');
        }
      }
      this.profile = localProfile();
    },
    applyCloud(p) {
      // pull settings saved from another device (first load of this session only)
      if (p.settings && !this.settingsPulled) {
        this.settingsPulled = true;
        for (const [k, v] of Object.entries(p.settings)) if (!LOCAL_ONLY.includes(k)) settings[k] = v;
        store.set(SET_KEY, settings);
        setTimeout(() => applySettings(), 0);
      }
      this.profile = normalizeProfile(p);
      this.profile.name = p.name;
    },
    persistLocal() { if (!this.cloud) store.set(PROFILE_KEY, this.profile); },
    async buy(cat, id, price) {
      if (this.cloud) return this.applyCloud(await this.cloud.api('/shop/buy', { method: 'POST', body: { cat, id } }));
      const p = this.profile;
      const key = `${cat}:${id}`;
      if (p.owned.includes(key) || p.coins < price) throw new Error('cannot_buy');
      p.coins -= price;
      p.owned.push(key);
      p.equipped[cat] = id;
      this.persistLocal();
    },
    async equip(cat, id) {
      // the survivor pick is also kept locally, so it works before the server knows about it
      if (cat === 'char') { settings.char = id; saveSettings(); this.profile.equipped.char = id; }
      if (this.cloud) {
        try { return this.applyCloud(await this.cloud.api('/equip', { method: 'POST', body: { cat, id } })); }
        catch (e) { if (cat === 'char') return; throw e; }
      }
      this.profile.equipped[cat] = id;
      this.persistLocal();
    },
    /** Record a finished (or abandoned) run. Returns { coins, rank, id }. */
    async finishRun(s, abandoned = false) {
      if (this.cloud) {
        const r = await this.cloud.api('/runs', { method: 'POST', body: { ...s, abandoned } });
        this.applyCloud(r.profile);
        return { coins: r.coins, rank: r.rank, id: r.id, career: r.career || null };
      }
      const p = this.profile;
      p.coins += s.coins;
      p.stats.runs += 1;
      p.stats.kills += s.kills;
      let rank = null, id = null;
      if (s.mode === 'outbreak' && !abandoned) {
        const entry = { name: s.name, wave: s.wave, level: s.level, kills: s.kills, time: s.time, diff: s.diff, score: s.score || 0, map: s.map, id: String(Date.now() + Math.random()) };
        const ob = store.get(OB_KEY, []);
        ob.push(entry);
        ob.sort((a, b) => b.wave - a.wave || b.score - a.score);
        store.set(OB_KEY, ob.slice(0, 100));
      }
      if (s.daily && !abandoned) {
        const db = store.get(DAILY_KEY, []);
        if (!db.some((e) => e.daily === s.daily && e.mine)) {
          db.push({ name: s.name, wave: s.wave, level: s.level, kills: s.kills, time: s.time, diff: s.diff, score: s.score || 0, daily: s.daily, mine: true, id: String(Date.now()) });
          db.sort((a, b) => b.wave - a.wave || b.score - a.score);
          store.set(DAILY_KEY, db.slice(0, 60));
        }
      }
      if (s.mode === 'infinite' && !s.mutated && !s.daily) {
        p.stats.bestWave = Math.max(p.stats.bestWave || 0, s.wave);
        if (!abandoned) {
          const entry = { name: s.name, wave: s.wave, level: s.level, kills: s.kills, time: s.time, diff: s.diff, gun: s.gun, players: s.players, pure: s.pure, id: String(Date.now() + Math.random()), date: new Date().toISOString() };
          const lb = store.get(LB_KEY, []);
          lb.push(entry);
          lb.sort(cmpScore);
          store.set(LB_KEY, lb.slice(0, 100));
          rank = lb.indexOf(entry) + 1;
          id = entry.id;
        }
      } else if (s.victory) {
        const c = p.campaign;
        if (!c.cleared.includes(s.chapter)) c.cleared.push(s.chapter);
        c.unlocked = Math.max(c.unlocked, Math.min(s.chapter + 1, STORY.length - 1));
      }
      // career progress (the server does the same in /runs when signed in)
      const career = window.UHProg.applyRun(p.progress, { ...s, abandoned });
      p.progress = career.progress;
      p.coins += career.coins;
      for (const it of career.items) if (!p.owned.includes(it)) p.owned.push(it);
      this.persistLocal();
      return { coins: s.coins, rank, id, career };
    },
    async buyMeta(id) {
      if (this.cloud) return this.applyCloud(await this.cloud.api('/meta/buy', { method: 'POST', body: { id } }));
      const p = this.profile.progress;
      const def = window.UHProg.META.find((m) => m.id === id);
      const rank = p.meta[id] || 0;
      const cost = window.UHProg.metaCost(id, rank);
      if (!def || rank >= def.max || this.profile.coins < cost) throw new Error('cannot_buy');
      this.profile.coins -= cost;
      p.meta[id] = rank + 1;
      this.persistLocal();
    },
    async equipRelics(uids) {
      if (this.cloud) return this.applyCloud(await this.cloud.api('/relics/equip', { method: 'POST', body: { uids } }));
      this.profile.progress.equippedRelics = uids.slice(0, 3);
      this.profile.progress = window.UHProg.normalize(this.profile.progress);
      this.persistLocal();
    },
    async eventAchievement(id) {
      const p = this.profile.progress;
      if (p.achievements.includes(id)) return false;
      if (this.cloud) { try { this.applyCloud(await this.cloud.api('/achievement', { method: 'POST', body: { id } })); } catch {} }
      if (!this.profile.progress.achievements.includes(id)) this.profile.progress.achievements.push(id);
      this.persistLocal();
      return true;
    },
    /** Each mode has its own slot; saving overwrites that slot only. */
    async putSave(data, summary) {
      const slot = summary.mode === 'campaign' ? 'campaign' : 'infinite';
      if (this.cloud) {
        const r = await this.cloud.api('/save', { method: 'PUT', body: { data, summary } });
        this.profile.saves[slot] = { summary: r.summary };
        return;
      }
      this.profile.saves[slot] = { data, summary };
      this.persistLocal();
    },
    /** Returns a slot's save string and removes it (a save resumes once). */
    async takeSave(slot) {
      if (this.cloud) {
        const r = await this.cloud.api('/save/consume', { method: 'POST', body: { slot } });
        this.profile.saves[slot] = null;
        return r.data;
      }
      const d = this.profile.saves[slot]?.data;
      this.profile.saves[slot] = null;
      this.persistLocal();
      return d;
    },
    /** Starting a new run of a mode overwrites that mode's save. */
    async discardSave(slot) {
      if (!this.profile.saves[slot]) return;
      if (this.cloud) await this.cloud.api('/save?slot=' + slot, { method: 'DELETE' });
      this.profile.saves[slot] = null;
      this.persistLocal();
    },
    async leaderboard(filter) {
      if (window.UHCloud) return window.UHCloud.leaderboard(filter);
      return localBoard(filter);
    },
  };
  window.UHData = Data;

  // ------------------------------------------------------------------ settings
  const settings = Object.assign({ vol: 0.7, music: 0.5, autoaim: false, name: '', mode: 'infinite', diff: 'survivor', chapter: 0 }, store.get(SET_KEY, {}));
  // per-device keys stay local; everything else syncs to the account
  const LOCAL_ONLY = ['name', 'lastSeen', 'iosHint', 'char'];
  let syncT = 0;
  const saveSettings = () => {
    store.set(SET_KEY, settings);
    if (!window.UHData?.cloud) return;
    clearTimeout(syncT);
    syncT = setTimeout(() => {
      const out = Object.fromEntries(Object.entries(settings).filter(([k, v]) => !LOCAL_ONLY.includes(k) && (typeof v !== 'object' || Array.isArray(v))));
      window.UHData.cloud.api('/me/settings', { method: 'PUT', body: out }).catch(() => {});
    }, 1500);
  };
  // Extra settings, rendered into every `.xsettings` box from this table.
  // kind 'toggle' stores a boolean, 'range' a number; `cmd` sends it to the game.
  const XSETTINGS = [
    { key: 'lookahead', label: 'Camera lookahead (leads your aim)', kind: 'toggle', def: true, cmd: (v) => 'lookahead|' + (v ? 1 : 0) },
    { key: 'haptics', label: 'Vibration', kind: 'range', min: 0, max: 1, step: 0.1, def: 0.6 },
    { key: 'announcer', label: 'Announcer voice', kind: 'toggle', def: true },
    { key: 'vendorvoice', label: 'Vendor voices (Home Base)', kind: 'toggle', def: true },
    { key: 'subtitles', label: 'Subtitles for voice lines', kind: 'toggle', def: true },
    { key: 'shake', label: 'Screen shake', kind: 'range', min: 0, max: 1, step: 0.1, def: 1 },
    { key: 'flash', label: 'Screen flashes', kind: 'range', min: 0, max: 1, step: 0.1, def: 1 },
    { key: 'motion', label: 'Reduce motion (no zoom punches, slow-mo, lookahead)', kind: 'toggle', def: false },
    { key: 'assist', label: 'Aim assist strength', kind: 'range', min: 0, max: 1, step: 0.1, def: 0.5 },
    { key: 'cb', label: 'Colorblind mode', kind: 'select', def: '0', options: [['0', 'Off'], ['1', 'Deuteranopia / Protanopia'], ['2', 'Tritanopia']] },
    { key: 'toggleuse', label: 'Tap interact to toggle (instead of hold)', kind: 'toggle', def: false },
    { key: 'uiscale', label: 'Menu text size', kind: 'range', min: 0.8, max: 1.4, step: 0.05, def: 1 },
    { key: 'contrast', label: 'High-contrast text', kind: 'toggle', def: false },
    { key: 'lang', label: 'Language', kind: 'select', def: (navigator.language || 'en').slice(0, 2) === 'es' ? 'es' : 'en', options: [['en', 'English'], ['es', 'Español']] },
  ];
  for (const x of XSETTINGS) if (settings[x.key] === undefined) settings[x.key] = x.def;
  function renderXSettings() {
    for (const box of document.querySelectorAll('.xsettings')) {
      box.innerHTML = XSETTINGS.map((x) => x.kind === 'toggle'
        ? `<label class="toggle"><input type="checkbox" data-x="${x.key}" ${settings[x.key] ? 'checked' : ''} /> ${x.label}</label>`
        : x.kind === 'select'
        ? `<div class="setting"><span class="lbl">${x.label}</span><select data-x="${x.key}">${x.options.map(([v, l]) => `<option value="${v}" ${String(settings[x.key]) === v ? 'selected' : ''}>${l}</option>`).join('')}</select></div>`
        : `<div class="setting"><span class="lbl">${x.label}</span><input type="range" data-x="${x.key}" min="${x.min}" max="${x.max}" step="${x.step}" value="${settings[x.key]}" /></div>`).join('');
    }
  }
  document.addEventListener('change', (e) => {
    const k = e.target?.dataset?.x;
    if (!k) return;
    const x = XSETTINGS.find((y) => y.key === k);
    settings[k] = x.kind === 'toggle' ? e.target.checked : x.kind === 'select' ? e.target.value : +e.target.value;
    applySettings();
  });
  function applySettings() {
    renderXSettings();
    for (const x of XSETTINGS) if (x.cmd) send(x.cmd(settings[x.key]));
    send('tune|' + (settings.tune || 0));
    send(`access|shake=${settings.shake};flash=${settings.flash};motion=${settings.motion ? 1 : 0};assist=${settings.assist};cb=${settings.cb};toggleuse=${settings.toggleuse ? 1 : 0}`);
    document.documentElement.style.setProperty('--ui-scale', settings.uiscale || 1);
    document.body.classList.toggle('hc', !!settings.contrast);
    window.UHI18n?.setLang(settings.lang || 'en');
    document.body.classList.toggle('reduce-motion', !!settings.motion);
    send('bind|' + (settings.binds || '?'));
    for (const id of ['vol', 'vol2']) $(id).value = settings.vol;
    for (const id of ['music', 'music2']) $(id).value = settings.music;
    for (const id of ['autoaim', 'autoaim2']) $(id).checked = settings.autoaim;
    send('vol|' + settings.vol);
    send('music|' + settings.music);
    send('autoaim|' + (settings.autoaim ? 1 : 0));
    saveSettings();
  }
  for (const id of ['vol', 'vol2']) $(id).addEventListener('input', (e) => { settings.vol = +e.target.value; applySettings(); });
  for (const id of ['music', 'music2']) $(id).addEventListener('input', (e) => { settings.music = +e.target.value; applySettings(); });
  for (const id of ['autoaim', 'autoaim2']) $(id).addEventListener('change', (e) => { settings.autoaim = e.target.checked; applySettings(); });

  // ------------------------------------------------------------------ key remapping
  const BIND_LABELS = { up: 'Move up', down: 'Move down', left: 'Move left', right: 'Move right', dash: 'Dash', ability: 'Survivor ability', ult: 'Ultimate', interact: 'Interact / talk', reload: 'Reload (Outbreak)', swap: 'Swap weapon / sell', pause: 'Pause', autoaim: 'Toggle auto-aim', minimap: 'Toggle minimap', ping: 'Ping' };
  let rebinding = null;
  const keyName = (c) => c.replace(/^Key/, '').replace(/^Digit/, '').replace('ArrowUp', '↑').replace('ArrowDown', '↓').replace('ArrowLeft', '←').replace('ArrowRight', '→');
  function renderBinds(spec) {
    const map = Object.fromEntries(spec.split(';').map((kv) => kv.split('=')));
    $('binds').innerHTML = Object.entries(BIND_LABELS).map(([id, label]) =>
      `<div class="bind"><span>${label}</span><span>${(map[id] || '').split(',').filter(Boolean).map((c) => `<span class="kbd">${esc(keyName(c))}</span>`).join(' ') || '—'}</span>
        <button class="btn small secondary" data-rebind="${id}">${rebinding === id ? 'Press a key…' : 'Change'}</button></div>`).join('');
  }
  on('binds', 'click', (e) => {
    const b = e.target.closest('[data-rebind]');
    if (!b) return;
    rebinding = b.dataset.rebind;
    b.textContent = 'Press a key…';
  });
  window.addEventListener('keydown', (e) => {
    if (!rebinding) return;
    e.preventDefault(); e.stopPropagation();
    if (e.code !== 'Escape') {
      const parts = (settings.binds || '').split(';').filter((kv) => kv && !kv.startsWith(rebinding + '='));
      parts.push(`${rebinding}=${e.code}`);
      settings.binds = parts.join(';');
      saveSettings();
      send('bind|' + settings.binds);
    } else send('bind|?');
    rebinding = null;
  }, true);
  on('binds-reset', 'click', () => { settings.binds = ''; saveSettings(); send('bind|'); });

  // ------------------------------------------------------------------ helpers
  let toastT = 0;
  function toast(msg) {
    const t = $('toast');
    t.textContent = msg;
    t.classList.add('show');
    clearTimeout(toastT);
    toastT = setTimeout(() => t.classList.remove('show'), 2200);
  }
  const isTouch = matchMedia('(pointer: coarse)').matches || 'ontouchstart' in window;
  if (isTouch) {
    document.body.classList.add('touch');
    $('controls-help').innerHTML = '<span>👈 Left side</span><b>Drag to move</b><span>👉 Right side</span><b>Tap to dash</b><span>☢️</span><b>Ultimate when charged</b><span>🎯</span><b>Aiming &amp; firing are automatic</b>';
    send('touch|1');
  }
  const equipString = () => {
    const e = Data.profile.equipped;
    const pr = Data.profile.progress || window.UHProg.fresh();
    const relics = pr.equippedRelics.map((u) => pr.relics.find((r) => r.uid === u)).filter(Boolean).map(window.UHProg.relicSpec).join(',');
    let ch = settings.char || e.char || 'rook';
    if (!window.UHProg.charUnlocked(ch, pr)) ch = 'rook';
    return `gun=${e.gun};perk=${e.perk === 'none' ? '' : e.perk};skin=${e.skin};hat=${e.hat};tracer=${e.tracer};char=${ch}` +
      `;meta=${window.UHProg.metaSpec(pr.meta)};relics=${relics};camo=${window.UHProg.camoTier(pr.gunKills[e.gun])}` +
      `;pack=${e.pack || 'pack'};killfx=${e.killfx || 'default'};deathfx=${e.deathfx || 'default'};emote=${e.emote || 'wave'}`;
  };
  const pushEquip = () => send('equip|' + equipString());

  // ------------------------------------------------------------------ screens
  let state = 'loading';
  function show(id) {
    for (const o of ['menu', 'pause', 'gameover', 'story', 'auth']) $(o).classList.toggle('show', o === id);
    document.body.classList.toggle('playing', id === null);
    document.body.classList.toggle('in-menu', id === 'menu');
  }

  // ------------------------------------------------------------------ lobby
  function renderCoins() { $('coins').textContent = Data.profile.coins.toLocaleString(); }
  function renderTag() {
    const e = Data.profile.equipped;
    $('tag-name').textContent = ($('name').value || '').trim() || 'Survivor';
    const lv = window.UHProg.levelFromXp(Data.profile.progress?.xp || 0).level;
    $('tag-loadout').textContent = `Lv ${lv} · ${find('char', settings.char || e.char).name} · ${find('gun', e.gun).name} · ${find('perk', e.perk).name}`;
  }
  function renderMode() {
    for (const b of $('mode-seg').children) b.classList.toggle('on', b.dataset.mode === settings.mode);
    for (const b of $('diff-seg').children) b.classList.toggle('on', b.dataset.diff === settings.diff);
    $('diff-hint').textContent = DIFF[settings.diff].hint;
    const camp = settings.mode === 'campaign';
    $('chapter-box').style.display = camp ? '' : 'none';
    $('map-box').style.display = settings.mode === 'outbreak' ? '' : 'none';
    $('mut-box').style.display = settings.mode === 'infinite' ? '' : 'none';
    const wk = window.UHProg.weeklyMutator();
    settings.muts = settings.muts || [];
    $('mut-seg').innerHTML = window.UHProg.MUTATORS.map((m) => `<button data-mut="${m.id}" class="${settings.muts.includes(m.id) ? 'on' : ''}" title="${esc(m.desc)}">${esc(m.name)}${m.id === wk.id ? ' ⭐' : ''}</button>`).join('');
    const dly = window.UHProg.daily();
    const tried = !!Data.profile.progress?.dailyBest?.[dly.key];
    const dm = window.UHProg.MUTATORS.find((m) => m.id === dly.mutator);
    $('daily-btn').textContent = `☀️ Daily Run · ${dm.name}${tried ? ' (practice: your score is in)' : ''}`;
    $('siege-box').style.display = settings.mode === 'siege' ? '' : 'none';
    const stars = settings.siegeStars || {};
    $('siege-seg').innerHTML = SIEGE_MAPS.map((m) => `<button data-smap="${m.id}" class="${(settings.siegeMap || 'keep') === m.id ? 'on' : ''}">${m.icon} ${m.name} <span style="color:#ffd36a">${'★'.repeat(stars[m.id] || 0)}${'☆'.repeat(3 - (stars[m.id] || 0))}</span></button>`).join('');
    $('glass').checked = !!settings.glass;
    for (const b of $('map-seg').children) b.classList.toggle('on', b.dataset.map === (settings.map || 'asylum'));
    $('mode-hint').textContent = camp ? 'Story chapters with unique boss fights.'
      : settings.mode === 'siege' ? 'The Last Keep: build towers, traps and barricades with mana, then hold the crystal for 15 waves. 1-6 to build, E to upgrade, X to sell.'
      : settings.mode === 'outbreak' ? 'Rounds, boarded windows, points, doors, wall guns, a mystery box, perks, traps and an upgrade machine.'
      : 'Endless waves. Bosses every 5 waves. Ranked on the leaderboard.';
    if (camp) {
      const c = Data.profile.campaign;
      settings.chapter = Math.min(settings.chapter, c.unlocked);
      $('chapters').innerHTML = STORY.map((s, i) => {
        const locked = i > c.unlocked;
        const done = c.cleared.includes(i);
        return `<div class="chapter ${locked ? 'locked' : ''} ${i === settings.chapter ? 'on' : ''}" data-ch="${i}">
          <div class="num">${i + 1}</div><div><div class="t">${esc(s.title)}</div><div class="s">${locked ? 'Locked: clear the previous chapter' : done ? 'Cleared' : 'Not yet cleared'}</div></div>
          <div class="badge">${locked ? '🔒' : done ? '✅' : ''}</div></div>`;
      }).join('');
    }
    // saved runs: one slot per mode
    const saves = Data.profile.saves;
    const describe = (sv) => `${sv.summary.mode === 'campaign' ? 'Ch ' + (sv.summary.chapter + 1) + ' · ' : ''}Wave ${sv.summary.wave} · Lv ${sv.summary.level} · ${DIFF[sv.summary.diff]?.name || ''} · ${new Date(sv.summary.date).toLocaleDateString()}`;
    $('continues').innerHTML = ['infinite', 'campaign'].filter((m) => saves[m]).map((m) =>
      `<button class="btn blue" data-continue="${m}" style="margin-top:12px">⏯ Continue ${m === 'campaign' ? 'Campaign' : 'Infinite'} run</button>
       <div class="hint" style="text-align:center">${esc(describe(saves[m]))}</div>`).join('');
    const existing = ['outbreak', 'siege'].includes(settings.mode) ? null : saves[settings.mode];
    const sameChapter = existing && (settings.mode !== 'campaign' || existing.summary.chapter === settings.chapter);
    const warn = existing
      ? `⚠ Starting a new ${settings.mode === 'campaign' ? 'Campaign' : 'Infinite'} run overwrites your saved ${settings.mode === 'campaign' ? 'Campaign' : 'Infinite'} run (${describe(existing)}).${!sameChapter ? ' It\'s from a different chapter.' : ''}`
      : '';
    $('play-tip').textContent = warn || `No saved ${settings.mode === 'campaign' ? 'Campaign' : 'Infinite'} run to overwrite. Have fun!`;
    $('play-warn').textContent = existing ? 'Overwrites your saved run: hover Play for details' : '';
    saveSettings();
  }
  on('mode-seg', 'click', (e) => { const b = e.target.closest('button'); if (b) { settings.mode = b.dataset.mode; renderMode(); send('equip|'); } });
  const SIEGE_MAPS = [
    { id: 'keep', name: 'The Last Keep', icon: '🏰' },
    { id: 'bridge', name: 'River Crossing', icon: '🌉' },
    { id: 'crossroads', name: 'The Crossroads', icon: '✖️' },
  ];
  on('mut-seg', 'click', (e) => {
    const b = e.target.closest('[data-mut]');
    if (!b) return;
    settings.muts = settings.muts || [];
    const id = b.dataset.mut;
    settings.muts = settings.muts.includes(id) ? settings.muts.filter((x) => x !== id) : [...settings.muts, id];
    renderMode();
  });
  on('daily-btn', 'click', () => {
    const d = window.UHProg.daily();
    settings.mode = 'infinite';
    dailyOverride = `;seed=${d.seed};mut=${d.mutator};daily=${d.key}`;
    start();
  });
  on('siege-seg', 'click', (e) => { const b = e.target.closest('button'); if (b) { settings.siegeMap = b.dataset.smap; renderMode(); } });
  on('glass', 'change', (e) => { settings.glass = e.target.checked; saveSettings(); });
  on('map-seg', 'click', (e) => { const b = e.target.closest('button'); if (b) { settings.map = b.dataset.map; renderMode(); } });
  on('diff-seg', 'click', (e) => {
    const b = e.target.closest('button');
    if (!b || b.disabled) return;
    settings.diff = b.dataset.diff;
    renderMode();
    window.UHCoop?.setDiff(settings.diff);
  });
  on('chapters', 'click', (e) => {
    const c = e.target.closest('.chapter');
    if (!c || c.classList.contains('locked')) return;
    settings.chapter = +c.dataset.ch;
    renderMode();
  });
  on('name', 'input', () => { settings.name = $('name').value; saveSettings(); renderTag(); });
  on('name', 'change', async () => {
    if (!Data.cloud) return;
    try { Data.applyCloud(await Data.cloud.api('/me/name', { method: 'POST', body: { name: playerName() } })); toast('Name updated'); }
    catch { toast('Name must be 2-16 letters or numbers.'); $('name').value = Data.profile.name; }
  });

  // tabs
  function renderCareer() {
    const P = window.UHProg;
    const pr = Data.profile.progress;
    const lv = P.levelFromXp(pr.xp);
    const got = new Set(pr.achievements);
    const ach = P.ACHIEVEMENTS.map((a) => {
      const has = got.has(a.id);
      const secret = a.hidden && !has;
      return `<div class="ach ${has ? 'got' : ''}" title="${esc(secret ? 'Secret achievement' : a.desc)}"><div class="ai">${secret ? '❔' : a.icon}</div><div><b>${esc(secret ? '???' : a.name)}</b><span>${esc(secret ? 'Secret' : a.desc)}</span></div></div>`;
    }).join('');
    const codex = P.KINDS.map((k) => {
      const n = pr.kinds[k] || 0;
      return `<div class="codex ${n ? '' : 'unk'}"><b>${n ? esc(k) : '???'}</b><span>${n ? esc(P.BESTIARY[k] || '') : 'Kill one to learn more.'}</span><i>${n ? n.toLocaleString() + ' killed' : ''}</i></div>`;
    }).join('');
    const guns = CATALOG.gun.map((g) => {
      const k = pr.gunKills[g.id] || 0;
      const tier = P.camoTier(k);
      const next = P.CAMO_AT[tier];
      return `<div class="mastery"><span>${g.icon} ${esc(g.name)}</span><span>${['No camo', '🌲 Woodland', '🌊 Urban', '👑 Gold'][tier]}</span><span class="hint">${k.toLocaleString()} kills${next ? ` · next at ${next.toLocaleString()}` : ' · mastered'}</span></div>`;
    }).join('');
    $('career').innerHTML = `
      <div class="acct"><div class="acct-lv">${lv.level}</div><div style="flex:1"><b>Account level ${lv.level}</b>
        <div class="acct-bar"><div style="width:${Math.round((lv.into / lv.need) * 100)}%"></div></div>
        <span class="hint">${lv.into.toLocaleString()} / ${lv.need.toLocaleString()} XP · next reward: 🪙 ${P.levelReward(lv.level + 1).coins}${P.levelReward(lv.level + 1).item ? ' + ' + esc(P.levelReward(lv.level + 1).item.split(':')[1]) : ''}</span></div></div>
      <div class="hint" style="margin:6px 0 14px">${pr.runs} runs · ${pr.kills.toLocaleString()} kills · ${pr.bosses} bosses · ${got.size}/${P.ACHIEVEMENTS.length} achievements</div>
      <h3>🎯 Challenges</h3><div class="chals">${P.currentChallenges().map((c, i) => {
        const st = pr.challenges?.[c.id] || { v: 0, done: false };
        const k = Math.min(1, st.v / c.goal);
        return `<div class="chal ${st.done ? 'done' : ''}"><span class="tag">${i < 3 ? 'Daily' : 'Weekly'}</span><b>${esc(c.text)}</b>
          <div class="acct-bar"><div style="width:${Math.round(k * 100)}%"></div></div><span class="hint">${st.done ? '✓ Done' : `${Math.min(st.v, c.goal).toLocaleString()} / ${c.goal.toLocaleString()}`} · 🪙 ${c.coins} · ${c.sxp} season XP</span></div>`;
      }).join('')}</div>
      <h3 style="margin-top:18px">🌸 ${esc(P.SEASON.name)} <span class="hint">(free · tier ${P.seasonTier(pr.season?.sxp)} / ${P.SEASON.tiers})</span></h3>
      <div class="season">${Array.from({ length: P.SEASON.tiers }, (_, i) => {
        const tier = i + 1;
        const rw = P.SEASON_REWARDS(tier);
        const got = P.seasonTier(pr.season?.sxp) >= tier;
        return `<div class="tier ${got ? 'got' : ''} ${rw.item ? 'big' : ''}" title="Tier ${tier}: ${rw.item ? esc(rw.item.split(':')[1]) : '🪙 ' + rw.coins}"><b>${tier}</b><span>${rw.item ? '🎁' : '🪙'}</span></div>`;
      }).join('')}</div>
      <h3 style="margin-top:18px">🏅 Achievements</h3><div class="achs">${ach}</div>
      <h3 style="margin-top:18px">📖 Bestiary</h3><div class="codexes">${codex}</div>
      <h3 style="margin-top:18px">🔫 Weapon mastery</h3><div class="masteries">${guns}</div>`;
  }
  on('tabs', 'click', (e) => {
    const b = e.target.closest('button');
    if (!b) return;
    for (const x of $('tabs').children) x.classList.toggle('on', x === b);
    for (const t of ['lb', 'shop', 'locker', 'career', 'friends', 'settings']) $('tab-' + t)?.classList.toggle('on', t === b.dataset.tab);
    if (b.dataset.tab === 'career') renderCareer();
    if (b.dataset.tab === 'friends') window.UHCoop?.render();
    if (b.dataset.tab === 'shop') renderShop();
    if (b.dataset.tab === 'locker') renderLocker();
    if (b.dataset.tab === 'lb') renderLB($('lb-menu'));
  });

  // ------------------------------------------------------------------ shop
  let shopCat = 'gun';
  $('shop-cats').innerHTML = Object.keys(CATALOG).filter((c) => c !== 'char').map((c) => `<button data-c="${c}" class="${c === shopCat ? 'on' : ''}">${CAT_LABEL[c]}</button>`).join('');
  on('shop-cats', 'click', (e) => {
    const b = e.target.closest('button');
    if (!b) return;
    shopCat = b.dataset.c;
    for (const x of $('shop-cats').children) x.classList.toggle('on', x === b);
    renderShop();
  });
  const visual = (cat, it) => it.color ? `<div class="sw" style="background:${it.color}"></div>` : `<div class="ic">${it.icon}</div>`;
  const rarityTag = (it) => it.rarity === 'epic' ? '<span class="rar" style="color:var(--epic)">EPIC</span>' : '';
  function renderMeta() {
    const pr = Data.profile.progress;
    $('meta-box').innerHTML = `<h3 style="margin:0 0 8px">⬆️ Permanent upgrades <span class="hint">(apply to every run; the board marks runs without them as Pure)</span></h3>
      <div class="items">${window.UHProg.META.map((m) => {
        const rank = pr.meta[m.id] || 0;
        const cost = window.UHProg.metaCost(m.id, rank);
        return `<div class="item"><div class="ic">${m.icon}</div><div class="nm">${esc(m.name)} ${'●'.repeat(rank)}${'○'.repeat(m.max - rank)}</div><div class="ds">${esc(m.desc)}</div>
          ${rank >= m.max ? '<button class="btn small secondary" disabled>Maxed</button>' : `<button class="btn small gold" data-meta="${m.id}" ${Data.profile.coins >= cost ? '' : 'disabled'}>🪙 ${cost.toLocaleString()}</button>`}</div>`;
      }).join('')}</div><h3 style="margin:16px 0 8px">🛍️ Items</h3>`;
  }
  on('meta-box', 'click', async (e) => {
    const b = e.target.closest('[data-meta]');
    if (!b) return;
    b.disabled = true;
    try { await Data.buyMeta(b.dataset.meta); toast('Upgrade purchased!'); } catch { toast('Not enough coins.'); }
    renderMeta(); renderCoins(); pushEquip();
  });
  function renderShop() {
    renderMeta();
    const p = Data.profile;
    $('shop-items').innerHTML = CATALOG[shopCat].filter((it) => it.price > 0).map((it) => {
      const owned = p.owned.includes(`${shopCat}:${it.id}`);
      const afford = p.coins >= it.price;
      return `<div class="item">${rarityTag(it)}${visual(shopCat, it)}<div class="nm">${esc(it.name)}</div>
        <div class="ds">${esc(it.desc || '')}</div>
        ${owned ? '<button class="btn small secondary" disabled>Owned ✓</button>'
          : `<button class="btn small gold" data-buy="${it.id}" ${afford ? '' : 'disabled'}>🪙 ${it.price.toLocaleString()}</button>`}</div>`;
    }).join('');
  }
  on('shop-items', 'click', async (e) => {
    const b = e.target.closest('[data-buy]');
    if (!b) return;
    const it = find(shopCat, b.dataset.buy);
    b.disabled = true;
    try {
      await Data.buy(shopCat, it.id, it.price);
    } catch (err) {
      toast(err.message === 'cannot_buy' ? 'Not enough coins.' : 'Purchase failed. Try again.');
      renderShop();
      return;
    }
    pushEquip();
    window.UHCoop?.lookChanged();
    renderShop(); renderCoins(); renderTag();
    toast(`Unlocked ${it.name}! Equipped.`);
  });

  // ------------------------------------------------------------------ locker (inventory + loadout)
  function renderLocker() {
    const p = Data.profile;
    const group = (cat, title) => {
      const items = CATALOG[cat].filter((it) => p.owned.includes(`${cat}:${it.id}`));
      return `<div class="locker-group"><h3>${title}</h3><div class="items">${items.map((it) =>
        `<div class="item ${p.equipped[cat] === it.id ? 'equipped' : ''}" data-eq="${cat}:${it.id}" style="cursor:pointer">${rarityTag(it)}${visual(cat, it)}<div class="nm">${esc(it.name)}</div>
          <div class="ds">${p.equipped[cat] === it.id ? '✓ Equipped' : 'Click to equip'}</div></div>`).join('')}</div></div>`;
    };
    const curChar = settings.char || p.equipped.char || 'rook';
    const prog = p.progress;
    const chars = `<div class="locker-group"><h3>Survivor</h3><div class="items chars">${CATALOG.char.map((it) => {
      const open = window.UHProg.charUnlocked(it.id, prog);
      const need = window.UHProg.ACHIEVEMENTS.find((a) => a.id === window.UHProg.CHAR_UNLOCK[it.id]);
      return `<div class="item ${curChar === it.id ? 'equipped' : ''} ${open ? '' : 'locked'}" ${open ? `data-eq="char:${it.id}"` : ''} style="cursor:${open ? 'pointer' : 'default'}"><div class="ico">${open ? it.icon : '🔒'}</div><div class="nm">${esc(it.name)}</div>
        <div class="ds">${open ? esc(it.desc) : `Unlock: ${esc(need?.name || '')} (${esc(need?.desc || '')})`}</div></div>`;
    }).join('')}</div></div>`;
    const relicBox = `<div class="locker-group"><h3>Relics <span class="hint">(equip up to 3 · bosses drop them)</span></h3>${prog.relics.length ? `<div class="items">${prog.relics.map((r) => {
      const l = window.UHProg.relicLabel(r);
      const on = prog.equippedRelics.includes(r.uid);
      return `<div class="item ${on ? 'equipped' : ''} rel-${r.rarity}" data-relic="${r.uid}" style="cursor:pointer"><div class="ico">${l.icon}</div><div class="nm">${esc(l.name)}</div><div class="ds">${esc(l.desc)}</div><div class="ds">${on ? '✓ Equipped' : 'Click to equip'}</div></div>`;
    }).join('')}</div>` : '<div class="empty">No relics yet. Kill bosses to find them.</div>'}</div>`;
    $('locker').innerHTML =
      `<h2>Survivor</h2>${chars}${relicBox}<h2 style="margin-top:20px">Loadout</h2>${group('gun', 'Primary weapon')}${group('perk', 'Starting perk')}
       <h2 style="margin-top:20px">Character</h2>${group('skin', 'Outfit')}${group('hat', 'Headgear')}${group('pack', 'Backpack')}${group('tracer', 'Bullet tracers')}
       ${group('killfx', 'Kill effect')}${group('deathfx', 'Death effect')}${group('emote', 'Emote (press Y)')}
       <div class="hint" style="margin-top:12px">Buy more in the 🛒 Shop with coins earned by surviving waves.</div>`;
  }
  on('locker', 'click', async (e) => {
    const rel = e.target.closest('[data-relic]');
    if (rel) {
      const pr = Data.profile.progress;
      const u = rel.dataset.relic;
      let eq = pr.equippedRelics.includes(u) ? pr.equippedRelics.filter((x) => x !== u) : [...pr.equippedRelics, u];
      if (eq.length > 3) { toast('Only 3 relics at a time.'); return; }
      try { await Data.equipRelics(eq); } catch { toast('Could not equip that relic.'); }
      pushEquip(); renderLocker();
      return;
    }
    const it = e.target.closest('[data-eq]');
    if (!it) return;
    const [cat, id] = it.dataset.eq.split(':');
    try { await Data.equip(cat, id); } catch { toast('Could not equip that item.'); }
    pushEquip();
    window.UHCoop?.lookChanged();
    renderLocker(); renderTag();
  });

  // ------------------------------------------------------------------ leaderboard
  let lbFilter = 'all';
  on('lb-filter', 'click', (e) => {
    const b = e.target.closest('button');
    if (!b) return;
    lbFilter = b.dataset.f;
    for (const x of $('lb-filter').children) x.classList.toggle('on', x === b);
    renderLB($('lb-menu'));
  });
  async function renderLB(el, highlightId) {
    let lb = [];
    let filter = el.id === 'lb-menu' ? lbFilter : 'all';
    if (filter === 'daily') filter = 'daily:' + window.UHProg.dayKey();
    try { lb = await Data.leaderboard(filter); } catch (err) { console.warn(err); lb = localBoard(filter); }
    $('lb-live').style.display = Data.globalBoard ? '' : 'none';
    const note = Data.globalBoard && !Data.cloud ? '<div class="hint" style="margin-top:10px">🌍 Global board. <a href="#" data-signin style="color:var(--accent)">Sign in</a> to get your runs on it.</div>' : '';
    if (!lb.length) { el.innerHTML = '<div class="empty">No runs yet. Be the first legend.</div>' + note; return; }
    const ob = filter.startsWith('ob:');
    el.innerHTML = `<table class="lb"><thead><tr><th>#</th><th>Name</th><th>${ob ? 'Round' : 'Wave'}</th><th>${ob ? 'Score' : 'Lv'}</th><th>Kills</th><th>Diff</th></tr></thead><tbody>` +
      lb.map((e, i) => `<tr class="${e.id === highlightId ? 'me' : ''}"><td class="rank">${i + 1}</td><td class="name">${esc(e.name)}</td><td class="wave">${e.wave}</td><td>${ob ? (e.score || 0).toLocaleString() : e.level}</td><td>${e.kills}</td><td><span class="dtag ${esc(e.diff || 'survivor')}">${esc(DIFF[e.diff || 'survivor']?.name || '')}</span></td></tr>`).join('') +
      '</tbody></table>' + note;
  }
  document.addEventListener('click', (e) => { if (e.target.closest('[data-signin]')) { e.preventDefault(); openAccount(); } });
  // Live refresh while the leaderboard is visible on the menu.
  setInterval(() => { if (state === 'menu' && $('tab-lb').classList.contains('on') && Data.globalBoard) renderLB($('lb-menu')); }, 5000);

  // ------------------------------------------------------------------ flow
  function toMenu() {
    if (typeof closeStall === 'function') closeStall(true);
    state = 'menu';
    renderCoins(); renderTag(); renderMode();
    renderLB($('lb-menu'));
    pushEquip();
    send('menu');
    show('menu');
  }
  function playerName() { return ($('name').value || '').trim().replace(/[|;=]/g, '') || 'Survivor'; }
  async function personalBest(name) {
    if (Data.cloud) return Data.profile.stats.bestWave || 0;
    const lb = store.get(LB_KEY, []);
    return lb.filter((e) => e.name === name).reduce((m, e) => Math.max(m, e.wave), Data.profile.stats.bestWave || 0);
  }

  let storyResolve = null;
  let storyTimer = 0;
  // comic-style panels before each chapter's intro (art is layered emoji over a painted sky)
  const PANELS = [
    [
      { bg: 'linear-gradient(#1a1030, #3a1a40 60%, #0b0e14)', art: '🌠 🌠\n🪦 🪦 🪦', cap: 'Three days after the meteor shower…' },
      { bg: 'linear-gradient(#0b0e14, #1f2b1b)', art: '🧟 🧟‍♀️ 🧟', cap: '…the dead got up.' },
      { bg: 'linear-gradient(#20140c, #0b0e14)', art: '🚚💨  📻', cap: 'A courier with a busted truck hears a voice on the radio.' },
    ],
    [
      { bg: 'linear-gradient(#0f1a0c, #0b0e14)', art: '🌲 🌫️ 🌲 🌲', cap: 'The pines are too quiet.' },
      { bg: 'linear-gradient(#1a2a10, #0b0e14)', art: '🕷️ 🧟‍♀️ 🕸️', cap: 'Something nests in the trees, and it spits.' },
    ],
    [
      { bg: 'linear-gradient(#3a1a0a, #120806)', art: '🏭 🔥 🏭', cap: 'Ash falls like grey rain over the refinery.' },
      { bg: 'linear-gradient(#2a0a04, #0b0e14)', art: '👑🔥', cap: 'Whatever wears the crown in there is the source.' },
    ],
    [
      { bg: 'linear-gradient(#10182a, #050810)', art: '🏚️ 🎶 🏚️', cap: 'The town sings at night.' },
      { bg: 'linear-gradient(#0a1020, #000)', art: '👻   👻', cap: 'Two voices. One choir.' },
    ],
    [
      { bg: 'linear-gradient(#2a0a20, #0b0e14)', art: '🗼 ⚡ 🌅', cap: 'The lighthouse at the horizon.' },
      { bg: 'linear-gradient(#400a30, #0b0e14)', art: '🫀🌸🫀', cap: 'And under the cliff, the Bloom has a heart.' },
    ],
  ];
  const PORTRAITS = { Rook: ['🧢', '#3b7bea'], Mara: ['📻', '#9cffb8'], Courier: ['🧢', '#3b7bea'] };
  function withPortraits(html) {
    return html.replace(/<span class="speaker">([A-Za-z]+):/g, (m, who) => {
      const p = PORTRAITS[who];
      return p ? `<span class="speaker"><i class="pt" style="background:${p[1]}">${p[0]}</i>${who}:` : m;
    });
  }
  async function playPanels(list) {
    const box = $('story-panel');
    if (!list?.length) { box.style.display = 'none'; return; }
    box.style.display = '';
    for (const p of list) {
      box.innerHTML = `<div class="sp" style="background:${p.bg}"><div class="sp-art">${p.art.replace(/\n/g, '<br>')}</div><div class="sp-cap">${esc(p.cap)}</div></div>`;
      await new Promise((res) => {
        const t = setTimeout(res, 2600);
        box.onclick = () => { clearTimeout(t); res(); };
      });
    }
    box.style.display = 'none';
  }
  function showStory(i, which, btnLabel) {
    const s = STORY[i];
    $('story-ch').textContent = which === 'outro' ? `CHAPTER ${i + 1} COMPLETE` : `CHAPTER ${i + 1}`;
    $('story-title').textContent = s.title;
    $('story-go').textContent = btnLabel;
    // typewriter, HTML-aware (speaker tags appear whole)
    const html = withPortraits(s[which]);
    const el = $('story-text');
    el.innerHTML = '';
    let k = 0;
    clearInterval(storyTimer);
    const type = () => {
      storyTimer = setInterval(() => {
        if (k >= html.length) { clearInterval(storyTimer); return; }
        if (html[k] === '<') k = html.indexOf('>', k) + 1;
        else k += 2;
        el.innerHTML = html.slice(0, k);
      }, 16);
    };
    if (which === 'intro' && !settings.motion) playPanels(PANELS[i]).then(type);
    else { $('story-panel').style.display = 'none'; type(); }
    $('story-skip').onclick = () => { clearInterval(storyTimer); $('story-panel').style.display = 'none'; el.innerHTML = html; };
    state = 'story';
    show('story');
    return new Promise((res) => { storyResolve = res; });
  }
  on('story-go', 'click', () => { clearInterval(storyTimer); const r = storyResolve; storyResolve = null; r && r(); });

  let runCtx = null;
  let dailyOverride = '';
  async function start() {
    const name = playerName();
    settings.name = name; saveSettings();
    $('name').blur();
    if (settings.mode === 'campaign') await showStory(settings.chapter, 'intro', 'Begin ▶');
    // a new run of this mode overwrites that mode's save (warned in the Play tooltip)
    await Data.discardSave(settings.mode).catch((e) => console.warn(e));
    const best = settings.mode === 'infinite' ? await personalBest(name) : 0;
    runCtx = { mode: settings.mode, chapter: settings.chapter, diff: settings.diff };
    state = 'playing';
    show(null);
    applySettings();
    pushEquip();
    let extra = '';
    if (dailyOverride) { extra = dailyOverride; dailyOverride = ''; }
    else if (settings.mode === 'infinite' && (settings.muts || []).length) extra = `;mut=${settings.muts.join(',')}`;
    const map = extra + (settings.mode === 'outbreak' ? `;map=${settings.map || 'asylum'}`
      : settings.mode === 'siege' ? `;map=${settings.siegeMap || 'keep'}${settings.glass ? ';variant=glass' : ''}` : '');
    const tut = !settings.tutorialDone && settings.mode === 'infinite' && !extra ? ';tutorial=1' : '';
    send(`start|name=${name};best=${best};mode=${settings.mode};chapter=${settings.chapter};diff=${settings.diff};${equipString()}${map}${tut}`);
  }
  async function continueRun(slot) {
    const sv = Data.profile.saves[slot];
    if (!sv) return;
    let data;
    try { data = await Data.takeSave(slot); } catch { toast('Could not load the save.'); renderMode(); return; }
    if (!data) { renderMode(); return; }
    runCtx = { mode: sv.summary.mode, chapter: sv.summary.chapter, diff: sv.summary.diff };
    state = 'playing';
    show(null);
    applySettings();
    pushEquip();
    send('load|' + data);
  }

  on('play', 'click', start);

  // ------------------------------------------------------------------ Home Base
  // Vendors are NPCs at stalls; their lines come from POST /vendor (Claude, with
  // canned fallbacks) and are spoken with per-vendor voices.
  const VENDORS = {
    gunsmith: { name: 'Rusty Vex', title: 'Gunsmith', icon: '🔫', cats: ['gun'], pitch: 0.75, rate: 1.15 },
    tailor: { name: 'Madame Lucienne', title: 'Tailor', icon: '🎩', cats: ['skin', 'hat'], pitch: 1.35, rate: 0.95 },
    quartermaster: { name: 'Sergeant Dole', title: 'Quartermaster', icon: '📦', cats: ['perk', 'tracer'], pitch: 0.55, rate: 0.85 },
    dispatcher: { name: 'Ada', title: 'Mission Dispatcher', icon: '📻', missions: true, pitch: 1.1, rate: 1.0 },
    monument: { name: 'Hall of Survivors', title: 'Memorial', icon: '🏆', board: true },
  };
  const LOCAL_LINES = {
    greet: ['Well, look who survived.', 'Courier. What do you need?'],
    buy: ['Pleasure doing business.', 'Good choice.'],
    broke: ['Come back with more coins.', 'Not quite enough, friend.'],
    browse: ['That one? Solid.', 'Take your time.'],
    owned: ['Already yours.', 'You have that one.'],
    chat: ['Hm. Interesting.', 'If you say so.'],
    leave: ['See you out there.', 'Stay alive.'],
  };
  let stall = null;
  let talkBusy = false;
  let browseT = 0;
  let subT = 0;
  function showSub(text) {
    const el = $('subs');
    el.textContent = text;
    el.classList.add('show');
    clearTimeout(subT);
    subT = setTimeout(() => el.classList.remove('show'), 2600 + text.length * 30);
  }
  window.UHSub = showSub;
  function speak(v, line) {
    if (!settings.vendorvoice || !window.speechSynthesis || !v.pitch) return;
    try {
      const u = new SpeechSynthesisUtterance(line);
      u.pitch = v.pitch; u.rate = v.rate; u.volume = Math.min(1, (settings.vol ?? 0.7) * 1.1);
      speechSynthesis.cancel();
      speechSynthesis.speak(u);
    } catch {}
  }
  async function talk(id, event, extra = {}) {
    const v = VENDORS[id];
    if (!v || v.board) return;
    const el = $('v-line');
    if (stall === id) el.classList.add('thinking');
    let line = null;
    try {
      const base = window.UH_CONFIG?.apiBase;
      if (base) {
        const tok = await window.UHCloud?.token?.().catch(() => null);
        const r = await fetch(base + '/vendor', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json', ...(tok ? { Authorization: 'Bearer ' + tok } : {}) },
          body: JSON.stringify({ vendor: id, event, coins: Data.profile.coins, name: playerName(), ...extra }),
        });
        if (r.ok) line = (await r.json()).line;
      }
    } catch (e) { console.warn('[vendor]', e); }
    if (!line) { const pool = LOCAL_LINES[event] || LOCAL_LINES.chat; line = pool[Math.floor(Math.random() * pool.length)]; }
    if (stall === id) { el.textContent = line; el.classList.remove('thinking'); }
    if (settings.subtitles && stall !== id) showSub(`${v.name}: ${line}`);
    send('npcsay|' + id + '|' + line.replace(/\|/g, '/'));
    speak(v, line);
    return line;
  }
  function renderStall() {
    const v = VENDORS[stall];
    if (!v) return;
    $('v-ic').textContent = v.icon;
    $('v-name').textContent = v.name;
    $('v-title').textContent = v.title;
    $('v-coins').textContent = '🪙 ' + Data.profile.coins.toLocaleString();
    $('v-chat').style.display = v.board ? 'none' : '';
    const p = Data.profile;
    if (v.board) {
      $('v-line').textContent = 'Names carved by the ones who lasted longest.';
      $('v-body').innerHTML = '<div id="lb-hub"></div>';
      renderLB($('lb-hub'));
    } else if (v.missions) {
      const ms = [
        ['infinite', null, '♾️ Infinite survival', 'Endless waves, ranked on the board'],
        ['campaign', null, '📖 Campaign', `Chapter ${settings.chapter + 1}: ${STORY[settings.chapter]?.title || ''}`],
        ['outbreak', 'asylum', '🏚️ Outbreak: Hollowbrook Asylum', 'Rounds, doors, the box, perks'],
        ['outbreak', 'refinery', '🏭 Outbreak: Ashfall Refinery', 'Rounds on the burning refinery'],
        ['siege', 'keep', '🏰 Siege: The Last Keep', 'Build defenses, hold the crystal'],
        ['siege', 'bridge', '🌉 Siege: River Crossing', 'Hold the bridges'],
        ['siege', 'crossroads', '✖️ Siege: The Crossroads', 'Four portals, one crystal'],
      ];
      $('v-body').innerHTML = `<div class="mission">${ms.map(([m, map, t, d]) =>
        `<button class="btn secondary" data-mission="${m}" data-map="${map || ''}"><b>${t}</b><br><span class="hint">${d} · ${DIFF[settings.diff].name}</span></button>`).join('')}</div>`;
    } else {
      $('v-body').innerHTML = v.cats.map((cat) => `<h3>${CAT_LABEL[cat] || cat}</h3><div class="items">${CATALOG[cat].map((it) => {
        const owned = p.owned.includes(`${cat}:${it.id}`);
        const eq = p.equipped[cat] === it.id;
        return `<div class="item ${eq ? 'equipped' : ''}" data-look="${cat}:${it.id}">${rarityTag(it)}${visual(cat, it)}<div class="nm">${esc(it.name)}</div>
          ${owned ? `<button class="btn small secondary" data-equip="${cat}:${it.id}" ${eq ? 'disabled' : ''}>${eq ? 'Equipped' : 'Equip'}</button>`
            : `<button class="btn small gold" data-vbuy="${cat}:${it.id}">🪙 ${it.price.toLocaleString()}</button>`}</div>`;
      }).join('')}</div>`).join('');
    }
  }
  function openStall(id) {
    stall = id;
    $('v-you').textContent = '';
    $('v-line').textContent = '…';
    renderStall();
    $('vendor').classList.add('show');
    talk(id, 'greet');
  }
  function closeStall(silent) {
    if (!stall) return;
    const id = stall;
    stall = null;
    $('vendor').classList.remove('show');
    send('hubclose');
    if (!silent) talk(id, 'leave');
    $('v-say').blur();
  }
  on('v-close', 'click', () => closeStall());
  on('vendor', 'click', async (e) => {
    const buy = e.target.closest('[data-vbuy]');
    const eq = e.target.closest('[data-equip]');
    const lookAt = e.target.closest('[data-look]');
    const mission = e.target.closest('[data-mission]');
    if (mission) {
      settings.mode = mission.dataset.mission;
      if (settings.mode === 'outbreak') settings.map = mission.dataset.map;
      if (settings.mode === 'siege') settings.siegeMap = mission.dataset.map;
      saveSettings();
      talk('dispatcher', 'buy', { item: mission.textContent.trim().slice(0, 40) });
      closeStall(true);
      setTimeout(() => start(), 600);
      return;
    }
    if (buy) {
      const [cat, id] = buy.dataset.vbuy.split(':');
      const it = find(cat, id);
      if (Data.profile.coins < it.price) { talk(stall, 'broke', { item: it.name }); return; }
      buy.disabled = true;
      try { await Data.buy(cat, id, it.price); }
      catch { talk(stall, 'broke', { item: it.name }); renderStall(); return; }
      pushEquip(); window.UHCoop?.lookChanged();
      renderCoins(); renderTag(); renderStall();
      send('sfx|start');
      talk(stall, 'buy', { item: it.name });
      return;
    }
    if (eq) {
      const [cat, id] = eq.dataset.equip.split(':');
      try { await Data.equip(cat, id); } catch { return; }
      pushEquip(); window.UHCoop?.lookChanged();
      renderTag(); renderStall();
      talk(stall, 'owned', { item: find(cat, id).name });
      return;
    }
    if (lookAt && Date.now() - browseT > 2500) {
      browseT = Date.now();
      const [cat, id] = lookAt.dataset.look.split(':');
      talk(stall, Data.profile.owned.includes(`${cat}:${id}`) ? 'owned' : 'browse', { item: find(cat, id).name });
    }
  });
  on('v-chat', 'submit', async (e) => {
    e.preventDefault();
    const text = $('v-say').value.trim();
    if (!text || !stall || talkBusy) return;
    talkBusy = true;
    $('v-say').value = '';
    $('v-you').textContent = 'You: ' + text;
    await talk(stall, 'chat', { text });
    setTimeout(() => (talkBusy = false), 1200);
  });
  document.addEventListener('keydown', (e) => { if (e.key === 'Escape' && stall) { e.stopPropagation(); closeStall(); } }, true);
  function enterHub() {
    settings.name = playerName(); saveSettings();
    runCtx = { mode: 'hub', chapter: 0, diff: settings.diff };
    state = 'hub';
    show(null);
    applySettings();
    pushEquip();
    send(`start|mode=hub;name=${playerName()};diff=${settings.diff};${equipString()}`);
  }
  on('hub-btn', 'click', enterHub);
  // ------------------------------------------------------------------ PWA: offline play + install
  if ('serviceWorker' in navigator && (location.protocol === 'https:' || location.hostname === 'localhost')) {
    navigator.serviceWorker.register('sw.js').catch((e) => console.warn('[sw]', e));
  }
  let installPrompt = null;
  window.addEventListener('beforeinstallprompt', (e) => {
    e.preventDefault();
    installPrompt = e;
    $('install-btn').style.display = '';
  });
  on('install-btn', 'click', async () => {
    if (!installPrompt) return;
    installPrompt.prompt();
    await installPrompt.userChoice.catch(() => null);
    installPrompt = null;
    $('install-btn').style.display = 'none';
  });
  const iOS = /iphone|ipad|ipod/i.test(navigator.userAgent) || (navigator.platform === 'MacIntel' && navigator.maxTouchPoints > 1);
  const standalone = navigator.standalone || matchMedia('(display-mode: standalone), (display-mode: fullscreen)').matches;
  if (iOS && !standalone && !settings.iosHint) {
    settings.iosHint = true; saveSettings();
    setTimeout(() => toast('📲 Tip: tap Share → "Add to Home Screen" to play full-screen'), 4000);
  }
  // lobby music picker (tunes live in src/audio.rs LOBBY_TUNES)
  on('tune-btn', 'click', () => { settings.tune = ((settings.tune || 0) + 1) % 4; saveSettings(); send('tune|' + settings.tune); });
  // party members walking around the hub, and their chat as speech bubbles
  setInterval(() => {
    if (state !== 'hub') return;
    const mates = window.UHCoop?.hubMates?.() || [];
    send('hubmates|' + mates.map((m) => [m.name, m.x, m.y, m.aim, m.look.skin, m.look.hat, m.look.gun].map((x) => String(x ?? '').replace(/[|~]/g, '')).join('~')).join('|'));
  }, 300);
  window.UHCoop?.on?.('chat', (m) => { if (state === 'hub' && m.name) send('hubbubble|' + String(m.name).replace(/\|/g, '') + '|' + String(m.text || '').replace(/\|/g, '/')); });
  on('continues', 'click', (e) => { const b = e.target.closest('[data-continue]'); if (b) continueRun(b.dataset.continue); });
  on('name', 'keydown', (e) => { if (e.key === 'Enter') start(); });
  on('resume', 'click', () => send('resume'));
  let afterSave = null;
  on('save', 'click', () => { afterSave = 'stay'; send('save'); });
  on('save-quit', 'click', () => { afterSave = 'quit'; send('save'); });
  on('quit', 'click', async () => {
    const coop = window.UHCoop?.inGame;
    if (!confirm(coop ? 'Leave the co-op game? Your coins are kept.' : 'Abandon this run? Coins earned so far are kept, but progress is lost.')) return;
    if (lastPause) await Data.finishRun(lastPause.run, true).catch((e) => console.warn(e));
    window.UHCoop?.gameEnded();
    toMenu();
  });
  let againGoesToLobby = false;
  on('again', 'click', () => (againGoesToLobby ? toMenu() : start()));
  on('tomenu', 'click', toMenu);
  on('next-ch', 'click', () => { settings.chapter = Math.min(settings.chapter + 1, STORY.length - 1); start(); });
  on('pause-btn', 'click', () => send('pause'));
  const tap = (id, cmd) => $(id).addEventListener('touchstart', (e) => { e.preventDefault(); send(cmd); }, { passive: false });
  tap('t-dash', 'dash'); tap('t-ult', 'ult'); tap('t-abil', 'act|ability');
  tap('t-ready', 'act|confirm');
  $('t-use').addEventListener('touchstart', (e) => { e.preventDefault(); send('use|1'); }, { passive: false });
  $('t-use').addEventListener('touchend', (e) => { e.preventDefault(); send('use|0'); }, { passive: false });

  window.addEventListener('keydown', (e) => {
    const typing = /^(INPUT|TEXTAREA|SELECT)$/.test(document.activeElement?.tagName || '');
    if (state === 'menu' && e.key === 'Enter' && !typing) start();
    else if (state === 'gameover' && e.key === 'Enter') start();
    else if (state === 'story' && e.key === 'Enter') $('story-go').click();
  });

  // ------------------------------------------------------------------ pause panel
  let lastPause = null;
  function renderPause(d) {
    lastPause = d;
    const r = d.run;
    const who = d.char ? `${d.char.icon} ${d.char.name} (${d.char.class})` : '';
    $('p-sub').textContent = (r.mode === 'campaign'
      ? `Chapter ${r.chapter + 1}: ${STORY[r.chapter].title} · ${d.diffName} · ${d.gunName}`
      : `Infinite · ${d.diffName} · ${d.gunName}`) + (who ? ` · ${who}` : '');
    $('p-hp').style.width = `${Math.max(0, (d.hp / d.maxHp) * 100)}%`;
    $('p-hp-t').textContent = `${d.hp} / ${d.maxHp}`;
    $('p-lvl').textContent = r.level;
    $('p-xp').style.width = `${Math.min(100, (d.xp / d.xpNext) * 100)}%`;
    $('p-xp-t').textContent = `${d.xp} / ${d.xpNext} XP`;
    $('p-wave').textContent = r.wave;
    $('p-kills').textContent = r.kills;
    $('p-coins').textContent = '🪙' + r.coins;
    $('p-dmg').textContent = d.dmg;
    $('p-rate').textContent = d.fireRate;
    $('p-crit').textContent = d.crit + '%';
    $('p-speed').textContent = d.speed;
    $('p-time').textContent = fmtTime(r.time);
    $('p-ult').textContent = d.ult + '%';
    $('p-ups').innerHTML = d.upgrades.length ? d.upgrades.map((u) =>
      `<div class="up"><span>${u.icon}</span><span>${esc(u.name)}</span><span class="pips">${Array.from({ length: u.max }, (_, i) => `<i style="${i < u.lvl ? 'background:' + u.color : ''}"></i>`).join('')}</span></div>`).join('')
      : '<div class="empty" style="padding:8px">No powers yet. Level up to pick some.</div>';
    const coopRun = !!window.UHCoop?.inGame;
    for (const id of ['save', 'save-quit']) { const el = $(id); if (el) el.closest('.tip-wrap, button').style.display = coopRun ? 'none' : ''; }
    $('quit').textContent = coopRun ? 'Leave co-op game' : 'Abandon run';
    $('save-hint').textContent = coopRun ? '⚠ Co-op games keep running while paused, so stay alert!' : 'Saved runs resume from the start of the current wave.';
    const slot = r.mode === 'campaign' ? 'campaign' : 'infinite';
    const label = slot === 'campaign' ? 'Campaign' : 'Infinite';
    const prev = Data.profile.saves[slot];
    $('save-tip').textContent = prev
      ? `⚠ Overwrites your saved ${label} run (Wave ${prev.summary.wave}, Lv ${prev.summary.level}). Your ${slot === 'campaign' ? 'Infinite' : 'Campaign'} save is untouched.`
      : `Saves to your ${label} slot. Your ${slot === 'campaign' ? 'Infinite' : 'Campaign'} save is untouched.`;
  }

  // ------------------------------------------------------------------ events from Rust
  window.uhEvent = async (kind, data) => {
    switch (kind) {
      case 'ready':
        $('name').value = settings.name || '';
        await Data.init();
        renderAccount();
        $('loader').classList.add('hide');
        applySettings();
        toMenu();
        break;
      case 'pause':
        state = 'paused';
        try { renderPause(JSON.parse(data)); } catch (e) { console.warn(e); }
        // Outbreak runs can't be saved mid-match (yet)
        for (const id of ['save', 'save-quit']) $(id).disabled = ['outbreak', 'siege'].includes(runCtx.mode) || !!window.UHCoop?.inGame;
        applySettings();
        show('pause');
        break;
      case 'resume': state = 'playing'; show(null); break;
      case 'crash': {
        reportError('panic', data, '');
        state = 'crash';
        show('auth');
        $('auth-title').textContent = 'Something went wrong';
        $('auth-body').innerHTML = `<p class="sub">The game hit an error and stopped. Your account progress is safe. Reload to keep playing.</p>
          <button class="btn" onclick="location.reload()">↻ Reload</button>
          <details style="margin-top:12px;color:var(--muted);font-size:12px"><summary>Details</summary><pre style="white-space:pre-wrap">${esc(data)}</pre></details>`;
        break;
      }
      case 'autoaim': settings.autoaim = data === '1'; applySettings(); break;
      case 'say': {
        if (settings.subtitles) showSub('📢 ' + String(data).replace(/^./, (c) => c.toUpperCase()));
        if (!settings.announcer || !window.speechSynthesis) break;
        try {
          const u = new SpeechSynthesisUtterance(String(data));
          u.pitch = 0.3; u.rate = 0.95; u.volume = Math.min(1, (settings.vol ?? 0.7) * 1.1);
          const v = speechSynthesis.getVoices().find((x) => /en[-_]/i.test(x.lang) && /male|daniel|david|fred|google uk/i.test(x.name));
          if (v) u.voice = v;
          speechSynthesis.cancel();
          speechSynthesis.speak(u);
        } catch {}
        break;
      }
      case 'siegestars': {
        // best stars per map; the first clear of a map is celebrated
        const [map, n, glass] = String(data).split(':');
        settings.siegeStars = settings.siegeStars || {};
        const first = !settings.siegeStars[map];
        settings.siegeStars[map] = Math.max(settings.siegeStars[map] || 0, +n);
        saveSettings();
        toast(first ? `🏰 First clear! ${'★'.repeat(+n)}${glass === '1' ? ' · Glass Crystal' : ''}` : `${'★'.repeat(+n)} on this map`);
        break;
      }
      case 'hubopen': openStall(data); break;
      case 'hubleave': closeStall(true); break;
      case 'hubpos': {
        const [x, y, aim] = String(data).split('~').map(Number);
        window.UHCoop?.hubPos?.(x, y, aim);
        break;
      }
      case 'hubmenu':
        if (stall) { closeStall(); break; }
        if (confirm('Leave the Home Base and return to the lobby?')) { toMenu(); }
        break;
      case 'tune': $('tune-btn').textContent = `🎵 ${data} ▸`; break;
      case 'achievement': {
        const a = window.UHProg.ACHIEVEMENTS.find((x) => x.id === data);
        if (a && await Data.eventAchievement(data)) { toast(`🏅 Achievement: ${a.icon} ${a.name}`); send('sfx|ready'); }
        break;
      }
      case 'bindings': renderBinds(String(data)); break;
      case 'tutorial': settings.tutorialDone = true; saveSettings(); break;
      case 'usebtn': $('t-use').classList.toggle('on', data === '1'); break;
      case 'readybtn': $('t-ready').classList.toggle('on', data === '1'); break;
      case 'haptic': {
        const [ms, str] = String(data).split(':').map(Number);
        const k = settings.haptics ?? 0.6;
        if (k <= 0) break;
        try { navigator.vibrate?.(Math.round(ms * k)); } catch {}
        for (const gp of navigator.getGamepads?.() || []) {
          gp?.vibrationActuator?.playEffect?.('dual-rumble', { duration: ms, strongMagnitude: str * k, weakMagnitude: Math.min(1, str * k * 1.2) }).catch?.(() => {});
        }
        break;
      }
      case 'netversion': {
        // a teammate runs a different build: leave before the sims desync
        const [mine, theirs] = String(data).split(':').map(Number);
        toast(theirs > mine ? 'A teammate has a newer version. Refresh the page to update, then rejoin.' : 'A teammate is on an older version. Ask them to refresh the page.');
        window.UHCoop?.gameEnded();
        toMenu();
        break;
      }
      case 'saved': {
        const r = lastPause ? lastPause.run : {};
        const summary = { mode: r.mode || runCtx.mode, chapter: r.chapter || 0, diff: r.diff || runCtx.diff, wave: r.wave || 1, level: r.level || 1, date: Date.now() };
        try { await Data.putSave(data, summary); } catch (e) { console.warn(e); afterSave = null; toast('Save failed. Check your connection.'); break; }
        if (afterSave === 'quit') { afterSave = null; toast('Game saved'); toMenu(); }
        else { afterSave = null; toast('Game saved 💾'); $('save-hint').textContent = '✓ Saved. Continue it from the main menu any time.'; }
        break;
      }
      case 'gameover':
      case 'victory': {
        const s = JSON.parse(data);
        const win = kind === 'victory';
        const wasCoop = !!window.UHCoop?.inGame;
        window.UHCoop?.gameEnded();
        $('again').textContent = wasCoop ? '👥 Back to party' : '↻ PLAY AGAIN';
        againGoesToLobby = wasCoop;
        let result = { coins: s.coins, rank: null, id: null };
        const prevBest = s.mode === 'infinite' ? await personalBest(s.name) : 0;
        const top = s.mode === 'infinite' ? (await Data.leaderboard('all').catch(() => []))[0] : null;
        try { result = await Data.finishRun(s); } catch (e) { console.warn(e); toast('Could not reach the server; run not recorded.'); }
        $('go-title').textContent = win ? 'VICTORY' : 'YOU DIED';
        $('go-title').classList.toggle('win', win);
        $('earned').textContent = result.coins ? `+🪙 ${result.coins} coins` : '';
        const cr = result.career;
        if (cr) {
          const bits = [`+${cr.xp.toLocaleString()} XP`];
          if (cr.levelsGained?.length) bits.push(`⬆️ Account level ${cr.levelsGained[cr.levelsGained.length - 1]} (+🪙 ${cr.coins})`);
          if (cr.relics?.length) bits.push(`🏺 ${cr.relics.length} relic${cr.relics.length > 1 ? 's' : ''} found`);
          if (cr.live?.challenges?.length) bits.push(`🎯 ${cr.live.challenges.length} challenge${cr.live.challenges.length > 1 ? 's' : ''} done`);
          if (cr.live?.tiers?.length) bits.push(`🌸 Season tier ${cr.live.tiers[cr.live.tiers.length - 1]}`);
          $('earned').textContent += ($('earned').textContent ? ' · ' : '') + bits.join(' · ');
          const P = window.UHProg;
          (cr.achievements || []).forEach((id, i) => {
            const a = P.ACHIEVEMENTS.find((x) => x.id === id);
            if (a) setTimeout(() => { toast(`🏅 Achievement: ${a.icon} ${a.name}`); send('sfx|ready'); }, 900 + i * 1600);
          });
        }
        $('s-wave').textContent = s.wave;
        $('s-level').textContent = s.level;
        $('s-kills').textContent = s.kills;
        $('s-score').textContent = (s.score || 0).toLocaleString();
        $('s-time').textContent = fmtTime(s.time);
        $('next-ch').style.display = 'none';
        let rec = '';
        if (s.mode === 'infinite') {
          const entry = { wave: s.wave, level: s.level, kills: s.kills, time: s.time };
          if (!top || cmpScore(entry, top) < 0) rec = '👑 NEW WORLD RECORD! 👑';
          else if (s.wave > prevBest) rec = '★ NEW PERSONAL BEST ★';
          else if (result.rank && result.rank <= 10) rec = `Ranked #${result.rank} on the leaderboard`;
          if (Data.globalBoard && !Data.cloud) rec = (rec ? rec + ' · ' : '') + 'Sign in to post runs globally';
          $('go-lb-wrap').style.display = '';
          await renderLB($('lb-over'), result.id);
        } else {
          $('go-lb-wrap').style.display = 'none';
          if (win) {
            rec = `Chapter ${s.chapter + 1} cleared!`;
            await showStory(s.chapter, 'outro', 'Continue ▶');
            if (s.chapter + 1 < STORY.length) $('next-ch').style.display = '';
          } else {
            rec = `Chapter ${s.chapter + 1}: ${STORY[s.chapter].title}`;
          }
        }
        renderCoins();
        $('record').textContent = rec;
        state = 'gameover';
        show('gameover');
        break;
      }
    }
  };

  // ------------------------------------------------------------------ account
  function renderAccount() {
    const signedIn = !!Data.cloud;
    $('account-name').textContent = signedIn ? Data.profile.name : (window.UHCloud ? 'Sign in' : 'Offline');
    if (signedIn && Data.profile.name) { $('name').value = Data.profile.name; settings.name = Data.profile.name; }
    window.UHCoop?.onAuthChanged();
  }
  function openAccount() {
    state = 'auth';
    show('auth');
    const body = $('auth-body');
    if (!window.UHCloud) {
      $('auth-title').textContent = 'Account';
      body.innerHTML = '<p class="sub">Cloud accounts are not configured for this build. Progress is saved in this browser.</p>';
      return;
    }
    if (Data.cloud) {
      const p = Data.profile, u = Data.cloud.user;
      $('auth-title').textContent = 'Your account';
      body.innerHTML = `<p class="sub">Signed in as <b>${esc(p.name)}</b> (${esc(u.email || '')}). Coins, items, campaign progress and saves sync across devices.</p>
        <div class="kv"><div class="stat"><div class="v">${p.stats.runs}</div><div class="k">Runs</div></div>
        <div class="stat"><div class="v">${p.stats.kills.toLocaleString()}</div><div class="k">Kills</div></div>
        <div class="stat"><div class="v">${p.stats.bestWave}</div><div class="k">Best wave</div></div></div>
        <div class="row"><button class="btn secondary" id="acct-out">Sign out</button></div>`;
      $('acct-out').onclick = async () => { await Data.cloud.signOut(); await Data.reload(); renderAccount(); toast('Signed out'); toMenu(); };
      return;
    }
    let mode = 'in';
    let resetEmail = '';
    const drawReset = () => {
      const step2 = mode === 'reset2';
      $('auth-title').textContent = 'Reset password';
      body.innerHTML = `<p class="sub">${step2 ? `We emailed a 6-digit code to <b>${esc(resetEmail)}</b>. Check your spam folder too.` : "Enter your account email and we'll send you a reset code."}</p>
        <form id="reset-form">
          ${step2
            ? `<label class="field">Code</label><input type="text" id="reset-otp" inputmode="numeric" maxlength="6" required autocomplete="one-time-code" />
               <label class="field">New password</label><input type="password" id="reset-pass" minlength="8" required autocomplete="new-password" />`
            : `<label class="field">Email</label><input type="email" id="reset-email" required autocomplete="email" />`}
          <div class="err" id="acct-err"></div>
          <button class="btn" id="reset-go" type="submit">${step2 ? 'Set new password' : 'Send code'}</button>
        </form>
        <div class="hint" style="text-align:center;margin-top:12px"><a href="#" id="reset-back" style="color:var(--accent)">Back to sign in</a></div>`;
      $('reset-back').onclick = (e) => { e.preventDefault(); mode = 'in'; draw(); };
      $('reset-form').onsubmit = async (e) => {
        e.preventDefault();
        const btn = $('reset-go');
        btn.disabled = true;
        $('acct-err').textContent = '';
        try {
          if (!step2) {
            resetEmail = $('reset-email').value.trim();
            await window.UHCloud.requestReset(resetEmail);
            mode = 'reset2';
            drawReset();
          } else {
            const pass = $('reset-pass').value;
            await window.UHCloud.resetPassword(resetEmail, $('reset-otp').value.trim(), pass);
            await window.UHCloud.signIn(resetEmail, pass);
            await Data.reload();
            renderAccount();
            toast('Password updated. Welcome back!');
            toMenu();
          }
        } catch (err) {
          $('acct-err').textContent = err.message;
          btn.disabled = false;
        }
      };
    };
    const draw = () => {
      if (mode === 'reset1' || mode === 'reset2') return drawReset();
      $('auth-title').textContent = mode === 'in' ? 'Sign in' : 'Create account';
      body.innerHTML = `<p class="sub">Save coins, items, campaign progress and runs to the cloud, and compete on the global leaderboard.</p>
        <form id="acct-form">
          ${mode === 'up' ? '<label class="field">Display name</label><input type="text" id="acct-name" maxlength="16" required autocomplete="nickname" />' : ''}
          <label class="field">Email</label><input type="email" id="acct-email" required autocomplete="email" />
          <label class="field">Password</label><input type="password" id="acct-pass" required minlength="8" autocomplete="${mode === 'in' ? 'current-password' : 'new-password'}" />
          <div class="err" id="acct-err"></div>
          <button class="btn" id="acct-go" type="submit">${mode === 'in' ? 'Sign in' : 'Create account'}</button>
        </form>
        <div class="row"><button class="btn secondary" id="acct-google">Continue with Google</button></div>
        <div class="hint" style="text-align:center;margin-top:12px">${mode === 'in' ? 'New here?' : 'Have an account?'} <a href="#" id="acct-switch" style="color:var(--accent)">${mode === 'in' ? 'Create an account' : 'Sign in'}</a></div>
        ${mode === 'in' ? '<div class="hint" style="text-align:center;margin-top:6px"><a href="#" id="acct-forgot" style="color:var(--muted)">Forgot password?</a></div>' : ''}`;
      if (mode === 'in') $('acct-forgot').onclick = (e) => { e.preventDefault(); mode = 'reset1'; draw(); };
      if (mode === 'up' && settings.name) $('acct-name').value = settings.name;
      $('acct-switch').onclick = (e) => { e.preventDefault(); mode = mode === 'in' ? 'up' : 'in'; draw(); };
      $('acct-google').onclick = async () => { try { await window.UHCloud.google(); } catch (err) { $('acct-err').textContent = err.message; } };
      $('acct-form').onsubmit = async (e) => {
        e.preventDefault();
        const btn = $('acct-go');
        btn.disabled = true; btn.textContent = 'Please wait…';
        $('acct-err').textContent = '';
        try {
          const email = $('acct-email').value.trim(), pass = $('acct-pass').value;
          if (mode === 'in') await window.UHCloud.signIn(email, pass);
          else await window.UHCloud.signUp($('acct-name').value.trim(), email, pass);
          await Data.reload();
          renderAccount();
          toast(`Welcome, ${Data.profile.name}!`);
          toMenu();
        } catch (err) {
          $('acct-err').textContent = err.message;
          btn.disabled = false; btn.textContent = mode === 'in' ? 'Sign in' : 'Create account';
        }
      };
    };
    draw();
  }
  on('account-pill', 'click', openAccount);
  on('auth-x', 'click', () => toMenu());
  // ---------------------------------------------------------------- phone lobby nav
  // On narrow screens the lobby shows one panel at a time (CSS), picked here.
  function setMobileTab(m) {
    document.body.dataset.mtab = m;
    for (const b of document.querySelectorAll('#mnav button')) b.classList.toggle('on', b.dataset.m === m);
    if (m !== 'play') document.querySelector(`#tabs button[data-tab="${m}"]`)?.click();
    document.querySelector('.side-tabs')?.scrollTo({ top: 0 });
  }
  on('mnav', 'click', (e) => { const b = e.target.closest('button'); if (b) setMobileTab(b.dataset.m); });
  setMobileTab('play');

  window.UHDiffName = (d) => DIFF[d]?.name || d;
  window.UHUI = {
    send,
    equipString,
    startCoop(o) {
      runCtx = { mode: 'infinite', chapter: 0, diff: settings.diff, coop: true };
      state = 'playing';
      show(null);
      applySettings();
      pushEquip();
      send('coop|' + o);
    },
    setDiff(diff, locked) {
      if (diff) settings.diff = diff;
      if (diff && settings.mode !== 'infinite') settings.mode = 'infinite';
      for (const b of $('diff-seg').children) b.disabled = !!locked;
      renderMode();
    }, show, toMenu, toast, renderLB, renderCoins, renderMode, renderTag, pushEquip, state: () => state, setState: (s) => { state = s; } };
})();
