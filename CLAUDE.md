# Undead Horizon — project notes for Claude

Infinite 2.5D top-down zombie wave survival in Rust + WebAssembly (Canvas2D, WebAudio).
Live at https://cpoooo.github.io/undead-horizon/ (deployed by `.github/workflows/pages.yml` on push to `main`).

## Build & run

```bash
trunk serve --release                              # local dev at http://127.0.0.1:8080
trunk build --release                              # production bundle in dist/
cargo build --target wasm32-unknown-unknown        # fast type-check
```

## Architecture

- `src/main.rs`: browser bootstrap, input listeners (keyboard, mouse, touch joystick), rAF loop, and the JS bridge (`window.uhPoll` command queue JS→Rust, `window.uhEvent` notifications Rust→JS).
- `src/game.rs`: all simulation. It uses a fixed 120 Hz step (`DT`) with render interpolation via `prev`/`pos` and an accumulator. It covers phases (Menu/Playing/LevelUp/Paused/Dying/GameOver), the player, zombie AI, weapons and powers, waves, pickups, and a spatial hash `Grid` that is rebuilt every step.
- `src/render.rs`: the Canvas2D renderer. Depth comes from `lift()` (perspective offset from the camera) plus y-sorting.
- `src/audio.rs`: procedural WebAudio sound effects and the adaptive music scheduler. There are no audio asset files.
- `src/upgrades.rs`: upgrade and superpower definitions and weighted level-up rolls.
- `src/world.rs`: the infinite deterministic world, generated from a hash of the cell coordinate, plus obstacle collision.
- `index.html`: HTML/CSS/JS menus, settings, and the leaderboard (currently localStorage).

## Conventions

- Gameplay feel comes first: keep the sim deterministic per step, interpolate everything that moves, and avoid per-frame allocations in hot loops.
- Sounds are queued through `Game::sfx(sfx, world_pos)` so they get stereo panning and distance attenuation.
- Entities are flagged `dead` during a step and removed with `retain` at the end of `step()`.

## Features to be implemented

- [x] 1. **Co-op mode**: real-time online co-op for up to 3 players (Infinite mode).
  - Gameplay is WebRTC peer-to-peer with the host as the hub. `src/net.rs` defines the wire format and authority model; `src/coop.rs` holds snapshots, replicas, hit reporting, kill credit, shared XP and revive; `ui/coop.js` holds the transport.
  - Downed players can crawl. A teammate standing within 75 units for 3s revives them. Bleed-out after 30s leads to spectating, and a team wipe ends the game for everyone.
  - Connecting: the host sends an offer per client over the social socket; ICE candidates are trickled in batches. `GET /turn` (API) returns STUN plus short-lived Metered TURN credentials (`METERED_APP` + `METERED_SECRET_KEY` or `METERED_API_KEY` in `.env.local`); without them it falls back to STUN only. Signals carry a session id (`party.seed`) so stale offers/candidates are ignored.
  - Match start: everyone sees the loading screen (`#coop-load`) with per-player link status; the host gets "Start without them / Retry / Cancel" after 20s, a client gives up after 40s. Sounds: `sfx|ready` (everyone ready), `sfx|start` (launch sting), `sfx|click`.
  - **Chat:** party text chat in the lobby panel and in matches (T or / to type, 💬 on touch), via the social socket (`chat` message). The server (`server/chat.ts`) caps length at 200, rate-limits (5-message bucket), masks profanity and skips recipients who blocked the sender. Click a name for volume / Block / Report (`blocks`, `reports` tables; unblock in Friends).
  - **Voice:** `ui/voice.js` runs an audio-only WebRTC mesh between party members who joined voice (lower user id offers; `v-offer|v-answer|v-ice` signals; TURN from `/turn`). Mute, open mic or push-to-talk (hold V), per-player volume, speaking rings, and in-game speaker chips. Blocked players are never connected. `party_members.voice` shows 🎙 in the party list.
