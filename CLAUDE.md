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

- [ ] 1. **Co-op mode**: live online, real-time co-op for up to 3 players for now, with the ability to revive a downed teammate. Needs a realtime/websocket server.
- [x] 2. **Persistence in Neon DB**: profiles (coins, owned items, equipped loadout and cosmetics, campaign progress, stats), save states and every run are stored in Neon Postgres. The leaderboard is global (each player's best infinite run) and auto-refreshes every 5s while visible.
- [x] 3. **Login / accounts**: Neon Managed Auth (Better Auth), with email and password sign-up/sign-in and Google sign-in. Sessions persist across reloads and devices. On the first sign-in, offline progress is imported once (coins capped at 5000). Guests can still play offline with browser storage.
- [x] 4. **Shop & customizations**: coins are earned by clearing waves (scaled by difficulty) plus boss and chapter bonuses. They buy guns, perks, skins, hats and bullet tracers in the 🛒 Shop.
- [x] 5. **Multiple difficulties**: Rookie, Survivor, Veteran and Nightmare (`content::DIFFICULTIES`). They scale HP, damage, horde size, speed and coin rewards.
- [x] 6. **Loadouts**: a primary weapon (6 guns with distinct handling, in `content::GUNS`) plus a starting perk, chosen in the 🎒 Locker.
- [x] 7. **Character / inventory screen**: the lobby renders your character in-engine on a pedestal, live, facing your cursor. The Locker equips owned items and updates the model instantly.
- [x] 8. **Campaign mode**: 5 chapters with a story ("Rook and Mara vs. the Bloom"). Each has a typewriter intro and outro, radio chatter per wave, and a unique boss fight (Gravedigger, Matriarch, Cinder King, Twin Wraiths, Bloom Heart). Chapters unlock in order.
- [x] 9. **Infinite mode**: endless survival. Bosses rotate every 5 waves, and it is the only mode ranked on the leaderboard.
- [ ] 10. **Fortnite / CoD-style lobby**: before starting infinite co-op, players can invite friends to their lobby, customize their character, and see their friends list. The host sets the difficulty before loading into the game. *(The solo lobby already exists: character showcase, customization, mode and difficulty selection. Invites and friends need the realtime backend.)*

- [x] 11. **Separate save slots**: one Infinite save and one Campaign save, which can coexist. Starting a new run of a mode overwrites that mode's save. Tooltips on Play and Save warn before that happens.
- [ ] 12. **Lobby music picker**: let players choose the lobby song on the home screen (cycle through pre-made procedural tunes). The first lobby tune ("Graveyard Shuffle") is in `src/audio.rs`; tunes should be data-driven so new ones are easy to add.
- [ ] 13. **Make it playable in Safari and on mobile**: support Safari (desktop and iOS) and offer mobile play as an option.
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

## Backend (Neon)

- Project `dawn-base-35494450`, branch `production`, region aws-us-east-2. Config is in `neon.ts` (`auth: true`, function `api`). `.neon` and `.env.local` are gitignored.
- **API**: `server/index.ts` (Hono, `pg`, JWT verified against the Neon Auth JWKS). Prices and coin caps are in `server/catalog.ts` and **must stay in sync** with `ui/app.js` CATALOG and the coin formulas in `src/game.rs` (`wave_cleared`, boss kill, `campaign_victory`).
- **Server-authoritative**: purchases (price and ownership checks are atomic in SQL), equips (must own the item), run rewards (coins capped by `waveCoins()` for the reported progress), and campaign unlocks.
- **Schema**: `server/schema.sql` (idempotent), applied with `npm run db:schema`. Tables are `profiles`, `saves` and `runs`; users live in `neon_auth."user"`.
- **Deploy the API**: `npm run deploy:api` (`neon deploy --env .env.local`). The function URL and auth URL are public and live in `ui/config.js`.
- **Browser auth client**: `ui/cloud-src.js`, bundled to the committed `ui/cloud.js` with `npm run build:cloud`. Rebuild after editing it.
- **CORS**: the allowlist is in `server/index.ts`. Auth trusted domains are managed with `neon neon-auth domain list|add` (https://cpoooo.github.io is added; localhost is allowed by default).
- **Auth emails**: these use Neon's shared SMTP, which is fine for development. Production should configure custom SMTP (https://neon.com/docs/auth/production-checklist.md). Google sign-in uses Neon's shared dev OAuth credentials until you configure your own.
