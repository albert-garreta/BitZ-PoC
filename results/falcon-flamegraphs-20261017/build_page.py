"""Build the flamegraph page from flame.py's folded stacks.

usage: python3 -I build_page.py <flame out dir> <template.html> <index.html>

Each frame gets a "kind of work" for coloring: proof-of-work wherever a
grinding frame is on the path, otherwise the shallowest frame on the path that
matches one of the profile's phase rules (frames above the phases are Other).
"""

import json
import re
import sys
from pathlib import Path

OUT = Path(sys.argv[1])
TEMPLATE = Path(sys.argv[2])
INDEX = Path(sys.argv[3])

CATEGORIES = [
    "Witness preparation",
    "Signature decoding",
    "Commitment",
    "Ring and norm arguments",
    "Bit-to-integer bridge (GKR forest)",
    "PCS opening",
    "Proof-of-work grinding",
    "Other",
]
WITNESS, DECODE, COMMIT, ARGS, BRIDGE, PCS, POW, OTHER = range(8)

# Phase rules: the shallowest matching frame on a stack decides.
BITZ_RULES = [
    (WITNESS, r"^WitnessData::"),
    (COMMIT, r"^PreparedFalconAlgebraic::commit_(data|packed)$"),
    (ARGS, r"^(arithmetic|ring|keyed_ring|word_binding|falcon_integer)::|^(BindingForm|KeyedRing)\w*::"),
    (BRIDGE, r"^integer_bridge::"),
    (PCS, r"^(opening::prove_joint|joint_sumcheck::|ligerito::)"),
]
AERIE_RULES = [
    (WITNESS, r"^transport::prepare_witness$"),
    (COMMIT, r"^commit::(precommit_s2_groups|commit_group)$"),
    (ARGS, r"^(stage[123]::(prepare|prove_prepared)|projection::project)$"),
    (PCS, r"^opening::open_groups$"),
]
# Overrides: any matching frame on a stack decides, wherever it sits.
OVERRIDES = [
    (POW, re.compile(r"[Gg]rinding|pow_nonce|grind_pow|proof_of_work")),
    (DECODE, re.compile(r"^source::decode_(signature_coefficients|compressed)$")),
]

PROFILES = [
    ("aerie", "Aerie v0.2.0", "as published · no LTO", AERIE_RULES),
    ("bitz-shipped", "BitZ, falcon-poc", "as shipped · two-limb bridge", BITZ_RULES),
    ("bitz-t16", "BitZ, experiment", "keyed ring · one-limb bridge, 2^16 rows · per-site grinding", BITZ_RULES),
]
MIN_MS = 0.3  # prune frames below this (ms per prove); their time stays in the parent


def build(name, rules, summary):
    folded = (OUT / f"{name}.folded").read_text().splitlines()
    total = summary["samples"]
    scale = summary["ms_per_prove"] / total
    compiled = [(cat, re.compile(rx)) for cat, rx in rules]
    root = {"n": None, "v": 0.0, "c": OTHER, "k": {}}
    per_cat = [0.0] * len(CATEGORIES)
    for line in folded:
        stack, _, count = line.rpartition(" ")
        ms = int(count) * scale
        frames = stack.split(";")
        node = root
        node["v"] += ms
        phase, override = None, None
        for depth, frame in enumerate(frames):
            if depth > 0:
                override = next((cat for cat, rx in OVERRIDES if rx.search(frame)), override)
                if phase is None:
                    phase = next((cat for cat, rx in compiled if rx.search(frame)), None)
            cat = override if override is not None else (phase if phase is not None else OTHER)
            child = node["k"].setdefault(frame, {"n": frame, "v": 0.0, "c": cat, "k": {}})
            child["v"] += ms
            node = child
        per_cat[node["c"]] += ms

    def finish(node):
        kids = [finish(k) for k in sorted(node["k"].values(), key=lambda k: -k["v"]) if k["v"] >= MIN_MS]
        out = {"n": node["n"], "v": round(node["v"], 2), "c": node["c"]}
        if kids:
            out["k"] = kids
        return out

    (top,) = root["k"].values()
    return finish(top), [round(x, 1) for x in per_cat]


