# Undead Horizon — project notes for Claude

Infinite 2.5D top-down zombie survival in Rust + WebAssembly (Canvas2D, procedural WebAudio), with online co-op, three game loops (Infinite/Campaign horde, Outbreak, Siege), a walkable social Home Base with AI vendors, and a Neon backend.
Live at https://cpoooo.github.io/undead-horizon/ (deployed by `.github/workflows/pages.yml` on every push to `main`).

_Last full status update: 2026-10-11 (latest commit at that time: `e1beb25`)._

---

## 0. Resume here (handoff for the next session)

_Written 2026-10-11 at commit `83f1dea` (plus this doc update). Read this section first, then §1 (status) and §2 (owner to-do)._

### How we work (owner's standing instructions)
- **Autonomy:**
  - Build roadmap items and new "AAA" features without asking.
  - Commit **one feature per commit** when the tests pass, then **push to `main`** right away. GitHub Pages deploys it and the owner tests on their phone.
  - Be creative: research features from big games (Destiny, L4D, Shadow of Mordor, CoD Zombies, Apex…) and adapt them.
- **The owner runs deploys:**
  - Claude must not run `npm run db:schema` / `npm run deploy:api` (production deploy) or read secrets (`.env.local`, TURN/Metered credential code).
  - Instead, append server changes to `server/schema.sql`/`server/*.ts` and list them in §2 item 1.
  - Read-only SQL against the Neon project (`dawn-base-35494450`) is fine for checking things; destructive SQL needs the owner's OK.
- **Verify before reporting:**
  - Build, run native tests, and run the relevant e2e.
  - For UI work, take Playwright screenshots (desktop and phone: portrait 390×844, landscape 844×390, small 360×640) and look at them.
  - Report honestly what is live vs waiting on deploy.

### What's been completed (full history: `git log`, 88 commits)
1. **Foundations and v1:**
   - core game, accounts, shop, campaign, 2-player co-op, social lobby, save slots, mobile layout
   - fixes: site not loading after deploys, lobby not connecting, autofill crash
2. **Roadmap v2, all milestones** (`F1`… through `V6`):
   - M0 foundations, M1 feel, M2 combat depth, M3 Outbreak, M4 Siege, M5 progression, M6 live features, M8 world, M9 platform/accessibility
   - each item is its own commit named after its id (e.g. `C7: Boss 2.0 …`)
3. **Home Base:**
   - walkable hub with AI vendors (Claude), firing range, mission terminal
   - later: the Outskirts free roam + Supply Drops, emotes, proximity voice, 16-player instances with strangers
4. **M7 multiplayer hardening:**
   - 4 players, pings/emotes, squad scoreboard
   - ICE restart + reconnect grace, rejoin / join in progress, host migration
   - public parties + Quick Play, campaign co-op
5. **Beyond v2:** AI Director, Photo mode, Nemesis system, Hollow Night limited-time event, death recap.
6. **Quality and ops:**
   - leaderboards hide e2e test accounts
   - phone HUD redesign (health stack, thumb cluster, a landscape crash fixed)
   - **one save per game type** (Infinite / Campaign / Outbreak / Siege)
   - this CLAUDE.md: status, code map, protocols, application state, scaling plan

### Next up (in priority order)
1. **Owner:**
   - Deploy the server (§2 item 1); a lot of signed-in functionality waits on it.
   - Then re-run `npm run e2e` against the live API and fix anything that breaks. Expect follow-ups in the routes that have never run in production: `/vendor`, `/hub/reward`, `quickplay`, `hub.pos` instances, Outbreak/Siege save slots.
2. **Owner:** add `ANTHROPIC_API_KEY` to `.env.local` (AI vendors), and fix the TURN credential cap if friends can't connect (§2 item 3).
3. **Owner:** a real-device iPhone pass (§2 item 4). Fix whatever it finds, especially phone HUD spacing with the notch and home bar, and the share sheets.
4. **Claude (small, known gaps):**
   - a Settings toggle for touch controls (Auto / On / Off)
   - hub mates keyed by user id instead of display name
   - Nemesis attribution when killed from far away
5. **Claude (backlog, §7.5):**
   - kill cam / "Play of the Run"
   - weapon attachments
   - more limited-time events
   - hub loot caches and more public event types
   - inviting a stranger by walking up to them in the hub
6. **When players grow:** start with Tier 1 of the scaling plan (§10).

### Quick start for a new session
```bash
cd ~/undead-horizon && git pull && git log --oneline -5
cargo build --target wasm32-unknown-unknown --release 2>&1 | grep -E "^(warning|error)"   # must print nothing
cargo test --release                                     # 56 tests
for f in ui/*.js; do node --check "$f"; done
trunk build --release                                    # writes dist/
(cd dist && python3 -m http.server 8765 &)               # serve for e2e
node tests/e2e/<suite>.mjs                               # or: npm run e2e (all 13 suites)
```
- The handy all-in-one check, as a shell function: wasm build (no warnings) → `cargo test --release` → `node --check ui/*.js` → `trunk build --release`. **If tests fail it stops before rebuilding `dist/`, so e2e would run against a stale build.**
- **Phone layout checks:** `tests/e2e/mobile-hud.mjs` (three phone sizes, screenshots in `tests/e2e/out/mobile-hud-*.png`).
- **Dev commands** for staging scenes are in §3 (`dev|tank`, `dev|boss=N`, `dev|horde`, `dev|drop`, `dev|tp=x,y`, …). Start modes through the real UI (click `[data-mode="siege"]` then `#play`) so body classes and touch UI match real play.

### Gotchas learned the hard way
- **`f32::clamp(min, max)` panics if min > max:** in wasm it shows as `unreachable`. Screen-relative bounds must use `.max(lo).min(hi.max(lo))`. Short landscape phones hit this.
- **Duplicate format strings in `game.rs`:** `stats_json` and `save_string` both contain `self.mode.id(), self.chapter(),`. Scripted edits must target the right one (anchor on the function first).
- **`neon_auth."user".id` is a uuid;** compare with `id::text` against `user_id` columns.
- **Tests share the world map:** `world::map()` is global per thread; native tests that load maps run on separate threads, so it's fine, but don't assume one map across tests.
- **Overlay visibility in Playwright:** overlays hide with opacity, so `isVisible` stays true. Check the `.show` class instead.
- **Headless Chromium has no emoji font** (boxes in screenshots). That's cosmetic, not a bug.
- **E2E accounts hit the live social server and auth** (`uh-test-*@example.com`). Party joins occasionally time out, so rerun once before debugging. Suites fail only on page errors (404s from undeployed routes are tolerated).
- **The level-up screen freezes the sim:** don't let the Home Base drop XP (it doesn't: `spawn_gem` is a no-op in the hub). Tests that farm kills must pick cards (`Digit1`).
- **The UI → game command queue runs before net messages each frame,** so a `coop|…late=1` start lands before the late-join sync.
- **CLUSTER sizes:** `CLUSTER_W`/`CLUSTER_H` in `src/hud_layout.rs` must match the `#touch-ui` box in `ui/style.css`.
- **Trunk is pinned to v0.21.14 in CI, and UI scripts are `data-no-minify`:** a newer trunk's JS minifier turned `get cloud()` into a plain method, so `Data.cloud` was always truthy and the account panel broke for signed-out players (it showed only "Your account"). Don't unpin it or drop the attribute without checking that `dist/app-*.js` still contains `get cloud(`.
- **Bump `PROTOCOL` (net) / `SAVE_VERSION` (saves)** whenever their formats change. They are 14 and 3 now.