- [x] 2. **Persistence in Neon DB**: profiles (coins, owned items, equipped loadout and cosmetics, campaign progress, stats), save states and every run are stored in Neon Postgres. The leaderboard is global (each player's best infinite run) and auto-refreshes every 5s while visible.
- [x] 3. **Login / accounts**: Neon Managed Auth (Better Auth), with email and password sign-up/sign-in and Google sign-in. Sessions persist across reloads and devices. On the first sign-in, offline progress is imported once (coins capped at 5000). Guests can still play offline with browser storage.
- [x] 4. **Shop & customizations**: coins are earned by clearing waves (scaled by difficulty) plus boss and chapter bonuses. They buy guns, perks, skins, hats and bullet tracers in the 🛒 Shop.
- [x] 5. **Multiple difficulties**: Rookie, Survivor, Veteran and Nightmare (`content::DIFFICULTIES`). They scale HP, damage, horde size, speed and coin rewards.
- [x] 6. **Loadouts**: a primary weapon (6 guns with distinct handling, in `content::GUNS`) plus a starting perk, chosen in the 🎒 Locker.
- [x] 7. **Character / inventory screen**: the lobby renders your character in-engine on a pedestal, live, facing your cursor. The Locker equips owned items and updates the model instantly.
- [x] 8. **Campaign mode**: 5 chapters with a story ("Rook and Mara vs. the Bloom"). Each has a typewriter intro and outro, radio chatter per wave, and a unique boss fight (Gravedigger, Matriarch, Cinder King, Twin Wraiths, Bloom Heart). Chapters unlock in order.
- [x] 9. **Infinite mode**: endless survival. Bosses rotate every 5 waves, and it is the only mode ranked on the leaderboard.
- [x] 10. **Fortnite / CoD-style lobby**: before starting infinite co-op, players can invite friends to their lobby, customize their character, and see their friends list. The host sets the difficulty before loading into the game.
  - The social server is in `server/social.ts`: a Neon WebSocket with fan-out across isolates through the `events` table.
  - **Friends:** friend codes, requests, presence, and an invite button.
  - **Parties:** up to 3 players, with ready checks and a host-set difficulty. A shareable `?party=ID&t=TOKEN` link joins the party and makes you friends with the host.
  - **Lobby:** party members stand next to you in the lobby showcase.

- [x] 11. **Separate save slots**: one Infinite save and one Campaign save, which can coexist. Starting a new run of a mode overwrites that mode's save. Tooltips on Play and Save warn before that happens.
- [x] 12. **Lobby music picker** *(→ Roadmap v2 · V5)*: let players choose the lobby song on the home screen (cycle through pre-made procedural tunes). The first lobby tune ("Graveyard Shuffle") is in `src/audio.rs`; tunes should be data-driven so new ones are easy to add.
- [ ] 13. **Make it playable in Safari and on mobile** *(→ Roadmap v2 · A3)* *(mobile layout done; Safari and iOS real-device testing still open)*. Done so far:
  - **Lobby:** the phone lobby uses a bottom nav with one panel at a time, a lifted character showcase, and large touch targets.
  - **Level-up:** cards stack vertically on narrow screens.
  - **Overlays:** they scroll full-width.
  - **Banners:** they stay clear of the touch minimap.
  - **Still open:** the items below.: support Safari (desktop and iOS) and offer mobile play as an option.
  - **Detecting mobile**: decide when mobile controls appear. Use `matchMedia('(pointer: coarse)')` together with `(hover: none)`, and switch on the first `touchstart` instead of sniffing the user agent. That way touch laptops and iPads with keyboards aren't misdetected, and the controls never get in the way on a laptop. Add an explicit override in Settings ("Touch controls: Auto / On / Off").
  - **Overhaul or adapt?** The game canvas already scales to any size and has a floating joystick, tap-to-dash, auto-aim and touch buttons. So in-game play does **not** need a redesign: add safe-area insets, bigger HUD hit targets, and a portrait-mode zoom. The HTML menus (lobby 3-column grid, shop and locker grids, pause stats) collapse into one column below 980px today but are long and scroll-heavy. A good middle ground is a **tabbed bottom-nav lobby on mobile** (Play / Party / Shop / Locker / Board), reusing the same panels, instead of a separate mobile UI.
  - **Safari specifics**:
    - Web Audio must be unlocked inside a touch or click gesture (we do this, but verify on iOS).
    - Check `desynchronized` canvas support.
    - WebRTC works in Safari but needs real-device testing.
    - Third-party cookie blocking (ITP) can break the Neon Auth session restore across domains. The bearer-JWT fallback in `ui/cloud-src.js` should cover it; verify on iOS.
  - **If this grows too big**: put a pin in the full mobile lobby redesign and ship only "Safari works + touch controls auto-detect" first.
 shows full run stats (HP, XP, damage per shot, fire rate, crit, speed, powers) and can **save the run** (resumes from the start of the current wave; one save slot, consumed on resume).

### Where things live
- Content tables (ids shared with the UI catalog): `src/content.rs`. The shop catalog, prices and story text are in `ui/app.js`.
- UI to game commands: `start|k=v;...`, `equip|k=v;...`, `save`, `load|<save string>`, `pause`, `resume`, `menu`, `vol|`, `music|`, `autoaim|`.
- Game to UI events: `ready`, `pause` (run JSON), `resume`, `saved` (save string), `gameover` and `victory` (stats JSON), `autoaim`, `wave`.
- Player profile, leaderboard and saves go through `Data` in `ui/app.js`: localStorage when signed out, and the cloud API when signed in (`window.UHCloud`, from `ui/cloud-src.js`).

## Home Base (free-roam social hub, à la Destiny's Tower)

**Built (v1):** `modes/hub.rs`, `render_hub.rs`, `assets/maps/hub.json`, the vendor stall UI in `ui/app.js`, and `server/vendor.ts`.
- Lobby "🏠 Enter Home Base" starts `mode=hub`. Vendor NPCs: Rusty Vex (guns), Madame Lucienne (skins/hats), Sgt. Dole (perks/tracers), Ada (missions terminal); the memorial shows the leaderboard; a firing range has unkillable dummies and a DPS meter (shooting is only allowed there).
- E opens a stall (docked panel): items to buy/equip, the vendor's line, and a "Say something" box. Lines come from `POST /vendor` (Claude `claude-opus-5-5` at low effort via `@anthropic-ai/sdk`, cached persona prompt, server-side refusal fallback, per-caller rate limits, a reuse cache) and fall back to canned lines without `ANTHROPIC_API_KEY`. Lines show as speech bubbles and are spoken with per-vendor Web Speech voices (Settings toggle).
- Party members in the hub are relayed through the social socket (`hub.pos` → `{t:'hub'}`), drawn walking around, with chat as speech bubbles.
- **Still open:** instanced hubs with strangers (16–24), emotes, a free-roam zone, hub-wide proximity voice.

Original design notes:

- **What:** a walkable, persistent hub world that replaces (or sits behind) the menu lobby. You spawn in as your equipped character and can see and walk around with other players who are in the Home Base at the same time, not only party members.
- **Vendors replace menu screens:** everything bought in the main menu today (guns, perks, skins, hats, tracers, later relics/permanent upgrades/season pass) is sold by in-world vendor NPCs at stalls. Walk up, press Interact, and a shop panel opens anchored to that vendor; the menu Shop stays as a fallback on mobile/accessibility.
- **AI vendors with personality:** each vendor has a name, look, voice and attitude (e.g. a paranoid gunsmith, a theatrical tailor, a deadpan quartermaster). They greet you, react to what you browse and buy ("Ooh, the Thunder Coat. Bold. Zombies hate bold."), complain when you're broke, and drop random funny one-liners. Lines come from an LLM (Claude via the `api` function, with a persona prompt, the item catalog and the player's purchase context; short, PG-13, cached and rate-limited, with a canned-line fallback offline). They make noises: synthesized voice barks/grunts in `audio.rs`, optionally Web Speech for full lines (toggle).
- **Social:** proximity text bubbles and the existing voice chat (proximity volume), emotes, party up by walking to someone and inviting, a mission terminal/board that starts Infinite, Campaign, Outbreak or Siege with your party, the leaderboard as an in-world monument, and a firing range to try guns before buying.
- **Tech sketch:**
  - Instanced hubs of ~16–24 players, assigned by the social server.
  - Movement-only replication (position, facing, look, emote) at about 10 Hz.
  - Relay over the social WebSocket, or a dedicated realtime relay, rather than mesh WebRTC.
  - An authored hub map (needs F2), reused NPC rendering, and server-authoritative purchases exactly as today.
