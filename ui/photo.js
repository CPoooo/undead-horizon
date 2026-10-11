// Photo mode: a free camera over the paused world. The game hides its HUD and
// draws vignette/letterbox (`photo|...` commands); this panel adds filters,
// controls (drag / WASD / wheel / pinch) and captures a PNG with the filter
// baked in (share sheet on phones, download elsewhere).
(() => {
  const $ = (id) => document.getElementById(id);
  const send = (c) => window.UHUI?.send(c);
  const FILTERS = [
    ['None', ''],
    ['Noir', 'grayscale(1) contrast(1.3) brightness(0.95)'],
    ['Vintage', 'sepia(0.55) contrast(1.05) saturate(0.85) brightness(1.05)'],
    ['Vivid', 'saturate(1.6) contrast(1.15)'],
    ['Frost', 'hue-rotate(-18deg) saturate(0.75) brightness(1.08) contrast(1.1)'],
    ['Blood Moon', 'sepia(0.4) hue-rotate(-35deg) saturate(2.2) contrast(1.15)'],
    ['Dream', 'brightness(1.12) contrast(1.05) saturate(1.25) blur(0.6px)'],
  ];
  const st = { on: false, filter: 0, zoom: 1, keys: new Set(), ptrs: new Map(), pinch: 0, raf: 0 };
  const canvas = () => $('game');

  function build() {
    const el = document.createElement('div');
    el.id = 'photo';
    el.innerHTML = `
      <div class="ph-hint">${matchMedia('(pointer: coarse)').matches ? 'Drag to move · pinch to zoom' : 'Drag or WASD to move · wheel to zoom · P to capture · Esc to exit'}</div>
      <div class="ph-bar">
        <div class="ph-filters">${FILTERS.map((f, i) => `<button data-phf="${i}" class="${i === 0 ? 'on' : ''}">${f[0]}</button>`).join('')}</div>
        <div class="ph-row">
          <label>🔍 <input type="range" id="ph-zoom" min="0.5" max="3" step="0.05" value="1"></label>
          <label>◐ <input type="range" id="ph-vig" min="0" max="1" step="0.05" value="0.35"></label>
          <label class="ph-tg"><input type="checkbox" id="ph-me" checked> Me</label>
          <label class="ph-tg"><input type="checkbox" id="ph-bars"> Cinema</label>
        </div>
        <div class="ph-row">
          <button class="btn gold" id="ph-shot">📸 Capture</button>
          <button class="btn secondary" id="ph-exit">✕ Exit</button>
        </div>
      </div>`;
    document.body.appendChild(el);
    el.querySelectorAll('[data-phf]').forEach((b) => (b.onclick = () => setFilter(Number(b.dataset.phf))));
    $('ph-zoom').oninput = (e) => setZoom(Number(e.target.value));
    $('ph-vig').oninput = (e) => send(`photo|vig=${e.target.value}`);
    $('ph-me').onchange = (e) => send(`photo|player=${e.target.checked ? 1 : 0}`);
    $('ph-bars').onchange = (e) => send(`photo|bars=${e.target.checked ? 1 : 0}`);
    $('ph-shot').onclick = capture;
    $('ph-exit').onclick = exit;
  }

  function setFilter(i) {
    st.filter = i;
    canvas().style.filter = FILTERS[i][1];
    document.querySelectorAll('[data-phf]').forEach((b) => b.classList.toggle('on', Number(b.dataset.phf) === i));
  }
  function setZoom(z) {
    st.zoom = Math.max(0.5, Math.min(3, z));
    $('ph-zoom').value = st.zoom;
    send(`photo|zoom=${st.zoom.toFixed(2)}`);
  }

  function enter() {
    if (st.on || window.UHCoop?.inGame) return;
    if (!$('photo')) build();
    st.on = true;
    send('photo|on');
    $('pause').classList.remove('show');
    document.body.classList.add('photo-mode');
    $('photo').classList.add('show');
    setFilter(0);
    setZoom(1);
    $('ph-vig').value = 0.35;
    $('ph-me').checked = true;
    $('ph-bars').checked = false;
    const loop = () => {
      if (!st.on) return;
      let dx = 0, dy = 0;
      const k = st.keys;
      if (k.has('KeyA') || k.has('ArrowLeft')) dx -= 12;
      if (k.has('KeyD') || k.has('ArrowRight')) dx += 12;
      if (k.has('KeyW') || k.has('ArrowUp')) dy -= 12;
      if (k.has('KeyS') || k.has('ArrowDown')) dy += 12;
      if (dx || dy) send(`photo|pan=${dx},${dy}`);
      st.raf = requestAnimationFrame(loop);
    };
    st.raf = requestAnimationFrame(loop);
  }

  function exit() {
    if (!st.on) return;
    st.on = false;
    cancelAnimationFrame(st.raf);
    st.keys.clear();
    st.ptrs.clear();
    send('photo|off');
    canvas().style.filter = '';
    $('photo').classList.remove('show');
    document.body.classList.remove('photo-mode');
    $('pause').classList.add('show');
  }

  async function capture() {
    const src = canvas();
    const out = document.createElement('canvas');
    out.width = src.width;
    out.height = src.height;
    const x = out.getContext('2d');
    if ('filter' in x) x.filter = FILTERS[st.filter][1] || 'none';
    x.drawImage(src, 0, 0);
    x.filter = 'none';
    // small watermark
    const s = Math.max(12, Math.round(out.height / 40));
    x.font = `900 ${s}px Inter, 'Segoe UI', system-ui, sans-serif`;
    x.textAlign = 'right';
    x.textBaseline = 'bottom';
    x.fillStyle = 'rgba(255,255,255,0.75)';
    x.shadowColor = 'rgba(0,0,0,0.8)';
    x.shadowBlur = s / 3;
    x.fillText('UNDEAD HORIZON', out.width - s, out.height - s * 0.8);
    send('sfx|start');
    const flash = document.createElement('div');
    flash.className = 'ph-flash';
    document.body.appendChild(flash);
    setTimeout(() => flash.remove(), 450);
    const blob = await new Promise((r) => out.toBlob(r, 'image/png'));
    if (!blob) return;
    const name = `undead-horizon-${new Date().toISOString().slice(0, 19).replace(/[:T]/g, '-')}.png`;
    const file = new File([blob], name, { type: 'image/png' });
    const coarse = matchMedia('(pointer: coarse)').matches;
    if (coarse && navigator.canShare?.({ files: [file] })) {
      try { await navigator.share({ files: [file], title: 'Undead Horizon' }); return; } catch (e) { if (e?.name === 'AbortError') return; }
    }
    const a = document.createElement('a');
    a.href = URL.createObjectURL(blob);
    a.download = name;
    document.body.appendChild(a);
    a.click();
    a.remove();
    setTimeout(() => URL.revokeObjectURL(a.href), 4000);
    window.UHUI?.toast?.('📸 Saved to your downloads');
  }

  // keys: handled before the game sees them (Esc would otherwise resume)
  document.addEventListener('keydown', (e) => {
    if (!st.on || e.target?.tagName === 'INPUT' && e.target.type !== 'range' && e.target.type !== 'checkbox') return;
    if (e.key === 'Escape') { e.preventDefault(); e.stopPropagation(); exit(); return; }
    if (e.code === 'KeyP' || e.code === 'F12') { if (e.code === 'KeyP') { e.preventDefault(); capture(); } return; }
    st.keys.add(e.code);
    e.stopPropagation();
  }, true);
  document.addEventListener('keyup', (e) => { if (st.on) { st.keys.delete(e.code); e.stopPropagation(); } }, true);

  // drag to pan, pinch / wheel to zoom
  const cv = () => canvas();
  addEventListener('pointerdown', (e) => { if (st.on && e.target === cv()) { st.ptrs.set(e.pointerId, { x: e.clientX, y: e.clientY }); st.pinch = 0; } });
  addEventListener('pointermove', (e) => {
    if (!st.on || !st.ptrs.has(e.pointerId)) return;
    const p = st.ptrs.get(e.pointerId);
    if (st.ptrs.size === 1) send(`photo|pan=${Math.round(p.x - e.clientX)},${Math.round(p.y - e.clientY)}`);
    p.x = e.clientX; p.y = e.clientY;
    if (st.ptrs.size === 2) {
      const [a, b] = [...st.ptrs.values()];
      const dist = Math.hypot(a.x - b.x, a.y - b.y);
      if (st.pinch) setZoom(st.zoom * (dist / st.pinch));
      st.pinch = dist;
    }
  });
  const up = (e) => { st.ptrs.delete(e.pointerId); st.pinch = 0; };
  addEventListener('pointerup', up);
  addEventListener('pointercancel', up);
  addEventListener('wheel', (e) => { if (st.on) { e.preventDefault(); setZoom(st.zoom * (e.deltaY < 0 ? 1.08 : 1 / 1.08)); } }, { passive: false });

  window.UHPhoto = { enter, exit, get on() { return st.on; } };
})();
