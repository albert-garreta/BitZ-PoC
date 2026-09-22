#!/usr/bin/env python3
"""Fetch the Limber checkout pinned in Cargo.toml for the matched MultiSwap comparison."""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile
import tomllib

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_DESTINATION = ROOT / ".tools/limber"


def limber_dependency(bitz_root: Path) -> dict[str, object]:
    with (bitz_root / "Cargo.toml").open("rb") as manifest:
        return tomllib.load(manifest)["dependencies"]["limber"]


def _git(directory: Path, *args: str) -> str:
    return subprocess.check_output(["git", "-C", str(directory), *args], text=True).strip()


def ensure_limber(destination: Path, bitz_root: Path = ROOT) -> str:
    """Return the revision of a Limber checkout at the rev Cargo.toml pins, fetching it if absent.

    An existing checkout is never modified: a different revision is an error.
    """
    pin = limber_dependency(bitz_root)
    url, revision = str(pin["git"]), str(pin["rev"])
    if not destination.exists() or not any(destination.iterdir()):
        destination.mkdir(parents=True, exist_ok=True)
        _git(destination, "init", "--quiet")
        _git(destination, "fetch", "--quiet", "--depth", "1", url, revision)
        _git(destination, "checkout", "--quiet", "--detach", "FETCH_HEAD")
    elif not (destination / ".git").exists():
        raise ValueError(f"{destination} exists and is not a Git checkout")
    actual = _git(destination, "rev-parse", "HEAD")
    if actual != revision:
        raise ValueError(f"Limber checkout {destination} is at {actual[:12]}, "
                         f"but Cargo.toml pins {revision[:12]}")
    return revision


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
    args = parser.parse_args()
    destination = args.destination.resolve()
    try:
        revision = ensure_limber(destination)
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        parser.error(str(error))
    path = destination / "benches/multiswap_modp.rs"
    source = path.read_text()
    expected = ["bitz/multiswap/circuit-digest/v1", "bitz/multiswap/integer-assignment/v1",
                "bitz-limber/multiswap-statement/v2"]
    if not all(domain in source for domain in expected):
        parser.error("Limber checkout lacks the matched benchmark domains")
    domains = {"namespace": "bitz", "path": "benches/multiswap_modp.rs",
               "changed": False, "sha256": hashlib.sha256(path.read_bytes()).hexdigest()}
    print(json.dumps({"path": str(destination), "git_revision": revision,
                      "benchmark_domains": domains}, indent=2))


if __name__ == "__main__":
    main()