- **Free roam:** the hub opens out into a small free-roam zone with ambient zombies, public events and loot caches. Pick-up/drop-out with anyone in the instance.

## Roadmap v2 (research-backed; no real-money purchases)

### Pillars
1. **Feel first.** Every frequent action gets layered feedback, and intensity is always adjustable.
2. **Readable depth.** Simple controls, deep builds: synergies, evolutions, elements, affixes.
3. **Three loops.** Infinite (survivor-like), Outbreak (CoD Zombies point economy), Siege (Dungeon Defenders build and fight). All three work in co-op.
4. **Respectful retention.** Free seasonal pass, dailies and weeklies, unlocks as goals, no FOMO or loot-box mechanics.
5. **Everywhere.** Keyboard, gamepad and touch; Safari and mobile; accessibility options.

### How agents execute a task (token-optimal protocol)
- **One task = one session = one commit.** Sizes: **S** ≈ under 40k tokens, **M** ≈ 40–120k. Anything larger is already split into subtasks.
- **Read only what's listed.** `src/game.rs` is about 2,900 lines: `grep -n` for the named anchors and read those ranges, never the whole file. Content goes in `src/content.rs` tables. Mode logic goes in its own module (`src/modes/*.rs`), not more branches in `game.rs`.
- **Reuse the existing patterns:**
  - spatial `Grid` (game.rs) for neighbor queries
  - `world::resolve`/`blocking` for collision
  - `Game::sfx`/`push_sfx` for positional audio
  - `burst`/`ring`/`text` for effects
  - `Banner`/`toast` for messages
  - `net.event(...)` plus a handler in `coop.rs::apply_events` for anything co-op must see
  - `Data.*` in `ui/app.js` with matching `server/index.ts` routes for persistence
  - `server/catalog.ts` must mirror any prices or rewards
