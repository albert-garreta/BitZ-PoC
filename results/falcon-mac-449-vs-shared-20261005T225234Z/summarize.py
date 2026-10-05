#!/usr/bin/env python3
"""Validate and summarize this Mac run without changing any raw artifacts.

Default requires all 32 matrix and 12 confirmation processes. --partial
validates available completed records and clearly marks missing work. Outputs:
summary.json, summary.csv and REPORT.txt, atomically replaced after validation.

Checks the complete REPORTED security ledger, its numerical totals, and
retained-term nonweakening across protocols. This is not an independent audit
of the protocol's security derivation. Timing results remain diagnostic, with
no acceptance CI or nonzero performance tolerance. Float closeness below is
only for checking arithmetic/printed numerical consistency.
"""
import argparse
from collections import defaultdict
import csv
import hashlib
import io
import json
import math
from pathlib import Path
import re
import statistics
import sys

BASELINE = "4491309f5d1e45d506410803e3b9ba488047991f"
CANDIDATE = "df96f014de291df4046464afeb6d8f09a9888a2c"
RUNTIME = "e370e3b35da692d86e814c687e597c3a1d0a6bea"
SCHEMA = "bitz/falcon-hybrid/v3"
PAYLOAD = "canonical stored payload; excludes Falcon framing and public statement"
PROTOCOL = {"baseline": "bitz/falcon1024-ct/hybrid/native-ring/non-zk/v4",
            "candidate": "bitz/falcon1024-ct/hybrid/shared-prime/non-zk/v1"}
TIMINGS = ("witness_commit_ms", "proof_prove_ms", "total_prover_ms", "proof_verify_ms", "native_verify_ms", "end_to_end_ms")
COMMON_TERMS = {
    "prime sampling", "per-signature norm identity", "norm sumchecks", "HashToPoint initial row point",
    "HashToPoint product sumcheck", "ordered compaction fingerprints", "ordered compaction forest sumchecks",
    "ordered compaction forest claim reductions", "compaction leaf instance batching", "compaction leaf sumcheck",
    "linear constraints and terminal batching", "prime source sumcheck", "batched integer-to-binary forest",
    "binary Keccak PIOP", "SHAKE wiring and binary claim batching", "joint binary sumcheck",
    "ring switch and support padding", "shared Ligerito",
}
RING_TERMS = {"baseline": {"native ideal batching and projection", "native coordinate carry batching"},
              "candidate": {"shared ring outer, endpoint batch and inner", "integer polynomial projection"}}
VARIABLE_HEADER = {"input_setup_ms", "prepare_ms"}
PROTOCOL_HEADER = VARIABLE_HEADER | {"protocol", "arithmetic_live_bits_per_signature", "security_terms", "algebraic_security_bits"}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def positive(value):
    return type(value) in (int, float) and math.isfinite(value) and value > 0


def close(a, b):
    return type(a) in (int, float) and math.isfinite(a) and math.isclose(a, b, rel_tol=1e-12, abs_tol=0)


def sha256(path):
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def read_json(path):
    return json.loads(path.read_text())


def expected_runs():
    expected = {}
    index = 0
    for security in (100, 128):
        for threads in (1, 8):
            for batch in (1, 3, 32, 1024):
                order = "AB" if index % 2 == 0 else "BA"
                for label in ("baseline", "candidate"):
                    name = f"matrix-s{security}-b{batch}-t{threads}-seed42-{order}-{label}"
                    expected[name] = dict(name=name, series="matrix", security=security, batch=batch, threads=threads, seed=42, order=order, label=label)
                index += 1
    for security in (100, 128):
        for seed in (42, 43, 44):
            order = "AB" if seed % 2 == 0 else "BA"
            for label in ("baseline", "candidate"):
                name = f"confirm-s{security}-b1024-t8-seed{seed}-{order}-{label}"
                expected[name] = dict(name=name, series="confirm", security=security, batch=1024, threads=8, seed=seed, order=order, label=label)
    return expected


