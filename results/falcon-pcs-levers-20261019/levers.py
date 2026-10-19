"""One table over the lever campaigns (rates/, ext/, polylog/): per run the
median of the 3 sample proves, then the mean over the two input seeds.

    python3 levers.py [--json out.json]
"""
import json
import statistics
import sys
from pathlib import Path

ROOT = Path(__file__).parent

# (label, levers, campaign dir, run stem without the seed suffix)
ROWS = [
    ("UDR 1/2 (setup G)", "-", "ext", "udr1"),
    ("UDR 1/4", "A", "rates", "r2"),
    ("UDR 1/8", "A", "rates", "r3"),
    ("UDR 1/16", "A", "rates", "r4"),
    ("UDR 1/32", "A", "rates", "r5"),
    ("Johnson GF(2^256) 1/2", "B", "ext", "ext1"),
    ("Johnson GF(2^256) 1/4", "A+B", "ext", "ext2"),
    ("Johnson GF(2^256) 1/8", "A+B", "ext", "ext3"),
    ("Johnson GF(2^256) 1/16", "A+B", "ext", "ext4"),
    ("Johnson GF(2^256) 1/32", "A+B", "ext", "ext5"),
    ("UDR 1/2 + polylog", "C", "polylog", "poly-udrg14"),
    ("UDR 1/8 + polylog", "A+C", "polylog", "poly-udrg34"),
    ("Johnson 1/2 + polylog", "B+C", "polylog", "poly-ext14"),
    ("Johnson 1/8 + polylog", "A+B+C", "polylog", "poly-ext34"),
    ("Johnson 1/16 + polylog", "A+B+C", "polylog", "poly-ext44"),
    ("Johnson 1/32 + polylog", "A+B+C", "polylog", "poly-ext54"),
    ("Johnson 1/32 + polylog, folds 1/8", "A+B+C", "polylog", "poly-ext54-foldext34"),
    ("Johnson 1/32 + polylog, folds 1/128", "A+B+C", "polylog", "poly-ext54-foldext74"),
    ("UDR 1/2 (same binary as polylog)", "-", "polylog", "base-udrg14"),
    ("Johnson 1/32 (same binary as polylog)", "A+B", "polylog", "base-ext54"),
]


def rss_mib(path):
    err = path.with_suffix(".err")
    for line in err.read_text().splitlines() if err.exists() else []:
        if "maximum resident set size" in line:
            return int(line.split()[0]) / 2**20
    return float("nan")


def run(path):
    rows = [json.loads(l) for l in path.read_text().splitlines() if l.startswith("{")]
    rows = [r for r in rows if r.get("trial") == "sample"]
    if not rows:
        return None
    med = lambda k: statistics.median(r[k] for r in rows)
    parts = dict(rows[0]["proof_payload_breakdown"])
    return {
        "prove_ms": med("total_prover_ms"),
        "verify_ms": med("verify_ms"),
        "proof_bytes": med("proof_payload_bytes"),
        "pcs_bytes": parts.get("pcs", 0),
        "bridge_bytes": parts.get("bitz_bridge", 0),
        "rss_mib": rss_mib(path),
        "bridge_breakdown": rows[0].get("bridge_breakdown"),
        "pcs_breakdown": rows[0].get("pcs_breakdown"),
    }


table = []
for label, levers, campaign, stem in ROWS:
    runs = [run(p) for p in sorted((ROOT / campaign).glob(f"{stem}-s*.jsonl"))]
    runs = [r for r in runs if r]
    if not runs:
        continue
    mean = lambda k: statistics.fmean(r[k] for r in runs)
    table.append({
        "label": label, "levers": levers, "seeds": len(runs),
        **{k: mean(k) for k in ("prove_ms", "verify_ms", "proof_bytes", "pcs_bytes", "bridge_bytes", "rss_mib")},
        "prove_ms_runs": [r["prove_ms"] for r in runs],
        "bridge_breakdown": runs[0]["bridge_breakdown"],
        "pcs_breakdown": runs[0]["pcs_breakdown"],
    })

print(f"| configuration | levers | prove ms | verify ms | proof kB | PCS kB | bridge kB | RSS GiB |")
print("|---|---|---:|---:|---:|---:|---:|---:|")
for r in table:
    print(f"| {r['label']} | {r['levers']} | {r['prove_ms']:.0f} | {r['verify_ms']:.1f} | "
          f"{r['proof_bytes']/1000:.0f} | {r['pcs_bytes']/1000:.0f} | {r['bridge_bytes']/1000:.0f} | "
          f"{r['rss_mib']/1024:.1f} |")
if "--json" in sys.argv:
    Path(sys.argv[sys.argv.index("--json") + 1]).write_text(json.dumps(table, indent=1))