- **Done means:**
  - `cargo test --release` passes; add a native sim test like `coop::tests` or `game::tests` for new rules
  - `cargo build --target wasm32-unknown-unknown` has no warnings
  - `trunk build --release` succeeds
  - a Playwright smoke script in `tests/e2e/` passes (move the ones from the scratchpad there in F5)
  - this checkbox is ticked
- **Server changes:** append to `server/schema.sql` (idempotent), then `npm run db:schema` and `npm run deploy:api`. After any edit to `ui/cloud-src.js`, run `npm run build:cloud` (CI also does this).

### M0 · Foundations (do first; unblocks everything)
- [x] F1 (M) **Enemy table.** Move `ZKind::base()` stats, colors and behavior flags into `content.rs::ENEMIES` with an `EnemyDef` struct. Add new kinds by data. Done when zombie behavior is unchanged and all tests pass.
- [x] F2 (M) **Authored maps.** Add a `Level` trait with `ProceduralWorld` (wraps `world.rs`) and `AuthoredMap`, loaded from `assets/maps/*.json`:
  - contents: walls as rect/polyline, doors with id/cost/linked zones, spawn windows, props, interactables with type and position
  - collision via `resolve`; rendering in `render.rs::draw_obstacle_base` style
  - Done when a test map loads and the player collides with its walls.
- [x] F3 (M) **Mode abstraction.** Replace `campaign: bool` with `enum Mode { Infinite, Campaign(ch), Outbreak(map), Siege(map) }` plus a `ModeHooks` trait:
  - hooks: `on_start`, `update_waves`, `on_kill`, `hud`, `is_over`
  - move the wave and campaign code into `src/modes/{infinite,campaign}.rs`
  - Done when behavior is identical.
- [x] F4 (S) **Version fields.** Add `v` to save strings (`save_string`/`load_save`) and to net messages (a hello with protocol version, rejecting mismatches with a toast).
- [x] F5 (S) **Committed tests.** Move the Playwright scripts (solo, co-op two-browser, mobile) into `tests/e2e/` with an `npm run e2e` script. Document the local NSS-lib workaround.
  - Done: `npm run e2e` (serve `dist/` on :8765 first: `cd dist && python3 -m http.server 8765`). In WSL without system libnss3, extract the `libnss3`/`libnspr4` debs into `~/.cache/uh-libs` (or set `UH_LIBS`); `UH_CHROME` overrides the Chromium path. Screenshots go to `tests/e2e/out/`.