## 1. Status at a glance

Legend:
- ✅ **Live**: works on the site now.
- 🟡 **Needs deploy**: the client is live, but the server part isn't deployed yet (`npm run db:schema && npm run deploy:api`). Guests (signed out) usually get the local fallback.
- 🔑 **Needs config**: needs a secret in `.env.local` before deploying.
- 📱 **Needs device test**: built and tested in desktop/mobile emulation, but not yet on a real iPhone.

| Area | Feature | State |
|---|---|---|
| Core | Infinite mode, Campaign (5 chapters), 4 difficulties, loadouts, shop, locker | ✅ |
| Core | Save & resume: **one save per game type** (Infinite, Campaign, Outbreak, Siege); a new save of a type replaces only that type's save | ✅ for guests and for signed-in Infinite/Campaign; 🟡 signed-in Outbreak/Siege slots (server deploy) |
| Core | Combat depth: evolutions, chests, elements, elite affixes, 5 new archetypes, Boss 2.0, characters/abilities, projectile pressure | ✅ |
| Core | Outbreak mode (2 maps, points, doors, box, perks, power, upgrade machine, power-ups, hounds, traps, buildables, easter egg) | ✅ (its leaderboard 🟡) |
| Core | Siege mode (3 maps, 6 defenses ×3 tiers, mana, stars, Glass variant) | ✅ |
| Core | AI Director (stress-paced spawns, horde rushes, mercy hearts) | ✅ |
| Core | Nemesis system | ✅ (synced across devices once settings sync 🟡 is deployed) |
| Core | Death recap ("Killed by …", last 8s) | ✅ |
| Core | Photo mode | ✅ (phone share sheet 📱) |
| Platform | Phone HUD layout: player, crystal and boss health readable at once; thumb cluster clear of the Siege build bar | ✅ (📱 real-device check) |
| Feel | Weapon feel, hit reactions, corpses, dynamic light, audio 2.0, camera, haptics, callouts | ✅ |
| World | Day/night + weather, biomes, destructibles, character polish + cosmetic slots, lobby music picker, cutscenes | ✅ |
| Platform | Gamepad + remap, accessibility suite, PWA/offline, perf/LOD, tutorial, i18n (en/es) | ✅ |
| Platform | Settings sync, balance telemetry, error reporting (`/me/settings`, run columns, `/errors`) | 🟡 |
| Platform | Safari / iOS | 📱 (layouts, touch, PWA done; needs a real-device pass) |
| Accounts | Sign-in (email + Google), cloud profile, global leaderboard | ✅ |
| Accounts | Leaderboard filters (mode/map/players/friends/week/pure), daily board, outbreak board, hiding e2e test accounts | 🟡 |
| Progression | Account level, achievements (53), meta upgrades, relics, weapon mastery, bestiary, Career tab | ✅ for guests; 🟡 for signed-in (server routes) |
| Live | Daily/weekly challenges, daily seeded run, Season 1 pass, mutators, welcome-back bonus | ✅ for guests; 🟡 for signed-in |
| Live | **Hollow Night** event (Oct 1 – Nov 2): pumpkin zombies, candy, free Jack-o'-Lantern hat | ✅ for guests; 🟡 candy for signed-in |
| Co-op | 2-player co-op (Infinite), lobby chat, voice chat, block/report | ✅ |
| Co-op | Pings/emotes, squad scoreboard + MVP, ICE restart/reconnect grace, host migration | ✅ |
| Co-op | TURN relay | ✅ (Metered free plan caps live credentials at 10; see §2) |
| Co-op | 4-player parties, co-op Outbreak/Siege/Campaign mode picker | 🟡 (game supports 4 and all modes; server still caps parties at 3 and accepts only the old mode set) |
| Co-op | Rejoin a running match after a drop | ✅ |
| Co-op | Invite friends from anywhere: lobby picker + open party slots, friends list (auto-creates a party), pause menu in game | ✅ (inviting from inside a running co-op match 🟡) |
| Co-op | A new friend joining a match in progress | 🟡 (server must allow `party.join` into `in_game` parties) |
| Co-op | Public parties, squad browser, Quick Play | 🟡 |
| Home Base | Walkable hub, vendor stalls, firing range + DPS meter, Hall of Survivors, mission terminal | ✅ |
| Home Base | AI vendor dialogue (Claude) | 🟡🔑 (canned lines until `/vendor` is deployed with `ANTHROPIC_API_KEY`) |
| Home Base | The Outskirts free-roam zone, Supply Drop events | ✅ (coin reward for signed-in 🟡 `/hub/reward`; guests get local coins) |
| Home Base | Party members walking the hub, emotes, proximity voice, instances with strangers | 🟡 |

---

## 2. Owner to-do (things only you can do)

1. **Deploy the server**: `npm run db:schema && npm run deploy:api`. This one step turns every 🟡 above live. It covers, in order of when it was built:
   - `/errors`
   - character + cosmetic categories
   - Outbreak/Siege run modes, score and map
   - leaderboard filters + the Outbreak board
   - `party.mode`
   - `hub.pos` relay
   - progression routes (`/meta/buy`, `/relics/equip`, `/achievement`, `/welcome`)
   - daily/mutated/pure/cause/build columns + the balance views
   - `/me/settings`
   - `MAX_PARTY` 4
   - campaign party mode + `parties.chapter`
   - joining `in_game` parties
   - public parties + Quick Play
   - `/hub/reward`
   - hub emotes + `hub_presence` instances
   - event candy in `/runs`
   - the leaderboard test-account filter
   - Outbreak + Siege save slots (`saves_slot_check` widened; `PUT /save` accepts the four types)
   - `party.invite` into `in_game` parties (inviting from inside a co-op match; the `invite` notice carries `inGame`)
2. **AI vendors**: add `ANTHROPIC_API_KEY` to `.env.local` before deploying (`neon.ts` already passes it). Without it, vendors use canned lines.
3. **TURN**: Metered's free plan allows 10 live credentials (4h expiry), and the server mints one per isolate when `METERED_API_KEY` is unset. If friends can't connect and `/turn` logs "Maximum credential limit reached", either set `METERED_API_KEY` or make `/turn` reuse an existing credential (`GET https://<app>.metered.live/api/v2/turn/credentials?secretKey=`).
4. **Real iPhone pass** (Safari + home-screen PWA):
   - audio unlock
   - sign-in and session restore (ITP)
   - co-op and voice
   - invite share sheet
   - Photo mode share sheet
   - touch controls
5. **Optional cleanup**: 114 of the 122 runs in `runs` come from e2e test accounts (`uh-test-*@example.com`). Once deployed they're hidden from leaderboards. To delete them, run:
   ```sql
   delete from runs where user_id in (select id::text from neon_auth."user" where email like 'uh-test-%@example.com');
   ```
   This has not been run.

