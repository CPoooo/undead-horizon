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
- [ ] 2. **Persistence in Neon DB**: persist everything (player data, runs, progression, save states) in Neon Postgres and make the leaderboard live and global.
- [ ] 3. **Login / accounts**: users can sign in so their data is saved.
- [x] 4. **Shop & customizations**: coins are earned by clearing waves (scaled by difficulty) plus boss and chapter bonuses. They buy guns, perks, skins, hats and bullet tracers in the 🛒 Shop.
- [x] 5. **Multiple difficulties**: Rookie, Survivor, Veteran and Nightmare (`content::DIFFICULTIES`). They scale HP, damage, horde size, speed and coin rewards.
- [x] 6. **Loadouts**: a primary weapon (6 guns with distinct handling, in `content::GUNS`) plus a starting perk, chosen in the 🎒 Locker.
- [x] 7. **Character / inventory screen**: the lobby renders your character in-engine on a pedestal, live, facing your cursor. The Locker equips owned items and updates the model instantly.
- [x] 8. **Campaign mode**: 5 chapters with a story ("Rook and Mara vs. the Bloom"). Each has a typewriter intro and outro, radio chatter per wave, and a unique boss fight (Gravedigger, Matriarch, Cinder King, Twin Wraiths, Bloom Heart). Chapters unlock in order.
- [x] 9. **Infinite mode**: endless survival. Bosses rotate every 5 waves, and it is the only mode ranked on the leaderboard.
- [ ] 10. **Fortnite / CoD-style lobby**: before starting infinite co-op, players can invite friends to their lobby, customize their character, and see their friends list. The host sets the difficulty before loading into the game. *(The solo lobby already exists: character showcase, customization, mode and difficulty selection. Invites and friends need the realtime backend.)*

Also done: the pause screen shows full run stats (HP, XP, damage per shot, fire rate, crit, speed, powers) and can **save the run** (resumes from the start of the current wave; one save slot, consumed on resume).

### Where things live
- Content tables (ids shared with the UI catalog): `src/content.rs`. The shop catalog, prices and story text are in `ui/app.js`.
- UI to game commands: `start|k=v;...`, `equip|k=v;...`, `save`, `load|<save string>`, `pause`, `resume`, `menu`, `vol|`, `music|`, `autoaim|`.
- Game to UI events: `ready`, `pause` (run JSON), `resume`, `saved` (save string), `gameover` and `victory` (stats JSON), `autoaim`, `wave`.
- Player profile, leaderboard and saves currently go through the `Data` backend in `ui/app.js`. It defaults to `LocalBackend` (localStorage). A cloud backend registered as `window.UHCloud` takes over when signed in.