- [x] F6 (M) **Action map.** All input goes through `Action` enums (`Move`, `Aim`, `Dash`, `Ult`, `Interact`, `Ping`, `Pause`, `Ability`) bound from a settings table. Key handlers in `main.rs` map codes to actions. Prerequisite for A1 and A2.

### M1 · Modern feel (quick wins, high impact)
- [x] J1 (S) **Weapon feel.** Per-gun recoil camera kick, shell-casing particles, muzzle light flash.
  - These go in the `GUNS` table (`content.rs`) and `fire()`.
- [x] J2 (M) **Hit reactions.**
  - Squash and stretch on hit, plus directional knockback tilt.
  - Death variety: gib limbs or dissolve on elemental kills.
  - Corpses fade instead of popping.
- [x] J3 (M) **Dynamic light.** A light-mask pass after the world draw: player flashlight cone that follows aim, plus muzzle, explosion, nova and lightning lights punching holes in the night overlay (`draw_lighting`).
- [x] J4 (M) **Audio 2.0.**
  - Layered gun sounds (transient, body, tail).
  - Low-HP heartbeat and muffle filter.
  - Music stems that crossfade by intensity.
  - Kill-streak stingers.
  - Announcer voice lines ("Multi-kill!", "Max ammo!") via short synthesized vocoder-style sounds or the Web Speech API behind a toggle.
- [x] J5 (S) **Camera.** Slight aim lookahead (toggle; the player stays roughly centered), zoom punch on ult or boss spawn, slow-mo on boss kill (extend `hitstop`).
- [x] J6 (S) **Haptics.** Gamepad `vibrationActuator` and `navigator.vibrate` on hurt, ult and boss, with an intensity setting.
- [x] J7 (S) **Callouts.** Kill feed (co-op aware), multi-kill and headshot-style callouts, and a combo multiplier that feeds score.

### M2 · Combat depth (survivor-like best practices)
- [x] C1 (S) **Level-up options.** Reroll, Banish and Skip with limited charges (meta-upgradable in P2). Extend `Levels::roll` and `draw_levelup`.
- [x] C2 (M) **Evolutions.**
  - A maxed power plus a specific passive evolves at a chest: Blades + Speed → "Reaper Ring", Lightning + Crit → "Storm Crown", and so on (6–8).
  - Evolution table in `upgrades.rs`; render an EVOLVED card.
- [x] C3 (M) **Chests.** Elites and bosses drop a chest with a slot-machine reveal of 1, 3 or 5 picks. Weight it so early chests are generous.
- [x] C4 (M) **Elements.** A status system on `Zombie`:
  - burn (damage over time)
  - chill (slow, then freeze)
  - shock (chains)
  - poison (stacking)
  - Combos: chill + shock = shatter; burn + poison = blight.
  - Rework the Frost Nova, Lightning and Explosive sources to apply them.
- [x] C5 (M) **Elite affixes.** Shielded, hasted, splitting, vampiric, explosive-death, reflective, with a colored aura, shape icon and nameplate. They spawn by wave scaling.
- [x] C6 (M) **New archetypes** (via F1 data):
  - crawler (low, fast)
  - screamer (buffs nearby)
  - armored (front-armor, needs pierce or flank)
  - bat swarm (flyer, ignores obstacles)
  - burrower (pops up near the player with a telegraph)
- [x] C7 (M) **Boss 2.0.** Multi-phase bosses with weak points (crit zones), clear telegraphs (ground decals), and enrage timers. Boss intro card (name plus taunt). Add 2 new bosses.
- [x] C8 (L, split) **Characters / classes** with unique starting weapon, passive and active ability (Q becomes a per-character ability, ultimate moves to R):
  - [x] C8a (M) character table plus selection UI in the lobby showcase; persist in profile and server `equipped.character`
  - [x] C8b (M) 4 kits:
    - **Gunner:** heat-sink burst
    - **Engineer:** deployable turret (reused by Siege)
    - **Medic:** heal pulse and faster revives
    - **Pyro:** flame cone
  - [x] C8c (S) unlock gating through achievements (P5)
- [x] C9 (S) **Projectile pressure.** More enemy projectiles with fair telegraphs (dodge skill expression), plus a "danger" outline for bullets hitting the player.

