#!/usr/bin/env python3
"""Render the MultiSwap comparison table (paper label ``t:multiswap``) from a
campaign directory written by the MultiSwap re-measurement queue::

    bitz-t{1,10}.log                       BitZ ``cargo bench --bench multiswap`` (BITZ_BENCH_LAMBDA=114)
    limber-brakedown-t{1,10}-run{1..N}.log Limber ``MSCFG=paper BDPCS=1``, one result line per run
    limber-hyrax-t{1,10}.log               Limber ``MSCFG=paper`` Criterion run (``prove`` = commit + prove)
    limber-hyrax-psize.log                 Limber ``MSCFG=paper PSIZE=1`` (serialized Hyrax proof size)
    zinc-t{1,10}.log                       Zinc+ ``--bench limber_multiswap`` (LIMB16=1 NVARS=13, sec-114)
    provenance.txt                         revisions and knobs, copied into the header

Prover times include the commitment and exclude witness generation. Every cell
is a median: BitZ and Zinc+ print their own medians over their repetitions,
Limber Brakedown is the median over the run files, Limber Hyrax is Criterion's
point estimate over its samples. A missing log leaves ``--`` cells; nothing is
typed in by hand. The two circuit-based baseline rows are quoted from Limber's
Table 1 and marked as such, exactly as in the paper.

    python3 scripts/multiswap_table.py bench_results/<run> --out paper/multiswap-table.tex
"""
from __future__ import annotations

import argparse
import datetime as dt
import json
import re
import statistics
import subprocess
import sys
from pathlib import Path

THREADS = (1, 10)  # overridden by --threads
SCHEMES = (
    ("bitz", r"\ftwoz-SNARK"),
    ("zinc", "Zinc+"),
    ("limber-brakedown", "Limber (Brakedown)"),
    ("limber-hyrax", "Limber (Hyrax)"),
)
QUOTED_ROWS = [
    r"    Arkworks/Garuda$^{\dagger}$      &  231000        & --           & 25           & --   & 7.2 \\",
    r"    MultiSwap-xJsnark$^{\dagger}$     & 88500         & --           & 2            & --   & 0.2 \\",
]


def read(path: Path) -> str | None:
    return path.read_text(errors="replace") if path.exists() else None


def grab(pattern: str, text: str) -> float | None:
    match = re.search(pattern, text)
    return float(match.group(1)) if match else None


def bitz(run: Path, threads: int) -> dict | None:
    text = read(run / f"bitz-t{threads}.log")
    if text is None:
        return None
    prove = grab(r"prove \(end-to-end, median of (?:\d+)\): ([\d.]+) ms", text)
    reps = grab(r"prove \(end-to-end, median of (\d+)\)", text)
    verify = grab(r"verify \(median\): ([\d.]+) ms", text)
    proof = grab(r"proof: (\d+) B", text)
    witness = grab(r"campaign witness generation \(median of \d+\): ([\d.]+) ms", text)
    achieved = grab(r"achieved[^0-9]*([\d.]+)", text)
    if prove is None or verify is None or proof is None:
        raise SystemExit(f"{run}/bitz-t{threads}.log: result lines missing (prove/verify/proof)")
    return {"prove_ms": prove, "verify_ms": verify, "proof_bytes": int(proof), "witness_ms": witness,
            "reps": int(reps) if reps else None, "achieved_bits": achieved}


BRAKEDOWN = re.compile(
    r"Brakedown Mod-PCS: commit ([\d.]+) ms, prove ([\d.]+) ms, total ([\d.]+) ms, "
    r"verify ([\d.]+) ms, proof (\d+) bytes")


def limber_brakedown(run: Path, threads: int) -> dict | None:
    runs = []
    for path in sorted(run.glob(f"limber-brakedown-t{threads}-run*.log")):
        match = BRAKEDOWN.search(read(path) or "")
        if match:
            runs.append(match)
    if not runs:
        return None
    totals = [float(m.group(3)) for m in runs]
    verifies = [float(m.group(4)) for m in runs]
    proofs = {int(m.group(5)) for m in runs}
    if len(proofs) != 1:
        raise SystemExit(f"{run}: Brakedown proof size differs between runs: {sorted(proofs)}")
    return {"prove_ms": statistics.median(totals), "verify_ms": statistics.median(verifies),
            "proof_bytes": proofs.pop(), "runs": len(runs), "prove_runs_ms": totals,
            "verify_runs_ms": verifies,
            "commit_runs_ms": [float(m.group(1)) for m in runs]}


