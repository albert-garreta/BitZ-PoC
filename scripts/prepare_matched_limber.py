#!/usr/bin/env python3
"""Check out the published Limber revision pinned in BitZ's Cargo.toml."""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import tomllib
import tempfile

DEFAULT_DESTINATION = Path("/tmp/limber-matched114")


def limber_dependency(bitz_root: Path) -> dict[str, object]:
    with (bitz_root / "Cargo.toml").open("rb") as manifest:
        return tomllib.load(manifest)["dependencies"]["limber"]


def migrate_multiswap_domains(limber_root: Path) -> dict[str, object]:
    """Align the dependency benchmark's three digest domains with BitZ."""
    path = limber_root / "benches/multiswap_modp.rs"
    original = path.read_bytes()
    source = original.decode()
    domains = (
        ("f2z/multiswap/circuit-digest/v1", "bitz/multiswap/circuit-digest/v1"),
        ("f2z/multiswap/integer-assignment/v1", "bitz/multiswap/integer-assignment/v1"),
        ("f2z-limber/multiswap-statement/v2", "bitz-limber/multiswap-statement/v2"),
    )
    for previous, current in domains:
        if previous not in source and current not in source:
            raise ValueError(f"Limber benchmark lacks matched digest domain {current}")
        source = source.replace(previous, current)
    encoded = source.encode()
    if encoded != original:
        with tempfile.NamedTemporaryFile(dir=path.parent, prefix=".bitz-domains-", delete=False) as stream:
            temporary = Path(stream.name)
            stream.write(encoded)
        try:
            temporary.chmod(path.stat().st_mode)
            temporary.replace(path)
        finally:
            temporary.unlink(missing_ok=True)
    return {"namespace": "bitz", "path": "benches/multiswap_modp.rs",
            "changed": encoded != original,
            "input_sha256": hashlib.sha256(original).hexdigest(),
            "sha256": hashlib.sha256(encoded).hexdigest()}


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
    domains = migrate_multiswap_domains(destination)
    print(json.dumps({"path": str(destination), "source": source, "git_revision": revision,
                      "benchmark_domains": domains}, indent=2))


if __name__ == "__main__":
    main()