---

## 3. Build, run, test

```bash
trunk serve --release                              # local dev at http://127.0.0.1:8080
trunk build --release                              # production bundle in dist/
cargo build --target wasm32-unknown-unknown        # fast type-check (must have no warnings)
cargo test --release                               # native sim tests (55 as of 2026-10-11)
for f in ui/*.js; do node --check "$f"; done       # JS syntax check
npm run build:cloud                                # after editing ui/cloud-src.js (CI also does it)
```

**End-to-end (Playwright, real Chromium):**
- First serve `dist/` on :8765 with `cd dist && python3 -m http.server 8765`, then run `npm run e2e`.
- Suites, in order: `turn`, `coop`, `chat-voice`, `outbreak`, `hub`, `career`, `photo`, `nemesis`, `event`, `mobile-hud`, `saves`, `invite`. Shared helpers are in `tests/e2e/lib.mjs`. Screenshots go to `tests/e2e/out/`.
- `coop`, `chat-voice` and `turn` sign up fresh accounts (`uh-test-<name>-<ts>@example.com`) against the live Neon Auth and social server.
- Suites fail only on page errors. Failed fetches (e.g. routes not deployed yet) are logged but tolerated.
- WSL without system libnss3: extract the `libnss3`/`libnspr4` debs into `~/.cache/uh-libs` (or set `UH_LIBS`). `UH_CHROME` overrides the Chromium path.
- Occasional flake: a party join in `coop`/`chat-voice` can time out on a slow social-server round trip. Rerun before debugging.

**Dev commands** (send from the console with `window.UHUI.send('dev|…')`):
- `chest`, `levelup` (fills the XP bar), `elites`, `kinds`, `horde` (800 zombies)
- `boss=N`, `tp=x,y`, `points=N`, `biome=snow|town|forest|graveyard`
- `sky=rain|fog|clear,clock`
- `tank` (10M HP)
- `drop` (Supply Drop next to you, hub only)

---

## 4. Code map

### Rust (`src/`)
| File | What |
|---|---|
| `main.rs` | Browser bootstrap, input listeners, rAF loop, the JS bridge (`uhPoll` JS→Rust command queue, `uhEvent` Rust→JS notifications, `uhNetSend`/`uhNetPoll` for co-op bytes) and the command dispatch (incl. `dev|`). |
| `game.rs` | The core sim (~4.9k lines; `grep -n` anchors, never read it whole). Fixed 120 Hz step (`DT`) with render interpolation (`prev`/`pos`); phases Menu/Playing/LevelUp/Paused/Dying/GameOver; player, zombies, weapons, powers, pickups, spatial hash `Grid`, `apply_opts`, `stats_json`, saves (`SAVE_VERSION` 3). |
| `modes/` | `Mode` enum + `ModeHooks` (`on_start`, `update_waves`, `on_kill`, `hud`, `is_over`). `infinite.rs`, `campaign.rs`, `horde.rs` (shared wave spawner, boss waves, chapter victory), `outbreak.rs`, `siege.rs`, `hub.rs` (Home Base + Outskirts). |
| `content.rs` | Data tables: `ENEMIES` (15 kinds), `GUNS`, `DIFFICULTIES`, `CHAPTERS`, bosses, skins/hats/tracers/packs/killfx/deathfx/emotes, mutators. Ids are shared with the UI catalog. |
| `coop.rs` / `net.rs` | Co-op: authority model, wire format (`PROTOCOL` 14), snapshots, replicas, hit reporting, kill credit, shared XP, revive, events, late join sync, checkpoints, host migration. |
| `director.rs` | AI Director pacing. |
| `save_modes.rs` | Outbreak/Siege save + restore (one save per game type). |
| `hud_layout.rs` | Where HUD pieces go (desktop vs compact phone layout). |
| `nemesis.rs` | Nemesis system. |
| `photo.rs` | Photo mode camera + overlays. |
| `upgrades.rs`, `chest.rs`, `status.rs`, `elites.rs`, `boss.rs`, `abilities.rs` | Level-up powers + evolutions, chests, elements, affixes, Boss 2.0, character abilities. |
| `outbreak_interact.rs`, `secrets.rs`, `nav.rs` | Outbreak interactables (doors/box/perks/power/traps), buildables + easter egg, flow-field navigation. |
| `world.rs` | Procedural infinite world (hash per cell, biomes) plus authored JSON maps (`Level` trait: `ProceduralWorld`/`AuthoredMap`), collision (`resolve`, `blocking`). |
| `render*.rs` | Canvas2D renderer (`lift()` perspective + y-sort), lighting, authored maps, Outbreak/Siege/Home Base visuals. |
| `audio.rs` | Procedural SFX, adaptive music stems, lobby tunes, announcer. |
| `input.rs`, `ping.rs`, `callouts.rs`, `corpses.rs`, `destruct.rs`, `weather.rs`, `tutorial.rs`, `math.rs` | Action map, pings/emotes, kill feed/combos, corpses, destructibles, day/night + weather, tutorial, vector math. |

### UI (`ui/`, loaded by `index.html` via trunk)
| File | What |
|---|---|
| `app.js` | Menus, lobby, shop/locker catalog + prices (`CATALOG`), story text, settings, game-event handlers, `Data` (localStorage when signed out, cloud API when signed in), Home Base stall UI, Nemesis naming, event UI. |
| `coop.js` | Social socket client (friends, parties, invites, chat relay), WebRTC game transport, loading screen, ICE restart, rejoin, host migration, public parties/Quick Play, hub presence. |
| `voice.js` | Voice chat mesh (+ hub proximity volume). |
| `chat.js` | Chat panels. |
| `photo.js` | Photo mode panel + capture. |
| `gamepad.js` | Gamepad input. |
| `i18n.js` | Translations. |
| `progression.js` | Shared (UMD) progression rules, used by both the UI and `server/index.ts`: account XP, achievements, meta, relics, challenges, season, mutators, the limited-time `EVENT`. |
| `cloud-src.js` → `cloud.js` | Neon Auth client bundle. |
| `config.js` | Public API/auth URLs. |
| `style.css` | Styles. |

### Server (`server/`, Neon Functions; deploy with `npm run deploy:api`)
| File | What |
|---|---|
| `index.ts` | HTTP API (Hono + `pg`, JWT via Neon Auth JWKS). |
| `social.ts` | WebSocket social server (fan-out across isolates via the `events` table). |
| `vendor.ts` | AI vendor dialogue. |
| `chat.ts` | Chat filter/limits. |
| `catalog.ts` | Prices + coin caps. These **must mirror** `ui/app.js` CATALOG and the coin formulas in Rust. |
| `shared.ts` | DB pool, token verify, profiles. |
| `schema.sql` | Idempotent schema; append only. |

### Assets
- `assets/maps/*.json`: `asylum`, `refinery` (Outbreak), `keep`, `bridge`, `crossroads` (Siege), `hub` (Home Base + Outskirts), `testbox`.
- `assets/manifest.webmanifest`, `sw.js` (network-first service worker), icons.

