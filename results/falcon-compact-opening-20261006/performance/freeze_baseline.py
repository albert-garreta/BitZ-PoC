#!/usr/bin/env python3
"""Freeze 9a401286 and certify reuse of the previously qualified V3 binary.

This does not build or execute a benchmark. The retained Cargo overlay only
registers an unrelated example, preserving the exact previously built manifest.
"""
import difflib
import hashlib
import json
from pathlib import Path
import shutil
import subprocess


REVISION = "9a401286c98db041762b97f48cfe4f1a8d85ce7c"
HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
SNAPSHOT = Path("/tmp/falcon-compact-opening-baseline-9a401286")
BINARY = Path("/tmp/falcon-compact-opening-binaries/baseline-9a401286")


def digest(data):
    return hashlib.sha256(data).hexdigest()


def write_json(path, value):
    path.write_text(json.dumps(value, indent=2) + "\n")


def git(*args):
    return subprocess.check_output(["git", *args], cwd=REPO)


def is_build_input(name):
    return name.startswith(("src/", "crates/", "vendor/", "benches/", ".cargo/")) or name in (
        "Cargo.toml", "Cargo.lock", "rust-toolchain.toml", "build.rs")


def main():
    assert not SNAPSHOT.exists() and not BINARY.exists(), "immutable baseline already exists"
    old = json.loads((REPO / "results/falcon-one-source-20261006/performance/candidate-build-v3.json").read_text())
    original_manifest = Path(old["source_manifest"])
    assert digest(original_manifest.read_bytes()) == old["source_manifest_sha256"]
    previous_files = json.loads(original_manifest.read_text())["files"]
    original_root = Path(old["source_root"])
    assert digest(Path(old["binary"]).read_bytes()) == old["sha256"]
    assert subprocess.check_output(["rustc", "-Vv"], text=True).strip() == old["rustc"]
    entries = [entry.split(b"\t", 1) for entry in git("ls-tree", "-rz", REVISION).split(b"\0") if entry]
    names = [name.decode() for metadata, name in entries if metadata.split()[1] == b"blob"]
    # Results are evidence, not compilation inputs; avoid recursively copying
    # the entire previous campaign into every new source snapshot.
    names = [name for name in names if not name.startswith("results/")]
    source = {name: git("show", REVISION + ":" + name) for name in names}
    committed_manifest = source["Cargo.toml"]
    build_manifest = (original_root / "Cargo.toml").read_bytes()
    overlay = b'\n[[example]]\nname = "falcon_size_breakdown"\nrequired-features = ["falcon-hybrid"]\n'
    assert build_manifest.replace(overlay, b"", 1) == committed_manifest
    source["Cargo.toml"] = build_manifest
    # Preserve pre-existing, untracked examples enumerated by the exact Cargo
    # manifest. None are part of the falcon_hybrid benchmark executable.
    extra_examples = []
    for name in previous_files:
        if name.startswith("examples/") and name not in source:
            source[name] = (original_root / name).read_bytes()
            assert digest(source[name]) == previous_files[name]
            extra_examples.append(name)
    build_inputs = {name: digest(data) for name, data in source.items() if is_build_input(name)}
    assert set(build_inputs) == {name for name in previous_files if is_build_input(name)}
    assert all(previous_files[name] == value for name, value in build_inputs.items())
    for name, data in source.items():
        dest = SNAPSHOT / name
        dest.parent.mkdir(parents=True, exist_ok=True)
        dest.write_bytes(data)
    patch = "".join(difflib.unified_diff(committed_manifest.decode().splitlines(True),
                                      build_manifest.decode().splitlines(True),
                                      fromfile="a/Cargo.toml", tofile="b/Cargo.toml"))
    (HERE / "baseline-example-overlay.patch").write_text(patch)
    source_manifest = dict(head=REVISION, working_tree=True,
                           note="Committed source plus existing example registration and untracked examples; no benchmark protocol modifications.",
                           files={name: digest(data) for name, data in sorted(source.items())})
    write_json(SNAPSHOT / "source-manifest.json", source_manifest)
    shutil.copyfile(SNAPSHOT / "source-manifest.json", HERE / "baseline-source-manifest.json")
    BINARY.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(old["binary"], BINARY)
    BINARY.chmod(0o500)
    for path in SNAPSHOT.rglob("*"):
        if path.is_file():
            path.chmod(0o400)
    proof = dict(schema="bitz/falcon-binary-reuse/v1", revision=REVISION,
                 previous_build=old, reused_binary_sha256=digest(BINARY.read_bytes()),
                 matched_build_inputs=build_inputs, matched_build_input_count=len(build_inputs),
                 committed_cargo_sha256=digest(committed_manifest), exact_build_cargo_sha256=digest(build_manifest),
                 cargo_overlay_sha256=digest(patch.encode()), extra_examples=extra_examples,
                 reason="Every benchmark source, workspace/vendor source and embedded asset, manifest, lockfile, and toolchain file is byte-identical to the prior frozen build. Only an unrelated example registration differs from commit 9a401286.")
    write_json(HERE / "baseline-reuse-proof.json", proof)
    build = {key: value for key, value in old.items() if key not in ("working_patch_sha256",)}
    build.update(revision=REVISION, binary=str(BINARY), source_root=str(SNAPSHOT),
                 source_manifest=str(SNAPSHOT / "source-manifest.json"),
                 source_manifest_sha256=digest((SNAPSHOT / "source-manifest.json").read_bytes()),
                 reuse_proof_sha256=digest((HERE / "baseline-reuse-proof.json").read_bytes()),
                 example_overlay_sha256=digest(patch.encode()),
                 rebuild_command="RUSTFLAGS='-C target-cpu=native' CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=/tmp/falcon-compact-opening-baseline-target cargo build --offline --locked --release --features falcon-hybrid --bench falcon_hybrid --message-format=json-render-diagnostics")
    write_json(HERE / "baseline-build.json", build)
    print(json.dumps(dict(binary=build["binary"], sha256=build["sha256"],
                          snapshot_files=len(source), matched_build_inputs=len(build_inputs))))


if __name__ == "__main__":
    main()
