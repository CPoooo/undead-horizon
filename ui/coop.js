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
    // game transport
    peers: new Map(), // slot -> { pc, u, r, uid }
    inbox: [],
    inGame: false,
    mySlot: 0,
    roster: null,
  };

  // ------------------------------------------------------------------ Rust bridge
  window.uhNetPoll = () => (S.inbox.length ? S.inbox.shift() : undefined);
  window.uhNetSend = (slot, reliable, data) => {
    const peer = S.mySlot === 0 ? S.peers.get(slot) : S.peers.get(0);
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
        if (S.party && S.party.members.some((x) => x.id === S.me?.id)) {
          // keep our look current for everyone in the lobby
          const mine = S.party.members.find((x) => x.id === S.me.id);
          const l = look();
          if (mine && (mine.look?.skin !== l.skin || mine.look?.hat !== l.hat || mine.look?.gun !== l.gun)) wsSend({ t: 'party.look', look: l });
        }
        if (!was && S.party) UI().toast('You\'re in a party. Invite friends or share the link!');
        if (S.party && S.party.diff) UI().setDiff?.(S.party.diff, S.party.hostId !== S.me?.id);
        if (!S.party) UI().setDiff?.(null, false);
        break;
      }
      case 'party.start':
        S.party = m.party;
        beginNetGame();
        break;
      case 'invite':
        showInvite(m.partyId, m.fromName);
        break;
      case 'signal':
        onSignal(m.from, m.data);
        break;
      case 'toast':
        UI().toast(m.msg);
        break;
      case 'error':
        UI().toast(m.msg);
        break;
    }
    render();
  }

  // ------------------------------------------------------------------ rendering
  function render() {
    renderParty();
    renderFriends();
    renderLobbyMates();
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

  function renderParty() {
    const box = $('party-box');
    if (!box) return;
    if (!window.UHCloud) { box.innerHTML = ''; return; }
    if (!signedIn()) {
      box.innerHTML = `<div class="party empty-party"><b>👥 Co-op</b><span>Sign in to party up with up to 2 friends.</span>
        <button class="btn small secondary" data-signin>Sign in</button></div>`;
      return;
    }
    if (S.status !== 'online' || !S.me) {
      box.innerHTML = `<div class="party empty-party"><b>👥 Co-op</b><span>${S.status === 'error' ? '⚠ Could not reach the lobby server, retrying…' : '⏳ Connecting to the lobby server…'}</span></div>`;
      return;
    }
    if (!S.party) {
      box.innerHTML = `<div class="party empty-party"><b>👥 Co-op</b><span>Play Infinite with up to 2 friends.</span>
        <button class="btn small blue" id="party-create">Create party</button></div>`;
      $('party-create').onclick = () => wsSend({ t: 'party.create', look: look() });
      return;
    }
    const p = S.party;
    const amHost = p.hostId === S.me?.id;
    const me = p.members.find((m) => m.id === S.me?.id);
    const allReady = p.members.every((m) => m.ready);
    const slots = [0, 1, 2].map((i) => {
      const m = p.members[i];
      if (!m) return `<div class="pm empty">Open slot<span>invite a friend</span></div>`;
      return `<div class="pm ${m.online ? '' : 'offline'}">
        <span class="dot ${m.online ? 'on' : ''}"></span>
        <b>${esc(m.name)}${m.id === p.hostId ? ' 👑' : ''}${m.id === S.me?.id ? ' (you)' : ''}</b>
        <span class="rdy ${m.ready ? 'yes' : ''}">${m.ready ? 'READY' : 'not ready'}</span>
        ${amHost && m.id !== S.me?.id ? `<button class="x" data-kick="${esc(m.id)}" title="Remove from party">×</button>` : ''}
      </div>`;
    }).join('');
    box.innerHTML = `<div class="party">
      <div class="party-head"><b>👥 Party ${p.members.length}/3</b><span class="hint">${esc(window.UHDiffName?.(p.diff) || p.diff)}${amHost ? ' · you set the difficulty' : ' · host sets difficulty'}</span></div>
      <div class="pms">${slots}</div>
      <div class="row" style="margin-top:8px">
        <button class="btn small secondary" id="party-link" title="Anyone signed in with this link joins your party and becomes your friend">🔗 Copy invite link</button>
        <button class="btn small secondary" id="party-leave">Leave</button>
      </div>
      ${amHost
        ? `<span class="tip-wrap"><button class="btn gold" id="party-start" style="margin-top:10px" ${allReady && p.members.length > 1 ? '' : 'disabled'}>▶ START CO-OP</button>
           <span class="tip">${p.members.length < 2 ? 'Invite at least one friend first' : allReady ? 'Everyone is ready!' : 'Waiting for everyone to ready up'}</span></span>`
        : `<button class="btn ${me?.ready ? 'secondary' : ''}" id="party-ready" style="margin-top:10px">${me?.ready ? 'Not ready' : '✔ Ready up'}</button>`}
    </div>`;
    $('party-link').onclick = async () => {
      const link = inviteLink();
      try { await navigator.clipboard.writeText(link); UI().toast('Invite link copied! Send it to your friends 🧟'); }
      catch { prompt('Copy this invite link:', link); }
    };
    $('party-leave').onclick = () => wsSend({ t: 'party.leave' });
    if ($('party-ready')) $('party-ready').onclick = () => wsSend({ t: 'party.ready', ready: !me?.ready });
    if ($('party-start')) $('party-start').onclick = () => wsSend({ t: 'party.start' });
    box.querySelectorAll('[data-kick]').forEach((b) => (b.onclick = () => wsSend({ t: 'party.kick', id: b.dataset.kick })));
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
      ${s.outgoing.length ? `<h3>Sent requests</h3>${s.outgoing.map((f) => `<div class="frow offline"><span>${esc(f.name)}</span><span class="tag">pending</span></div>`).join('')}` : ''}`;
    $('copy-code').onclick = async () => { try { await navigator.clipboard.writeText(S.me?.code || ''); UI().toast('Friend code copied'); } catch {} };
    $('fadd').onsubmit = (e) => { e.preventDefault(); const c = $('fadd-code').value.trim(); if (c) { wsSend({ t: 'friends.add', code: c }); $('fadd-code').value = ''; } };
    el.querySelectorAll('[data-accept]').forEach((b) => (b.onclick = () => wsSend({ t: 'friends.accept', id: b.dataset.accept })));
    el.querySelectorAll('[data-remove]').forEach((b) => (b.onclick = () => wsSend({ t: 'friends.remove', id: b.dataset.remove })));
    el.querySelectorAll('[data-invite]').forEach((b) => (b.onclick = () => { wsSend({ t: 'party.invite', id: b.dataset.invite }); UI().toast('Invite sent'); }));
    el.querySelectorAll('[data-join]').forEach((b) => (b.onclick = () => wsSend({ t: 'party.join', partyId: b.dataset.join, look: look() })));
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
  function slotsFor(party) {
    // host = slot 0, everyone else in join order
    const others = party.members.filter((m) => m.id !== party.hostId);
    const list = [party.members.find((m) => m.id === party.hostId), ...others].filter(Boolean);
    return list.map((m, i) => ({ ...m, slot: i }));
  }

  function wire(slot, pc) {
    const peer = { pc, u: null, r: null, slot };
    const onChannel = (ch) => {
      ch.binaryType = 'arraybuffer';
      if (ch.label === 'u') peer.u = ch; else peer.r = ch;
      ch.onmessage = (e) => {
        if (typeof e.data === 'string') return onControl(slot, JSON.parse(e.data));
        S.inbox.push({ from: slot, data: new Uint8Array(e.data) });
        if (S.inbox.length > 2000) S.inbox.splice(0, 500);
      };
      ch.onopen = () => checkAllOpen();
      ch.onclose = () => peerGone(slot);
    };
    pc.ondatachannel = (e) => onChannel(e.channel);
    pc.onconnectionstatechange = () => {
      if (['failed', 'closed'].includes(pc.connectionState)) peerGone(slot);
    };
    peer.onChannel = onChannel;
    S.peers.set(slot, peer);
    return peer;
  }

  async function gathered(pc) {
    if (pc.iceGatheringState === 'complete') return;
    await new Promise((res) => {
      const t = setTimeout(res, 2500);
      pc.addEventListener('icegatheringstatechange', () => { if (pc.iceGatheringState === 'complete') { clearTimeout(t); res(); } });
    });
  }

  let startTimer = 0;
  async function beginNetGame() {
    closePeers();
    const roster = slotsFor(S.party);
    S.roster = roster;
    const mine = roster.find((m) => m.id === S.me.id);
    if (!mine) return;
    S.mySlot = mine.slot;
    UI().toast('Connecting to your party…');
    if (S.mySlot === 0) {
      // host: offer to every client (non-trickle: candidates are inside the SDP)
      for (const m of roster.slice(1)) {
        const pc = new RTCPeerConnection({ iceServers: ICE });
        const peer = wire(m.slot, pc);
        peer.uid = m.id;
        peer.onChannel(pc.createDataChannel('u', { ordered: false, maxRetransmits: 0 }));
        peer.onChannel(pc.createDataChannel('r', { ordered: true }));
        await pc.setLocalDescription(await pc.createOffer());
        await gathered(pc);
        wsSend({ t: 'signal', to: m.id, data: { kind: 'offer', sdp: pc.localDescription.sdp, slot: m.slot } });
      }
      clearTimeout(startTimer);
      startTimer = setTimeout(() => hostGo(true), 15000);
    }
  }

  async function onSignal(from, data) {
    if (!data || !S.party) return;
    if (data.kind === 'offer' && S.mySlot !== 0) {
      const pc = new RTCPeerConnection({ iceServers: ICE });
      const peer = wire(0, pc);
      peer.uid = from;
      await pc.setRemoteDescription({ type: 'offer', sdp: data.sdp });
      await pc.setLocalDescription(await pc.createAnswer());
      await gathered(pc);
      wsSend({ t: 'signal', to: from, data: { kind: 'answer', sdp: pc.localDescription.sdp } });
    } else if (data.kind === 'answer' && S.mySlot === 0) {
      const peer = [...S.peers.values()].find((p) => p.uid === from);
      if (peer && peer.pc.signalingState === 'have-local-offer') await peer.pc.setRemoteDescription({ type: 'answer', sdp: data.sdp });
    }
  }

  function checkAllOpen() {
    if (S.mySlot !== 0 || S.inGame) return;
    const want = (S.roster || []).length - 1;
    const open = [...S.peers.values()].filter((p) => p.u?.readyState === 'open' && p.r?.readyState === 'open').length;
    if (open >= want) hostGo(false);
  }

  function opts(role, present) {
    const names = [0, 1, 2].map((i) => (S.roster.find((m) => m.slot === i)?.name || '').replace(/[;,=|]/g, ''));
    return `role=${role};me=${S.mySlot};names=${names.join(',')};present=${present.join(',')};mode=infinite;diff=${S.party.diff};name=${names[S.mySlot]};${UI().equipString()}`;
  }

  function hostGo(timedOut) {
    if (S.inGame || S.mySlot !== 0) return;
    clearTimeout(startTimer);
    const present = [0, 1, 2].map((i) => {
      if (i === 0) return 1;
      const p = S.peers.get(i);
      return p && p.r?.readyState === 'open' && p.u?.readyState === 'open' ? 1 : 0;
    });
    if (timedOut) {
      const missing = S.roster.filter((m) => m.slot > 0 && !present[m.slot]).map((m) => m.name);
      if (missing.length) UI().toast(`Couldn't connect to ${missing.join(', ')} (network/firewall).`);
    }
    for (const [slot, p] of S.peers) if (present[slot]) p.r.send(JSON.stringify({ t: 'go', present }));
    startGame(opts('host', present));
  }

  function onControl(from, m) {
    if (m.t === 'go' && S.mySlot !== 0 && !S.inGame) startGame(opts('client', m.present));
  }

  function startGame(o) {
    S.inGame = true;
    S.inbox.length = 0;
    UI().startCoop(o);
  }

  function peerGone(slot) {
    if (!S.peers.has(slot)) return;
    S.peers.delete(slot);
    if (S.inGame) send('peerleft|' + slot);
  }

  function closePeers() {
    for (const p of S.peers.values()) { try { p.pc.close(); } catch {} }
    S.peers.clear();
    S.inbox.length = 0;
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
    },
    setDiff(diff) { if (S.party && S.party.hostId === S.me?.id) wsSend({ t: 'party.diff', diff }); },
    lookChanged() { if (S.party) wsSend({ t: 'party.look', look: look() }); },
    render,
    /** for tests/debugging */
    get debug() { return { slot: S.mySlot, inGame: S.inGame, peers: [...S.peers.entries()].map(([k, p]) => [k, p.u?.readyState, p.r?.readyState]), party: S.party, me: S.me }; },
    inviteLink,
  };

  // ?party=<id>&t=<token> invite links
  const q = new URLSearchParams(location.search);
  if (q.get('party') && q.get('t')) {
    S.pendingLink = { partyId: q.get('party'), token: q.get('t') };
    setTimeout(() => { if (!signedIn()) UI()?.toast('Sign in to join your friend\'s party!'); }, 2500);
  }
})();