CRITERION = re.compile(
    r"multiswap_modp/(prove|verify)/paper_k0_c2\^13\s*\n\s*time:\s*\[([\d.]+) (\S+) ([\d.]+) (\S+) ([\d.]+) (\S+)\]")
UNIT_MS = {"ns": 1e-6, "µs": 1e-3, "us": 1e-3, "ms": 1.0, "s": 1000.0}


def limber_hyrax(run: Path, threads: int) -> dict | None:
    text = read(run / f"limber-hyrax-t{threads}.log")
    if text is None:
        return None
    found = {}
    for match in CRITERION.finditer(text):
        found[match.group(1)] = float(match.group(4)) * UNIT_MS[match.group(5)]
    if "prove" not in found or "verify" not in found:
        raise SystemExit(f"{run}/limber-hyrax-t{threads}.log: Criterion prove/verify estimates missing")
    size = None
    psize = read(run / "limber-hyrax-psize.log")
    if psize:
        match = re.search(r"eval_arg (\d+) bytes \+ ~(\d+) B", psize)
        if match:
            size = int(match.group(1)) + int(match.group(2))
    return {"prove_ms": found["prove"], "verify_ms": found["verify"], "proof_bytes": size,
            "criterion": True}


ZINC = re.compile(
    r"prove median ([\d.]+) s \| prove\+witgen ([\d.]+) s \| verify median ([\d.]+) s \((\d+) reps\)")


def zinc(run: Path, threads: int) -> dict | None:
    text = read(run / f"zinc-t{threads}.log")
    if text is None:
        return None
    match = ZINC.search(text)
    if not match:
        raise SystemExit(f"{run}/zinc-t{threads}.log: result line missing")
    raw = re.search(r"raw\): ([\d ]+) bytes", text)
    zstd = re.search(r"zstd-3\): ([\d ]+) bytes", text)
    return {"prove_ms": float(match.group(1)) * 1000, "verify_ms": float(match.group(3)) * 1000,
            "witness_ms": (float(match.group(2)) - float(match.group(1))) * 1000,
            "proof_bytes": int(raw.group(1).replace(" ", "")) if raw else None,
            "zstd_bytes": int(zstd.group(1).replace(" ", "")) if zstd else None,
            "reps": int(match.group(4))}


PARSERS = {"bitz": bitz, "zinc": zinc, "limber-brakedown": limber_brakedown, "limber-hyrax": limber_hyrax}


def fmt_prove(ms: float | None) -> str:
    if ms is None:
        return "--"
    return f"{ms:.0f}" if ms >= 100 else f"{ms:.1f}"


def fmt_verify(ms: float | None) -> str:
    return "--" if ms is None else (f"{ms:.1f}" if ms < 100 else f"{ms:.0f}")


def fmt_kb(nbytes: int | None) -> str:
    return "--" if nbytes is None else f"{nbytes / 1000:.0f}"


