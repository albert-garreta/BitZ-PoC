"""Full-Falcon numbers for the flamegraph page: writes page.json next to this file.

Prove, verify, proof size and grinding work per configuration, as means over
input seeds of each seed's median (seeds 1..5 from seeds/, seed 42 pooled from
the earlier campaigns), plus the proof composition of three configurations.
"""
import json
import statistics
from pathlib import Path

ROOT = Path(__file__).parent
EXCLUDED = {"seeds/S4-s1", "seeds/S5-s1"}  # disturbed, see README
SEED42 = {
    "S0": ["ladder/S0-t10-r1", "ladder/S0-t10-r2"],
    "S1": ["ladder/S1-t10-r1", "ladder/S1-t10-r2"],
    "S2": ["ladder/S2-t10-r1", "rows/default-t13-r1", "rows/default-t13-r2"],
    "S3": ["ladder/S3-t10-r1", "rows/default-t16-r2"],
    "S4": ["ladder/S4-t10-r1", "rows/small-t16-r1", "rows/small-t16-r2"],
    "S5": ["ladder/S5-t10-r1", "ladder/S5-t10-r2", "rows/small-t13-r1", "rows/small-t13-r2"],
    "T4": ["tight/tight-small-t16-r1", "tight/tight-small-t16-r2"],
    "T5": ["tight/tight-small-t13-r1", "tight/tight-small-t13-r2"],
}
# (key, name, detail, kind) with kind "base" or "tight" (accounting experiment).
CONFIGS = [
    ("S0", "As built on macOS", "union grinding · reference binder", "base"),
    ("S1", "+ optimized binder", "BITZ_BENCH_BINDER_REPLAY=on", "base"),
    ("S2", "+ per-site grinding", "John's prime · 2^13 bridge rows", "base"),
    ("S3", "+ 2^16 bridge rows", "smallest proof at John's prime", "base"),
    ("S4", "+ small prime", "2^16 bridge rows", "base"),
    ("S5", "Small prime, 2^13 rows", "per-site · optimized binder", "base"),
    ("T5", "Tight numerator, small prime, 2^13 rows", "fastest · accounting experiment", "tight"),
    ("T4", "Tight numerator, small prime, 2^16 rows", "accounting experiment", "tight"),
]
BREAKDOWN = [("S0", "ladder/S0-t10-r1"), ("S3", "ladder/S3-t10-r1"), ("T5", "tight/tight-small-t13-r1")]
PARTS = [
    ("Ligerito opening", lambda k: k.startswith("pcs_") or k.startswith("opening_") or k == "source_authentication_joint"),
    ("HashToPoint selection masks", lambda k: k == "hash_to_point_selection_masks"),
    ("Bridge column sums", lambda k: k.startswith("integer_column_sums")),
    ("Bridge GKR and nonces", lambda k: k.startswith("binary_bridge")),
    ("Keccak (SHAKE256)", lambda k: k == "keccak_piops"),
    ("Ring check", lambda k: k == "ring_certificate_and_reduction"),
    ("Arithmetic, joint sumcheck, links", lambda k: True),
]


def samples(stem):
    path = ROOT / f"{stem}.jsonl"
    if not path.exists():
        return []
    rows = [json.loads(line) for line in path.read_text().splitlines() if line.startswith("{")]
    return [r for r in rows if r.get("trial") == "sample"]


def work(record):
    return sum(int(c["serial_nonce_prefix_sum"]) for c in record["grinding_diagnostics"]["categories"])


