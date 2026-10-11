// Party voice chat: a small full mesh of audio-only WebRTC links between
// everyone in the party who joined voice (max 3, so at most 2 links each).
// Separate from the game links, so it works in the lobby and during matches,
// and survives a match ending. Signaling rides the social socket as
// "v-offer" / "v-answer" / "v-ice"; the lower user id always offers.
// Open mic by default; push-to-talk holds V. Speaking rings poll at 10 Hz.
(() => {
  'use strict';
  const $ = (id) => document.getElementById(id);
  const C = () => window.UHCoop;
  const store = {
    get: (k, d) => { try { const v = localStorage.getItem('uh.voice.' + k); return v == null ? d : JSON.parse(v); } catch { return d; } },
    set: (k, v) => { try { localStorage.setItem('uh.voice.' + k, JSON.stringify(v)); } catch {} },
  };

  const V = {
    on: false,
    joining: false,
    muted: store.get('muted', false),
    ptt: store.get('ptt', false),
    pttDown: false,
    stream: null,
    ctx: null,
    localAn: null,
    peers: new Map(), // uid -> { pc, nonce, audio, an, described, pending: [] }
    volumes: store.get('volumes', {}),
    speaking: new Set(),
  };

  const meId = () => C()?.me?.id;
  const blocked = (uid) => (C()?.social?.blocked || []).some((b) => b.id === uid);

  function micLive() {
    return V.on && !V.muted && (!V.ptt || V.pttDown);
  }
  function applyMic() {
    for (const t of V.stream?.getAudioTracks() || []) t.enabled = micLive();
    render();
  }

  // ---------------------------------------------------------------- join / leave
  async function join() {
    if (V.on || V.joining || !C()?.party) return;
    V.joining = true;
    render();
    try {
      V.stream = await navigator.mediaDevices.getUserMedia({
        audio: { echoCancellation: true, noiseSuppression: true, autoGainControl: true },
        video: false,
      });
    } catch (e) {
      V.joining = false;
      render();
      window.UHUI?.toast(e?.name === 'NotAllowedError' ? 'Microphone blocked. Allow it in your browser to use voice.' : 'No microphone found.');
      return;
    }
    V.on = true;
    V.joining = false;
    try {
      V.ctx = V.ctx || new (window.AudioContext || window.webkitAudioContext)();
      V.ctx.resume?.();
      V.localAn = analyser(V.stream);
    } catch {}
    applyMic();
    C().wsSend({ t: 'party.voice', on: true });
    window.UHChat?.system('You joined voice' + (V.ptt ? ' (hold V to talk)' : ''));
    sync();
  }

  function leave(silent) {
    if (!V.on && !V.joining) return;
    V.on = false;
    for (const uid of [...V.peers.keys()]) drop(uid);
    for (const t of V.stream?.getTracks() || []) t.stop();
    V.stream = null;
    V.localAn = null;
    V.speaking.clear();
    if (!silent) C()?.wsSend({ t: 'party.voice', on: false });
    render();
  }

  function analyser(stream) {
    if (!V.ctx) return null;
    const src = V.ctx.createMediaStreamSource(stream);
    const an = V.ctx.createAnalyser();
    an.fftSize = 512;
    src.connect(an);
    return { an, buf: new Uint8Array(an.fftSize) };
  }
  function level(a) {
    if (!a) return 0;
    a.an.getByteTimeDomainData(a.buf);
    let sum = 0;
    for (let i = 0; i < a.buf.length; i += 4) { const v = (a.buf[i] - 128) / 128; sum += v * v; }
    return Math.sqrt(sum / (a.buf.length / 4));
  }

  // ---------------------------------------------------------------- mesh
  /** Connect to every party member in voice; drop links to anyone who left. */
  function sync() {
    const p = C()?.party;
    const want = new Set();
    if (V.on && p) for (const m of p.members) if (m.voice && m.id !== meId() && !blocked(m.id)) want.add(m.id);
    for (const uid of [...V.peers.keys()]) if (!want.has(uid)) drop(uid);
    for (const uid of want) if (!V.peers.has(uid) && meId() < uid) offer(uid);
    render();
  }

  function drop(uid) {
    const p = V.peers.get(uid);
    if (!p) return;
    V.peers.delete(uid);
    try { p.pc.close(); } catch {}
    if (p.audio) { p.audio.srcObject = null; p.audio.remove(); }
    V.speaking.delete(uid);
  }

  async function makePeer(uid, nonce) {
    const pc = new RTCPeerConnection({ iceServers: await C().iceServers() });
    const peer = { pc, nonce, audio: null, an: null, described: false, pending: [], iceQ: [], iceT: 0 };
    for (const t of V.stream.getAudioTracks()) pc.addTrack(t, V.stream);
    pc.onicecandidate = (e) => {
      if (!e.candidate) return;
      peer.iceQ.push(e.candidate.toJSON());
      if (!peer.iceT) peer.iceT = setTimeout(() => {
        C().wsSend({ t: 'signal', to: uid, data: { kind: 'v-ice', nonce, c: peer.iceQ } });
        peer.iceQ = [];
        peer.iceT = 0;
      }, 150);
    };
    pc.ontrack = (e) => {
      if (peer.audio) return;
      const a = document.createElement('audio');
      a.autoplay = true;
      a.setAttribute('playsinline', '');
      a.srcObject = e.streams[0] || new MediaStream([e.track]);
      a.volume = Math.min(1, V.volumes[uid] ?? 1);
      document.body.appendChild(a);
      a.play?.().catch(() => {});
      peer.audio = a;
      try { peer.an = analyser(a.srcObject); } catch {}
      render();
    };
    pc.onconnectionstatechange = () => {
      if (V.peers.get(uid) !== peer) return;
      if (pc.connectionState === 'failed') {
        // re-negotiate from scratch; the offerer side drives it
        drop(uid);
        if (meId() < uid) setTimeout(() => { if (V.on && !V.peers.has(uid)) sync(); }, 1500);
      }
      render();
    };
    V.peers.set(uid, peer);
    return peer;
  }

  async function offer(uid) {
    const nonce = Math.random().toString(36).slice(2, 10);
    const peer = await makePeer(uid, nonce);
    await peer.pc.setLocalDescription(await peer.pc.createOffer());
    C().wsSend({ t: 'signal', to: uid, data: { kind: 'v-offer', nonce, sdp: peer.pc.localDescription.sdp } });
  }

  async function onSignal({ from, data }) {
    if (!V.on || blocked(from)) return;
    if (data.kind === 'v-offer') {
      drop(from);
      const peer = await makePeer(from, data.nonce);
      await peer.pc.setRemoteDescription({ type: 'offer', sdp: data.sdp });
      await peer.pc.setLocalDescription(await peer.pc.createAnswer());
      C().wsSend({ t: 'signal', to: from, data: { kind: 'v-answer', nonce: data.nonce, sdp: peer.pc.localDescription.sdp } });
      peer.described = true;
      for (const c of peer.pending.splice(0)) await peer.pc.addIceCandidate(c).catch(() => {});
    } else {
      const peer = V.peers.get(from);
      if (!peer || peer.nonce !== data.nonce) return;
      if (data.kind === 'v-answer' && peer.pc.signalingState === 'have-local-offer') {
        await peer.pc.setRemoteDescription({ type: 'answer', sdp: data.sdp });
        peer.described = true;
        for (const c of peer.pending.splice(0)) await peer.pc.addIceCandidate(c).catch(() => {});
      } else if (data.kind === 'v-ice') {
        for (const c of data.c || []) {
          if (peer.described) await peer.pc.addIceCandidate(c).catch(() => {});
          else peer.pending.push(c);
        }
      }
    }
  }

  // ---------------------------------------------------------------- speaking indicators (10 Hz)
  setInterval(() => {
    if (!V.on) return;
    const now = new Set();
    if (micLive() && level(V.localAn) > 0.04) now.add(meId());
    for (const [uid, p] of V.peers) if (level(p.an) > 0.04) now.add(uid);
    const changed = now.size !== V.speaking.size || [...now].some((u) => !V.speaking.has(u));
    if (!changed) return;
    V.speaking = now;
    for (const el of document.querySelectorAll('[data-voice-uid]')) el.classList.toggle('speaking', now.has(el.dataset.voiceUid));
    renderHud();
  }, 100);

  // ---------------------------------------------------------------- UI
  function render() {
    const ctl = $('voice-ctl');
    if (ctl) {
      if (!C()?.party) ctl.innerHTML = '';
      else if (!V.on) {
        ctl.innerHTML = `<div class="vrow"><button class="btn small blue" id="v-join" ${V.joining ? 'disabled' : ''}>🎙 ${V.joining ? 'Joining…' : 'Join voice'}</button></div>`;
        $('v-join').onclick = join;
      } else {
        const links = [...V.peers.values()].filter((p) => p.pc.connectionState === 'connected').length;
        ctl.innerHTML = `<div class="vrow">
          <button class="btn small ${V.muted ? '' : 'secondary'}" id="v-mute" data-voice-uid="${meId()}">${V.muted ? '🔇 Unmute' : '🎙 Mute'}</button>
          <button class="btn small secondary" id="v-ptt" title="Push-to-talk: hold V">${V.ptt ? 'PTT: V' : 'Open mic'}</button>
          <button class="btn small secondary" id="v-leave">Leave voice</button>
          <span class="hint">${links} connected</span></div>`;
        $('v-mute').onclick = () => { V.muted = !V.muted; store.set('muted', V.muted); applyMic(); };
        $('v-ptt').onclick = () => { V.ptt = !V.ptt; store.set('ptt', V.ptt); applyMic(); };
        $('v-leave').onclick = () => leave();
      }
      for (const el of document.querySelectorAll('[data-voice-uid]')) el.classList.toggle('speaking', V.speaking.has(el.dataset.voiceUid));
    }
    const mb = $('mic-btn');
    if (mb) { mb.style.display = V.on ? '' : 'none'; mb.classList.toggle('off', !micLive()); mb.textContent = micLive() ? '🎙' : '🔇'; }
    renderHud();
  }
  function renderHud() {
    const hud = $('voice-hud');
    if (!hud) return;
    const names = new Map((C()?.party?.members || []).map((m) => [m.id, m.name]));
    hud.innerHTML = V.on ? [...V.speaking].map((u) => `<span class="vchip">🔊 ${String(names.get(u) || '').replace(/[<>&]/g, '')}</span>`).join('') : '';
  }

  function init() {
    const c = C();
    c.on('voice-signal', (m) => onSignal(m).catch((e) => console.warn('[voice]', e)));
    c.on('party', (m) => { if (!m.party) leave(true); else sync(); });
    c.on('social', () => { if (V.on) sync(); });
    c.on('render', render);
    c.on('offline', () => leave(true));
    $('mic-btn').onclick = () => { V.muted = !V.muted; store.set('muted', V.muted); applyMic(); };
    // push-to-talk (ignored while typing)
    const typingIn = (e) => e.target && (e.target.tagName === 'INPUT' || e.target.tagName === 'TEXTAREA');
    window.addEventListener('keydown', (e) => { if (e.code === 'KeyV' && V.ptt && V.on && !typingIn(e) && !V.pttDown) { V.pttDown = true; applyMic(); } });
    window.addEventListener('keyup', (e) => { if (e.code === 'KeyV' && V.pttDown) { V.pttDown = false; applyMic(); } });
    window.addEventListener('blur', () => { if (V.pttDown) { V.pttDown = false; applyMic(); } });
  }
  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', init); else setTimeout(init, 0);

  window.UHVoice = {
    join, leave,
    getVolume: (uid) => V.volumes[uid] ?? 1,
    setVolume(uid, v) {
      V.volumes[uid] = v;
      store.set('volumes', V.volumes);
      const p = V.peers.get(uid);
      if (p?.audio) p.audio.volume = Math.min(1, v);
    },
    get debug() {
      return { on: V.on, muted: V.muted, ptt: V.ptt, live: micLive(), speaking: [...V.speaking],
        peers: [...V.peers.entries()].map(([u, p]) => ({ uid: u, state: p.pc.connectionState, audio: !!p.audio, level: level(p.an) })) };
    },
  };
})();
