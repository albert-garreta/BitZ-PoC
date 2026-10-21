"""The recursion step with the base opening under unique decoding (udr) against
Johnson over GF(2^256) with the standard terminal (ext), outer ext:3:4 with the
standard opener at T = 18, λ=128, 10 threads.

base*        = the sha256-fs base proof the circuit verifies (prove time,
               payload bytes)
rec step     = witness + recording (concurrent) + early commitment + outer
               relation + outer witness + FlockRec prove (the harness's median
               rep; at 32K with two reps, the slower one)
final verify = the recording replayed from the public stream + relation +
               FlockRec verify

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


print("| batch | base opening | base* prove | base* proof | blocks | f / h bits | record | relation | FlockRec prove | rec step | final verify | π | π + public |")
print("|---|---|---|---|---|---|---|---|---|---|---|---|---|")
for batch in (1024, 4096, 32768):
    for kind, label in (("udr", "udrg:1:4"), ("ext", "ext:3:4 standard")):
        r = row(f"{kind}-b{batch}")
        if r is None:
            continue
        c = r["circuit"]
        public = r["public_stream_bytes"] + r["public_input_bytes"]
        print(
            f"| {batch:,} | {label} | {1000 * r['base_prove_s']:.0f} ms | {r['base_proof_bytes'] / 1000:.1f} KB "
            f"| {c['blocks']:,} | {c['outer_f_bits']} / {c['outer_h_bits']} | {r['rec_record_s']:.2f} s "
            f"| {r['rec_relation_s']:.2f} s | {r['rec_protocol_s']:.2f} s | {r['rec_prove_s']:.2f} s "
            f"| {r['final_verify_s']:.2f} s | {r['pi_bytes'] / 1000:.1f} KB | {(r['pi_bytes'] + public) / 1000:.1f} KB |"
        )
print()
print("Per rep (s): witness | record | early commit | passes wall | relation | outer witness | FlockRec prove | total")
for batch in (1024, 4096, 32768):
    for kind in ("udr", "ext"):
        r = row(f"{kind}-b{batch}")
        if r is None:
            continue
        reps = "; ".join(" ".join(f"{x:.2f}" for x in run) for run in r["rec_runs"])
        base = "; ".join(f"{1000 * run[1]:.0f}" for run in r["base_runs"])
        print(f"  b{batch} {kind}: {reps}   verify {' '.join(f'{x:.2f}' for x in r['verify_runs'])}   base* prove ms {base}")
