#!/usr/bin/env python3
"""LaTeX table for the SHA-256-only comparison (bench_results/r0917-sha-chain).

Reads `<scheme>-t<threads>-e<k>.out` (one process per cell under
`/usr/bin/time -l`) and writes the suite layout: Witgen, Prover 1|10 thr,
Verifier 1|10 thr, Proof, Peak mem.; Witgen and Peak mem. from the 10-thread
runs; proof sizes must agree across thread counts. Bold = best displayed value
per column within a size group.

    python3 scripts/sha256_table.py <out-dir> <table.tex> [--with-flock] [--sizes ...]

`--with-flock` appends the two Flock rows (measured in the same campaign but
left out of the paper table by default, cf. the ``Non-comparisons'' paragraph
of the experiments section).  `--sizes` selects the compression counts shown;
the campaign covers 7..14, but eight groups of six rows do not fit one page,
so the paper table shows every other exponent (as `tab:fields-witch` does).
"""
import argparse
import json
import re
import subprocess
from pathlib import Path

F2Z_SCHEMES = [
    ("f2z-ind-r2", r"\ftwoz-SNARK, rate $1/2$"),
    ("f2z-ind-r8", r"\ftwoz-SNARK, rate $1/8$"),
]
BINIUS_SCHEMES = [
    ("bin-r1", r"Binius (UDR), rate $1/2$"),
    ("bin-r3", r"Binius (UDR), rate $1/8$"),
    ("lig-r1", r"Binius (Johnson), rate $1/2$"),
    ("lig-r3", r"Binius (Johnson), rate $1/8$"),
]
FLOCK_SCHEMES = [
    ("flock-fast100", r"Flock, rate $1/2$"),
    ("flock-slim100", r"Flock, rate $1/4$"),
]
SIZES = [8, 10, 12, 14]
ALL_SIZES = list(range(7, 15))


def parse(path: Path):
    if not path.is_file():
        return None
    text = path.read_text(errors="replace")
    rss = re.search(r"(\d+)\s+maximum resident set size", text)
    out = None
    for line in text.splitlines():
        line = line.strip()
        if line.startswith("SHA_CHAIN_RESULT "):
            r = json.loads(line[len("SHA_CHAIN_RESULT "):])
            out = dict(witness=r["witness_ms"], prove=r["prove_ms"], verify=r["verify_ms"],
                       proof=r["proof_bytes"], threads=r["threads"])
        elif line.startswith("RESULT schema=f2z") or line.startswith("RESULT schema=bitz"):
            kv = dict(tok.split("=", 1) for tok in line.split()[1:] if "=" in tok)
            out = dict(witness=float(kv["witness_ms"]), prove=float(kv["prove_ms"]),
                       verify=float(kv["verify_ms"]), proof=int(kv["proof_bytes"]),
                       threads=int(kv["threads"]))
    if out is None:
        return None
    out["rss"] = int(rss.group(1)) if rss else None
    return out


def fmt_ms(v):
    if v is None:
        return "--"
    return f"{v:.0f}" if v >= 100 else f"{v:.1f}" if v >= 10 else f"{v:.2f}"


def fmt_kb(b):
    kb = b / 1000
    return f"{kb:.0f}" if kb >= 100 else f"{kb:.1f}"


