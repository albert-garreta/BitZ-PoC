"""Per-stage in-circuit SHA-256 compressions of the Falcon verifier circuit
from `[falcon record]` / `[falcon record opening]` stage lines, then the
opening's Merkle compressions per level split into leaf hashes and tree nodes
(`[falcon record opening merkle]`, when the outputs have it).

usage: python3 -I shabreak_table.py <dir with shabreak-s{sec}-b{batch}.out | gplog-style outs> [prefix]
"""
import re
import sys
from pathlib import Path

d = Path(sys.argv[1])
prefix = sys.argv[2] if len(sys.argv) > 2 else "shabreak"
stage_re = re.compile(r"([A-Za-z0-9+ ]+?) [0-9.]+ sha (\d+)/(\d+)/(\d+)")


def stages(line):
    body = line.split("] ", 1)[1].rsplit(" | total", 1)[0]
    out = []
    for part in body.split(" | "):
        m = stage_re.fullmatch(part.strip())
        if m:
            out.append((m.group(1), int(m.group(2)), int(m.group(3)), int(m.group(4))))
    return out


rows = {}
cells = [(sec, b) for sec in (128, 100) for b in (1024, 4096, 32768)]
for sec, b in cells:
    text = (d / f"{prefix}-s{sec}-b{b}.out").read_text()
    main = [l for l in text.splitlines() if l.startswith("[falcon record] ")][-1]
    opening = [l for l in text.splitlines() if l.startswith("[falcon record opening] ")][-1]
    for name, tr, pw, mk in stages(main):
        if name == "opening":
            continue
        rows.setdefault((name, "transcript"), {})[(sec, b)] = tr
        rows.setdefault((name, "proof of work"), {})[(sec, b)] = pw
    for name, tr, pw, mk in stages(opening):
        key = f"opening: {name}"
        rows.setdefault((key, "transcript"), {})[(sec, b)] = tr
        rows.setdefault((key, "proof of work"), {})[(sec, b)] = pw
        rows.setdefault((key, "Merkle"), {})[(sec, b)] = mk

# Per stage, all kinds summed (the opening's recursive levels together), and
# the total split by kind.
groups = [
    ("keyed ring", ["keyed ring"]),
    ("norm sumcheck", ["norm"]),
    ("binder sumcheck", ["binder"]),
    ("bridge (product forest)", ["bridge"]),
    ("joint sumcheck", ["joint"]),
    ("opening: ring switch + L0 folds", ["opening: ring switch", "opening: L0 folds"]),
    ("opening: level 0", ["opening: L0 opening"]),
    ("opening: recursive levels", [n for n, _ in rows if n.startswith("opening: level ")]),
    ("opening: final level", ["opening: final level"]),
]
labels = " | ".join(f"λ{sec} {b // 1024}K" for sec, b in cells)
print(f"| stage | {labels} |")
print("|" + "---|" * (1 + len(cells)))
for label, names in groups:
    names = set(names)
    sums = {c: sum(v.get(c, 0) for (n, _), v in rows.items() if n in names) for c in cells}
    print(f"| {label} | " + " | ".join(f"{sums[c]:,}" for c in cells) + " |")
kinds = {k: {c: sum(v.get(c, 0) for (_, kk), v in rows.items() if kk == k) for c in cells} for k in ("transcript", "proof of work", "Merkle")}
grand = {c: sum(kinds[k][c] for k in kinds) for c in cells}
print("| **total** | " + " | ".join(f"**{grand[c]:,}**" for c in cells) + " |")
for k, vals in kinds.items():
    print(f"| of which {k} | " + " | ".join(f"{vals[c]:,} ({100 * vals[c] / grand[c]:.0f} %)" for c in cells) + " |")
print()

header = "| stage | kind | " + " | ".join(f"λ{sec} {b // 1024}K" for sec, b in cells) + " |"
print(header)
print("|" + "---|" * (2 + len(cells)))
totals = {c: 0 for c in cells}
for (name, kind), vals in rows.items():
    if all(vals.get(c, 0) == 0 for c in cells):
        continue
    for c in cells:
        totals[c] += vals.get(c, 0)
    print(f"| {name} | {kind} | " + " | ".join(str(vals.get(c, 0)) for c in cells) + " |")
print("| **total** | | " + " | ".join(f"**{totals[c]}**" for c in cells) + " |")

merkle_re = re.compile(r"(.+?) (\d+) rows x (\d+): leaves (\d+) nodes (\d+)")
detail = {}
for sec, b in cells:
    text = (d / f"{prefix}-s{sec}-b{b}.out").read_text()
    lines = [l for l in text.splitlines() if l.startswith("[falcon record opening merkle] ")]
    if not lines:
        continue
    for part in lines[-1].split("] ", 1)[1].split(" | "):
        m = merkle_re.fullmatch(part.strip())
        if m:
            name, rows_, width, leaves, nodes = m.group(1), *map(int, m.groups()[1:])
            detail.setdefault(name, {})[(sec, b)] = (rows_, width, leaves, nodes)
if detail:
    print()
    print("Merkle per level: opened rows x row elements, leaf + node compressions")
    print()
    print("| level | " + " | ".join(f"λ{sec} {b // 1024}K" for sec, b in cells) + " |")
    print("|" + "---|" * (1 + len(cells)))
    order = sorted(detail, key=lambda n: (n == "final level", n))
    for name in order:
        vals = detail[name]
        cell = lambda c: f"{vals[c][0]}x{vals[c][1]}: {vals[c][2]} + {vals[c][3]}" if c in vals else "–"
        print(f"| {name} | " + " | ".join(cell(c) for c in cells) + " |")
    leaves = {c: sum(v[c][2] for v in detail.values() if c in v) for c in cells}
    nodes = {c: sum(v[c][3] for v in detail.values() if c in v) for c in cells}
    print("| **leaves + nodes** | " + " | ".join(f"**{leaves[c]} + {nodes[c]}**" for c in cells) + " |")
