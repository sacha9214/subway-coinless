# No-coin planner

**[Watch the runs in your browser](https://sacha9214.github.io/subway-nocoin/)**

A planner that finds runs of Subway Surfers (this Rust port of the web build) in which the hero never picks up a coin.

The port's simulation is deterministic: a seed and the key presses give the run. So the planner needs no vision and no learning. It searches the simulation itself (`ss_port/src/bin/nocoin.rs`).

## Results

Ten-minute runs with 0 coins, checked by replaying the keys through the game's own driver (`--check`):

| Seed | Coins | Score | Key presses | Stumbles |
|---|---|---|---|---|
| 1 | 0 | 12494 | 429 | 2 |
| 2 | 0 | 12146 | 450 | 7 |
| 3 | 0 | 12650 | 455 | 3 |
| 4 | 0 | 12697 | 428 | 3 |

The keys are in `runs/`. A ten-minute search takes about 5 minutes on a MacBook Air M3.

## Use

```bash
cd ss_port
# search: 10 minutes of run on seed 2
cargo run --release --bin nocoin -- --seed 2 --minutes 10 --out ../runs/seed2-10min.keys
# check a keys file without a window (exit code 1 if a coin or a crash)
cargo run --release --bin nocoin -- --check ../runs/seed2-10min.keys
# watch it in the game
cargo run --release --bin ss_port -- --keys ../runs/seed2-10min.keys
```

Options of the search: `--period N` (a key can be pressed every N frames, default 4), `--cap N` (states followed, default 60), `--allow-boxes` (mystery boxes are avoided by default: their prize can be coins).

## How it works

Every `period` frames each state the run can be in is played with each key (none, left, right, up, down). States that crashed or touched a coin are dropped. Two heroes at the same place in the same pose have the same future, so of two equal states only the one with fewer stumbles and key presses is kept. Only `cap` states are followed, picked to cover every lane, pose and height. When that loses the way (every state kept fails), the search goes back a few seconds and follows 4, 16, then 64 times as many.

## Changes to the port

- `Game` is `Clone` (`game.rs`, `rng.rs`), and the scene's objects are shared between copies until written (`scene.rs`): a copy of the game takes 8 µs instead of 35.
- `ss_port --keys FILE` plays a keys file (`main.rs`, `KeysDriver` in `sim_plugin.rs`); `--mute` plays without sound.
- A web build (`src/vfs.rs`, `web/`, `tools/web_build.sh`, `tools/web_pack.py`).

The port's test suite (trace parity with the original included) passes with these changes.

## The web page

`ss_port/tools/web_build.sh` builds the game as WebAssembly with the four runs inside; the page plays the one picked by `?seed=N`, silent unless `&sound=1`. It needs rustup's `wasm32-unknown-unknown` target, `wasm-bindgen-cli` and `wasm-opt`. File reads go through `src/vfs.rs`, which in the browser serves a pack built into the binary.

## Limits

- The runs are for this port with its seeded random numbers. The original web game draws its level from an unseeded `Math.random()`, so a keys file does not transfer to it.
- Based on [zebmeow/SubwaySurfers](https://github.com/zebmeow/SubwaySurfers) at commit `61315db`: the port, its tests and everything outside the changes listed above are that author's work. Subway Surfers and the game files in `oracle/site` are © SYBO Games / Kiloo; this is an unofficial, non-commercial fan project.
