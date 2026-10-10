// Undead Horizon UI layer: menus, shop, locker, campaign story, pause/save,
// leaderboard. The game itself (Rust/WASM) talks to this file through
// window.uhPoll (commands JS -> Rust) and window.uhEvent (events Rust -> JS).
(() => {
  'use strict';
  const $ = (id) => document.getElementById(id);
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
  const CAT_LABEL = { gun: '🔫 Guns', perk: '✨ Perks', skin: '👕 Skins', hat: '🎩 Hats', tracer: '☄️ Tracers' };
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
    owned: ['gun:pistol', 'perk:none', 'skin:default', 'hat:none', 'tracer:default'],
    equipped: { gun: 'pistol', perk: 'none', skin: 'default', hat: 'none', tracer: 'default' },
    campaign: { unlocked: 0, cleared: [] },
    save: null,
    stats: { runs: 0, kills: 0, bestWave: 0 },
  });
  const normalizeProfile = (p) => {
    const f = freshProfile();
    p = Object.assign(f, p || {});
    p.equipped = Object.assign(freshProfile().equipped, p.equipped || {});
    p.campaign = Object.assign({ unlocked: 0, cleared: [] }, p.campaign || {});
    p.stats = Object.assign({ runs: 0, kills: 0, bestWave: 0 }, p.stats || {});
    for (const o of freshProfile().owned) if (!p.owned.includes(o)) p.owned.push(o);
    return p;
  };

  // Local backend (always available). A cloud backend can replace it when signed in.
  const LocalBackend = {
    name: 'local',
    async loadProfile() { return normalizeProfile(store.get(PROFILE_KEY, null)); },
    async saveProfile(p) { store.set(PROFILE_KEY, p); },
    async leaderboard(filter) {
      let lb = store.get(LB_KEY, []);
      if (filter && filter !== 'all') lb = lb.filter((e) => (e.diff || 'survivor') === filter);
      return lb.slice(0, 10);
    },
    async submitScore(entry) {
      const lb = store.get(LB_KEY, []);
      lb.push(entry);
      lb.sort(cmpScore);
      store.set(LB_KEY, lb.slice(0, 100));
      return lb.indexOf(entry) + 1;
    },
  };
  const cmpScore = (a, b) => b.wave - a.wave || b.level - a.level || b.kills - a.kills || a.time - b.time;

  // window.UHCloud (ui/cloud.js-style module, optional) may register a cloud backend.
  const Data = {
    backend: LocalBackend,
    profile: null,
    async init() {
      if (window.UHCloud && (await window.UHCloud.restore?.())) this.backend = window.UHCloud;
      this.profile = await this.backend.loadProfile();
    },
    async save() { await this.backend.saveProfile(this.profile); },
  };
  window.UHData = Data;

  // ------------------------------------------------------------------ settings
  const settings = Object.assign({ vol: 0.7, music: 0.5, autoaim: false, name: '', mode: 'infinite', diff: 'survivor', chapter: 0 }, store.get(SET_KEY, {}));
  const saveSettings = () => store.set(SET_KEY, settings);
  function applySettings() {
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
    return `gun=${e.gun};perk=${e.perk === 'none' ? '' : e.perk};skin=${e.skin};hat=${e.hat};tracer=${e.tracer}`;
  };
  const pushEquip = () => send('equip|' + equipString());

  // ------------------------------------------------------------------ screens
  let state = 'loading';
  function show(id) {
    for (const o of ['menu', 'pause', 'gameover', 'story', 'auth']) $(o).classList.toggle('show', o === id);
    document.body.classList.toggle('playing', id === null);
  }

  // ------------------------------------------------------------------ lobby
  function renderCoins() { $('coins').textContent = Data.profile.coins.toLocaleString(); }
  function renderTag() {
    const e = Data.profile.equipped;
    $('tag-name').textContent = ($('name').value || '').trim() || 'Survivor';
    $('tag-loadout').textContent = `${find('gun', e.gun).name} · ${find('perk', e.perk).name}`;
  }
  function renderMode() {
    for (const b of $('mode-seg').children) b.classList.toggle('on', b.dataset.mode === settings.mode);
    for (const b of $('diff-seg').children) b.classList.toggle('on', b.dataset.diff === settings.diff);
    $('diff-hint').textContent = DIFF[settings.diff].hint;
    const camp = settings.mode === 'campaign';
    $('chapter-box').style.display = camp ? '' : 'none';
    $('mode-hint').textContent = camp ? 'Story chapters with unique boss fights.' : 'Endless waves. Bosses every 5 waves. Ranked on the leaderboard.';
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
    const sv = Data.profile.save;
    $('continue').style.display = sv ? '' : 'none';
    $('continue-hint').textContent = sv ? `${sv.summary.mode === 'campaign' ? 'Chapter ' + (sv.summary.chapter + 1) + ' · ' : ''}Wave ${sv.summary.wave} · Lv ${sv.summary.level} · ${DIFF[sv.summary.diff]?.name || ''} · saved ${new Date(sv.summary.date).toLocaleDateString()}` : '';
    saveSettings();
  }
  $('mode-seg').addEventListener('click', (e) => { const b = e.target.closest('button'); if (b) { settings.mode = b.dataset.mode; renderMode(); send('equip|'); } });
  $('diff-seg').addEventListener('click', (e) => { const b = e.target.closest('button'); if (b) { settings.diff = b.dataset.diff; renderMode(); } });
  $('chapters').addEventListener('click', (e) => {
    const c = e.target.closest('.chapter');
    if (!c || c.classList.contains('locked')) return;
    settings.chapter = +c.dataset.ch;
    renderMode();
  });
  $('name').addEventListener('input', () => { settings.name = $('name').value; saveSettings(); renderTag(); });

  // tabs
  $('tabs').addEventListener('click', (e) => {
    const b = e.target.closest('button');
    if (!b) return;
    for (const x of $('tabs').children) x.classList.toggle('on', x === b);
    for (const t of ['lb', 'shop', 'locker', 'settings']) $('tab-' + t).classList.toggle('on', t === b.dataset.tab);
    if (b.dataset.tab === 'shop') renderShop();
    if (b.dataset.tab === 'locker') renderLocker();
    if (b.dataset.tab === 'lb') renderLB($('lb-menu'));
  });

  // ------------------------------------------------------------------ shop
  let shopCat = 'gun';
  $('shop-cats').innerHTML = Object.keys(CATALOG).map((c) => `<button data-c="${c}" class="${c === shopCat ? 'on' : ''}">${CAT_LABEL[c]}</button>`).join('');
  $('shop-cats').addEventListener('click', (e) => {
    const b = e.target.closest('button');
    if (!b) return;
    shopCat = b.dataset.c;
    for (const x of $('shop-cats').children) x.classList.toggle('on', x === b);
    renderShop();
  });
  const visual = (cat, it) => it.color ? `<div class="sw" style="background:${it.color}"></div>` : `<div class="ic">${it.icon}</div>`;
  const rarityTag = (it) => it.rarity === 'epic' ? '<span class="rar" style="color:var(--epic)">EPIC</span>' : '';
  function renderShop() {
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
  $('shop-items').addEventListener('click', async (e) => {
    const b = e.target.closest('[data-buy]');
    if (!b) return;
    const it = find(shopCat, b.dataset.buy);
    const p = Data.profile;
    const key = `${shopCat}:${it.id}`;
    if (p.owned.includes(key) || p.coins < it.price) return;
    p.coins -= it.price;
    p.owned.push(key);
    p.equipped[shopCat] = it.id; // auto-equip new purchases
    await Data.save();
    pushEquip();
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
    $('locker').innerHTML =
      `<h2>Loadout</h2>${group('gun', 'Primary weapon')}${group('perk', 'Starting perk')}
       <h2 style="margin-top:20px">Character</h2>${group('skin', 'Outfit')}${group('hat', 'Headgear')}${group('tracer', 'Bullet tracers')}
       <div class="hint" style="margin-top:12px">Buy more in the 🛒 Shop with coins earned by surviving waves.</div>`;
  }
  $('locker').addEventListener('click', async (e) => {
    const it = e.target.closest('[data-eq]');
    if (!it) return;
    const [cat, id] = it.dataset.eq.split(':');
    Data.profile.equipped[cat] = id;
    await Data.save();
    pushEquip();
    renderLocker(); renderTag();
  });

  // ------------------------------------------------------------------ leaderboard
  let lbFilter = 'all';
  $('lb-filter').addEventListener('click', (e) => {
    const b = e.target.closest('button');
    if (!b) return;
    lbFilter = b.dataset.f;
    for (const x of $('lb-filter').children) x.classList.toggle('on', x === b);
    renderLB($('lb-menu'));
  });
  async function renderLB(el, highlightId) {
    let lb = [];
    try { lb = await Data.backend.leaderboard(el.id === 'lb-menu' ? lbFilter : 'all'); } catch (err) { console.warn(err); lb = await LocalBackend.leaderboard(lbFilter); }
    $('lb-live').style.display = Data.backend.name === 'cloud' ? '' : 'none';
    if (!lb.length) { el.innerHTML = '<div class="empty">No runs yet. Be the first legend.</div>'; return; }
    el.innerHTML = '<table class="lb"><thead><tr><th>#</th><th>Name</th><th>Wave</th><th>Lv</th><th>Kills</th><th>Diff</th></tr></thead><tbody>' +
      lb.map((e, i) => `<tr class="${e.id === highlightId ? 'me' : ''}"><td class="rank">${i + 1}</td><td class="name">${esc(e.name)}</td><td class="wave">${e.wave}</td><td>${e.level}</td><td>${e.kills}</td><td><span class="dtag ${esc(e.diff || 'survivor')}">${esc(DIFF[e.diff || 'survivor']?.name || '')}</span></td></tr>`).join('') +
      '</tbody></table>';
  }
  // Live refresh while the leaderboard is visible on the menu.
  setInterval(() => { if (state === 'menu' && $('tab-lb').classList.contains('on') && Data.backend.name === 'cloud') renderLB($('lb-menu')); }, 10000);

  // ------------------------------------------------------------------ flow
  function toMenu() {
    state = 'menu';
    renderCoins(); renderTag(); renderMode();
    renderLB($('lb-menu'));
    pushEquip();
    send('menu');
    show('menu');
  }
  function playerName() { return ($('name').value || '').trim().replace(/[|;=]/g, '') || 'Survivor'; }
  async function personalBest(name) {
    const lb = store.get(LB_KEY, []);
    return lb.filter((e) => e.name === name).reduce((m, e) => Math.max(m, e.wave), Data.profile.stats.bestWave || 0);
  }

  let storyResolve = null;
  let storyTimer = 0;
  function showStory(i, which, btnLabel) {
    const s = STORY[i];
    $('story-ch').textContent = which === 'outro' ? `CHAPTER ${i + 1} COMPLETE` : `CHAPTER ${i + 1}`;
    $('story-title').textContent = s.title;
    $('story-go').textContent = btnLabel;
    // typewriter, HTML-aware (speaker tags appear whole)
    const html = s[which];
    const el = $('story-text');
    el.innerHTML = '';
    let k = 0;
    clearInterval(storyTimer);
    storyTimer = setInterval(() => {
      if (k >= html.length) { clearInterval(storyTimer); return; }
      if (html[k] === '<') k = html.indexOf('>', k) + 1;
      else k += 2;
      el.innerHTML = html.slice(0, k);
    }, 16);
    $('story-skip').onclick = () => { clearInterval(storyTimer); el.innerHTML = html; };
    state = 'story';
    show('story');
    return new Promise((res) => { storyResolve = res; });
  }
  $('story-go').addEventListener('click', () => { clearInterval(storyTimer); const r = storyResolve; storyResolve = null; r && r(); });

  let runCtx = null;
  async function start() {
    const name = playerName();
    settings.name = name; saveSettings();
    $('name').blur();
    if (settings.mode === 'campaign') await showStory(settings.chapter, 'intro', 'Begin ▶');
    const best = settings.mode === 'infinite' ? await personalBest(name) : 0;
    runCtx = { mode: settings.mode, chapter: settings.chapter, diff: settings.diff };
    state = 'playing';
    show(null);
    applySettings();
    pushEquip();
    send(`start|name=${name};best=${best};mode=${settings.mode};chapter=${settings.chapter};diff=${settings.diff};${equipString()}`);
  }
  async function continueRun() {
    const sv = Data.profile.save;
    if (!sv) return;
    Data.profile.save = null; // a save resumes once
    await Data.save();
    runCtx = { mode: sv.summary.mode, chapter: sv.summary.chapter, diff: sv.summary.diff };
    state = 'playing';
    show(null);
    applySettings();
    pushEquip();
    send('load|' + sv.data);
  }

  $('play').addEventListener('click', start);
  $('continue').addEventListener('click', continueRun);
  $('name').addEventListener('keydown', (e) => { if (e.key === 'Enter') start(); });
  $('resume').addEventListener('click', () => send('resume'));
  let afterSave = null;
  $('save').addEventListener('click', () => { afterSave = 'stay'; send('save'); });
  $('save-quit').addEventListener('click', () => { afterSave = 'quit'; send('save'); });
  $('quit').addEventListener('click', async () => {
    if (!confirm('Abandon this run? Coins earned so far are kept, but progress is lost.')) return;
    if (lastPause) await awardCoins(lastPause.run.coins);
    toMenu();
  });
  $('again').addEventListener('click', start);
  $('tomenu').addEventListener('click', toMenu);
  $('next-ch').addEventListener('click', () => { settings.chapter = Math.min(settings.chapter + 1, STORY.length - 1); start(); });
  $('pause-btn').addEventListener('click', () => send('pause'));
  const tap = (id, cmd) => $(id).addEventListener('touchstart', (e) => { e.preventDefault(); send(cmd); }, { passive: false });
  tap('t-dash', 'dash'); tap('t-ult', 'ult');

  window.addEventListener('keydown', (e) => {
    if (state === 'menu' && e.key === 'Enter' && document.activeElement !== $('name')) start();
    else if (state === 'gameover' && e.key === 'Enter') start();
    else if (state === 'story' && e.key === 'Enter') $('story-go').click();
  });

  async function awardCoins(n) {
    if (!n) return;
    Data.profile.coins += n;
    await Data.save();
  }

  // ------------------------------------------------------------------ pause panel
  let lastPause = null;
  function renderPause(d) {
    lastPause = d;
    const r = d.run;
    $('p-sub').textContent = r.mode === 'campaign'
      ? `Chapter ${r.chapter + 1}: ${STORY[r.chapter].title} · ${d.diffName} · ${d.gunName}`
      : `Infinite · ${d.diffName} · ${d.gunName}`;
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
    $('save-hint').textContent = 'Saved runs resume from the start of the current wave.';
  }

  // ------------------------------------------------------------------ events from Rust
  window.uhEvent = async (kind, data) => {
    switch (kind) {
      case 'ready':
        await Data.init();
        $('name').value = settings.name || '';
        $('loader').classList.add('hide');
        applySettings();
        toMenu();
        break;
      case 'pause':
        state = 'paused';
        try { renderPause(JSON.parse(data)); } catch (e) { console.warn(e); }
        applySettings();
        show('pause');
        break;
      case 'resume': state = 'playing'; show(null); break;
      case 'autoaim': settings.autoaim = data === '1'; applySettings(); break;
      case 'saved': {
        const r = lastPause ? lastPause.run : {};
        Data.profile.save = {
          data,
          summary: { mode: r.mode || runCtx.mode, chapter: r.chapter || 0, diff: r.diff || runCtx.diff, wave: r.wave || 1, level: r.level || 1, date: Date.now() },
        };
        await Data.save();
        if (afterSave === 'quit') { afterSave = null; toast('Game saved'); toMenu(); }
        else { afterSave = null; toast('Game saved 💾'); $('save-hint').textContent = '✓ Saved. Continue it from the main menu any time.'; }
        break;
      }
      case 'gameover':
      case 'victory': {
        const s = JSON.parse(data);
        const win = kind === 'victory';
        const p = Data.profile;
        p.coins += s.coins;
        p.stats.runs += 1;
        p.stats.kills += s.kills;
        $('go-title').textContent = win ? 'VICTORY' : 'YOU DIED';
        $('go-title').classList.toggle('win', win);
        $('earned').textContent = s.coins ? `+🪙 ${s.coins} coins` : '';
        $('s-wave').textContent = s.wave;
        $('s-level').textContent = s.level;
        $('s-kills').textContent = s.kills;
        $('s-time').textContent = fmtTime(s.time);
        $('next-ch').style.display = 'none';
        let rec = '';
        if (s.mode === 'infinite') {
          const prevBest = await personalBest(s.name);
          const top = (await Data.backend.leaderboard('all').catch(() => []))[0];
          const entry = { name: s.name, wave: s.wave, level: s.level, kills: s.kills, time: s.time, diff: s.diff, gun: s.gun, id: Date.now() + Math.random(), date: new Date().toISOString() };
          const rank = await Data.backend.submitScore(entry).catch(() => LocalBackend.submitScore(entry));
          if (Data.backend !== LocalBackend) await LocalBackend.submitScore({ ...entry });
          p.stats.bestWave = Math.max(p.stats.bestWave || 0, s.wave);
          if (!top || cmpScore(entry, top) < 0) rec = '👑 NEW WORLD RECORD! 👑';
          else if (s.wave > prevBest) rec = '★ NEW PERSONAL BEST ★';
          else if (rank && rank <= 10) rec = `Ranked #${rank} on the leaderboard`;
          $('go-lb-wrap').style.display = '';
          await renderLB($('lb-over'), entry.id);
        } else {
          $('go-lb-wrap').style.display = 'none';
          if (win) {
            const c = p.campaign;
            if (!c.cleared.includes(s.chapter)) c.cleared.push(s.chapter);
            c.unlocked = Math.max(c.unlocked, Math.min(s.chapter + 1, STORY.length - 1));
            rec = `Chapter ${s.chapter + 1} cleared!`;
            await Data.save();
            await showStory(s.chapter, 'outro', 'Continue ▶');
            if (s.chapter + 1 < STORY.length) $('next-ch').style.display = '';
          } else {
            rec = `Chapter ${s.chapter + 1}: ${STORY[s.chapter].title}`;
          }
        }
        await Data.save();
        $('record').textContent = rec;
        state = 'gameover';
        show('gameover');
        break;
      }
    }
  };

  // ------------------------------------------------------------------ account modal (cloud hooks)
  $('account-pill').addEventListener('click', () => {
    if (window.UHCloud) window.UHCloud.openAccount();
    else { show('auth'); state = 'auth'; $('auth-body').innerHTML = '<p class="sub">Cloud accounts are not configured for this build. Progress is saved in this browser.</p>'; }
  });
  $('auth-x').addEventListener('click', () => toMenu());
  window.UHUI = { show, toMenu, toast, renderLB, renderCoins, renderMode, renderTag, pushEquip, state: () => state, setState: (s) => { state = s; } };
})();
