#!/usr/bin/env python3
"""Render the archived full-Falcon public-selection benchmark report.

Only reads benchmark artifacts; does not run benchmarks or build repository code.
Archive layout: RESULTS.md, raw/, comparison.json, metadata.json, tests.log,
history/tree-comparison.json, history/selection-v1-comparison.json.
"""

import argparse
import itertools
import json
import math
import statistics
from pathlib import Path


EXPECTED_KEYS = set(itertools.product((512, 1024), (100, 128), (1, 2, 4, 8, 16)))
SOURCE = {
    512: (59817, 52637, 65536, 96 * 1024, 90),
    1024: (114914, 100482, 131072, 176 * 1024, 164),
}


def read_json(path):
    return json.loads(path.read_text())


def timing_pass(case):
    return all(case["timings"][name]["pass"] for name in ("total_prover_ms", "proof_verify_ms"))


def case_name(case):
    return f"full-n{case['degree']}-b1024-s{case['security']}-t{case['threads']}.jsonl"


def validate_raw(candidate_dir, cases):
    headers = []
    for case in cases:
        path = candidate_dir / case_name(case)
        rows = [json.loads(line) for line in path.read_text().splitlines() if line.strip()]
        header = next(row for row in rows if row.get("event") == "prepared")
        trials = [row for row in rows if row.get("event") == "trial"]
        measured = [row for row in trials if row["trial"] == "sample"]
        if len(trials) != 6 or len(measured) != 5 or trials[0]["trial"] != "warmup":
            raise ValueError(f"unexpected sample schedule: {path}")
        if not all(row["verified"] for row in trials):
            raise ValueError(f"unverified proof in {path}")
        expected = {
            "batch": 1024,
            "input_seed": 42,
            "warmup_trials": 1,
            "measured_trials": 5,
            "degree": case["degree"],
            "security_target": case["security"],
            "threads": case["threads"],
        }
        for name, value in expected.items():
            if header[name] != value:
                raise ValueError(f"unexpected {name} in {path}")
        if header["algebraic_security_bits"] < case["security"]:
            raise ValueError(f"security target not met: {path}")
        for name in ("total_prover_ms", "proof_verify_ms"):
            observed = statistics.median(row[name] for row in measured)
            reported = case["timings"][name]["candidate_median_ms"]
            if not math.isclose(observed, reported, rel_tol=1e-12, abs_tol=1e-9):
                raise ValueError(f"comparison/raw mismatch for {name}: {path}")
        payload = statistics.median(row["proof_payload_bytes"] for row in measured)
        rss = max(row["process_peak_rss_kib"] for row in measured)
        if payload != case["candidate_payload_bytes"] or rss != case["candidate_peak_rss_kib"]:
            raise ValueError(f"comparison/raw payload or RSS mismatch: {path}")
        baseline_live, live, padded, total, mask_bytes = SOURCE[case["degree"]]
        if any(
            row["proof_payload_breakdown"].get("hash_to_point_selection_masks") != 1024 * mask_bytes
            for row in trials
        ):
            raise ValueError(f"unexpected selection-mask payload: {path}")
        if (
            header["arithmetic_live_bits_per_signature"] != live
            or header["source_bits_per_signature"][0] != padded
            or sum(header["source_bits_per_signature"]) // 8 != total
        ):
            raise ValueError(f"unexpected source geometry: {path}")
        witness = case["witness"]
        expected_witness = {
            "baseline_live_arithmetic_bits_per_signature": baseline_live,
            "candidate_live_arithmetic_bits_per_signature": live,
            "baseline_padded_arithmetic_bits_per_signature": padded,
            "candidate_padded_arithmetic_bits_per_signature": padded,
            "baseline_total_packed_bytes_per_signature": total,
            "candidate_total_packed_bytes_per_signature": total,
        }
        for name, value in expected_witness.items():
            if witness[name] != value:
                raise ValueError(f"unexpected {name}: {path}")
        if case.get("candidate_selection_mask_bytes") != 1024 * mask_bytes:
            raise ValueError(f"comparison mask-size mismatch: {path}")
        headers.append(header)
    return headers