def render(run: Path, cells: dict, command: str) -> str:
    columns = []  # (getter, formatter) in table order
    for threads in THREADS:
        columns.append((lambda c, t=threads: c.get(t, {}).get("prove_ms") if c.get(t) else None, fmt_prove))
    for threads in THREADS:
        columns.append((lambda c, t=threads: c.get(t, {}).get("verify_ms") if c.get(t) else None, fmt_verify))
    columns.append((lambda c: next((c[t]["proof_bytes"] for t in THREADS if c.get(t) and c[t].get("proof_bytes") is not None), None), fmt_kb))
    values = {scheme: [getter(cells[scheme]) for getter, _ in columns] for scheme, _ in SCHEMES}
    best = []
    for index in range(len(columns)):
        present = [values[s][index] for s, _ in SCHEMES if values[s][index] is not None]
        best.append(min(present) if present else None)
    rows = []
    for scheme, label in SCHEMES:
        parts = [label.ljust(32)]
        for index, (_, formatter) in enumerate(columns):
            value = values[scheme][index]
            text = formatter(value)
            if value is not None and best[index] is not None and value == best[index]:
                text = r"\textbf{" + text + "}"
            parts.append(text)
        rows.append("    " + " & ".join(parts) + r" \\")
    provenance = read(run / "provenance.txt") or "(no provenance.txt)"
    header = [
        f"% Generated by {command}",
        f"% on {dt.datetime.now(dt.timezone.utc):%Y-%m-%d %H:%M} (UTC) from {run}",
        "% Prover times include the commitment and exclude witness generation;",
        "% medians (BitZ, Zinc+: their own; Limber Brakedown: over the run files;",
        "% Limber Hyrax: Criterion's point estimate). Proof sizes uncompressed.",
        "% provenance.txt:",
    ] + [f"%   {line}" for line in provenance.strip().splitlines()] + ["% per-row medians:"]
    for scheme, label in SCHEMES:
        for threads in THREADS:
            cell = cells[scheme].get(threads)
            if cell:
                extra = {k: v for k, v in cell.items() if k not in ("prove_ms", "verify_ms", "proof_bytes")}
                header.append(f"%   {scheme} {threads} thr: prove {cell['prove_ms']:.1f} ms, verify {cell['verify_ms']:.2f} ms, "
                              f"proof {cell['proof_bytes']} B; {json.dumps(extra, sort_keys=True)}")
            else:
                header.append(f"%   {scheme} {threads} thr: MISSING")
    intro = []
    b, l, z = cells["bitz"], cells["limber-brakedown"], cells["zinc"]
    if all(b.get(t) for t in THREADS) and all(l.get(t) for t in THREADS):
        group = [("RSA and & \\ftwoz-SNARK, rate $1/8$", b), ("Poseidon \\cite{limber} & Limber", l)]
        if all(z.get(t) for t in THREADS):
            group.append((" & Zinc+", z))
        printed = [[fmt_prove(c[1]["prove_ms"]), fmt_prove(c[10]["prove_ms"]), fmt_verify(c[1]["verify_ms"]),
                    fmt_verify(c[10]["verify_ms"]), fmt_kb(c[1]["proof_bytes"])] for _, c in group]
        best = [min(float(p[i]) for p in printed) for i in range(5)]
        intro = ["% intro table rows (tab:intro_table), same medians; bold = best of the group as printed:"]
        for (label, _), p in zip(group, printed):
            tex_cells = " & ".join((r"\textbf{" + v + "}") if float(v) == best[i] else v for i, v in enumerate(p))
            intro.append(f"%   {label} & {tex_cells} & -- \\\\")
    table = [
        r"\begin{table}[H]",
        r"  \centering",
        r"  \begin{tabular}{@{}llrrrrr@{}}",
        r"    \toprule",
        r"    Scheme &  \multicolumn{2}{c}{Prover (ms)} & \multicolumn{2}{c}{Verifier (ms)} & Proof \\",
        r"    \cmidrule(lr){3-4} \cmidrule(lr){5-6}",
        r"      & 1 thr & 10 thr & 1 thr & 10 thr & (KB) \\",
        r"    \midrule",
        *rows,
        r"    \midrule",
        *QUOTED_ROWS,
        r"    \bottomrule",
        r"  \end{tabular}",
        r"  \caption{Proving one MultiSwap \cite{multiswap} computation (four exponentiations modulo a $2048$-bit RSA modulus, with $352$-bit exponents, plus a Poseidon-based hash-to-prime evaluation). The first four schemes prove the same $6209$-row integer Mod-R1CS statement from Limber's repository at $114$ bits of security, measured on the same machine (Apple M5, 24\,GB) with one and ten threads as medians; prover time includes the commitment and excludes witness generation. $^{\dagger}$ Quoted from \cite{limber}; not measured here.}",
        r"  \label{t:multiswap}",
        r"\end{table}",
    ]
    return "\n".join(header + intro + [""] + table) + "\n"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("run", type=Path, help="campaign directory")
    parser.add_argument("--out", type=Path, help="LaTeX table path (default: stdout)")
    parser.add_argument("--json", type=Path, help="also write the parsed metrics here")
    parser.add_argument("--threads", default="1,10",
                        help="thread counts to tabulate, comma-separated (each needs its <scheme>-t<N>.log; default 1,10)")
    args = parser.parse_args()
    global THREADS
    THREADS = tuple(int(t) for t in args.threads.split(",") if t)
    cells = {scheme: {} for scheme, _ in SCHEMES}
    for scheme, _ in SCHEMES:
        for threads in THREADS:
            cell = PARSERS[scheme](args.run, threads)
            if cell:
                cells[scheme][threads] = cell
    command = "scripts/multiswap_table.py " + " ".join(sys.argv[1:])
    tex = render(args.run, cells, command)
    if args.out:
        args.out.write_text(tex)
        print(f"wrote {args.out}")
    else:
        sys.stdout.write(tex)
    if args.json:
        args.json.write_text(json.dumps(cells, indent=2, sort_keys=True) + "\n")
    missing = [(s, t) for s, _ in SCHEMES for t in THREADS if t not in cells[s]]
    if missing:
        print("missing cells: " + ", ".join(f"{s}@{t}" for s, t in missing), file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
