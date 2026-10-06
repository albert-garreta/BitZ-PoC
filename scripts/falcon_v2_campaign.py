#!/usr/bin/env python3
"""Matched Falcon V2 measurements. Raw failures are retained; partial runs cannot pass.

The manifest supplies immutable binaries and their build provenance. Discovery
uses seeds42..46; confirmation uses47..66, one warmup and three samples. Each
seed is the statistical unit. Both baseline comparisons use the same V2 run.
"""
import argparse
import hashlib
import itertools
import json
import math
import os
from pathlib import Path
import platform
import re
import signal
import statistics
import subprocess
import sys
import time

from bench_statistics import paired_interval, classify_interval
from bench_gate import stop_process_group, terminate

PROFILES = {
    "native": ("bitz/falcon1024-ct/hybrid/native-ring/non-zk/v4", "wfbitz-joint-limbs"),
    "v1": ("bitz/falcon1024-ct/hybrid/shared-prime/non-zk/v1", "wfbitz-joint-limbs"),
    "v2": ("bitz/falcon1024-ct/hybrid/shared-prime/non-zk/v2", "wfbitz-unsplit"),
    "direct": ("bitz/falcon1024-ct/hybrid/shared-prime/non-zk/v2", "wfbitz-joint-limbs"),
}
MATCHED = ("batch", "capacity", "security_target", "threads", "target_arch", "gf128_kernel",
           "compiled_target_features", "input_source", "input_implementation_version", "input_mode",
           "input_rng", "input_seed", "input_count", "input_digest", "public_inputs",
           "source_bits_per_signature", "build_rustflags", "runtime_rustflags",
           "native_verify_includes_public_key_decode", "warmup_trials", "measured_trials")
COMMON_TERMS = {
    "prime sampling", "per-signature norm identity", "norm sumchecks", "HashToPoint initial row point",
    "HashToPoint product sumcheck", "ordered compaction fingerprints", "ordered compaction forest sumchecks",
    "ordered compaction forest claim reductions", "compaction leaf instance batching", "compaction leaf sumcheck",
    "linear constraints and terminal batching", "prime source sumcheck", "batched integer-to-binary forest",
    "binary Keccak PIOP", "SHAKE wiring and binary claim batching", "joint binary sumcheck",
    "ring switch and support padding", "shared Ligerito",
}
RING_TERMS = {
    "native": {"native ideal batching and projection", "native coordinate carry batching"},
    "v1": {"shared ring outer, endpoint batch and inner", "integer polynomial projection"},
    "v2": {"shared ring outer and endpoint batch", "integer polynomial projection"},
    "direct": {"shared ring outer and endpoint batch", "integer polynomial projection"},
}

def require(test, message):
    if not test:
        raise ValueError(message)

