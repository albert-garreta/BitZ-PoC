#!/usr/bin/env python3
"""LaTeX table of absolute numbers from a SHA-256 + P-256 ECDSA campaign (scripts/run_sha256_ecdsa_compare.py).

Reads the `*.result.json` records of one or more campaign output directories
(later directories override earlier ones for the same case) and writes a
self-documenting `paper/sha256-ecdsa-table.tex`: one row group per
(security target, thread count), one row per scheme, with the medians of
witness generation, the complete prover (commitment included) and the complete
verifier, plus the complete proof material and the worker's peak memory.

    python3 scripts/sha256_ecdsa_table.py bench_results/sha256-ecdsa-i7-f2z-binius
"""
from __future__ import annotations

import argparse
import datetime
import json
import platform
import statistics
import subprocess
from pathlib import Path

SCHEMA = "f2z/sha256-ecdsa-compare/v1"
# Table rows are (method key, LaTeX label), in display order.
SCHEMES = [
    ("f2z-split", "\\ftwoz\\ (this work)"),
    ("f2z-all", "\\ftwoz\\ (this work), all rows"),
    ("binius64", "Binius64~\\cite{binius64}"),
    ("spartan-mc", "Spartan2 (NeutronNova, non-ZK)"),
]
PLACEHOLDER = "--"


def fmt_ms(v: float) -> str:
    if v >= 100:
        return f"{v:.0f}"
    if v >= 10:
        return f"{v:.1f}"
    return f"{v:.2f}"


def fmt_gb(peak_rss_bytes: float) -> str:
    v = peak_rss_bytes / (1 << 30)
    if v >= 10:
        return f"{v:.1f}"
    if v >= 1:
        return f"{v:.2f}"
    return f"{v:.3f}"


def fmt_kb(b: float) -> str:
    return f"{b / 1000:.0f}"


def probe(cmd: list[str], default: str) -> str:
    try:
        return subprocess.run(cmd, capture_output=True, text=True, check=True).stdout.strip() or default
    except Exception:
        return default


def load_cases(directory: Path) -> dict:
    """Complete cases keyed by (log_compressions, security_target, threads, method)."""
    result = {}
    for path in sorted(directory.glob("*.result.json")):
        record = json.loads(path.read_text())
        if record["status"] != "complete":
            continue
        case = record["case"]
        samples = [r for r in record["rows"] if r.get("trial") == "sample"]
        if not samples or any(r.get("schema") != SCHEMA or r.get("verified") is not True for r in samples):
            raise ValueError(f"{path}: complete case without verified samples")
        key = (case["log_compressions"], case["security_target"], case["threads"], case["method"])
        if key in result:
            raise ValueError(f"duplicate case {key} in {directory}")
        med = lambda k: statistics.median(r[k] for r in samples)  # noqa: E731
        result[key] = dict(
            key=key, run_dir=str(directory), file=path.name, samples=len(samples),
            seed=case["seed"], compressions=samples[0]["compressions"], message_bytes=samples[0]["message_bytes"],
            fixture_id=samples[0]["fixture_id"], security=samples[0].get("security", {}),
            circuit=samples[0].get("circuit"), peak_rss_bytes=record["peak_rss_bytes"],
            witness_ms=med("witness_ms"), prove_ms=med("prove_ms"), e2e_prover_ms=med("e2e_prover_ms"),
            verify_ms=med("verify_ms"), proof_bytes=med("proof_material_bytes"), setup_ms=med("setup_ms"),
            opening_ms=med("opening_ms") if all(isinstance(r.get("opening_ms"), (int, float)) for r in samples) else None,
        )
    return result


def f2z_security(row: dict) -> str:
    """One caption clause for an F2Z row's Ligerito policy at level 0."""
    lig = row["security"].get("ligerito", {})
    levels = (lig.get("configuration") or lig).get("levels") or []
    if not levels:
        return "Lambda%d" % row["key"][1]
    top = levels[0]
    regime = "Johnson" if str(top.get("regime", "")).startswith("johnson") else "unique decoding radius"
    ood = "early Round-0 OOD" if str(top.get("regime", "")).endswith("ood") else "no OOD"
    return (f"target {row['key'][1]} bits: Ligerito {regime}, rate $1/{1 << int(top.get('log_inv_rate', 1))}$, "
            f"{top.get('queries')} level-0 queries, {top.get('grinding_bits', 0)} bits of grinding, "
            f"{top.get('fold_grinding_bits', 0)} bits of fold grinding, {ood}; {row['security'].get('economic_bits', 0):.1f} bits achieved")


