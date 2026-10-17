"""Print the top of each profile tree and any unsymbolized frames.

usage: python3 -I show_tree.py <flame out dir> [min share] [max depth]
"""

import json
import sys
from pathlib import Path

OUT = Path(sys.argv[1])
MIN = float(sys.argv[2]) if len(sys.argv) > 2 else 0.012
DEPTH = int(sys.argv[3]) if len(sys.argv) > 3 else 6

for name in sys.argv[4].split(",") if len(sys.argv) > 4 else ["aerie", "bitz-shipped", "bitz-onelimb", "bitz-t16"]:
    path = OUT / f"{name}.json"
    if not path.exists():
        continue
    tree = json.loads(path.read_text())
    total = tree["value"]
    print(f"===== {name} {total:.0f} ms/prove")
    hexes = []

    def walk(node, depth):
        if node["name"].startswith("0x") or node["name"].startswith("?"):
            hexes.append((node["value"], node["name"]))
        if node["value"] >= MIN * total and depth <= DEPTH:
            print("  " * depth + f"{node['value']:7.0f} {node['name'][:96]}")
        for child in node["children"]:
            walk(child, depth + 1)

    walk(tree, 0)
    if hexes:
        print("   unsymbolized:", sorted(hexes, reverse=True)[:5], "total", round(sum(v for v, _ in hexes), 1))
