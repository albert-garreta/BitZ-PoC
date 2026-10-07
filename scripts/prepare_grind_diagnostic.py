#!/usr/bin/env python3
"""Build the ignored grinding diagnostics without modifying a tracked lockfile.

Requires Python 3.11+. Run from the clean candidate checkout on the test host:
  python3 scripts/prepare_grind_diagnostic.py --revision FULL_SHA --output .tmp/grind-FULL_SHA

Archives the exact committed vendored sources, seeds their isolated lockfile
with the root lock, then builds offline using native flags and the root release
profile. Every resulting package identity must already exist in the root lock.
This only builds; it does not acquire a benchmark lock or execute a diagnostic.
The output manifest contains binary/commands for a subsequent bench_gate run.
"""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import platform
import re
import subprocess
import shlex
import signal
import sys
import time
import tomllib

import bench_support as support


def require(condition, message):
    if not condition:
        raise ValueError(message)


def verify_revision(root, revision):
    require(re.fullmatch(r"(?:[0-9a-f]{40}|[0-9a-f]{64})", revision), "revision must be a full commit SHA")
    source = support.root_metadata(root)
    require(source["revision"] == revision, "HEAD differs from the requested revision")
    require(not source["git_dirty"], "checkout is dirty; no changes were discarded")


def require_ignored(root, path):
    if path.is_relative_to(root):
        try:
            support.git(root, "check-ignore", "-q", "--", str(path))
        except subprocess.CalledProcessError as error:
            raise ValueError(f"campaign artifact path must be Git-ignored: {path}") from error


def compiler_version(root):
    return subprocess.check_output(["rustc", "-Vv"], cwd=root, text=True)


def campaign_environment(target_dir):
    env = support.clean_environment(os.environ)
    # Encoded flags override RUSTFLAGS in Cargo. Keep one explicit build policy.
    env.pop("CARGO_ENCODED_RUSTFLAGS", None)
    env.pop("RUST_LOG", None)
    env.update(RUSTFLAGS="-C target-cpu=native", CARGO_TARGET_DIR=str(target_dir))
    return env


def execute(command, *, root, env, output, errors=None, timeout):
    print(f"Running {shlex.join(map(str, command))}; log: {output}", flush=True)
    started = time.monotonic()
    with output.open("w") as stdout:
        if errors is None:
            code, timed_out = support.run_process(command, cwd=root, env=env, stdout=stdout,
                                                 stderr=subprocess.STDOUT, timeout=timeout)
        else:
            with errors.open("w") as stderr:
                code, timed_out = support.run_process(command, cwd=root, env=env, stdout=stdout,
                                                     stderr=stderr, timeout=timeout)
    if timed_out:
        raise TimeoutError(f"command timed out after {timeout}s; inspect {output}")
    if code:
        raise RuntimeError(f"command exited {code}; inspect {output}" + (f" and {errors}" if errors else ""))
    return time.monotonic() - started



def check_packages(root_lock, isolated_lock):
    def identities(path):
        lock = tomllib.loads(path.read_text())
        return {(item["name"], item["version"], item.get("source"), item.get("checksum")) for item in lock["package"]}
    added = identities(isolated_lock) - identities(root_lock)
    require(not added, "isolated diagnostic resolved packages absent from root Cargo.lock: " + repr(sorted(added, key=repr)))