### M3 · Outbreak mode (CoD Zombies homage — use original names, no trademarks)
- [x] O1 (M) **First map.** "Hollowbrook Asylum" in `assets/maps/asylum.json`: about 6 zones, doors and debris, spawn windows with barricades. Depends on F2 and F3.
- [x] O2 (M) **Point economy.** Points per hit, kill and headshot-crit; repairing barricades earns points. HUD points counter; co-op has per-player points. New `src/modes/outbreak.rs`.
- [x] O3 (S) **Interact prompt.** An `Interact` action (E, or a tap button on touch) with a world-space prompt "[E] Open door — 750". Doors open zones; spawns are gated by opened zones.
- [x] O4 (M) **Ammo, reload and wall-buys.**
  - Introduce magazines and reload (Outbreak only; Infinite keeps infinite ammo) with a reload bar and an empty-click sound.
  - Wall-buys: a chalk outline sells a weapon or refills its ammo.
- [x] O5 (M) **Mystery Box.** Spin animation cycling gun icons, a weighted pool including 2 box-exclusive wonder weapons (e.g. "Ray Rifle", "Thundergun-style" wave gun with original visuals), and a "zombie hand" result that relocates the box.
- [x] O6 (M) **Perk machines.** 4 perks, original names: *Iron Gut* (+HP), *Quickdraw Cola* (reload/fire), *Second Wind* (fast revive, or solo self-revive ×3), *Fleetfoot* (speed). Max 4 perks, all lost when downed. Jingles via synth.
- [x] O7 (S) **Upgrade machine.** Power switch, then the upgrade machine ("Pack-a-Punch" homage): ×2 damage, bigger magazine, an element (from C4), a renamed weapon and a camo tint.
- [x] O8 (S) **Power-ups.** Max Ammo, Insta-Kill, Double Points, Nuke, Carpenter, each with a timer HUD and an announcer line.
- [x] O9 (S) **Rounds feel.** Tally-mark round counter, round-transition sting, and special "hound rounds" (fast pack) every 5–6 rounds.
- [x] O10 (M) **Secrets.** Buildables (collect 3 parts to build a shield or trap) and a multi-step easter-egg quest with a reward; secrets are server-tracked achievements.
- [x] O11 (S) **Traps.** Buyable electric and fire traps (shared code with Siege S4).
- [x] O12 (M) **Co-op replication.** Host-authoritative door, box, power and perk-machine state through snapshot fields and events. Per-player points via events. Revive costs nothing; Second Wind changes the revive time.
- [x] O13 (M) **Second map.** "Ashfall Refinery", plus an Outbreak leaderboard (round reached, per map, by player count).

### M4 · Siege mode (Dungeon Defenders homage)
- [x] S1 (M) **Crystal.** Crystal entity with HP and a lose condition. Authored map with lanes and spawn portals. New `src/modes/siege.rs`. Depends on F2 and F3.
- [x] S2 (S) **Phases.** Build phase (timer or "Ready" vote in co-op) alternates with combat waves; phase HUD and wave preview (enemy icons per lane).
- [x] S3 (S) **Mana.** Drops from kills and is collected like gems (reuse `Gem` with a kind field). Shared team pool vs per-player: per-player.
- [x] S4 (L, split) **Defenses:**
  - S4a (M) placement system: grid snap, ghost preview, range ring, valid/invalid tint, mana cost, sell and upgrade menu
  - S4b (M) 6 defenses:
    - arrow tower
    - flame trap
    - slow aura
    - barricade wall (blocks paths)
    - proximity mine
    - healing totem
    - Each has 3 upgrade tiers.
  - S4c (S) the Engineer class (C8) gets +defense stats, and towers scale with hero level
- [x] S5 (M) **Pathfinding.**
  - Flow-field (BFS on a coarse grid) to the crystal, recomputed when defenses change.
  - Enemies attack blockers in their way.
  - Flyers ignore the grid.
  - Add a native test for path recompute.
- [x] S6 (S) **Siege enemies.** Siege-only types: goblin sapper (targets towers), ogre (high HP), archer (outranges towers).
- [x] S7 (M) **Maps and ratings.** 3 Siege maps with star ratings (crystal HP left), first-clear rewards, and Siege-specific challenge variants.
- [x] S8 (M) **Co-op replication.** Host-authoritative defense placement and build-phase ready-vote via the party and net events.