SIZE_CATEGORIES = [
    ("Coefficient exception counts", 1),
    ("Signature salts", 2),
    ("Commitment", 3),
    ("Ring and norm messages", 4),
    ("Bit-to-integer bridge", 5),
    ("PCS opening", 6),
]
EXCEPTIONS, SALTS, SIZE_COMMIT, PIOP, SIZE_BRIDGE, SIZE_PCS = range(6)
PCS_LABELS = {
    "ring_switch_values": "ring-switch values",
    "initial_opened_rows": "opened rows, first level",
    "initial_merkle_paths": "Merkle paths, first level",
    "recursive_opened_rows": "opened rows, recursive levels",
    "recursive_merkle_paths": "Merkle paths, recursive levels",
    "roots": "Merkle roots",
    "final_message": "final message",
    "sumcheck_messages": "sumcheck messages",
    "ood_values": "out-of-domain values",
    "nonces": "proof-of-work nonces",
    "framing": "length prefixes",
}


def bitz_sizes(path: Path, label: str, sub: str, limbs: int):
    rows = [json.loads(line) for line in path.read_text().splitlines() if line.startswith("{")]
    row = rows[-1]
    parts = dict(row["proof_payload_breakdown"])
    columns = 1 << row["bridge_layouts"][0][1]
    sums = columns * (20 if limbs == 2 else 16)
    gkr = parts["bitz_bridge"] - sums
    cats = [0] * len(SIZE_CATEGORIES)
    cats[EXCEPTIONS] = parts["coefficient_exceptions"]
    cats[SIZE_COMMIT] = parts["commitment"]
    cats[PIOP] = parts["ring"] + parts["norm_and_binding"]
    cats[SIZE_BRIDGE] = parts["bitz_bridge"]
    cats[SIZE_PCS] = parts["pcs"] + parts["binary_sumcheck"]
    details = [
        ("Coefficient exception counts", parts["coefficient_exceptions"], 0, EXCEPTIONS),
        ("Commitment root", parts["commitment"], 0, SIZE_COMMIT),
        ("Ring and norm messages", cats[PIOP], 0, PIOP),
        ("ring check", parts["ring"], 1, PIOP),
        ("norm and word-binding sumchecks", parts["norm_and_binding"], 1, PIOP),
        ("Bit-to-integer bridge", parts["bitz_bridge"], 0, SIZE_BRIDGE),
        ("column sums", sums, 1, SIZE_BRIDGE),
        ("GKR messages and nonces", gkr, 1, SIZE_BRIDGE),
        ("PCS opening", cats[SIZE_PCS], 0, SIZE_PCS),
        ("binary source sumcheck", parts["binary_sumcheck"], 1, SIZE_PCS),
    ]
    for key, bytes_ in row.get("pcs_breakdown", []):
        if bytes_ > 0:
            details.append((PCS_LABELS.get(key, key), bytes_, 1, SIZE_PCS))
    assert sum(cats) == row["proof_payload_bytes"], (path, sum(cats), row["proof_payload_bytes"])
    return {"name": label, "sub": sub, "total": row["proof_payload_bytes"], "cats": cats, "details": details}


def aerie_sizes(path: Path, common_salt: bool):
    rows, header = [], None
    for line in path.read_text().splitlines():
        if line.startswith("records\t"):
            header = line.split("\t")
        elif header and line[:1].isdigit():
            rows.append(dict(zip(header, line.split("\t"))))
    row = rows[-1]
    records = int(row["records"])
    envelope = int(row["envelope_bytes"])
    commitments = int(row["commitment_bytes"]) + 8  # group commitments and their length word
    opening = int(row["pcs_proof_bytes"])
    salts = 40 * records
    piop = envelope - salts - commitments - opening
    if common_salt:
        salts = 40
    cats = [0] * len(SIZE_CATEGORIES)
    cats[SALTS], cats[SIZE_COMMIT], cats[PIOP], cats[SIZE_PCS] = salts, commitments, piop, opening
    details = [
        ("Signature salts" + (" (one shared)" if common_salt else f" ({records:,} × 40 B)"), salts, 0, SALTS),
        ("Commitments and length word", commitments, 0, SIZE_COMMIT),
        ("Header and PIOP messages", piop, 0, PIOP),
        ("Akita opening", opening, 0, SIZE_PCS),
    ]
    return {
        "name": "Aerie, one shared salt" if common_salt else "Aerie v0.2.0",
        "sub": "derived: per-record proof minus salts" if common_salt else "per-record salts (default)",
        "total": sum(cats), "cats": cats, "details": details,
    }


