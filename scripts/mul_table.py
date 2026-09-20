#!/usr/bin/env python3
"""LaTeX table of one multiplication workload from `mul-bench/v2` campaigns.

Reads one or more campaign directories written by
`scripts/run_multiplication_benchmarks.py` (BitZ and the native backends) and
`scripts/run_zinc_plus_campaign.py` (the external Zinc+ worker), keeps the
`proof`-mode cases of `--workload`, and writes a self-documenting
`paper/native-mul-<workload>-table.tex`: one row group per size `N`, one row
per scheme, the medians of witness generation, the complete prover
(commitment included) and the complete verifier at every thread count, plus
the proof size and the worker's peak resident set.

Cells a scheme could not measure are never dropped silently. An exclusions
file (`--exclusions`; a JSON list, or one JSON object per line as
`scripts/mul_memory_probe.py` appends them, keeping the records whose `status`
is `excluded`) lists them with the reason, the observed peak resident set and
the machine's memory, and the table prints them as `--`
with the reason in the caption and the header. Every measured row of one size
must carry the same corpus digest, and every row must come from the same
machine; the generator refuses otherwise.

    python3 scripts/mul_table.py PerfRuns/cs-u64-* --workload u64 --exclusions PerfRuns/cs-mul-exclusions.jsonl
"""
from __future__ import annotations

import argparse
import datetime
import json
import platform
import subprocess
from pathlib import Path

from mul_results import aggregate, load

# Table rows are (scheme key, LaTeX label), in display order. Keys are
# `backend@log_inv_rate` for the rate-parameterised schemes.
SCHEMES = [
    ("bitz@1", "\\ftwoz-SNARK, rate $1/2$"),
    ("bitz@3", "\\ftwoz-SNARK, rate $1/8$"),
    ("binius64@1", "Binius (UDR), rate $1/2$"),
    ("binius64@3", "Binius (UDR), rate $1/8$"),
    ("binius64-ligerito-rbr@1", "Binius (Johnson), rate $1/2$"),
    ("binius64-ligerito-rbr@3", "Binius (Johnson), rate $1/8$"),
    ("plonky3-fri@1", "Plonky3 (FRI), rate $1/2$"),
    ("limber", "Limber (Brakedown)"),
    ("zinc-plus@2", "Zinc+, rate $1/4$"),
]
NAMES = {"bitz": "\\ftwoz-SNARK", "binius64": "Binius (UDR)", "binius64-ligerito-rbr": "Binius (Johnson)",
         "plonky3-fri": "Plonky3 (FRI)", "limber": "Limber", "zinc-plus": "Zinc+"}
PLACEHOLDER = "--"
WORKLOAD_ALIASES = {"u32": "u32-mod32"}


def scheme_key(case: dict) -> str:
    backend = case["backend"]
    if backend == "bitz":
        profile = (case.get("bitz") or {}).get("ligerito") or ""
        parts = profile.split(":")
        if len(parts) < 2 or not parts[1].isdigit():
            raise ValueError(f"cannot read the Ligerito rate from BitZ profile {profile!r}")
        return f"bitz@{int(parts[1])}"
    if backend == "binius64-ligerito":
        accounting = case.get("binius_ligerito_accounting") or "union"
        return f"binius64-ligerito-{accounting}@{int(case.get('log_inv_rate') or 1)}"
    if backend in ("binius64", "plonky3-fri", "zinc-plus"):
        return f"{backend}@{int(case.get('log_inv_rate') or 1)}"
    return backend


def scheme_name(key: str) -> str:
    family, _, rate = key.partition("@")
    name = NAMES.get(family, family)
    return name + (f" at rate $1/{1 << int(rate)}$" if rate else "")


def fmt_ms(v: float) -> str:
    if v >= 100:
        return f"{v:.0f}"
    if v >= 10:
        return f"{v:.1f}"
    return f"{v:.2f}"


def fmt_gb(b: float) -> str:
    v = b / (1 << 30)
    if v >= 10:
        return f"{v:.1f}"
    if v >= 1:
        return f"{v:.2f}"
    return f"{v:.3f}"


def probe(cmd, default):
    try:
        return subprocess.run(cmd, capture_output=True, text=True, check=True).stdout.strip() or default
    except Exception:
        return default


