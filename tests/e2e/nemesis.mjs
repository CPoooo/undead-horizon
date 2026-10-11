// Nemesis: dying creates a named Nemesis (kept in settings); a later run with
// it from wave 8 announces it and shows its name plate.
import { launch, open, check, OUT } from './lib.mjs';

const log = [];
const browser = await launch();
try {
  const p = await open(browser, 'solo', log);
  const settings = () => p.evaluate(() => JSON.parse(localStorage.getItem('undeadHorizon.settings.v1') || '{}'));
  // a doomed run: whatever kills us becomes the Nemesis
  await p.evaluate(() => window.UHUI.send('load|mode=infinite;diff=nightmare;savewave=6;level=3;hp=1;maxhp=100'));
  await p.evaluate(() => document.getElementById('menu').classList.remove('show'));
  await p.waitForSelector('#gameover.show', { timeout: 60000 });
  await p.waitForFunction(() => /Nemesis/.test(document.getElementById('record').textContent), null, { timeout: 10000 });
  check(true, `death screen names the Nemesis: "${await p.textContent('#record')}"`);
  const n = (await settings()).nemesis;
  check(n && n.rank === 1 && n.name.length > 3 && Number.isInteger(n.kind), `the Nemesis is remembered (${JSON.stringify(n)})`);
  // next run: it comes hunting by wave 8
  await p.click('#tomenu');
  await p.evaluate((n) => window.UHUI.send(`load|mode=infinite;diff=rookie;savewave=8;level=5;hp=100000;maxhp=100000;nemesis=${n.kind},${n.rank},${n.name}`), n);
  await p.evaluate(() => document.getElementById('menu').classList.remove('show'));
  await p.evaluate(() => { window.__says = []; const o = window.uhEvent; window.uhEvent = (k, d) => { if (k === 'say') window.__says.push(String(d)); return o(k, d); }; });
  await p.waitForFunction(() => window.__says.some((s) => /taste|waiting|scar|hunt|drag you/i.test(s)), null, { timeout: 15000 });
  check(true, `the Nemesis comes hunting with a taunt: "${(await p.evaluate(() => window.__says)).find((s) => /taste|waiting|scar|hunt|drag/i.test(s))}"`);
  await p.waitForTimeout(2500);
  await p.screenshot({ path: `${OUT}/nemesis-hunts.png` });
  check(!log.some((l) => l.includes('pageerror')), 'no page errors');
} finally {
  console.log(log.filter((l) => l.includes('pageerror')).join('\n') || 'no page errors');
  await browser.close();
}