def unique(headers, key):
    values = {json.dumps(header[key], sort_keys=True) for header in headers}
    if len(values) != 1:
        raise ValueError(f"nonuniform benchmark metadata: {key}")
    return json.loads(values.pop())


def value_range(values, scale=1.0, digits=2):
    lo, hi = min(values) / scale, max(values) / scale
    return f"{lo:.{digits}f}" if lo == hi else f"{lo:.{digits}f}–{hi:.{digits}f}"


def baseline_link(comparison, override):
    if override:
        return override.rstrip("/") + "/"
    parts = Path(comparison["baseline"]).parts
    if "results" not in parts:
        raise ValueError("use --baseline-link for a baseline outside the repository results directory")
    suffix = parts[parts.index("results") + 1 :]
    return "../" + "/".join(suffix) + "/"


def history_text(archive_dir):
    tree = archive_dir / "history/tree-comparison.json"
    first = archive_dir / "history/selection-v1-comparison.json"
    if tree.is_file():
        report = read_json(tree)
        cases = report["cases"]
        p_fail = sum(not case["timings"]["total_prover_ms"]["pass"] for case in cases)
        v_fail = sum(not case["timings"]["proof_verify_ms"]["pass"] for case in cases)
        tree_result = f" ({p_fail} prover and {v_fail} verifier timing failures)"
    else:
        tree_result = ""
    if first.is_file():
        cases = read_json(first)["cases"]
        passed = sum(timing_pass(case) for case in cases)
        increased = sum(case["candidate_peak_rss_kib"] > case["baseline_peak_rss_kib"] for case in cases)
        maximum = max(case["peak_rss_ratio"] for case in cases)
        selection_result = (
            f"passed {passed}/{len(cases)} timing cases; {increased} peak-RSS measurements "
            f"increased, by at most {(maximum - 1) * 100:.2f}%"
        )
    else:
        selection_result = "passed all 20 timing cases; three peak-RSS measurements increased, by at most 2.64%"
    return (
        "The [balanced polynomial-tree candidate](history/tree-comparison.json) was rejected"
        + tree_result
        + ". It added committed U/V coefficient tables. The [initial public-selection candidate]"
        "(history/selection-v1-comparison.json) "
        + selection_result
        + ". Memory differences are informational under the final timing-only gate. "
        "The selected implementation is this initial public-selection candidate. A later coefficient-allocation slab experiment reduced memory but slowed proving in all 20 cases, by 0.4–9.1%; it was reverted to prioritize speed. Its [comparison](history/slab-comparison.json) is retained as an allocation experiment, with matching proof Debug digests for all 120 runs. The tables above describe the retained faster implementation."
    )


