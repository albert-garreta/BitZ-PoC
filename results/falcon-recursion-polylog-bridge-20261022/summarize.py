"""Recursion over the logarithmic BitZ bridge (BITZ_FALCON_BRIDGE=polylog)
against the standard bridge, FlockRec at T = 18 and 16, ext:3:4 base and
outer, λ=128, 10 threads.

base*        = the sha256-fs base proof the circuit verifies (prove time,
               payload bytes)
blocks       = the circuit's SHA-256 compressions
rec step     = witness + recording (concurrent) + early commitment + outer
               relation + outer witness + FlockRec prove (the harness's
               median rep; at 32K with two reps, the slower one)
public       = the public stream (with the polylog bridge: no folds)

usage: python3 summarize.py <runs dir>
"""
import json
import sys
from pathlib import Path

runs = Path(sys.argv[1])


def row(name):
    path = runs / f"{name}.out"
    if not path.exists():
        return None
    lines = [l for l in path.read_text().splitlines() if l.startswith("TIMING ")]
    return json.loads(lines[-1][len("TIMING "):]) if lines else None


print("| batch | bridge | T | base* prove | base* proof | blocks | FlockRec prove | rec step | final verify | π | public | π + public |")
print("|---|---|---|---|---|---|---|---|---|---|---|---|")
for batch in (1024, 4096, 32768):
    for t in (18, 16):
        for bridge in ("standard", "polylog"):
            r = row(f"{bridge}-t{t}-b{batch}")
            if r is None:
                continue
            public = r["public_stream_bytes"] + r["public_input_bytes"]
            print(
                f"| {batch:,} | {bridge} | {t} | {1000 * r['base_prove_s']:.0f} ms | {r['base_proof_bytes'] / 1000:.1f} KB "
                f"| {r['circuit']['blocks']:,} | {r['rec_protocol_s']:.2f} s | {r['rec_prove_s']:.2f} s | {r['final_verify_s']:.2f} s "
                f"| {r['pi_bytes'] / 1000:.1f} KB | {public / 1000:.1f} KB | {(r['pi_bytes'] + public) / 1000:.1f} KB |"
            )
print()
print("Public stream by label (32K):")
for bridge in ("standard", "polylog"):
    r = row(f"{bridge}-t18-b32768")
    if r is not None:
        print(f"  {bridge}: {', '.join(r['public_top'])}")
print()
print("Per rep (s): witness | record | early commit | passes wall | relation | outer witness | FlockRec prove | total")
for batch in (1024, 4096, 32768):
    for t in (18, 16):
        for bridge in ("standard", "polylog"):
            r = row(f"{bridge}-t{t}-b{batch}")
            if r is None:
                continue
            reps = "; ".join(" ".join(f"{x:.2f}" for x in run) for run in r["rec_runs"])
            base = "; ".join(f"{1000 * run[1]:.0f}" for run in r["base_runs"])
            print(f"  b{batch} {bridge} t{t}: {reps}   verify {' '.join(f'{x:.2f}' for x in r['verify_runs'])}   base* prove ms {base}")