def validate_manifest(directory, meta, present, partial):
    require(meta["baseline"]["revision"] == BASELINE and meta["candidate"]["revision"] == CANDIDATE and
            meta["candidate"]["runtime_revision"] == RUNTIME, "unexpected revision provenance")
    require(meta["rustflags"] == "-C target-cpu=native" and meta["features"] == "falcon-hybrid" and meta["profile"] == "release", "build policy differs")
    require(meta["securities"] == [100, 128] and meta["threads"] == [1, 8] and meta["batches"] == [1, 3, 32, 1024]
            and meta["warmups"] == 1 and meta["iterations"] == 5 and meta["seed"] == 42, "unexpected run matrix")
    require(meta["rust"]["returncode"] == 0 and "host: aarch64-apple-darwin" in meta["rust"]["stdout"] and
            meta["rust"]["stdout"].startswith("rustc 1.98.1 "), "missing Mac/compiler provenance")
    require(meta["os"]["returncode"] == 0 and "macOS" in meta["os"]["stdout"], "missing macOS provenance")
    target_dirs = [Path(meta[label]["target_dir"]).resolve() for label in ("baseline", "candidate")]
    require(target_dirs[0] != target_dirs[1], "baseline/candidate must have separate Cargo target directories")
    evidence = {}
    for label in ("baseline", "candidate"):
        item = meta[label]
        if not all(key in item for key in ("binary", "sha256", "build_exit", "build_command", "help")):
            require(partial and not any(case["label"] == label for case in present.values()), f"{label} build incomplete")
            evidence[label] = {"status": "pending"}
            continue
        require(item["build_exit"] == 0, f"{label} build failed")
        binary = directory / f"falcon_hybrid_{label}_{item['revision'][:12]}"
        require(Path(item["binary"]).resolve() == binary.resolve(), f"{label} saved executable path differs")
        require(re.fullmatch(r"[0-9a-f]{64}", item["sha256"]) and sha256(binary) == item["sha256"], f"{label} executable hash differs")
        command = ["cargo", "bench", "--offline", "--locked", "--profile", "release", "--features", "falcon-hybrid", "--bench", "falcon_hybrid", "--no-run", "--message-format=json-render-diagnostics"]
        require(item["build_command"] == command, f"{label} build command differs")
        require(isinstance(item["help"], str) and "falcon_hybrid --security 100|128" in item["help"] and
                ("--protocol" in item["help"]) == (label == "candidate"), f"{label} saved help identifies wrong benchmark")
        if label == "candidate":
            require("native|shared-prime" in item["help"] and item["sha256"] != meta["baseline"]["sha256"],
                    "candidate is a reused baseline executable")
        build_rows = [json.loads(line) for line in (directory / f"{label}.build.jsonl").read_text().splitlines() if line.strip()]
        executables = [row["executable"] for row in build_rows if row.get("reason") == "compiler-artifact" and
                       row.get("target", {}).get("name") == "falcon_hybrid" and row.get("executable")]
        require(len(executables) == 1 and Path(executables[0]).resolve().is_relative_to(Path(item["target_dir"]).resolve()),
                f"{label} Cargo executable came from wrong target directory")
        evidence[label] = {"status": "validated", "binary_sha256": item["sha256"],
                           "target_dir": item["target_dir"], "recorded_help": item["help"],
                           "build_log_sha256": sha256(directory / f"{label}.build.jsonl")}
    return evidence


def security_ledger(header, label):
    terms = header.get("security_terms", [])
    require(len(terms) == len(COMMON_TERMS | RING_TERMS[label]) and
            {term["name"] for term in terms} == COMMON_TERMS | RING_TERMS[label], "missing, duplicate or unexpected security term")
    errors = {}
    for term in terms:
        error = term["error_bound"]
        require(type(error) in (int, float) and math.isfinite(error) and error >= 0, "invalid security error")
        require(term["bits"] is None if error == 0 else close(term["bits"], -math.log2(error)), "security term bits inconsistent")
        errors[term["name"]] = error
    total = sum(errors.values())
    require(0 < total <= 2.0 ** -header["security_target"], "reported security error exceeds target")
    require(close(header["algebraic_security_bits"], -math.log2(total)), "security ledger total inconsistent")
    return total, errors