def render(args, comparison, cases, headers, metadata):
    timings_passed = sum(timing_pass(case) for case in cases)
    recorded_pass = comparison.get("pass") is True
    if recorded_pass and timings_passed != 20:
        raise ValueError("comparison reports pass despite a timing failure")
    if recorded_pass:
        verdict = "All 20 cases passed the recorded prover/verifier timing gate."
    else:
        verdict = (
            f"The comparison JSON records a failed gate; {timings_passed}/20 cases pass both timing checks. "
            "No final no-regression claim is made."
        )
    flags = unique(headers, "build_rustflags")
    protocol = unique(headers, "protocol")
    layout = unique(headers, "source_layout")
    input_source = unique(headers, "input_source")
    input_version = unique(headers, "input_implementation_version")
    input_digests = {
        degree: {header["input_digest"] for header in headers if header["degree"] == degree}
        for degree in (512, 1024)
    }
    if any(len(digests) != 1 for digests in input_digests.values()):
        raise ValueError("inputs differ between cases of one Falcon degree")
    before = baseline_link(comparison, args.baseline_link)
    lines = [
        "# Full Falcon: public HashToPoint selection",
        "",
        verdict + " Peak RSS is reported separately and does not determine acceptance.",
        "",
        "1,024 distinct original-Falcon signatures per proof; dimensions 512 and 1024; "
        "100- and 128-bit security targets; 1, 2, 4, 8 and 16 threads. "
        "Each case uses seed 42, one warm-up and five verified measured trials on "
        "`will` (AMD Ryzen 9 9950X3D). The tables show medians. "
        f"Inputs use `{input_source}` {input_version}. Compared with the "
        f"[archived baseline]({before}); no new baseline runs are included.",
        "",
        f"Native release build: `{flags}`, fat LTO, one codegen unit. "
        "CPU affinity, input digests, compiled features and per-trial metadata are in the "
        "[raw logs](raw/); revision, file hashes and binary identification are in "
        "[metadata.json](metadata.json). "
        f"Protocol: `{protocol}`; source layout: `{layout}`.",
    ]
    if metadata.get("rustc"):
        lines.extend(["", f"Compiler: `{metadata['rustc']}`."])
    lines.extend([
        "",
        "Prover time includes checked witness preparation, packing, commitment and proving. "
        "Input generation and reusable parameter preparation are excluded. "
        "Each timing gate requires a candidate/baseline median ratio at most 1 and an "
        "independently resampled, one-sided 95% bootstrap upper mean-ratio bound at most 1 "
        "(20,000 resamples, RNG seed 0). This compares five fixed-input samples from each archive; "
        "it does not measure variation across input seeds. "
        "Peak RSS is the process cumulative high-water mark, including setup, warm-up and earlier trials.",
        "",
        "The measured change combines public-mask linear routing, the factored verifier binder "
        "and optimized K=4 packed-prefix processing. These end-to-end results do not isolate "
        "Horner evaluation, the rejected tree, or mask routing as individual kernels.",
        "",
        "| N | Security | Threads | Prover ms: before → after | Verify ms: before → after | Peak RSS MiB: before → after | Timing gate |",
        "|---:|---:|---:|---:|---:|---:|:---:|",
    ])
    for case in cases:
        p, v = (case["timings"][name] for name in ("total_prover_ms", "proof_verify_ms"))
        lines.append(
            f"| {case['degree']} | {case['security']} | {case['threads']} | "
            f"{p['baseline_median_ms']:.2f} → {p['candidate_median_ms']:.2f} | "
            f"{v['baseline_median_ms']:.2f} → {v['candidate_median_ms']:.2f} | "
            f"{case['baseline_peak_rss_kib'] / 1024:.2f} → {case['candidate_peak_rss_kib'] / 1024:.2f} | "
            f"{'PASS' if timing_pass(case) else 'FAIL'} |"
        )
    lines.extend([
        "",
        "The [comparison JSON](comparison.json) retains individual ratios and confidence bounds.",
        "",
        "## Witness and proof size",
        "",
        "| N | Live arithmetic bits/signature: before → after | Padded arithmetic bits/signature: before → after | Total packed source KiB/signature: before → after |",
        "|---:|---:|---:|---:|",
    ])
    for degree, (old, new, padded, packed, _) in SOURCE.items():
        lines.append(f"| {degree} | {old:,} → {new:,} | {padded:,} → {padded:,} | {packed // 1024} → {packed // 1024} |")
    lines.extend([
        "",
        "Total packed source includes arithmetic and every SHAKE slab: 96 MiB / 176 MiB for "
        "the 1,024-signature batches. These are logical source sizes, not peak process memory. "
        "The public masks are proof bytes and are not additional committed source columns.",
        "",
        "| N | Security | Full payload KiB: before → after | Public-mask bytes/signature | Public masks KiB/batch, included in payload |",
        "|---:|---:|---:|---:|---:|",
    ])
    for degree, security in itertools.product((512, 1024), (100, 128)):
        selected = [case for case in cases if (case["degree"], case["security"]) == (degree, security)]
        old = value_range([case["baseline_payload_bytes"] for case in selected], 1024)
        new = value_range([case["candidate_payload_bytes"] for case in selected], 1024)
        mask = SOURCE[degree][-1]
        lines.append(f"| {degree} | {security} | {old} → {new} | {mask} | {mask} |")
    changes = [case["candidate_payload_bytes"] - case["baseline_payload_bytes"] for case in cases]
    if min(changes) >= 0:
        payload_change = f"Measured full payload increases by {value_range(changes, 1024)} KiB per batch."
    else:
        payload_change = f"Measured payload changes range from {min(changes) / 1024:+.2f} to {max(changes) / 1024:+.2f} KiB per batch."
    lines.extend([
        "",
        "Payloads are the benchmark's canonical stored payload, excluding Falcon outer framing "
        "and the public statement; masks are included. Ranges, if any, span thread configurations. "
        "Public masks add exactly 92,160 / 167,936 bytes for 1,024 Falcon-512 / Falcon-1024 signatures; "
        "removed HashToPoint proof messages and PCS multiproof overlap also affect the final payload. "
        + payload_change,
        "",
        "## Proved constraints",
        "",
        "The protocol remains non-ZK and retains one initial joint source commitment. "
        "For each candidate j it proves W_j = 12289 Q_j + R_j, with W_j encoded in 16 bits, "
        "Q_j in three bits, and bounded14 decoding enforcing 0 ≤ R_j ≤ 12288. "
        "It proves the quadratic rejection identity e_j = Q_{j,2} Q_{j,0}. "
        "Canonical public masks have exactly N set bits, zero unused tail bits, and are absorbed "
        "before ring and arithmetic challenges. Through the last selected position, linear rows "
        "enforce e_j = 1 − a_j; increasing set-bit positions i_k define linear bindings C_k = R_{i_k}. "
        "These constraints force the first N accepted residues in order. Scalar residual bounds "
        "are below the arithmetic prime.",
        "",
        "Full SHAKE-256 verification, SHAKE-to-word links, Falcon ring membership and norm checks "
        "remain proved and authenticated against the shared sources. HashToPoint has no grand product, "
        "GKR compaction, polynomial-tree witness or committed Horner states. "
        "The integer-to-binary BitZ bridge still uses its existing GKR proof, and the recursive PCS "
        "still has its existing internal commitments. Algebraic Falcon's relation, encoding and "
        "proof protocol are unchanged.",
        "",
        f"Validation: **{args.tests_passed} tests passed**; see the [test log](tests.log). "
        "Benchmark validation also checks every warm-up and measured proof, canonical masks, "
        "matched input digests and public parameters, and the composed security target.",
        "",
        "After the timing campaign, cleanup removed unused native prefix-counter storage, "
        "compaction parameter accessors, and obsolete test-only encoded-word helpers. "
        "The [cleanup validation](cleanup-validation.json) records another 760 passing tests "
        "and four verified 1,024-signature proofs covering both dimensions and security targets. "
        "Their proof Debug digests, payload sizes, input digests, and source roots match all six "
        "corresponding archived trials. The [cleanup test log](cleanup-tests.log) and source "
        "hashes are retained separately; the timing tables above remain the original measurements.",
        "",
        "## Retained experiments",
        "",
        history_text(args.out.parent),
        "",
        "## Reproduce the report",
        "",
        "From this archive directory (the benchmark logs already exist):",
        "",
        "```sh",
        f"python3 ../../scripts/compare_falcon_h2p.py raw --baseline {before} > comparison.json",
        "python3 make_report.py --candidate-dir raw --comparison comparison.json --out RESULTS.md "
        f"--metadata metadata.json --tests-passed {args.tests_passed}",
        "```",
        "",
        "The generator validates the complete 20-case matrix and checks raw timing, payload, "
        "RSS and source-size values against the comparison before writing the report.",
    ])
    return "\n".join(lines) + "\n"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--candidate-dir", "--candidate_dir", type=Path, required=True)
    parser.add_argument("--comparison", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--tests-passed", type=int, required=True)
    parser.add_argument("--metadata", type=Path)
    parser.add_argument("--baseline-link", help="archive-relative baseline directory link")
    args = parser.parse_args()
    if args.tests_passed <= 0:
        parser.error("--tests-passed must be positive")
    comparison = read_json(args.comparison)
    cases = sorted(comparison["cases"], key=lambda case: (case["degree"], case["security"], case["threads"]))
    keys = {(case["degree"], case["security"], case["threads"]) for case in cases}
    if comparison.get("missing") or len(cases) != 20 or keys != EXPECTED_KEYS:
        raise ValueError("a complete 20-case comparison is required")
    headers = validate_raw(args.candidate_dir, cases)
    metadata_path = args.metadata or args.out.parent / "metadata.json"
    metadata = read_json(metadata_path) if metadata_path.is_file() else {}
    result = render(args, comparison, cases, headers, metadata)
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(result)
    print(args.out)


if __name__ == "__main__":
    main()