def pcs_parameters(here: Path):
    """Resolved Ligerito shapes: the algebraic grinding report's "pcs shape" line
    at 32,768 signatures, and the full-Falcon bench's pcs_query_shape at 1024."""
    rows = []
    shape = re.compile(r"pcs shape: committed 2\^(\d+) bits \| log_inv_rates \[([\d, ]*)\] \| queries \[([\d, ]*)\]"
                       r" \| initial_k (\d+) \| recursive_ks \[[\d, ]*\] \| fold grinding \[([\d, ]*)\] \| ood \[([\d, ]*)\]")
    ints = lambda s: [int(x) for x in s.split(",") if x.strip()]
    for key, name, records in [("shipped", "BitZ, falcon-poc", "32,768 signatures"),
                               ("t16", "BitZ, experiment", "32,768 signatures")]:
        path = here / "rate" / f"{key}.err"
        if not path.exists():
            return None
        m = shape.search(path.read_text())
        rows.append({"name": name, "records": records, "log_bits": int(m[1]), "rates": ints(m[2]),
                     "queries": ints(m[3]), "fold": ints(m[5]), "ood": ints(m[6])})
    full = here.parent / "full-falcon-20261018" / "ladder" / "S0-t10-r1.jsonl"
    if full.exists():
        record = next(json.loads(l) for l in full.read_text().splitlines() if '"pcs_query_shape"' in l)
        s = record["pcs_query_shape"]
        rows.append({"name": "Full Falcon (hybrid)", "records": "1,024 signatures",
                     "log_bits": s["initial_log_msg_cols"] + s["initial_log_num_interleaved"] + 7,
                     "rates": s["log_inv_rates"], "queries": s["queries"], "fold": s["fold_grinding_bits"],
                     "ood": s["ood_samples"]})
    return rows


def median(values):
    values = sorted(values)
    mid = len(values) // 2
    return values[mid] if len(values) % 2 else (values[mid - 1] + values[mid]) / 2


def peak_mib(err: Path) -> float:
    for line in err.read_text().splitlines():
        if "maximum resident set size" in line:
            return int(line.split()[0]) / 2**20
    return float("nan")


def aerie_rows(path: Path):
    rows, header = [], None
    for line in path.read_text().splitlines():
        if line.startswith("records\t"):
            header = line.split("\t")
        elif header and line[:1].isdigit():
            rows.append(dict(zip(header, line.split("\t"))))
    return rows


def bitz_rows(path: Path):
    return [r for r in (json.loads(l) for l in path.read_text().splitlines() if l.startswith("{"))
            if r.get("trial") == "sample"]


def threads_data(here: Path, ms_one_thread: dict, proofs: dict):
    t10 = here / "t10"
    if not (t10 / "DONE").exists():
        return None
    aerie = [aerie_rows(t10 / f"aerie-r{rep}.out")[-1] for rep in range(1, 6)]
    aerie_rss = median([peak_mib(t10 / f"aerie-r{rep}.err") for rep in range(1, 6)])
    rows = [{
        "name": "Aerie v0.2.0", "sub": "as published",
        "t1": {"prove": ms_one_thread["aerie"], "verify": median(float(r["verify_single_thread_ms"]) for r in aerie)},
        "t10": {"prove": median(float(r["prove_ms"]) for r in aerie),
                "verify": median(float(r["verify_parallel_ms"]) for r in aerie), "rss_mib": aerie_rss},
        "proof": proofs["aerie"],
    }]
    one_thread_verify = {
        "bitz-shipped": here / "samples" / "bitz-shipped.clock.out",
        "bitz-t16": here / "samply" / "bitz-t16.clock.out",
    }
    labels = {
        "bitz-shipped": ("BitZ, falcon-poc", "as shipped, optimized binder"),
        "bitz-t16": ("BitZ, experiment", "keyed ring · 2^16 bridge rows"),
    }
    for key in ["bitz-shipped", "bitz-t16"]:
        runs = bitz_rows(t10 / f"{key}.out")
        one = [json.loads(l) for l in one_thread_verify[key].read_text().splitlines() if l.startswith("{")]
        rows.append({
            "name": labels[key][0], "sub": labels[key][1],
            "t1": {"prove": ms_one_thread[key], "verify": median(r["verify_ms"] for r in one)},
            "t10": {"prove": median(r["total_prover_ms"] for r in runs),
                    "verify": median(r["verify_ms"] for r in runs), "rss_mib": peak_mib(t10 / f"{key}.err")},
            "proof": proofs[key],
        })
    default = bitz_rows(t10 / "bitz-shipped-default.out")
    notes_file = here / "threads_notes.json"
    notes = json.loads(notes_file.read_text()) if notes_file.exists() else []
    notes = [n.replace("{DEFAULT_PROVE}", f"{median(r['total_prover_ms'] for r in default) / 1000:.2f} s")
              .replace("{DEFAULT_VERIFY}", f"{median(r['verify_ms'] for r in default):.0f} ms") for n in notes]
    return {"figure": "M5 · medians of five proves", "rows": rows, "notes": notes}


