# 🧟 Undead Horizon

An infinite 2.5D top-down zombie wave survival game written in **Rust** and compiled to **WebAssembly**.

**▶ Play:** https://cpoooo.github.io/undead-horizon/

Your survivor holds the centre of the screen while endless waves of undead close in from every side of a procedurally generated, infinite world. Kill zombies, collect soul gems, level up, and stack superpowers into an unstoppable build. How far can you get?

## Features

- **Smooth, responsive gameplay**: a fixed 120 Hz simulation with interpolated rendering, snappy acceleration, buffered dash input, hit-stop, screen shake, and knockback.
- **Endless waves**: walkers, runners, brutes, acid spitters, and exploders, with a boss every 5 waves that telegraphs its charge attacks.
- **20 upgrades and superpowers**: Orbiting Blades, Chain Lightning, Frost Nova, Explosive Rounds, Ricochet, Seeker Rounds, Split Shot, Vampirism, Shadow Step, and more. Each level-up offers 3 choices.
- **Ultimate ability**: charges with kills and clears the screen with a nuke.
- **Infinite procedural world**: forests, clearings, rocks, and graveyards, generated from a hash so it costs no memory. Tree canopies and rock tops use perspective lift, which gives real parallax depth.
- **Procedural audio**: every sound effect and the adaptive soundtrack are synthesized live with WebAudio, with stereo panning based on position. There are no audio files.
- **Modes**: **Infinite** (endless and ranked) and a 5-chapter **Campaign** with a story, radio chatter and unique boss fights.
- **4 difficulties**: Rookie, Survivor, Veteran and Nightmare.
- **Shop and Locker**: earn coins by surviving waves, then buy 6 guns, starting perks, outfits, hats and bullet tracers. Build your loadout and see your character in the lobby.
- **Pause and save**: full run stats when paused. Save a run and continue it later.
- **Minimap**: a radar showing terrain, zombies, pickups and which directions hordes are coming from.
- **Leaderboard**: tracks the best wave reached, then level, kills, and time. It is saved in your browser.
- **Mobile support**: a floating virtual joystick, tap to dash, and automatic aiming.

## Controls

| Input | Action |
| --- | --- |
| `WASD` / arrows | Move |
| Mouse | Aim (fires automatically when zombies are near; hold LMB to force fire); includes slight aim assist |
| `Space` / `Shift` | Dash (invulnerable) |
| `Q` / right-click | Ultimate (when charged) |
| `F` | Toggle full auto-aim |
| `1` `2` `3` / click | Pick a level-up |
| `Esc` / `P` | Pause (stats, save game) |
| `N` | Toggle minimap |
| `M` | Toggle music |
| `` ` `` | FPS counter |

## Build & run locally

```bash
rustup target add wasm32-unknown-unknown
cargo install trunk
trunk serve --release   # http://127.0.0.1:8080
```

## Code layout

| File | Purpose |
| --- | --- |
| `src/main.rs` | Browser bootstrap, input (keyboard, mouse, touch), main loop, JS UI bridge |
| `src/game.rs` | Simulation: player, zombies AI, weapons, powers, waves, pickups, spatial hash grid |
| `src/render.rs` | Canvas2D renderer: 2.5D world, lighting, particles, HUD, level-up cards |
| `src/audio.rs` | Procedural WebAudio sound effects and adaptive music |
| `src/content.rs` | Difficulties, guns, cosmetics, perks, boss variants, campaign chapters |
| `src/upgrades.rs` | Upgrade and superpower definitions and weighted offer rolls |
| `src/world.rs` | Infinite deterministic world generation and collision |
| `index.html`, `ui/` | Lobby, shop, locker, story, pause/save, leaderboard (data layer in `ui/app.js`) |
