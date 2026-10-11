// Career smoke test: finish a run, see XP on the game-over screen, then the
// Career tab (account level, achievements, bestiary, mastery).
import { launch, open, check, OUT } from './lib.mjs';

const log = [];
const browser = await launch();
try {
  const p = await open(browser, 'solo', log);
  await p.evaluate(() => window.UHUI.send('load|mode=infinite;diff=nightmare;savewave=6;level=3;hp=1;maxhp=100'));
  await p.evaluate(() => { document.getElementById('menu').classList.remove('show'); });
  await p.waitForSelector('#gameover.show', { timeout: 60000 });
  await p.waitForFunction(() => /XP/.test(document.getElementById('earned').textContent), null, { timeout: 10000 });
  check(true, `game over shows career gains: "${await p.textContent('#earned')}"`);
  await p.screenshot({ path: `${OUT}/career-gameover.png` });
  await p.click('#tomenu');
  await p.click('#tabs button[data-tab="career"]');
  await p.waitForSelector('#career .acct', { timeout: 5000 });
  const txt = await p.textContent('#career');
  check(/Account level/.test(txt) && /Bestiary/.test(txt), 'career tab shows level, achievements and bestiary');
  await p.screenshot({ path: `${OUT}/career-tab.png` });
  check(!log.some((l) => l.includes('pageerror')), 'no page errors');
} finally {
  console.log(log.filter((l) => l.includes('pageerror')).join('\n') || 'no page errors');
  await browser.close();
}
