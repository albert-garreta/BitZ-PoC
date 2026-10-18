"""Seed-averaged results: mean over input seeds of each run's median prove time.

    python3 seed_means.py

Seeds 1..5 come from seeds/ (warm-up plus three proves each). Seed 42 is
taken from the pooled earlier campaigns (ladder/, rows/, tight/) when present.
Also prints the mean grinding work (sum of found nonces) per run.
"""
import json
import statistics
from pathlib import Path

ROOT = Path(__file__).parent
# Disturbed by another session (native check 24-29 ms instead of 18, commit
# 239 and 1,236 ms instead of about 80, all-core clock 3.2-3.3 GHz).
EXCLUDED = {"seeds/S4-s1", "seeds/S5-s1"}
SEED42 = {
    "S0": ["ladder/S0-t10-r1", "ladder/S0-t10-r2"],
    "S1": ["ladder/S1-t10-r1", "ladder/S1-t10-r2"],
    "S2": ["ladder/S2-t10-r1", "rows/default-t13-r1", "rows/default-t13-r2"],
    "S3": ["ladder/S3-t10-r1", "rows/default-t16-r2"],
    "S4": ["ladder/S4-t10-r1", "rows/small-t16-r1", "rows/small-t16-r2"],
    "S5": ["ladder/S5-t10-r1", "ladder/S5-t10-r2", "rows/small-t13-r1", "rows/small-t13-r2"],
    "T2": ["tight/tight-default-t13-r1", "tight/tight-default-t13-r2"],
    "T4": ["tight/tight-small-t16-r1", "tight/tight-small-t16-r2"],
    "T5": ["tight/tight-small-t13-r1", "tight/tight-small-t13-r2"],
}
LABELS = {
    "S0": "as built on macOS (union, reference binder)",
    "S1": "+ binder replay",
    "S2": "+ per-site grinding",
    "S3": "+ 2^16 bridge rows",
    "S4": "+ small prime (2^16 rows)",
    "S5": "per-site + small prime, 2^13 rows",
    "T2": "per-site, tight linear numerator",
    "T4": "tight: small prime, 2^16 rows",
    "T5": "tight: small prime, 2^13 rows",
}


def samples(stem):
    path = ROOT / f"{stem}.jsonl"
    if not path.exists():
        return []
    return [json.loads(line) for line in path.read_text().splitlines()
            if line.startswith("{") and json.loads(line).get("trial") == "sample"]


def work(record):
    return sum(int(c["serial_nonce_prefix_sum"]) for c in record["grinding_diagnostics"]["categories"])


print(f"{'config':4} {'label':46} {'seeds':>5} {'prove ms':>9} {'min':>7} {'max':>7} {'verify':>7} {'proof kB':>8} {'Mhash':>7}")
for key, label in LABELS.items():
    per_seed = []
    for seed in range(1, 6):
        if f"seeds/{key}-s{seed}" in EXCLUDED:
            continue
        runs = samples(f"seeds/{key}-s{seed}")
        if runs:
            per_seed.append(runs)
    pooled42 = [s for stem in SEED42[key] for s in samples(stem)]
    if pooled42:
        per_seed.append(pooled42)
    if not per_seed:
        continue
    prove = [statistics.median(s["total_prover_ms"] for s in runs) for runs in per_seed]
    verify = statistics.mean(statistics.median(s["proof_verify_ms"] for s in runs) for runs in per_seed)
    proof = statistics.mean(statistics.median(s["proof_payload_bytes"] for s in runs) for runs in per_seed)
    hashes = statistics.mean(work(runs[0]) for runs in per_seed) / 1e6
    print(f"{key:4} {label:46} {len(per_seed):5} {statistics.mean(prove):9.1f} {min(prove):7.1f} "
          f"{max(prove):7.1f} {verify:7.2f} {proof / 1000:8.1f} {hashes:7.1f}")
