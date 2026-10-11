// Party text chat (filter, in-game T chat, block/unblock) and voice chat
// (mesh link, audio flowing from the fake mic, mute).
import { launch, open, signIn, signup, check, makeParty, OUT } from './lib.mjs';

const log = [];
const browser = await launch(['--use-fake-ui-for-media-stream', '--use-fake-device-for-media-stream']);
try {
  const [eA, eB] = await Promise.all([signup('Ana'), signup('Ben')]);
  const A = await open(browser, 'A', log);
  const B = await open(browser, 'B', log);
  await signIn(A, eA);
  await signIn(B, eB);
  await makeParty(A, B);
  for (const p of [A, B]) await p.waitForSelector('#party-chat:not([style*="none"])', { timeout: 10000 });
  check(true, 'party chat panel shows once in a party');

  await A.fill('#pc-input', 'hello squad, what the fuck is up');
  await A.press('#pc-input', 'Enter');
  await B.waitForFunction(() => window.UHChat.messages.some((m) => m.text?.startsWith('hello squad')), null, { timeout: 10000 });
  const got = await B.evaluate(() => window.UHChat.messages.find((m) => m.text?.startsWith('hello squad')).text);
  check(got === 'hello squad, what the f*** is up', `lobby message delivered and filtered: "${got}"`);
  check(await A.evaluate(() => window.UHUI.state() === 'menu'), 'Enter in the chat box does not start a run');
  check(await B.evaluate(() => window.__cmds.includes('sfx|chat')), 'incoming message plays a blip');

  // voice
  await A.click('#v-join');
  await B.click('#v-join');
  await A.waitForFunction(() => window.UHVoice.debug.peers.some((p) => p.state === 'connected' && p.audio), null, { timeout: 30000 });
  await B.waitForFunction(() => window.UHVoice.debug.peers.some((p) => p.state === 'connected' && p.audio), null, { timeout: 30000 });
  check(true, 'voice link connected both ways');
  await B.waitForFunction(() => window.UHVoice.debug.peers[0].level > 0.01, null, { timeout: 10000 });
  check(true, 'B hears audio from A\'s mic');
  await A.waitForFunction(() => window.UHCoop.debug.party.members.every((m) => m.voice), null, { timeout: 10000 });
  check(true, 'party shows both players in voice');
  await A.click('#v-mute');
  check(await A.evaluate(() => !window.UHVoice.debug.live), 'mute disables the mic');
  await A.click('#v-mute');
  await A.screenshot({ path: `${OUT}/chat-voice-lobby-A.png` });

  // in-game chat
  await B.click('#party-ready');
  await A.waitForFunction(() => !document.getElementById('party-start')?.disabled, null, { timeout: 10000 });
  await A.click('#party-start');
  await Promise.all([A, B].map((p) => p.waitForFunction(() => window.UHCoop.debug.inGame && !window.UHCoop.debug.loading, null, { timeout: 30000 })));
  await B.keyboard.press('KeyT');
  await B.waitForFunction(() => document.activeElement?.id === 'gc-input', null, { timeout: 3000 });
  await B.keyboard.type('behind you!');
  await B.keyboard.press('Enter');
  await A.waitForFunction(() => window.UHChat.messages.some((m) => m.text === 'behind you!'), null, { timeout: 10000 });
  await A.waitForFunction(() => document.getElementById('gc-feed').textContent.includes('behind you!'), null, { timeout: 3000 });
  check(true, 'in-game T chat reaches the other player\'s feed');
  check(await A.evaluate(() => window.UHVoice.debug.peers[0]?.state === 'connected'), 'voice stays connected during the match');
  await A.screenshot({ path: `${OUT}/chat-voice-game-A.png` });

  // block / unblock
  const aId = await A.evaluate(() => window.UHCoop.me.id);
  await B.evaluate((id) => window.UHCoop.wsSend({ t: 'block', id }), aId);
  await B.waitForFunction(() => window.UHCoop.social.blocked?.length === 1, null, { timeout: 10000 });
  await B.waitForFunction(() => window.UHVoice.debug.peers.length === 0, null, { timeout: 10000 });
  check(true, 'blocking drops the voice link');
  await A.evaluate(() => window.UHChat.send('you cannot see this'));
  await A.waitForTimeout(2500);
  check(!(await B.evaluate(() => window.UHChat.messages.some((m) => m.text === 'you cannot see this'))), 'blocked player\'s messages are not delivered');
  await B.evaluate((id) => window.UHCoop.wsSend({ t: 'unblock', id }), aId);
  await B.waitForFunction(() => !window.UHCoop.social.blocked?.length, null, { timeout: 10000 });
  await A.evaluate(() => window.UHChat.send('back again'));
  await B.waitForFunction(() => window.UHChat.messages.some((m) => m.text === 'back again'), null, { timeout: 10000 });
  check(true, 'unblocking restores messages');
} finally {
  // failed fetches (e.g. a route not deployed yet) are logged but only page errors fail the test
  const errs = log.filter((l) => l.includes('pageerror'));
  console.log(errs.length ? 'PAGE ERRORS:\n' + errs.slice(0, 20).join('\n') : 'no page errors');
  await browser.close();
}
if (log.some((l) => l.includes('pageerror'))) process.exit(1);
