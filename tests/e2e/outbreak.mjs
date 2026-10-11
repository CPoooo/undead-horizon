// Outbreak smoke test: start a match on Hollowbrook Asylum, survive a bit,
// spin the mystery box and buy a door, with no page errors.
import { launch, open, check, OUT } from './lib.mjs';

const log = [];
const browser = await launch();
try {
  const p = await open(browser, 'solo', log);
  const send = (c) => p.evaluate((c) => window.UHUI.send(c), c);
  await send('start|mode=outbreak;diff=rookie');
  await p.waitForTimeout(6000);
  await p.screenshot({ path: `${OUT}/outbreak-round1.png` });
  // walk up to the box with points and press E
  await send('dev|points=5000');
  await send('dev|tp=-300,-60');
  await p.waitForTimeout(300);
  await p.keyboard.press('KeyE');
  await p.waitForTimeout(3500);
  await p.screenshot({ path: `${OUT}/outbreak-box.png` });
  await p.keyboard.press('KeyE'); // take the gun
  // buy the lobby door
  await send('dev|tp=0,-150');
  await p.waitForTimeout(300);
  await p.keyboard.press('KeyE');
  await p.waitForTimeout(800);
  await p.screenshot({ path: `${OUT}/outbreak-door.png` });
  check(log.length === 0, 'no page errors during an Outbreak match');
} finally {
  console.log(log.length ? 'PAGE ERRORS:\n' + log.slice(0, 20).join('\n') : 'no page errors');
  await browser.close();
}