def load_rows(run_dirs, workload):
    """Measured proof-mode rows of `workload`, keyed by (scheme, log_n, threads)."""
    rows, skipped = {}, []
    for run_dir in run_dirs:
        for row in aggregate(load(run_dir)):
            case = row["case"]
            if case["mode"] != "proof" or WORKLOAD_ALIASES.get(case["workload"], case["workload"]) != workload:
                continue
            if row["status"] != "measured":
                skipped.append((run_dir, case, row["reason"]))
                continue
            key = (scheme_key(case), case["log_n"], case["threads"])
            if key in rows:
                raise ValueError(f"{key} measured in both {rows[key]['source']} and {row['source']}; choose one directory per case")
            row["run_dir"] = str(run_dir)
            rows[key] = row
    return rows, skipped


def load_exclusions(path, workload):
    if path is None:
        return []
    raw = Path(path).read_text()
    if raw.lstrip().startswith("["):
        entries = json.loads(raw)
    else:
        entries = [json.loads(line) for line in raw.splitlines() if line.strip()]
    out = []
    for entry in entries:
        if entry.get("status", "excluded") != "excluded":
            continue
        if WORKLOAD_ALIASES.get(entry["workload"], entry["workload"]) != workload:
            continue
        for field in ("scheme", "log_n", "reason"):
            if field not in entry:
                raise ValueError(f"exclusion lacks {field}: {entry}")
        out.append(entry)
    return out