rows = []
for key, name, detail, kind in CONFIGS:
    per_seed = [samples(f"seeds/{key}-s{seed}") for seed in range(1, 6) if f"seeds/{key}-s{seed}" not in EXCLUDED]
    per_seed = [runs for runs in per_seed if runs]
    pooled = [s for stem in SEED42[key] for s in samples(stem)]
    if pooled:
        per_seed.append(pooled)
    prove = [statistics.median(s["total_prover_ms"] for s in runs) for runs in per_seed]
    rows.append({
        "key": key, "name": name, "sub": detail, "kind": kind, "seeds": len(per_seed),
        "prove": round(statistics.mean(prove), 1), "min": round(min(prove), 1), "max": round(max(prove), 1),
        "verify": round(statistics.mean(statistics.median(s["proof_verify_ms"] for s in runs) for runs in per_seed), 2),
        "proof": round(statistics.mean(statistics.median(s["proof_payload_bytes"] for s in runs) for runs in per_seed)),
        "mhash": round(statistics.mean(work(runs[0]) for runs in per_seed) / 1e6, 1),
    })

names = {key: name for key, name, _, _ in CONFIGS}
breakdown = []
for key, stem in BREAKDOWN:
    record = samples(stem)[0]
    parts = dict(record["proof_payload_breakdown"])
    cats = [0] * len(PARTS)
    for part, size in parts.items():
        index = next(i for i, (_, test) in enumerate(PARTS) if test(part))
        cats[index] += size
    assert sum(cats) == record["proof_payload_bytes"], (stem, sum(cats), record["proof_payload_bytes"])
    breakdown.append({"key": key, "name": names[key], "total": record["proof_payload_bytes"], "cats": cats})

one_thread = {stem: statistics.median(s["total_prover_ms"] for s in samples(stem))
              for stem in ["ladder/S0-t1", "ladder/S2-t1", "ladder/S3-t1"]}
verify_one = statistics.median(s["proof_verify_ms"] for s in samples("ladder/S2-t1"))
seconds = lambda ms: f"{ms / 1000:.2f} s"
notes = [
    "Full Falcon is John's <code>falcon_hybrid</code> prover. Besides the ring and norm it proves SHAKE256 "
    "HashToPoint, and its statement is the public keys, messages and signatures, so Aerie has no counterpart here.",
    "Each bar is the mean over input seeds of the median prove for that seed, and the line spans the lowest and "
    "highest seed. Two configurations have five seeds because one run each was disturbed by another session. With "
    "fixed inputs every prove replays one transcript, so each large grinding site is a single draw. With the small "
    "prime at 2^16 rows, the binder's grinding (its 28-bit linear site and 26 rounds) took 60M to 966M hashes "
    "depending on the seed, against about 275M expected.",
    "The tight numerator (<code>BITZ_FALCON_LINEAR_NUMERATOR=tight</code>) counts the linear block's grinding "
    "numerator as m + d + 6 instead of m + d + B + 5. The block draws one multilinear point over rows and "
    "signatures, η and one merge coefficient, so the batch size B should not enter. It stays an accounting "
    "experiment until John confirms this.",
    "The optimized binder replay is on by default only on Linux x86-64, so on the M5 it needs "
    "<code>BITZ_BENCH_BINDER_REPLAY=on</code>. Per-site grinding, the bridge rows and the small prime use the same "
    "switches as the algebraic prover.",
    f"On one thread (one seed) the hybrid proves in {seconds(one_thread['ladder/S0-t1'])} as built, "
    f"{seconds(one_thread['ladder/S2-t1'])} with per-site grinding and {seconds(one_thread['ladder/S3-t1'])} with "
    f"2^16 rows, and verifies in {verify_one:.0f} ms. Peak memory at 10 threads is 2.4 to 2.5 GiB in every configuration.",
]
page = {
    "figure": "M5 · 10 threads · mean over input seeds",
    "subtitle": "128-bit security. Prove includes the witness commitment.",
    "rows": rows,
    "parts": [name for name, _ in PARTS],
    "breakdown": breakdown,
    "notes": notes,
}
(ROOT / "page.json").write_text(json.dumps(page, indent=1))
for row in rows:
    print(row)
for row in breakdown:
    print(row)
for note in notes:
    print("-", note)
