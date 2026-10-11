// Checks GET /turn hands a signed-in player TURN relay servers.
import { launch, open, signIn, signup, check } from './lib.mjs';
const log = [];
const browser = await launch();
try {
  const page = await open(browser, 'T', log);
  await signIn(page, await signup('Turn'));
  const r = await page.evaluate(() => window.UHCloud.api('/turn'));
  const urls = r.iceServers.flatMap((s) => [].concat(s.urls));
  check(urls.some((u) => u.startsWith('turn:') || u.startsWith('turns:')), `TURN servers returned (${urls.length} urls)`);
  check(r.iceServers.filter((s) => s.username).every((s) => s.credential), 'relay entries carry credentials');
} finally { await browser.close(); }