def stripped(header, excluded):
    return {key: value for key, value in header.items() if key not in excluded}


def validate_run(directory, meta, record, requested):
    name, label = requested["name"], requested["label"]
    require(all(record.get(key) == value for key, value in requested.items()), f"run identity differs: {name}")
    require(record.get("exit") == 0 and positive(record.get("elapsed_seconds")) and positive(record.get("started")), f"failed/invalid run: {name}")
    command = ["/usr/bin/time", "-l", meta[label]["binary"], "--security", str(requested["security"]), "--batch", str(requested["batch"]),
               "--threads", str(requested["threads"]), "--seed", str(requested["seed"]), "--warmup", "1", "--iterations", "5"]
    if label == "candidate":
        command += ["--protocol", "shared-prime"]
    require(record.get("command") == command, f"run command differs: {name}")
    sample_path, stderr_path = directory / (name + ".jsonl"), directory / (name + ".stderr")
    # Parse every nonempty stdout line; never hide unexpected output.
    rows = [json.loads(line) for line in sample_path.read_text().splitlines() if line.strip()]
    require(len(rows) == 8 and all(row.get("schema") == SCHEMA for row in rows), f"wrong process schema/count: {name}")
    header, summary = rows[0], rows[-1]
    trials = rows[1:-1]
    require(header.get("event") == "prepared" and summary.get("event") == "summary" and all(row.get("event") == "trial" for row in trials), f"event sequence differs: {name}")
    expected_header = {"protocol": PROTOCOL[label], "batch": requested["batch"], "capacity": 1 << (requested["batch"] - 1).bit_length(),
        "security_target": requested["security"], "threads": requested["threads"], "input_seed": requested["seed"], "input_count": requested["batch"],
        "warmup_trials": 1, "measured_trials": 5, "build_rustflags": meta["rustflags"], "runtime_rustflags": meta["rustflags"],
        "target_arch": "aarch64", "gf128_kernel": "neon", "integer_bridge": "wfbitz-joint-limbs",
        "input_source": "pornin/rust-fn-dsa", "input_implementation_version": "0.3.0", "input_mode": "original-falcon-1024",
        "input_rng": "rand_chacha 0.3.1 ChaCha20Rng::seed_from_u64", "arithmetic_auxiliary_values_per_signature": 8605,
        "arithmetic_live_bits_per_signature": 100578 if label == "baseline" else 114914,
        "public_inputs": ["public_key", "message", "signature_nonce", "signature_s2"]}
    require(all(header.get(key) == value for key, value in expected_header.items()), f"requested header/protocol/build differs: {name}")
    require(header.get("stage_timings") is False and header.get("native_verify_includes_public_key_decode") is True, "instrumentation/native verification policy differs")
    require(re.fullmatch(r"[0-9a-f]{64}", header.get("input_digest", "")), "invalid input digest")
    features = header.get("compiled_target_features", {})
    require(set(features) == {"pclmulqdq", "sse4.1", "avx2", "aes", "avx512f", "vpclmulqdq", "gfni"} and all(type(value) is bool for value in features.values()), "invalid compiled feature metadata")
    require(header.get("source_bits_per_signature") == [131072, 1048576, 524288 if requested["batch"] == 1 else 262144], "source geometry differs")
    total_error, terms = security_ledger(header, label)
    require([(row.get("trial"), row.get("sample")) for row in trials] == [("warmup", None)] + [("sample", i) for i in range(1, 6)], "wrong warmup/sample ordering")
    samples = trials[1:]
    for row in trials:
        require(row.get("verified") is True, "unverified proof")
        require(all(row.get(key) == header.get(key) for key in ("batch", "capacity", "security_target", "threads", "input_digest", "prepare_ms", "input_setup_ms")), "trial/header mismatch")
        require(all(positive(row.get(key)) for key in TIMINGS), "nonpositive/nonfinite timing")
        require(close(row["total_prover_ms"], row["witness_commit_ms"] + row["proof_prove_ms"]), "prover accounting mismatch")
        require(close(row["end_to_end_ms"], row["total_prover_ms"] + row["proof_verify_ms"]), "end-to-end accounting mismatch")
        require(type(row.get("proof_payload_bytes")) is int and row["proof_payload_bytes"] > 0 and row.get("proof_payload_definition") == PAYLOAD, "invalid payload accounting")
        require(re.fullmatch(r"[0-9a-f]{64}", row.get("proof_debug_digest", "")), "invalid proof digest")
        require(row.get("process_peak_rss_kib") is None, "Mac harness RSS expected null; external time supplies bytes")
        require(isinstance(row.get("roots"), list) and len(row["roots"]) == 3 and all(re.fullmatch(r"[0-9a-f]{64}", root) for root in row["roots"]), "invalid commitment roots")
    identity = {(row["proof_debug_digest"], row["proof_payload_bytes"], tuple(row["roots"])) for row in trials}
    require(len(identity) == 1, "proof/payload/roots changed within process")
    medians = {key: statistics.median(row[key] for row in samples) for key in TIMINGS}
    require(summary.get("samples") == 5 and summary.get("verified") is True and summary.get("process_peak_rss_kib") is None, "invalid process summary")
    require(all(summary.get(key) == header.get(key) for key in ("batch", "capacity", "security_target", "threads", "prepare_ms", "input_setup_ms")), "summary/header mismatch")
    require(all(close(summary.get("median_" + key), value) for key, value in medians.items()), "summary medians inconsistent")
    require(close(summary.get("proof_signatures_per_second"), 1000 * requested["batch"] / medians["proof_prove_ms"]) and
            close(summary.get("total_prover_signatures_per_second"), 1000 * requested["batch"] / medians["total_prover_ms"]), "summary throughput inconsistent")
    require(record.get("prepared") == header and record.get("trials") == trials and record.get("medians") == medians and
            record.get("payload_bytes") == trials[0]["proof_payload_bytes"], "run record differs from raw JSON or is not finalized")
    rss_matches = re.findall(r"^\s*(\d+)\s+maximum resident set size\s*$", stderr_path.read_text(), re.M)
    require(len(rss_matches) == 1 and int(rss_matches[0]) > 0 and record.get("peak_rss_bytes") == int(rss_matches[0]), "Mac external RSS missing/inconsistent")
    proof_digest, payload, roots = next(iter(identity))
    return {**requested, "prepared": header, "security_error": total_error, "security_terms": terms, "medians": medians,
            "payload_bytes": payload, "peak_rss_bytes": int(rss_matches[0]), "proof_debug_digest": proof_digest, "roots": roots,
            "started": record["started"], "elapsed_seconds": record["elapsed_seconds"], "verified_proofs": 6,
            "artifacts_sha256": {"raw_jsonl": sha256(sample_path), "stderr": sha256(stderr_path), "run_json": sha256(directory / (name + ".run.json"))}}