def prepare(args):
    root = args.repo.resolve()
    verify_revision(root, args.revision)
    output = (root / args.output).resolve()
    require(not output.exists(), f"output directory already exists: {output}")
    require_ignored(root, output)
    target = (root / "target" / ("falcon-grind-" + args.revision[:12])).resolve()
    require_ignored(root, target)
    env = campaign_environment(target)
    # Root release profile: match the actual Falcon binary's code generation.
    env.update(CARGO_PROFILE_RELEASE_LTO="true", CARGO_PROFILE_RELEASE_CODEGEN_UNITS="1")
    manifest = dict(schema="bitz/grind-diagnostic-build/v1", revision=args.revision,
                    root_lock_sha256=support.file_hash(root / "Cargo.lock"), rustc=compiler_version(root),
                    host=platform.node(), architecture=platform.machine(), cpu_model=support.cpu_name(),
                    affinity=sorted(os.sched_getaffinity(0)) if hasattr(os, "sched_getaffinity") else None,
                    rustflags=env["RUSTFLAGS"], profile="release", lto=True, codegen_units=1,
                    started_at=time.time(), status="preparing")
    output.mkdir(parents=True, exist_ok=False)
    path = output / "manifest.json"
    support.write_json(path, manifest)
    try:
        archive = output / "source.tar"
        with archive.open("wb") as stream:
            code, timed_out = support.run_process(["git", "archive", "--format=tar", args.revision,
                                                  "Cargo.lock", "rust-toolchain.toml", "vendor/flock-mod", "vendor/field"],
                                                 cwd=root, stdout=stream, timeout=120)
        require(not timed_out and code == 0, "source archive failed")
        source = output / "source"
        source.mkdir()
        execute(["tar", "-xf", str(archive), "-C", str(source)], root=root, env=env,
                output=output / "extract.log", timeout=120)
        workspace = source / "vendor/flock-mod"
        lock = workspace / "Cargo.lock"
        lock.write_bytes((source / "Cargo.lock").read_bytes())
        command = ["cargo", "test", "--offline", "--manifest-path", str(workspace / "Cargo.toml"),
                   "-p", "flock-core", "--lib", "--release", "--no-run", "--message-format=json-render-diagnostics"]
        manifest.update(status="building", archive_sha256=support.file_hash(archive), build_command=command)
        support.write_json(path, manifest)
        execute(command, root=root, env=env, output=output / "build.jsonl",
                errors=output / "build.stderr.log", timeout=args.build_timeout)
        check_packages(source / "Cargo.lock", lock)
        executables = support.cargo_executables((output / "build.jsonl").read_text())
        require(len(executables) == 1, "expected exactly one grinding test executable")
        binary = Path(next(iter(executables.values())))
        require(binary.is_absolute() and binary.is_file(), "test executable unavailable")
        verify_revision(root, args.revision)
        commands = [[str(binary), "--exact", "challenger::tests::" + name,
                     "--ignored", "--nocapture", "--test-threads=1"]
                    for name in ("grind_scheduler_diagnostic", "grind_scheduler_diagnostic_26")]
        manifest.update(status="prepared", finished_at=time.time(), binary=str(binary),
                        binary_sha256=support.file_hash(binary), isolated_lock_sha256=support.file_hash(lock),
                        package_identities_preserved=True, diagnostic_commands=commands)
        support.write_json(path, manifest)
    except BaseException as error:
        manifest.update(status="interrupted" if isinstance(error, (KeyboardInterrupt, InterruptedError)) else "failed",
                        finished_at=time.time(), error=f"{type(error).__name__}: {error}")
        support.write_json(path, manifest)
        raise
    print("GRIND_DIAGNOSTIC_MANIFEST=" + str(path), flush=True)
    for command in commands:
        print("GRIND_DIAGNOSTIC_COMMAND=" + shlex.join(command), flush=True)
    return path


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--repo", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--revision", required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--build-timeout", type=int, default=1800)
    args = parser.parse_args(argv)
    if args.build_timeout <= 0:
        parser.error("build timeout must be positive")
    def interrupted(signum, _frame):
        raise InterruptedError(f"received signal {signum}")
    signal.signal(signal.SIGTERM, interrupted)
    try:
        prepare(args)
    except (Exception, KeyboardInterrupt) as error:
        print(f"Grinding diagnostic preparation failed: {error}", file=sys.stderr)
        return 130 if isinstance(error, (KeyboardInterrupt, InterruptedError)) else 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
