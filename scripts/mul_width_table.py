#!/usr/bin/env python3
"""LaTeX table of the multiplication campaigns, grouped by operand width.

The sibling of `scripts/mul_table.py`, which fixes the operand width and lets
`N` vary down the table. This generator has the width as an axis, in one of
two shapes:

  * one size, several schemes (`--log-n 19 --schemes bitz@3,binius64@3,...`):
    one row group per width, one row per scheme, so a single table answers
    "at this size, what does each scheme cost for 32-, 64- and 128-bit
    multiplications?";
  * several sizes, ONE scheme (`--log-n 19,21,22,23 --schemes bitz@3`):
    one row group per size, one row per width -- how a single scheme scales
    in both `N` and the operand width. Nothing is comparable across the rows
    of a group there, so nothing is bolded.

Columns, provenance, exclusions and the caption machinery are the ones of
`scripts/mul_table.py`, and the scheme keys are the same
(`backend@log_inv_rate` where the rate is a parameter).

    python3 scripts/mul_width_table.py PerfRuns/cs-mul-* --log-n 19 \
        --schemes bitz@3,binius64@3,limber,zinc-plus@2 \
        --exclusions PerfRuns/cs-mul-exclusions.jsonl --out paper/mul-2p19-table.tex
"""
from __future__ import annotations

import argparse
import datetime
import json
from pathlib import Path

from mul_results import aggregate, load
from mul_table import (PLACEHOLDER, SCHEMES, WORKLOAD_ALIASES, fmt_gb, fmt_ms,
                       probe, scheme_key, scheme_name)

# Display order and labels of the operand widths.
WIDTHS = [("u32-mod32", 32), ("u64", 64), ("u128", 128)]
LIMBS = {"u32-mod32": 8, "u64": 16, "u128": 32}
U32_NOTE = "at $w = 32$ the product is committed as its low and high $32$-bit limbs, i.e.\\ $x \\cdot y = z_0 + 2^{32} z_1$"


def load_rows(run_dirs, workloads, sizes):
    """Measured proof-mode rows, keyed by (workload, scheme, log_n, threads)."""
    rows, skipped = {}, []
    for run_dir in run_dirs:
        for row in aggregate(load(run_dir)):
            case = row["case"]
            workload = WORKLOAD_ALIASES.get(case["workload"], case["workload"])
            if case["mode"] != "proof" or workload not in workloads or case["log_n"] not in sizes:
                continue
            if row["status"] != "measured":
                skipped.append((run_dir, case, row["reason"]))
                continue
            key = (workload, scheme_key(case), case["log_n"], case["threads"])
            if key in rows:
                raise ValueError(f"{key} measured in both {rows[key]['run_dir']} and {run_dir}; choose one directory per case")
            row["run_dir"] = str(run_dir)
            rows[key] = row
    return rows, skipped


def load_exclusions(path, workloads, sizes):
    if path is None:
        return []
    raw = Path(path).read_text()
    entries = json.loads(raw) if raw.lstrip().startswith("[") else [json.loads(line) for line in raw.splitlines() if line.strip()]
    out = []
    for entry in entries:
        if entry.get("status", "excluded") != "excluded":
            continue
        for field in ("scheme", "log_n", "reason", "workload"):
            if field not in entry:
                raise ValueError(f"exclusion lacks {field}: {entry}")
        workload = WORKLOAD_ALIASES.get(entry["workload"], entry["workload"])
        if workload not in workloads or entry["log_n"] not in sizes:
            continue
        out.append({**entry, "workload": workload})
    return out