def pair_checks(old, new):
    require(stripped(old["prepared"], PROTOCOL_HEADER) == stripped(new["prepared"], PROTOCOL_HEADER), "paired source/build/kernel inputs differ")
    require(new["security_error"] <= old["security_error"], "candidate reported total security error is weaker")
    require(all(new["security_terms"][name] <= old["security_terms"][name] for name in COMMON_TERMS), "candidate retained security term is weaker")
    require(old["order"] == new["order"], "recorded pair orders differ")
    first, last = (old, new) if old["order"] == "AB" else (new, old)
    require(first["started"] + first["elapsed_seconds"] <= last["started"], "pair execution order overlaps or differs")


def metric_rows(before, candidate):
    values = {}
    for metric in TIMINGS + ("payload_bytes", "peak_rss_bytes"):
        def aggregate(records):
            if metric in TIMINGS:
                return statistics.median(record["medians"][metric] for record in records)
            if metric == "payload_bytes":
                require(len({record[metric] for record in records}) == 1, "same-input payload varies between processes")
            return max(record[metric] for record in records)
        old, new = aggregate(before), aggregate(candidate)
        values[metric] = {"baseline": old, "candidate": new, "ratio": new / old, "change_percent": 100 * (new / old - 1)}
    return values


def summarize(directory, partial=False):
    meta = read_json(directory / "manifest.json")
    expected = expected_runs()
    present = {path.name.removesuffix(".run.json"): path for path in directory.glob("*.run.json")}
    require(not present.keys() - expected.keys(), "unexpected/duplicate run filename")
    missing = sorted(expected.keys() - present.keys())
    require(partial or not missing, f"incomplete run: {len(present)}/44 process records; use --partial for progress")
    builds = validate_manifest(directory, meta, {name: expected[name] for name in present}, partial)
    records = {name: validate_run(directory, meta, read_json(path), expected[name]) for name, path in sorted(present.items())}
    ordered = sorted(records.values(), key=lambda record: record["started"])
    require(all(a["started"] + a["elapsed_seconds"] <= b["started"] for a, b in zip(ordered, ordered[1:])), "benchmark processes overlap")
    stable = {}
    inputs = {}
    compiled_features = None
    paired = defaultdict(dict)
    for record in records.values():
        header = record["prepared"]
        input_key = (record["batch"], record["seed"])
        require(input_key not in inputs or inputs[input_key] == header["input_digest"],
                "same batch/seed input differs across protocols, security levels or thread counts")
        inputs[input_key] = header["input_digest"]
        require(compiled_features is None or compiled_features == header["compiled_target_features"],
                "compiled target features differ across runs")
        compiled_features = header["compiled_target_features"]
        key = (record["label"], record["security"], record["batch"], record["threads"], record["seed"])
        identity = (stripped(record["prepared"], VARIABLE_HEADER), record["proof_debug_digest"], record["payload_bytes"], record["roots"])
        require(key not in stable or stable[key] == identity, "same-protocol/input repeat changed header, proof, payload or roots")
        stable[key] = identity
        pair = (record["series"], record["security"], record["batch"], record["threads"], record["seed"])
        paired[pair][record["label"]] = record
    matrix = []
    complete_pairs = {}
    for key, sides in sorted(paired.items()):
        if set(sides) != {"baseline", "candidate"}:
            continue
        pair_checks(sides["baseline"], sides["candidate"])
        complete_pairs[key] = sides
        if key[0] == "matrix":
            matrix.append({"security": key[1], "batch": key[2], "threads": key[3], "seed": key[4], "order": sides["baseline"]["order"],
                           "metrics": metric_rows([sides["baseline"]], [sides["candidate"]])})
    confirmation = []
    for security in (100, 128):
        seeds = []
        for seed in (42, 43, 44):
            # Seed 42 contributes its matrix and reversed-order confirmation.
            # First reduce each protocol to one median per seed, then give all
            # three seeds equal weight. Never pool their unequal trial counts.
            pairs = [sides for key, sides in complete_pairs.items() if key[1:] == (security, 1024, 8, seed)]
            if pairs:
                old, new = [sides["baseline"] for sides in pairs], [sides["candidate"] for sides in pairs]
                seeds.append({"seed": seed, "processes_per_protocol": len(pairs), "orders": [r["order"] for r in old],
                              "metrics": metric_rows(old, new)})
        complete = len(seeds) == 3 and [s["processes_per_protocol"] for s in seeds] == [2, 1, 1]
        rollup = {metric: {"equal_seed_geomean_ratio": math.exp(statistics.mean(math.log(seed["metrics"][metric]["ratio"]) for seed in seeds)),
                           "seeds_with_observed_increase": sum(seed["metrics"][metric]["ratio"] > 1 for seed in seeds)}
                  for metric in TIMINGS + ("payload_bytes", "peak_rss_bytes")} if complete else None
        confirmation.append({"security": security, "batch": 1024, "threads": 8, "complete": complete, "seeds": seeds, "aggregate": rollup})
    complete = not missing and len(matrix) == 16 and all(item["complete"] for item in confirmation)
    require(partial or complete, "expected 16 matrix cells and complete three-seed confirmation")
    return {"schema": "bitz/falcon-mac-summary/v1", "validation_status": "complete" if complete else "partial",
        "performance_acceptance": "not_established_diagnostic_only", "expected_processes": 44, "validated_processes": len(records),
        "verified_proofs": 6 * len(records), "expected_verified_proofs": 264, "missing_processes": missing,
        "baseline_revision": BASELINE, "candidate_revision": CANDIDATE, "runtime_revision": RUNTIME,
        "manifest_sha256": sha256(directory / "manifest.json"), "summarizer_sha256": sha256(Path(__file__)), "builds": builds,
        "timing_definition": "Process medians of five measured proofs after one warmup; total prover=commit+prove; setup/prepare/native verification/digest work excluded.",
        "rss_definition": "macOS external /usr/bin/time -l maximum resident set size in bytes for the full six-proof process, including setup, warmup, digest work and allocator retention; not a per-proof delta or fresh single-proof RSS.",
        "confirmation_definition": "Use matrix+confirmation for seed42 (two process medians per protocol), confirmation only for seeds43/44 (one each). Per seed, median the process medians separately by protocol, then candidate/baseline. Aggregate is geometric mean of three equally weighted seed ratios. RSS uses per-seed maximum process RSS, not median. Payload is stable per seed. No confidence interval; only seed42 has both execution orders.",
        "security_validation": "Complete reported ledger term sets, nonnegative finite errors, per-term bits, total target bound, reported algebraic bits, and no-weaker retained terms/total are checked. This validates numerical consistency of supplied security claims, not an independent protocol/security audit.",
        "matrix": matrix, "confirmation": confirmation, "processes": list(records.values())}


