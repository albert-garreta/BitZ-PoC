#!/usr/bin/env python3
"""Create an isolated Limber revision with the matched-comparison patch."""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import subprocess

BASE_REVISION = "861f10a6a4d705d92a9faf13a8f860d8ba057ca0"
UPSTREAM = "https://github.com/wu-s-john/limber-impl.git"


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("destination", type=Path)
    parser.add_argument("--source", default=UPSTREAM, help="Git URL or existing local clone")
    args = parser.parse_args()
    destination = args.destination.resolve()
    if destination.exists():
        parser.error("destination already exists; choose an unused checkout path")
    patch = Path(__file__).resolve().parents[1] / "patches/limber-multiswap-112.patch"
    subprocess.run(["git", "clone", "--no-checkout", args.source, str(destination)], check=True)
    subprocess.run(["git", "checkout", "-b", "codex/multiswap-112", BASE_REVISION], cwd=destination, check=True)
    subprocess.run(["git", "apply", "--check", str(patch)], cwd=destination, check=True)
    subprocess.run(["git", "apply", str(patch)], cwd=destination, check=True)
    patch_sha256 = hashlib.sha256(patch.read_bytes()).hexdigest()
    subprocess.run(["git", "add", "--all"], cwd=destination, check=True)
    subprocess.run([
        "git", "-c", "user.name=Codex", "-c", "user.email=codex@openai.com", "commit",
        "-m", "Match MultiSwap batches and integer commitment security targets",
        "-m", f"Base: {BASE_REVISION}\nPatch-SHA256: {patch_sha256}",
    ], cwd=destination, check=True)
    revision = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=destination, text=True).strip()
    print(json.dumps({"path": str(destination), "base_revision": BASE_REVISION,
                      "patch_sha256": patch_sha256, "git_revision": revision}, indent=2))


if __name__ == "__main__":
    main()
