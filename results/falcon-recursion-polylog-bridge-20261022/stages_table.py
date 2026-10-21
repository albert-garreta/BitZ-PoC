"""Per-stage in-circuit SHA-256 compressions (transcript + proof of work +
Merkle) of the Falcon circuit, standard against polylog bridge, from the
`[falcon record]` lines of stages.sh.

usage: python3 stages_table.py <dir with stages-{bridge}-b{batch}.out>
"""
import re
import sys
from pathlib import Path

d = Path(sys.argv[1])
stage_re = re.compile(r"(.+?) [0-9.]+ sha (\d+)/(\d+)/(\d+)")
order = [
    "statement", "keyed ring", "norm", "binder", "bridge", "polylog forests", "polylog read-off",
    "polylog joint", "polylog opening", "joint", "opening",
]
cells = [(bridge, b) for b in (1024, 4096, 32768) for bridge in ("standard", "polylog")]
table = {}
for bridge, b in cells:
    text = (d / f"stages-{bridge}-b{b}.out").read_text()
    line = [l for l in text.splitlines() if l.startswith("[falcon record] ")][-1]
    body = line.split("] ", 1)[1].rsplit(" | total", 1)[0]
    for part in body.split(" | "):
        m = stage_re.fullmatch(part.strip())
        name, tr, pw, mk = m.group(1), int(m.group(2)), int(m.group(3)), int(m.group(4))
        table.setdefault(name, {})[(bridge, b)] = (tr, pw, mk)

print("| stage | " + " | ".join(f"{bridge} {b // 1024}K" for bridge, b in cells) + " |")
print("|---|" + "---|" * len(cells))
totals = {c: [0, 0, 0] for c in cells}
for name in order:
    if name not in table:
        continue
    row = []
    for c in cells:
        v = table[name].get(c)
        if v is None or sum(v) == 0:
            row.append("–")
            continue
        for k in range(3):
            totals[c][k] += v[k]
        row.append(f"{sum(v):,}")
    print(f"| {name} | " + " | ".join(row) + " |")
print("| **total** | " + " | ".join(f"**{sum(totals[c]):,}**" for c in cells) + " |")
for k, kind in enumerate(("transcript", "proof of work", "Merkle")):
    print(f"| of which {kind} | " + " | ".join(f"{totals[c][k]:,}" for c in cells) + " |")
