// Photo mode: pause a run, open the free camera, pan/zoom, pick a filter,
// capture a PNG (download on desktop) and get back to the pause menu.
import { launch, open, check, OUT } from './lib.mjs';

const log = [];
const browser = await launch();
try {
  const p = await open(browser, 'solo', log);
  await p.click('#play');
  await p.waitForFunction(() => window.UHUI.state() === 'playing', null, { timeout: 15000 });
  await p.waitForTimeout(2500);
  await p.keyboard.press('Escape');
  await p.waitForSelector('#pause.show', { timeout: 5000 });
  await p.click('#photo-btn');
  await p.waitForSelector('#photo.show', { timeout: 3000 });
  check(!(await p.evaluate(() => document.getElementById('pause').classList.contains('show'))), 'photo mode hides the pause menu');
  await p.keyboard.down('KeyD');
  await p.waitForTimeout(400);
  await p.keyboard.up('KeyD');
  await p.mouse.move(640, 300);
  await p.mouse.wheel(0, -400);
  await p.click('[data-phf="5"]');
  check(/hue-rotate/.test(await p.evaluate(() => document.getElementById('game').style.filter)), 'filter applies to the view');
  await p.click('#ph-bars');
  await p.waitForTimeout(300);
  await p.screenshot({ path: `${OUT}/photo-mode.png` });
  const [dl] = await Promise.all([p.waitForEvent('download', { timeout: 10000 }), p.click('#ph-shot')]);
  const file = `${OUT}/photo-capture.png`;
  await dl.saveAs(file);
  check(/undead-horizon-.*\.png$/.test(dl.suggestedFilename()), `capture downloads a PNG (${dl.suggestedFilename()})`);
  // the filter is baked into the file: Noir => every pixel is gray
  await p.click('[data-phf="1"]');
  const [dl2] = await Promise.all([p.waitForEvent('download', { timeout: 10000 }), p.click('#ph-shot')]);
  const b64 = (await import('node:fs')).readFileSync(await dl2.path()).toString('base64');
  const gray = await p.evaluate(async (src) => {
    const img = new Image();
    img.src = 'data:image/png;base64,' + src;
    await img.decode();
    const c = document.createElement('canvas');
    c.width = img.width; c.height = img.height;
    const x = c.getContext('2d');
    x.drawImage(img, 0, 0);
    const d = x.getImageData(0, 0, c.width, c.height).data;
    let off = 0;
    for (let i = 0; i < d.length; i += 4 * 97) off = Math.max(off, Math.abs(d[i] - d[i + 1]), Math.abs(d[i + 1] - d[i + 2]));
    return off;
  }, b64);
  check(gray <= 3, `the capture has the filter baked in (max channel spread ${gray})`);
  await p.keyboard.press('Escape');
  await p.waitForSelector('#pause.show', { timeout: 3000 });
  check(!(await p.isVisible('#photo')) && (await p.evaluate(() => document.getElementById('game').style.filter)) === '', 'Esc returns to the pause menu with the filter cleared');
  check(await p.evaluate(() => window.UHUI.state() === 'paused'), 'the run is still paused');
  check(!log.some((l) => l.includes('pageerror')), 'no page errors');
} finally {
  console.log(log.filter((l) => l.includes('pageerror')).join('\n') || 'no page errors');
  await browser.close();
}
