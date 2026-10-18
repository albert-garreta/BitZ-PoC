"""Grinding work actually spent per category (sum of found nonces) for bench runs.

    python3 nonces.py <run>.jsonl ...

The bench reports, per grinding category, the sum of the nonces it found
(`serial_nonce_prefix_sum`, the serial search work) and the number of
grinding boundaries. A difficulty-g site costs a geometric number of hashes
with mean 2^g; with fixed inputs every trial replays the same transcript, so
one run measures one draw per site.
"""
import json
import sys
from pathlib import Path

for name in sys.argv[1:]:
    path = Path(name)
    record = next(
        json.loads(line) for line in path.read_text().splitlines()
        if line.startswith("{") and json.loads(line).get("trial") == "sample"
    )
    print(f"== {path.stem}")
    for category in record["grinding_diagnostics"]["categories"]:
        total = int(category["serial_nonce_prefix_sum"])
        print(f"  {category['category']:22} {category['stored_nonce_boundaries']:5} sites  "
              f"{total:14,d} hashes ({total / 1e6:8.2f} M)")
