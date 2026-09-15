#!/usr/bin/env python3
"""LaTeX table for the hybrid mod-2^32 multiplication + chained SHA-256 benchmark.

Reads `benches/hybrid_u32_sha256` sweep results — `<run>/<mode>/summary.csv` plus
the sampler's `<run>/peak-rss-and-swap.tsv` (scripts/rss_sampler.py) — for the
F2Z hybrid rows and the all-Binius rows, and writes a self-documenting
`paper/hybrid-table.tex`: one row group per shape (N multiplications, M
compressions), one row per scheme, medians of the complete prover, the
verifier, the proof size and the sampled peak memory, bold = best per size and
column, dagger = the case paged. The two runs may differ (a re-measure of the
F2Z rows keeps the Binius rows of an earlier campaign):

    python3 scripts/hybrid_table.py --hybrid PerfRuns/<new-run> --binius PerfRuns/<binius-run>

`--variant counts` writes the equal operation-count table (N multiplications
together with M = N compressions, `paper/hybrid-table-equal-counts.tex`)
instead of the equal packed-witness table (M = N/256).
"""
from __future__ import annotations

import argparse
import csv
import datetime
import platform
import statistics
import subprocess
from pathlib import Path

ROWS = [
    ("hybrid", r"\ftwoz\ (this work)"),
    ("all-binius", r"Binius~\cite{binius64}, rate $1/8$"),
]

SCHEMES = (
    r" \ftwoz\ proves the multiplications with Spartan over a transcript-sampled prime and the SHA-256 chain with the Binius64 PIOP, and discharges both through one shared \ftwoz\ opening (Johnson-regime Ligerito at rate $1/2$ with the out-of-domain Round~0); its whole-protocol union bound is gated at $\lambda = 100$. Binius64 proves the same SHA-256 chain and a native four-limb multiplication gadget in one proof with ring switching and FRI at rate $1/8$ and a $100$-bit query-phase target ($121$ queries)."
    r" \emph{Prover} includes witness synthesis and the commitments; \emph{verifier} includes decoding; \emph{peak mem.} is the high-water resident set of the proving process ($1$\,GB $= 2^{30}$ bytes); $^{\dagger}$ marks cases that paged. Apple M5, 24\,GB, $8$ threads; medians of $11$ verified runs."
)

VARIANTS = {
    # Equal packed witnesses: one packed 128-bit word per multiplication,
    # 256 per compression, so M = N/256.
    "witness": {
        "output": "paper/hybrid-table.tex",
        "label": "tab:hybrid-sha256-mul",
        "title": "Hybrid u32-multiplication + chained SHA-256 comparison (F2Z hybrid vs all-Binius64)",
        "caption": r"End-to-end proofs of $N$ multiplications $x \cdot y = z + 2^{32} w$ of $32$-bit integers together with $M = N/256$ chained SHA-256 compressions (equal packed witnesses for the two branches)."
        + SCHEMES,
    },
    # Equal operation counts: N = M, the SHA-256 branch's packed witness is
    # 256 times the multiplication branch's.
    "counts": {
        "output": "paper/hybrid-table-equal-counts.tex",
        "label": "tab:hybrid-sha256-mul-equal-counts",
        "title": "Hybrid u32-multiplication + chained SHA-256 comparison at equal operation counts N = M (F2Z hybrid vs all-Binius64)",
        "caption": r"End-to-end proofs of $N$ multiplications $x \cdot y = z + 2^{32} w$ of $32$-bit integers together with $M = N$ chained SHA-256 compressions (equal operation counts; the packed SHA-256 witness is $256\times$ the multiplication witness, so the workload is dominated by the compressions)."
        + SCHEMES,
    },
}


def medians(run: Path, mode: str):
    summary = run / mode / "summary.csv"
    by_shape: dict[tuple[int, int], list[dict]] = {}
    with open(summary) as f:
        for row in csv.DictReader(f):
            if row["mode"] != mode:
                continue
            by_shape.setdefault((int(row["mul_log"]), int(row["sha_log"])), []).append(row)
    peaks = {}
    tsv = run / "peak-rss-and-swap.tsv"
    if tsv.exists():
        with open(tsv) as f:
            for line in f:
                parts = line.split("\t")
                if len(parts) < 6 or parts[0] != mode or not parts[1].isdigit():
                    continue
                compressions = int(parts[8]) if len(parts) > 8 and parts[8].strip().isdigit() else 0
                peaks[(int(parts[1]), int(parts[2]))] = (int(parts[4]), int(parts[5]), compressions)
    out = {}
    for shape, rows in by_shape.items():
        bits = None
        log = run / mode / f"{mode}-m{shape[0]}-s{shape[1]}.log"
        if log.exists():
            for line in log.read_text().splitlines():
                if "algebraic_security_bits=" in line:
                    bits = float(line.split("algebraic_security_bits=")[1].split()[0])
        peak, swapouts, compressions = peaks.get(shape, (None, 0, 0))
        out[shape] = {
            "prover": statistics.median(float(r["total_prover_ms"]) for r in rows),
            "verify": statistics.median(float(r["verify_ms"]) for r in rows),
            "proof": int(statistics.median(int(r["proof_bytes"]) for r in rows)),
            "peak_mib": peak,
            "swapouts": swapouts,
            "compressions": compressions,
            # The dagger keeps the paper's definition (the case paged); page
            # compressions are reported in the header for transparency (they
            # can happen during setup without touching the timed proves).
            "pressured": swapouts > 0,
            "bits": bits,
            "samples": len(rows),
        }
    return out


