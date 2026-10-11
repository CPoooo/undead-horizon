// Invites from everywhere: three accounts become friends; Alice (no party yet)
// invites Bob from the lobby's invite picker, Carol from the friends list, and
// opens the picker from the in-game pause menu. Screenshots on desktop + phone.
import { launch, open, signIn, signup, check, OUT } from './lib.mjs';

const log = [];
const browser = await launch();
try {
  const [eA, eB, eC] = await Promise.all([signup('Alice'), signup('Bob'), signup('Carol')]);
  const A = await open(browser, 'A', log);
  const B = await open(browser, 'B', log);
  const C = await open(browser, 'C', log, { mobile: true });
  await signIn(A, eA);
  await signIn(B, eB);
  await signIn(C, eC);

  // friendships: A sends requests by code, B and C accept
  const me = (p) => p.evaluate(() => window.UHCoop.debug.me);
  const [mA, mB, mC] = await Promise.all([me(A), me(B), me(C)]);
  for (const [p, m] of [[B, mB], [C, mC]]) {
    await A.evaluate((code) => window.UHCoop.wsSend({ t: 'friends.add', code }), m.code);
    await p.waitForFunction((id) => window.UHCoop.social.incoming.some((f) => f.id === id), mA.id, { timeout: 15000 });
    await p.evaluate((id) => window.UHCoop.wsSend({ t: 'friends.accept', id }), mA.id);
  }
  await A.waitForFunction(() => window.UHCoop.social.friends.filter((f) => f.online).length === 2, null, { timeout: 15000 });
  check(true, 'Alice has two online friends');

  // 1) lobby: no party yet, "+ Invite friends" opens the picker
  check(await A.evaluate(() => !window.UHCoop.party), 'Alice starts without a party');
  await A.click('#party-invite');
  await A.waitForSelector('#invite-picker.show');
  await A.waitForTimeout(300); // fade-in
  await A.screenshot({ path: `${OUT}/invite-picker.png` });
  await A.click(`#invite-picker [data-invite="${mB.id}"]`);
  await A.waitForFunction(() => window.UHCoop.party, null, { timeout: 10000 });
  check(true, 'inviting without a party creates one');
  await B.waitForSelector('#invite-pop.show', { timeout: 15000 });
  check((await B.textContent('#invite-pop')).includes('invited you'), 'Bob gets the invite popup');
  await B.click('#inv-join');
  await A.waitForFunction(() => window.UHCoop.party?.members.length === 2, null, { timeout: 15000 });
  check(true, 'Bob joined from the popup');
  await A.waitForFunction(() => document.querySelector('#invite-picker .tag')?.textContent.includes('in your party'), null, { timeout: 5000 });
  check(true, 'picker marks Bob as in the party');
  await A.keyboard.press('Escape');
  check(!(await A.evaluate(() => document.getElementById('invite-picker').classList.contains('show'))), 'Esc closes the picker');

  // the lobby's open slots are invite buttons too
  check(await A.locator('#party-box [data-addinvite]').count() === 2, 'two open slots offer "Invite a friend"');

  // 2) friends list: invite Carol (she's on a phone)
  await A.click('#tabs button[data-tab="friends"]');
  await A.click(`#friends [data-invite="${mC.id}"]`);
  await A.waitForFunction((id) => !document.querySelector(`#friends [data-invite="${id}"]`), mC.id);
  check((await A.textContent('#friends')).includes('Invited'), 'friends list shows "Invited"');
  await C.waitForSelector('#invite-pop.show', { timeout: 15000 });
  await C.screenshot({ path: `${OUT}/invite-pop-phone.png` });
  await C.click('#inv-join');
  await A.waitForFunction(() => window.UHCoop.party?.members.length === 3, null, { timeout: 15000 });
  check(true, 'Carol joined from the friends-list invite');

  // 3) in game: the pause menu has "Invite friends" (B leaves so Alice can play solo)
  await B.evaluate(() => window.UHCoop.wsSend({ t: 'party.leave' }));
  await C.evaluate(() => window.UHCoop.wsSend({ t: 'party.leave' }));
  await A.waitForFunction(() => window.UHCoop.party?.members.length === 1, null, { timeout: 15000 });
  await A.click('#tabs button[data-tab="play"]').catch(() => {});
  await A.click('#play');
  await A.waitForTimeout(1500);
  await A.keyboard.press('Escape');
  await A.waitForSelector('#pause.show', { timeout: 5000 });
  check(await A.isVisible('#invite-btn'), 'pause menu shows Invite friends');
  await A.click('#invite-btn');
  await A.waitForSelector('#invite-picker.show');
  await A.waitForTimeout(300); // fade-in
  check((await A.textContent('#invite-picker')).includes('start co-op together'), 'solo-run picker explains what happens next');
  await A.screenshot({ path: `${OUT}/invite-pause.png` });
  await A.click(`#invite-picker [data-invite="${mB.id}"]`);
  await B.waitForSelector('#invite-pop.show', { timeout: 15000 });
  check(true, 'Bob gets an invite sent from the pause menu');
  await A.keyboard.press('Escape');
  check(await A.evaluate(() => document.getElementById('pause').classList.contains('show')), 'Esc closes the picker but keeps the game paused');

  // phone layout of the picker
  await C.evaluate(() => window.UHCoop.openInvites());
  await C.waitForSelector('#invite-picker.show');
  await C.waitForTimeout(300);
  await C.screenshot({ path: `${OUT}/invite-picker-phone.png` });

  const errs = log.filter((l) => l.includes('pageerror'));
  check(errs.length === 0, 'no page errors ' + errs.join(' | '));
  console.log('invite: all good');
} catch (e) {
  console.error(e.message);
  console.error(log.join('\n'));
  process.exitCode = 1;
} finally {
  await browser.close();
}
