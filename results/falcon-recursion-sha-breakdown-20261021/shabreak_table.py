"""Per-stage in-circuit SHA-256 compressions of the Falcon verifier circuit
from `[falcon record]` / `[falcon record opening]` stage lines.

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
