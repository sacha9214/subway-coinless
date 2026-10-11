#!/usr/bin/env python3
"""Pack the files the web build reads at start-up into web/pack.bin
(`src/vfs.rs`), and copy what it fetches (textures, sounds) and the page
to a folder to publish.

    python3 tools/web_pack.py            # web/pack.bin
    python3 tools/web_pack.py --site DIR # also DIR/site/... (after the wasm build)
"""
import json
import os
import shutil
import struct
import sys

PORT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
ROOT = os.path.dirname(PORT)
SITE = os.path.join(ROOT, "oracle/site")
FETCHED = (".webp", ".ogg", ".png")


def walk(base):
    for d, _, files in os.walk(base):
        for f in sorted(files):
            yield os.path.join(d, f)


def main():
    entries = []
    # the original game's models and tables (read at start-up)
    for p in walk(SITE):
        if p.endswith((".pk", ".json")):
            entries.append(("/site/" + os.path.relpath(p, SITE), p))
    # the port's own data, fonts and thumbnails, and the planner's runs
    for sub in ("data", "assets/fonts", "assets/ui/thumbs"):
        for p in walk(os.path.join(PORT, sub)):
            entries.append(("/port/" + os.path.relpath(p, PORT), p))
    entries.append(("/port/assets/icon/icon.png", os.path.join(PORT, "assets/icon/icon.png")))
    # one run per seed: the longest (a search under way rewrites its file as it goes)
    runs = {}
    for p in walk(os.path.join(ROOT, "runs")):
        head = open(p).readline().split()
        if p.endswith(".keys") and "true" in head[-1:]:
            seed = os.path.basename(p).split("-")[0]
            frames = int(head[head.index("frames") + 1])
            if frames > runs.get(seed, (0, ""))[0]:
                runs[seed] = (frames, p)
    listing = []
    for seed, (_, p) in sorted(runs.items()):
        entries.append((f"/port/runs/{seed}.keys", p))
        lines = open(p).read().split("\n")
        head = lines[0].split()
        listing.append({"seed": int(seed[4:]), "frames": int(head[head.index("frames") + 1]), "keys": sum(1 for l in lines[1:] if l.strip())})
    # the runs, for the page
    with open(os.path.join(PORT, "web/runs.json"), "w") as f:
        json.dump(listing, f)
    out = os.path.join(PORT, "web/pack.bin")
    with open(out, "wb") as f:
        for name, p in entries:
            data = open(p, "rb").read()
            n = name.replace(os.sep, "/").encode()
            f.write(struct.pack("<H", len(n)) + n + struct.pack("<I", len(data)) + data)
    print(f"{out}: {len(entries)} files, {os.path.getsize(out) / 1e6:.1f} MB")

    if "--site" in sys.argv:
        dest = sys.argv[sys.argv.index("--site") + 1]
        for p in walk(SITE):
            if p.endswith(FETCHED):
                to = os.path.join(dest, "site", os.path.relpath(p, SITE))
                os.makedirs(os.path.dirname(to), exist_ok=True)
                shutil.copyfile(p, to)
        for f in ("index.html", "runs.json"):
            shutil.copyfile(os.path.join(PORT, "web", f), os.path.join(dest, f))
        print(f"{dest}: page, textures and sounds")


if __name__ == "__main__":
    main()