summary = json.loads((OUT / "summary.json").read_text())
profiles = []
for key, label, sub, rules in PROFILES:
    if key not in summary:
        continue
    tree, cats = build(key, rules, summary[key])
    profiles.append({
        "key": key, "name": label, "sub": sub, "ms": round(summary[key]["ms_per_prove"], 1),
        "cats": cats, "tree": tree,
    })

data = {
    "records": 32768,
    "subtitle": (
        "32,768 Falcon-512 signatures, 128-bit security, one thread (RAYON_NUM_THREADS=1) on an "
        "Apple M5. Widths are milliseconds per prove, on one scale across all three provers."
    ),
    "categories": [{"name": n, "slot": i + 1} for i, n in enumerate(CATEGORIES)],
    "profiles": profiles,
    "notes": [],
}
notes_file = Path(__file__).parent / "notes.json"
if notes_file.exists():
    data["notes"] = json.loads(notes_file.read_text())
SIZES = Path(__file__).parent / "sizes"
if SIZES.exists():
    data["sizes"] = {
        "categories": [{"name": n, "slot": s} for n, s in SIZE_CATEGORIES],
        "rows": [
            aerie_sizes(SIZES / "aerie.out", common_salt=False),
            aerie_sizes(SIZES / "aerie.out", common_salt=True),
            bitz_sizes(SIZES / "bitz-shipped.out", "BitZ, falcon-poc", "as shipped · two-limb bridge", 2),
            bitz_sizes(SIZES / "bitz-t16.out", "BitZ, experiment", "keyed ring · one-limb bridge, 2^16 rows", 1),
        ],
    }
    size_notes = Path(__file__).parent / "size_notes.json"
    data["sizes"]["notes"] = json.loads(size_notes.read_text()) if size_notes.exists() else []
    pcs = pcs_parameters(Path(__file__).parent)
    if pcs:
        data["sizes"]["pcs"] = pcs
        for row in pcs:
            print("pcs", row["name"], row["log_bits"], row["rates"], row["queries"])
    for row in data["sizes"]["rows"]:
        print(row["name"], row["total"], dict(zip([n for n, _ in SIZE_CATEGORIES], row["cats"])))
    proofs = {
        "aerie": data["sizes"]["rows"][0]["total"],
        "bitz-shipped": data["sizes"]["rows"][2]["total"],
        "bitz-t16": data["sizes"]["rows"][3]["total"],
    }
    threads = threads_data(Path(__file__).parent, {p["key"]: p["ms"] for p in profiles}, proofs)
    if threads:
        data["threads"] = threads
        for row in threads["rows"]:
            print(row["name"], row["t1"], row["t10"], row["proof"])
FULL = Path(__file__).parent.parent / "full-falcon-20261018" / "page.json"
if FULL.exists():
    data["full"] = json.loads(FULL.read_text())
    for row in data["full"]["rows"]:
        print("full", row["key"], row["prove"], row["min"], row["max"], row["proof"])
html = TEMPLATE.read_text().replace("/*DATA*/", json.dumps(data, separators=(",", ":")).replace("</", "<\\/"))
INDEX.parent.mkdir(parents=True, exist_ok=True)
INDEX.write_text(html)
for p in profiles:
    print(p["key"], p["ms"], dict(zip(CATEGORIES, p["cats"])))
print(f"{INDEX}: {len(html) / 1024:.0f} KiB")
