#!/usr/bin/env python3
"""Read-only shared-prime campaign summary, using only the Python standard library.

Each timing observation is a process median for one seed. Geometric means never
pool seeds across cells or hosts. Confidence intervals are the saved campaign
intervals, checked against the paired raw-data point estimate. Incomplete
processes contribute verified-proof/nonce diagnostics, never timing or RSS means.
"""
import argparse
import json
import math
from pathlib import Path
import re
import statistics
import sys

METRICS = ("total_prover_ms", "proof_verify_ms")


def require(ok, message):
    if not ok:
        raise ValueError(message)


def geometric_mean(values):
    require(values and all(math.isfinite(v) and v > 0 for v in values), "invalid timing values")
    return math.exp(statistics.mean(map(math.log, values)))


def exact_median(values):
    values = sorted(values)
    mid = len(values) // 2
    if len(values) % 2:
        return str(values[mid])
    numerator = values[mid - 1] + values[mid]
    return str(numerator // 2) + (".5" if numerator % 2 else "")


def read_jsonl(path):
    return [json.loads(line) for line in path.read_text().splitlines() if line.strip()] if path.exists() else []


def nonce_inventory(trials, where):
    inventories = []
    for trial in trials:
        diagnostic = trial.get("grinding_diagnostics", {})
        require("not executed hashes" in diagnostic.get("definition", ""), f"{where}: missing nonce definition")
        inventory = {}
        for item in diagnostic.get("categories", []):
            name, count, encoded = item["category"], item["stored_nonce_boundaries"], item["serial_nonce_prefix_sum"]
            require(isinstance(name, str) and name not in inventory, f"{where}: duplicate nonce category")
            require(type(count) is int and count >= 0, f"{where}: invalid nonce count")
            require(isinstance(encoded, str) and re.fullmatch(r"[0-9]+", encoded), f"{where}: nonce total must be a decimal string")
            total = int(encoded)
            require(count <= total < 2**128 and total <= count * 2**64, f"{where}: invalid nonce prefix total")
            inventory[name] = (count, total)
        require(inventory, f"{where}: missing nonce categories")
        inventories.append(inventory)
    require(all(x == inventories[0] for x in inventories), f"{where}: nonce drift between repeated proofs")
    return inventories[0]


def check_header(header, label, case):
    for name, value in zip(("security_target", "batch", "threads", "input_seed"), case):
        require(header.get(name) == value, f"{label}/{case}: wrong {name}")
    require(header.get("stage_timings") is False, "instrumented benchmark")
    historical = {
        "native": ("bitz/falcon1024-ct/hybrid/native-ring/non-zk/v4", "wfbitz-joint-limbs"),
        "v1": ("bitz/falcon1024-ct/hybrid/shared-prime/non-zk/v1", "wfbitz-joint-limbs"),
        "v2": ("bitz/falcon1024-ct/hybrid/shared-prime/non-zk/v2", "wfbitz-unsplit"),
        "direct": ("bitz/falcon1024-ct/hybrid/shared-prime/non-zk/v2", "wfbitz-joint-limbs"),
    }
    require(label in historical or label == "v3", "unknown protocol label")
    if label in historical:
        require((header.get("protocol"), header.get("integer_bridge")) == historical[label], "wrong historical protocol/bridge")
    if label == "v3":
        small = case[0] == 100
        require(case[0] in (100, 128), "unsupported V3 target")
        require(header.get("protocol") == "bitz/falcon1024-ct/hybrid/shared-prime/non-zk/v3", "wrong V3 protocol")
        expected = dict(integer_bridge="wfbitz-unsplit" if small else "wfbitz-joint-limbs",
                        arithmetic_prime_bits=115 if small else 126,
                        arithmetic_prime_min=str(1 << (114 if small else 125)),
                        arithmetic_prime_max=str((1 << 115) - (1 << 102) - 1 if small else (1 << 126) - 1))
        require(all(type(header.get(k)) is type(v) and header.get(k) == v for k, v in expected.items()), "wrong V3 prime/bridge profile")


def read_record(path, state):
    record = json.loads(path.read_text())
    label, case = record["label"], tuple(record["case"])
    require(len(case) == 4 and case[3] in state["seeds"], f"{path}: unexpected case")
    provenance = state["manifest"]["binaries"][label]
    require(record["binary_sha256"] == provenance["sha256"], f"{path}: hash provenance mismatch")
    stem = path.name.removesuffix(".run.json")
    raw = read_jsonl(path.with_name(stem + ".jsonl"))
    require(all(r.get("schema") == "bitz/falcon-hybrid/v3" for r in raw), f"{path}: wrong schema")
    headers = [r for r in raw if r.get("event") == "prepared"]
    trials = [r for r in raw if r.get("event") == "trial"]
    finals = [r for r in raw if r.get("event") == "summary"]
    require(len(headers) <= 1 and len(finals) <= 1 and len(raw) == len(headers) + len(trials) + len(finals), f"{path}: unknown or duplicate event")
    if headers:
        check_header(headers[0], label, case)
    require(not trials or headers, f"{path}: trials without header")
    require(all(t.get("verified") is True for t in trials), f"{path}: unverified trial")
    if trials:
        h = headers[0]
        require(all(t.get("input_digest") == h.get("input_digest") for t in trials), f"{path}: input drift")
        require(len({t["proof_debug_digest"] for t in trials}) == len({t["proof_payload_bytes"] for t in trials}) == 1, f"{path}: proof drift")
        for t in trials:
            require(t.get("trial") in ("warmup", "sample"), f"{path}: unknown trial kind")
            require(all(t.get(k) == h.get(k) for k in ("batch", "capacity", "threads", "security_target")), f"{path}: workload drift")
            require(type(t["proof_payload_bytes"]) is int and t["proof_payload_bytes"] > 0, f"{path}: invalid payload")
            require(re.fullmatch(r"[0-9a-f]{64}", t["proof_debug_digest"]) is not None, f"{path}: invalid proof digest")
            require(all(math.isfinite(t[k]) and t[k] > 0 for k in METRICS), f"{path}: invalid timing")
            require(math.isclose(t["total_prover_ms"], t["witness_commit_ms"] + t["proof_prove_ms"], rel_tol=1e-12), f"{path}: timing sum mismatch")
            require(t.get("proof_payload_definition") == "canonical stored payload; excludes Falcon framing and public statement", f"{path}: payload definition changed")
        if label == state["manifest"].get("candidate", "v2"):
            record["nonce_inventory"] = nonce_inventory(trials, path)
    record["raw_verified_proofs"] = len(trials)
    if record["status"] == "complete":
        require(len(headers) == len(finals) == 1 and record.get("exit_code") == 0 and finals[0].get("verified") is True, f"{path}: incomplete successful process")
        require(record["prepared"] == headers[0], f"{path}: saved header mismatch")
        samples = [t for t in trials if t.get("trial") == "sample"]
        require(len(trials) == state["warmup"] + state["iterations"] and len(samples) == state["iterations"], f"{path}: trial count mismatch")
        require([t.get("sample") for t in samples] == list(range(1, state["iterations"] + 1)), f"{path}: sample numbering mismatch")
        require(sum(t["trial"] == "warmup" for t in trials) == state["warmup"], f"{path}: warmup count mismatch")
        require(record["verified_proofs"] == len(trials), f"{path}: saved proof count mismatch")
        for key in METRICS:
            median = statistics.median(t[key] for t in samples)
            require(record["medians"][key] == median == finals[0]["median_" + key], f"{path}: median mismatch")
        require(record["payload_bytes"] == samples[0]["proof_payload_bytes"], f"{path}: saved payload mismatch")
        stderr = path.with_name(stem + ".stderr").read_text()
        mac = re.search(r"^\s*(\d+)\s+maximum resident set size", stderr, re.M)
        linux = re.search(r"Maximum resident set size \(kbytes\):\s*(\d+)", stderr)
        require(bool(mac) != bool(linux), f"{path}: external RSS missing/ambiguous")
        rss = int(mac[1]) if mac else 1024 * int(linux[1])
        require(rss == record["rss_bytes"] and rss > 0, f"{path}: external RSS mismatch")
    return (label, case), record


def summarize(directory):
    state = json.loads((directory / "campaign.json").read_text())
    records = {}
    for path in sorted(directory.glob("*.run.json")):
        key, record = read_record(path, state)
        require(key not in records, f"{directory}: duplicate process")
        records[key] = record
    complete = {k: r for k, r in records.items() if r["status"] == "complete"}
    candidate = state["manifest"].get("candidate", "v2")
    report = dict(directory=str(directory), phase=state["phase"], status=state["status"], qualification_status=state["qualification_status"],
                  completed_processes=len(complete), complete_verified_proofs=sum(r["raw_verified_proofs"] for r in complete.values()),
                  partial_verified_proofs=sum(r["raw_verified_proofs"] for r in records.values() if r["status"] != "complete"),
                  incomplete_processes=[dict(label=k[0], case=list(k[1]), status=r["status"], verified_proofs=r["raw_verified_proofs"]) for k,r in records.items() if r["status"] != "complete"],
                  variants=[], comparisons=[], nonce_categories=[])
    cells = sorted({case[:3] for _,case in records})
    for cell in cells:
        for label in state["manifest"]["binaries"]:
            data = [(case[3], r) for (name,case),r in complete.items() if name == label and case[:3] == cell]
            if not data:
                continue
            report["variants"].append(dict(cell=list(cell), variant=label, seeds=sorted(s for s,_ in data), verified_proofs=sum(r["raw_verified_proofs"] for _,r in data),
                 geometric_mean_ms={k:geometric_mean([r["medians"][k] for _,r in data]) for k in METRICS},
                 payload_bytes=[min(r["payload_bytes"] for _,r in data), max(r["payload_bytes"] for _,r in data)],
                 rss_bytes=[min(r["rss_bytes"] for _,r in data), max(r["rss_bytes"] for _,r in data)]))
        inventories = sorted((case[3],r) for (label,case),r in records.items() if label == candidate and case[:3] == cell and "nonce_inventory" in r)
        if inventories:
            categories = set(inventories[0][1]["nonce_inventory"])
            require(all(set(r["nonce_inventory"]) == categories for _,r in inventories), f"{directory}: nonce category drift")
            for category in sorted(categories):
                counts = [r["nonce_inventory"][category][0] for _,r in inventories]
                sums = [r["nonce_inventory"][category][1] for _,r in inventories]
                report["nonce_categories"].append(dict(cell=list(cell), variant=candidate, category=category,
                    boundaries_min=min(counts), boundaries_max=max(counts), prefix_min=str(min(sums)), prefix_median=exact_median(sums), prefix_max=str(max(sums)),
                    prefix_sum_across_seeds=str(sum(sums)), seeds=[dict(seed=s, process_status=r["status"], boundaries=r["nonce_inventory"][category][0], prefix_sum=str(r["nonce_inventory"][category][1])) for s,r in inventories]))
    for cell_state in state["cells"]:
        cell = tuple(cell_state["cell"])
        for comparison in cell_state["comparisons"]:
            before = [complete[comparison["baseline"], (*cell,s)] for s in comparison["seeds"]]
            after = [complete[comparison["candidate"], (*cell,s)] for s in comparison["seeds"]]
            require(all(a["prepared"]["input_digest"] == b["prepared"]["input_digest"] for a,b in zip(before,after)), "unmatched pair inputs")
            for key in METRICS:
                observed = geometric_mean([b["medians"][key] / a["medians"][key] for a,b in zip(before,after)])
                saved = comparison["metrics"][key]
                require(math.isclose(observed, saved["ratio"], rel_tol=1e-12), "paired point estimate differs from raw records")
                interval = saved["interval_95"]
                require(interval is None or len(interval) == 2 and all(math.isfinite(x) and x > 0 for x in interval) and interval[0] <= interval[1], "invalid saved confidence interval")
            require(comparison["payload_pass"] == all(b["payload_bytes"] <= a["payload_bytes"] for a,b in zip(before,after)), "payload gate mismatch")
            require(comparison["memory_pass"] == all(b["rss_bytes"] <= a["rss_bytes"] for a,b in zip(before,after)), "RSS gate mismatch")
            deltas = [b["rss_bytes"] - a["rss_bytes"] for a,b in zip(before,after)]
            report["comparisons"].append(dict(cell=list(cell), **comparison, rss_increase_pairs=sum(x > 0 for x in deltas), rss_delta_bytes=[min(deltas), max(deltas)]))
    return report


def show(report):
    print(f"{report['directory']}: phase={report['phase']} status={report['status']} qualification={report['qualification_status']} completed_processes={report['completed_processes']} completed_verified_proofs={report['complete_verified_proofs']} partial_verified_proofs={report['partial_verified_proofs']}")
    for row in report["variants"]:
        times = row["geometric_mean_ms"]
        print(f"  s{row['cell'][0]}/b{row['cell'][1]}/t{row['cell'][2]} {row['variant']} seeds={row['seeds']} GM_ms total={times['total_prover_ms']:.6f} verify={times['proof_verify_ms']:.6f} payloadB={row['payload_bytes']} RSS_B={row['rss_bytes']} verified={row['verified_proofs']}")
    for row in report["comparisons"]:
        def metric(key):
            value = row["metrics"][key]
            ci = value["interval_95"]
            return f"{value['ratio']:.6f}[{','.join(f'{x:.6f}' for x in ci) if ci else 'NA'}]"
        print(f"  paired {row['cell']} {row['candidate']}/{row['baseline']} seeds={row['seeds']} total={metric('total_prover_ms')} verify={metric('proof_verify_ms')} RSS+={row['rss_increase_pairs']}/{len(row['seeds'])} deltaB={row['rss_delta_bytes']} payload_pass={row['payload_pass']} memory_pass={row['memory_pass']} status={row['status']}")
    for row in report["nonce_categories"]:
        per_seed = ','.join(f"{x['seed']}:{x['prefix_sum']}" + ("(partial)" if x['process_status'] != 'complete' else '') for x in row['seeds'])
        print(f"  nonce {row['cell']} {row['variant']} {row['category']} boundaries={row['boundaries_min']}..{row['boundaries_max']} prefix[min,median,max]=[{row['prefix_min']},{row['prefix_median']},{row['prefix_max']}] sum_across_seeds={row['prefix_sum_across_seeds']} by_seed={per_seed}")
    for row in report["incomplete_processes"]:
        print(f"  incomplete {row['label']} {row['case']} status={row['status']} verified_proofs={row['verified_proofs']}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("campaigns", nargs="+", type=Path)
    parser.add_argument("--format", choices=("text", "json"), default="text")
    args = parser.parse_args()
    reports = [summarize(p.resolve()) for p in args.campaigns]
    notes = ["Timing geometric means use one process median per seed; cells/hosts are separate. Saved paired CI95 values come from campaign.json; this script checks point estimates but does not recompute intervals.",
             "RSS is the external warmed-process peak in bytes. Incomplete processes contribute no timing, RSS, or paired estimates. Proof counts include verified warmups.",
             "Nonce prefix=sum(nonce+1) over stored entries, one repeat-stable proof per seed; includes zero-difficulty placeholders, excludes SIMD/parallel overscan, and is not executed hashes. Totals remain decimal strings in JSON.",
             "This summarizes measurement evidence and metadata consistency; it is not an independent security audit or a new qualification decision."]
    if args.format == "json":
        print(json.dumps(dict(notes=notes, campaigns=reports), indent=2, allow_nan=False))
    else:
        for note in notes:
            print(note)
        for report in reports:
            show(report)


if __name__ == "__main__":
    try:
        main()
    except (ValueError, KeyError, TypeError, OSError) as error:
        print(f"summary invalid: {error}", file=sys.stderr)
        sys.exit(1)
