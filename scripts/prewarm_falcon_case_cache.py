#!/usr/bin/env python3
"""Generate and verify the fixed qualification inputs once, outside measurements."""
import argparse
import json
from pathlib import Path
import time

import bench_support as support
from falcon_v2_campaign import environment
from qualify_falcon_one_source import BATCHES, SEEDS, require, validate_rows, verify_build


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--build", type=Path, required=True)
    parser.add_argument("--cache", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    build = json.loads(args.build.read_text())
    verify_build(build)
    args.cache.mkdir(parents=True, exist_ok=True)
    args.output.mkdir(parents=True, exist_ok=True)
    manifest_path = args.output / "cache-reference.json"
    manifest = (json.loads(manifest_path.read_text()) if manifest_path.exists()
                else dict(schema="bitz/falcon-case-cache-reference/v1", build=build, cases={}))
    require(manifest["build"] == build, "prewarm binary changed")
    env = environment(16)
    env["BITZ_FALCON_CASE_CACHE"] = str(args.cache.resolve())
    for batch in BATCHES:
        for seed in SEEDS:
            key = f"{batch}:{seed}"
            cache = args.cache / f"fn-dsa-0.3.0-b{batch}-seed{seed}.bin"
            if key in manifest["cases"]:
                require(support.file_hash(cache) == manifest["cases"][key]["sha256"], "cached fixture changed")
                continue
            require(not cache.exists(), "unrecorded existing fixture; use a fresh cache: " + str(cache))
            output = args.output / f"b{batch}-seed{seed}.jsonl"
            errors = output.with_suffix(".stderr")
            command = [build["binary"], "--protocol", "shared-prime", "--security", "100",
                       "--batch", str(batch), "--seed", str(seed), "--threads", "16",
                       "--warmup", "0", "--iterations", "1"]
            print(f"PREWARM batch={batch} seed={seed}", flush=True)
            started = time.monotonic()
            with output.open("w") as out, errors.open("w") as err:
                code, timed_out = support.run_process(command, env=env, stdout=out, stderr=err, timeout=300)
            require(code == 0 and not timed_out, "fixture generation/verification failed")
            rows = [json.loads(line) for line in output.read_text().splitlines()]
            result = validate_rows(rows, build, "shared-prime", 100, batch, 16, seed, 0, 1, False)
            manifest["cases"][key] = dict(sha256=support.file_hash(cache),
                                          input_digest=result["header"]["input_digest"],
                                          bytes=cache.stat().st_size, elapsed_seconds=time.monotonic() - started,
                                          generated_by=output.name, proof_verified=True)
            support.write_json(manifest_path, manifest)
    verify_build(build)
    require(len(manifest["cases"]) == len(BATCHES) * len(SEEDS), "incomplete cache")
    manifest["complete"] = True
    support.write_json(manifest_path, manifest)


if __name__ == "__main__":
    main()
