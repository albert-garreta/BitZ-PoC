#!/usr/bin/env python3
"""LaTeX table of absolute numbers from a native-mul run (benches/mul_e2e_compare.rs).

Reads the `summary.json` of one or more `PerfRuns/<stamp>-native-mul` run
directories (later directories override earlier ones for the same scheme and
size, so a follow-up run of the large sizes extends a first run) and writes a self-documenting
`paper/native-mul-table.tex`: one row group per size N, one row per scheme, with
the medians of witness generation, the complete prover (commit included) and the
complete verifier, plus placeholder columns for proof size and peak memory.

    python3 scripts/native_mul_table.py PerfRuns/2026-09-08T20-07-32Z-native-mul PerfRuns/2026-09-08T20-55-10Z-native-mul
"""
from __future__ import annotations

import argparse
import datetime
import json
import platform
import statistics
import subprocess
from pathlib import Path

SCHEMES = [  # (backend slug, LaTeX label)
    ("f2z", "\\ftwoz\\ (this work)"),
    ("binius64", "Binius64~\\cite{binius64}"),
    ("plonky3-whir", "Plonky3~\\cite{plonky} (WHIR)"),
]
PLACEHOLDER = "--"


def fmt_ms(v: float) -> str:
    if v >= 100:
        return f"{v:.0f}"
    if v >= 10:
        return f"{v:.1f}"
    return f"{v:.2f}"


def proof_sizes(run_dir: Path, workload: str) -> dict:
    """Median `proof_bytes` per (backend, exponent) over the run's measured samples."""
    by = {}
    for line in (run_dir / "samples.jsonl").read_text().splitlines():
        r = json.loads(line)
        if r["workload"] != workload or r["trial"]["kind"] != "sample":
            continue
        size = r["metrics"].get("proof_bytes")
        if size is not None:
            by.setdefault((r["backend"], r["log_multiplications"]), []).append(size)
    return {k: statistics.median(v) for k, v in by.items()}


