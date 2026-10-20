"""Falcon-512 recursion: the outer proof opened by standard BitZ (std, the
folds sent, T = 18/19 rows) against BitZ logarithmic at the reference split
(log, the folds' bits committed and proved), same binary, per (security,
batch, threads).

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
clock = {}
for line in (runs / "clock.log").read_text().splitlines():
    parts = line.split()
    if len(parts) > 2 and "=" in parts[2]:
        clock[parts[1]] = dict(p.split("=", 1) for p in parts[2:] if "=" in p)


def rec_row(name):
    path = runs / f"{name}.out"
    if not path.exists():
        return None
    lines = [l for l in path.read_text().splitlines() if l.startswith("TIMING ")]
    return json.loads(lines[-1][len("TIMING "):]) if lines else None


def kb(b):
    return f"{b / 1000:.1f}"


cells = [(sec, batch) for sec in (128, 100) for batch in (1024, 4096, 32768)]
for threads in (10, 1):
    print(f"{threads} thread{'s' if threads > 1 else ''}:")
    print()
    print("| λ | batch | rows std / log | π std → log | rec step std → log | final verify std → log | GHz std/log | RSS std/log |")
    print("|---|---|---|---|---|---|---|---|")
    for sec, batch in cells:
        key = f"s{sec}-b{batch}-t{threads}"
        std, log = rec_row(f"rec-std-{key}"), rec_row(f"rec-log-{key}")
        if std is None or log is None:
            continue
        cs, cl = clock.get(f"rec-std-{key}", {}), clock.get(f"rec-log-{key}", {})
        rows = f"{std['circuit'].get('outer_rows', '?')} / {log['circuit'].get('outer_rows', '?')}"
        row = [str(sec), f"{batch:,}", rows]
        row.append(f"{kb(std['pi_bytes'])} → {kb(log['pi_bytes'])} KB")
        row.append(f"{std['rec_prove_s']:.2f} → {log['rec_prove_s']:.2f} s")
        row.append(f"{std['final_verify_s']:.2f} → {log['final_verify_s']:.2f} s")
        row.append(f"{cs.get('ghz', '?')}/{cl.get('ghz', '?')}")
        row.append(f"{cs.get('rss', '?')}/{cl.get('rss', '?')}")
        print("| " + " | ".join(row) + " |")
    print()

print("Step breakdown (s): witness | record (overlaps) | early commit | passes wall | relation | outer witness | FlockRec prove | total; per rep")
for threads in (10, 1):
    for sec, batch in cells:
        for which in ("std", "log"):
            r = rec_row(f"rec-{which}-s{sec}-b{batch}-t{threads}")
            if r is None:
                continue
            reps = "; ".join(" ".join(f"{x:.2f}" for x in run) for run in r["rec_runs"])
            verify = " ".join(f"{x:.2f}" for x in r["verify_runs"])
            c = r["circuit"]
            print(f"  λ{sec} b{batch} t{threads} {which}: {reps}   verify {verify}   "
                  f"[outer f/h {c['outer_f_bits']}/{c['outer_h_bits']}, rows {c.get('outer_rows', '?')}, π {r['pi_bytes']} B]")
