"""The PCS-lever numbers for the flamegraph page: writes page.json next to this
file. Per configuration: the mean over input seeds of each seed's median
prove, verify and proof size, the seed range of prove, and the proof split
into the size panel's categories.
"""
import json
import statistics
from pathlib import Path

ROOT = Path(__file__).parent

# (name, detail, levers, campaign, run stem)
CONFIGS = [
    ("UDR, rate 1/2", "setup G as measured before", "–", "ext", "udr1"),
    ("UDR, rate 1/8", "smaller rate only", "A", "rates", "r3"),
    ("Johnson in GF(2^256), rate 1/2", "larger field only", "B", "ext", "ext1"),
    ("UDR 1/2 + polylog bridge", "polylog only · folds at 1/32", "C", "polylog", "poly-udrg14"),
    ("Johnson in GF(2^256), rate 1/8", "", "A+B", "ext", "ext3"),
    ("Johnson in GF(2^256), rate 1/32", "", "A+B", "ext", "ext5"),
    ("UDR 1/8 + polylog bridge", "", "A+C", "polylog", "poly-udrg34"),
    ("Johnson 1/2 + polylog bridge", "", "B+C", "polylog", "poly-ext14"),
    ("Johnson 1/8 + polylog bridge", "", "A+B+C", "polylog", "poly-ext34"),
    ("Johnson 1/16 + polylog bridge", "", "A+B+C", "polylog", "poly-ext44"),
    ("Johnson 1/32 + polylog bridge", "", "A+B+C", "polylog", "poly-ext54"),
]
# The size panel's categories and palette slots, in its order.
PARTS = [
    ("Coefficient exception counts", 1, lambda p: p["coefficient_exceptions"]),
    ("Commitment", 3, lambda p: p["commitment"]),
    ("Ring and norm messages", 4, lambda p: p["ring"] + p["norm_and_binding"]),
    ("Bit-to-integer bridge", 5, lambda p: p["bitz_bridge"]),
    ("PCS opening", 6, lambda p: p["pcs"] + p["binary_sumcheck"]),
]


def samples(campaign, stem):
    runs = []
    for path in sorted((ROOT / campaign).glob(f"{stem}-s*.jsonl")):
        rows = [json.loads(l) for l in path.read_text().splitlines() if l.startswith("{")]
        rows = [r for r in rows if r.get("trial") == "sample"]
        if rows:
            runs.append(rows)
    return runs


rows = []
for name, detail, levers, campaign, stem in CONFIGS:
    runs = samples(campaign, stem)
    if not runs:
        continue
    med = lambda run, key: statistics.median(r[key] for r in run)
    proves = [med(run, "total_prover_ms") for run in runs]
    parts = dict(runs[0][0]["proof_payload_breakdown"])
    rows.append({
        "name": name, "sub": detail, "levers": levers, "seeds": len(runs),
        "prove": statistics.fmean(proves), "min": min(proves), "max": max(proves),
        "verify": statistics.fmean(med(run, "verify_ms") for run in runs),
        "proof": statistics.fmean(med(run, "proof_payload_bytes") for run in runs),
        "cats": [f(parts) for _, _, f in PARTS],
        "bridge": runs[0][0].get("bridge_breakdown"),
    })

notes_file = ROOT / "page_notes.json"
page = {
    "figure": "M5 · 10 threads · mean of two input seeds",
    "subtitle": (
        "Algebraic Falcon-512, 32,768 signatures, 128-bit, setup G (keyed ring, small prime, "
        "2^16 bridge rows, per-site grinding). A: smaller Reed-Solomon rate. B: Johnson list "
        "decoding with every Ligerito challenge in GF(2^256), so Round 0 reaches 128 bits. "
        "C: polylog BitZ bridge, which commits the bridge's column sums instead of sending them."
    ),
    "parts": [{"name": n, "slot": s} for n, s, _ in PARTS],
    "rows": rows,
    "notes": json.loads(notes_file.read_text()) if notes_file.exists() else [],
}
(ROOT / "page.json").write_text(json.dumps(page, indent=1))
for row in rows:
    print(f"{row['levers']:6} {row['name']:34} {row['prove']:7.0f} ms ({row['min']:.0f}-{row['max']:.0f}) "
          f"{row['verify']:5.1f} ms {row['proof'] / 1e3:6.1f} kB {row['cats']}")
