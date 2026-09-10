#!/usr/bin/env python3
"""Build the pinned, isolated Binius SHA+ECDSA worker and retain its provenance."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parent


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--offline", action="store_true")
    args = parser.parse_args()
    env = dict(os.environ, CARGO_TARGET_DIR=str(ROOT / "target"))
    env.setdefault("RUSTFLAGS", "-C target-cpu=native")
    command = ["cargo", "build", "--release", "--locked"]
    if args.offline:
        command.append("--offline")
    subprocess.run(command, cwd=ROOT, env=env, stdout=sys.stderr, check=True)
    binary = ROOT / "target/release/binius64-sha256-ecdsa"
    info = json.loads(subprocess.check_output([str(binary), "--build-info"], text=True))
    info.update(binary=str(binary), binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest())
    binary.with_suffix(".build.json").write_text(json.dumps(info, indent=2) + "\n")
    print(json.dumps(info))


if __name__ == "__main__":
    main()
