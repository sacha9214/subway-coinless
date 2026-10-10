#!/bin/bash
# Build the web version into DIR (default ../../site): the page, the game
# as WebAssembly, and the textures and sounds it fetches.
# Needs: rustup's wasm32-unknown-unknown target, wasm-bindgen-cli (the
# version of wasm-bindgen in Cargo.lock), wasm-opt (binaryen).
set -euo pipefail
cd "$(dirname "$0")/.."
out="${1:-../../site}"
mkdir -p "$out"
python3 tools/web_pack.py
cargo build --release --target wasm32-unknown-unknown --bin ss_port
wasm-bindgen --target web --no-typescript --out-dir "$out" --out-name game target/wasm32-unknown-unknown/release/ss_port.wasm
wasm-opt -Oz --strip-debug --strip-producers --enable-bulk-memory --enable-bulk-memory-opt --enable-nontrapping-float-to-int --enable-sign-ext --enable-mutable-globals --enable-reference-types --enable-multivalue --enable-extended-const -o "$out/game_bg.opt.wasm" "$out/game_bg.wasm"
mv "$out/game_bg.opt.wasm" "$out/game_bg.wasm"
python3 tools/web_pack.py --site "$out"
touch "$out/.nojekyll"
