#!/usr/bin/env python3
"""Snapshot candidate sources, then freeze an already completed benchmark build."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def write_json(path, value):
    path.write_text(json.dumps(value, indent=2) + "\n")


def git(*args):
    return subprocess.check_output(["git", *args], cwd=REPO)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("phase", choices=("snapshot", "finalize"))
    parser.add_argument("--version", default="v1")
    parser.add_argument("--baseline-build", type=Path, default=HERE / "baseline-build.json")
    args = parser.parse_args()
    assert re.fullmatch(r"v[1-9][0-9]*", args.version)
    baseline = json.loads(args.baseline_build.read_text())
    snapshot = Path("/tmp/falcon-compact-opening-candidate-" + args.version)
    manifest_path = snapshot / "source-manifest.json"
    if args.phase == "snapshot":
        assert not snapshot.exists(), "snapshot already exists"
        names = set(json.loads((HERE / "baseline-source-manifest.json").read_text())["files"])
        names.update(git("ls-files").decode().splitlines())
        names.update(name for name in git("ls-files", "--others", "--exclude-standard").decode().splitlines()
                     if name.startswith(("src/", "vendor/", "crates/", "benches/", "tests/", "examples/")))
        names = sorted(name for name in names if not name.startswith("results/") and (REPO / name).is_file())
        for name in names:
            destination = snapshot / name
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(REPO / name, destination)
            destination.chmod(0o400)
        patch = git("diff", "--binary", "HEAD")
        (snapshot / "working-tree.patch").write_bytes(patch)
        manifest = dict(head=git("rev-parse", "HEAD").decode().strip(), working_tree=True,
                        files={name: digest(snapshot / name) for name in names})
        write_json(manifest_path, manifest)
        shutil.copyfile(manifest_path, HERE / ("candidate-source-manifest-" + args.version + ".json"))
        shutil.copyfile(snapshot / "working-tree.patch", HERE / ("candidate-working-tree-" + args.version + ".patch"))
        manifest_path.chmod(0o400)
        (snapshot / "working-tree.patch").chmod(0o400)
        print(json.dumps(dict(snapshot=str(snapshot), files=len(names), manifest_sha256=digest(manifest_path))))
        return
    output = HERE / ("candidate-build-" + args.version + ".jsonl")
    rows = [json.loads(line) for line in output.read_text().splitlines() if line.strip()]
    assert rows[-1] == dict(reason="build-finished", success=True), "build has not completed successfully"
    artifacts = [row for row in rows if row.get("reason") == "compiler-artifact"
                 and row["target"]["name"] == "falcon_hybrid" and row.get("executable")]
    assert len(artifacts) == 1
    artifact = artifacts[0]
    assert artifact["profile"]["opt_level"] == "3" and not artifact["profile"]["debug_assertions"]
    source = json.loads(manifest_path.read_text())
    assert all(digest(REPO / name) == value == digest(snapshot / name) for name, value in source["files"].items()), "sources changed during build"
    assert subprocess.check_output(["rustc", "-Vv"], text=True).strip() == baseline["rustc"]
    binary = Path("/tmp/falcon-compact-opening-binaries/candidate-" + args.version)
    assert not binary.exists(), "binary already frozen"
    shutil.copyfile(artifact["executable"], binary)
    binary.chmod(0o500)
    build = {key: value for key, value in baseline.items()
             if key not in ("reuse_proof_sha256", "example_overlay_sha256", "rebuild_command")}
    build.update(revision=source["head"], binary=str(binary), sha256=digest(binary), source_root=str(snapshot),
                 source_manifest=str(manifest_path), source_manifest_sha256=digest(manifest_path),
                 working_patch_sha256=digest(snapshot / "working-tree.patch"),
                 build_log_sha256=digest(output), compiler_artifact=artifact,
                 build_command="RUSTFLAGS='-C target-cpu=native' CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=target/falcon-native cargo build --offline --locked --release --features falcon-hybrid --bench falcon_hybrid --message-format=json-render-diagnostics")
    write_json(HERE / ("candidate-build-" + args.version + ".json"), build)
    write_json(HERE / ("builds-" + args.version + ".json"), dict(binaries=dict(baseline=baseline, candidate=build)))
    print(json.dumps(dict(binary=str(binary), sha256=build["sha256"], source_files=len(source["files"]))))


if __name__ == "__main__":
    main()