### M5 · Progression & loot
- [x] P1 (M) **Account level.** Account XP from all modes; level rewards (coins, cosmetics) shown on a progress track. Server: `profiles.account_xp`; award in `/runs`.
- [x] P2 (M) **Permanent upgrades.** A coin-sink tree (tiny % boosts, plus reroll and banish charges).
  - The leaderboard gets a "Pure" filter (no meta upgrades) for fairness.
  - Server stores ranks and validates costs (mirror in `catalog.ts`).
- [x] P3 (M) **Relics.** 3 loadout relic slots. Relics drop from bosses with rarity and 1–2 affixes (Dungeon Defenders loot); inventory UI in the Locker. Server: `relics` table; drops validated against run progress.
- [x] P4 (S) **Weapon mastery.** Kills per gun unlock camo tints (per-gun counters in the profile).
- [x] P5 (M) **Achievements.** 50+ achievements (some hidden) with toasts. Server table plus client checks; they gate characters, cosmetics and the Outbreak secret rewards.
- [x] P6 (S) **Bestiary and Codex.** Enemy entries unlocked on first kill, plus a lore page (campaign story).

### M6 · Live & retention (free, no FOMO)
- [x] L1 (M) **Challenges.** Daily (3) and weekly (5) challenges, generated server-side from a date seed. Progress is reported in `/runs`; rewards are coins and season XP.
- [x] L2 (M) **Daily seeded run.**
  - Same seed, mode and mutator for everyone.
  - One attempt counts for the board; a separate daily leaderboard.
  - Seed passed via `start|seed=`; the rng already takes a seed.
- [x] L3 (M) **Free seasonal pass.** 30 tiers of cosmetics. The season config is server JSON; earned through season XP; past seasons' cosmetics rotate back into the shop later (no permanent FOMO).
- [x] L4 (S) **Mutators.**
  - Weekly mutators: big-head zombies, glass cannon, fast horde, one-hit, low gravity knockback.
  - Selectable in a "Custom" mode with no ranking.
- [x] L5 (S) **Leaderboard upgrades.** Filters: mode, map, player count, friends-only, weekly and all-time. Server query params plus indexes.
- [x] L6 (S) **Welcome-back bonus.** Gentle, with no streak-loss punishment.

### M7 · Multiplayer & social hardening
- [ ] M1 (S) **TURN relay.** Add a TURN server (Cloudflare Calls TURN, or a free tier) with credentials minted by the `api` function (`/turn`), plus ICE restart on failure. Known gap today.
- [ ] M2 (M) **Reconnect and join in progress.**
  - A dropped client rejoins: same slot, host resends a full snapshot.
  - A new friend joins mid-run as spectator, then spawns on wave clear.
- [ ] M3 (L, split) **Host migration.**
  - M3a (M) periodic compact world checkpoint from host to clients
  - M3b (M) on host loss, the lowest slot promotes itself, rebuilds from the checkpoint, and re-signals peers
- [ ] M4 (S) **4 players.** `MAX_PLAYERS=4`, party max 4, UI slots. Profile the snapshot size; quantize to i16 if needed.
- [ ] M5 (S) **Ping and quick chat.**
  - Ping wheel: "Here", "Enemy", "Need revive", "Box here".
  - World markers and minimap pings, plus quick-chat emotes.
- [ ] M6 (S) **End-of-match scoreboard.** Kills, downs, revives, points, damage, MVP; per-player stats via events.
- [ ] M7 (M) **Public parties and Quick Play.** Opt-in public parties listed via the social server; auto-fill to 3–4.
- [x] M8 (S) **Lobby text chat.** Rate limit, length cap, a basic profanity filter, and block/report (server table).
- [ ] M9 (S) **Co-op modes.** Campaign co-op: the host's chapter, with progress credited to all who have it unlocked.

