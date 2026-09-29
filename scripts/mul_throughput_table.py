#!/usr/bin/env python3
"""LaTeX table of multiplication THROUGHPUT, by operand width and scheme.

Where `scripts/mul_table.py` and `scripts/mul_width_table.py` report one row
per measured size, this generator reports, for each operand width and scheme,
the best throughput the campaigns measured at any size:

    throughput = N / (the complete prover call after witness generation)

one column per thread count, together with the size `N` that achieved it, the
proof size at that size, and the largest `N` the scheme proved without
exhausting the machine's memory. Peak throughput is not in general reached at
the largest `N` — commitment work amortises as `N` grows, but the working set
eventually leaves cache and then memory — which is why the achieving size is
a column rather than a footnote.

`N_max` is the largest MEASURED size, and the note under the table names the
next size up and what the memory probe observed when it was stopped, so the
ceiling is pinned from both sides rather than inferred.

    python3 scripts/mul_throughput_table.py PerfRuns/cs-mul-* \
        --schemes bitz@3,binius64@3 --exclusions PerfRuns/cs-mul-exclusions.jsonl
"""
from __future__ import annotations

import argparse
import datetime
import json
import re
from pathlib import Path

from mul_results import aggregate, load
from mul_table import (PLACEHOLDER, SCHEMES, WORKLOAD_ALIASES, fmt_ms, probe,
                       scheme_key)

WIDTHS = [("u32-mod32", 32), ("u64", 64), ("u128", 128)]
# Row labels drop the opener tag and the rate that `mul_table.SCHEMES` carries:
# this table shows ONE configuration per scheme, so both would repeat on every
# row while the caption states them once. Two schemes that collapse to the same
# label would hide a real distinction, and `display_labels` refuses that.
TAGS = re.compile(r",\s*rate \$1/\d+\$|\s*\((?:UDR|Johnson)\)")


def display_labels(wanted):
    out = [(slug, TAGS.sub("", label).strip()) for slug, label in wanted]
    seen = {}
    for slug, label in out:
        if label in seen:
            raise SystemExit(f"{slug} and {seen[label]} would both be labelled {label!r}; "
                             "name them apart or tabulate one of them")
        seen[label] = slug
    return out
U32_NOTE = "at $w = 32$ the product is committed as its low and high $32$-bit limbs, i.e.\\ $x \\cdot y = z_0 + 2^{32} z_1$"


def load_rows(run_dirs, workloads, slugs):
    rows, skipped = {}, []
    for run_dir in run_dirs:
        for row in aggregate(load(run_dir)):
            case = row["case"]
            workload = WORKLOAD_ALIASES.get(case["workload"], case["workload"])
            if case["mode"] != "proof" or workload not in workloads:
                continue
            slug = scheme_key(case)
            if slug not in slugs:
                continue
            if row["status"] != "measured":
                skipped.append((run_dir, case, row["reason"]))
                continue
            key = (workload, slug, case["log_n"], case["threads"])
            if key in rows:
                raise ValueError(f"{key} measured in both {rows[key]['run_dir']} and {run_dir}; choose one directory per case")
            row["run_dir"] = str(run_dir)
            rows[key] = row
    return rows, skipped


def load_exclusions(path, workloads, slugs):
    if path is None:
        return []
    raw = Path(path).read_text()
    entries = json.loads(raw) if raw.lstrip().startswith("[") else [json.loads(line) for line in raw.splitlines() if line.strip()]
    out = []
    for entry in entries:
        if entry.get("status", "excluded") != "excluded":
            continue
        workload = WORKLOAD_ALIASES.get(entry["workload"], entry["workload"])
        if workload not in workloads or entry["scheme"] not in slugs:
            continue
        out.append({**entry, "workload": workload})
    return out


def fmt_rate(v):
    """Throughput in millions of multiplications per second."""
    m = v / 1e6
    return f"{m:.2f}" if m < 10 else f"{m:.1f}"