def probe(cmd: list[str], default: str) -> str:
    try:
        return subprocess.run(cmd, capture_output=True, text=True, check=True).stdout.strip() or default
    except Exception:
        return default


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("run_dirs", type=Path, nargs="+", metavar="RUN_DIR")
    ap.add_argument("--out", type=Path, default=None, help="default: paper/native-mul-table.tex (u32) or paper/native-mul-<workload>-table.tex")
    ap.add_argument("--exponents", default="", help="comma list or lo-hi; default: every size in the run")
    ap.add_argument("--workload", default="u32", choices=["u32", "babybear", "u64", "u128"])
    ap.add_argument("--memory-bound", default="", metavar="BACKEND:EXP[,...]",
                    help="rows measured while paging (prover exceeded the machine's memory): omitted from the table and noted in the caption")
    ap.add_argument("--proof-sizes-from", default="", metavar="RUN_DIR[,...]",
                    help="run directories that only contribute proof_bytes (for rows whose own run predates proof-size recording); proof sizes are deterministic per scheme, workload, and size")
    args = ap.parse_args()
    memory_bound = set()
    for item in filter(None, args.memory_bound.split(",")):
        backend, exp = item.split(":")
        memory_bound.add((backend, int(exp)))
    if args.out is None:
        args.out = Path("paper/native-mul-table.tex" if args.workload == "u32" else f"paper/native-mul-{args.workload}-table.tex")

    by = {}
    for run_dir in args.run_dirs:
        for r in json.loads((run_dir / "summary.json").read_text()):
            if r["workload"] == args.workload:
                r["run_dir"] = str(run_dir)
                by[(r["backend"], r["log_multiplications"])] = r
    paged = {k: by.pop(k) for k in list(by) if k in memory_bound}
    # Proof sizes: the row's own run first, then the proof-size-only runs.
    sizes = {}
    for run_dir in args.run_dirs:
        sizes.update(proof_sizes(run_dir, args.workload))
    size_dirs = [Path(d) for d in filter(None, args.proof_sizes_from.split(","))]
    for run_dir in size_dirs:
        for k, v in proof_sizes(run_dir, args.workload).items():
            sizes.setdefault(k, v)
    rows = list(by.values())
    if not rows:
        raise SystemExit(f"no {args.workload} rows in {args.run_dirs}")
    exps = sorted({r["log_multiplications"] for r in rows})
    if args.exponents:
        if "-" in args.exponents:
            lo, hi = map(int, args.exponents.split("-"))
            exps = [e for e in exps if lo <= e <= hi]
        else:
            keep = {int(x) for x in args.exponents.split(",")}
            exps = [e for e in exps if e in keep]
    samples = sorted({r["samples"] for r in rows})
    run_list = " ".join(str(d) for d in args.run_dirs)
    cpu = probe(["sysctl", "-n", "machdep.cpu.brand_string"], platform.processor() or "unknown CPU")
    mem_gb = int(probe(["sysctl", "-n", "hw.memsize"], "0")) // (1 << 30)
    commit = probe(["git", "describe", "--always", "--dirty"], "unknown")
    date = datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%d")

    out = []
    w = out.append
    w("% Native end-to-end u32-multiplication comparison (F2Z vs Binius64 vs Plonky3/WHIR) — GENERATED FILE, do not edit by hand.")
    w(f"% Generated by scripts/native_mul_table.py on {date} (UTC) at {commit} from {run_list} (benches/mul_e2e_compare.rs,")
    w("%   see docs/native-mul-compare.md; later run directories override earlier ones per scheme and size).")
    if size_dirs:
        w(f"% Proof sizes for rows whose run predates proof-size recording come from {' '.join(map(str, size_dirs))}.")
    w("% Regenerate (from the repo root; this file is overwritten):")
    w(f"%   python3 scripts/native_mul_table.py {run_list} --workload {args.workload}" + (f" --exponents {args.exponents}" if args.exponents else "")
      + (f" --memory-bound {args.memory_bound}" if args.memory_bound else "") + (f" --proof-sizes-from {args.proof_sizes_from}" if args.proof_sizes_from else ""))
    w(f"% Machine: {cpu}, {mem_gb} GB; medians of {'/'.join(map(str, samples))} samples after one warm-up.")
    w("% Columns: witgen = witness_ms (native witness generation; Binius64 packs its witness inside its prover, so its witgen")
    w("%   overlaps the prover column); prover = online_prover_ms (the complete native prover call after witness generation,")
    w("%   i.e. commitment + PIOP + PCS opening); verifier = verify_ms; proof = median proof_bytes of the samples, KB = 1000 bytes;")
    w("%   peak memory is a PLACEHOLDER (--).")
    w("% Bold = best of the schemes for that size and column.")
    w("% Medians (ms) as recorded in summary.json (`--` rows below = scheme not run at that size):")
    for e in exps:
        for slug, _ in SCHEMES:
            r = by.get((slug, e))
            if r:
                m = r["medians"]
                size = sizes.get((slug, e))
                w(f"%   2^{e} {slug:13s} witness={m['witness_ms']:.3f} commit={m['commit_ms']:.3f} piop={m['piop_ms']:.3f} opening={m['opening_ms']:.3f} "
                  f"online_prover={m['online_prover_ms']:.3f} witness_to_proof={m['witness_to_proof_ms']:.3f} verify={m['verify_ms']:.3f} setup={r['setup_ms']:.3f} "
                  f"proof_bytes={'n/a' if size is None else int(size)} samples={r['samples']} run={Path(r['run_dir']).name}")
            else:
                w(f"%   2^{e} {slug:13s} not run")
    missing = {}
    for slug, label in SCHEMES:
        gone = [e for e in exps if (slug, e) not in by and (slug, e) not in paged]
        if gone and len(gone) < len(exps):
            missing[slug] = gone
    for (slug, e), r in sorted(paged.items()):
        m = r["medians"]
        w(f"%   2^{e} {slug:13s} OMITTED (paging): online_prover={m['online_prover_ms']:.3f} verify={m['verify_ms']:.3f} run={Path(r['run_dir']).name}")
    paged_by_scheme = {}
    for slug, e in sorted(paged):
        paged_by_scheme.setdefault(slug, []).append(e)
    schemes = [(slug, label) for slug, label in SCHEMES if any((slug, e) in by for e in exps)]
    w("")
    w("\\begin{table}[H]")
    w("  \\centering")
    w("  \\small")
    w("  \\setlength{\\tabcolsep}{4.5pt}")
    w("  \\begin{tabular}{@{}rlrrrrr@{}}")
    w("    \\toprule")
    w("    $N$ & Scheme & Witgen (ms) & Prover (ms) & Verifier (ms) & Proof (KB) & Peak mem.\\ (GB) \\\\")
    w("    \\midrule")
    for gi, e in enumerate(exps):
        present = {slug: by[(slug, e)]["medians"] for slug, _ in SCHEMES if (slug, e) in by}
        best = {k: min(m[k] for m in present.values()) for k in ("witness_ms", "online_prover_ms", "verify_ms")}
        present_sizes = {slug: sizes[(slug, e)] for slug in present if (slug, e) in sizes}
        best_size = min(present_sizes.values()) if present_sizes else None
        for ri, (slug, label) in enumerate(schemes):
            n_cell = f"$2^{{{e}}}$" if ri == 0 else ""
            m = present.get(slug)
            if m is None:
                w(f"    {n_cell} & {label} & {PLACEHOLDER} & {PLACEHOLDER} & {PLACEHOLDER} & {PLACEHOLDER} & {PLACEHOLDER} \\\\")
                continue
            cells = []
            for k in ("witness_ms", "online_prover_ms", "verify_ms"):
                s = fmt_ms(m[k])
                cells.append(f"\\textbf{{{s}}}" if m[k] == best[k] and len(present) > 1 else s)
            size = present_sizes.get(slug)
            if size is None:
                size_cell = PLACEHOLDER
            else:
                s = fmt_ms(size / 1000)
                size_cell = f"\\textbf{{{s}}}" if size == best_size and len(present_sizes) > 1 else s
            w(f"    {n_cell} & {label} & {cells[0]} & {cells[1]} & {cells[2]} & {size_cell} & {PLACEHOLDER} \\\\")
        if gi + 1 < len(exps):
            w("    \\addlinespace")
    w("    \\bottomrule")
    w("  \\end{tabular}")
    statement = {
        "u32": "Native end-to-end proofs of $N$ multiplications $x \\cdot y = z$ of random $32$-bit integers: ",
        "u64": "Native end-to-end proofs of $N$ multiplications $x \\cdot y = z$ of random $64$-bit integers ($z$ a $128$-bit integer): ",
        "babybear": "Native end-to-end proofs of $N$ multiplications $a \\cdot b = c$ in the BabyBear field: ",
        "u128": "Native end-to-end proofs of $N$ multiplications $x \\cdot y = z$ of random $128$-bit integers ($z$ a $256$-bit integer): ",
    }[args.workload]
    # Binius64's opener geometry as the run recorded it (rate override
    # F2Z_BINIUS_LOG_INV_RATE; the query count follows from it).
    binius_rows = [r for r in rows if r["backend"] == "binius64"]
    binius_rate = 1 << int(binius_rows[0]["config"].get("log_inv_rate", 1)) if binius_rows else 2
    binius_queries = int(binius_rows[0]["config"].get("fri_queries", 241)) if binius_rows else 241
    scheme_notes = {
        "f2z": "\\ftwoz\\ (Spartan over a transcript-sampled prime with the \\ftwoz\\ opening, $\\lambda = 100$)",
        "binius64": "Binius64 (native multiplication" + (" and bit constraints" if args.workload not in ("u64", "u128") else "") + f", ring switching and BaseFold at rate $1/{binius_rate}$ with ${binius_queries}$ queries for $100$ bits)",
        "plonky3-whir": "Plonky3 (" + ("Goldilocks AIR with $32$-bit decompositions" if args.workload == "u32" else "BabyBear AIR") + ", WHIR over a degree-$5$ extension at rate $1/2$, $100$ bits)",
    }
    present = [slug for slug, _ in schemes]
    joiner = ", and " if len(present) > 2 else (" and " if len(present) == 2 else "")
    scheme_text = ", ".join(scheme_notes[s] for s in present[:-1]) + joiner + scheme_notes[present[-1]] + ". "
    w("  \\caption{" + statement + scheme_text
      + "\\emph{Witgen} is the native witness generation; \\emph{prover} is the complete prover call after witness generation, "
      + "commitment included; \\emph{verifier} is the complete verification. "
      + "".join(f"{dict(SCHEMES)[slug].split('~')[0].replace(chr(92) + 'ftwoz' + chr(92), chr(92) + 'ftwoz')} was not run at " + ", ".join(f"$2^{{{e}}}$" for e in gone) + ". " for slug, gone in missing.items())
      + "".join(f"At " + ", ".join(f"$2^{{{e}}}$" for e in gone) + f" the {dict(SCHEMES)[slug].split('~')[0]} prover exceeds the machine's memory and pages; that row is omitted. " for slug, gone in paged_by_scheme.items())
      + f"{cpu}, {mem_gb}\\,GB, $8$ threads; medians of {samples[0]} runs after one warm-up.}}")
    w("  \\label{tab:native-mul" + ("" if args.workload == "u32" else "-" + args.workload) + "}")
    w("\\end{table}")
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text("\n".join(out) + "\n")
    print(f"wrote {args.out} ({len(exps)} sizes, {len(schemes)} schemes)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