def main(argv=None) -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("run_dirs", type=Path, nargs="+", metavar="RUN_DIR")
    ap.add_argument("--workload", required=True, choices=["u32-mod32", "u32", "u64", "u128"])
    ap.add_argument("--exclusions", type=Path, default=None, help="excluded cells with reasons and observed peaks: a JSON list or the probe's JSONL")
    ap.add_argument("--exponents", default="", help="comma list or lo-hi of sizes to show; default every measured size")
    ap.add_argument("--out", type=Path, default=None, help="default paper/native-mul-<workload>-table.tex")
    ap.add_argument("--label", default=None, help="default tab:native-mul[-<workload>]")
    args = ap.parse_args(argv)
    workload = WORKLOAD_ALIASES.get(args.workload, args.workload)
    args.out = args.out or Path("paper/native-mul-table.tex" if workload == "u32-mod32" else f"paper/native-mul-{workload}-table.tex")
    args.label = args.label or ("tab:native-mul" + ("" if workload == "u32-mod32" else f"-{workload}"))

    by, skipped = load_rows(args.run_dirs, workload)
    if not by:
        raise SystemExit(f"no measured {workload} proof rows in {args.run_dirs}")
    exclusions = load_exclusions(args.exclusions, workload)
    known = dict(SCHEMES)
    for key in by:
        if key[0] not in known:
            raise ValueError(f"unknown table scheme {key[0]!r} at 2^{key[1]}/{key[2]} threads (union-bound opener rows are not tabulated; re-measure with --binius-ligerito-accounting rbr)")
    for entry in exclusions:
        if entry["scheme"] not in known:
            raise ValueError(f"exclusion names unknown scheme {entry['scheme']!r}")
        if any(k[0] == entry["scheme"] and k[1] == entry["log_n"] and (entry.get("threads") in (None, k[2])) for k in by):
            raise ValueError(f"{entry['scheme']} at 2^{entry['log_n']} is both measured and excluded")
    # One corpus per size across schemes, one machine across rows.
    digests = {}
    cpus = set()
    for (scheme, exponent, threads), row in by.items():
        digest = row["effective"].get("corpus_digest")
        if digests.setdefault(exponent, digest) != digest:
            raise ValueError(f"2^{exponent}: {scheme} proves a different corpus ({digest}) than the other rows ({digests[exponent]})")
        cpu = (row["provenance"].get("machine") or {}).get("cpu") or row["provenance"].get("cpu")
        cpus.add(cpu)
    if len(cpus) != 1:
        raise ValueError(f"comparison mixes machines {sorted(map(str, cpus))}")
    cpu = cpus.pop()

    exps = sorted({k[1] for k in by} | {e["log_n"] for e in exclusions})
    if args.exponents:
        if "-" in args.exponents:
            lo, hi = map(int, args.exponents.split("-"))
            exps = [e for e in exps if lo <= e <= hi]
        else:
            keep = {int(x) for x in args.exponents.split(",")}
            exps = [e for e in exps if e in keep]
    threads_list = sorted({k[2] for k in by})
    schemes = [(k, label) for k, label in SCHEMES if any(key[0] == k for key in by) or any(e["scheme"] == k for e in exclusions)]
    samples = sorted({row["metrics"]["online_prover_ms"]["count"] for row in by.values()})

    def med(row, metric):
        stats = row["metrics"].get(metric)
        return None if stats is None else stats["median"]

    def excluded(scheme, exponent, threads=None):
        return next((e for e in exclusions if e["scheme"] == scheme and e["log_n"] == exponent
                     and (e.get("threads") is None or threads is None or e["threads"] == threads)), None)

    stamp = datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%d")
    rev = probe(["git", "rev-parse", "HEAD"], "unknown")
    dirty = "+dirty" if probe(["git", "status", "--porcelain", "--untracked-files=no"], "") else ""
    revisions = sorted({str(row["provenance"].get("git_revision") or (row["provenance"].get("zinc_plus") or {}).get("revision") or "?")[:12] for row in by.values()})
    dirs = " ".join(str(d) for d in args.run_dirs)
    ram = next((row["provenance"].get("ram_bytes") or (row["provenance"].get("machine") or {}).get("physical_memory_bytes") for row in by.values() if row["provenance"].get("ram_bytes") or (row["provenance"].get("machine") or {}).get("physical_memory_bytes")), None)
    statement = {
        "u32-mod32": "Native end-to-end proofs of $N$ independent multiplications $z = xy \\bmod 2^{32}$, with $x,y,z$ unsigned $32$-bit integers: ",
        "u64": "Native end-to-end proofs of $N$ multiplications $x \\cdot y = z$ of random $64$-bit integers ($z$ a $128$-bit integer): ",
        "u128": "Native end-to-end proofs of $N$ multiplications $x \\cdot y = z$ of random $128$-bit integers ($z$ a $256$-bit integer): ",
    }[workload]

    header = [
        f"% Native end-to-end {workload} multiplication comparison — GENERATED FILE, do not edit by hand.",
        f"% Generated by scripts/mul_table.py on {stamp} (UTC) at {rev}{dirty} from {dirs}",
        f"%   (mul-bench/v2 campaigns of scripts/run_multiplication_benchmarks.py and scripts/run_zinc_plus_campaign.py;",
        "%   see docs/native-mul-compare.md). Measured-tree revisions of the rows: " + ", ".join(revisions) + ".",
        "% Regenerate (from the repo root; this file is overwritten):",
        f"%   python3 scripts/mul_table.py {dirs} --workload {workload}" + (f" --exclusions {args.exclusions}" if args.exclusions else "")
        + (f" --exponents {args.exponents}" if args.exponents else "") + f" --out {args.out} --label {args.label}",
        f"% Machine: {cpu}" + (f", {ram / 2**30:.0f} GiB installed" if ram else "") + f"; thread counts {', '.join(map(str, threads_list))}; medians of {'/'.join(map(str, samples))} samples after one warm-up.",
        "% Columns: witgen = witness_ms (native witness generation); prover = online_prover_ms (the complete prover call after",
        "%   witness generation: commitment + PIOP + PCS opening); verifier = verify_ms; proof = median proof_bytes, KB = 1000 bytes;",
        "%   peak mem. = peak_rss_bytes (BitZ and the native backends: a separate single-proof child; Zinc+: its whole worker",
        "%   process, setup and warm-up included), GB = 2^30 bytes.",
        "% Bold = best of the schemes for that size and column (each thread count is its own column).",
        "% Corpus digest per size (identical for every row of the size):",
        *[f"%   2^{e} {digests[e]}" for e in exps if e in digests],
        "% Medians as recorded (ms unless noted):",
    ]
    for e in exps:
        for t in threads_list:
            for slug, _ in schemes:
                row = by.get((slug, e, t))
                if row is not None:
                    header.append(f"%   2^{e} t={t:<2} {slug:24s} witness={med(row, 'witness_ms'):.3f} online_prover={med(row, 'online_prover_ms'):.3f} "
                                  f"verify={med(row, 'verify_ms'):.3f} proof_bytes={int(med(row, 'proof_bytes'))} "
                                  f"peak_rss={row['memory'].get('peak_rss_bytes')} samples={row['metrics']['online_prover_ms']['count']} run={Path(row['run_dir']).name}")
                elif excluded(slug, e, t):
                    ex = excluded(slug, e, t)
                    header.append(f"%   2^{e} t={t:<2} {slug:24s} EXCLUDED: {ex['reason']}" + (f" (observed peak {ex['observed_peak_rss_bytes'] / 2**30:.1f} GiB" if ex.get('observed_peak_rss_bytes') else "") + (f", machine {ex['machine_ram_bytes'] / 2**30:.0f} GiB)" if ex.get('machine_ram_bytes') else (")" if ex.get('observed_peak_rss_bytes') else "")))
    for run_dir, case, reason in skipped:
        header.append(f"%   skipped by the harness: {case['backend']} 2^{case['log_n']} t={case['threads']}: {reason} ({Path(str(run_dir)).name})")

    span = len(threads_list)
    lines = ["", "\\begin{table}[H]", "  \\centering", "  \\small", "  \\setlength{\\tabcolsep}{4pt}",
             "  \\begin{tabular}{@{}rl" + "r" * (1 + 2 * span + 2) + "@{}}", "    \\toprule",
             f"     &  & Witgen & \\multicolumn{{{span}}}{{c}}{{Prover (ms)}} & \\multicolumn{{{span}}}{{c}}{{Verifier (ms)}} & Proof & Peak mem. \\\\",
             f"    \\cmidrule(lr){{4-{3 + span}}} \\cmidrule(lr){{{4 + span}-{3 + 2 * span}}}"]
    thr = " & ".join(f"{t} thr" for t in threads_list)
    lines.append("    $N$ & Scheme & (ms) & " + thr + " & " + thr + " & (KB) & (GB) \\\\")
    lines.append("    \\midrule")
    order_t = sorted(threads_list, reverse=True)
    for gi, e in enumerate(exps):
        present = [slug for slug, _ in schemes if any((slug, e, t) in by for t in threads_list)]

        def single(slug, metric, memory=False):
            for t in order_t:
                row = by.get((slug, e, t))
                if row is not None:
                    return row["memory"].get("peak_rss_bytes") if memory else med(row, metric)
            return None

        witgen = {s: single(s, "witness_ms") for s in present}
        peak = {s: single(s, None, memory=True) for s in present}
        size = {s: single(s, "proof_bytes") for s in present}
        prov = {(s, t): med(by[(s, e, t)], "online_prover_ms") if (s, e, t) in by else None for s in present for t in threads_list}
        ver = {(s, t): med(by[(s, e, t)], "verify_ms") if (s, e, t) in by else None for s in present for t in threads_list}

        def best(values):
            vs = [v for v in values if v is not None]
            return min(vs) if len(vs) > 1 else None

        b_w, b_pk, b_sz = best(witgen.values()), best(peak.values()), best(size.values())
        b_pr = {t: best(prov[(s, t)] for s in present) for t in threads_list}
        b_vr = {t: best(ver[(s, t)] for s in present) for t in threads_list}

        def cell(v, bst, fmt):
            if v is None:
                return PLACEHOLDER
            txt = fmt(v)
            return f"\\textbf{{{txt}}}" if bst is not None and txt == fmt(bst) else txt

        first = True
        for slug, label in schemes:
            if slug not in present and not excluded(slug, e):
                continue
            n_cell = f"$2^{{{e}}}$" if first else ""
            first = False
            if slug not in present:
                lines.append(f"    {n_cell} & {label} & " + " & ".join([PLACEHOLDER] * (1 + 2 * span + 2)) + " \\\\")
                continue
            cells = [cell(witgen[slug], b_w, fmt_ms)]
            cells += [cell(prov[(slug, t)], b_pr[t], fmt_ms) if not excluded(slug, e, t) or prov[(slug, t)] is not None else PLACEHOLDER for t in threads_list]
            cells += [cell(ver[(slug, t)], b_vr[t], fmt_ms) if ver[(slug, t)] is not None else PLACEHOLDER for t in threads_list]
            cells += [cell(size[slug], b_sz, lambda v: fmt_ms(v / 1000)), cell(peak[slug], b_pk, fmt_gb)]
            lines.append(f"    {n_cell} & {label} & " + " & ".join(cells) + " \\\\")
        if gi + 1 < len(exps):
            lines.append("    \\addlinespace")
    lines += ["    \\bottomrule", "  \\end{tabular}"]

    # Caption: one clause per scheme family from the recorded configurations,
    # then the exclusions with their observed peaks.
    def config_of(slug):
        rows = [row for (s, _, _), row in by.items() if s == slug]
        return rows

    clauses = []
    if any(s.startswith("bitz@") for s, _ in schemes):
        bitz_rows = [row for (s, _, _), row in by.items() if s.startswith("bitz@")]
        profiles = sorted({(row["case"]["bitz"]["ligerito"], row["case"]["bitz"]["bound"], row["case"]["bitz"]["profile"]) for row in bitz_rows})
        clauses.append("\\ftwoz-SNARK (integer R1CS with $\\FF_2$-virtualization; Ligerito " + "; ".join(
            f"profile \\texttt{{{p}}} ({b} bound, $\\lambda = {t}$)" for p, b, t in profiles) + ")")
    binius_rows = [row for (s, _, _), row in by.items() if s.startswith("binius64@")]
    if binius_rows:
        rates = sorted({(int(row['case'].get('log_inv_rate') or 1), int((row['effective'].get('config') or {}).get('fri_queries') or 0)) for row in binius_rows})
        clauses.append("Binius (UDR) (Binius64 with native multiplication, ring switching and BaseFold at " + " and ".join(
            f"rate $1/{1 << r}$" + (f" with ${q}$ queries" if q else "") for r, q in rates) + " for $100$ bits)")
    if any(s.startswith("binius64-ligerito-rbr@") for s, _ in schemes):
        clauses.append("Binius (Johnson) (the same Binius64 circuit and PIOP, every oracle committed and opened by ring switching and "
                       "Johnson-regime Ligerito, every error term gated at $100$ bits round by round)")
    fri_rows = config_of("plonky3-fri@1")
    if fri_rows:
        cfgs = [row["effective"].get("config") or {} for row in fri_rows]
        widths = sorted({int(c.get("trace_width", 0)) for c in cfgs})
        queries = sorted({int(c.get("num_queries", 0)) for c in cfgs})
        clauses.append("Plonky3 (FRI) (Goldilocks univariate STARK, degree-$5$ extension, FRI at rate $1/2$ with the smallest query count clearing a proven "
                       f"round-by-round $100$-bit report per size: ${queries[0]}$" + (f"--${queries[-1]}$" if len(queries) > 1 else "") + " queries; "
                       + ("the wrapping u32 AIR" if workload == "u32-mod32" else "a full-product AIR over $16$-bit limbs with a carry chain")
                       + f", every value bit-decomposed, ${widths[0]}$ trace columns)")
    if config_of("limber"):
        clauses.append("Limber (one independent integer-mod R1CS row per multiplication, IntEval/Brakedown at its $100$-bit column-open target)")
    zinc_rows = config_of("zinc-plus@2")
    if zinc_rows:
        cfgs = [row["effective"].get("config") or {} for row in zinc_rows]
        logup = min(float(c.get("logup_bits", 0)) for c in cfgs)
        one = lambda k: sorted({str(c.get(k)) for c in cfgs})  # noqa: E731
        clauses.append("Zinc+ (" + {"u32-mod32": "one integer constraint $xy = z + 2^{32} w$ per multiplication over $8$",
                                  "u64": "one integer constraint $xy = z$ per multiplication over $16$",
                                  "u128": "one integer constraint $xy = z$ per multiplication over $32$"}[workload]
                       + " int columns of $16$-bit limbs, every column range-checked by a GKR-LogUp word lookup; Zip+/IPRS over $\\FF_{65537}$ at rate $1/4$ with "
                       + f"${'/'.join(one('column_openings'))}$ column openings for $100$ bits, a transcript-drawn ${'/'.join(one('prime_bits'))}$-bit projecting prime, "
                       + f"the range-check term at least ${logup:.0f}$ bits; rows of ${'/'.join(one('row_len'))}$ columns; its peak memory is that of the whole worker process)")
    exclusion_sentences = []
    for entry in sorted(exclusions, key=lambda e: (e["log_n"], e["scheme"])):
        where = f"{scheme_name(entry['scheme'])} at $2^{{{entry['log_n']}}}$" + (f" ({entry['threads']} threads)" if entry.get("threads") else "")
        detail = entry["reason"].rstrip(".")
        if entry.get("observed_peak_rss_bytes"):
            detail += f"; observed peak resident set {entry['observed_peak_rss_bytes'] / 2**30:.1f}\\,GiB"
            if entry.get("machine_ram_bytes"):
                detail += f" against {entry['machine_ram_bytes'] / 2**30:.0f}\\,GiB installed"
        exclusion_sentences.append(f"{where} is excluded: {detail}. ")
    caption = (statement + "; ".join(clauses) + ". Native security targets are reported separately; these are not a uniform complete-protocol bound. "
               "\\emph{Witgen} is the native witness generation; \\emph{prover} is the complete prover call after witness generation, commitment included; "
               "\\emph{verifier} is the complete verification; \\emph{peak mem.} is the high-water resident set ($1$\\,GB $= 2^{30}$ bytes). "
               f"\\emph{{Witgen}} and \\emph{{peak mem.}} are from the {max(threads_list)}-thread runs; proof sizes do not depend on the thread count. "
               + "".join(exclusion_sentences)
               + f"{cpu}; threads per run: {', '.join(map(str, threads_list))}; medians of {samples[0]} runs after one warm-up.")
    lines += ["  \\caption{" + caption + "}", f"  \\label{{{args.label}}}", "\\end{table}", ""]
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text("\n".join(header + lines))
    print(f"wrote {args.out} ({len(by)} rows, {len(exclusions)} exclusions)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