def binius_security(row: dict) -> str:
    s = row["security"]
    return (f"target {s.get('fri_query_target_bits')} bits: {s.get('fri_queries')} queries at rate "
            f"$1/{1 << int(s.get('log_inv_rate', 1))}$")


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("run_dirs", type=Path, nargs="+", metavar="RUN_DIR")
    ap.add_argument("--out", type=Path, default=Path("paper/sha256-ecdsa-table.tex"))
    ap.add_argument("--label", default="tab:sha256-ecdsa")
    args = ap.parse_args()

    by = {}
    for run_dir in args.run_dirs:
        by.update(load_cases(run_dir))
    if not by:
        raise SystemExit("no complete cases found")
    fixtures = {}
    for row in by.values():
        fixtures.setdefault((row["key"][0], row["seed"]), set()).add(row["fixture_id"])
    if any(len(ids) != 1 for ids in fixtures.values()):
        raise SystemExit("cases at one size and seed used different fixtures")

    exponents = sorted({k[0] for k in by})
    targets = sorted({k[1] for k in by if k[1] is not None})
    threads = sorted({k[2] for k in by})
    methods = [m for m, _ in SCHEMES if any(k[3] == m for k in by)]
    show_n, show_t, show_th = len(exponents) > 1, len(targets) > 1, len(threads) > 1
    lead = ([("$N$", "r")] if show_n else []) + ([("$\\lambda$ (bits)", "r")] if show_t else []) + ([("Threads", "r")] if show_th else [])
    metrics = [("witness_ms", "Witgen (ms)", fmt_ms), ("prove_ms", "Prover (ms)", fmt_ms), ("verify_ms", "Verifier (ms)", fmt_ms),
               ("proof_bytes", "Proof (KB)", fmt_kb), ("peak_rss_bytes", "Peak mem.\\ (GB)", fmt_gb)]

    stamp = datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%d")
    rev = probe(["git", "rev-parse", "HEAD"], "unknown")
    dirty = "+dirty" if probe(["git", "status", "--porcelain", "--untracked-files=no"], "") else ""
    machine = probe(["sysctl", "-n", "machdep.cpu.brand_string"], platform.processor() or platform.machine())
    for run_dir in args.run_dirs:
        manifest = run_dir / "manifest.json"
        if manifest.exists():
            cpu = json.loads(manifest.read_text()).get("cpu")
            if cpu and cpu not in ("arm", "arm64", "x86_64", "i386"):
                machine = cpu
    dirs = " ".join(str(d) for d in args.run_dirs)
    header = [
        "% SHA-256 chain + P-256 ECDSA comparison (F2Z vs Binius64) — GENERATED FILE, do not edit by hand.",
        f"% Generated by scripts/sha256_ecdsa_table.py on {stamp} (UTC) at {rev}{dirty} from {dirs}",
        "%   (benches/sha256_ecdsa_compare.rs + benchmarks/binius64, driven by scripts/run_sha256_ecdsa_compare.py;",
        "%   see docs/sha256-ecdsa-comparison.md; later run directories override earlier ones per case).",
        "% Regenerate (from the repo root; this file is overwritten):",
        f"%   python3 scripts/sha256_ecdsa_table.py {dirs}",
        f"% Machine: {machine}; medians of the measured samples after one warm-up; one worker process per case.",
        "% Columns: witgen = witness_ms (witness generation from the message and signature; Binius64 counts its witness packing here);",
        "%   prover = prove_ms = commit_ms + protocol_ms (the complete prover after witness generation: commitment + PIOP + PCS opening);",
        "%   verifier = verify_ms (statement validation + verification of the decoded proof); proof = proof_material_bytes",
        "%   (complete proof material incl. commitments and auxiliary public inputs, statement excluded), KB = 1000 bytes;",
        "%   peak mem. = peak_rss_bytes of the whole worker process (setup, warm-up and all samples included), GB = 2^30 bytes.",
        "% Bold = best of the schemes for that (target, threads) group and column.",
        "% Medians as recorded (ms unless noted):",
    ]
    for key in sorted(by, key=lambda k: (k[0], k[1] if k[1] is not None else -1, k[2], methods.index(k[3]))):
        r = by[key]
        header.append(
            f"%   2^{key[0]} target={key[1]} threads={key[2]} {key[3]:<10} witness={r['witness_ms']:.3f} prove={r['prove_ms']:.3f} "
            f"e2e_prover={r['e2e_prover_ms']:.3f} verify={r['verify_ms']:.3f} setup={r['setup_ms']:.3f} "
            f"opening={r['opening_ms'] if r['opening_ms'] is None else round(r['opening_ms'], 3)} proof_bytes={int(r['proof_bytes'])} "
            f"peak_rss={r['peak_rss_bytes']} samples={r['samples']} file={r['file']}")
    circuits = {}
    for key in sorted(by):
        circuits.setdefault(key[3], json.dumps(by[key].get("circuit"), sort_keys=True))
    for method, circuit in circuits.items():
        header.append(f"%   circuit {method}: {circuit}")

    lines = ["", "\\begin{table}[H]", "  \\centering", "  \\small", "  \\setlength{\\tabcolsep}{4.5pt}",
             "  \\begin{tabular}{@{}" + "".join(a for _, a in lead) + "l" + "r" * len(metrics) + "@{}}", "    \\toprule",
             "    " + " & ".join([h for h, _ in lead] + ["Scheme"] + [h for _, h, _ in metrics]) + " \\\\", "    \\midrule"]
    first_group = True
    for n in exponents:
        first_n = True
        for t in targets + ([None] if any(k[1] is None for k in by) else []):
            first_t = True
            for th in threads:
                rows = [(m, by[(n, t, th, m)]) for m in methods if (n, t, th, m) in by]
                if not rows:
                    continue
                if not first_group:
                    lines.append("    \\addlinespace")
                first_group = False
                best = {}
                for k, _, _ in metrics:
                    values = [r[k] for _, r in rows if r.get(k) is not None]
                    best[k] = min(values) if len(values) > 1 else None
                for i, (m, r) in enumerate(rows):
                    cells = []
                    if show_n:
                        cells.append(f"$2^{{{n}}}$" if first_n else "")
                    if show_t:
                        cells.append(("$%d$" % t if t is not None else "n/a") if first_t else "")
                    if show_th:
                        cells.append(str(th) if i == 0 else "")
                    first_n = first_t = False
                    cells.append(dict(SCHEMES)[m])
                    for k, _, f in metrics:
                        v = r.get(k)
                        s = PLACEHOLDER if v is None else f(v)
                        cells.append(f"\\textbf{{{s}}}" if best[k] is not None and v == best[k] else s)
                    lines.append("    " + " & ".join(cells) + " \\\\")
    lines += ["    \\bottomrule", "  \\end{tabular}"]

    any_row = next(iter(by.values()))
    f2z_rows = [r for r in by.values() if r["key"][3].startswith("f2z")]
    binius_rows = [r for r in by.values() if r["key"][3] == "binius64"]
    clauses = []
    if f2z_rows:
        policies = []
        for r in sorted(f2z_rows, key=lambda r: r["key"]):
            clause = f2z_security(r)
            if clause not in policies:
                policies.append(clause)
        clauses.append("\\ftwoz\\ (integer R1CS with $\\FF_2$-virtualization; round-by-round economic security model; " + "; ".join(policies) + ")")
    if binius_rows:
        policies = []
        for r in sorted(binius_rows, key=lambda r: r["key"]):
            clause = binius_security(r)
            if clause not in policies:
                policies.append(clause)
        s = binius_rows[0]["security"]
        clauses.append(f"Binius64 (fixed SHA-256 circuit and complete-arithmetic P-256 gadget, ring switching and BaseFold with "
                       f"{s.get('merkle_hash', 'SHA-256')} Merkle hashing; FRI query target only, " + "; ".join(policies) + ")")
    compressions = any_row["compressions"]
    n_desc = f"$2^{{{exponents[0]}}} = {compressions}$" if len(exponents) == 1 else "$N$"
    msg = f"{any_row['message_bytes']:,}".replace(",", "{,}") + " bytes" if len(exponents) == 1 else "$64(N-1)$ bytes"
    threads_desc = " and ".join(str(t) for t in threads)
    reps = sorted({r["samples"] for r in by.values()})
    caption = (f"Proving one SHA-256 hash of a message of {msg} ({n_desc} compressions, padding block included) followed by one P-256 ECDSA "
               f"signature verification of the digest, non-ZK: " + "; ".join(clauses) + ". "
               "Native security targets are reported separately; these are not a uniform complete-protocol bound. "
               "\\emph{Witgen} is the witness generation from the message and signature; \\emph{prover} is the complete prover after "
               "witness generation, commitment included; \\emph{verifier} is the complete verification of the decoded proof; "
               "\\emph{proof} is the complete proof material, commitments included ($1$\\,KB $= 1000$ bytes); \\emph{peak mem.} is the "
               "high-water resident set of the worker process, setup and warm-up included ($1$\\,GB $= 2^{30}$ bytes). "
               f"Apple M5; threads per run: {threads_desc}; medians of {' or '.join(map(str, reps))} runs after one warm-up.")
    lines += ["  \\caption{" + caption + "}", f"  \\label{{{args.label}}}", "\\end{table}", ""]
    args.out.write_text("\n".join(header + lines))
    print(f"wrote {args.out} ({len(by)} cases)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
