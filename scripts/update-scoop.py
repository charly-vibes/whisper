#!/usr/bin/env python3
"""Create or update a Scoop manifest for turu (whisper-vibes)."""
import json
import os
import sys

manifest_path = sys.argv[1]
version = sys.argv[2]
tag = sys.argv[3]
checksums_path = sys.argv[4]

sha_win = None
with open(checksums_path) as f:
    for line in f:
        parts = line.split()
        if len(parts) == 2 and "windows_amd64" in parts[1]:
            sha_win = parts[0]
            break

if sha_win is None:
    print("ERROR: windows_amd64 checksum not found", file=sys.stderr)
    sys.exit(1)

url = f"https://github.com/charly-vibes/whisper/releases/download/{tag}/turu_{version}_windows_amd64.zip"

manifest = {
    "version": version,
    "description": "Deterministic knowledge workspace management for AI agents",
    "homepage": "https://github.com/charly-vibes/whisper",
    "license": "MIT",
    "url": url,
    "hash": f"sha256:{sha_win}",
    "bin": ["turu.exe", "turututu.exe", "whisper.exe"],
    "checkver": {
        "github": "https://github.com/charly-vibes/whisper"
    },
    "autoupdate": {
        "url": "https://github.com/charly-vibes/whisper/releases/download/v$version/turu_$version_windows_amd64.zip"
    }
}

os.makedirs(os.path.dirname(manifest_path), exist_ok=True)
with open(manifest_path, "w") as f:
    json.dump(manifest, f, indent=4)
    f.write("\n")

print(f"Wrote {manifest_path} (version {version})")
print(f"  url: {url}")
print(f"  sha256: {sha_win[:16]}...")