def sig3(x: float) -> str:
    """Three significant figures, as the paper's tables print them."""
    if x >= 100:
        return f"{x:.0f}"
    if x >= 10:
        return f"{x:.1f}"
    if x >= 1:
        return f"{x:.2f}"
    return f"{x:.3f}"


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--hybrid", required=True, type=Path, help="run directory with hybrid/summary.csv")
    parser.add_argument("--binius", required=True, type=Path, help="run directory with all-binius/summary.csv")
    parser.add_argument("--variant", choices=sorted(VARIANTS), default="witness", help="which table: equal packed witnesses (M = N/256) or equal operation counts (N = M)")
    parser.add_argument("--output", type=Path, help="defaults to the variant's file under paper/")
    args = parser.parse_args()
    variant = VARIANTS[args.variant]
    output = args.output or Path(variant["output"])
    data = {"hybrid": medians(args.hybrid, "hybrid"), "all-binius": medians(args.binius, "all-binius")}
    shapes = sorted(set(data["hybrid"]) | set(data["all-binius"]))
    rev = subprocess.run(["git", "rev-parse", "--short", "HEAD"], capture_output=True, text=True).stdout.strip()
    dirty = subprocess.run(["git", "status", "--porcelain"], capture_output=True, text=True).stdout.strip() != ""
    lines = [
        f"% {variant['title']} — GENERATED FILE, do not edit by hand.",
        f"% Generated by scripts/hybrid_table.py on {datetime.date.today()} at {rev}{'-dirty' if dirty else ''}: F2Z rows from {args.hybrid}, Binius rows from {args.binius};",
        "%   benches/hybrid_u32_sha256 medians of the verified iterations, one process per case.",
        "% F2Z hybrid: shared opener = Johnson-regime Ligerito at rate 1/2 (opening::LOG_INV_RATE = 1) with Round 0, 106-bit opener component, 100-bit end-to-end gate.",
        "% Binius: rate 1/8, 100-bit FRI query target (121 queries).",
        f"% Machine: {platform.machine()} {platform.platform()}. Columns: prover = total_prover_ms (witness synthesis, commitments, PIOPs, opening, encoding; setup excluded);",
        "%   verifier = verify_ms; proof = proof_bytes, KB = 1000 bytes; peak mem. = external RSS sample of the child process, GB = 2^30 bytes. Bold = best per size and column.",
        "%   dagger = the case paged (swap-outs > 0), so its timings are inflated; compressions = pages the kernel compressed while the case ran (setup included). Raw medians:",
    ]
    for shape in shapes:
        for mode, _ in ROWS:
            row = data[mode].get(shape)
            if row is None:
                continue
            bits = f" bits={row['bits']:.2f}" if row["bits"] is not None else ""
            peak = row["peak_mib"] if row["peak_mib"] is not None else "?"
            lines.append(
                f"%   2^{shape[0]}:2^{shape[1]:<3} {mode:<12} prover={row['prover']:<8.1f} verify={row['verify']:<7.2f} "
                f"proof_bytes={row['proof']} peak_mib={peak} swapouts={row['swapouts']} compressions={row['compressions']} samples={row['samples']}{bits}"
            )
    lines += [
        "",
        r"\begin{table}[H]",
        r"  \centering",
        r"  \small",
        r"  \setlength{\tabcolsep}{4.5pt}",
        r"  \begin{tabular}{@{}rrlrrrr@{}}",
        r"    \toprule",
        r"    $N$ & $M$ & Scheme & Prover (ms) & Verifier (ms) & Proof (KB) & Peak mem.\ (GB) \\",
        r"    \midrule",
    ]
    for i, shape in enumerate(shapes):
        present = [(mode, label, data[mode][shape]) for mode, label in ROWS if shape in data[mode]]
        best = {}
        for key in ("prover", "verify", "proof", "peak_mib"):
            values = [r[key] for _, _, r in present if r[key] is not None]
            best[key] = min(values) if values else None
        for j, (mode, label, row) in enumerate(present):
            dagger = r"$^{\dagger}$" if row["pressured"] else ""
            cells = []
            for key, text in (
                ("prover", sig3(row["prover"])),
                ("verify", sig3(row["verify"])),
                ("proof", f"{row['proof'] / 1000:.0f}"),
                ("peak_mib", sig3(row["peak_mib"] / 1024) if row["peak_mib"] is not None else "--"),
            ):
                bold = row[key] is not None and row[key] == best[key]
                cell = rf"\textbf{{{text}}}" if bold else text
                cells.append(cell + (dagger if key in ("prover", "verify", "peak_mib") and dagger else ""))
            head = rf"    $2^{{{shape[0]}}}$ & $2^{{{shape[1]}}}$" if j == 0 else "     &"
            lines.append(f"{head} & {label} & " + " & ".join(cells) + r" \\")
        if i + 1 < len(shapes):
            lines.append(r"    \addlinespace")
    lines += [
        r"    \bottomrule",
        r"  \end{tabular}",
        rf"  \caption{{{variant['caption']}}}",
        rf"  \label{{{variant['label']}}}",
        r"\end{table}",
    ]
    output.write_text("\n".join(lines) + "\n")
    print(f"wrote {output} ({len(shapes)} shapes)")


if __name__ == "__main__":
    main()
