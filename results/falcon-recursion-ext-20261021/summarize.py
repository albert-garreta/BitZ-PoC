"""Falcon-512 recursion at λ=128: the outer Ligerito ladder, unique decoding
(udrg:3:4) against Johnson over GF(2^256) (ext:R:4), standard BitZ opener at
T = 18, per (batch, threads).

rec step     = witness + recording (concurrent) + early commitment + outer
               relation + outer witness + FlockRec prove (the harness's median
               rep; at 32K with two reps, the slower one)
final verify = the recording replayed from the public stream + relation +
               FlockRec verify
base*        = the sha256-fs base proof the circuit verifies (payload bytes)

usage: python3 summarize.py <runs dir>
"""
import json
import sys
from pathlib import Path

runs = Path(sys.argv[1])
clock = {}
for line in (runs / "clock.log").read_text().splitlines():
    parts = line.split()
    if len(parts) > 2 and "=" in parts[2]:
        clock[parts[1]] = dict(p.split("=", 1) for p in parts[2:] if "=" in p)

configs = ["udrg:3:4", "ext:3:4", "ext:5:4", "ext:6:4"]


def rec_row(name):
    path = runs / f"{name}.out"
    if not path.exists():
        return None
    lines = [l for l in path.read_text().splitlines() if l.startswith("TIMING ")]
    return json.loads(lines[-1][len("TIMING "):]) if lines else None


for threads in (10, 1):
    print(f"{threads} thread{'s' if threads > 1 else ''}:")
    print()
    print("| batch | ladder | π | π + public | base* | rec step | FlockRec prove | early commit | final verify | GHz | RSS |")
    print("|---|---|---|---|---|---|---|---|---|---|---|")
    for batch in (1024, 4096, 32768):
        for lig in configs:
            name = f"rec-{lig.replace(':', '_')}-b{batch}-t{threads}"
            r = rec_row(name)
            if r is None:
                continue
            c = clock.get(name, {})
            public = r["public_stream_bytes"] + r["public_input_bytes"]
            print(
                f"| {batch:,} | {lig} | {r['pi_bytes'] / 1000:.1f} KB | {(r['pi_bytes'] + public) / 1000:.1f} KB "
                f"| {r['base_proof_bytes'] / 1000:.1f} KB "
                f"| {r['rec_prove_s']:.2f} s | {r['rec_protocol_s']:.2f} s | {r['rec_early_commit_s']:.2f} s "
                f"| {r['final_verify_s']:.2f} s | {c.get('ghz', '?')} | {c.get('rss', '?')} |"
            )
    print()

print("Step breakdown (s): witness | record (overlaps) | early commit | passes wall | relation | outer witness | FlockRec prove | total; per rep")
for threads in (10, 1):
    for batch in (1024, 4096, 32768):
        for lig in configs:
            r = rec_row(f"rec-{lig.replace(':', '_')}-b{batch}-t{threads}")
            if r is None:
                continue
            reps = "; ".join(" ".join(f"{x:.2f}" for x in run) for run in r["rec_runs"])
            verify = " ".join(f"{x:.2f}" for x in r["verify_runs"])
            print(f"  b{batch} t{threads} {lig}: {reps}   verify {verify}   [π {r['pi_bytes']} B]")