### M8 · World & presentation
- [ ] V1 (M) **Day/night and weather.** Day/night cycle in Infinite, plus weather (rain streaks, fog) that changes visibility and spawns (more runners at night).
- [ ] V2 (M) **Biomes.** Biome regions in the procedural world (graveyard, pine forest, town ruins, snowfield) with biome props and native zombie variants (via F1).
- [ ] V3 (S) **Destructibles.** Explosive barrels, breakable crates (drops), and environmental kills.
- [ ] V4 (M) **Character polish.** Better animation (procedural walk bob, reload, hurt flinch) and new cosmetic slots: backpack, kill effect, death effect, emote.
- [x] V5 (S) **Music picker.** Implements item 12: a data-driven tune list in `audio.rs` and a selector in Settings.
- [ ] V6 (S) **Story presentation.** Campaign dialogue portraits and panel cutscenes in the story overlay.

### M9 · Platform, UX & accessibility
- [x] A1 (M) **Controls.** Gamepad support (Gamepad API: twin-stick, triggers fire, D-pad UI navigation) and a remapping UI. Depends on F6.
- [x] A2 (M) **Accessibility.**
  - colorblind palettes with shape-coded telegraphs and elites
  - shake and flash intensity sliders, plus reduce-motion
  - UI scale and high-contrast text (WCAG 4.5:1)
  - hold vs toggle options
  - aim-assist strength slider
  - subtitles for all radio and announcer lines
- [x] A3 (M) **Safari, iOS and PWA.** (PWA + layouts done; a real-device iOS pass is still worth doing) Finishes item 13: real-device Safari/iOS pass (Web Audio unlock, ITP and auth), a PWA manifest and service worker (offline solo play, install prompt), and landscape vs portrait HUD layouts.
- [ ] A4 (M) **Performance.** Pool particles, bullets and texts; batch draws; add a perf budget test (1,000 zombies at 60 fps). Evaluate a WebGL renderer if Canvas2D tops out (spike only).
- [ ] A5 (S) **Onboarding.** First-run interactive tutorial (move, dash, level-up, revive), contextual tips, and loading tips.
- [ ] A6 (S) **Settings sync.** Sync settings to the cloud profile.
- [ ] A7 (S) **Balance telemetry.** Privacy-light: run summaries (mode, wave, build, cause of death) to Neon, plus a SQL view for balancing.
- [ ] A8 (S) **Localization.** i18n string table framework (en first).
- [x] A9 (S) **Error reporting.** The panic hook and `window.onerror` post to `/errors` (rate-limited) for live debugging.

### Suggested order
- M0
- Then quick wins: J1, J5, J7, C1, C3, A9
- Then M2 combat depth
- Then the **Outbreak MVP**: O1–O8
- Then the **Siege MVP**: S1–S5
- Then M5 and M6 (meta and live)
- Then M7: M1 first
- Then M8 and M9 polish

Use M7 · M1 (TURN) earlier if friends report connection failures.

## Backend (Neon)

- Project `dawn-base-35494450`, branch `production`, region aws-us-east-2. Config is in `neon.ts` (`auth: true`, function `api`). `.neon` and `.env.local` are gitignored.
- **API**: `server/index.ts` (Hono, `pg`, JWT verified against the Neon Auth JWKS). Prices and coin caps are in `server/catalog.ts` and **must stay in sync** with `ui/app.js` CATALOG and the coin formulas in `src/game.rs` (`wave_cleared`, boss kill, `campaign_victory`).
- **Server-authoritative**: purchases (price and ownership checks are atomic in SQL), equips (must own the item), run rewards (coins capped by `waveCoins()` for the reported progress), and campaign unlocks.
- **Schema**: `server/schema.sql` (idempotent), applied with `npm run db:schema`. Tables are `profiles`, `saves` and `runs`; users live in `neon_auth."user"`.
- **Deploy the API**: `npm run deploy:api` (`neon deploy --env .env.local`). The function URL and auth URL are public and live in `ui/config.js`.
- **Browser auth client**: `ui/cloud-src.js`, bundled to the committed `ui/cloud.js` with `npm run build:cloud`. Rebuild after editing it.
- **CORS**: the allowlist is in `server/index.ts`. Auth trusted domains are managed with `neon neon-auth domain list|add` (https://cpoooo.github.io is added; localhost is allowed by default).
- **Auth emails**: these use Neon's shared SMTP, which is fine for development. Production should configure custom SMTP (https://neon.com/docs/auth/production-checklist.md). Google sign-in uses Neon's shared dev OAuth credentials until you configure your own.
