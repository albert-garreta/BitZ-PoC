"""Falcon-512 recursion campaign: one markdown row per (security, batch, threads).

base  = falcon_algebraic, production Blake3 transcript (medians of its samples)
base* = the sha256-fs base proof the recursion verifies (falcon_recursion's own
        base runs; same relation, SHA-256 transcript, PoW and Merkle trees)
rec   = one recursion step: witness + recording + outer relation + FlockRec
        prove (the timed total), and the final verifier (recording replayed
        from the public stream + relation + FlockRec verify).
"""
import json
import statistics
import sys
from pathlib import Path

runs = Path(sys.argv[1])
clock = {}
for line in (runs / "clock.log").read_text().splitlines():
    parts = line.split()
    if len(parts) > 2:
        fields = dict(p.split("=", 1) for p in parts[2:] if "=" in p)
        clock[parts[1]] = fields


def base_row(name):
    path = runs / f"{name}.jsonl"
    if not path.exists():
        return None
    rows = [json.loads(l) for l in path.read_text().splitlines() if l.startswith("{")]
    samples = [r for r in rows if r.get("trial") == "sample"]
    if not samples:
        return None
    med = lambda key: statistics.median(r[key] for r in samples)
    last = samples[-1]
    parts = dict(last["proof_payload_breakdown"])
    return {
        "prover_ms": med("total_prover_ms"),
        "verify_ms": med("verify_ms"),
        "bytes": last["proof_payload_bytes"],
        "exceptions": parts.get("coefficient_exceptions", 0),
        "digest": last["proof_debug_digest"],
        "n": len(samples),
    }


def rec_row(name):
    path = runs / f"{name}.out"
    if not path.exists():
        return None
    lines = [l for l in path.read_text().splitlines() if l.startswith("TIMING ")]
    return json.loads(lines[-1][len("TIMING "):]) if lines else None


def kb(b):
    return f"{b / 1000:.1f}"


print("| λ | batch | thr | base prove (Blake3) | base* prove (SHA) | rec step | base verify | base* verify | final verify "
      "| base proof | base* proof | π | public | rec total | GHz rec/base | RSS rec |")
print("|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|")
for threads in (10, 1):
    for sec in (128, 100):
        for batch in (1024, 4096, 32768):
            b = base_row(f"base-s{sec}-b{batch}-t{threads}")
            r = rec_row(f"rec-s{sec}-b{batch}-t{threads}")
            if b is None and r is None:
                continue
            c_rec = clock.get(f"rec-s{sec}-b{batch}-t{threads}", {})
            c_base = clock.get(f"base-s{sec}-b{batch}-t{threads}", {})
            cells = [str(sec), str(batch), str(threads)]
            cells.append(f"{b['prover_ms']:.0f} ms" if b else "–")
            cells.append(f"{r['base_prover_s'] * 1e3:.0f} ms" if r else "–")
            cells.append(f"{r['rec_prove_s']:.2f} s" if r else "–")
            cells.append(f"{b['verify_ms']:.1f} ms" if b else "–")
            cells.append(f"{r['base_verify_s'] * 1e3:.1f} ms" if r else "–")
            cells.append(f"{r['final_verify_s']:.2f} s" if r else "–")
            cells.append(f"{kb(b['bytes'])} KB" if b else "–")
            cells.append(f"{kb(r['base_proof_bytes'])} KB" if r else "–")
            cells.append(f"{kb(r['pi_bytes'])} KB" if r else "–")
            cells.append(f"{kb(r['public_stream_bytes'] + r['public_input_bytes'])} KB" if r else "–")
            cells.append(f"{kb(r['pi_bytes'] + r['public_stream_bytes'] + r['public_input_bytes'])} KB" if r else "–")
            cells.append(f"{c_rec.get('ghz', '?')}/{c_base.get('ghz', '?')}")
            cells.append(c_rec.get("rss", "?"))
            print("| " + " | ".join(cells) + " |")

print()
print("Recursion step breakdown (s): witness | record (overlaps) | early commit | passes wall | relation | outer witness | FlockRec prove")
for threads in (10, 1):
    for sec in (128, 100):
        for batch in (1024, 4096, 32768):
            r = rec_row(f"rec-s{sec}-b{batch}-t{threads}")
            if r is None:
                continue
            c = r["circuit"]
            print(
                f"  λ{sec} b{batch} t{threads}: {r['rec_witness_s']:.2f} | {r['rec_record_s']:.2f} | {r['rec_early_commit_s']:.2f} | "
                f"{r['rec_passes_s']:.2f} | {r['rec_relation_s']:.2f} | {r['rec_outer_witness_s']:.2f} | {r['rec_protocol_s']:.2f}"
                f"   circuit: {c['blocks']} SHA blocks, {c['committed'] / 1e6:.1f} M committed bits, {c['cells'] / 1e6:.0f} M cells, "
                f"outer f/h bits {c['outer_f_bits']}/{c['outer_h_bits']}; public top {r['public_top'][:4]}"
            )

print()
print("Base proof digests (Blake3, proof_debug_digest):")
for sec in (128, 100):
    for batch in (1024, 4096, 32768):
        b = base_row(f"base-s{sec}-b{batch}-t10")
        if b:
            print(f"  λ{sec} b{batch}: {b['digest']} ({b['bytes']} B, exception counts {b['exceptions']} B)")
