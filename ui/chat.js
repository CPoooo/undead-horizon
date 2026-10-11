// Party text chat: a log + input in the lobby party panel, and a fading feed
// with a T-to-type input during matches. Messages travel over the social
// socket (server filters, rate-limits and drops blocked senders).
// Clicking a player's name opens a small menu: volume (voice), block, report.
(() => {
  'use strict';
  const $ = (id) => document.getElementById(id);
  const esc = (s) => String(s ?? '').replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));
  const C = () => window.UHCoop;
  const MAX = 60;
  const FADE = 9000;

  const msgs = []; // { from, name, text, ts, sys }
  let typing = false;

  const fmt = (m) => m.sys
    ? `<div class="cm sys">${esc(m.text)}</div>`
    : `<div class="cm"><b class="${m.from === C()?.me?.id ? 'me' : 'clickable'}" ${m.from !== C()?.me?.id ? `data-pmenu="${esc(m.from)}" data-pname="${esc(m.name)}"` : ''}>${esc(m.name)}</b> ${esc(m.text)}</div>`;

  function push(m) {
    msgs.push(m);
    if (msgs.length > MAX) msgs.shift();
    renderLobby();
    renderGame();
  }

  // ---------------------------------------------------------------- lobby panel
  function renderLobby() {
    const box = $('party-chat');
    if (!box) return;
    const inParty = !!C()?.party;
    box.style.display = inParty ? '' : 'none';
    if (!inParty) return;
    const log = $('pc-log');
    const stick = log.scrollTop + log.clientHeight >= log.scrollHeight - 8;
    log.innerHTML = msgs.length ? msgs.map(fmt).join('') : '<div class="cm sys">Say hi to your squad 👋</div>';
    if (stick) log.scrollTop = log.scrollHeight;
  }

  // ---------------------------------------------------------------- in-game feed
  function renderGame() {
    const feed = $('gc-feed');
    if (!feed) return;
    const now = Date.now();
    const show = typing ? msgs.slice(-10) : msgs.filter((m) => now - m.ts < FADE).slice(-5);
    feed.innerHTML = show.map(fmt).join('');
    $('game-chat').classList.toggle('typing', typing);
  }
  setInterval(() => { if (document.body.classList.contains('playing') && msgs.length) renderGame(); }, 1000);

  function openInput() {
    if (!C()?.party || typing) return;
    typing = true;
    $('game-chat').classList.add('typing');
    const inp = $('gc-input');
    inp.value = '';
    renderGame();
    setTimeout(() => inp.focus(), 0);
  }
  function closeInput() {
    typing = false;
    $('gc-input').blur();
    renderGame();
  }

  function send(text) {
    text = String(text || '').trim();
    if (!text) return false;
    if (text.length > 200) text = text.slice(0, 200);
    return C()?.wsSend({ t: 'chat', text });
  }

  // ---------------------------------------------------------------- player menu
  function playerMenu(uid, name, anchor) {
    const pop = $('pmenu');
    const vol = window.UHVoice?.getVolume(uid) ?? 1;
    pop.innerHTML = `<b>${esc(name)}</b>
      <label class="pm-vol">🔊 Voice <input type="range" min="0" max="1" step="0.05" value="${vol}" id="pm-vol"></label>
      <button class="btn small secondary" id="pm-block">🚫 Block</button>
      <button class="btn small secondary" id="pm-report">⚑ Report</button>`;
    const r = anchor.getBoundingClientRect();
    pop.style.left = Math.min(innerWidth - 220, Math.max(8, r.left)) + 'px';
    pop.style.top = Math.min(innerHeight - 180, r.bottom + 6) + 'px';
    pop.classList.add('show');
    $('pm-vol').oninput = (e) => window.UHVoice?.setVolume(uid, +e.target.value);
    $('pm-block').onclick = () => {
      if (confirm(`Block ${name}? You won't see their messages or hear their voice.`)) C().wsSend({ t: 'block', id: uid });
      pop.classList.remove('show');
    };
    $('pm-report').onclick = () => {
      const reason = prompt(`Report ${name}. What happened? (e.g. harassment, cheating, offensive name)`);
      if (reason) {
        const last = msgs.filter((m) => m.from === uid).slice(-3).map((m) => m.text).join(' | ');
        C().wsSend({ t: 'report', id: uid, reason, message: last });
      }
      pop.classList.remove('show');
    };
  }
  document.addEventListener('pointerdown', (e) => {
    const pop = $('pmenu');
    if (pop?.classList.contains('show') && !pop.contains(e.target) && !e.target.closest('[data-pmenu]')) pop.classList.remove('show');
  });
  document.addEventListener('click', (e) => {
    const b = e.target.closest('#party-chat [data-pmenu], #game-chat [data-pmenu]');
    if (b) playerMenu(b.dataset.pmenu, b.dataset.pname, b);
  });

  // ---------------------------------------------------------------- wiring
  function init() {
    $('pc-form').onsubmit = (e) => {
      e.preventDefault();
      if (send($('pc-input').value)) $('pc-input').value = '';
    };
    $('gc-form').onsubmit = (e) => {
      e.preventDefault();
      send($('gc-input').value);
      closeInput();
    };
    $('gc-input').addEventListener('keydown', (e) => { if (e.key === 'Escape') { e.preventDefault(); closeInput(); } });
    $('gc-input').addEventListener('blur', () => { if (typing) setTimeout(closeInput, 0); });
    $('chat-btn').onclick = () => (typing ? closeInput() : openInput());
    window.addEventListener('keydown', (e) => {
      const t = e.target;
      if (t && (t.tagName === 'INPUT' || t.tagName === 'TEXTAREA')) return;
      if (!document.body.classList.contains('playing') || !C()?.party) return;
      if (e.code === 'KeyT' || e.key === '/') { e.preventDefault(); e.stopPropagation(); openInput(); }
    }, true);

    const c = C();
    c.on('chat', (m) => {
      push({ from: m.from, name: m.name, text: m.text, ts: Date.now() });
      if (m.from !== c.me?.id) window.UHUI?.send('sfx|chat');
    });
    let lastMembers = null;
    c.on('party', (m) => {
      const p = m.party;
      const ids = new Map((p?.members || []).map((x) => [x.id, x.name]));
      if (lastMembers && p) {
        for (const [id, name] of ids) if (!lastMembers.has(id)) push({ sys: true, text: `${name} joined the party`, ts: Date.now() });
        for (const [id, name] of lastMembers) if (!ids.has(id)) push({ sys: true, text: `${name} left the party`, ts: Date.now() });
      }
      if (!p) msgs.length = 0;
      lastMembers = p ? ids : null;
      document.body.classList.toggle('in-party', !!p);
      renderLobby();
    });
    c.on('render', renderLobby);
  }
  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', init); else setTimeout(init, 0);

  window.UHChat = { playerMenu, send, get messages() { return msgs.slice(); }, open: openInput, system: (text) => push({ sys: true, text, ts: Date.now() }) };
})();
