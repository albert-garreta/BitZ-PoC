#!/usr/bin/env python3
"""Check out the published Limber revision pinned in F2Z's Cargo.toml."""
from __future__ import annotations

import argparse
import json
from pathlib import Path
import subprocess
import tomllib

DEFAULT_DESTINATION = Path("/tmp/limber-matched114")


def limber_dependency(f2z_root: Path) -> dict[str, object]:
    with (f2z_root / "Cargo.toml").open("rb") as manifest:
        return tomllib.load(manifest)["dependencies"]["limber"]


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("destination", type=Path, nargs="?", default=DEFAULT_DESTINATION)
    parser.add_argument("--source", help="Git URL or existing local clone (default: Cargo.toml dependency)")
    args = parser.parse_args()
    destination = args.destination.resolve()
    if destination.exists():
        parser.error("destination already exists; choose an unused checkout path")
    dependency = limber_dependency(Path(__file__).resolve().parents[1])
    source = args.source or dependency["git"]
    subprocess.run(["git", "clone", "--no-checkout", source, str(destination)], check=True)
    subprocess.run(["git", "checkout", "--detach", dependency["rev"]], cwd=destination, check=True)
    revision = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=destination, text=True).strip()
    print(json.dumps({"path": str(destination), "source": source, "git_revision": revision}, indent=2))


if __name__ == "__main__":
    main()
