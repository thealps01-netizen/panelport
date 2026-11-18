#!/usr/bin/env python3
"""Writes THIRD_PARTY_LICENSES.txt for the Windows build.

1. cargo-about renders every license (with the copyright lines found in each crate's own
   license files) for the crates compiled into panelport.exe on x86_64-pc-windows-msvc.
2. NOTICE files shipped by those crates are appended (Apache-2.0 section 4(d)).

Usage: python3 scripts/gen-licenses.py   (needs cargo-about on PATH)
"""
import json
import pathlib
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
MANIFEST = ROOT / "src-tauri" / "Cargo.toml"
OUT = ROOT / "THIRD_PARTY_LICENSES.txt"
TARGET = "x86_64-pc-windows-msvc"


def normal_dependencies():
    meta = json.loads(
        subprocess.check_output(
            ["cargo", "metadata", "--format-version", "1", "--locked",
             "--filter-platform", TARGET, "--manifest-path", str(MANIFEST)]
        )
    )
    packages = {p["id"]: p for p in meta["packages"]}
    nodes = {n["id"]: n for n in meta["resolve"]["nodes"]}
    root = meta["resolve"]["root"]
    seen, stack = set(), [root]
    while stack:
        nid = stack.pop()
        if nid in seen:
            continue
        seen.add(nid)
        for dep in nodes[nid]["deps"]:
            if any(k["kind"] is None for k in dep["dep_kinds"]):
                stack.append(dep["pkg"])
    seen.discard(root)
    return [packages[i] for i in sorted(seen, key=lambda i: (packages[i]["name"], packages[i]["version"]))]


def main():
    subprocess.check_call(["cargo", "fetch", "--locked", "--manifest-path", str(MANIFEST)])
    subprocess.check_call(
        ["cargo", "about", "generate", "--locked", "--offline", "--fail", "--manifest-path", str(MANIFEST),
         "--config", str(ROOT / "about.toml"), str(ROOT / "about.hbs"), "-o", str(OUT)]
    )
    notices = []
    for pkg in normal_dependencies():
        folder = pathlib.Path(pkg["manifest_path"]).parent
        for f in sorted(folder.glob("NOTICE*")):
            if f.is_file():
                notices.append((pkg, f.read_text(encoding="utf-8", errors="replace")))
    with OUT.open("a", encoding="utf-8", newline="\n") as fh:
        fh.write("\nNOTICE FILES\n============\n\n")
        if not notices:
            fh.write("None of the bundled crates ships a NOTICE file.\n")
        for pkg, text in notices:
            fh.write(f"--- {pkg['name']} {pkg['version']} ---\n{text.rstrip()}\n\n")
    size = OUT.stat().st_size
    print(f"wrote {OUT.name}: {size} bytes, {len(notices)} NOTICE file(s)")
    if size < 50_000:
        sys.exit("THIRD_PARTY_LICENSES.txt looks too small; check cargo-about output")


if __name__ == "__main__":
    main()