def fmt_gb(b):
    gb = b / 2**30
    return f"{gb:.2f}" if gb < 10 else f"{gb:.1f}"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("out_dir", type=Path)
    ap.add_argument("tex_path", type=Path)
    ap.add_argument("--with-flock", action="store_true")
    ap.add_argument("--sizes", type=int, nargs="+", default=SIZES,
                    help=f"compression exponents to show (measured: {ALL_SIZES})")
    args = ap.parse_args()
    out_dir, tex_path = args.out_dir, args.tex_path
    schemes = F2Z_SCHEMES + BINIUS_SCHEMES + (FLOCK_SCHEMES if args.with_flock else [])
    ncols = 7

    lines, missing = [], []
    for k in args.sizes:
        cells = []
        for key, name in schemes:
            t10 = parse(out_dir / f"{key}-t10-e{k}.out")
            t1 = parse(out_dir / f"{key}-t1-e{k}.out")
            if t10 is None and t1 is None:
                missing.append(f"{key}@{k}")
                cells.append((name, ["--"] * ncols))
                continue
            if t10 and t1 and t10["proof"] != t1["proof"]:
                print(f"note: {key} 2^{k} proof size differs across threads: {t1['proof']} vs {t10['proof']}")
            ref = t10 or t1
            cells.append((name, [
                fmt_ms(ref["witness"]),
                fmt_ms(t1["prove"]) if t1 else "--", fmt_ms(t10["prove"]) if t10 else "--",
                fmt_ms(t1["verify"]) if t1 else "--", fmt_ms(t10["verify"]) if t10 else "--",
                fmt_kb(ref["proof"]),
                fmt_gb(t10["rss"]) if t10 and t10["rss"] else "--",
            ]))
        best = []
        for i in range(ncols):
            vals = [float(c[1][i]) for c in cells if c[1][i] != "--"]
            best.append(min(vals) if vals else None)
        for j, (name, vals) in enumerate(cells):
            shown = [rf"\textbf{{{v}}}" if v != "--" and best[i] is not None and float(v) == best[i] else v
                     for i, v in enumerate(vals)]
            label = f"$2^{{{k}}}$" if j == 0 else ""
            lines.append(f"    {label} & {name} & " + " & ".join(shown) + r" \\")
        lines.append(r"    \addlinespace")
    lines = lines[:-1]

    rev = subprocess.run(["git", "rev-parse", "--short", "HEAD"], capture_output=True,
                         text=True, cwd=Path(__file__).resolve().parent.parent).stdout.strip()
    flock_caption = (
        r"Flock is the last revision carrying its SHA-256 chain prover "
        r"(\texttt{fea8877}; endpoints public; \texttt{fast100} at rate $1/2$ and "
        r"\texttt{slim100} at rate $1/4$, its 100-bit query floor). "
    ) if args.with_flock else ""
    tex = "\n".join([
        r"% SHA-256-only comparison --- GENERATED FILE, do not edit by hand.",
        rf"% Generated by scripts/sha256_table.py at {rev} from {out_dir}",
        rf"% Sizes shown: {' '.join('2^'+str(k) for k in args.sizes)} (measured: {' '.join('2^'+str(k) for k in ALL_SIZES)}).",
        r"% Campaign of 2026-09-17 (BitZ at 66d5ab2, Binius64 at bc73510, Flock at fea8877):",
        r"%   medians of 5 after one warm-up, one process per cell under /usr/bin/time -l on an",
        r"%   otherwise idle box; Witgen and Peak mem. from the 10-thread runs; every measured",
        r"%   proof is verified; KB = 1000 bytes, GB = 2^30 bytes. Bold = best row of the size group.",
        rf"% Include with \input{{{tex_path.stem}}} (relative to paper/). Regenerate with",
        rf"%   python3 scripts/sha256_table.py {out_dir} {tex_path}"
        + ("  --with-flock" if args.with_flock else "")
        + ("" if list(args.sizes) == SIZES else "  --sizes " + " ".join(str(k) for k in args.sizes)),
        r"\begin{table}[H]", r"  \centering", r"  \small", r"  \setlength{\tabcolsep}{4pt}",
        r"  \begin{tabular}{@{}rlrrrrrrr@{}}", r"    \toprule",
        r"     &  & Witgen & \multicolumn{2}{c}{Prover (ms)} & \multicolumn{2}{c}{Verifier (ms)} & Proof & Peak mem. \\",
        r"    \cmidrule(lr){4-5} \cmidrule(lr){6-7}",
        r"    $N$ & Scheme & (ms) & 1 thr & 10 thr & 1 thr & 10 thr & (KB) & (GB) \\",
        r"    \midrule",
        *lines,
        r"    \bottomrule", r"  \end{tabular}",
        r"  \caption{Cost of proving $N$ SHA-256 compressions, $\lambda = 100$ bits. "
        r"\ftwoz-SNARK proves $N$ independent compressions (every input public) as a single "
        r"$\mathsf{CM}$ instance over $\ZZ$, the assignment being $\FF_2$-virtualized through "
        r"$\mathrm{Id}_{2^r}\otimes M$ with $r$ chosen so that the grand-product tensor split is "
        r"near $\codedim^{0.6}$; Binius proves a chain of $N$ compressions from the standard IV "
        r"(blocks witness, final chaining value public), Binius (UDR) with its own BaseFold "
        r"opening and Binius (Johnson) with the \ftwoz\ opener, cf.\ \cref{s:binius_suite}. "
        + flock_caption +
        r"\emph{Prover} excludes witness generation, which is reported separately (\emph{Witgen}). "
        r"Ran on a MacBook Air M5, 24\,GB, CPU only.}",
        r"  \label{tab:sha256}", r"\end{table}", ""])
    tex_path.write_text(tex)
    print(f"wrote {tex_path}; missing cells: {missing if missing else 'none'}")


if __name__ == "__main__":
    main()