def digest(path):
    with open(path, "rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()

def save(path, data):
    path.write_text(json.dumps(data, indent=2, allow_nan=False) + "\n")

def validate(rows, label, case, warmup, measured):
    headers = [r for r in rows if r.get("event") == "prepared"]
    finals = [r for r in rows if r.get("event") == "summary"]
    trials = [r for r in rows if r.get("event") == "trial"]
    require(len(headers) == len(finals) == 1 and len(rows) == len(trials)+2, "incomplete process")
    h = headers[0]
    require(all(r.get("schema") == "bitz/falcon-hybrid/v3" for r in rows), "unknown schema")
    require((h.get("protocol"), h.get("integer_bridge")) == PROFILES[label], "wrong protocol/bridge")
    require(h.get("stage_timings") is False, "instrumented latency")
    for key, value in zip(("security_target", "batch", "threads", "input_seed"), case):
        require(h.get(key) == value, "wrong workload: " + key)
    require(h.get("warmup_trials") == warmup and h.get("measured_trials") == measured, "wrong repetitions")
    terms = h.get("security_terms", [])
    require(len(terms) == len(COMMON_TERMS | RING_TERMS[label]), "missing/duplicate security terms")
    require({t["name"] for t in terms} == COMMON_TERMS | RING_TERMS[label], "unexpected security ledger")
    require(all(math.isfinite(t["error_bound"]) and t["error_bound"] >= 0 for t in terms), "invalid security error")
    error = sum(t["error_bound"] for t in terms)
    require(0 < error <= 2.0**-case[0], "security below requested target")
    require(math.isclose(-math.log2(error), h["algebraic_security_bits"], rel_tol=1e-12), "ledger total mismatch")
    samples = [r for r in trials if r.get("trial") == "sample"]
    require(len(trials) == warmup+measured and len(samples) == measured, "wrong trial count")
    require([r.get("sample") for r in samples] == list(range(1, measured+1)), "sample numbering")
    require(all(r.get("verified") is True for r in trials+finals), "unverified proof")
    require(all(r.get("trial") in ("warmup", "sample") for r in trials), "unknown trial kind")
    require(sum(r.get("trial") == "warmup" for r in trials) == warmup, "wrong warmup count")
    require(all(r.get("sample") is None for r in trials if r["trial"] == "warmup"), "numbered warmup")
    require(re.fullmatch(r"[0-9a-f]{64}", h.get("input_digest") or "") is not None, "invalid input digest")
    require(all(re.fullmatch(r"[0-9a-f]{64}", r.get("proof_debug_digest") or "") for r in trials), "invalid proof digest")
    require(all(finals[0].get(k) == h.get(k) for k in ("batch", "capacity", "security_target", "threads")), "summary workload drift")
    require(finals[0].get("samples") == measured, "summary sample count")
    require(all(k in h for k in MATCHED), "missing workload metadata")
    for r in trials:
        require(all(r.get(k) == h.get(k) for k in ("batch", "capacity", "threads", "security_target", "input_digest")), "trial drift")
        for k in ("total_prover_ms", "proof_verify_ms", "witness_commit_ms", "proof_prove_ms"):
            require(math.isfinite(r[k]) and r[k] > 0, "invalid timing")
        require(math.isclose(r["total_prover_ms"], r["witness_commit_ms"]+r["proof_prove_ms"], rel_tol=1e-12), "prover accounting")
        require(type(r["proof_payload_bytes"]) is int and r["proof_payload_bytes"] > 0, "invalid payload")
        require(r.get("proof_payload_definition") == "canonical stored payload; excludes Falcon framing and public statement", "wrong payload accounting")
    require(len({r["proof_debug_digest"] for r in trials}) == 1, "proof changed between repeats")
    require(len({r["proof_payload_bytes"] for r in trials}) == 1, "payload changed between repeats")
    medians = {k: statistics.median(r[k] for r in samples) for k in ("total_prover_ms", "proof_verify_ms", "witness_commit_ms", "proof_prove_ms")}
    require(all(finals[0]["median_"+k] == v for k,v in medians.items()), "summary differs from raw samples")
    return dict(prepared=h, medians=medians, payload_bytes=samples[0]["proof_payload_bytes"],
                proof_digest=samples[0]["proof_debug_digest"], verified_proofs=len(trials))

def environment(threads):
    env = dict(os.environ)
    for key in list(env):
        if key.startswith(("BITZ_", "FLOCK_", "CARGO_PROFILE_")) or key in ("RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "RAYON_NUM_THREADS", "PERFETTO_TRACE", "RUST_LOG"):
            env.pop(key, None)
    env.update(RUSTFLAGS="-C target-cpu=native", RAYON_NUM_THREADS=str(threads))
    return env

def run_one(out, item, label, case, warmup, measured, timeout):
    sec,batch,threads,seed = case
    stem = f"s{sec}-b{batch}-t{threads}-seed{seed}-{label}"
    record_path = out / (stem+".run.json")
    require(not record_path.exists(), "refusing to overwrite " + stem)
    require(digest(item["binary"]) == item["sha256"], "binary hash changed")
    command = [item["binary"], "--security",str(sec),"--batch",str(batch),"--threads",str(threads),
               "--seed",str(seed),"--warmup",str(warmup),"--iterations",str(measured)]
    if label != "native":
        command += ["--protocol","shared-prime"]
    time_cmd = ["/usr/bin/time", "-l"] if sys.platform == "darwin" else ["/usr/bin/time", "-v"]
    command = time_cmd + command
    record = dict(label=label, case=case, command=command, started=time.time(), status="running", binary_sha256=item["sha256"])
    save(record_path,record)
    print("START " + stem, flush=True)
    start=time.monotonic()
    with (out/(stem+".jsonl")).open("w") as stdout, (out/(stem+".stderr")).open("w") as stderr:
        process=subprocess.Popen(command,stdout=stdout,stderr=stderr,env=environment(threads),start_new_session=True)
        try:
            code=process.wait(timeout=timeout)
        except BaseException:
            stop_process_group(process)
            record.update(status="interrupted",elapsed_seconds=time.monotonic()-start)
            save(record_path,record)
            raise
    record.update(exit_code=code,elapsed_seconds=time.monotonic()-start,status="failed")
    save(record_path,record)
    require(code == 0, "benchmark failed: " + stem)
    raw=(out/(stem+".stderr")).read_text()
    match=re.search(r"^\s*(\d+)\s+maximum resident set size",raw,re.M) if sys.platform == "darwin" else re.search(r"Maximum resident set size \(kbytes\):\s*(\d+)",raw)
    require(match is not None,"missing external RSS")
    record["rss_bytes"]=int(match[1])*(1 if sys.platform == "darwin" else 1024)
    rows=[json.loads(line) for line in (out/(stem+".jsonl")).read_text().splitlines() if line.strip()]
    record.update(validate(rows,label,case,warmup,measured),status="complete")
    require(record["prepared"]["build_rustflags"] == item["rustflags"], "reported build flags differ from provenance")
    require(record["prepared"]["target_arch"] == {"arm64": "aarch64"}.get(item["architecture"], item["architecture"]), "reported architecture differs from provenance")
    save(record_path,record)
    print(f"DONE {stem}: {record['medians']['total_prover_ms']:.3f}ms, {record['payload_bytes']}bytes",flush=True)
    return record

def compare(records, candidate, expected_seeds, confirmation):
    result=[]
    for baseline in (x for x in records if x != candidate):
        before,after=records[baseline],records[candidate]
        require(before.keys() == after.keys(),"unmatched inputs")
        for seed in before:
            a,b=before[seed]["prepared"],after[seed]["prepared"]
            require(a.get("input_seed") == seed and b.get("input_seed") == seed, "seed key differs from workload")
            require(all(k in a and k in b and a[k] == b[k] for k in MATCHED),"workload mismatch")
        metrics={}
        for key in ("total_prover_ms","proof_verify_ms"):
            ratios=[after[s]["medians"][key]/before[s]["medians"][key] for s in before]
            interval=paired_interval(ratios) if len(ratios)>=2 else None
            state=classify_interval(interval) if interval else "inconclusive"
            if state == "pass" and not confirmation: state="inconclusive"
            metrics[key]=dict(ratio=math.exp(statistics.mean(map(math.log,ratios))),interval_95=interval,status=state)
        payload=all(after[s]["payload_bytes"]<=before[s]["payload_bytes"] for s in before)
        memory=all(after[s]["rss_bytes"]<=before[s]["rss_bytes"] for s in before)
        complete=set(before)==set(expected_seeds)
        states=[m["status"] for m in metrics.values()]
        status="regression" if "regression" in states or not payload or not memory else "pass" if confirmation and complete and all(s=="pass" for s in states) else "inconclusive"
        result.append(dict(baseline=baseline,candidate=candidate,seeds=list(before),complete=complete,
                           metrics=metrics,payload_pass=payload,memory_pass=memory,status=status))
    return result

def main():
    # Let bench_gate termination run child-group cleanup and retain evidence.
    signal.signal(signal.SIGTERM, terminate)
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("manifest",type=Path)
    parser.add_argument("--output",type=Path,required=True)
    parser.add_argument("--phase",choices=("diagnostic","discovery","confirmation"),default="discovery")
    parser.add_argument("--batches",type=int,nargs="+",default=[1,3,32,1024])
    parser.add_argument("--threads",type=int,nargs="+",default=[1,8] if sys.platform=="darwin" else [1,16])
    parser.add_argument("--security",type=int,nargs="+",default=[100,128])
    parser.add_argument("--seeds",type=int,nargs="+")
    parser.add_argument("--warmup",type=int,default=1)
    parser.add_argument("--iterations",type=int,default=3)
    parser.add_argument("--timeout",type=float,default=1800)
    parser.add_argument("--stop-on-regression",action="store_true")
    args=parser.parse_args()
    seeds=args.seeds or list(range(47,67) if args.phase=="confirmation" else range(42,47))
    require(args.phase!="confirmation" or (seeds==list(range(47,67)) and args.warmup==1 and args.iterations==3),"confirmation policy mismatch")
    require(args.warmup>=0 and args.iterations>0 and args.timeout>0,"invalid run settings")
    manifest=json.loads(args.manifest.read_text())
    binaries=manifest["binaries"]
    candidate=manifest.get("candidate","v2")
    require(candidate in binaries and len(binaries)>=2,"need candidate and baseline")
    require(all(x in PROFILES for x in binaries),"unknown variant")
    provenance=("host","architecture","rustc","rustflags","features","profile","lock_sha256")
    for item in binaries.values():
        require(all(item.get(k) == manifest.get(k) and item.get(k) is not None for k in provenance),"binary provenance mismatch")
        require(re.fullmatch(r"[0-9a-f]{40}", item.get("revision", "")) is not None, "missing pinned source revision")
        require(digest(item["binary"])==item["sha256"],"binary mismatch")
    require(manifest.get("rustflags") == "-C target-cpu=native" and manifest.get("profile") == "release" and manifest.get("features") == ["falcon-hybrid"], "wrong build policy")
    require(manifest["host"]==platform.node() and manifest["architecture"]==platform.machine(),"wrong benchmark host")
    args.output.mkdir(parents=True,exist_ok=False)
    state=dict(manifest=manifest,phase=args.phase,seeds=seeds,warmup=args.warmup,iterations=args.iterations,
               status="running",qualification_status="inconclusive",cells=[],confidence_scope="95% per metric/cell; seed-level paired bootstrap, not simultaneous")
    save(args.output/"campaign.json",state)
    try:
        for cell in itertools.product(args.security,args.batches,args.threads):
            records={k:{} for k in binaries}
            for i,seed in enumerate(seeds):
                labels=list(binaries)
                if i%2: labels.reverse()
                for label in labels:
                    records[label][seed]=run_one(args.output,binaries[label],label,(*cell,seed),args.warmup,args.iterations,args.timeout)
            comparisons=compare(records,candidate,seeds,args.phase=="confirmation")
            state["cells"].append(dict(cell=cell,comparisons=comparisons))
            save(args.output/"campaign.json",state)
            if args.stop_on_regression and any(c["status"]=="regression" for c in comparisons):
                state["status"]="stopped_on_regression"; break
        else:
            state["status"]="complete"
    except BaseException as error:
        state.update(status="failed",error=f"{type(error).__name__}: {error}")
        raise
    finally:
        full_cells = set(itertools.product((100,128),(1,3,32,1024),(1,8) if sys.platform=="darwin" else (1,16)))
        seen_cells = {tuple(c["cell"]) for c in state["cells"]}
        comparisons = [x for c in state["cells"] for x in c["comparisons"]]
        state["matrix_complete"] = seen_cells == full_cells
        if any(x["status"] == "regression" for x in comparisons):
            state["qualification_status"] = "regression"
        elif (state["status"] == "complete" and args.phase == "confirmation" and seen_cells == full_cells
              and set(binaries) == {"native","v1","v2"} and candidate == "v2"
              and all(x["status"] == "pass" for x in comparisons)):
            state["qualification_status"] = "pass"
        save(args.output/"campaign.json",state)
    print(json.dumps(dict(status=state["status"],cells=state["cells"])),flush=True)

if __name__=="__main__":
    main()
