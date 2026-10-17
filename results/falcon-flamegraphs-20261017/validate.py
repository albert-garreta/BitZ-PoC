"""Check sampled prover profiles against the harnesses' own wall-clock timers.

usage: python3 -I validate.py <dir with NAME.out and NAME.samply.folded or out/NAME.folded>
"""

import json
import sys
from pathlib import Path

D = Path(sys.argv[1])


def region_ms(folded: Path, marker: str) -> float:
    total = 0
    for line in folded.read_text().splitlines():
        stack, _, count = line.rpartition(" ")
        if marker in stack:
            total += int(count)
    return total / 1000.0


for name in ["aerie", "bitz-shipped", "bitz-onelimb", "bitz-t16"]:
    folded = D / f"{name}.samply.folded"
    out = D / f"{name}.out"
    if not folded.exists() or not out.exists():
        continue
    if name == "aerie":
        rows, header = [], None
        for line in out.read_text().splitlines():
            if line.startswith("records\t"):
                header = line.split("\t")
            elif header and line[:1].isdigit():
                rows.append(dict(zip(header, line.split("\t"))))
        wall = sum(float(r["prove_ms"]) for r in rows)
        cpu = region_ms(folded, "profile_prove_region")
        print(f"{name}: {len(rows)} proves, wall {wall / len(rows):.0f} ms/prove, "
              f"cpu {cpu / len(rows):.0f} ms/prove, cpu/wall {cpu / wall:.3f}")
        continue
    rows = [json.loads(line) for line in out.read_text().splitlines() if line.startswith("{")]
    wall = sum(r["total_prover_ms"] for r in rows)
    commit_wall = sum(r["witness_commit_ms"] for r in rows)
    cpu_commit = region_ms(folded, "profile_commit_region")
    cpu_prove = region_ms(folded, "profile_prove_region")
    cpu = cpu_commit + cpu_prove
    print(f"{name}: {len(rows)} proves {[round(r['total_prover_ms']) for r in rows]}, "
          f"wall {wall / len(rows):.0f} ms/prove, cpu {cpu / len(rows):.0f} ms/prove, cpu/wall {cpu / wall:.3f}")
    print(f"   commit share: timer {100 * commit_wall / wall:.2f}%, cpu-weighted {100 * cpu_commit / cpu:.2f}%")