def main(argv=None) -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("run_dirs", type=Path, nargs="+", metavar="RUN_DIR")
    ap.add_argument("--log-n", required=True, help="the size, or a comma list of sizes (then --schemes must name exactly one scheme)")
    ap.add_argument("--schemes", required=True, help="comma list of scheme keys, in display order (e.g. bitz@3,binius64@3)")
    ap.add_argument("--widths", default="u32-mod32,u64,u128", help="comma list of workloads, in display order")
    ap.add_argument("--exclusions", type=Path, default=None, help="excluded cells with reasons and observed peaks: a JSON list or the probe's JSONL")
    ap.add_argument("--unmeasured", default="", help="comma list of workload:scheme[:log_n] cells that were never run; they print as -- and are named in the note")
    ap.add_argument("--unmeasured-reason", default="was not run", help="how the note explains an --unmeasured cell (the exclusions file can only speak for cells a probe killed)")
    ap.add_argument("--out", type=Path, default=None)
    ap.add_argument("--label", default=None)
    args = ap.parse_args(argv)

    sizes = [int(x) for x in args.log_n.split(",")]
    if len(set(sizes)) != len(sizes):
        raise SystemExit("--log-n repeats a size")
    known = dict(SCHEMES)
    wanted = []
    for slug in args.schemes.split(","):
        if slug not in known:
            raise SystemExit(f"unknown scheme {slug!r}; known: {', '.join(known)}")
        wanted.append((slug, known[slug]))
    multi = len(sizes) > 1
    if multi and len(wanted) != 1:
        raise SystemExit("several sizes tabulate one scheme; pass a single --schemes key")
    widths = [(w, bits) for w, bits in WIDTHS if w in {WORKLOAD_ALIASES.get(x, x) for x in args.widths.split(",")}]
    workloads = {w for w, _ in widths}
    slug_of = {slug for slug, _ in wanted}
    stem = "-".join(f"2p{e}" for e in sizes)
    args.out = args.out or Path(f"paper/mul-{stem}-table.tex")
    args.label = args.label or f"tab:mul-{stem}"

    unmeasured = set()
    for cell in filter(None, args.unmeasured.split(",")):
        parts = cell.split(":")
        if len(parts) == 2 and not multi:
            parts.append(str(sizes[0]))
        if len(parts) != 3:
            raise SystemExit(f"--unmeasured wants workload:scheme:log_n, got {cell!r}")
        workload, slug, exponent = WORKLOAD_ALIASES.get(parts[0], parts[0]), parts[1], int(parts[2])
        if workload not in workloads or slug not in slug_of or exponent not in sizes:
            raise SystemExit(f"--unmeasured names an unknown cell {cell!r}")
        unmeasured.add((workload, slug, exponent))

    by, skipped = load_rows(args.run_dirs, workloads, set(sizes))
    by = {k: row for k, row in by.items() if k[1] in slug_of}
    if not by:
        raise SystemExit(f"no measured proof rows for {args.schemes} at 2^{args.log_n} in {args.run_dirs}")
    exclusions = [e for e in load_exclusions(args.exclusions, workloads, set(sizes)) if e["scheme"] in slug_of]
    for entry in exclusions:
        if any(k[:3] == (entry["workload"], entry["scheme"], entry["log_n"]) and (entry.get("threads") in (None, k[3])) for k in by):
            raise ValueError(f"{entry['scheme']} on {entry['workload']} at 2^{entry['log_n']} is both measured and excluded")
    for cell in unmeasured:
        if any(k[:3] == cell for k in by) or any((e["workload"], e["scheme"], e["log_n"]) == cell for e in exclusions):
            raise ValueError(f"--unmeasured names {cell}, which the campaigns do measure or exclude")

    # One corpus per (width, size), one machine across rows.
    digests, cpus = {}, set()
    for (workload, slug, exponent, threads), row in by.items():
        digest = row["effective"].get("corpus_digest")
        if digests.setdefault((workload, exponent), digest) != digest:
            raise ValueError(f"{workload} 2^{exponent}: {slug} proves a different corpus ({digest}) than the other rows")
        cpus.add((row["provenance"].get("machine") or {}).get("cpu") or row["provenance"].get("cpu"))
    if len(cpus) != 1:
        raise ValueError(f"comparison mixes machines {sorted(map(str, cpus))}")
    cpu = cpus.pop()

    threads_list = sorted({k[3] for k in by})
    samples = sorted({row["metrics"]["online_prover_ms"]["count"] for row in by.values()})

    # The two shapes, as (group label, [(row label, (workload, scheme, log_n))]).
    if multi:
        groups = [(f"$2^{{{e}}}$", [(f"${bits}$", (w, wanted[0][0], e)) for w, bits in widths]) for e in sizes]
        columns = ("$N$", "$w$")
    else:
        e = sizes[0]
        groups = [(f"${bits}$", [(label, (w, slug, e)) for slug, label in wanted]) for w, bits in widths]
        columns = ("$w$", "Scheme")

    def med(row, metric):
        stats = row["metrics"].get(metric)
        return None if stats is None else stats["median"]

    def excluded(cell, threads=None):
        return next((e for e in exclusions if (e["workload"], e["scheme"], e["log_n"]) == cell
                     and (e.get("threads") is None or threads is None or e["threads"] == threads)), None)

    def where(cell):
        workload, slug, exponent = cell
        if multi:  # the scheme is fixed and named in the caption; the size is not
            return f"$w = {dict(widths)[workload]}$ at $2^{{{exponent}}}$"
        return f"{scheme_name(slug)} at $w = {dict(widths)[workload]}$"

    stamp = datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%d")
    rev = probe(["git", "rev-parse", "HEAD"], "unknown")
    dirty = "+dirty" if probe(["git", "status", "--porcelain", "--untracked-files=no"], "") else ""
    revisions = sorted({str(row["provenance"].get("git_revision") or (row["provenance"].get("zinc_plus") or {}).get("revision") or "?")[:12] for row in by.values()})
    dirs = " ".join(str(d) for d in args.run_dirs)
    ram = next((row["provenance"].get("ram_bytes") or (row["provenance"].get("machine") or {}).get("physical_memory_bytes")
                for row in by.values() if row["provenance"].get("ram_bytes") or (row["provenance"].get("machine") or {}).get("physical_memory_bytes")), None)

    header = [
        f"% Native end-to-end multiplication comparison by operand width — GENERATED FILE, do not edit by hand.",
        f"% Generated by scripts/mul_width_table.py on {stamp} (UTC) at {rev}{dirty} from {dirs}",
        "%   (mul-bench/v2 campaigns of scripts/run_multiplication_benchmarks.py and scripts/run_zinc_plus_campaign.py;",
        "%   see docs/native-mul-compare.md). Measured-tree revisions of the rows: " + ", ".join(revisions) + ".",
        "% Regenerate (from the repo root; this file is overwritten):",
        f"%   python3 scripts/mul_width_table.py {dirs} --log-n {args.log_n} --schemes {args.schemes}"
        + (f" --widths {args.widths}" if args.widths != "u32-mod32,u64,u128" else "")
        + (f" --exclusions {args.exclusions}" if args.exclusions else "")
        + (f" --unmeasured {args.unmeasured}" if args.unmeasured else "")
        + (f" --unmeasured-reason {args.unmeasured_reason!r}" if args.unmeasured else "")
        + f" --out {args.out} --label {args.label}",
        f"% Machine: {cpu}" + (f", {ram / 2**30:.0f} GiB installed" if ram else "") + f"; thread counts {', '.join(map(str, threads_list))}; medians of {'/'.join(map(str, samples))} samples after one warm-up.",
        "% Columns: witgen = witness_ms (native witness generation); prover = online_prover_ms (the complete prover call after",
        "%   witness generation: commitment + PIOP + PCS opening); verifier = verify_ms; proof = median proof_bytes, KB = 1000 bytes;",
        "%   peak mem. = peak_rss_bytes (a separate single-proof child), GB = 2^30 bytes.",
    ]
    header.append("% Nothing is bolded: no two rows of a group prove the same statement." if multi
                  else "% Bold = best of the schemes for that width and column (each thread count is its own column).")
    header += ["% Corpus digest per width and size (identical for every row of the cell):",
               *[f"%   {w:10s} 2^{e} {digests[(w, e)]}" for (w, e) in sorted(digests, key=lambda k: ([x for x, _ in widths].index(k[0]), k[1]))],
               "% Medians as recorded (ms unless noted):"]
    for _, rows in groups:
        for t in threads_list:
            for label, cell in rows:
                row = by.get((*cell, t))
                name = f"{cell[0]:10s} 2^{cell[2]:<2} {cell[1]:10s}"
                if row is not None:
                    header.append(f"%   {name} t={t:<2} witness={med(row, 'witness_ms'):.3f} online_prover={med(row, 'online_prover_ms'):.3f} "
                                  f"verify={med(row, 'verify_ms'):.3f} proof_bytes={int(med(row, 'proof_bytes'))} "
                                  f"peak_rss={row['memory'].get('peak_rss_bytes')} samples={row['metrics']['online_prover_ms']['count']} run={Path(row['run_dir']).name}")
                elif (ex := excluded(cell, t)):
                    header.append(f"%   {name} t={t:<2} EXCLUDED: {ex['reason']}"
                                  + (f" (observed peak {ex['observed_peak_rss_bytes'] / 2**30:.1f} GiB" if ex.get('observed_peak_rss_bytes') else "")
                                  + (f", machine {ex['machine_ram_bytes'] / 2**30:.0f} GiB)" if ex.get('machine_ram_bytes') else (")" if ex.get('observed_peak_rss_bytes') else "")))
                elif cell in unmeasured:
                    header.append(f"%   {name} t={t:<2} NOT RUN")
    for run_dir, case, reason in skipped:
        header.append(f"%   skipped by the harness: {case['backend']} {case['workload']} 2^{case['log_n']} t={case['threads']}: {reason} ({Path(str(run_dir)).name})")

    span = len(threads_list)
    lines = ["", "\\begin{table}[H]", "  \\centering", "  \\small", "  \\setlength{\\tabcolsep}{4pt}",
             "  \\begin{tabular}{@{}rl" + "r" * (1 + 2 * span + 2) + "@{}}", "    \\toprule",
             f"     &  & Witgen & \\multicolumn{{{span}}}{{c}}{{Prover (ms)}} & \\multicolumn{{{span}}}{{c}}{{Verifier (ms)}} & Proof & Peak mem. \\\\",
             f"    \\cmidrule(lr){{4-{3 + span}}} \\cmidrule(lr){{{4 + span}-{3 + 2 * span}}}"]
    thr = " & ".join(f"{t} thr" for t in threads_list)
    lines.append(f"    {columns[0]} & {columns[1]} & (ms) & " + thr + " & " + thr + " & (KB) & (GB) \\\\")
    lines.append("    \\midrule")
    order_t = sorted(threads_list, reverse=True)
    for gi, (group_label, rows) in enumerate(groups):
        present = [cell for _, cell in rows if any((*cell, t) in by for t in threads_list)]

        def single(cell, metric, memory=False):
            for t in order_t:
                row = by.get((*cell, t))
                if row is not None:
                    return row["memory"].get("peak_rss_bytes") if memory else med(row, metric)
            return None

        witgen = {c: single(c, "witness_ms") for c in present}
        peak = {c: single(c, None, memory=True) for c in present}
        size = {c: single(c, "proof_bytes") for c in present}
        prov = {(c, t): med(by[(*c, t)], "online_prover_ms") if (*c, t) in by else None for c in present for t in threads_list}
        ver = {(c, t): med(by[(*c, t)], "verify_ms") if (*c, t) in by else None for c in present for t in threads_list}

        def best(values):
            if multi:  # rows of a group prove different statements
                return None
            vs = [v for v in values if v is not None]
            return min(vs) if len(vs) > 1 else None

        b_w, b_pk, b_sz = best(witgen.values()), best(peak.values()), best(size.values())
        b_pr = {t: best(prov[(c, t)] for c in present) for t in threads_list}
        b_vr = {t: best(ver[(c, t)] for c in present) for t in threads_list}

        def cell_text(v, bst, fmt):
            if v is None:
                return PLACEHOLDER
            txt = fmt(v)
            return f"\\textbf{{{txt}}}" if bst is not None and txt == fmt(bst) else txt

        first = True
        for label, cell in rows:
            if cell not in present and not excluded(cell) and cell not in unmeasured:
                continue
            lead = group_label if first else ""
            first = False
            if cell not in present:
                lines.append(f"    {lead} & {label} & " + " & ".join([PLACEHOLDER] * (1 + 2 * span + 2)) + " \\\\")
                continue
            cells = [cell_text(witgen[cell], b_w, fmt_ms)]
            cells += [cell_text(prov[(cell, t)], b_pr[t], fmt_ms) for t in threads_list]
            cells += [cell_text(ver[(cell, t)], b_vr[t], fmt_ms) for t in threads_list]
            cells += [cell_text(size[cell], b_sz, lambda v: fmt_ms(v / 1000)), cell_text(peak[cell], b_pk, fmt_gb)]
            lines.append(f"    {lead} & {label} & " + " & ".join(cells) + " \\\\")
        if gi + 1 < len(groups):
            lines.append("    \\addlinespace")
    lines += ["    \\bottomrule", "  \\end{tabular}"]

    # Caption: one clause per scheme family, read from the recorded configurations.
    clauses = []
    if any(s.startswith("bitz@") for s in slug_of):
        bitz_rows = [row for k, row in by.items() if k[1].startswith("bitz@")]
        profiles = sorted({(row["case"]["bitz"]["ligerito"], row["case"]["bitz"]["bound"], row["case"]["bitz"]["profile"]) for row in bitz_rows})
        clauses.append("\\ftwoz-SNARK (integer R1CS with $\\FF_2$-virtualization; Ligerito "
                       + ", ".join(f"\\texttt{{{p}}}" for p, _, _ in profiles) + ", "
                       + "/".join(sorted({b for _, b, _ in profiles})) + " bound, $\\lambda = "
                       + "/".join(sorted({str(t) for _, _, t in profiles})) + "$)")
    binius_rows = [row for k, row in by.items() if k[1].startswith("binius64@")]
    if binius_rows:
        rates = sorted({(int(row['case'].get('log_inv_rate') or 1), int((row['effective'].get('config') or {}).get('fri_queries') or 0)) for row in binius_rows})
        clauses.append("Binius (UDR) (Binius64, ring switching and BaseFold at " + " and ".join(
            f"rate $1/{1 << r}$" + (f" (${q}$ queries)" if q else "") for r, q in rates) + " for $100$ bits)")
    if any(s.startswith("binius64-ligerito-rbr@") for s in slug_of):
        clauses.append("Binius (Johnson) (the same circuit and PIOP, every oracle opened by Johnson-regime Ligerito, $100$ bits round by round)")
    if any(k[1] == "limber" for k in by):
        clauses.append("Limber (one integer-mod R1CS row per multiplication, IntEval/Brakedown, $100$-bit column-open target)")
    zinc_rows = [row for k, row in by.items() if k[1].startswith("zinc-plus@")]
    if zinc_rows:
        cfgs = [row["effective"].get("config") or {} for row in zinc_rows]
        logup = min(float(c.get("logup_bits", 0)) for c in cfgs)
        shown = [(w, bits) for w, bits in widths if any(k[0] == w and k[1].startswith("zinc-plus@") for k in by)]
        one = lambda k: sorted({str(c.get(k)) for c in cfgs})  # noqa: E731
        clauses.append("Zinc+ (one integer constraint per multiplication"
                       + (" ($x y = z_0 + 2^{32} z_1$ at $w = 32$, $x y = z$ otherwise)" if any(w == "u32-mod32" for w, _ in shown) else "")
                       + f" over ${'/'.join(str(LIMBS[w]) for w, _ in shown)}$ int columns of $16$-bit limbs for "
                       + f"$w = {'/'.join(str(bits) for _, bits in shown)}$, with GKR-LogUp range checks; "
                       "Zip+/IPRS over $\\FF_{65537}$ at rate $1/4$, "
                       + f"${'/'.join(one('column_openings'))}$ openings, a ${'/'.join(one('prime_bits'))}$-bit projecting prime, "
                       + f"range-check term $\\geq {logup:.0f}$ bits; peak memory is the whole worker's)")

    notes = []
    for entry in sorted(exclusions, key=lambda e: (e["log_n"], [w for w, _ in widths].index(e["workload"]), e["scheme"], e.get("threads") or 0)):
        text = where((entry["workload"], entry["scheme"], entry["log_n"]))
        if entry.get("threads"):
            text += f", {entry['threads']} thread" + ("s" if entry["threads"] != 1 else "")
        if entry.get("observed_peak_rss_bytes") and entry.get("peak_compressed_bytes") is not None:
            detail = (f"killed at {entry['observed_peak_rss_bytes'] / 2**30:.1f}\\,GiB resident with "
                      f"{entry['peak_compressed_bytes'] / 2**30:.1f}\\,GiB of its own pages compressed out")
        else:
            detail = entry["reason"].rstrip(".")
            if entry.get("observed_peak_rss_bytes"):
                detail += f"; observed peak resident set {entry['observed_peak_rss_bytes'] / 2**30:.1f}\\,GiB"
        if entry.get("machine_ram_bytes"):
            detail += f" ({entry['machine_ram_bytes'] / 2**30:.0f}\\,GiB installed)"
        notes.append(f"{text}: {detail}")

    size_text = "$N$" if multi else f"$N = 2^{{{sizes[0]}}}$"
    statement = (f"Native end-to-end proofs of {size_text} independent multiplications $x \\cdot y = z$ of random "
                 + "$w$-bit integers, for $w \\in \\{" + ", ".join(str(b) for _, b in widths) + "\\}$ ($z$ a $2w$-bit integer"
                 + ("; " + U32_NOTE if any(w == "u32-mod32" for w, _ in widths) else "") + "): ")
    caption = (statement + "; ".join(clauses) + ". Native security targets are reported separately; these are not a uniform complete-protocol bound. "
               "\\emph{Witgen} is the native witness generation; \\emph{prover} is the complete prover call after witness generation, commitment included; "
               "\\emph{verifier} is the complete verification; \\emph{peak mem.} is the high-water resident set ($1$\\,GB $= 2^{30}$ bytes). "
               f"\\emph{{Witgen}} and \\emph{{peak mem.}} are from the {max(threads_list)}-thread runs; proof sizes do not depend on the thread count. "
               + ("Cells marked " + PLACEHOLDER + " did not fit the machine's memory; the note below the table gives each one. " if notes or unmeasured else "")
               + f"{cpu}; threads per run: {', '.join(map(str, threads_list))}; medians of {samples[0]} runs after one warm-up.")
    lines += ["  \\caption{" + caption + "}", f"  \\label{{{args.label}}}"]
    if notes or unmeasured:
        text = ""
        if notes:
            text += ("Excluded cells (" + PLACEHOLDER + "), each stopped by the memory probe when its own working set no longer fit: "
                     + "; ".join(notes) + ".")
        for cell in sorted(unmeasured, key=lambda c: (c[2], [w for w, _ in widths].index(c[0]))):
            text += (" " if text else "") + f"{where(cell)} {args.unmeasured_reason.rstrip('.')}."
        lines += ["  \\par\\smallskip\\noindent{\\footnotesize " + text + "}"]
    lines += ["\\end{table}", ""]
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text("\n".join(header + lines))
    print(f"wrote {args.out} ({len(by)} rows, {len(exclusions)} exclusions, {len(unmeasured)} not run)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
