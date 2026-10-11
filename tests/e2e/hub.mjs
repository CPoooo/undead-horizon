// Home Base smoke test: enter from the lobby, open a vendor stall, browse,
// chat with the vendor (canned or AI line), walk away, visit the range.
import { launch, open, check, OUT } from './lib.mjs';

const log = [];
const browser = await launch();
try {
  const p = await open(browser, 'solo', log);
  const send = (c) => p.evaluate((c) => window.UHUI.send(c), c);
  await p.click('#hub-btn');
  await p.waitForTimeout(1500);
  await send('dev|tp=640,-250');
  await p.waitForTimeout(400);
  await p.keyboard.press('KeyE');
  await p.waitForSelector('#vendor.show', { timeout: 5000 });
  check(await p.textContent('#v-name') === 'Madame Lucienne', 'the tailor opens her stall');
  await p.waitForFunction(() => document.getElementById('v-line').textContent.length > 3, null, { timeout: 15000 });
  const greet = await p.textContent('#v-line');
  check(greet.length > 3, `she greets the courier: "${greet}"`);
  await p.fill('#v-say', 'What should I wear to fight zombies?');
  await p.press('#v-say', 'Enter');
  await p.waitForFunction((g) => document.getElementById('v-line').textContent !== g, greet, { timeout: 15000 });
  check(true, `she answers: "${await p.textContent('#v-line')}"`);
  await p.screenshot({ path: `${OUT}/hub-stall.png` });
  await send('dev|tp=0,300');
  await p.waitForFunction(() => !document.getElementById('vendor').classList.contains('show'), null, { timeout: 5000 });
  check(true, 'walking away closes the stall');
  await send('dev|tp=-700,500');
  await p.waitForTimeout(2500);
  await p.screenshot({ path: `${OUT}/hub-range.png` });
  // the Outskirts: ambient zombies, a Supply Drop held for 30s pays out
  await send('dev|tp=1150,0');
  await p.waitForTimeout(3000);
  await p.screenshot({ path: `${OUT}/hub-outskirts.png` });
  await send('dev|tank');
  await send('dev|tp=1600,100');
  await send('dev|drop');
  await p.waitForTimeout(4000);
  await p.screenshot({ path: `${OUT}/hub-supply-drop.png` });
  // stand on the beacon like a player would (knockback shoves an idle player out)
  let secured = false;
  for (let i = 0; i < 25 && !secured; i++) {
    await send('dev|tp=1640,100');
    await p.waitForTimeout(1500);
    secured = await p.evaluate(() => /Supplies secured/.test(document.getElementById('toast').textContent));
  }
  if (!secured) await p.waitForFunction(() => /Supplies secured/.test(document.getElementById('toast').textContent), null, { timeout: 5000 });
  check(true, 'holding a Supply Drop secures the supplies and pays out');
  check(!log.some((l) => l.includes('pageerror')), 'no page errors in the Home Base');
} finally {
  console.log(log.length ? 'PAGE ERRORS:\n' + log.slice(0, 20).join('\n') : 'no page errors');
  await browser.close();
}
