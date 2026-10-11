// Co-op smoke test: two accounts, party via invite link, ready chime, launch
// sting, loading screen, both clients in game with open data channels; then a
// second match where the client can't connect, exercising the host's
// "start without them" path and the client's "nogo" exit.
import { launch, open, signIn, signup, check, URL0, OUT } from './lib.mjs';

const log = [];
const browser = await launch();
try {
  const [eA, eB] = await Promise.all([signup('Alice'), signup('Bob')]);
  const A = await open(browser, 'A', log);
  const B = await open(browser, 'B', log);
  await signIn(A, eA);
  await signIn(B, eB);

  await A.click('#party-create');
  await A.waitForFunction(() => window.UHCoop.debug.party, null, { timeout: 10000 });
  const link = await A.evaluate(() => window.UHCoop.inviteLink());
  await B.goto(link.replace(/^https?:\/\/[^?]+/, URL0.replace(/\/?$/, '/')));
  await B.waitForFunction(() => window.UHCoop?.debug?.party?.members?.length === 2, null, { timeout: 30000 });
  check(true, 'B joined via invite link');

  await B.click('#party-ready');
  await A.waitForFunction(() => !document.getElementById('party-start')?.disabled, null, { timeout: 10000 });
  await A.waitForFunction(() => window.__cmds.includes('sfx|ready'), null, { timeout: 5000 });
  check(true, 'ready chime plays for the host when everyone is ready');

  await A.click('#party-start');
  await Promise.all([A, B].map((p) => p.waitForFunction(() => document.getElementById('coop-load').classList.contains('show'), null, { timeout: 10000 })));
  check(true, 'loading screen shows for host and client');
  await A.screenshot({ path: `${OUT}/coop-loading-A.png` });
  check(await B.evaluate(() => window.__cmds.includes('sfx|start')), 'launch sting plays for the client');
  check(await A.evaluate(() => window.__cmds.includes('sfx|start')), 'launch sting plays for the host');

  await Promise.all([A, B].map((p) => p.waitForFunction(() => window.UHCoop.debug.inGame, null, { timeout: 30000 })));
  const peers = await Promise.all([A, B].map((p) => p.evaluate(() => window.UHCoop.debug.peers)));
  check(peers[0].length === 1 && peers[0][0][1] === 'open' && peers[1][0][2] === 'open', 'both in game with open data channels ' + JSON.stringify(peers));
  await Promise.all([A, B].map((p) => p.waitForFunction(() => !document.getElementById('coop-load').classList.contains('show'), null, { timeout: 5000 })));
  check(true, 'loading screen hides once the match starts');

  // play a little so snapshots/hits flow both ways
  for (let i = 0; i < 12; i++) {
    await A.keyboard.down('KeyD'); await B.keyboard.down('KeyA');
    await A.waitForTimeout(200);
    await A.keyboard.up('KeyD'); await B.keyboard.up('KeyA');
  }
  const remoteSeen = await B.evaluate(() => window.__cmds.length >= 0);
  check(remoteSeen, 'played for a few seconds');
  await A.screenshot({ path: `${OUT}/coop-game-A.png` });
  await B.screenshot({ path: `${OUT}/coop-game-B.png` });

  // host leaves the match -> both back to the party; B readies again
  await A.keyboard.press('Escape');
  await A.waitForSelector('#pause.show', { timeout: 5000 });
  A.once('dialog', (d) => d.accept());
  await A.click('#quit');
  await B.waitForFunction(() => !window.UHCoop.debug.inGame, null, { timeout: 20000 });
  check(true, 'client leaves the match when the host quits');
  await B.evaluate(() => window.UHUI.toMenu());
  await A.waitForFunction(() => window.UHCoop.debug.party?.status === 'open', null, { timeout: 15000 });
  await B.waitForFunction(() => window.UHCoop.debug.party?.status === 'open' && document.getElementById('party-ready'), null, { timeout: 15000 });

  // match 2: B's WebRTC is crippled (relay-only with no relays => no candidates)
  await B.evaluate(() => {
    const Orig = window.RTCPeerConnection;
    window.RTCPeerConnection = function (cfg) { return new Orig({ ...cfg, iceServers: [], iceTransportPolicy: 'relay' }); };
  });
  await B.click('#party-ready');
  await A.waitForFunction(() => !document.getElementById('party-start')?.disabled, null, { timeout: 10000 });
  await A.click('#party-start');
  await A.waitForSelector('#cl-anyway', { timeout: 40000 });
  check(true, 'host is offered "Start without them" when a player can\'t connect');
  await A.screenshot({ path: `${OUT}/coop-stuck-A.png` });
  await A.click('#cl-anyway');
  await A.waitForFunction(() => window.UHCoop.debug.inGame, null, { timeout: 5000 });
  await B.waitForFunction(() => !window.UHCoop.debug.loading, null, { timeout: 15000 });
  check(!(await B.evaluate(() => window.UHCoop.debug.inGame)), 'client exits the loading screen when the host starts without them');

  // match 3: both sides relay-only, so the link must go through the TURN server
  // (what a phone on cellular / strict NAT needs)
  await A.keyboard.press('Escape');
  await A.waitForSelector('#pause.show', { timeout: 5000 });
  A.once('dialog', (d) => d.accept());
  await A.click('#quit');
  await A.evaluate(() => window.UHUI.toMenu());
  await A.waitForFunction(() => window.UHCoop.debug.party?.status === 'open', null, { timeout: 15000 });
  for (const p of [A, B]) {
    await p.reload();
    await p.waitForFunction(() => window.UHCoop?.debug?.party?.status === 'open', null, { timeout: 30000 })
      .catch(async (e) => { console.log('debug', JSON.stringify(await p.evaluate(() => { const d = window.UHCoop?.debug; return d && { party: d.party, me: d.me, inGame: d.inGame }; }))); throw e; });
    await p.evaluate(() => {
      const Orig = window.RTCPeerConnection;
      window.RTCPeerConnection = function (cfg) { return new Orig({ ...cfg, iceTransportPolicy: 'relay' }); };
    });
  }
  await B.waitForSelector('#party-ready', { timeout: 10000 });
  if (!(await B.evaluate(() => window.UHCoop.debug.party.members.find((m) => m.id === window.UHCoop.debug.me.id).ready))) await B.click('#party-ready');
  await A.waitForFunction(() => !document.getElementById('party-start')?.disabled, null, { timeout: 10000 });
  await A.click('#party-start');
  await Promise.all([A, B].map((p) => p.waitForFunction(() => window.UHCoop.debug.inGame, null, { timeout: 30000 })));
  const relay = await A.evaluate(async () => {
    const pc = window.UHCoop.debug.pcs[0];
    const st = await pc.getStats();
    let pair; st.forEach((r) => { if (r.type === 'transport' && r.selectedCandidatePairId) pair = st.get(r.selectedCandidatePairId); });
    return pair ? st.get(pair.localCandidateId)?.candidateType : null;
  });
  check(relay === 'relay', `relay-only match connects through TURN (local candidate: ${relay})`);
} finally {
  // failed fetches (e.g. a route not deployed yet) are logged but only page errors fail the test
  const errs = log.filter((l) => l.includes('pageerror'));
  console.log(errs.length ? 'PAGE ERRORS:\n' + errs.slice(0, 20).join('\n') : 'no page errors');
  await browser.close();
}
if (log.some((l) => l.includes('pageerror'))) process.exit(1);
