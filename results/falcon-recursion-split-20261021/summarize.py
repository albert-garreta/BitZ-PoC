"""The recursion step with the split relation (SHA batch + glue on one
commitment, BITZ_REC_SPLIT=1) against the FlockRec step, both with the ext:3:4
standard-terminal base proof and the ext:3:4 outer ladder, λ=128, 10 threads.

FlockRec t<T>     = the FlockRec step, standard opener at T rows
split <S>-<G>     = the split step, the batch's claim rows S, the glue's G
SHA witness       = the batch's witness generation (every compression's
                    trace; reported apart, inside rec step)
prove             = the protocol alone (FlockRec prove; split: commitment
                    excluded, both PIOPs and the shared opening)
rec step          = witness + recording (concurrent) + relation + witness
                    (+ the split's commitment) + prove (the harness's median
                    rep; at 32K with two reps, the slower one)
step − SHA wit.   = rec step without the batch's witness generation
final verify      = the recording replayed from the public stream + relation
                    + verify

usage: python3 summarize.py <runs dir>
"""
import json
import sys
from pathlib import Path

runs = Path(sys.argv[1])
CONFIGS = (
    ("flockrec-t18", "FlockRec t18"),
    ("flockrec-t17", "FlockRec t17"),
    ("flockrec-t16", "FlockRec t16"),
    ("flockrec-t15", "FlockRec t15"),
    ("split-17-15", "split 17-15"),
    ("split-16-15", "split 16-15"),
)


def row(name):
    path = runs / f"{name}.out"
    if not path.exists():
        return None
    lines = [l for l in path.read_text().splitlines() if l.startswith("TIMING ")]
    return json.loads(lines[-1][len("TIMING "):]) if lines else None


print("| batch | step | blocks | SHA witness | prove | rec step | step − SHA wit. | final verify | π | π + public |")
print("|---|---|---|---|---|---|---|---|---|---|")
for batch in (1024, 4096, 32768):
    for name, label in CONFIGS:
        r = row(f"{name}-b{batch}")
        if r is None:
            continue
        c = r["circuit"]
        public = r["public_stream_bytes"] + r["public_input_bytes"]
        split = r.get("mode") == "split"
        sha = f"{r['rec_sha_witness_s']:.2f} s" if split else "—"
        protocol = r["rec_protocol_s"]
        ex = r["rec_prove_ex_sha_witness_s"] if split else r["rec_prove_s"]
        print(
            f"| {batch:,} | {label} | {c['blocks']:,} | {sha} | {protocol:.2f} s | {r['rec_prove_s']:.2f} s "
            f"| {ex:.2f} s | {r['final_verify_s']:.2f} s | {r['pi_bytes'] / 1000:.1f} KB | {(r['pi_bytes'] + public) / 1000:.1f} KB |"
        )
print()
print("Split shapes and proof parts:")
for batch in (1024, 4096, 32768):
    for name, label in CONFIGS:
        r = row(f"{name}-b{batch}")
        if r is None or r.get("mode") != "split":
            continue
        c = r["circuit"]
        print(
            f"  b{batch} {label}: batch 2^{c['sha_log_batch']}, tail {c['tail_hints']:,} bits, shared grid 2^{c['shared_f_bits']}, "
            f"glue grid 2^{c['glue_h_bits']} (rows {c['glue_rows']}); opening {r['pi_opening_bytes'] / 1000:.1f} KB, "
            f"SHA PIOP {r['pi_sha_piop_bytes']} B, glue PIOP {r['pi_glue_piop_bytes']} B"
        )
print()
print("Per rep (s):")
for batch in (1024, 4096, 32768):
    for name, label in CONFIGS:
        r = row(f"{name}-b{batch}")
        if r is None:
            continue
        if r.get("mode") == "split":
            head = "witness record passes relation glue-wit SHA-wit rows commit prove total total-SHA"
            reps = "; ".join(" ".join(f"{x:.2f}" for x in run) for run in r["split_runs"])
        else:
            head = "witness record early passes relation outer-wit prove total"
            reps = "; ".join(" ".join(f"{x:.2f}" for x in run) for run in r["rec_runs"])
        print(f"  b{batch} {label} [{head}]: {reps}   verify {' '.join(f'{x:.2f}' for x in r['verify_runs'])}")
