// Shared Playwright helpers for the end-to-end smoke tests.
// URL: first CLI arg, else UH_URL, else the local `dist/` server on :8765
// (an origin the API's CORS allowlist accepts). Screenshots go to UH_OUT.
import { chromium } from 'playwright';
import fs from 'node:fs';

export const AUTH = 'https://ep-crimson-bonus-b59agchg.neonauth.c-7.us-east-2.aws.neon.tech/neondb/auth';
export const URL0 = process.argv[2] || process.env.UH_URL || 'http://localhost:8765/';
export const OUT = process.env.UH_OUT || 'tests/e2e/out';
fs.mkdirSync(OUT, { recursive: true });

const PW = 'zombies-are-cool-123';
const ts = Date.now();

/** Creates a fresh test account and returns its email. */
export async function signup(name) {
  const email = `uh-test-${name.toLowerCase()}-${ts}@example.com`;
  const r = await fetch(AUTH + '/sign-up/email', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json', Origin: 'https://cpoooo.github.io' },
    body: JSON.stringify({ name, email, password: PW }),
  });
  if (!r.ok) throw new Error(`signup ${name}: ${r.status}`);
  return email;
}

// Use UH_CHROME, or an already-installed Chromium if Playwright's own isn't downloaded.
const fallbackChrome = `${process.env.HOME}/.cache/ms-playwright/chromium-1033/chrome-linux/chrome`;
const executablePath = process.env.UH_CHROME || (fs.existsSync(fallbackChrome) ? fallbackChrome : undefined);

export async function launch(extraArgs = []) {
  return chromium.launch({
    executablePath,
    // WSL without libnss3 installed: point UH_LIBS (default ~/.cache/uh-libs) at extracted libnss3/libnspr4 debs
    env: { ...process.env, LD_LIBRARY_PATH: [process.env.UH_LIBS || `${process.env.HOME}/.cache/uh-libs`, process.env.LD_LIBRARY_PATH].filter(Boolean).join(':') },
    args: ['--enable-unsafe-swiftshader', '--autoplay-policy=no-user-gesture-required', ...extraArgs],
  });
}

/** Opens the game in a new context. Collects page errors into `log`. */
export async function open(browser, tag, log, { viewport = { width: 1280, height: 720 }, mobile = false } = {}) {
  const ctx = await browser.newContext(mobile ? { viewport: { width: 390, height: 844 }, isMobile: true, hasTouch: true } : { viewport });
  const page = await ctx.newPage();
  page.on('pageerror', (e) => log.push(`${tag} pageerror: ${e.message}`));
  page.on('console', (m) => { if (m.type() === 'error') log.push(`${tag} console: ${m.text().slice(0, 200)}`); });
  // record every UI->game command so tests can assert on sounds etc.
  await page.addInitScript(() => {
    window.__cmds = [];
    const hook = () => {
      if (!window.UHUI || window.UHUI.__hooked) return;
      const orig = window.UHUI.send;
      window.UHUI.send = (c) => { window.__cmds.push(c); return orig(c); };
      window.UHUI.__hooked = true;
    };
    setInterval(hook, 50);
  });
  await page.goto(URL0);
  await page.waitForFunction(() => document.getElementById('loader')?.classList.contains('hide'), null, { timeout: 30000 });
  return page;
}

export async function signIn(page, email) {
  await page.evaluate(() => window.UHUI.toMenu && window.UHUI.toMenu());
  await page.click('#account-pill');
  await page.fill('#acct-email', email);
  await page.fill('#acct-pass', PW);
  await page.click('#acct-go');
  await page.waitForFunction(() => window.UHCoop?.debug?.me, null, { timeout: 20000 });
}

export function check(cond, msg) {
  if (!cond) throw new Error('FAIL: ' + msg);
  console.log('  ok -', msg);
}

/** A creates a party, B joins via the invite link. */
export async function makeParty(A, B) {
  await A.click('#party-create');
  await A.waitForFunction(() => window.UHCoop.debug.party, null, { timeout: 10000 });
  const link = await A.evaluate(() => window.UHCoop.inviteLink());
  await B.goto(link.replace(/^https?:\/\/[^?]+/, URL0.replace(/\/?$/, '/')));
  await B.waitForFunction(() => window.UHCoop?.debug?.party?.members?.length === 2, null, { timeout: 30000 });
}