def main(argv=None) -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("run_dirs", type=Path, nargs="+", metavar="RUN_DIR")
    ap.add_argument("--schemes", required=True, help="comma list of scheme keys, in display order")
    ap.add_argument("--widths", default="u32-mod32,u64,u128")
    ap.add_argument("--exclusions", type=Path, default=None)
    ap.add_argument("--out", type=Path, default=Path("paper/mul-throughput-table.tex"))
    ap.add_argument("--label", default="tab:mul-throughput")
    args = ap.parse_args(argv)

    known = dict(SCHEMES)
    wanted = []
    for slug in args.schemes.split(","):
        if slug not in known:
            raise SystemExit(f"unknown scheme {slug!r}; known: {', '.join(known)}")
        wanted.append((slug, known[slug]))
    wanted = display_labels(wanted)
    slugs = {s for s, _ in wanted}
    widths = [(w, bits) for w, bits in WIDTHS if w in {WORKLOAD_ALIASES.get(x, x) for x in args.widths.split(",")}]
    workloads = {w for w, _ in widths}

    by, skipped = load_rows(args.run_dirs, workloads, slugs)
    if not by:
        raise SystemExit(f"no measured proof rows for {args.schemes} in {args.run_dirs}")
    exclusions = load_exclusions(args.exclusions, workloads, slugs)
    for entry in exclusions:
        if any(k[:3] == (entry["workload"], entry["scheme"], entry["log_n"]) and (entry.get("threads") in (None, k[3])) for k in by):
            raise ValueError(f"{entry['scheme']} on {entry['workload']} at 2^{entry['log_n']} is both measured and excluded")

    digests, cpus = {}, set()
    for (workload, slug, exponent, threads), row in by.items():
        digest = row["effective"].get("corpus_digest")
        if digests.setdefault((workload, exponent), digest) != digest:
            raise ValueError(f"{workload} 2^{exponent}: {slug} proves a different corpus than the other rows")
        cpus.add((row["provenance"].get("machine") or {}).get("cpu") or row["provenance"].get("cpu"))
    if len(cpus) != 1:
        raise ValueError(f"comparison mixes machines {sorted(map(str, cpus))}")
    cpu = cpus.pop()
    threads_list = sorted({k[3] for k in by})
    samples = sorted({row["metrics"]["online_prover_ms"]["count"] for row in by.values()})

    def rate(workload, slug, exponent, t):
        row = by.get((workload, slug, exponent, t))
        if row is None:
            return None
        ms = row["metrics"]["online_prover_ms"]["median"]
        return (1 << exponent) / (ms / 1000.0)

    def best_at(workload, slug, t):
        """(throughput, exponent, proof bytes) at the size that maximises throughput."""
        sizes = sorted({k[2] for k in by if k[:2] == (workload, slug) and k[3] == t})
        if not sizes:
            return None
        top = max(sizes, key=lambda e: rate(workload, slug, e, t))
        return rate(workload, slug, top, t), top, by[(workload, slug, top, t)]["metrics"]["proof_bytes"]["median"]

    def ceiling(workload, slug, t):
        """(largest measured size, smallest excluded size above it or None)."""
        sizes = sorted({k[2] for k in by if k[:2] == (workload, slug) and k[3] == t})
        if not sizes:
            return None, None
        top = sizes[-1]
        above = sorted(e["log_n"] for e in exclusions
                       if (e["workload"], e["scheme"]) == (workload, slug) and e["log_n"] > top
                       and (e.get("threads") is None or e["threads"] == t))
        return top, (above[0] if above else None)

    stamp = datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%d")
    rev = probe(["git", "rev-parse", "HEAD"], "unknown")
    dirty = "+dirty" if probe(["git", "status", "--porcelain", "--untracked-files=no"], "") else ""
    revisions = sorted({str(row["provenance"].get("git_revision") or "?")[:12] for row in by.values()})
    dirs = " ".join(str(d) for d in args.run_dirs)
    ram = next((row["provenance"].get("ram_bytes") or (row["provenance"].get("machine") or {}).get("physical_memory_bytes")
                for row in by.values() if row["provenance"].get("ram_bytes") or (row["provenance"].get("machine") or {}).get("physical_memory_bytes")), None)

    header = [
        "% Multiplication throughput by operand width — GENERATED FILE, do not edit by hand.",
        f"% Generated by scripts/mul_throughput_table.py on {stamp} (UTC) at {rev}{dirty} from {dirs}",
        "%   (mul-bench/v2 campaigns of scripts/run_multiplication_benchmarks.py; see docs/native-mul-compare.md).",
        "%   Measured-tree revisions of the rows: " + ", ".join(revisions) + ".",
        "% Regenerate (from the repo root; this file is overwritten):",
        f"%   python3 scripts/mul_throughput_table.py {dirs} --schemes {args.schemes}"
        + (f" --widths {args.widths}" if args.widths != "u32-mod32,u64,u128" else "")
        + (f" --exclusions {args.exclusions}" if args.exclusions else "") + f" --out {args.out} --label {args.label}",
        f"% Machine: {cpu}" + (f", {ram / 2**30:.0f} GiB installed" if ram else "") + f"; thread counts {', '.join(map(str, threads_list))}; medians of {'/'.join(map(str, samples))} samples after one warm-up.",
        "% throughput = N / online_prover_ms, the complete prover call after witness generation (commitment + PIOP + PCS",
        "%   opening); the size shown is the one of the measured sizes that maximises it. N_max = the largest measured size;",
        "%   the note names the next size up and what the memory probe saw. proof = median proof_bytes, KB = 1000 bytes.",
        "% Every measured size, as throughput (millions of multiplications per second):",
    ]
    for workload, _ in widths:
        for slug, _ in wanted:
            for t in threads_list:
                sizes = sorted({k[2] for k in by if k[:2] == (workload, slug) and k[3] == t})
                if not sizes:
                    continue
                cells = " ".join(f"2^{e}:{rate(workload, slug, e, t) / 1e6:.3f}" for e in sizes)
                header.append(f"%   {workload:10s} {slug:12s} t={t:<2} {cells}")
    for entry in sorted(exclusions, key=lambda e: (e["workload"], e["scheme"], e["log_n"], e.get("threads") or 0)):
        header.append(f"%   {entry['workload']:10s} {entry['scheme']:12s} t={entry.get('threads', '?'):<2} 2^{entry['log_n']} EXCLUDED: {entry['reason']}"
                      + (f" (observed peak {entry['observed_peak_rss_bytes'] / 2**30:.1f} GiB)" if entry.get("observed_peak_rss_bytes") else ""))
    for run_dir, case, reason in skipped:
        header.append(f"%   skipped by the harness: {case['backend']} {case['workload']} 2^{case['log_n']} t={case['threads']}: {reason} ({Path(str(run_dir)).name})")

    span = len(threads_list)
    lines = ["", "\\begin{table}[H]", "  \\centering", "  \\small", "  \\setlength{\\tabcolsep}{4pt}",
             "  \\begin{tabular}{@{}rl" + "rrr" * span + "r@{}}", "    \\toprule",
             "     &  & " + " & ".join(f"\\multicolumn{{3}}{{c}}{{{t} thread{'s' if t != 1 else ''}}}" for t in threads_list) + " &  \\\\",
             "    " + " ".join(f"\\cmidrule(lr){{{3 + 3 * i}-{5 + 3 * i}}}" for i in range(span)),
             "    $w$ & Scheme & " + " & ".join(["Mult/s & at $N$ & Proof"] * span) + " & $N_{\\max}$ \\\\",
             "     &  & " + " & ".join(["($\\times 10^6$) &  & (KB)"] * span) + " &  \\\\",
             "    \\midrule"]
    for gi, (workload, bits) in enumerate(widths):
        rows_data = {}
        for slug, _ in wanted:
            per_t = {t: best_at(workload, slug, t) for t in threads_list}
            tops = {t: ceiling(workload, slug, t) for t in threads_list}
            rows_data[slug] = (per_t, tops)

        def bold(value, candidates, fmt, better=max):
            if value is None:
                return PLACEHOLDER
            txt = fmt(value)
            vs = [c for c in candidates if c is not None]
            return f"\\textbf{{{txt}}}" if len(vs) > 1 and txt == fmt(better(vs)) else txt

        first = True
        for slug, label in wanted:
            per_t, tops = rows_data[slug]
            if all(v is None for v in per_t.values()):
                continue
            lead = f"${bits}$" if first else ""
            first = False
            cells = []
            for t in threads_list:
                got = per_t[t]
                others = [rows_data[s][0][t] for s, _ in wanted]
                if got is None:
                    cells += [PLACEHOLDER] * 3
                    continue
                throughput, exponent, proof = got
                cells.append(bold(throughput, [o[0] for o in others if o], fmt_rate))
                cells.append(f"$2^{{{exponent}}}$")
                cells.append(bold(proof, [o[2] for o in others if o], lambda v: fmt_ms(v / 1000), better=min))
            measured_tops = [tops[t][0] for t in threads_list if tops[t][0] is not None]
            all_tops = [max([rows_data[s][1][t][0] for t in threads_list if rows_data[s][1][t][0] is not None] or [None])
                        for s, _ in wanted]
            rivals = [a for a in all_tops if a is not None]
            if not measured_tops:
                cells.append(PLACEHOLDER)
            elif len(set(measured_tops)) > 1:  # the ceiling is thread-count dependent: show each
                cells.append("/".join(f"$2^{{{tops[t][0]}}}$" for t in threads_list))
            else:
                top = measured_tops[0]
                best = len(rivals) > 1 and top == max(rivals)
                cells.append(f"$\\mathbf{{2^{{{top}}}}}$" if best else f"$2^{{{top}}}$")
            lines.append(f"    {lead} & {label} & " + " & ".join(cells) + " \\\\")
        if gi + 1 < len(widths):
            lines.append("    \\addlinespace")
    lines += ["    \\bottomrule", "  \\end{tabular}"]

    # Caption: the scheme clauses, then how each ceiling was pinned.
    clauses = []
    if any(s.startswith("bitz@") for s in slugs):
        bitz_rows = [row for k, row in by.items() if k[1].startswith("bitz@")]
        profiles = sorted({(row["case"]["bitz"]["ligerito"], row["case"]["bitz"]["bound"], row["case"]["bitz"]["profile"]) for row in bitz_rows})
        clauses.append("\\ftwoz-SNARK (integer R1CS with $\\FF_2$-virtualization; Ligerito "
                       + ", ".join(f"\\texttt{{{p}}}" for p, _, _ in profiles) + ", "
                       + "/".join(sorted({b for _, b, _ in profiles})) + " bound, $\\lambda = "
                       + "/".join(sorted({str(t) for _, _, t in profiles})) + "$)")
    binius_rows = [row for k, row in by.items() if k[1].startswith("binius64@")]
    if binius_rows:
        rates = sorted({(int(row['case'].get('log_inv_rate') or 1), int((row['effective'].get('config') or {}).get('fri_queries') or 0)) for row in binius_rows})
        clauses.append("Binius (Binius64, ring switching and BaseFold at " + " and ".join(
            f"rate $1/{1 << r}$" + (f" (${q}$ queries)" if q else "") for r, q in rates) + " for $100$ bits)")

    ceilings = []
    for workload, bits in widths:
        for slug, label in wanted:
            top, above = max(((ceiling(workload, slug, t)) for t in threads_list), key=lambda p: (p[0] is not None, p[0] or 0))
            if top is None:
                continue
            matching = [e for e in exclusions if (e["workload"], e["scheme"]) == (workload, slug) and e["log_n"] == above]
            if not matching:
                ceilings.append(f"{label} at $w = {bits}$ was not run above $2^{{{top}}}$")
                continue
            entry = max(matching, key=lambda e: e.get("observed_peak_rss_bytes") or 0)
            detail = (f"killed at {entry['observed_peak_rss_bytes'] / 2**30:.1f}\\,GiB resident with "
                      f"{entry['peak_compressed_bytes'] / 2**30:.1f}\\,GiB of its own pages compressed out"
                      if entry.get("observed_peak_rss_bytes") and entry.get("peak_compressed_bytes") is not None
                      else entry["reason"].rstrip("."))
            counts = sorted({e["threads"] for e in matching if e.get("threads")})
            if len(counts) == len(threads_list) > 1:  # every thread count was stopped; the worst peak is shown
                qualifier = "\x00"  # universal: hoisted into the lead sentence below if every entry has it
            elif len(counts) > 1 or counts:
                qualifier = f" at {entry['threads']} thread" + ("s" if entry["threads"] != 1 else "")
            else:
                qualifier = ""
            ceilings.append(f"{label} at $w = {bits}$: $2^{{{above}}}$ {detail}{qualifier}")

    caption = ("Throughput of native end-to-end proofs of $N$ independent multiplications $x \\cdot y = z$ of random $w$-bit integers "
               + "($z$ a $2w$-bit integer; " + U32_NOTE + "): " + "; ".join(clauses) + ". "
               "\\emph{Mult/s} is $N$ divided by the complete prover call after witness generation (commitment, PIOP and PCS opening included), "
               "at the one of the measured sizes that maximises it --- given in the \\emph{at $N$} column, since commitment work amortises as $N$ grows "
               "while the working set eventually leaves cache. The curve is flat near its maximum, so that size marks a plateau rather than a sharp optimum. "
               "\\emph{Proof} is the proof at that same size. "
               "$N_{\\max}$ is the largest size the scheme proved without exhausting the machine's memory; the note below the table gives, for each, "
               "what the memory probe observed one size above it. Native security targets are reported separately; these are not a uniform "
               f"complete-protocol bound. {cpu}" + (f", {ram / 2**30:.0f}\\,GiB installed" if ram else "")
               + f"; medians of {samples[0]} runs after one warm-up.")
    lines += ["  \\caption{" + caption + "}", f"  \\label{{{args.label}}}"]
    if ceilings:
        universal = all("\x00" in c for c in ceilings)
        ceilings = [c.replace("\x00", "" if universal else " (at every thread count; the highest peak shown)") for c in ceilings]
        lead = ("One size above $N_{\\max}$, each stopped by the memory probe"
                + (" at every thread count" if universal else "") + " when its own working set no longer fit ("
                + ("the highest peak shown; " if universal else "") + f"{ram / 2**30:.0f}" + "\\,GiB installed): ")
        lines += ["  \\par\\smallskip\\noindent{\\footnotesize " + lead + "; ".join(ceilings) + ".}"]
    lines += ["\\end{table}", ""]
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text("\n".join(header + lines))
    print(f"wrote {args.out} ({len(by)} measured cells, {len(exclusions)} exclusions)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
