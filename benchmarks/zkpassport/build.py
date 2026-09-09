#!/usr/bin/env python3
"""Build the pinned native Linux x86_64 Noir worker, without Node or C++ compilation."""
import argparse
import ctypes.util
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shutil
import subprocess
import sys
import tarfile

ROOT = Path(__file__).resolve().parent
CACHE = ROOT / ".cache/tools"
RELEASES = {
    "nargo": ("https://github.com/noir-lang/noir/releases/download/v1.0.0-beta.22/nargo-x86_64-unknown-linux-gnu.tar.gz",
              "384c4fc800905b213e26aabd738a96a4a85b1a76ffc27fb19aeb6d33494a787b", "nargo",
              "d57714db2a94f26409b6ff56555438227ecf0b9d185d220fd072b42465dcd686"),
    "barretenberg": ("https://github.com/AztecProtocol/barretenberg/releases/download/v5.0.0/barretenberg-static-amd64-linux.tar.gz",
                    "dcd12d1fdf41459a0c6de5f3d019992b19ddbcdf79fd1e213aa2bf8c1a11eff6", "libbb-external.a",
                    "3e77d3a5205763123b1c88c6efba49531b0f1bf2772ec2dfe66d02bfc474bf65"),
}


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def release(name, offline):
    url, archive_hash, filename, file_hash = RELEASES[name]
    destination = CACHE / name
    file = destination / filename
    if file.exists() and digest(file) == file_hash:
        return file
    archive = CACHE / f"{name}.tar.gz"
    if not archive.exists():
        if offline:
            raise RuntimeError(f"missing cached {name} release")
        subprocess.run(["curl", "-fL", "--retry", "3", "-o", str(archive), url], check=True)
    if digest(archive) != archive_hash:
        raise RuntimeError(f"{name} archive hash mismatch")
    destination.mkdir(exist_ok=True)
    with tarfile.open(archive) as packed:
        packed.extractall(destination, filter="data")
    if digest(file) != file_hash:
        raise RuntimeError(f"{name} extracted file hash mismatch")
    return file


def cxx_directory(override, offline):
    if override:
        return override.resolve(strict=True)
    native = CACHE / "native"
    cached = list(native.glob("usr/lib/llvm-*/lib/libc++.so"))
    if cached:
        return cached[0].parent
    if ctypes.util.find_library("c++"):
        return None
    if offline or not shutil.which("apt-get"):
        raise RuntimeError("libc++ is missing; supply --cxx-lib-dir or install libc++/libc++abi development packages")
    # Extract OS packages privately; no sudo and no system package changes.
    depends = subprocess.check_output(["apt-cache", "depends", "libc++-dev"], text=True)
    version = re.search(r"Depends: libc\+\+-(\d+)-dev", depends)
    if not version:
        raise RuntimeError("cannot resolve libc++ packages; use --cxx-lib-dir")
    version = version.group(1)
    packages = [f"libc++-{version}-dev", f"libc++abi-{version}-dev", f"libc++1-{version}",
                f"libc++abi1-{version}", f"libunwind-{version}", f"libunwind-{version}-dev"]
    native.mkdir(exist_ok=True)
    subprocess.run(["apt-get", "download", *packages], cwd=native, check=True)
    hashes = {}
    for deb in native.glob("*.deb"):
        hashes[deb.name] = digest(deb)
        subprocess.run(["dpkg-deb", "-x", str(deb), str(native)], check=True)
    (native / "packages.json").write_text(json.dumps(hashes, indent=2) + "\n")
    return native / f"usr/lib/llvm-{version}/lib"


def build(offline=False, cxx_lib_dir=None, test=False):
    if platform.system() != "Linux" or platform.machine() != "x86_64":
        raise RuntimeError("the managed native toolchain currently supports Linux x86_64")
    CACHE.mkdir(parents=True, exist_ok=True)
    nargo, bb = release("nargo", offline), release("barretenberg", offline)
    cxx = cxx_directory(cxx_lib_dir, offline)
    flags = os.environ.get("RUSTFLAGS", "-C target-cpu=native")
    if cxx:
        flags += f" -L native={cxx} -C link-arg=-Wl,-rpath,{cxx}"
    env = dict(os.environ, BB_LIB_DIR=str(bb.parent), RUSTFLAGS=flags,
               CARGO_TARGET_DIR=str(ROOT / "target"), CARGO_NET_GIT_FETCH_WITH_CLI="true")
    command = ["cargo", "build", "--release", "--locked", "--manifest-path", str(ROOT / "Cargo.toml")]
    if offline:
        command.append("--offline")
    subprocess.run(command, cwd=ROOT, env=env, check=True)
    if test:
        test_command = ["cargo", "test", "--release", "--locked", "--manifest-path", str(ROOT / "Cargo.toml")]
        if offline:
            test_command.append("--offline")
        subprocess.run(test_command, cwd=ROOT, env=env, check=True, stdout=sys.stderr)
    binary = ROOT / "target/release/zkpassport-bench"
    result = dict(binary=str(binary), binary_sha256=digest(binary), nargo=str(nargo),
                  nargo_sha256=digest(nargo), native_library_sha256=digest(bb), rustflags=flags,
                  cargo_lock_sha256=digest(ROOT / "Cargo.lock"), cxx_library_directory=str(cxx) if cxx else None,
                  cxx_library_hashes={p.name: digest(p) for p in sorted(cxx.glob("*.so*")) if p.is_file()} if cxx else None)
    (ROOT / ".cache/build.json").write_text(json.dumps(result, indent=2) + "\n")
    return result


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--offline", action="store_true")
    parser.add_argument("--cxx-lib-dir", type=Path)
    parser.add_argument("--test", action="store_true", help="Run the shared fixture unit test")
    args = parser.parse_args()
    print(json.dumps(build(args.offline, args.cxx_lib_dir, args.test)))
