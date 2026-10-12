# Coinless Surfers

**[Play it in your browser](https://sacha9214.github.io/subway-coinless/)**

Subway Surfers upside down: the first coin you touch ends the run. Go as far as you can.

It is the Rust port of the web game by [zebmeow](https://github.com/zebmeow/SubwaySurfers) with one rule added (`--coinless`, in `ss_port/src/main.rs`) and built for the browser (`ss_port/tools/web_build.sh`, page in `ss_port/web/index.html`). Every visit is a new track; the page keeps your best score on your device and starts silent.

A bot plays the same challenge for over an hour: [subway-nocoin](https://github.com/sacha9214/subway-nocoin).

Unofficial, non-commercial fan project. Subway Surfers and the game files in `oracle/site` are © SYBO Games / Kiloo. The port's README follows.

---

# Subway Surfers Rust / Bevy port

Everything is in this folder:

| Folder | Contents |
|---|---|
| `ss_port/` | the game's source code ([`ss_port/README.md`](ss_port/README.md)) |
| `oracle/site/` | the original web build's files (models, textures, sounds, levels) that the game reads |
| `oracle/traces/`, `oracle/dumps/` | recordings of the original game, used by the tests |

## Play

You need Rust (https://rustup.rs). Then double-click **Play.command**, or:

```bash
cd ss_port
cargo run --release
```

The first build compiles the engine and takes a few minutes; after that it
starts in seconds. Controls: arrows / WASD, Space (hoverboard), Esc (pause),
R (restart).

## Test

```bash
cd ss_port
cargo test --release
```

**Don't delete or move the `oracle` folder.** It holds the original game's
files (`oracle/site`), which the game reads at startup next to `ss_port`.
