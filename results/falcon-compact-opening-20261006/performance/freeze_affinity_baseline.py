#!/usr/bin/env python3
"""Apply only the shared benchmark-affinity overlay to the frozen baseline."""
import argparse
import difflib
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
OVERLAY_FILES = ("benches/falcon_hybrid.rs", "benches/common/falcon_affinity.rs")


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def write_json(path, value):
    path.write_text(json.dumps(value, indent=2) + "\n")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("phase", choices=("snapshot", "finalize"))
    parser.add_argument("--version", default="v3")
    args = parser.parse_args()
    assert re.fullmatch(r"v[1-9][0-9]*", args.version)
    snapshot = Path("/tmp/falcon-compact-opening-baseline-" + args.version)
    binary = Path("/tmp/falcon-compact-opening-binaries/baseline-" + args.version)
    overlay_stem = "benchmark-affinity-overlay" + ("" if args.version == "v3" else "-" + args.version)
    original = json.loads((HERE / "baseline-build.json").read_text())
    original_root = Path(original["source_root"])
    original_source = json.loads(Path(original["source_manifest"]).read_text())
    manifest_path = snapshot / "source-manifest.json"
    if args.phase == "snapshot":
        assert not snapshot.exists(), "baseline snapshot already exists"
        files = dict(original_source["files"])
        for name, value in files.items():
            assert digest(original_root / name) == value
            destination = snapshot / name
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(original_root / name, destination)
        patch = ""
        for name in OVERLAY_FILES:
            before = (original_root / name).read_text() if name in files else ""
            after = (REPO / name).read_text()
            patch += "".join(difflib.unified_diff(before.splitlines(True), after.splitlines(True),
                                               fromfile="a/" + name if before else "/dev/null", tofile="b/" + name))
            destination = snapshot / name
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.write_text(after)
            files[name] = digest(destination)
        assert all(files[name] == value for name, value in original_source["files"].items() if name not in OVERLAY_FILES)
        (HERE / (overlay_stem + ".patch")).write_text(patch)
        overlay = dict(files={name: files[name] for name in OVERLAY_FILES},
                       patch_sha256=digest(HERE / (overlay_stem + ".patch")),
                       unchanged_original_files=len(original_source["files"]) - 1,
                       original_source_manifest_sha256=original["source_manifest_sha256"],
                       production_changed=False)
        write_json(HERE / (overlay_stem + ".json"), overlay)
        manifest = dict(head=original["revision"], working_tree=True,
                        note="Original frozen baseline plus only the two benchmark-affinity files; all protocol sources unchanged.",
                        benchmark_affinity_overlay=overlay, files=dict(sorted(files.items())))
        write_json(manifest_path, manifest)
        shutil.copyfile(manifest_path, HERE / ("baseline-source-manifest-" + args.version + ".json"))
        for path in snapshot.rglob("*"):
            if path.is_file():
                path.chmod(0o400)
        print(json.dumps(dict(snapshot=str(snapshot), source_files=len(files),
                              overlay_files=list(OVERLAY_FILES), unchanged_production=True)))
        return
    assert not binary.exists(), "baseline binary already frozen"
    log = HERE / ("baseline-build-" + args.version + ".jsonl")
    rows = [json.loads(line) for line in log.read_text().splitlines() if line.strip()]
    assert rows[-1] == dict(reason="build-finished", success=True)
    artifacts = [row for row in rows if row.get("reason") == "compiler-artifact"
                 and row["target"]["name"] == "falcon_hybrid" and row.get("executable")]
    assert len(artifacts) == 1
    artifact = artifacts[0]
    assert artifact["profile"]["opt_level"] == "3" and not artifact["profile"]["debug_assertions"]
    source = json.loads(manifest_path.read_text())
    assert all(digest(snapshot / name) == value for name, value in source["files"].items())
    assert all(digest(REPO / name) == source["files"][name] for name in OVERLAY_FILES)
    assert subprocess.check_output(["rustc", "-Vv"], text=True).strip() == original["rustc"]
    shutil.copyfile(artifact["executable"], binary)
    binary.chmod(0o500)
    build = {key: value for key, value in original.items()
             if key not in ("reuse_proof_sha256", "rebuild_command")}
    build.update(binary=str(binary), sha256=digest(binary), source_root=str(snapshot),
                 source_manifest=str(manifest_path), source_manifest_sha256=digest(manifest_path),
                 benchmark_affinity_overlay=source["benchmark_affinity_overlay"],
                 build_log_sha256=digest(log), compiler_artifact=artifact,
                 build_command="RUSTFLAGS='-C target-cpu=native' CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=/tmp/falcon-compact-opening-baseline-target cargo build --offline --locked --release --features falcon-hybrid --bench falcon_hybrid --message-format=json-render-diagnostics")
    write_json(HERE / ("baseline-build-" + args.version + ".json"), build)
    print(json.dumps(dict(binary=str(binary), sha256=build["sha256"], source_files=len(source["files"]))))


if __name__ == "__main__":
    main()
