"""Polylog outer-proof byte breakdown per cell from traced runs.

usage: python3 -I gplog_table.py <dir with gplog-s{sec}-b{batch}.out>
"""
import json
import re
import sys
from pathlib import Path

d = Path(sys.argv[1])
parts = ["log head", "log gkr h", "log gkr fold bits", "log rec folds", "log rec gkr", "log open f", "log open fold bits"]
print("| λ | batch | ℓ₁ × ℓ₂ | GKR h | fold-bit tree | recursive (folds + GKR) | open f | open fold bits | π |")
print("|---|---|---|---|---|---|---|---|---|")
for sec in (128, 100):
    for b in (1024, 4096, 32768):
        text = (d / f"gplog-s{sec}-b{b}.out").read_text()
        got = {}
        for p in parts:
            found = re.findall(rf"^bitz: bytes {re.escape(p)}\s+(\d+)$", text, re.M)
            got[p] = int(found[-1]) if found else 0
        timing = json.loads(re.findall(r"^TIMING (.*)$", text, re.M)[-1])
        c = timing["circuit"]
        t, h = c["outer_rows"], c["outer_h_bits"]
        kb = lambda x: f"{x / 1000:.1f} KB"
        rec = got["log rec folds"] + got["log rec gkr"]
        print(f"| {sec} | {b:,} | 2^{t} × 2^{h - t} | {kb(got['log gkr h'])} | {kb(got['log gkr fold bits'])} | {kb(rec)} "
              f"| {kb(got['log open f'])} | {kb(got['log open fold bits'])} | {kb(timing['pi_bytes'])} |")
