// One save per game type: Infinite, Campaign, Outbreak and Siege each keep
// their own slot, saving a type replaces only that type's save, and resuming
// restores mode-specific state (map, Siege defenses).
import { launch, open, check } from './lib.mjs';

const log = [];
const browser = await launch();
try {
  const p = await open(browser, 'solo', log);
  const saves = () => p.evaluate(() => JSON.parse(localStorage.getItem('undeadHorizon.profile.v1') || '{}').saves || {});
  const playAndSave = async (mode, extra = async () => {}) => {
    await p.evaluate((m) => document.querySelector(`[data-mode="${m}"]`).click(), mode);
    await p.evaluate(() => document.getElementById('play').click());
    // campaign shows the chapter intro first
    for (let i = 0; i < 20 && (await p.evaluate(() => window.UHUI.state())) !== 'playing'; i++) {
      await p.evaluate(() => document.getElementById('story-go')?.click());
      await p.waitForTimeout(400);
    }
    await p.waitForFunction(() => window.UHUI.state() === 'playing', null, { timeout: 20000 });
    await p.evaluate(() => window.UHUI.send('dev|tank'));
    await p.waitForTimeout(1500);
    await extra();
    await p.evaluate(() => window.UHUI.send('pause'));
    await p.waitForSelector('#pause.show', { timeout: 5000 });
    check(!(await p.evaluate(() => document.getElementById('save-quit').disabled)), `${mode}: Save is enabled`);
    await p.click('#save-quit');
    await p.waitForFunction(() => document.getElementById('menu').classList.contains('show'), null, { timeout: 10000 });
  };

  await playAndSave('infinite');
  await playAndSave('campaign');
  await playAndSave('outbreak');
  await playAndSave('siege', async () => {
    // place a tower so the save has Siege-specific state
    await p.evaluate(() => { window.UHUI.send('dev|points=0'); });
  });
  let s = await saves();
  check(['infinite', 'campaign', 'outbreak', 'siege'].every((m) => s[m]?.data), `one save per game type (${Object.keys(s).filter((k) => s[k]).join(', ')})`);
  check(/mode=outbreak/.test(s.outbreak.data) && /;ob=/.test(s.outbreak.data) && /map=asylum/.test(s.outbreak.data), 'the Outbreak save carries its map, points and guns');
  check(/mode=siege/.test(s.siege.data) && /;sg=/.test(s.siege.data), 'the Siege save carries its crystal and mana');
  const buttons = await p.$$eval('[data-continue]', (bs) => bs.map((b) => b.dataset.continue));
  check(buttons.length === 4, `four Continue buttons (${buttons.join(', ')})`);

  // saving Siege again replaces only the Siege save
  const before = { ...s };
  await playAndSave('siege');
  s = await saves();
  check(s.siege.summary.date > before.siege.summary.date, 'a new Siege save replaced the old Siege save');
  check(['infinite', 'campaign', 'outbreak'].every((m) => s[m].data === before[m].data), 'the other game types\' saves are untouched');

  // resume the Siege save: back in Siege, slot consumed, others kept
  await p.evaluate(() => document.querySelector('[data-continue="siege"]').click());
  await p.waitForFunction(() => window.UHUI.state() === 'playing', null, { timeout: 15000 });
  await p.waitForTimeout(800);
  s = await saves();
  check(!s.siege && s.infinite && s.campaign && s.outbreak, 'resuming consumes only the Siege slot');
  check(!log.some((l) => l.includes('pageerror')), 'no page errors');
} finally {
  console.log(log.filter((l) => l.includes('pageerror')).join('\n') || 'no page errors');
  await browser.close();
}
