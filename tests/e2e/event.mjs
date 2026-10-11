// Hollow Night event: pumpkin-headed zombies, candy from kills, progress in
// the profile and the Career tab event card.
import { launch, open, check, OUT } from './lib.mjs';

const log = [];
const browser = await launch();
try {
  const p = await open(browser, 'solo', log);
  const active = await p.evaluate(() => window.UHProg.eventActive());
  check(active, 'Hollow Night is live (Oct 1 – Nov 2)');
  await p.click('#play');
  await p.waitForFunction(() => window.UHUI.state() === 'playing', null, { timeout: 15000 });
  await p.evaluate(() => { window.UHUI.send('dev|tank'); window.UHUI.send('dev|horde'); });
  await p.waitForTimeout(1500);
  await p.screenshot({ path: `${OUT}/event-pumpkins.png` });
  // keep picking level-up cards while the horde dies
  for (let i = 0; i < 20; i++) { await p.keyboard.press('Digit1'); await p.waitForTimeout(500); }
  await p.evaluate(() => window.UHUI.send('pause'));
  await p.waitForSelector('#pause.show', { timeout: 5000 });
  p.once('dialog', (d) => d.accept());
  await p.click('#quit');
  await p.waitForTimeout(1500);
  const ev = await p.evaluate(() => {
    for (const k of Object.keys(localStorage)) {
      const v = localStorage.getItem(k);
      if (v && v.includes('hollow-2026')) return JSON.parse(v).progress?.event || 'found';
    }
    return null;
  });
  check(ev && ev.candy > 0, `candy banked in the profile (${JSON.stringify(ev)})`);
  await p.click('#tabs button[data-tab="career"]');
  await p.waitForSelector('#career .event-card', { timeout: 5000 });
  check(/candy/.test(await p.textContent('#career .event-card')), 'Career tab shows the event card');
  await (await p.$('#career .event-card')).screenshot({ path: `${OUT}/event-card.png` });
  check(!log.some((l) => l.includes('pageerror')), 'no page errors');
} finally {
  console.log(log.filter((l) => l.includes('pageerror')).join('\n') || 'no page errors');
  await browser.close();
}
