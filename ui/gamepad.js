// Gamepad support (standard mapping): twin-stick play and D-pad menu navigation.
//   Left stick move · Right stick aim (fires while aiming) · RT fire · A dash/confirm
//   B interact (hold to rebuild) · X reload · Y ability · LB swap/sell · RB ultimate
//   LT ping · Start pause · Back minimap · D-pad: level-up cards and menus
(() => {
  'use strict';
  const send = (c) => window.UHUI?.send(c);
  const DEAD = 0.18;
  const prev = [];
  let lastActive = 0;
  let wasSending = false;
  let menuIdx = -1;
  let navT = 0;

  const dz = (v) => (Math.abs(v) < DEAD ? 0 : (v - Math.sign(v) * DEAD) / (1 - DEAD));
  const pressed = (pad, i) => !!pad.buttons[i] && (pad.buttons[i].pressed || pad.buttons[i].value > 0.5);

  /** visible, enabled controls in the topmost open panel (for D-pad navigation) */
  function focusables() {
    const root = document.querySelector('#vendor.show') || [...document.querySelectorAll('.overlay.show')].pop();
    if (!root) return [];
    return [...root.querySelectorAll('button:not([disabled]), [data-eq], [data-buy], input, select, a[href]')]
      .filter((el) => el.offsetParent !== null);
  }
  function moveFocus(dir) {
    const list = focusables();
    if (!list.length) return;
    const cur = list.indexOf(document.activeElement);
    menuIdx = cur < 0 ? 0 : (cur + dir + list.length) % list.length;
    list[menuIdx].focus();
    list[menuIdx].scrollIntoView?.({ block: 'nearest' });
  }

  function poll() {
    requestAnimationFrame(poll);
    const pads = navigator.getGamepads ? navigator.getGamepads() : [];
    const pad = [...pads].find((p) => p && p.connected);
    if (!pad) return;
    const now = performance.now();
    const ax = pad.axes;
    const mx = dz(ax[0] || 0), my = dz(ax[1] || 0), rx = dz(ax[2] || 0), ry = dz(ax[3] || 0);
    const rt = pad.buttons[7]?.value || 0;
    const any = mx || my || rx || ry || rt > 0.2 || pad.buttons.some((b, i) => i !== 7 && b?.pressed);
    if (any) lastActive = now;
    const edge = (i) => pressed(pad, i) && !prev[i];
    const playing = document.body.classList.contains('playing') && !document.querySelector('#vendor.show');
    if (playing) {
      if (now - lastActive < 5000) {
        send(`pad|${mx.toFixed(2)},${my.toFixed(2)},${rx.toFixed(2)},${ry.toFixed(2)},${rt > 0.3 ? 1 : 0}`);
        wasSending = true;
      } else if (wasSending) {
        send('pad|off');
        wasSending = false;
      }
      if (edge(0)) send('act|dash');
      if (edge(1)) send('use|1');
      if (!pressed(pad, 1) && prev[1]) send('use|0');
      if (edge(2)) send('act|reload');
      if (edge(3)) send('act|ability');
      if (edge(4)) send('act|swap');
      if (edge(5)) send('act|ult');
      if (edge(6)) send('act|ping');
      if (edge(8)) send('act|minimap');
      if (edge(9)) send('act|pause');
      if (edge(12)) send('act|prev');
      if (edge(13)) send('act|next');
      if (edge(14)) send('act|prev');
      if (edge(15)) send('act|next');
    } else {
      // menus: D-pad / left stick moves focus, A presses, B backs out
      if (wasSending) { send('pad|off'); wasSending = false; }
      const up = edge(12) || edge(14) || (my < -0.6 && now - navT > 220);
      const down = edge(13) || edge(15) || (my > 0.6 && now - navT > 220);
      if (up || down) { navT = now; moveFocus(up ? -1 : 1); }
      if (edge(0) && document.activeElement && document.activeElement !== document.body) document.activeElement.click();
      if (edge(1) || edge(9)) document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', code: 'Escape', bubbles: true }));
    }
    for (let i = 0; i < pad.buttons.length; i++) prev[i] = pressed(pad, i);
  }
  window.addEventListener('gamepadconnected', (e) => {
    window.UHUI?.toast?.(`🎮 ${e.gamepad.id.split('(')[0].trim() || 'Controller'} connected`);
  });
  requestAnimationFrame(poll);
})();