---

## 5. Bridges, protocols and APIs

**UI → game commands** (`window.UHUI.send('cmd|args')`):
- Run control: `start|k=v;…`, `load|<save>`, `save`, `pause`, `resume`, `menu`.
- Co-op: `coop|<opts>`, `peerleft|slot`, `peerjoin|slot|name`, `promote|old`, `hostmoved|old|new`, `lobbymates|…`.
- Loadout: `equip|k=v;…`.
- Settings: `vol|`, `music|`, `tune|`, `autoaim|`, `access|`, `lookahead|`, `bind|`, `touch|`.
- Input: `dash`, `ult`, `use`, `act|`, `pad|`.
- Home Base: `hubmates|name~x~y~aim~skin~hat~gun~emote~party|…`, `hubbubble|name|text`, `hubclose`, `npcsay|id|line`.
- Other: `photo|…`, `sfx|ready|start|click`, `dev|…`.

**Start options** (`start|`, `coop|`, `load|`):
- Run setup: `name`, `best`, `mode=infinite|campaign|outbreak|siege|hub`, `chapter`, `diff`, `map`, `variant=glass`.
- Modifiers: `mut=…`, `seed`, `tutorial=1`.
- Loadout and cosmetics: `gun`, `perk`, `skin`, `hat`, `tracer`, `char`, `pack`, `killfx`, `deathfx`, `emote`.
- New in this round: `nemesis=<kind>,<rank>,<name>`, `event=hollow`.
- Co-op only: `role`, `me`, `names`, `present`, `late=1`.

**Game → UI events** (`uhEvent(kind, data)`):
- Run lifecycle: `ready`, `pause` (run JSON), `resume`, `saved`, `gameover`/`victory` (stats JSON with `squad`, `recap`, `candy`, `cause`, `build`, …), `wave`, `siegestars`.
- Feedback: `achievement`, `haptic`, `say` (announcer/subtitles), `tune`, `bindings`, `tutorial`, `usebtn`/`readybtn`, `netversion`.
- Home Base: `hubopen`/`hubleave`/`hubmenu`, `hubpos`, `hubemote`, `hubreward`.
- Nemesis: `nemesis` (`kind|again`), `nemesisslain` (rank).

**Co-op wire format** (`src/net.rs`, `PROTOCOL` = 14; mismatched peers bail with a toast):
- **Authority model:**
  - The host owns zombies, waves, enemy shots, gems, pickups and kill credit.
  - Every player owns their own character and bullets. Clients report hits; the host applies them.
- **Messages:**
  - `MSG_PSTATE`, `MSG_HITS`, `MSG_SNAP` (20 Hz, i16 positions)
  - `MSG_GEMS`, `MSG_EVENTS`, `MSG_HELLO`, `MSG_WORLD`
  - `MSG_BUILD`, `MSG_DEFENSE`, `MSG_READY`, `MSG_PING`
  - `MSG_CHECKPOINT` (1 Hz, host-only state for migration)
- **Events:**
  - `E_HURT`, `E_KILL`, `E_XP`, `E_HEAL`, `E_FRENZY`, `E_GAMEOVER`, `E_BLAST`, `E_PULL`
  - `E_WAVE_START`, `E_WAVE_CLEAR`, `E_BOSS`, `E_CHEST`, `E_POWERUP`
  - `E_BOARDS`, `E_WORLD`, `E_DEFENSE`, `E_DEFCHANGE`, `E_MANA`, `E_CRYSTAL`
  - `E_PING`, `E_VICTORY`
- On clients the host's WebRTC link is always keyed `0`; `Net.host_slot` tracks the real host slot after a migration.

**WebRTC signalling** (over the social socket, `signal` messages):
- `offer` (with `late`/`migrate`/`roster`), `answer`, `ice`, `nogo`
- `reoffer`/`restart` (ICE restart), `joinreq` (late join), `alive?`/`alive` (migration check)
- voice: `v-offer`/`v-answer`/`v-ice`

**Social socket** (`server/social.ts`):
- Client → server:
  - friends: `friends.add|accept|remove`
  - party: `party.create|invite|join|decline|leave|ready|diff|mode{mode,chapter}|look|kick|start|end|voice|public{on}`, `party.list`, `quickplay`
  - hub: `hub.pos{x,y,aim,look,emote?}`, `hub.leave`
  - moderation: `chat`, `block`, `unblock`, `report`
  - other: `signal`, `ping`
- Server → client: `hello`, `social`, `party`, `party.start`, `invite`, `signal`, `chat`, `hub`, `hub.instance`, `parties`, `toast`, `error`.

**HTTP API** (`server/index.ts`, plus `POST /vendor` in `vendor.ts`):
- Public: `GET /health`, `GET /leaderboard`, `POST /errors`.
- Signed-in:
  - profile: `GET /me`, `PUT /me/settings`, `POST /me/name`, `POST /me/import`
  - shop and loadout: `POST /shop/buy`, `POST /equip`
  - saves: `PUT /save`, `POST /save/consume`, `DELETE /save`
  - runs and rewards: `POST /runs`, `POST /welcome`, `POST /hub/reward`
  - progression: `POST /meta/buy`, `POST /relics/equip`, `POST /achievement`
  - networking: `GET /turn`

---

## 6. Conventions & how to work here

- **Feel comes first.** Keep the sim deterministic per step, interpolate everything that moves, and avoid per-frame allocations in hot loops.
- **Sounds** go through `Game::sfx(sfx, world_pos)`/`push_sfx` (stereo pan + distance). **Entities** are flagged `dead` during a step and removed with `retain` at the end of `step()`.
- **Reuse the patterns:**
  - Simulation: the spatial `Grid` for neighbour queries; `world::resolve`/`blocking` for collision.
  - Feedback: `burst`/`ring`/`push_text` for effects; `Banner`/`toast`/`say` for messages.
  - Co-op: `net.event(...)` plus a handler in `coop.rs::apply_events` for anything co-op must see.
  - Persistence: `Data.*` in `ui/app.js` with a matching `server/index.ts` route.
  - Content: tables go in `content.rs`; mode logic goes in its own `src/modes/*.rs` module, not new branches in `game.rs`.
- **Done means:**
  - `cargo test --release` passes, with a native test for new rules.
  - The wasm build has no warnings.
  - `trunk build --release` succeeds.
  - A relevant `tests/e2e/` script passes.
  - This file is updated.
- **Commit, push and server rules:**
  - One feature per commit, pushed to `main` when the tests pass (the owner tests on their phone).
  - Server changes: append to `server/schema.sql` (idempotent) and note them in §2 until deployed.
  - Never put secrets in the repo.
- **Bump protocols** for incompatible changes: `PROTOCOL` for wire or sim-rule changes, `SAVE_VERSION` for save-string changes.

---

## 7. Feature details

