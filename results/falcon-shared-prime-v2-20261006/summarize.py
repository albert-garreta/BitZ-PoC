#!/usr/bin/env python3
"""Read-only Falcon V2 campaign summary; completed processes only, no polling.

Use the committed campaign validator and paired-bootstrap implementation from
--scripts (or a campaign ancestor's scripts directory). No result files or
bytecode caches are written. Timing units are per-seed process medians; nonce
statistics use one repeat-stable proof per seed, never pooled repetitions.
"""
import argparse
import csv
import importlib.util
import json
import math
import re
from pathlib import Path
import statistics
import sys

sys.dont_write_bytecode = True


def require(ok, message):
    if not ok:
        raise ValueError(message)


def load_tools(directories, explicit):
    choices = [explicit] if explicit else [p / "scripts" for d in directories for p in (d, *d.parents)]
    scripts = next((p for p in choices if p and (p / "falcon_v2_campaign.py").is_file()), None)
    require(scripts is not None, "cannot locate campaign validator; provide --scripts /path/to/repo/scripts")
    sys.path.insert(0, str(scripts))
    spec = importlib.util.spec_from_file_location("falcon_summary_validator", scripts / "falcon_v2_campaign.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def exact_median(values):
    ordered = sorted(values)
    middle = len(ordered) // 2
    if len(ordered) % 2:
        return str(ordered[middle])
    numerator = ordered[middle - 1] + ordered[middle]
    return str(numerator // 2) + (".5" if numerator % 2 else "")


def nonce_inventory(trials, where):
    inventories = []
    for trial in trials:
        diagnostic = trial.get("grinding_diagnostics")
        require(isinstance(diagnostic, dict), f"{where}: V2 nonce diagnostics missing")
        require("not executed hashes" in diagnostic.get("definition", ""), f"{where}: unknown nonce definition")
        inventory = {}
        for item in diagnostic.get("categories", []):
            name = item["category"]
            count, encoded = item["stored_nonce_boundaries"], item["serial_nonce_prefix_sum"]
            require(isinstance(name, str) and name not in inventory, f"{where}: duplicate nonce category")
            require(type(count) is int and count >= 0, f"{where}: invalid nonce count")
            require(isinstance(encoded, str) and encoded.isascii() and encoded.isdecimal(), f"{where}: nonce total is not a decimal string")
            total = int(encoded)
            require(count <= total < 2**128 and total <= count * 2**64, f"{where}: invalid nonce prefix total")
            inventory[name] = (count, total)
        require(inventory, f"{where}: empty nonce inventory")
        inventories.append(inventory)
    require(all(x == inventories[0] for x in inventories), f"{where}: nonce inventory changed between repeats")
    return inventories[0]


def read_campaign(directory, tools):
    state = json.loads((directory / "campaign.json").read_text())
    manifest, records, pending = state["manifest"], {}, []
    for path in sorted(directory.glob("*.run.json")):
        record = json.loads(path.read_text())
        if record.get("status") != "complete":
            pending.append(f"{path.name}:{record.get('status', 'missing')}")
            continue
        label, case = record["label"], tuple(record["case"])
        require((label, case) not in records, f"{path}: duplicate completed process")
        require(len(case) == 4 and case[3] in state["seeds"], f"{path}: unexpected case/seed")
        require(record.get("exit_code") == 0, f"{path}: unsuccessful completed process")
        rows = [json.loads(line) for line in path.with_name(path.name.removesuffix(".run.json") + ".jsonl").read_text().splitlines() if line.strip()]
        actual = tools.validate(rows, label, case, state["warmup"], state["iterations"])
        for key, value in actual.items():
            require(record.get(key) == value, f"{path}: saved {key} differs from raw trials")
        provenance = manifest["binaries"][label]
        require(record["binary_sha256"] == provenance["sha256"], f"{path}: binary provenance mismatch")
        require(record["prepared"]["build_rustflags"] == provenance["rustflags"], f"{path}: build flags mismatch")
        expected_arch = {"arm64": "aarch64"}.get(provenance["architecture"], provenance["architecture"])
        require(record["prepared"]["target_arch"] == expected_arch, f"{path}: architecture mismatch")
        require(type(record.get("rss_bytes")) is int and record["rss_bytes"] > 0, f"{path}: missing external RSS")
        stderr = path.with_name(path.name.removesuffix(".run.json") + ".stderr").read_text()
        mac = re.search(r"^\s*(\d+)\s+maximum resident set size", stderr, re.M)
        linux = re.search(r"Maximum resident set size \(kbytes\):\s*(\d+)", stderr)
        require(bool(mac) != bool(linux), f"{path}: missing or ambiguous external RSS")
        rss = int(mac[1]) if mac else 1024 * int(linux[1])
        require(rss == record["rss_bytes"], f"{path}: external RSS differs from saved record")
        if label == "v2":
            record["nonce_inventory"] = nonce_inventory([r for r in rows if r.get("event") == "trial"], path)
        records[label, case] = record
    return state, records, pending


def summarize(directory, tools):
    state, records, pending = read_campaign(directory, tools)
    manifest = state["manifest"]
    candidate = manifest.get("candidate", "v2")
    rows = []
    comparisons = {(tuple(c["cell"]), x["baseline"]): x for c in state["cells"] for x in c["comparisons"]}
    cells = sorted({case[:3] for _, case in records})
    for cell in cells:
        for baseline in manifest["binaries"]:
            if baseline == candidate:
                continue
            seeds = [s for s in state["seeds"] if (baseline, (*cell, s)) in records and (candidate, (*cell, s)) in records]
            if not seeds:
                continue
            before = [records[baseline, (*cell, s)] for s in seeds]
            after = [records[candidate, (*cell, s)] for s in seeds]
            for a, b in zip(before, after):
                require(all(a["prepared"][k] == b["prepared"][k] for k in tools.MATCHED), f"{directory}: unmatched workload {cell}")
            row = dict(kind="comparison", campaign=str(directory), security=cell[0], batch=cell[1], threads=cell[2], baseline=baseline, candidate=candidate,
                       seeds=";".join(map(str, seeds)), paired_seeds=len(seeds), cell_complete=set(seeds) == set(state["seeds"]),
                       verified_proofs=sum(r["verified_proofs"] for r in before + after), status="inconclusive")
            saved = comparisons.get((cell, baseline))
            for metric, short in (("total_prover_ms", "prover"), ("proof_prove_ms", "proof_only"), ("proof_verify_ms", "verify")):
                ratios = [b["medians"][metric] / a["medians"][metric] for a, b in zip(before, after)]
                ratio = math.exp(statistics.mean(map(math.log, ratios)))
                interval = tools.paired_interval(ratios) if len(ratios) > 1 else None
                row.update({short + "_ratio": ratio, short + "_ci95_low": interval[0] if interval else "", short + "_ci95_high": interval[1] if interval else ""})
                if saved and metric in saved["metrics"]:
                    require(saved["seeds"] == seeds, f"{directory}: saved comparison seed drift")
                    require(math.isclose(saved["metrics"][metric]["ratio"], ratio, rel_tol=1e-12), f"{directory}: saved ratio drift")
                    require(saved["metrics"][metric]["interval_95"] == interval, f"{directory}: saved interval drift")
            for label, data in (("baseline", before), ("candidate", after)):
                for metric, short in (("payload_bytes", "payload"), ("rss_bytes", "rss")):
                    row[f"{label}_{short}_min_bytes"] = min(r[metric] for r in data)
                    row[f"{label}_{short}_max_bytes"] = max(r[metric] for r in data)
            delta = [b["rss_bytes"] - a["rss_bytes"] for a, b in zip(before, after)]
            row.update(rss_increase_pairs=sum(v > 0 for v in delta), rss_delta_min_bytes=min(delta), rss_delta_max_bytes=max(delta),
                       rss_paired_ratio_max=max(b["rss_bytes"] / a["rss_bytes"] for a, b in zip(before, after)),
                       payload_increase_pairs=sum(b["payload_bytes"] > a["payload_bytes"] for a, b in zip(before, after)))
            if saved:
                row["status"] = saved["status"]
            elif row["rss_increase_pairs"] or row["payload_increase_pairs"] or any(row[k + "_ci95_low"] != "" and row[k + "_ci95_low"] > 1 for k in ("prover", "verify")):
                row["status"] = "regression"
            rows.append(row)
        v2 = [(case[3], r) for (label, case), r in records.items() if label == "v2" and case[:3] == cell]
        if v2:
            categories = set(v2[0][1]["nonce_inventory"])
            require(all(set(r["nonce_inventory"]) == categories for _, r in v2), f"{directory}: nonce categories changed across seeds")
            for category in sorted(categories):
                boundaries = [r["nonce_inventory"][category][0] for _, r in v2]
                prefixes = [r["nonce_inventory"][category][1] for _, r in v2]
                rows.append(dict(kind="nonce", campaign=str(directory), security=cell[0], batch=cell[1], threads=cell[2], candidate="v2",
                                 seeds=";".join(str(s) for s, _ in sorted(v2)), paired_seeds=len(v2), category=category,
                                 boundaries_min=min(boundaries), boundaries_max=max(boundaries), prefix_min=str(min(prefixes)),
                                 prefix_median=exact_median(prefixes), prefix_max=str(max(prefixes))))
    overview = dict(campaign=str(directory), phase=state["phase"], status=state["status"], qualification=state["qualification_status"],
                    complete_processes=len(records), verified_proofs=sum(r["verified_proofs"] for r in records.values()), pending=pending)
    return overview, rows


def show_text(overview, rows):
    print(f"{overview['campaign']}: phase={overview['phase']} status={overview['status']} qualification={overview['qualification']} completed_processes={overview['complete_processes']} verified_proofs={overview['verified_proofs']} incomplete_processes={len(overview['pending'])}")
    for row in rows:
        cell = f"s{row['security']}/b{row['batch']}/t{row['threads']}"
        if row["kind"] == "nonce":
            print(f"  {cell} v2 nonce {row['category']}: seeds={row['paired_seeds']} boundaries={row['boundaries_min']}..{row['boundaries_max']} prefix[min,median,max]=[{row['prefix_min']},{row['prefix_median']},{row['prefix_max']}]")
            continue
        def timing(key):
            ci = "NA" if row[key + "_ci95_low"] == "" else f"{row[key + '_ci95_low']:.6f},{row[key + '_ci95_high']:.6f}"
            return f"{row[key + '_ratio']:.6f}[{ci}]"
        print(f"  {cell} {row['candidate']}/{row['baseline']} seeds={row['seeds']} prover={timing('prover')} proof_only={timing('proof_only')} verify={timing('verify')} payloadB={row['baseline_payload_min_bytes']}..{row['baseline_payload_max_bytes']}->{row['candidate_payload_min_bytes']}..{row['candidate_payload_max_bytes']} RSS+={row['rss_increase_pairs']}/{row['paired_seeds']} deltaB={row['rss_delta_min_bytes']}..{row['rss_delta_max_bytes']} maxRSSratio={row['rss_paired_ratio_max']:.6f} proofs={row['verified_proofs']} status={row['status']}")
    for pending in overview["pending"]:
        print("  incomplete: " + pending)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("campaigns", nargs="+", type=Path)
    parser.add_argument("--scripts", type=Path)
    parser.add_argument("--format", choices=("text", "csv"), default="text")
    args = parser.parse_args()
    directories = [p.resolve() for p in args.campaigns]
    tools = load_tools(directories, args.scripts.resolve() if args.scripts else None)
    reports = [summarize(d, tools) for d in directories]
    if args.format == "csv":
        rows = []
        for overview, detail in reports:
            rows.append({"kind": "campaign", **overview, "pending": ";".join(overview["pending"])})
            rows.extend(detail)
        fields = list(dict.fromkeys(key for row in rows for key in row))
        writer = csv.DictWriter(sys.stdout, fieldnames=fields)
        writer.writeheader()
        writer.writerows(rows)
    else:
        print("Ratios: geometric mean of paired per-seed process medians; CI95: 10,000 seed-bootstrap draws, per metric/cell, not simultaneous. RSS: external warmed-process peak, bytes. Proof totals include warmups; comparison rows share candidate proofs and must not be added together.")
        print("V2 nonce prefixes: one repeat-stable proof per seed; sum(nonce+1), includes zero-difficulty placeholders, excludes SIMD/parallel overscan; not executed hashes. Incomplete processes are excluded. This is a consistency check, not an independent security audit.")
        for overview, rows in reports:
            show_text(overview, rows)


if __name__ == "__main__":
    try:
        main()
    except (ValueError, KeyError, OSError, json.JSONDecodeError) as error:
        print(f"summary invalid: {error}", file=sys.stderr)
        sys.exit(1)
