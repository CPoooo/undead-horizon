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

1. **Co-op mode**: live online, real-time co-op for up to 3 players for now, with the ability to revive a downed teammate.
2. **Persistence in Neon DB**: persist everything (player data, runs, progression) in Neon Postgres and make the leaderboard live and global.
3. **Login / accounts**: users can sign in so their data is saved.
4. **Shop & customizations**: earn coins by surviving waves and spend them on cosmetic customizations in a shop.
5. **Multiple difficulties**.
6. **Loadouts**: buy different guns with earned coins and try different loadouts.
7. **Character / inventory screen**: a new screen that shows the character in 2D/3D and lets the user equip items they've bought over the course of their gameplay.
8. **Campaign mode** (possible): a fun storyline with boss fights.
9. **Infinite mode**: once the campaign exists, keep the current endless survival as a separate "Infinite" mode.
10. **Fortnite / CoD-style lobby**: before starting infinite co-op, players can invite friends to their lobby, customize their character, and see their friends list. The host sets the difficulty before loading into the game.
