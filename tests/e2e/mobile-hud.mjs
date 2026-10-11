// Phone HUD: Siege with a boss on portrait, landscape and small phones.
// No crash (a short landscape screen used to panic in the Siege HUD), the
// thumb buttons stay on screen without overlapping, and ☢/Q get charge rings.
import { launch, check, OUT, URL0 } from './lib.mjs';

const log = [];
const browser = await launch();
try {
  for (const [tag, vp] of [['portrait', { width: 390, height: 844 }], ['landscape', { width: 844, height: 390 }], ['small', { width: 360, height: 640 }]]) {
    const ctx = await browser.newContext({ viewport: vp, isMobile: true, hasTouch: true, deviceScaleFactor: 2 });
    const p = await ctx.newPage();
    p.on('pageerror', (e) => log.push(`${tag} pageerror: ${e.message}`));
    await p.goto(URL0);
    await p.waitForFunction(() => document.getElementById('loader')?.classList.contains('hide'), null, { timeout: 30000 });
    await p.evaluate(() => document.querySelector('[data-mode="siege"]').click());
    await p.evaluate(() => document.getElementById('play').click());
    await p.waitForFunction(() => window.UHUI.state() === 'playing', null, { timeout: 15000 });
    await p.waitForTimeout(1000);
    await p.evaluate(() => { window.UHUI.send('dev|tank'); window.UHUI.send('dev|boss=1'); });
    await p.waitForTimeout(3500);
    await p.screenshot({ path: `${OUT}/mobile-hud-${tag}.png` });
    const r = await p.evaluate(() => {
      const box = (id) => { const b = document.getElementById(id).getBoundingClientRect(); return { x: b.x, y: b.y, r: b.right, b: b.bottom, w: b.width }; };
      return { ids: ['t-ult', 't-abil', 't-dash', 't-ready'].map((id) => [id, box(id)]), k: document.getElementById('t-ult').style.getPropertyValue('--k'), vw: innerWidth, vh: innerHeight };
    });
    const shown = r.ids.filter(([, b]) => b.w > 0);
    const inside = shown.every(([, b]) => b.x >= 0 && b.y >= 0 && b.r <= r.vw && b.b <= r.vh);
    const overlap = shown.some(([a, A], i) => shown.slice(i + 1).some(([, B]) => A.x < B.r && B.x < A.r && A.y < B.b && B.y < A.b));
    check(inside && !overlap, `${tag}: thumb buttons on screen, no overlaps (${shown.map((s) => s[0]).join(', ')})`);
    check(r.k !== '', `${tag}: ultimate shows its charge ring (--k=${r.k})`);
    await ctx.close();
  }
  check(!log.some((l) => l.includes('pageerror')), 'no page errors or panics on any phone size');
} finally {
  console.log(log.join('\n') || 'no page errors');
  await browser.close();
}