def outputs(report):
    csv_buffer = io.StringIO()
    fields = ["series", "security", "batch", "threads", "seed", "processes_per_protocol", "metric", "baseline", "candidate", "ratio", "change_percent"]
    writer = csv.DictWriter(csv_buffer, fieldnames=fields)
    writer.writeheader()
    for series, cells in (("matrix", report["matrix"]), ("confirmation_seed", [dict(security=item["security"], batch=1024, threads=8, **seed) for item in report["confirmation"] for seed in item["seeds"]])):
        for cell in cells:
            for metric, values in cell["metrics"].items():
                writer.writerow({"series": series, **{key: cell[key] for key in ("security", "batch", "threads", "seed")},
                                 "processes_per_protocol": cell.get("processes_per_protocol", 1), "metric": metric, **values})
    for item in report["confirmation"]:
        if item["aggregate"]:
            for metric, values in item["aggregate"].items():
                ratio = values["equal_seed_geomean_ratio"]
                writer.writerow(dict(series="confirmation_equal_seed_aggregate", security=item["security"], batch=1024, threads=8,
                                     metric=metric, ratio=ratio, change_percent=100 * (ratio - 1)))
    text = [f"Falcon Mac comparison: {report['validation_status'].upper()}",
            f"Processes {report['validated_processes']}/44; verified proofs {report['verified_proofs']}/264.",
            f"Baseline {BASELINE}; candidate {CANDIDATE} (runtime {RUNTIME}).",
            "Ratios are shared-prime/native; less than 1 is lower. Timing is diagnostic, not a no-regression acceptance result.",
            "Matrix timings are process medians of five samples. Payload is bytes. RSS is whole-process maximum MiB from macOS time; includes all six trials/setup.",
            "", "Matrix: security batch threads | total prover ms baseline -> shared (ratio) | proof ms ratio | verify ms baseline -> shared (ratio) | payload ratio | RSS MiB baseline -> shared (ratio)"]
    for cell in report["matrix"]:
        m = cell["metrics"]
        total, proof, verify, payload, rss = (m[name] for name in ("total_prover_ms", "proof_prove_ms", "proof_verify_ms", "payload_bytes", "peak_rss_bytes"))
        text.append(f"{cell['security']:3} {cell['batch']:4} {cell['threads']:2} | {total['baseline']:.3f} -> {total['candidate']:.3f} ({total['ratio']:.4f}) | {proof['ratio']:.4f} | {verify['baseline']:.3f} -> {verify['candidate']:.3f} ({verify['ratio']:.4f}) | {payload['ratio']:.4f} | {rss['baseline']/2**20:.2f} -> {rss['candidate']/2**20:.2f} ({rss['ratio']:.4f})")
    text.extend(["", "B1024/t8 confirmation: seed42 uses median of two process medians/protocol; seeds43/44 use one each. Aggregate equally weights the three seed ratios. RSS uses each seed's maximum process RSS."])
    for item in report["confirmation"]:
        for seed in item["seeds"]:
            m = seed["metrics"]
            text.append(f"security {item['security']}, seed {seed['seed']}, processes/protocol {seed['processes_per_protocol']}: total {m['total_prover_ms']['baseline']:.3f} -> {m['total_prover_ms']['candidate']:.3f} ms ({m['total_prover_ms']['ratio']:.4f}); verify ratio {m['proof_verify_ms']['ratio']:.4f}; payload ratio {m['payload_bytes']['ratio']:.4f}; RSS ratio {m['peak_rss_bytes']['ratio']:.4f}")
        if item["aggregate"]:
            values = item["aggregate"]
            text.append(f"security {item['security']}, equal-seed aggregate ratios: total {values['total_prover_ms']['equal_seed_geomean_ratio']:.4f}; proof {values['proof_prove_ms']['equal_seed_geomean_ratio']:.4f}; verify {values['proof_verify_ms']['equal_seed_geomean_ratio']:.4f}; payload {values['payload_bytes']['equal_seed_geomean_ratio']:.4f}; RSS {values['peak_rss_bytes']['equal_seed_geomean_ratio']:.4f}.")
    text.extend(["", report["security_validation"], "Only seed42 has both orders; three seeds do not establish the strict performance gate."])
    if report["missing_processes"]:
        text.append(f"INCOMPLETE: {len(report['missing_processes'])} process records missing; no complete aggregate is inferred.")
    return csv_buffer.getvalue(), "\n".join(text) + "\n"


def atomic_write(path, text):
    temporary = path.with_name(path.name + ".tmp")
    temporary.write_text(text)
    temporary.replace(path)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--partial", action="store_true")
    parser.add_argument("--directory", type=Path, default=Path(__file__).resolve().parent)
    args = parser.parse_args(argv)
    report = summarize(args.directory.resolve(), args.partial)
    table, plain = outputs(report)
    atomic_write(args.directory / "summary.json", json.dumps(report, indent=2, allow_nan=False) + "\n")
    atomic_write(args.directory / "summary.csv", table)
    atomic_write(args.directory / "REPORT.txt", plain)
    print(f"Validated {report['validated_processes']}/44 processes, {report['verified_proofs']}/264 proofs: {report['validation_status']}")
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (ValueError, OSError, KeyError, TypeError) as error:
        print(f"Falcon Mac postflight validation failed: {error}", file=sys.stderr)
        sys.exit(1)
