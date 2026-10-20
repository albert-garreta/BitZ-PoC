"""Falcon-512 recursion A/B campaign: the 2026-10-18 binary (old) against the
optimized one (new), per (security, batch, threads), and the production base.

rec step     = witness + recording (concurrent) + early commitment + outer
               relation + outer witness + FlockRec prove (the harness's median
               rep; at 32K with two reps, the slower one)
final verify = the recording replayed from the public stream + relation +
               FlockRec verify

usage: python3 summarize.py <runs dir>
"""
import json
import statistics
import sys
from pathlib import Path

runs = Path(sys.argv[1])
clock = {}
for line in (runs / "clock.log").read_text().splitlines():
    parts = line.split()
    if len(parts) > 2 and "=" in parts[2]:
        clock[parts[1]] = dict(p.split("=", 1) for p in parts[2:] if "=" in p)


def base_row(name):
    path = runs / f"{name}.jsonl"
    if not path.exists():
        return None
    rows = [json.loads(l) for l in path.read_text().splitlines() if l.startswith("{")]
    samples = [r for r in rows if r.get("trial") == "sample"]
    if not samples:
        return None
    med = lambda key: statistics.median(r[key] for r in samples)
    return {"prover_ms": med("total_prover_ms"), "verify_ms": med("verify_ms"), "bytes": samples[-1]["proof_payload_bytes"]}


def rec_row(name):
    path = runs / f"{name}.out"
    if not path.exists():
        return None
    lines = [l for l in path.read_text().splitlines() if l.startswith("TIMING ")]
    return json.loads(lines[-1][len("TIMING "):]) if lines else None


def kb(b):
    return f"{b / 1000:.1f}"


def speed(old, new):
    return f"{old:.2f} → {new:.2f} s ({old / new:.2f}×)"


cells = [(sec, batch) for sec in (128, 100) for batch in (1024, 4096, 32768)]
for threads in (10, 1):
    print(f"{threads} thread{'s' if threads > 1 else ''}:")
    print()
    print("| λ | batch | rec step old → new | final verify old → new | π old / new | public | base prove | base verify | GHz old/new | RSS old/new |")
    print("|---|---|---|---|---|---|---|---|---|---|")
    for sec, batch in cells:
        key = f"s{sec}-b{batch}-t{threads}"
        old, new, b = rec_row(f"rec-old-{key}"), rec_row(f"rec-new-{key}"), base_row(f"base-{key}")
        if old is None or new is None:
            continue
        co, cn = clock.get(f"rec-old-{key}", {}), clock.get(f"rec-new-{key}", {})
        row = [str(sec), f"{batch:,}", speed(old["rec_prove_s"], new["rec_prove_s"]), speed(old["final_verify_s"], new["final_verify_s"])]
        row.append(f"{kb(old['pi_bytes'])} / {kb(new['pi_bytes'])} KB")
        row.append(f"{kb(new['public_stream_bytes'] + new['public_input_bytes'])} KB")
        row.append(f"{b['prover_ms']:.0f} ms" if b else "–")
        row.append(f"{b['verify_ms']:.1f} ms" if b else "–")
        row.append(f"{co.get('ghz', '?')}/{cn.get('ghz', '?')}")
        row.append(f"{co.get('rss', '?')}/{cn.get('rss', '?')}")
        print("| " + " | ".join(row) + " |")
    print()

print("Step breakdown (s): witness | record (overlaps) | early commit | passes wall | relation | outer witness | FlockRec prove | total; per rep")
for threads in (10, 1):
    for sec, batch in cells:
        for which in ("old", "new"):
            r = rec_row(f"rec-{which}-s{sec}-b{batch}-t{threads}")
            if r is None:
                continue
            reps = "; ".join(" ".join(f"{x:.2f}" for x in run) for run in r["rec_runs"])
            verify = " ".join(f"{x:.2f}" for x in r["verify_runs"])
            c = r["circuit"]
            print(f"  λ{sec} b{batch} t{threads} {which}: {reps}   verify {verify}   "
                  f"[{c['blocks']} blocks, {c['committed'] / 1e6:.1f} M committed, outer f/h {c['outer_f_bits']}/{c['outer_h_bits']}]")