### 7.1 Original feature list (v1)
- [x] 1. **Co-op**: real-time WebRTC co-op for **up to 4 players**, host as hub, in Infinite, Campaign (the host's chapter), Outbreak and Siege.
  - **Down and out:**
    - Downed players can crawl. A teammate standing within 75 units for 3s revives them; Medic and Second Wind make it faster.
    - Bleeding out (30s) means spectating until the wave is cleared, then a respawn next to a teammate.
    - A team wipe ends the run.
  - **Connecting:**
    - The host offers per client over the social socket; ICE is trickled in batches.
    - `GET /turn` returns STUN plus short-lived Metered TURN credentials (`METERED_APP` + `METERED_SECRET_KEY` or `METERED_API_KEY`), falling back to STUN only.
    - Signals carry a session id (`party.seed`).
  - **Match start:**
    - Loading screen (`#coop-load`) with per-player link status.
    - After 20s the host gets "Start without them / Retry / Cancel"; a client gives up after 40s.
  - **Resilience:**
    - ICE restart with a 15s grace period.
    - Rejoin a running match, and join one in progress.
    - Host migration (see M7).
  - **Chat:**
    - Party text chat in the lobby and in matches (T or / to type, 💬 on touch).
    - The server caps length at 200, rate-limits, masks profanity and honours blocks.
    - Click a name for volume / Block / Report.
  - **Voice** (`ui/voice.js`):
    - An audio-only mesh: mute, open mic or push-to-talk (hold V), per-player volume, speaking rings, in-game chips.
    - Blocked players are never connected.
- [x] 2. **Persistence in Neon**:
  - Profiles: coins, owned items, equipped loadout/cosmetics, campaign progress, stats, progression JSON.
  - Saves and every run.
  - A global leaderboard that auto-refreshes every 5s while visible.
- [x] 3. **Accounts**: Neon Managed Auth with email+password and Google sign-in; sessions persist across reloads and devices. The first sign-in imports offline progress once (coins capped at 5000). Guests play offline with browser storage.
- [x] 4. **Shop & customizations**: coins come from waves (scaled by difficulty), bosses and chapter wins. They buy guns, perks, skins, hats, tracers, backpacks, kill/death effects and emotes.
- [x] 5. **Difficulties**: Rookie, Survivor, Veteran, Nightmare (`content::DIFFICULTIES`).
- [x] 6. **Loadouts**: 6 guns plus a starting perk, a survivor (character), relics and cosmetics, set in the 🎒 Locker.
- [x] 7. **Character screen**: an in-engine showcase on a pedestal (party members stand beside you); the Locker updates it instantly.
- [x] 8. **Campaign**: 5 chapters ("Rook and Mara vs. the Bloom").
  - Typewriter intros/outros with portraits and panel cutscenes, plus radio chatter per wave.
  - Unique bosses: Gravedigger, Matriarch, Cinder King, Twin Wraiths, Bloom Heart.
  - Chapters unlock in order.
- [x] 9. **Infinite**: endless waves, bosses every 5 waves, ranked on the leaderboard.
- [x] 10. **CoD/Fortnite-style lobby**:
  - Friends (codes, requests, presence).
  - Parties: up to 4 (server deploy pending; 3 live), ready checks, host-set difficulty and mode, and a shareable invite link via the native share sheet on phones.
  - Invites (`ui/coop.js` `inviteFriend`/`openPicker`): the "＋ Invite friends" button and open party slots open a friend picker; the friends list invites any online friend; the pause menu has "👥 Invite friends" (the note says what happens next: drop in after the wave in co-op, wait in the party after a solo run). Without a party one is created and queued invites go out when it exists. Accepting an invite into a running match auto-sends `joinreq`. E2E: `invite.mjs`.
  - Party members stand in your lobby.
- [x] 11. **Save slots: one per game type.** Infinite, Campaign, Outbreak and Siege each have their own slot, and they coexist.
  - **Overwriting:** saving a type (Save, or Save & quit, in the pause menu) replaces that type's save only. Starting a new run of a type discards that type's save. Tooltips on Play and Save say which save would be overwritten.
  - **Resuming:** a save resumes once (it's consumed), at the start of the current round/wave. The lobby shows one "⏯ Continue … run" button per saved type, with the selected mode's first.
  - **What's saved:**
    - common: `save_string`, `SAVE_VERSION` 3 (map + variant included)
    - Outbreak (`src/save_modes.rs`): points, guns (mag/reserve/upgraded), perks, opened doors, power, box position, self-revives, hound timing, shield parts / easter-egg progress
    - Siege: mana, crystal health, every defense (kind, position, tier, health); it resumes in the build phase before the next wave
  - **Not saved:** co-op games and the Home Base (Save is disabled there). Guests save in localStorage; signed-in players save in the `saves` table (primary key `(user_id, slot)`).
  - **Tests:** `save_modes::tests` (round-trips) and the `saves.mjs` e2e.
- [x] 12. **Lobby music picker**: 4 procedural tunes (`audio.rs` `LOBBY_TUNES`), cycled with 🎵.
- [ ] 13. **Safari and mobile.** Layouts, touch and the PWA are done; a real-device iOS pass is still open (see §2.4).
  - **Done:**
    - phone lobby with bottom nav, one panel at a time, and a lifted showcase
    - touch auto-detect (`pointer: coarse` / first touch); a Settings override (Auto/On/Off) is not built yet
    - floating joystick, tap-to-dash, auto-aim, touch USE/ready/emote/chat buttons
    - safe-area insets, portrait zoom, stacked level-up cards
    - Web Audio unlock on gesture, bearer-JWT auth fallback for ITP
    - PWA install
    - level-up power icons now visible on phones (solid card fill)
  - **To verify on a real device:** see §2.4.

### 7.1b Phone HUD (`src/hud_layout.rs`)
- `Game::hud_layout()` gives every HUD piece its place. Desktop keeps the classic layout. Phones (touch, or narrower than 640px, or shorter than 500px) get the **compact** layout:
  - **Top-left:** level badge + HP bar, ending above the HTML pause/chat row (`top: 52px`).
  - **Top-right:** a tight wave/remaining/stats/score column. Outbreak shows "ROUND n" (no chalk tallies on phones).
  - **Status stack:** boss and crystal bars with labels inside, plus a short phase line (`stack_bar`). Full width at y=112 in portrait; centred between the side panels in landscape.
  - **Under the stack:** teammate cards + kill feed on the left, the minimap on the right (no caption). The tutorial card goes under the minimap.
  - **Bottom-right thumb cluster** (`#touch-ui`, absolute positions in `ui/style.css`): DASH, ☢ above it, Q diagonally up-left, USE / READY beside DASH. ☢ and Q show charge/cooldown rings from the game's `abil` event (`ult|abilK|cd|icon|active`); the canvas copies are hidden on touch. `CLUSTER_W`/`CLUSTER_H` in `hud_layout.rs` must match the CSS box.
  - **Siege build bar:** full width above the cluster (portrait), or bottom-centre left of it (landscape). Portal labels are clamped into the free area.
  - **Outbreak:** points/weapon/perks above the cluster (portrait) or left of it (landscape); power-up timers under the stack.
  - **Subtitles** (`#subs`) lift above the build bar and the cluster (`body[data-mode]`).
- **Level-up tray** (`Game::levelup_layout`/`levelup_title`): three wide cards low on the screen with a light dim, so the player stays visible; buttons above (phones) or below (desktop) the cards; phone landscape puts the title on the buttons' row. The thumb cluster hides meanwhile (`levelup` UI event → `body.levelup`).
- A native test (`hud_layout::tests`) checks for overlaps on six phone sizes, and that the level-up tray and chest strip stay off the player. The e2e `mobile-hud.mjs` checks portrait, landscape and small phones.
- Fixed along the way: on short landscape phones (≤400px tall) the Siege portal labels called `f32::clamp` with min > max, which crashed the game.

### 7.2 Home Base (Destiny-style social hub)
Files: `modes/hub.rs`, `render_hub.rs`, `assets/maps/hub.json`, the stall UI in `ui/app.js`, `server/vendor.ts`.
- **Entry:** the lobby's "🏠 Enter Home Base" starts `mode=hub`. Esc returns to the lobby (with a confirm).
- **Vendors** (E or USE opens a docked stall with items to buy/equip, the vendor's line and a "Say something" box):
  - Rusty Vex: guns
  - Madame Lucienne: skins/hats
  - Sgt. Dole: perks/tracers
  - Ada: the mission terminal; launches Infinite, Campaign, Outbreak and Siege, with your party.
  - Lines come from `POST /vendor`:
    - Claude `claude-opus-5-5` at low effort via `@anthropic-ai/sdk`, with a cached persona prompt and server-side fallback.
    - Per-caller and global rate limits, plus a reuse cache.
    - Canned fallback lines.
  - Lines show as speech bubbles and are spoken with per-vendor Web Speech voices (Settings toggle).
- **Hall of Survivors** monument (leaderboard) and a **firing range** with unkillable dummies and a DPS meter.
- **The Outskirts** (free roam, east through the gate):
  - Weapons free, ambient zombies, destructible crates.
  - A gate turret zaps anything that follows you home.
  - No XP or gems in the hub.
  - Going down is a rescue back to base, never a game over.
  - **Supply Drops** land about every 2 minutes. Hold the beacon ring for 30s against a surge for 🪙 40 (`POST /hub/reward`, at most once per 2 min; guests get local coins).
- **Social:**
  - Party members walk the hub, with chat as speech bubbles and emotes over heads.
  - Proximity voice: full within 250 units, 10% at 1400+.
  - **Instances:** 16 players per instance, assigned by the server, with party members kept together and blocks respected.
    - Every `hub.pos` upserts a `hub_presence` row and returns a ≤2 Hz `hub.instance` snapshot.
    - Strangers have light-blue name plates, party members green; up to 24 are shown.
- **Hollow Night:** jack-o'-lanterns around the plaza while the event runs.
- **Ideas not built yet:** walking up to a stranger to invite them, loot caches, more public event types.

### 7.3 Roadmap v2 (research-backed; no real-money purchases) — all done except item 13's device test

**Pillars:**
1. Feel first.
2. Readable depth.
3. Three loops (Infinite / Outbreak / Siege) that all work in co-op.
4. Respectful retention: a free pass, no FOMO or loot boxes.
5. Everywhere: keyboard, gamepad and touch.

#### M0 · Foundations
- [x] F1 Enemy table (`content.rs::ENEMIES`, `EnemyDef`).
- [x] F2 Authored maps (`Level` trait, JSON maps, collision, `render_map.rs`).
- [x] F3 `Mode` enum + `ModeHooks`.
- [x] F4 Versioned saves + `MSG_HELLO` protocol check.
- [x] F5 Committed Playwright tests (`npm run e2e`).
- [x] F6 Action map with rebindable keys.

#### M1 · Modern feel
- [x] J1 Weapon feel: recoil kick, shell casings, muzzle flash light.
- [x] J2 Hit reactions: squash/stretch, knockback tilt, gibs/dissolves, fading corpses.
- [x] J3 Dynamic light: flashlight cone, muzzle/explosion/nova/lightning lights.
- [x] J4 Audio 2.0: layered guns, low-HP heartbeat + muffle, music stems, streak stingers, Web Speech announcer.
- [x] J5 Camera: aim lookahead, zoom punch, boss-kill slow-mo.
- [x] J6 Haptics: gamepad + `navigator.vibrate`, with an intensity setting.
- [x] J7 Callouts: kill feed, multi-kills, combo score.

#### M2 · Combat depth
- [x] C1 Reroll / Banish / Skip.
- [x] C2 8 evolutions.
- [x] C3 Chests with a 1/3/5 reveal: a small loot strip near the top (`Game::chest_rect`) that never pauses or waits for input and fades on its own (`chest::HOLD_T`).
- [x] C4 Elements: burn, chill/freeze, shock, poison, plus shatter and blight combos.
- [x] C5 Elite affixes: shielded, hasted, splitting, vampiric, explosive, reflective.
- [x] C6 Archetypes: crawler, screamer, armored, bat swarm, burrower.
- [x] C7 Boss 2.0: phases, weak points, telegraphs, enrage, intro cards; new bosses Butcher and Hive Queen.
- [x] C8 Survivors:
  - Rook/Gunner (heat-sink burst)
  - Mara/Engineer (turret)
  - Doc/Medic (heal pulse)
  - Ash/Pyro (flame cone)
  - Q is the ability, R the ultimate; survivors unlock via achievements.
- [x] C9 Projectile pressure, with fair telegraphs and graze outlines.

#### M3 · Outbreak (CoD Zombies homage, original names)
- [x] O1 Hollowbrook Asylum.
- [x] O2 Points.
- [x] O3 Interact prompts / doors.
- [x] O4 Magazines, reload, wall-buys.
- [x] O5 Mystery Box with 2 wonder weapons.
- [x] O6 Perks: Iron Gut, Quickdraw Cola, Second Wind, Fleetfoot.
- [x] O7 Power + upgrade machine.
- [x] O8 Power-ups: Max Ammo, Insta-Kill, Double Points, Nuke, Carpenter.
- [x] O9 Rounds feel + hound rounds.
- [x] O10 Buildables + easter egg.
- [x] O11 Traps.
- [x] O12 Co-op replication.
- [x] O13 Ashfall Refinery + Outbreak leaderboard.

#### M4 · Siege (Dungeon Defenders homage)
- [x] S1 Crystal + lanes/portals.
- [x] S2 Build/combat phases + wave preview.
- [x] S3 Per-player mana.
- [x] S4 Placement + 6 defenses ×3 tiers: arrow tower, flame trap, slow aura, barricade, mine, healing totem.
- [x] S5 Flow-field pathing.
- [x] S6 Sapper, ogre and archer enemies.
- [x] S7 3 maps (Last Keep, River Crossing, Crossroads), stars, first-clear rewards, Glass Crystal.
- [x] S8 Co-op placement + ready vote. The vote is sized for 4 players; slot 3 used to vote as the host.

#### M5 · Progression & loot (rules in `ui/progression.js`, shared with the server)
- [x] P1 Account levels.
- [x] P2 Meta upgrades + a "Pure" leaderboard filter.
- [x] P3 Relics (3 slots, rarity, affixes).
- [x] P4 Weapon mastery camos.
- [x] P5 53 achievements that gate survivors and cosmetics.
- [x] P6 Bestiary + codex; Career tab.

#### M6 · Live & retention (free, no FOMO)
- [x] L1 Daily (3) and weekly (5) challenges.
- [x] L2 Daily seeded run + daily board.
- [x] L3 Season 1 "The Bloom" free pass (30 tiers).
- [x] L4 Mutators + Custom mode (unranked).
- [x] L5 Leaderboard filters.
- [x] L6 Welcome-back bonus.

#### M7 · Multiplayer & social hardening
- [x] M1 **TURN + ICE restart.**
  - Metered TURN via `/turn`; the relay-only e2e passes.
  - A `disconnected`/`failed` link mid-match triggers an ICE restart: the host re-offers (`reoffer`), and clients ask with `restart`.
  - Players are kept for a 15s grace period with "reconnecting…" / "Reconnected ✔" toasts.
- [x] M2 **Rejoin & join in progress.**
  - "⚔ Join the match in progress" sends `joinreq`. The host re-offers (`late`) in the player's old slot or the lowest free one.
  - `peerjoin|slot|name` makes the host send HELLO plus a one-off world sync (doors, power, box, boards, defenses, crystal).
  - Late joiners spectate until the wave is cleared.
- [x] M3 **Host migration.**
  - M3a: a 1 Hz `MSG_CHECKPOINT` carries wave counters, uids, Outbreak round state, and the Siege phase/queue.
  - M3b: a client that loses the host first asks it `alive?` (2.5s).
    - If it answers, *this* client dropped: it leaves and can rejoin.
    - Otherwise the lowest slot still in the party promotes: replicas are rescaled to wave stats, and a lone spectator respawns. It then re-offers with `migrate`; the others accept only that offer (20s timeout).
  - A lone survivor carries on as a solo host.
- [x] M4 **4 players:** `MAX_PLAYERS` = 4, compressed snapshots.
- [x] M5 **Pings and quick-chat emotes.**
- [x] M6 **Squad scoreboard + MVP.**
- [x] M7 **Public parties & Quick Play:**
  - `parties.public`; the host toggles it with `party.public`.
  - `party.list` returns public parties with room and an online host, skipping blocks, fullest first.
  - `quickplay` joins the fullest squad or hosts a public one.
  - Joining one mid-match auto-sends `joinreq`.
- [x] M8 **Lobby chat** with block/report.
- [x] M9 **Campaign co-op:**
  - Party mode `campaign` + `parties.chapter`, clamped server-side to the host's unlocked chapter.
  - `E_VICTORY` replays the win on clients.
  - `/runs` only credits a chapter win if `chapter <= campaign.unlocked`.

#### M8 · World & presentation
- [x] V1 Day/night + rain/fog.
- [x] V2 Biomes: graveyard, pine forest, town ruins, snowfield.
- [x] V3 Explosive barrels / breakable crates.
- [x] V4 Animation polish + backpack, kill-effect, death-effect and emote slots.
- [x] V5 Music picker (4 tunes).
- [x] V6 Portraits + panel cutscenes.

#### M9 · Platform, UX & accessibility
- [x] A1 Gamepad + remap UI.
- [x] A2 Accessibility: colorblind palettes + shape coding, shake/flash sliders, reduce motion, UI scale, high contrast, hold/toggle, aim-assist strength, subtitles.
- [x] A3 PWA: manifest, network-first `sw.js`, install button. A real-device iOS pass is still open.
- [x] A4 Performance: about 0.5 ms/step at 1,000 zombies natively; LOD rendering for big hordes.
- [x] A5 Tutorial + loading tips.
- [x] A6 Settings sync.
- [x] A7 Balance telemetry (cause of death, build) + SQL views.
- [x] A8 i18n (en/es).
- [x] A9 Error reporting to `/errors`.

### 7.4 Beyond v2 (AAA research additions)
- [x] **AI Director** (`src/director.rs`, Left 4 Dead style):
  - Squad stress from crowding, low HP and damage drives BuildUp → Peak (hold 4s) → Relax (spawns 3.5× slower for 8–20s, plus a mercy heart for the most hurt player).
  - Quiet build-ups get a telegraphed **horde rush** from one side.
  - Wave totals are unchanged, and music follows the tempo. Host/solo only (Infinite + Campaign).
- [x] **Photo mode** (`src/photo.rs`, `ui/photo.js`), from the solo pause menu:
  - **Camera:** free camera (drag/WASD/pinch/wheel, leashed to 900 units) with the HUD and floating text hidden.
  - **Look:** vignette, 2.39:1 letterbox, a hide-player toggle, and 7 filters: None, Noir, Vintage, Vivid, Frost, Blood Moon, Dream.
  - **Capture:** P or the button saves a watermarked PNG with the filter baked in, via the share sheet on phones or a download elsewhere.
- [x] **Nemesis system** (`src/nemesis.rs`, Shadow of Mordor style):
  - **Rising:** the zombie that kills you in solo Infinite/Campaign rises as a named Nemesis. The name and title come from `ui/app.js` and are kept in synced settings.
  - **Hunting:** from wave 3 (certain by wave 8) it hunts you once per run with an elite affix, 8×+ HP, a red aura, a name plate and a taunt.
  - **Outcome:** kill it for a chest. If it kills you again it ranks up (max 5) with a scarier title.
  - Exploders/bats rise as brutes/runners.
- [x] **Death recap** (Apex style): the death screen shows "Killed by …" plus chips per damage source for the last 8s (`recap` in the run summary).
- [x] **Limited-time event: Hollow Night** (Oct 1 – Nov 2, `EVENT` in `ui/progression.js`, start option `event=hollow`):
  - A quarter of zombies wear jack-o'-lanterns.
  - About 5% of your credited kills drop 🍬. The server caps candy by kills.
  - The Home Base gets lanterns.
  - 100 candy unlocks the free **Jack-o'-Lantern** hat (`hat:pumpkin`, not for sale).
  - The Career tab shows an event card.
  - **Next event:** change `EVENT` (id/dates/item). Old rewards can rotate into the shop later.

### 7.5 Ideas backlog (not started)
- Kill cam / "Play of the Run" replay.
- Weapon attachments.
- More limited-time events.
- Hub loot caches and other public event types.
- Inviting a stranger by walking up to them in the hub.
- Raising the Siege `final_wave` for endless mode.

---

## 8. Backend (Neon)

- **Project:** `dawn-base-35494450`, branch `production`, region aws-us-east-2.
  - Config is in `neon.ts` (`auth: true`, function `api`, env `METERED_APP`, `METERED_SECRET_KEY`, `ANTHROPIC_API_KEY`).
  - `.neon` and `.env.local` are gitignored.
- **Server-authoritative:**
  - Purchases: atomic price + ownership checks.
  - Equips: you must own the item.
  - Run rewards: coins capped by `waveCoins()`; candy capped by kills; relic drops by bosses.
  - Unlocks: campaign unlocks and achievements.
  - Rate limits: Supply Drop rewards and vendor lines.
- **Tables:**
  - `profiles`, `saves`, `runs`, `friends`, `parties`, `party_members`, `party_invites`, `events`, `presence`, `blocks`, `reports`, `hub_presence`, relics/progression JSON on `profiles`.
  - Users live in `neon_auth."user"`; its `id` is a **uuid**, so cast with `id::text` when comparing to `user_id`.
- **Leaderboards:** best run per player, ranked; e2e test accounts (`uh-test-*@example.com`) are excluded.
- **Deploy:** `npm run db:schema` (applies `server/schema.sql`), then `npm run deploy:api` (`neon deploy --env .env.local`). The function and auth URLs are public, in `ui/config.js`.
- **CORS:** the allowlist is in `server/index.ts`. Auth trusted domains are managed with `neon neon-auth domain list|add` (https://cpoooo.github.io is added).
- **Auth emails/Google:** Neon's shared SMTP and dev OAuth for now. Production should configure its own (https://neon.com/docs/auth/production-checklist.md).

## 9. Application state (snapshot, 2026-10-11)

- **Size:** about 23.4k lines of Rust (game), 4.5k of UI JS/CSS, 1.8k of server TS/SQL.
- **Tests:** 55 native tests and 12 Playwright e2e suites (`npm run e2e`), all passing.
- **Live:** GitHub Pages serves the latest `main` (static WASM + JS). Everything client-side is live.
- **Backend:** one Neon project (Postgres + Managed Auth + one Functions app with HTTP API and WebSocket social server). The deployed function **lags the repo**: see §2 item 1 for everything waiting on `npm run db:schema && npm run deploy:api`.
- **Data:** roughly 120 runs, mostly from e2e test accounts (hidden from leaderboards after the deploy). There are no real-money purchases; coins are earned only.
- **Multiplayer:** peer-to-peer WebRTC (host-authoritative star, up to 4 players) with Metered TURN on the free plan. The social server relays lobby, chat, signalling and hub presence.
- **Open:** a real-device iOS pass (§2.4); Settings toggle for touch controls (Auto/On/Off); backlog in §7.5.

## 10. Scaling plan (for future reference)

What breaks first as players grow, and what to do about it. The tiers are rough guides; measure before acting (Neon dashboard, `client_errors`, function logs).

### Where the limits are today
- **Social fan-out by polling:** every function isolate polls the `events` table every 250 ms (`server/social.ts`), and every notification is a row per recipient. Load grows with isolates × poll rate plus messages × recipients.
- **Hub presence writes:** each player in the Home Base upserts `hub_presence` about 4×/s, and reads a snapshot up to 2×/s. 1,000 players in the hub means about 4,000 writes/s on the primary.
- **Leaderboards:** computed per request with `distinct on (user_id)` over `runs` (indexed, but it grows with every run). `/leaderboard` is public and uncached.
- **P2P co-op:** quality depends on the host's device and upload; there's no anti-cheat beyond server caps on rewards. TURN is on Metered's free plan (10 live credentials).
- **AI vendors:** each uncached line is a Claude call. Per-caller and per-isolate hourly caps exist, but there is no global budget across isolates.
- **Operations:** one production branch; the schema is an idempotent SQL file with no migration history; there's no staging environment and no alerting.

### Tier 1: hundreds of concurrent players (cheap, do first)
1. **Leaderboards:**
   - Keep a `best_runs` table (one row per user × board) upserted in `/runs`, and read boards from it.
   - Add `Cache-Control: public, s-maxage=10` on `/leaderboard`, and slow the client's 5s refresh to 15–30s.
2. **Connections:** use the Neon pooled connection string for functions; keep `pg` pools small per isolate.
3. **Hygiene:**
   - Cron-delete `events` older than 5 min (done in poll today) and stale `hub_presence`/`presence` rows.
   - Add a `runs (user_id, mode)` index.
   - Archive runs older than 90 days to a summary table.
4. **TURN:** move to a paid Metered plan or Cloudflare Calls TURN, and reuse credentials per user instead of minting per isolate.
5. **AI cost guard:**
   - A global daily budget row in Postgres, checked before calling Claude.
   - Cache more lines (the reuse pool), and use a smaller model (Haiku) for greet/browse events.
6. **Staging:** a Neon branch per PR (Neon branching) plus a preview Pages deploy, with `npm run e2e` in CI against it.

### Tier 2: thousands of concurrent players
1. **Realtime layer:** replace `events` polling with a push transport. The options, in order of effort:
   - Postgres `LISTEN/NOTIFY` (one listener per isolate).
   - Redis/Upstash pub/sub.
   - A managed realtime service (Ably / Pusher / Cloudflare Durable Objects).
   The client protocol (`{t:…}` messages) can stay the same.
2. **Hub presence out of Postgres:** keep positions in Redis (or Durable Objects per instance) with TTL keys, and broadcast instance snapshots from there. Assign instances in that store too.
3. **Read scaling:** a Neon read replica for leaderboards, profiles reads and the Career tab; writes stay on the primary.
4. **Matchmaking:** move Quick Play from a SQL scan to a queue (Redis sorted sets by mode/region/skill) and add region tags to parties.
5. **Observability:**
   - Structured logs, an error tracker (e.g. Sentry) next to `/errors`, and dashboards for p95 latency, socket counts and DB CPU.
   - Alerting on 5xx rate and TURN failures.
6. **Schema migrations:** switch `schema.sql` to numbered migrations (e.g. `node-pg-migrate`/`dbmate`) with history, and keep PITR backups enabled.

### Tier 3: tens of thousands and up / competitive play
1. **Authoritative servers** for public matches: run the Rust sim headless (it's already deterministic per step and has a native build) on regional game servers (e.g. Fly.io / Agones on Kubernetes). Keep P2P for friend parties. This also gives anti-cheat for leaderboards.
2. **Run validation:** have clients upload compact input/replay logs for top leaderboard runs and re-simulate them server-side before ranking.
3. **Data pipeline:**
   - Partition `runs` by month.
   - Stream to a warehouse (logical replication → BigQuery/ClickHouse) for balance telemetry instead of SQL views on the primary.
4. **Global edge:** keep static assets on a CDN (Pages/Cloudflare) with hashed filenames (trunk already does this) and long cache times. Put the API behind a CDN/WAF with per-IP rate limits.
5. **Accounts and trust & safety:**
   - Production SMTP and OAuth credentials (§8).
   - Abuse reporting review tools for `reports`/`blocks`.
   - Chat moderation beyond the word list (a moderation API).
   - Age/region compliance as needed.
6. **Team process:** load tests (k6 on the API and socket), chaos tests for host migration, a release train with feature flags (the Season/Event configs already work like content flags).

## 11. Known issues & caveats
- **TURN:** the Metered credential cap (see §2.3).
- **Pending deploy:** many signed-in features wait on the deploy (see §1).
- **AI vendors** are untested against the live Claude API locally: there's no local key, but the code typechecks against SDK 0.133.
- **Flaky e2e:** the party-join step in `coop`/`chat-voice` occasionally times out. Rerun.
- **Hub mates** are keyed by display name, so two strangers with the same name in one instance merge.
- **A Nemesis needs a melee or nearby killer;** deaths with no zombie within 900 units create none.
