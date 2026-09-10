#!/usr/bin/env python3
"""LaTeX table of absolute numbers from a native-mul run (benches/mul_e2e_compare.rs).

Reads the `summary.json` of one or more `PerfRuns/<stamp>-native-mul` run
directories (later directories override earlier ones for the same scheme and
size, so a follow-up run of the large sizes extends a first run) and writes a self-documenting
`paper/native-mul-table.tex`: one row group per size N, one row per scheme, with
the medians of witness generation, the complete prover (commit included) and the
complete verifier, plus complete proof size and isolated peak memory.

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

from native_mul_results import SAMPLE_SCHEMA, finite_number, require_compatible, validate_summary

# Table rows are (scheme key, LaTeX label). Binius64 appears once per BaseFold
# rate it was run at: scheme key `binius64@<log_inv_rate>` (rate 1/2 is the
# Binius64 default, rate 1/8 needs the fewest queries that still pay off).
SCHEMES = [
    ("f2z", "\\ftwoz\\ (this work)"),
    ("binius64@1", "Binius64~\\cite{binius64}, rate $1/2$"),
    ("binius64@3", "Binius64~\\cite{binius64}, rate $1/8$"),
    # Binius64's circuit and PIOP with F2Z's opener (rate 1/8, Johnson regime,
    # grinding, Round 0), gated at 100 bits by a whole-protocol union bound.
    ("binius64-ligerito", "Binius64~\\cite{binius64} + \\ftwoz\\ opener, rate $1/8$"),
    ("plonky3-fri", "Plonky3~\\cite{plonky} (FRI)"),
    ("plonky3-whir", "Plonky3~\\cite{plonky} (WHIR)"),
    ("limber", "Limber~\\cite{limber} (Brakedown)"),
]
BINIUS_QUERIES = {1: 241, 2: 148, 3: 121, 4: 110}  # 100-bit FRI query counts per log inverse rate


def f2z_caption(rows):
    """Describe the recorded Ligerito policy; reject mixed or historical series."""
    from ligerito_results import validate_ligerito
    policies = set()
    for row in rows:
        if row["backend"] == "f2z":
            report = validate_ligerito(row.get("config", {}).get("ligerito"), 100)
            policies.add((report["resolved_profile"], report["regime"], report["outer_ood"]))
    if not policies:
        return r"\ftwoz\ (integer R1CS)"
    if len(policies) != 1:
        raise ValueError("cannot combine different Ligerito policies in one F2Z table series")
    profile, regime, ood = policies.pop()
    bound = "Johnson" if regime == "johnson" else "unique decoding radius"
    evaluation = "early Round-0 OOD" if ood else "without OOD"
    return rf"\ftwoz\ (integer R1CS, Lambda100; Ligerito {bound}, {profile}, {evaluation})"


def scheme_key(r: dict) -> str:
    """Row key: the backend slug, with Binius64 split by its recorded rate."""
    if r["backend"] == "binius64":
        return f"binius64@{int(r['config'].get('log_inv_rate', 1))}"
    return r["backend"]


def scheme_name(key: str) -> str:
    """Short scheme name for caption sentences."""
    names = {"f2z": "\\ftwoz", "plonky3-fri": "Plonky3-FRI", "plonky3-whir": "Plonky3-WHIR", "limber": "Limber",
             "binius64-ligerito": "Binius64 with the \\ftwoz\\ opener"}
    if key in names:
        return names[key]
    return "Binius64 at rate $1/%d$" % (1 << int(key.split("@")[1]))
PLACEHOLDER = "--"


def fmt_ms(v: float) -> str:
    if v >= 100:
        return f"{v:.0f}"
    if v >= 10:
        return f"{v:.1f}"
    return f"{v:.2f}"


def fmt_gb(peak_rss_bytes: float) -> str:
    """Peak resident set in GiB, as the memory-wall discussion reports it."""
    v = peak_rss_bytes / (1 << 30)
    if v >= 10:
        return f"{v:.1f}"
    if v >= 1:
        return f"{v:.2f}"
    return f"{v:.3f}"


def load_summaries(run_dir: Path, workload: str) -> dict:
    result = {}
    for row in json.loads((run_dir / "summary.json").read_text()):
        if row["workload"] != workload:
            continue
        validate_summary(row)
        key = (scheme_key(row), row["log_multiplications"])
        if key in result:
            raise ValueError("duplicate summary in run")
        result[key] = row
    return result


def proof_sizes(run_dir: Path, workload: str) -> dict:
    """Verified measured payload sizes, tied to the matching protocol summary."""
    summaries = load_summaries(run_dir, workload)
    by = {}
    for line in (run_dir / "samples.jsonl").read_text().splitlines():
        row = json.loads(line)
        if row["workload"] != workload or row["trial"]["kind"] != "sample":
            continue
        key = (scheme_key(row), row["log_multiplications"])
        if key not in summaries or row.get("schema") != SAMPLE_SCHEMA or row.get("proof_verified") is not True:
            raise ValueError("proof-size sample has no compatible verified summary")
        require_compatible(summaries[key], row | {"schema": summaries[key]["schema"]})
        size = finite_number(row["metrics"].get("proof_bytes"), "proof_bytes", positive=True)
        by.setdefault(key, []).append((row["trial"]["index"], size))
    for key, summary in summaries.items():
        if [index for index, _ in by.get(key, [])] != list(range(summary["samples"])):
            raise ValueError("proof-size samples have missing or duplicate repetitions")
    return {key: statistics.median(size for _, size in values) for key, values in by.items()}


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
    ap.add_argument("--workload", default="u32-mod32", choices=["u32-mod32", "u32", "u64", "u128"])
    ap.add_argument("--memory-bound", default="", metavar="BACKEND:EXP[,...]",
                    help="rows measured while paging (prover exceeded the machine's memory): omitted from the table and noted in the caption")
    ap.add_argument("--drop", default="", metavar="SCHEME:EXP[,...]",
                    help="rows to leave out as if not run (e.g. a measurement taken on a memory-starved box, pending a re-run); scheme keys as in --memory-bound")
    ap.add_argument("--proof-sizes-from", default="", metavar="RUN_DIR[,...]",
                    help="run directories that only contribute proof_bytes (only when protocol, corpus, source, machine and measurement policy match")
    args = ap.parse_args()
    if args.workload == "u32":
        args.workload = "u32-mod32"
    memory_bound = set()  # (scheme key, exponent); a bare `binius64` applies to every rate
    for item in filter(None, args.memory_bound.split(",")):
        backend, exp = item.split(":")
        memory_bound.add((backend, int(exp)))
    if args.out is None:
        args.out = Path("paper/native-mul-table.tex" if args.workload == "u32-mod32" else f"paper/native-mul-{args.workload}-table.tex")

    by = {}
    for run_dir in args.run_dirs:
        for key, row in load_summaries(run_dir, args.workload).items():
            if key in by:
                require_compatible(by[key], row)
            row["run_dir"] = str(run_dir)
            by[key] = row
    identities = {}
    source_by_backend = {}
    machines = [row["provenance"]["machine"] for row in by.values()]
    if any(machine != machines[0] for machine in machines):
        raise ValueError("comparison mixes measurement machines")
    for row in by.values():
        source = row["provenance"]
        source_identity = (source["source_sha256"], source["build"])
        backend = row["backend"]
        if backend in source_by_backend and source_by_backend[backend] != source_identity:
            raise ValueError("comparison mixes source revisions or builds for one backend")
        source_by_backend[backend] = source_identity
        exponent = row["log_multiplications"]
        identity = (row["corpus_digest"], row["threads"], row["measurement_policy"], row["provenance"]["machine"])
        if exponent in identities and identities[exponent] != identity:
            raise ValueError("comparison mixes corpora, machines, or measurement policies")
        identities[exponent] = identity
    dropped = set()
    for item in filter(None, args.drop.split(",")):
        scheme, exp = item.split(":")
        dropped.add((scheme, int(exp)))
    for k in list(by):
        if k in dropped or (k[0].split("@")[0], k[1]) in dropped:
            by.pop(k)
    paged = {k: by.pop(k) for k in list(by) if k in memory_bound or (k[0].split("@")[0], k[1]) in memory_bound}
    # Proof sizes: the row's own run first, then the proof-size-only runs.
    sizes = {}
    for run_dir in args.run_dirs:
        sizes.update(proof_sizes(run_dir, args.workload))
    size_dirs = [Path(d) for d in filter(None, args.proof_sizes_from.split(","))]
    for run_dir in size_dirs:
        for key, row in load_summaries(run_dir, args.workload).items():
            if key in by:
                require_compatible(by[key], row)
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
    cpu = rows[0]["provenance"]["machine"]["cpu"]
    thread_counts = ", ".join(map(str, sorted({r["threads"] for r in rows})))
    machine_text = cpu.replace("_", r"\_")
    commit = ", ".join(sorted({r["provenance"]["git_revision"] + ("+dirty" if r["provenance"]["git_dirty"] else "") for r in rows}))
    date = datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%d")

    out = []
    w = out.append
    w("% Native end-to-end u32-multiplication comparison (F2Z vs Binius64 vs Plonky3/FRI) — GENERATED FILE, do not edit by hand.")
    w(f"% Generated by scripts/native_mul_table.py on {date} (UTC) at {commit} from {run_list} (benches/mul_e2e_compare.rs,")
    w("%   see docs/native-mul-compare.md; later run directories override earlier ones per scheme and size).")
    if size_dirs:
        w(f"% Proof sizes for rows whose run predates proof-size recording come from {' '.join(map(str, size_dirs))}.")
    w("% Regenerate (from the repo root; this file is overwritten):")
    w(f"%   python3 scripts/native_mul_table.py {run_list} --workload {args.workload}" + (f" --exponents {args.exponents}" if args.exponents else "")
      + (f" --memory-bound {args.memory_bound}" if args.memory_bound else "") + (f" --drop {args.drop}" if args.drop else "") + (f" --proof-sizes-from {args.proof_sizes_from}" if args.proof_sizes_from else ""))
    w(f"% Machine: {cpu}; medians of {'/'.join(map(str, samples))} samples after one warm-up.")
    w("% Columns: witgen = witness_ms (native witness generation; Binius64 packs its witness inside its prover, so its witgen")
    w("%   overlaps the prover column); prover = online_prover_ms (the complete native prover call after witness generation,")
    w("%   i.e. commitment + PIOP + PCS opening); verifier = verify_ms; proof = median proof_bytes of the samples, KB = 1000 bytes;")
    w("%   peak mem. = peak_rss_bytes of the separate single-proof memory child, GB = 2^30 bytes (`--` where the run predates it).")
    w("% Bold = best of the schemes for that size and column.")
    w("% Medians (ms) as recorded in summary.json (`--` rows below = scheme not run at that size):")
    for e in exps:
        for slug, _ in SCHEMES:
            r = by.get((slug, e))
            if r:
                m = r["medians"]
                size = sizes.get((slug, e))
                w(f"%   2^{e} {slug:13s} witness={m['witness_ms']:.3f} "
                  f"online_prover={m['online_prover_ms']:.3f} witness_to_proof={m['witness_to_proof_ms']:.3f} verify={m['verify_ms']:.3f} setup={r['setup_ms']:.3f} "
                  f"proof_bytes={'n/a' if size is None else int(size)} samples={r['samples']} run={Path(r['run_dir']).name}")
            else:
                w(f"%   2^{e} {slug:13s} not run")
    # Sizes above a scheme's memory wall are implied by the wall, not "not run".
    wall = {}
    for slug, e in paged:
        wall[slug] = min(wall.get(slug, e), e)
    missing = {}
    for slug, label in SCHEMES:
        gone = [e for e in exps if (slug, e) not in by and (slug, e) not in paged and e < wall.get(slug, float("inf"))]
        if gone and any((slug, e) in by for e in exps):
            missing[slug] = gone
    for (slug, e), r in sorted(paged.items()):
        m = r["medians"]
        w(f"%   2^{e} {slug:13s} OMITTED (paging): online_prover={m['online_prover_ms']:.3f} verify={m['verify_ms']:.3f} run={Path(r['run_dir']).name}")
    # One paging sentence per wall exponent; the Binius64 rates share it when they agree.
    walls = {}
    for slug, e in sorted(wall.items()):
        walls.setdefault(e, []).append(slug)
    def wall_sentence(e: int, slugs: list) -> str:
        families = {slug.split("@")[0] for slug in slugs}
        if families == {"binius64"} and len(slugs) > 1:
            who = "the Binius64 provers at both rates exceed"
        else:
            who = " and ".join(f"the {scheme_name(slug)} prover" for slug in slugs) + (" exceeds" if len(slugs) == 1 else " exceed")
        return f"From $2^{{{e}}}$ {who} the machine's memory and page" + ("s" if len(slugs) == 1 else "") + "; those rows are omitted. "
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
        present_peaks = {slug: by[(slug, e)]["peak_rss_bytes"] for slug in present
                         if by[(slug, e)].get("peak_rss_bytes")}
        best_peak = min(present_peaks.values()) if present_peaks else None
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
            peak = present_peaks.get(slug)
            if peak is None:
                peak_cell = PLACEHOLDER
            else:
                s = fmt_gb(peak)
                peak_cell = f"\\textbf{{{s}}}" if peak == best_peak and len(present_peaks) > 1 else s
            w(f"    {n_cell} & {label} & {cells[0]} & {cells[1]} & {cells[2]} & {size_cell} & {peak_cell} \\\\")
        if gi + 1 < len(exps):
            w("    \\addlinespace")
    w("    \\bottomrule")
    w("  \\end{tabular}")
    statement = {
        "u32-mod32": "Native end-to-end proofs of $N$ independent multiplications $z = xy \\bmod 2^{32}$, with $x,y,z$ unsigned $32$-bit integers: ",
        "u64": "Native end-to-end proofs of $N$ multiplications $x \\cdot y = z$ of random $64$-bit integers ($z$ a $128$-bit integer): ",
        "u128": "Native end-to-end proofs of $N$ multiplications $x \\cdot y = z$ of random $128$-bit integers ($z$ a $256$-bit integer): ",
    }[args.workload]
    # Binius64's opener geometry as the runs recorded it (rate override
    # F2Z_BINIUS_LOG_INV_RATE; the query count follows from the rate).
    binius_keys = [slug for slug, _ in schemes if slug.startswith("binius64@")]
    binius_rate_text = ""
    if binius_keys:
        parts = []
        for key in binius_keys:
            log_rate = int(key.split("@")[1])
            queries = next((int(r["config"].get("fri_queries", BINIUS_QUERIES.get(log_rate, 0))) for r in rows if scheme_key(r) == key), BINIUS_QUERIES.get(log_rate, 0))
            parts.append(f"rate $1/{1 << log_rate}$ with ${queries}$ queries")
        binius_rate_text = " and ".join(parts) + " for $100$ bits"
    binius_note = "Binius64 (native multiplication" + (" and bit constraints" if args.workload not in ("u64", "u128") else "") + f", ring switching and BaseFold at {binius_rate_text})"
    # The opener geometry of the binius64-ligerito rows as the runs recorded it.
    lig = next((r["config"] for r in rows if scheme_key(r) == "binius64-ligerito"), {})
    lig_bits = lig.get("whole_protocol_bits")
    ligerito_note = ("Binius64 with the \\ftwoz\\ opener (the same circuit and PIOP; every oracle committed at rate $1/8$ and opened by "
                     "ring switching and Johnson-regime Ligerito"
                     + (f" with ${lig['level0_queries']}$ level-0 queries" if "level0_queries" in lig else "")
                     + (f", ${lig['level0_fold_grinding_bits']}$ bits of fold grinding" if "level0_fold_grinding_bits" in lig else "")
                     + " and Round~0; whole-protocol union bound gated at $100$ bits"
                     + (f", ${lig_bits:.1f}$ achieved" if isinstance(lig_bits, (int, float)) else "")
                     + ")")
    scheme_notes = {
        "f2z": "\\ftwoz\\ (Spartan over a transcript-sampled prime with the \\ftwoz\\ opening, $\\lambda = 100$)",
        "binius64": binius_note,
        "binius64-ligerito": ligerito_note,
        "plonky3-fri": "Plonky3 (Goldilocks AIR, degree-$5$ extension, FRI at rate $1/8$, $100$ queries, proven round-by-round target $100$ bits)",
        "plonky3-whir": "Plonky3 (shared Goldilocks mod32 AIR, multilinear zerocheck/sumcheck, WHIR with per-run tuning and at least $100$ bits under the recorded Johnson accounting)",
        "limber": "Limber (one independent integer-mod R1CS row per multiplication, IntEval/Brakedown, native approximately $114$-bit policy)",
    }
    scheme_notes["f2z"] = f2z_caption(rows)
    # One caption note per scheme family (both Binius64 rates share one).
    present = []
    for slug, _ in schemes:
        family = slug.split("@")[0]
        if family not in present:
            present.append(family)
    joiner = ", and " if len(present) > 2 else (" and " if len(present) == 2 else "")
    scheme_text = ", ".join(scheme_notes[s] for s in present[:-1]) + joiner + scheme_notes[present[-1]] + ". Native security targets are reported separately; these are not a uniform complete-protocol bound. "
    w("  \\caption{" + statement + scheme_text
      + "\\emph{Witgen} is the native witness generation; \\emph{prover} is the complete prover call after witness generation, "
      + "commitment included; \\emph{verifier} is the complete verification; \\emph{peak mem.} is the high-water resident set "
      + "of a separate child process proving and verifying once ($1$\\,GB $= 2^{30}$ bytes). "
      + "".join(f"{scheme_name(slug)} was not run at " + ", ".join(f"$2^{{{e}}}$" for e in gone) + ". " for slug, gone in missing.items())
      + "".join(wall_sentence(e, slugs) for e, slugs in sorted(walls.items()))
      + f"{machine_text}; threads per run: {thread_counts}; medians of {samples[0]} runs after one warm-up.}}")
    w("  \\label{tab:native-mul" + ("" if args.workload == "u32-mod32" else "-" + args.workload) + "}")
    w("\\end{table}")
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text("\n".join(out) + "\n")
    print(f"wrote {args.out} ({len(exps)} sizes, {len(schemes)} schemes)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
