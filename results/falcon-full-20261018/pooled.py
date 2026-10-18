"""Pooled medians per configuration over the clean runs of the 1024 campaigns.

Excluded (disturbed by another session's unlocked work; the native Falcon check,
which none of these changes touch, ran at 23-26 ms instead of 18 ms, or the
all-core clock fell to 3.0-3.3 GHz): ladder S3-t10-r2, S4-t10-r2, and
rows default-t16-r1 (prove 364 ms against 327-329 in the three other runs).
"""
import json
import statistics
from pathlib import Path

ROOT = Path(__file__).parent
CONFIGS = {
    "S0 as built on macOS": ["ladder/S0-t10-r1", "ladder/S0-t10-r2"],
    "S1 + optimized binder": ["ladder/S1-t10-r1", "ladder/S1-t10-r2"],
    "S2 + per-site grinding": ["ladder/S2-t10-r1", "ladder/S2-t10-r2", "rows/default-t13-r1", "rows/default-t13-r2"],
    "S3 + 2^16 bridge rows": ["ladder/S3-t10-r1", "rows/default-t16-r2"],
    "S4 + small prime (2^16 rows)": ["ladder/S4-t10-r1", "rows/small-t16-r1", "rows/small-t16-r2"],
    "small prime, 2^15 rows": ["rows/small-t15-r1", "rows/small-t15-r2"],
    "small prime, 2^14 rows": ["rows/small-t14-r1", "rows/small-t14-r2"],
    "S5 small prime, 2^13 rows": ["ladder/S5-t10-r1", "ladder/S5-t10-r2", "rows/small-t13-r1", "rows/small-t13-r2"],
    "one thread S0": ["ladder/S0-t1"],
    "one thread S2": ["ladder/S2-t1"],
    "one thread S3": ["ladder/S3-t1"],
    "one thread S4": ["ladder/S4-t1"],
    "one thread S5": ["rows/small-t13-t1"],
}


def samples(stem):
    for line in (ROOT / f"{stem}.jsonl").read_text().splitlines():
        if line.startswith("{"):
            record = json.loads(line)
            if record.get("trial") == "sample":
                yield record


def rss_gib(stem):
    for line in (ROOT / f"{stem}.err").read_text().splitlines():
        if "maximum resident set size" in line:
            return int(line.split()[0]) / 2**30
    return float("nan")


print(f"{'configuration':32} {'n':>3} {'prover ms':>10} {'verify ms':>10} {'proof B':>9} {'RSS GiB':>8}")
for name, stems in CONFIGS.items():
    pool = [s for stem in stems for s in samples(stem)]
    prover = statistics.median(s["total_prover_ms"] for s in pool)
    verify = statistics.median(s["proof_verify_ms"] for s in pool)
    proof = statistics.median(s["proof_payload_bytes"] for s in pool)
    rss = max(rss_gib(stem) for stem in stems)
    print(f"{name:32} {len(pool):3d} {prover:10.1f} {verify:10.2f} {proof:9.0f} {rss:8.2f}")
