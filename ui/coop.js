// Co-op layer: social WebSocket (friends, presence, parties, invites, share
// links) + WebRTC peer-to-peer game transport. The host's browser runs the
// world; each player is linked to the host with two data channels:
//   "u" unordered/unreliable (snapshots, player states)
//   "r" reliable (hits, events, control)
// Signaling (offer/answer with ICE candidates) goes through the Neon WebSocket.
(() => {
  'use strict';
  const $ = (id) => document.getElementById(id);
  const esc = (s) => String(s ?? '').replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));
  const UI = () => window.UHUI;
  const ICE = [{ urls: ['stun:stun.l.google.com:19302', 'stun:stun1.l.google.com:19302'] }];

  const S = {
    ws: null,
    retry: 0,
    closed: true,
    me: null, // { id, name, code }
    social: { friends: [], incoming: [], outgoing: [], invites: [] },
    party: null,
    pendingLink: null, // { partyId, token } from a ?party= URL
    hubMates: {}, // Home Base: party members walking around (by user id)
    // game transport
    peers: new Map(), // slot -> { pc, u, r, uid, sid }
    sid: null, // current match session id
    loading: false,
    linkStatus: null, // slot -> connecting|retrying|connected|failed
    inbox: [],
    inGame: false,
    mySlot: 0,
    roster: null,
  };

  // after a host migration (M3) the host is whoever took over, not slot 0;
  // on clients the host's link is always keyed 0
  const hosting = () => S.mySlot === (S.hostSlot || 0);

  // ------------------------------------------------------------------ Rust bridge
  window.uhNetPoll = () => (S.inbox.length ? S.inbox.shift() : undefined);
  window.uhNetSend = (slot, reliable, data) => {
    const peer = hosting() ? S.peers.get(slot) : S.peers.get(0);
    const ch = peer && (reliable ? peer.r : peer.u);
    if (!ch || ch.readyState !== 'open') return;
    if (!reliable && ch.bufferedAmount > 256 * 1024) return; // drop stale snapshots under congestion
    try { ch.send(data); } catch (e) { console.warn('[coop] send', e); }
  };
  const send = (cmd) => UI().send(cmd);

  // ------------------------------------------------------------------ social socket
  const signedIn = () => !!(window.UHCloud && window.UHCloud.user);

  async function connect() {
    if (!signedIn() || (S.ws && S.ws.readyState <= 1)) return;
    S.closed = false;
    let token;
    try {
      token = await window.UHCloud.token();
    } catch (e) {
      console.warn('[coop] could not get a lobby token', e);
      S.status = 'error';
      render();
      setTimeout(connect, 5000);
      return;
    }
    S.status = 'connecting';
    const ws = new WebSocket(window.UHCloud.wsUrl + '?token=' + encodeURIComponent(token));
    S.ws = ws;
    ws.onopen = () => { S.retry = 0; S.status = 'online'; render(); };
    ws.onmessage = (e) => { try { onMsg(JSON.parse(e.data)); } catch (err) { console.warn(err); } };
    ws.onclose = () => {
      S.ws = null;
      S.status = 'offline';
      emit('offline');
      render();
      if (!S.closed) setTimeout(connect, Math.min(1000 * 2 ** S.retry++, 15000));
    };
    ws.onerror = () => ws.close();
  }
  function disconnect() {
    S.closed = true;
    S.ws?.close();
    S.ws = null;
    S.me = null;
    S.party = null;
    S.social = { friends: [], incoming: [], outgoing: [], invites: [] };
    emit('party', { party: null });
    render();
  }
  const wsSend = (m) => {
    if (S.ws && S.ws.readyState === 1) { S.ws.send(JSON.stringify(m)); return true; }
    UI().toast('Not connected to the lobby server yet');
    return false;
  };
  setInterval(() => { if (S.ws && S.ws.readyState === 1) S.ws.send('{"t":"ping"}'); }, 20000);

  function look() {
    const e = window.UHData?.profile?.equipped || {};
    return { skin: e.skin, hat: e.hat, tracer: e.tracer, gun: e.gun, perk: e.perk };
  }

  // other modules (chat.js, voice.js) subscribe to server messages
  const listeners = {};
  const emit = (t, m) => { for (const fn of listeners[t] || []) { try { fn(m); } catch (e) { console.warn('[coop] listener', t, e); } } };

  function onMsg(m) {
    switch (m.t) {
      case 'hello':
        S.me = m.me;
        if (S.pendingLink) {
          wsSend({ t: 'party.join', partyId: S.pendingLink.partyId, token: S.pendingLink.token, look: look() });
          S.pendingLink = null;
          history.replaceState(null, '', location.pathname);
        }
        break;
      case 'social':
        S.social = m;
        break;
      case 'party': {
        const was = S.party;
        S.party = m.party;
        const p = S.party;
        const allReady = !!p && p.status === 'open' && p.members.length > 1 && p.members.every((x) => x.ready);
        if (allReady && !S.allReady) send('sfx|ready');
        S.allReady = allReady;
        // host cancelled (or the match ended) while we were still connecting
        if (S.loading && !S.inGame && (!p || p.status === 'open')) {
          abortLoading();
          UI().toast('The host cancelled the match.');
        }
        if (S.party && S.party.members.some((x) => x.id === S.me?.id)) {
          // keep our look current for everyone in the lobby
          const mine = S.party.members.find((x) => x.id === S.me.id);
          const l = look();
          if (mine && (mine.look?.skin !== l.skin || mine.look?.hat !== l.hat || mine.look?.gun !== l.gun)) wsSend({ t: 'party.look', look: l });
        }
        if (!was && S.party) { if (!S.qpJoin) UI().toast('You\'re in a party. Invite friends or share the link!'); iceServers(); S.publicList = null; }
        // joined a public squad mid-match (Quick Play / browser): ask to drop in
        if (S.qpJoin && p && p.hostId !== S.me?.id && p.members.length > 1) {
          S.qpJoin = false;
          if (p.status === 'in_game' && !S.inGame && !S.loading) wsSend({ t: 'signal', to: p.hostId, data: { kind: 'joinreq', sid: String(p.seed || p.id) } });
        } else if (S.qpJoin && p && p.hostId === S.me?.id) S.qpJoin = false;
        if (S.party && S.party.diff) UI().setDiff?.(S.party.diff, S.party.hostId !== S.me?.id);
        if (!S.party) UI().setDiff?.(null, false);
        break;
      }
      case 'party.start':
        S.party = m.party;
        S.allReady = false;
        if (S.party?.status === 'in_game') beginNetGame().catch((e) => { console.error('[coop] start', e); abortLoading(); UI().toast('Couldn\'t start the match.'); });
        break;
      case 'invite':
        showInvite(m.partyId, m.fromName);
        break;
      case 'parties':
        S.publicList = Array.isArray(m.list) ? m.list : [];
        break;
      case 'signal':
        if (typeof m.data?.kind === 'string' && m.data.kind.startsWith('v-')) emit('voice-signal', m);
        else onSignal(m.from, m.data);
        break;
      case 'chat':
        emit('chat', m);
        break;
      case 'hub':
        S.hubMates[m.from] = {
          name: m.name, x: m.x, y: m.y, aim: m.aim, look: m.look || {}, at: Date.now(),
          emote: Number.isInteger(m.emote) ? { e: m.emote, at: Date.now() } : S.hubMates[m.from]?.emote,
        };
        emit('hub', m);
        return; // positions don't need a re-render
      case 'toast':
        UI().toast(m.msg);
        break;
      case 'error':
        UI().toast(m.msg);
        break;
    }
    if (m.t === 'party' || m.t === 'party.start' || m.t === 'social' || m.t === 'hello' || m.t === 'parties') emit(m.t, m);
    render();
  }

  // ------------------------------------------------------------------ rendering
  function render() {
    renderParty();
    renderFriends();
    renderLobbyMates();
    emit('render');
  }

  function renderLobbyMates() {
    if (!S.party || !S.me) return send('lobbymates|');
    const mates = S.party.members.filter((m) => m.id !== S.me.id);
    send('lobbymates|' + mates.map((m) => [m.name, m.look?.skin || 'default', m.look?.hat || 'none', m.look?.gun || 'pistol'].map((x) => String(x).replace(/[|~]/g, '')).join('~')).join('|'));
  }

  function inviteLink() {
    if (!S.party?.token) return '';
    return `${location.origin}${location.pathname}?party=${S.party.id}&t=${S.party.token}`;
  }

  // Native share sheet (Messages, WhatsApp, Discord…) on phones and tablets;
  // desktop browsers keep the plain copy, since their share dialogs are clunky.
  function canShare() {
    return !!navigator.share && matchMedia('(pointer: coarse)').matches;
  }

  async function copyInvite(link) {
    try { await navigator.clipboard.writeText(link); UI().toast('Invite link copied! Send it to your friends 🧟'); }
    catch { prompt('Copy this invite link:', link); }
  }

  async function shareInvite() {
    const link = inviteLink();
    if (!link) return;
    const data = { title: 'Undead Horizon', text: `${S.me?.name || 'A friend'} wants you in their squad. Join the party in Undead Horizon 🧟`, url: link };
    if (canShare() && (!navigator.canShare || navigator.canShare(data))) {
      try { await navigator.share(data); return; }
      catch (e) { if (e?.name === 'AbortError') return; } // closed the sheet
    }
    copyInvite(link);
  }

  const MODE_LABEL = { infinite: '♾️ Infinite', campaign: '📖 Campaign', outbreak: '🧟 Outbreak', siege: '🏰 Siege' };

  // campaign co-op: the host picks one of their unlocked chapters; a win counts
  // for every member who has that chapter unlocked
  function chapterRow(p, amHost) {
    const info = UI().chapters?.() || { titles: [], unlocked: 0 };
    const ch = p.chapter || 0;
    const title = info.titles[ch] || '';
    const pick = amHost
      ? `<select id="party-ch">${info.titles.map((t, i) => (i <= info.unlocked ? `<option value="${i}" ${i === ch ? 'selected' : ''}>Chapter ${i + 1} · ${esc(t)}</option>` : '')).join('')}</select>`
      : `<b>Chapter ${ch + 1} · ${esc(title)}</b>`;
    const note = ch <= info.unlocked ? 'Clearing it counts toward your story.' : 'You haven\'t reached this chapter yet: play along, but it won\'t unlock anything for you.';
    return `<div class="party-ch" style="margin-top:6px">${pick}<div class="hint">${note}</div></div>`;
  }

  function renderParty() {
    const box = $('party-box');
    if (!box) return;
    if (!window.UHCloud) { box.innerHTML = ''; return; }
    if (!signedIn()) {
      box.innerHTML = `<div class="party empty-party"><b>👥 Co-op</b><span>Sign in to party up with up to 3 friends.</span>
        <button class="btn small secondary" data-signin>Sign in</button></div>`;
      return;
    }
    if (S.status !== 'online' || !S.me) {
      box.innerHTML = `<div class="party empty-party"><b>👥 Co-op</b><span>${S.status === 'error' ? '⚠ Could not reach the lobby server, retrying…' : '⏳ Connecting to the lobby server…'}</span></div>`;
      return;
    }
    if (!S.party) {
      const list = S.publicList;
      box.innerHTML = `<div class="party empty-party"><b>👥 Co-op</b><span>Play with up to 3 friends, or squad up with anyone.</span>
        <div class="row"><button class="btn small blue" id="party-create">Create party</button>
        <button class="btn small gold" id="quickplay" title="Join the fullest public squad with room, or host one">⚡ Quick Play</button></div>
        <button class="btn small secondary" id="party-browse">🌐 Browse public squads</button>
        ${list ? `<div class="pub-list">${list.length ? list.map((x) => `<div class="pub"><span><b>${esc(x.host)}</b> · ${MODE_LABEL[x.mode] || esc(x.mode)}${x.mode === 'campaign' ? ` ch.${x.chapter + 1}` : ''} · ${esc(window.UHDiffName?.(x.diff) || x.diff)}${x.status === 'in_game' ? ' · ⚔ in match' : ''}</span><span class="n">${x.n}/4</span><button class="btn small" data-pjoin="${esc(x.id)}">Join</button></div>`).join('') : '<div class="hint">No public squads right now. Quick Play hosts one for you.</div>'}</div>` : ''}
      </div>`;
      $('party-create').onclick = () => wsSend({ t: 'party.create', look: look() });
      $('quickplay').onclick = () => { send('sfx|click'); S.qpJoin = true; wsSend({ t: 'quickplay', look: look() }); UI().toast('Finding a squad…'); };
      $('party-browse').onclick = () => { send('sfx|click'); wsSend({ t: 'party.list' }); };
      box.querySelectorAll('[data-pjoin]').forEach((b) => (b.onclick = () => { S.qpJoin = true; wsSend({ t: 'party.join', partyId: b.dataset.pjoin, look: look() }); }));
      return;
    }
    const p = S.party;
    const amHost = p.hostId === S.me?.id;
    const me = p.members.find((m) => m.id === S.me?.id);
    const allReady = p.members.every((m) => m.ready);
    const slots = [0, 1, 2, 3].map((i) => {
      const m = p.members[i];
      if (!m) return `<div class="pm empty">Open slot<span>invite a friend</span></div>`;
      return `<div class="pm ${m.online ? '' : 'offline'}">
        <span class="dot ${m.online ? 'on' : ''}"></span>
        <b ${m.id !== S.me?.id ? `data-pmenu="${esc(m.id)}" data-pname="${esc(m.name)}" class="clickable"` : ''} data-voice-uid="${esc(m.id)}">${m.voice ? '<span class="vic" title="In voice">🎙</span>' : ''}${esc(m.name)}${m.id === p.hostId ? ' 👑' : ''}${m.id === S.me?.id ? ' (you)' : ''}</b>
        <span class="rdy ${m.ready ? 'yes' : ''}">${m.ready ? 'READY' : 'not ready'}</span>
        ${amHost && m.id !== S.me?.id ? `<button class="x" data-kick="${esc(m.id)}" title="Remove from party">×</button>` : ''}
      </div>`;
    }).join('');
    box.innerHTML = `<div class="party">
      <div class="party-head"><b>👥 Party ${p.members.length}/4</b><span class="hint">${esc(window.UHDiffName?.(p.diff) || p.diff)}${amHost ? ' · you set the difficulty' : ' · host sets difficulty'}</span></div>
      <div class="pms">${slots}</div>
      <div class="seg party-mode" style="margin-top:8px">${['infinite', 'campaign', 'outbreak', 'siege'].map((m) =>
        `<button data-pmode="${m}" class="${(p.mode || 'infinite') === m ? 'on' : ''}" ${amHost ? '' : 'disabled'}>${MODE_LABEL[m]}</button>`).join('')}</div>
      ${p.mode === 'campaign' ? chapterRow(p, amHost) : ''}
      ${amHost ? `<label class="pub-toggle"><input type="checkbox" id="party-public" ${p.public ? 'checked' : ''}> 🌐 Public: anyone can join (Quick Play fills empty slots)</label>` : p.public ? '<div class="hint">🌐 Public squad</div>' : ''}
      <div id="voice-ctl"></div>
      <div class="row" style="margin-top:8px">
        <button class="btn small secondary" id="party-link" title="Anyone signed in with this link joins your party and becomes your friend">${canShare() ? '📤 Share invite link' : '🔗 Copy invite link'}</button>
        <button class="btn small secondary" id="party-leave">Leave</button>
      </div>
      ${p.status === 'in_game' && !S.inGame && !S.loading && !amHost ? `<button class="btn gold" id="party-joinrun" style="margin-top:8px">⚔ Join the match in progress</button><div class="hint">You'll spectate until the current wave is cleared, then drop in.</div>` : ''}
      ${amHost
        ? `<span class="tip-wrap"><button class="btn gold" id="party-start" style="margin-top:10px" ${allReady && p.members.length > 1 && p.status === 'open' && !S.loading ? '' : 'disabled'}>▶ START CO-OP</button>
           <span class="tip">${p.members.length < 2 ? 'Invite at least one friend first' : allReady ? 'Everyone is ready!' : 'Waiting for everyone to ready up'}</span></span>`
        : `<button class="btn ${me?.ready ? 'secondary' : ''}" id="party-ready" style="margin-top:10px">${me?.ready ? 'Not ready' : '✔ Ready up'}</button>`}
    </div>`;
    $('party-link').onclick = () => shareInvite();
    $('party-leave').onclick = () => wsSend({ t: 'party.leave' });
    box.querySelectorAll('[data-pmode]').forEach((b) => (b.onclick = () => { if (amHost) wsSend({ t: 'party.mode', mode: b.dataset.pmode, chapter: p.chapter || 0 }); }));
    if ($('party-public')) $('party-public').onchange = (e) => wsSend({ t: 'party.public', on: e.target.checked });
    if ($('party-ch')) $('party-ch').onchange = (e) => { if (amHost) wsSend({ t: 'party.mode', mode: 'campaign', chapter: Number(e.target.value) }); };
    if ($('party-ready')) $('party-ready').onclick = () => { send('sfx|click'); wsSend({ t: 'party.ready', ready: !me?.ready }); };
    if ($('party-start')) $('party-start').onclick = () => wsSend({ t: 'party.start' });
    if ($('party-joinrun')) $('party-joinrun').onclick = () => {
      send('sfx|click');
      wsSend({ t: 'signal', to: p.hostId, data: { kind: 'joinreq', sid: String(p.seed || p.id) } });
      UI().toast('Asking the host to drop you in…');
    };
    box.querySelectorAll('[data-kick]').forEach((b) => (b.onclick = () => wsSend({ t: 'party.kick', id: b.dataset.kick })));
    box.querySelectorAll('[data-pmenu]').forEach((b) => (b.onclick = (e) => window.UHChat?.playerMenu(b.dataset.pmenu, b.dataset.pname, e.currentTarget)));
  }

  function renderFriends() {
    const el = $('friends');
    if (!el) return;
    if (!window.UHCloud) { el.innerHTML = '<div class="empty">Friends need the online backend.</div>'; return; }
    if (!signedIn()) { el.innerHTML = '<div class="empty">Sign in to add friends and play co-op.<br><br><button class="btn small blue" data-signin>Sign in</button></div>'; return; }
    if (S.status !== 'online' || !S.me) { el.innerHTML = '<div class="empty">⏳ Connecting to the lobby server…</div>'; return; }
    const s = S.social;
    const inParty = !!S.party;
    const partyIds = new Set((S.party?.members || []).map((m) => m.id));
    el.innerHTML = `
      <div class="fcode">Your friend code <b id="my-code">${esc(S.me?.code || '…')}</b><button class="btn small secondary" id="copy-code">Copy</button></div>
      <form class="fadd" id="fadd"><input type="text" id="fadd-code" maxlength="6" placeholder="Friend's code" autocomplete="off" spellcheck="false" />
        <button class="btn small" type="submit">Add</button></form>
      ${s.invites.length ? `<h3>Party invites</h3>${s.invites.map((i) => `<div class="frow"><span>🎉 <b>${esc(i.fromName)}</b> invited you</span>
        <span><button class="btn small" data-join="${esc(i.partyId)}">Join</button> <button class="btn small secondary" data-decline="${esc(i.partyId)}">✕</button></span></div>`).join('')}` : ''}
      ${s.incoming.length ? `<h3>Friend requests</h3>${s.incoming.map((f) => `<div class="frow"><span>${esc(f.name)}</span>
        <span><button class="btn small" data-accept="${esc(f.id)}">Accept</button> <button class="btn small secondary" data-remove="${esc(f.id)}">✕</button></span></div>`).join('')}` : ''}
      <h3>Friends (${s.friends.length})</h3>
      ${s.friends.length ? s.friends.map((f) => `<div class="frow ${f.online ? '' : 'offline'}">
        <span><span class="dot ${f.online ? 'on' : ''}"></span>${esc(f.name)} ${partyIds.has(f.id) ? '<span class="tag">in your party</span>' : f.partyId ? '<span class="tag">in a party</span>' : ''}</span>
        <span>${inParty && !partyIds.has(f.id) && f.online ? `<button class="btn small blue" data-invite="${esc(f.id)}">Invite</button>` : ''}
        <button class="btn small secondary" data-remove="${esc(f.id)}" title="Remove friend">✕</button></span></div>`).join('')
        : '<div class="empty" style="padding:8px">No friends yet. Share your code, or create a party and send the invite link.</div>'}
      ${s.blocked?.length ? `<h3>Blocked</h3>${s.blocked.map((f) => `<div class="frow offline"><span>${esc(f.name)}</span>
        <button class="btn small secondary" data-unblock="${esc(f.id)}">Unblock</button></div>`).join('')}` : ''}
      ${s.outgoing.length ? `<h3>Sent requests</h3>${s.outgoing.map((f) => `<div class="frow offline"><span>${esc(f.name)}</span><span class="tag">pending</span></div>`).join('')}` : ''}`;
    $('copy-code').onclick = async () => { try { await navigator.clipboard.writeText(S.me?.code || ''); UI().toast('Friend code copied'); } catch {} };
    $('fadd').onsubmit = (e) => { e.preventDefault(); const c = $('fadd-code').value.trim(); if (c) { wsSend({ t: 'friends.add', code: c }); $('fadd-code').value = ''; } };
    el.querySelectorAll('[data-accept]').forEach((b) => (b.onclick = () => wsSend({ t: 'friends.accept', id: b.dataset.accept })));
    el.querySelectorAll('[data-remove]').forEach((b) => (b.onclick = () => wsSend({ t: 'friends.remove', id: b.dataset.remove })));
    el.querySelectorAll('[data-invite]').forEach((b) => (b.onclick = () => { wsSend({ t: 'party.invite', id: b.dataset.invite }); UI().toast('Invite sent'); }));
    el.querySelectorAll('[data-join]').forEach((b) => (b.onclick = () => wsSend({ t: 'party.join', partyId: b.dataset.join, look: look() })));
    el.querySelectorAll('[data-unblock]').forEach((b) => (b.onclick = () => wsSend({ t: 'unblock', id: b.dataset.unblock })));
    el.querySelectorAll('[data-decline]').forEach((b) => (b.onclick = () => wsSend({ t: 'party.decline', partyId: b.dataset.decline })));
  }

  function showInvite(partyId, fromName) {
    const pop = $('invite-pop');
    if (!pop) return;
    pop.innerHTML = `<b>🎉 ${esc(fromName)}</b> invited you to their party
      <span><button class="btn small" id="inv-join">Join</button> <button class="btn small secondary" id="inv-no">Later</button></span>`;
    pop.classList.add('show');
    $('inv-join').onclick = () => { wsSend({ t: 'party.join', partyId, look: look() }); pop.classList.remove('show'); };
    $('inv-no').onclick = () => pop.classList.remove('show');
    setTimeout(() => pop.classList.remove('show'), 20000);
  }

  // ------------------------------------------------------------------ WebRTC
  // Each match has a session id (the party seed from the server). Signals carry
  // it so a late offer/candidate from an older attempt is never applied, and an
  // offer that beats `party.start` to the client still works.
  // ICE is trickled: candidates are sent in small batches as they're found, so
  // slow networks (phones on cellular) don't lose their relay candidates.
  const HOST_WAIT = 20000; // ms before the host is offered "start without them"
  const CLIENT_WAIT = 40000; // ms before a client gives up on the host

  let iceCache = null;
  async function iceServers() {
    if (iceCache && Date.now() - iceCache.at < 30 * 60_000) return iceCache.servers;
    try {
      const r = await Promise.race([
        window.UHCloud.api('/turn'),
        new Promise((_, rej) => setTimeout(() => rej(new Error('turn timeout')), 4000)),
      ]);
      if (Array.isArray(r?.iceServers) && r.iceServers.length) {
        iceCache = { at: Date.now(), servers: r.iceServers };
        return iceCache.servers;
      }
    } catch (e) {
      console.warn('[coop] no TURN servers, direct connections only', e);
    }
    return ICE;
  }

  function slotsFor(party) {
    // host = slot 0, everyone else in join order
    const others = party.members.filter((m) => m.id !== party.hostId);
    const list = [party.members.find((m) => m.id === party.hostId), ...others].filter(Boolean);
    return list.map((m, i) => ({ ...m, slot: i }));
  }

  // batched trickle-ICE candidates per remote user
  const iceOut = new Map(); // uid -> { list, timer }
  function queueIce(uid, cand) {
    let q = iceOut.get(uid);
    if (!q) iceOut.set(uid, (q = { list: [], timer: 0 }));
    q.list.push(cand);
    if (!q.timer) q.timer = setTimeout(() => {
      iceOut.delete(uid);
      wsSend({ t: 'signal', to: uid, data: { kind: 'ice', sid: S.sid, c: q.list } });
    }, 120);
  }
  const pendingIce = []; // { sid, from, c } that arrived before their peer/description

  function wire(slot, pc, uid) {
    const peer = { pc, u: null, r: null, slot, uid, sid: S.sid, ready: false };
    const onChannel = (ch) => {
      ch.binaryType = 'arraybuffer';
      if (ch.label === 'u') peer.u = ch; else peer.r = ch;
      ch.onmessage = (e) => {
        if (typeof e.data === 'string') return onControl(slot, JSON.parse(e.data));
        S.inbox.push({ from: slot, data: new Uint8Array(e.data) });
        if (S.inbox.length > 2000) S.inbox.splice(0, 500);
      };
      ch.onopen = () => checkAllOpen();
      ch.onclose = () => { if (S.peers.get(slot) === peer) peerGone(slot); };
    };
    pc.ondatachannel = (e) => onChannel(e.channel);
    pc.onicecandidate = (e) => { if (e.candidate) queueIce(uid, e.candidate.toJSON()); };
    pc.onconnectionstatechange = () => {
      if (S.peers.get(slot) !== peer) return;
      const st = pc.connectionState;
      if (st === 'connected') linkRecovered(peer);
      else if (S.inGame && (st === 'disconnected' || st === 'failed')) linkTrouble(peer, st === 'failed');
      else if (st === 'failed') onLinkFailed(slot);
      else if (st === 'closed') peerGone(slot);
      setLinkStatus(slot);
    };
    peer.onChannel = onChannel;
    S.peers.set(slot, peer);
    return peer;
  }
  // ------------------------------------------------------------------ ICE restart (mid-match)
  // A phone switching Wi-Fi -> cellular or a NAT rebinding drops the ICE path
  // while the data channels survive. The host (always the offerer, so there is
  // no offer glare) restarts ICE and re-offers; a client that notices first
  // asks the host to. The player only leaves the match after RESTART_GRACE.
  const RESTART_GRACE = 15000;
  function linkTrouble(peer, failed) {
    if (!peer.troubleAt) {
      peer.troubleAt = Date.now();
      const name = S.roster?.find((m) => m.slot === peer.slot)?.name || 'a teammate';
      UI().toast(hosting() || peer.slot !== 0 ? `Connection to ${name} dropped, reconnecting…` : 'Connection to the host dropped, reconnecting…');
      clearTimeout(peer.giveUp);
      peer.giveUp = setTimeout(() => {
        if (S.peers.get(peer.slot) === peer && peer.troubleAt) { try { peer.pc.close(); } catch {} peerGone(peer.slot); }
      }, RESTART_GRACE);
    }
    // 'disconnected' often heals by itself: give it a moment before restarting
    clearTimeout(peer.restartT);
    peer.restartT = setTimeout(() => restartIce(peer), failed ? 0 : 2500);
  }
  function linkRecovered(peer) {
    clearTimeout(peer.restartT);
    clearTimeout(peer.giveUp);
    if (peer.troubleAt && S.inGame) UI().toast('Reconnected ✔');
    peer.troubleAt = 0;
    peer.restarts = 0;
  }
  async function restartIce(peer) {
    if (S.peers.get(peer.slot) !== peer || !peer.troubleAt || !S.inGame) return;
    if ((peer.restarts = (peer.restarts || 0) + 1) > 3) return;
    S.restarts = (S.restarts || 0) + 1;
    if (!hosting()) { wsSend({ t: 'signal', to: peer.uid, data: { kind: 'restart', sid: peer.sid } }); return; }
    const pc = peer.pc;
    if (pc.signalingState !== 'stable') return;
    try {
      await pc.setLocalDescription(await pc.createOffer({ iceRestart: true }));
      wsSend({ t: 'signal', to: peer.uid, data: { kind: 'reoffer', sid: peer.sid, sdp: pc.localDescription.sdp } });
    } catch (e) { console.warn('[coop] ice restart', e); }
  }

  const linkOpen = (p) => p && p.u?.readyState === 'open' && p.r?.readyState === 'open';

  async function applyIce(peer, list) {
    for (const c of list) {
      try { await peer.pc.addIceCandidate(c); } catch (e) { console.warn('[coop] bad candidate', e); }
    }
  }
  async function flushPendingIce(peer) {
    for (let i = pendingIce.length - 1; i >= 0; i--) {
      const q = pendingIce[i];
      if (q.sid === peer.sid && q.from === peer.uid) { pendingIce.splice(i, 1); await applyIce(peer, q.c); }
    }
  }

  /** Called on `party.start` for everyone in the party. */
  async function beginNetGame() {
    const sid = String(S.party.seed || S.party.id);
    if (S.inGame) return;
    if (S.sid !== sid) { closePeers(); S.sid = sid; pendingIce.length = 0; }
    else if (S.loading) return; // duplicate start event
    const roster = slotsFor(S.party);
    S.roster = roster;
    S.hostSlot = 0;
    const mine = roster.find((m) => m.id === S.me.id);
    if (!mine) return;
    S.mySlot = mine.slot;
    S.linkStatus = {};
    send('sfx|start');
    showLoading();
    clearTimeout(startTimer);
    if (S.mySlot === 0) {
      if (roster.length === 1) return hostGo(false);
      await Promise.all(roster.slice(1).map((m) => connectTo(m, 0)));
      startTimer = setTimeout(hostTimedOut, HOST_WAIT);
    } else {
      startTimer = setTimeout(clientTimedOut, CLIENT_WAIT);
    }
  }

  async function connectTo(m, attempt, late = false, migrate = false) {
    const sid = S.sid;
    const old = S.peers.get(m.slot);
    if (old) { S.peers.delete(m.slot); try { old.pc.close(); } catch {} }
    const servers = await iceServers();
    if (S.sid !== sid || (S.inGame && !late && !migrate)) return;
    const pc = new RTCPeerConnection({ iceServers: servers });
    const peer = wire(m.slot, pc, m.id);
    peer.attempt = attempt;
    peer.late = late;
    peer.migrate = migrate;
    peer.onChannel(pc.createDataChannel('u', { ordered: false, maxRetransmits: 0 }));
    peer.onChannel(pc.createDataChannel('r', { ordered: true }));
    await pc.setLocalDescription(await pc.createOffer());
    wsSend({ t: 'signal', to: m.id, data: { kind: 'offer', sid, slot: m.slot, attempt, late: late ? 1 : 0, migrate: migrate ? 1 : 0, roster: late ? S.roster.map(({ id, name, slot }) => ({ id, name, slot })) : undefined, sdp: pc.localDescription.sdp } });
    if (late || migrate) return;
    S.linkStatus[m.slot] = attempt ? 'retrying' : 'connecting';
    renderLoading();
  }

  function onLinkFailed(slot) {
    const m = S.roster?.find((x) => x.slot === slot);
    if (S.mySlot === 0 && m) {
      const p = S.peers.get(slot);
      if ((p?.attempt || 0) < 2) { connectTo(m, (p?.attempt || 0) + 1); return; }
      S.linkStatus[slot] = 'failed';
      renderLoading();
      checkAllOpen();
    } else {
      S.linkStatus[0] = 'retrying'; // the host re-offers; we just wait for it
      renderLoading();
    }
  }

  async function onSignal(from, data) {
    if (!data || !S.party) return;
    if (data.sid && S.sid && data.sid !== S.sid && data.kind !== 'offer') return; // older match
    switch (data.kind) {
      case 'offer': {
        if (data.migrate && S.inGame) {
          // a teammate took over as host. If our own host link is still fine,
          // they lost the host, not us: stay put (they carry on solo).
          if (linkOpen(S.peers.get(0)) && S.awaitHost !== from) return;
          if (S.awaitHost !== from) {
            const nx = successor(S.hostSlot || 0);
            if (!nx || nx.id !== from) return;
            const old = S.hostSlot || 0;
            S.roster = S.roster.filter((m) => m.slot !== old);
            S.hostSlot = nx.slot;
            send(`hostmoved|${old}|${nx.slot}`);
          }
          S.awaitHost = null;
          clearTimeout(S.migrateT);
          const servers = await iceServers();
          const prev = S.peers.get(0);
          if (prev) { S.peers.delete(0); try { prev.pc.close(); } catch {} }
          const pc = new RTCPeerConnection({ iceServers: servers });
          const peer = wire(0, pc, from);
          await pc.setRemoteDescription({ type: 'offer', sdp: data.sdp });
          await pc.setLocalDescription(await pc.createAnswer());
          wsSend({ t: 'signal', to: from, data: { kind: 'answer', sid: data.sid, sdp: pc.localDescription.sdp } });
          peer.described = true;
          await flushPendingIce(peer);
          S.migrations = (S.migrations || 0) + 1;
          break;
        }
        if (S.inGame || from !== S.party.hostId) return;
        if (S.sid !== data.sid) { closePeers(); S.sid = data.sid; }
        S.mySlot = data.slot;
        const servers = await iceServers();
        if (S.sid !== data.sid) return;
        const old = S.peers.get(0);
        if (old) { S.peers.delete(0); try { old.pc.close(); } catch {} }
        const pc = new RTCPeerConnection({ iceServers: servers });
        const peer = wire(0, pc, from);
        await pc.setRemoteDescription({ type: 'offer', sdp: data.sdp });
        await pc.setLocalDescription(await pc.createAnswer());
        wsSend({ t: 'signal', to: from, data: { kind: 'answer', sid: data.sid, sdp: pc.localDescription.sdp } });
        peer.described = true;
        await flushPendingIce(peer);
        S.linkStatus = S.linkStatus || {};
        S.linkStatus[0] = 'connecting';
        if (data.late && Array.isArray(data.roster)) S.roster = data.roster;
        if (data.late && !S.loading) {
          // the host is letting us into a match in progress
          showLoading();
          clearTimeout(startTimer);
          startTimer = setTimeout(clientTimedOut, CLIENT_WAIT);
        }
        renderLoading();
        break;
      }
      case 'joinreq': {
        // host: a party member wants into the running match (rejoin or a new friend)
        if (!hosting() || !S.inGame || data.sid !== S.sid) return;
        const mem = S.party?.members.find((x) => x.id === from);
        if (!mem) return;
        let slot = S.roster.find((x) => x.id === from)?.slot;
        if (slot !== undefined && linkOpen(S.peers.get(slot))) return; // already connected
        if (slot === undefined) {
          const inParty = (x) => S.party.members.some((pm) => pm.id === x.id);
          const taken = (i) => S.peers.has(i) || S.roster.some((x) => x.slot === i && inParty(x));
          slot = [1, 2, 3].find((i) => !taken(i));
          if (slot === undefined) { wsSend({ t: 'signal', to: from, data: { kind: 'nogo', sid: S.sid } }); return; }
          S.roster = S.roster.filter((x) => x.slot !== slot).concat({ ...mem, slot });
        }
        connectTo({ ...mem, slot }, 0, true);
        break;
      }
      case 'reoffer': {
        // the host restarted ICE mid-match: answer on the existing connection
        const peer = S.peers.get(0);
        if (!peer || from !== peer.uid || data.sid !== peer.sid) return;
        try {
          await peer.pc.setRemoteDescription({ type: 'offer', sdp: data.sdp });
          await peer.pc.setLocalDescription(await peer.pc.createAnswer());
          wsSend({ t: 'signal', to: from, data: { kind: 'answer', sid: data.sid, sdp: peer.pc.localDescription.sdp } });
          S.restarts = (S.restarts || 0) + 1;
        } catch (e) { console.warn('[coop] reoffer', e); }
        break;
      }
      case 'alive?':
        // a client lost its link to us: tell it we're still running the match
        if (hosting() && S.inGame && data.sid === S.sid) wsSend({ t: 'signal', to: from, data: { kind: 'alive', sid: S.sid } });
        break;
      case 'alive':
        if (S.aliveCheck && S.aliveCheck === from && S.inGame) {
          S.aliveCheck = null;
          clearTimeout(S.aliveT);
          UI().toast('You lost connection to the host. Rejoin from the party panel.');
          send('peerleft|0');
        }
        break;
      case 'restart': {
        // a client lost us: restart ICE from our side
        if (!hosting() || !S.inGame) return;
        const peer = [...S.peers.values()].find((p) => p.uid === from && p.sid === data.sid);
        if (peer) { peer.troubleAt ||= Date.now(); restartIce(peer); }
        break;
      }
      case 'answer': {
        if (!hosting()) return;
        const peer = [...S.peers.values()].find((p) => p.uid === from);
        if (peer && peer.pc.signalingState === 'have-local-offer') {
          await peer.pc.setRemoteDescription({ type: 'answer', sdp: data.sdp });
          peer.described = true;
          await flushPendingIce(peer);
        }
        break;
      }
      case 'ice': {
        const peer = [...S.peers.values()].find((p) => p.uid === from && p.sid === data.sid);
        if (peer && peer.described) await applyIce(peer, data.c || []);
        else { pendingIce.push({ sid: data.sid, from, c: data.c || [] }); if (pendingIce.length > 50) pendingIce.shift(); }
        break;
      }
      case 'nogo':
        if (!S.inGame && S.loading) {
          abortLoading();
          UI().toast('Couldn\'t connect to the host, so they started without you. Check your connection and try the next match.');
        }
        break;
    }
  }

  function setLinkStatus(slot) {
    const p = S.peers.get(slot);
    if (!S.linkStatus || !p) return;
    if (linkOpen(p)) S.linkStatus[slot] = 'connected';
    renderLoading();
  }

  function checkAllOpen() {
    for (const slot of S.peers.keys()) setLinkStatus(slot);
    if (S.inGame && hosting()) {
      for (const p of S.peers.values()) if (p.late && !p.lateSent && linkOpen(p)) lateGo(p);
      return;
    }
    if (S.mySlot !== 0 || S.inGame || !S.loading) return;
    broadcastLoading();
    const want = (S.roster || []).slice(1);
    if (want.every((m) => linkOpen(S.peers.get(m.slot)))) hostGo(false);
    else if (want.every((m) => linkOpen(S.peers.get(m.slot)) || S.linkStatus[m.slot] === 'failed')) hostTimedOut();
  }

  // host: a late joiner's link is up. Start them as a spectator, tell everyone
  // else, and let the game send them the world state.
  function lateGo(p) {
    p.lateSent = true;
    const present = [0, 1, 2, 3].map((i) => (i === 0 || linkOpen(S.peers.get(i)) ? 1 : 0));
    const name = (S.roster.find((x) => x.slot === p.slot)?.name || '').replace(/[|;,=]/g, '');
    const roster = S.roster.map(({ id, name: n, slot }) => ({ id, name: n, slot }));
    try { p.r.send(JSON.stringify({ t: 'go', late: 1, present, roster })); } catch {}
    for (const q of S.peers.values()) if (q !== p && linkOpen(q)) { try { q.r.send(JSON.stringify({ t: 'joined', slot: p.slot, name, roster })); } catch {} }
    send(`peerjoin|${p.slot}|${name}`);
  }

  // host -> clients: who's connected so far, for their loading screens
  function broadcastLoading() {
    const msg = JSON.stringify({ t: 'load', status: S.linkStatus });
    for (const p of S.peers.values()) if (linkOpen(p)) { try { p.r.send(msg); } catch {} }
  }

  function opts(role, present) {
    const names = [0, 1, 2, 3].map((i) => (S.roster.find((m) => m.slot === i)?.name || '').replace(/[;,=|]/g, ''));
    return `role=${role};me=${S.mySlot};names=${names.join(',')};present=${present.join(',')};mode=${['campaign', 'outbreak', 'siege'].includes(S.party.mode) ? S.party.mode : 'infinite'};chapter=${S.party.chapter || 0};diff=${S.party.diff};name=${names[S.mySlot]};${UI().equipString()}`;
  }

  function hostTimedOut() {
    if (S.inGame || S.mySlot !== 0 || !S.loading) return;
    clearTimeout(startTimer);
    S.hostStuck = true;
    for (const m of S.roster.slice(1)) if (!linkOpen(S.peers.get(m.slot))) S.linkStatus[m.slot] = 'failed';
    broadcastLoading();
    renderLoading();
  }

  function hostGo() {
    if (S.inGame || S.mySlot !== 0) return;
    clearTimeout(startTimer);
    const present = [0, 1, 2, 3].map((i) => (i === 0 ? 1 : linkOpen(S.peers.get(i)) ? 1 : 0));
    for (const m of S.roster.slice(1)) {
      const p = S.peers.get(m.slot);
      if (present[m.slot]) p.r.send(JSON.stringify({ t: 'go', present }));
      else {
        wsSend({ t: 'signal', to: m.id, data: { kind: 'nogo', sid: S.sid } });
        if (p) { S.peers.delete(m.slot); try { p.pc.close(); } catch {} }
      }
    }
    startGame(opts('host', present));
  }

  function onControl(from, m) {
    if (m.t === 'go' && m.late && S.mySlot !== 0 && !S.inGame) {
      if (Array.isArray(m.roster)) S.roster = m.roster;
      S.late = true;
      startGame(opts('client', m.present) + ';late=1');
    } else if (m.t === 'joined' && S.inGame && !hosting()) {
      if (Array.isArray(m.roster)) S.roster = m.roster;
      send(`peerjoin|${m.slot}|${String(m.name || '').replace(/[|;]/g, '')}`);
    } else if (m.t === 'go' && S.mySlot !== 0 && !S.inGame) { S.late = false; startGame(opts('client', m.present)); }
    else if (m.t === 'load' && S.mySlot !== 0 && !S.inGame) { S.hostView = m.status; renderLoading(); }
  }

  let startTimer = 0;
  function startGame(o) {
    clearTimeout(startTimer);
    S.inGame = true;
    S.hostStuck = false;
    S.inbox.length = 0;
    UI().startCoop(o);
    renderLoading('go');
    // keep the loading screen up long enough for the launch sting, then fade it
    const left = Math.max(350, 1700 - (Date.now() - S.loadT0));
    setTimeout(hideLoading, left);
  }

  function clientTimedOut() {
    if (S.inGame || S.mySlot === 0 || !S.loading) return;
    abortLoading();
    UI().toast('Couldn\'t connect to the host. Check your connection and try again.');
  }

  /** Leave the loading screen without starting (cancel, failure, host cancelled). */
  function abortLoading() {
    clearTimeout(startTimer);
    closePeers();
    S.sid = null;
    S.hostStuck = false;
    hideLoading();
  }

  function peerGone(slot) {
    if (!S.peers.has(slot)) return;
    const p = S.peers.get(slot);
    clearTimeout(p.restartT);
    clearTimeout(p.giveUp);
    S.peers.delete(slot);
    if (S.inGame && slot === 0 && !hosting()) hostLost();
    else if (S.inGame) send('peerleft|' + slot);
    else if (S.loading) { if (S.linkStatus) S.linkStatus[slot] = 'failed'; renderLoading(); }
  }

  // ------------------------------------------------------------------ host migration (M3)
  // Everyone left computes the same successor: the lowest slot still in the
  // party. The successor promotes its game (replica + last checkpoint) and
  // re-offers to the others, who accept only from that player. Alone, you
  // simply carry on as a solo host.
  const MIGRATE_WAIT = 20000;
  function successor(oldSlot) {
    const inParty = (m) => m.id === S.me?.id || (S.party?.members || []).some((pm) => pm.id === m.id);
    return (S.roster || []).filter((m) => m.slot !== oldSlot && inParty(m)).sort((a, b) => a.slot - b.slot)[0];
  }
  // First ask the old host (over the social socket) whether it's still running
  // the match: if it answers, *we* dropped, so leave and offer the rejoin.
  const ALIVE_WAIT = 2500;
  function hostLost() {
    const old = S.hostSlot || 0;
    const uid = S.roster?.find((m) => m.slot === old)?.id;
    if (!uid || !S.ws || S.ws.readyState !== 1) { migrate(); return; }
    S.aliveCheck = uid;
    wsSend({ t: 'signal', to: uid, data: { kind: 'alive?', sid: S.sid } });
    clearTimeout(S.aliveT);
    S.aliveT = setTimeout(() => { if (S.aliveCheck === uid) { S.aliveCheck = null; migrate(); } }, ALIVE_WAIT);
  }
  function migrate() {
    if (!S.inGame) return;
    const old = S.hostSlot || 0;
    const next = successor(old);
    if (!next) { send('peerleft|0'); return; }
    S.roster = S.roster.filter((m) => m.slot !== old);
    S.hostSlot = next.slot;
    if (next.slot === S.mySlot) {
      send(`promote|${old}`);
      S.migrations = (S.migrations || 0) + 1;
      for (const m of S.roster) if (m.slot !== S.mySlot) connectTo(m, 0, false, true);
    } else {
      send(`hostmoved|${old}|${next.slot}`);
      S.awaitHost = next.id;
      clearTimeout(S.migrateT);
      S.migrateT = setTimeout(() => {
        if (S.inGame && !linkOpen(S.peers.get(0))) { S.awaitHost = null; send('peerleft|0'); }
      }, MIGRATE_WAIT);
    }
  }

  function closePeers() {
    for (const p of S.peers.values()) { try { p.pc.close(); } catch {} }
    S.peers.clear();
    S.inbox.length = 0;
    for (const q of iceOut.values()) clearTimeout(q.timer);
    iceOut.clear();
  }

  // ------------------------------------------------------------------ loading screen
  const TIPS = [
    'Downed teammates can crawl. Stand next to them for 3 seconds to revive.',
    'Everyone shares XP: stick together and level up as a squad.',
    'Bosses rotate every 5 waves. Save your ultimate for them.',
    'Dash through gaps in the horde. You\'re briefly untouchable.',
    'A team wipe ends the run for everyone. Don\'t stray too far.',
    'Pick powers that complement your squad: crowd control pairs well with burst damage.',
  ];
  function showLoading() {
    S.loading = true;
    S.loadT0 = Date.now();
    S.hostView = null;
    S.hostStuck = false;
    const el = $('coop-load');
    el.classList.add('show');
    $('cl-tip').textContent = TIPS[Math.floor(Math.random() * TIPS.length)];
    clearInterval(S.tipTimer);
    S.tipTimer = setInterval(() => { $('cl-tip').textContent = TIPS[Math.floor(Math.random() * TIPS.length)]; }, 5000);
    renderLoading();
  }
  function hideLoading() {
    S.loading = false;
    clearInterval(S.tipTimer);
    $('coop-load')?.classList.remove('show');
  }
  function renderLoading(phase) {
    if (!S.loading || !S.roster) return;
    const host = S.mySlot === 0;
    const label = { connecting: 'Connecting…', retrying: 'Retrying…', connected: 'Connected', failed: 'Can\'t connect' };
    const total = S.roster.length;
    let done = 0;
    $('cl-diff').textContent = `Infinite · ${window.UHDiffName?.(S.party?.diff) || ''} · ${total} survivors`;
    $('cl-players').innerHTML = S.roster.map((m) => {
      let st;
      if (phase === 'go') st = 'connected';
      else if (m.slot === S.mySlot) st = 'connected';
      else if (host) st = S.linkStatus?.[m.slot] || 'connecting';
      else if (m.slot === 0) st = S.linkStatus?.[0] || 'connecting';
      else st = S.hostView?.[m.slot] || 'connecting';
      if (st === 'connected') done++;
      return `<div class="cl-p ${st}"><span class="cl-dot"></span><b>${esc(m.name)}${m.slot === 0 ? ' 👑' : ''}${m.slot === S.mySlot ? ' (you)' : ''}</b><span>${m.slot === S.mySlot ? 'Ready' : label[st] || st}</span></div>`;
    }).join('');
    $('cl-bar').style.width = (phase === 'go' ? 100 : 15 + 75 * (done / total)) + '%';
    $('cl-status').textContent = phase === 'go' ? 'Dropping in!'
      : host ? (S.hostStuck ? 'Some players couldn\'t connect.' : 'Connecting to your squad…')
      : (S.linkStatus?.[0] === 'connected' ? 'Connected! Waiting for the squad…' : 'Connecting to the host…');
    const acts = $('cl-actions');
    if (phase === 'go') acts.innerHTML = '';
    else if (host && S.hostStuck) {
      acts.innerHTML = `<button class="btn gold" id="cl-anyway">▶ Start without them</button>
        <button class="btn secondary" id="cl-retry">↻ Retry</button>
        <button class="btn secondary" id="cl-cancel">Cancel</button>`;
      $('cl-anyway').onclick = () => hostGo();
      $('cl-retry').onclick = () => {
        S.hostStuck = false;
        for (const m of S.roster.slice(1)) if (!linkOpen(S.peers.get(m.slot))) connectTo(m, 0);
        clearTimeout(startTimer);
        startTimer = setTimeout(hostTimedOut, HOST_WAIT);
        renderLoading();
      };
      $('cl-cancel').onclick = cancelMatch;
    } else {
      acts.innerHTML = `<button class="btn secondary" id="cl-cancel">Cancel</button>`;
      $('cl-cancel').onclick = cancelMatch;
    }
  }
  function cancelMatch() {
    const host = S.mySlot === 0;
    abortLoading();
    if (host) wsSend({ t: 'party.end' });
    else UI().toast('Left the match. You\'re still in the party.');
  }

  // ------------------------------------------------------------------ public
  window.UHCoop = {
    get inGame() { return S.inGame; },
    get partySize() { return S.party?.members.length || 0; },
    isHost: () => S.party && S.me && S.party.hostId === S.me.id,
    /** called by app.js whenever sign-in state changes */
    onAuthChanged() { if (signedIn()) connect(); else disconnect(); render(); },
    /** called when the run ends or the player leaves it */
    gameEnded() {
      if (!S.inGame) return;
      S.inGame = false;
      closePeers();
      if (S.party && S.me && S.party.hostId === S.me.id) wsSend({ t: 'party.end' });
      render(); // e.g. offer "Join the match in progress" if the host is still playing
    },
    setDiff(diff) { if (S.party && S.party.hostId === S.me?.id) wsSend({ t: 'party.diff', diff }); },
    lookChanged() { if (S.party) wsSend({ t: 'party.look', look: look() }); },
    render,
    /** subscribe to server messages: chat, party, social, hello, voice-signal, render, offline */
    on(t, fn) { (listeners[t] ||= []).push(fn); },
    wsSend,
    iceServers,
    get party() { return S.party; },
    get me() { return S.me; },
    get social() { return S.social; },
    /** for tests/debugging */
    get debug() { return { hostSlot: S.hostSlot || 0, migrations: S.migrations || 0, late: !!S.late, restarts: S.restarts || 0, restart: (slot) => { const p = S.peers.get(slot); if (p) { p.troubleAt = Date.now(); restartIce(p); } }, slot: S.mySlot, inGame: S.inGame, loading: S.loading, sid: S.sid, links: S.linkStatus, peers: [...S.peers.entries()].map(([k, p]) => [k, p.u?.readyState, p.r?.readyState]), pcs: [...S.peers.values()].map((p) => p.pc), party: S.party, me: S.me }; },
    inviteLink,
    /** Home Base: share our position with the party, and read theirs */
    hubPos(x, y, aim) { S.hubAt = { x, y, aim }; if (S.party && S.ws?.readyState === 1) wsSend({ t: 'hub.pos', x, y, aim, look: look() }); },
    hubEmote(e) { const p = S.hubAt; if (p && S.party && S.ws?.readyState === 1) wsSend({ t: 'hub.pos', ...p, look: look(), emote: e }); },
    hubMates() {
      const now = Date.now();
      return Object.values(S.hubMates).filter((m) => now - m.at < 3000);
    },
  };

  // ?party=<id>&t=<token> invite links
  const q = new URLSearchParams(location.search);
  if (q.get('party') && q.get('t')) {
    S.pendingLink = { partyId: q.get('party'), token: q.get('t') };
    setTimeout(() => { if (!signedIn()) UI()?.toast('Sign in to join your friend\'s party!'); }, 2500);
  }
})();
