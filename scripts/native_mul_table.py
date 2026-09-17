#!/usr/bin/env python3
"""LaTeX table of absolute numbers from a native-mul run (benches/mul_e2e_compare.rs).

Reads the `summary.json` of one or more `PerfRuns/<stamp>-native-mul` run
directories (later directories override earlier ones for the same scheme, size
and thread count, so a follow-up run of the large sizes extends a first run)
and writes a self-documenting `outputs/tables/native-mul-table.tex` in the SHA+ECDSA
table's format: one row group per size N, sub-grouped by thread count (1 then
10), one row per scheme, with the medians of witness generation, the complete
prover (commit included) and the complete verifier, plus complete proof size
and isolated peak memory. Proof sizes must agree between the thread counts of
one (scheme, size) — the proof does not depend on the thread count.

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

# Table rows are (scheme key, LaTeX label), the 2026-09-13 suite: BitZ and
# Binius64 once per rate (scheme keys `bitz@<log_inv_rate>` and
# `binius64@<log_inv_rate>`), the Binius64/BitZ-opener rows under the
# ROUND-BY-ROUND accounting only (`binius64-ligerito-rbr@<log_inv_rate>`;
# union-bound runs are rejected, re-measure with
# BITZ_BINIUS_LIGERITO_ACCOUNTING=rbr), Plonky3-FRI at rate 1/2 and Limber at
# the pinned 100-bit Brakedown target. Naming per the user's directive: no
# \cite after a scheme name, and the BitZ rows are \ftwoz-SNARK.
SCHEMES = [
    ("bitz@1", "\\ftwoz-SNARK, rate $1/2$"),
    ("bitz@3", "\\ftwoz-SNARK, rate $1/8$"),
    ("binius64@1", "Binius (UDR), rate $1/2$"),
    ("binius64@3", "Binius (UDR), rate $1/8$"),
    ("binius64-ligerito-rbr@1", "Binius (Johnson), rate $1/2$"),
    ("binius64-ligerito-rbr@3", "Binius (Johnson), rate $1/8$"),
    ("plonky3-fri", "Plonky3 (FRI), rate $1/2$"),
    ("plonky3-whir", "Plonky3 (WHIR)"),
    ("limber", "Limber (Brakedown)"),
]
BINIUS_QUERIES = {1: 241, 2: 148, 3: 121, 4: 110}  # 100-bit FRI query counts per log inverse rate


def bitz_rate(r: dict) -> int:
    """Level-0 inverse-rate exponent of the BitZ opener this row recorded."""
    levels = ((r.get("config", {}).get("ligerito") or {}).get("configuration") or {}).get("levels") or []
    if not levels:
        raise ValueError("BitZ row records no Ligerito configuration")
    return int(levels[0]["log_inv_rate"])


def bitz_caption(rows):
    """Describe every recorded BitZ Ligerito policy, one clause per rate."""
    from ligerito_results import validate_ligerito
    policies = {}
    for row in rows:
        if row["backend"] != "bitz":
            continue
        report = validate_ligerito(row.get("config", {}).get("ligerito"), 100)
        key = bitz_rate(row)
        policy = (report["resolved_profile"], report["regime"], report["outer_ood"])
        if policies.setdefault(key, policy) != policy:
            raise ValueError("cannot combine different Ligerito policies at one BitZ rate")
    if not policies:
        return r"\ftwoz-SNARK (integer R1CS)"
    clauses = []
    for rate, (profile, regime, ood) in sorted(policies.items()):
        bound = "Johnson" if regime == "johnson" else "unique decoding radius"
        evaluation = "early Round-0 OOD" if ood else "without OOD"
        clauses.append(rf"at rate $1/{1 << rate}$ is {bound}, {profile}, {evaluation}")
    shifts = {int(row["config"].get("u64_split_shift", 0)) for row in rows if row["backend"] == "bitz"}
    shift_note = ""
    if shifts - {0}:
        if len(shifts) != 1:
            raise ValueError("cannot combine different u64 split shifts in one BitZ table series")
        k = shifts.pop()
        shift_note = (f"; the BitZ row side is lowered by ${k}$ variable{'s' if k != 1 else ''} below the "
                      f"default split at every size (one more column variable each, so the read-off is "
                      f"${1 << k}\\times$ longer)")
    return r"\ftwoz-SNARK (integer R1CS, Lambda100; Ligerito " + "; ".join(clauses) + shift_note + ")"


def scheme_key(r: dict) -> str:
    """Row key: the backend slug, with BitZ and Binius64 split by their rate."""
    if r["backend"] == "binius64":
        return f"binius64@{int(r['config'].get('log_inv_rate', 1))}"
    if r["backend"] == "binius64-ligerito":
        rbr = r["config"].get("accounting", "union-bound") == "round-by-round"
        return f"binius64-ligerito{'-rbr' if rbr else ''}@{int(r['config'].get('log_inv_rate', 1))}"
    if r["backend"] == "bitz":
        return f"bitz@{bitz_rate(r)}"
    return r["backend"]


def scheme_name(key: str) -> str:
    """Short scheme name for caption sentences."""
    names = {"plonky3-fri": "Plonky3-FRI", "plonky3-whir": "Plonky3-WHIR", "limber": "Limber"}
    if key in names:
        return names[key]
    family, _, rate = key.partition("@")
    prefix = {"bitz": "\\ftwoz-SNARK at ", "binius64": "Binius (UDR) at ",
              "binius64-ligerito-rbr": "Binius (Johnson) at "}[family]
    return prefix + ("rate $1/%d$" % (1 << int(rate)) if rate else "any rate")
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
        key = (scheme_key(row), row["log_multiplications"], row["threads"])
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
        key = (scheme_key(row), row["log_multiplications"], row["threads"])
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
    ap.add_argument("--out", type=Path, default=None, help="default: outputs/tables/native-mul-table.tex (u32) or outputs/tables/native-mul-<workload>-table.tex")
    ap.add_argument("--exponents", default="", help="comma list or lo-hi; default: every size in the run")
    ap.add_argument("--workload", default="u32-mod32", choices=["u32-mod32", "u32", "u64", "u128"])
    ap.add_argument("--memory-bound", default="", metavar="BACKEND:EXP[,...]",
                    help="rows measured while paging (prover exceeded the machine's memory): omitted from the table and noted in the caption")
    ap.add_argument("--paging", default="", metavar="BACKEND:EXP[,...]",
                    help="rows measured while paging that are REPORTED anyway: kept in the table, with the caption saying which they are")
    ap.add_argument("--unsupported", default="", metavar="BACKEND:EXP[,...]",
                    help="sizes the backend itself refuses (a capacity limit, not the machine): shown as `--` and explained in the caption")
    ap.add_argument("--unsupported-reason", default="", metavar="TEXT",
                    help="the sentence explaining --unsupported, e.g. why the backend rejects those sizes")
    ap.add_argument("--label", default="", metavar="LABEL",
                    help="LaTeX label; default tab:native-mul[-<workload>]. Set it when a variant table (e.g. a "
                         "single-thread run) must not collide with the main one")
    ap.add_argument("--pick-least-disturbed", action="store_true",
                    help="when a (scheme, size) was measured in more than one run directory, keep the run with the "
                         "fastest median prover among those whose measured samples spread at most 10%% (max/min), "
                         "or the smallest spread if none qualifies; every disturbance on a shared box only slows a "
                         "run, so this is the run closest to an idle machine. Choices are listed in the header")
    ap.add_argument("--allow-source-drift", default="", metavar="BACKEND[,...]",
                    help="accept rows of one backend from different source trees, and name the trees in the "
                         "header. Use ONLY when the measuring binary is provably identical (its hash did not "
                         "change) and the drift is post-processing code such as this generator")
    ap.add_argument("--drop", default="", metavar="SCHEME:EXP[,...]",
                    help="rows to leave out as if not run (e.g. a measurement taken on a memory-starved box, pending a re-run); scheme keys as in --memory-bound")
    ap.add_argument("--proof-sizes-from", default="", metavar="RUN_DIR[,...]",
                    help="run directories that only contribute proof_bytes (only when protocol, corpus, source, machine and measurement policy match")
    args = ap.parse_args()
    if args.workload == "u32":
        args.workload = "u32-mod32"
    def pairs(value):
        """(scheme key, exponent) set; a bare family like `binius64` covers every rate."""
        out = set()
        for item in filter(None, value.split(",")):
            backend, exp = item.split(":")
            out.add((backend, int(exp)))
        return out
    memory_bound = pairs(args.memory_bound)
    paging = pairs(args.paging)
    unsupported = pairs(args.unsupported)
    if unsupported and not args.unsupported_reason:
        ap.error("--unsupported needs --unsupported-reason")
    if args.out is None:
        args.out = Path("outputs/tables/native-mul-table.tex" if args.workload == "u32-mod32" else f"outputs/tables/native-mul-{args.workload}-table.tex")

    drift_allowed = {b for b in args.allow_source_drift.split(",") if b}
    drifted = {}
    def prover_spread(run_dir: Path, key) -> float:
        values = []
        for line in (run_dir / "samples.jsonl").read_text().splitlines():
            row = json.loads(line)
            if (row["workload"] == args.workload and row["trial"]["kind"] == "sample"
                    and scheme_key(row) == key[0] and row["log_multiplications"] == key[1]
                    and row["threads"] == key[2]):
                values.append(row["metrics"]["online_prover_ms"])
        return (max(values) - min(values)) / min(values) if values else float("inf")

    by = {}
    picks = {}      # key -> list of (run_dir name, spread, median) considered
    cross = {}      # key -> {tree: proof_bytes}, for the drift check
    candidates = {}
    for run_dir in args.run_dirs:
        dir_sizes = proof_sizes(run_dir, args.workload)
        for key, row in load_summaries(run_dir, args.workload).items():
            if key in candidates:
                require_compatible(candidates[key][0][0], row,
                                   allow_source_drift=key[0].split("@")[0] in drift_allowed)
            row["run_dir"] = str(run_dir)
            cross.setdefault(key, {})[row["provenance"]["source_sha256"][:12]] = dir_sizes.get(key)
            candidates.setdefault(key, []).append((row, prover_spread(run_dir, key)))
    for key, options in candidates.items():
        if args.pick_least_disturbed and len(options) > 1:
            quiet = [o for o in options if o[1] <= 0.10]
            chosen = (min(quiet, key=lambda o: o[0]["medians"]["online_prover_ms"]) if quiet
                      else min(options, key=lambda o: o[1]))
            picks[key] = [(Path(o[0]["run_dir"]).name, o[1], o[0]["medians"]["online_prover_ms"]) for o in options]
        else:
            chosen = options[-1]  # last directory given wins, as before
        by[key] = chosen[0]
    known = dict(SCHEMES)
    for slug, e, t in by:
        if slug not in known:
            raise ValueError(
                f"unknown table scheme {slug!r} at 2^{e}/{t} threads — the suite renders the round-by-round "
                "opener rows only; re-measure union-bound binius64-ligerito runs with BITZ_BINIUS_LIGERITO_ACCOUNTING=rbr")
    identities = {}
    source_by_backend = {}
    machines = [row["provenance"]["machine"] for row in by.values()]
    if any(machine != machines[0] for machine in machines):
        raise ValueError("comparison mixes measurement machines")
    for row in by.values():
        source = row["provenance"]
        source_identity = (source["source_sha256"], {k: v for k, v in source["build"].items() if k != "threads"})
        backend = row["backend"]
        if backend in source_by_backend and source_by_backend[backend] != source_identity:
            if backend not in drift_allowed:
                raise ValueError("comparison mixes source revisions or builds for one backend")
            drifted.setdefault(backend, set()).update(
                {source_by_backend[backend][0][:12], source_identity[0][:12]})
        source_by_backend[backend] = source_identity
        # The thread count is a table dimension now, so the per-size identity
        # covers corpus, policy and machine — every thread group of one size
        # must still prove the same corpus under the same policy.
        exponent = row["log_multiplications"]
        identity = (row["corpus_digest"], row["measurement_policy"], row["provenance"]["machine"])
        if exponent in identities and identities[exponent] != identity:
            raise ValueError("comparison mixes corpora, machines, or measurement policies")
        identities[exponent] = identity
    dropped = set()
    for item in filter(None, args.drop.split(",")):
        scheme, exp = item.split(":")
        dropped.add((scheme, int(exp)))
    for k in list(by):
        if (k[0], k[1]) in dropped or (k[0].split("@")[0], k[1]) in dropped:
            by.pop(k)
    def selected(key, chosen):
        """Annotation selectors are (scheme, exponent) and cover both thread counts."""
        return (key[0], key[1]) in chosen or (key[0].split("@")[0], key[1]) in chosen
    paged = {k: by.pop(k) for k in list(by) if selected(k, memory_bound)}
    for k in list(by):
        if selected(k, unsupported):
            by.pop(k)
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
    # The proof does not depend on the thread count: where one (scheme, size)
    # was measured at several thread counts, the median proof sizes must agree.
    for slug, e in {(k[0], k[1]) for k in by}:
        per_thread = {k[2]: sizes[k] for k in sizes if (k[0], k[1]) == (slug, e) and k in by}
        if len(set(per_thread.values())) > 1:
            raise ValueError(f"{slug} at 2^{e}: proof sizes differ between thread counts {per_thread}")
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
    threads_list = sorted({r["threads"] for r in rows})
    run_list = " ".join(str(d) for d in args.run_dirs)
    cpu = rows[0]["provenance"]["machine"]["cpu"]
    thread_counts = ", ".join(map(str, sorted({r["threads"] for r in rows})))
    machine_text = cpu.replace("_", r"\_")
    commit = ", ".join(sorted({r["provenance"]["git_revision"] + ("+dirty" if r["provenance"]["git_dirty"] else "") for r in rows}))
    date = datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%d")

    out = []
    w = out.append
    w("% Native end-to-end u32-multiplication comparison (BitZ vs Binius64 vs Plonky3/FRI) — GENERATED FILE, do not edit by hand.")
    w(f"% Generated by scripts/native_mul_table.py on {date} (UTC) at {commit} from {run_list} (benches/mul_e2e_compare.rs,")
    w("%   see README.md, Integer multiplication; later run directories override earlier ones per scheme and size).")
    if size_dirs:
        w(f"% Proof sizes for rows whose run predates proof-size recording come from {' '.join(map(str, size_dirs))}.")
    w("% Regenerate (from the repo root; this file is overwritten):")
    w(f"%   python3 scripts/native_mul_table.py {run_list} --workload {args.workload}" + (f" --exponents {args.exponents}" if args.exponents else "")
      + (f" --memory-bound {args.memory_bound}" if args.memory_bound else "")
      + (f" --paging {args.paging}" if args.paging else "")
      + (f" --unsupported {args.unsupported}" if args.unsupported else "")
      + (f" --unsupported-reason {args.unsupported_reason!r}" if args.unsupported_reason else "")
      + (f" --allow-source-drift {args.allow_source_drift}" if args.allow_source_drift else "")
      + (f" --label {args.label}" if args.label else "")
      + (" --pick-least-disturbed" if args.pick_least_disturbed else "")
      + (f" --drop {args.drop}" if args.drop else "") + (f" --proof-sizes-from {args.proof_sizes_from}" if args.proof_sizes_from else ""))
    w(f"% Machine: {cpu}; thread counts {', '.join(map(str, threads_list))}; medians of {'/'.join(map(str, samples))} samples after one warm-up.")
    for backend, trees in sorted(drifted.items()):
        shared = [k for k, per in cross.items() if k[0].split("@")[0] == backend and len(per) > 1]
        for k in shared:
            if len(set(cross[k].values())) != 1:
                raise ValueError(f"source drift for {backend} at 2^{k[1]}: proof bytes differ across trees {cross[k]}")
        w(f"% Source-tree drift accepted for {backend}: rows come from trees {', '.join(sorted(trees))}."
          f" Proof bytes are identical across the trees at every shared size ({len(shared)} checked), so the"
          " measured computation is the same; the trees differ in harness plumbing or post-processing.")
    if args.pick_least_disturbed and picks:
        w("% --pick-least-disturbed: where a case was measured in several run directories, the run with the")
        w("%   fastest median prover among those with <=10% sample spread was kept (or the smallest spread if")
        w("%   none qualified); * marks the kept run, entries are dir=median ms/spread:")
        for key in sorted(picks):
            chosen = Path(by[key]["run_dir"]).name
            w(f"%   2^{key[1]} t={key[2]:<2} {key[0]:13} " + ", ".join(f"{d}={med:.0f}/{sp*100:.0f}%" + (" *" if d == chosen else "") for d, sp, med in picks[key]))
    w("% Columns: witgen = witness_ms (native witness generation; Binius64 packs its witness inside its prover, so its witgen")
    w("%   overlaps the prover column); prover = online_prover_ms (the complete native prover call after witness generation,")
    w("%   i.e. commitment + PIOP + PCS opening); verifier = verify_ms; proof = median proof_bytes of the samples, KB = 1000 bytes;")
    w("%   peak mem. = peak_rss_bytes of the separate single-proof memory child, GB = 2^30 bytes (`--` where the run predates it).")
    w("% Bold = best of the schemes for that size and column (each thread count is its own column).")
    w("% Medians (ms) as recorded in summary.json (`--` rows below = scheme not run at that size and thread count):")
    for e in exps:
        for t in threads_list:
            for slug, _ in SCHEMES:
                r = by.get((slug, e, t))
                if r:
                    m = r["medians"]
                    size = sizes.get((slug, e, t))
                    w(f"%   2^{e} t={t:<2} {slug:13s} witness={m['witness_ms']:.3f} "
                      f"online_prover={m['online_prover_ms']:.3f} witness_to_proof={m['witness_to_proof_ms']:.3f} verify={m['verify_ms']:.3f} setup={r['setup_ms']:.3f} "
                      f"proof_bytes={'n/a' if size is None else int(size)} samples={r['samples']} run={Path(r['run_dir']).name}")
                elif any((slug, e2, t2) in by for e2 in exps for t2 in threads_list):
                    w(f"%   2^{e} t={t:<2} {slug:13s} not run")
    # Sizes above a scheme's memory wall are implied by the wall, not "not run".
    present_pairs = {(k[0], k[1]) for k in by}
    paged_pairs = {(k[0], k[1]) for k in paged}
    wall = {}
    for slug, e in paged_pairs:
        wall[slug] = min(wall.get(slug, e), e)
    missing = {}
    for slug, label in SCHEMES:
        gone = [e for e in exps if (slug, e) not in present_pairs and (slug, e) not in paged_pairs
                and not ((slug, e) in unsupported or (slug.split("@")[0], e) in unsupported)
                and e < wall.get(slug, float("inf"))]
        if gone and any((slug, e) in present_pairs for e in exps):
            missing[slug] = gone
    for (slug, e, t), r in sorted(paged.items()):
        m = r["medians"]
        w(f"%   2^{e} t={t:<2} {slug:13s} OMITTED (paging): online_prover={m['online_prover_ms']:.3f} verify={m['verify_ms']:.3f} run={Path(r['run_dir']).name}")
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
    schemes = [(slug, label) for slug, label in SCHEMES if slug in {p[0] for p in present_pairs}]

    def listed(chosen):
        """Group the selected (scheme, exponent) pairs by scheme, in table order."""
        out = {}
        for slug, _ in SCHEMES:
            sizes = sorted(e for e in exps if selected((slug, e), chosen))
            if sizes:
                out[slug] = sizes
        return out

    def sizes_text(sizes):
        return ", ".join(f"$2^{{{e}}}$" for e in sizes)

    def join(parts):
        if len(parts) < 3:
            return " and ".join(parts)
        return ", ".join(parts[:-1]) + ", and " + parts[-1]

    unsupported_sentence = ""
    if unsupported:
        parts = [f"{scheme_name(slug)} at {sizes_text(sizes)}" for slug, sizes in listed(unsupported).items()]
        verb = "is" if len(parts) == 1 else "are"
        unsupported_sentence = (join(parts) + f" {verb} absent because "
                                + args.unsupported_reason.rstrip(".") + ". ")
    paging_sentence = ""
    reported = listed(paging)
    if reported:
        parts = [f"{scheme_name(slug)} at {sizes_text(sizes)}" for slug, sizes in reported.items()]
        paging_sentence = ("The prover's working set exceeds the machine's memory for "
                           + join(parts) + ", so those rows are reported but paging-dominated "
                           "and are not comparable with the rest; their peak memory is capped by memory pressure and not shown. ")
    w("")
    w("\\begin{table}[H]")
    w("  \\centering")
    w("  \\small")
    w("  \\setlength{\\tabcolsep}{4pt}")
    span = len(threads_list)
    w("  \\begin{tabular}{@{}rl" + "r" * (1 + 2 * span + 2) + "@{}}")
    w("    \\toprule")
    pstart = 4
    vstart = pstart + span
    w("     &  & Witgen & \\multicolumn{%d}{c}{Prover (ms)} & \\multicolumn{%d}{c}{Verifier (ms)} & Proof & Peak mem. \\\\" % (span, span))
    w("    \\cmidrule(lr){%d-%d} \\cmidrule(lr){%d-%d}" % (pstart, vstart - 1, vstart, vstart + span - 1))
    thr = " & ".join(f"{t} thr" for t in threads_list)
    w("    $N$ & Scheme & (ms) & " + thr + " & " + thr + " & (KB) & (GB) \\\\")
    w("    \\midrule")
    ref_t = max(threads_list)
    order_t = [ref_t] + [t for t in threads_list if t != ref_t]
    for gi, e in enumerate(exps):
        present = [slug for slug, _ in SCHEMES if any((slug, e, t) in by for t in threads_list)]
        def single(slug, getter):
            # Thread-independent columns: the largest thread count's run, else any.
            for t in order_t:
                r = by.get((slug, e, t))
                if r is not None:
                    return getter(r, t)
            return None
        witgen = {s: single(s, lambda r, t: r["medians"]["witness_ms"]) for s in present}
        peak = {s: single(s, lambda r, t: r.get("peak_rss_bytes") or None) for s in present}
        size = {s: single(s, lambda r, t, s=s: sizes.get((s, e, t))) for s in present}
        # Paging rows: the peak resident set is capped by memory pressure, so it is not shown.
        peak = {s: (None if selected((s, e), paging) else v) for s, v in peak.items()}
        def med(s, t, k):
            r = by.get((s, e, t))
            return None if r is None else r["medians"][k]
        prov = {(s, t): med(s, t, "online_prover_ms") for s in present for t in threads_list}
        ver = {(s, t): med(s, t, "verify_ms") for s in present for t in threads_list}
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
        for ri, (slug, label) in enumerate(schemes):
            n_cell = f"$2^{{{e}}}$" if ri == 0 else ""
            if slug not in present:
                w(f"    {n_cell} & {label} & " + " & ".join([PLACEHOLDER] * (1 + 2 * span + 2)) + " \\\\")
                continue
            cells = [cell(witgen[slug], b_w, fmt_ms)]
            cells += [cell(prov[(slug, t)], b_pr[t], fmt_ms) for t in threads_list]
            cells += [cell(ver[(slug, t)], b_vr[t], fmt_ms) for t in threads_list]
            cells += [cell(size[slug], b_sz, lambda v: fmt_ms(v / 1000)), cell(peak[slug], b_pk, fmt_gb)]
            w(f"    {n_cell} & {label} & " + " & ".join(cells) + " \\\\")
        if gi + 1 < len(exps):
            w("    \\addlinespace")
    w("    \\bottomrule")
    w("  \\end{tabular}")
    statement = {
        "u32-mod32": "Native end-to-end proofs of $N$ independent multiplications $z = xy \\bmod 2^{32}$, with $x,y,z$ unsigned $32$-bit integers: ",
        "u64": "Native end-to-end proofs of $N$ multiplications $x \\cdot y = z$ of random $64$-bit integers ($z$ a $128$-bit integer; from $2^{21}$ the \\ftwoz\\ layout caps its row side at $t = 18$, which trades read-off bytes for prover time): ",
        "u128": "Native end-to-end proofs of $N$ multiplications $x \\cdot y = z$ of random $128$-bit integers ($z$ a $256$-bit integer): ",
    }[args.workload]
    # Binius64's opener geometry as the runs recorded it (rate override
    # BITZ_BINIUS_LOG_INV_RATE; the query count follows from the rate).
    binius_keys = [slug for slug, _ in schemes if slug.startswith("binius64@")]
    binius_rate_text = ""
    if binius_keys:
        parts = []
        for key in binius_keys:
            log_rate = int(key.split("@")[1])
            queries = next((int(r["config"].get("fri_queries", BINIUS_QUERIES.get(log_rate, 0))) for r in rows if scheme_key(r) == key), BINIUS_QUERIES.get(log_rate, 0))
            parts.append(f"rate $1/{1 << log_rate}$ with ${queries}$ queries")
        binius_rate_text = " and ".join(parts) + " for $100$ bits"
    binius_note = "Binius (UDR) (Binius64 with native multiplication" + (" and bit constraints" if args.workload not in ("u64", "u128") else "") + f", ring switching and BaseFold at {binius_rate_text})"
    # The opener geometry of the binius64-ligerito rows as the runs recorded
    # it, one clause per rate (queries are size-independent; fold grinding and
    # the achieved bound move with N, so both are given as ranges).
    def span(cfgs, name, fmt):
        """Math-wrapped value or en-dashed range across the recorded configs."""
        values = [c[name] for c in cfgs if isinstance(c.get(name), (int, float))]
        if not values:
            return None
        lo, hi = min(values), max(values)
        return f"${fmt(lo)}$" if fmt(lo) == fmt(hi) else f"${fmt(lo)}$--${fmt(hi)}$"
    def ligerito_family_note(family):
        parts = []
        for key in [slug for slug, _ in schemes if slug.startswith(family + "@")]:
            cfgs = [r["config"] for r in rows if scheme_key(r) == key]
            if not cfgs:
                continue
            log_rate = int(key.split("@")[1])
            queries = span(cfgs, "level0_queries", lambda v: f"{int(v)}")
            fold = span(cfgs, "level0_fold_grinding_bits", lambda v: f"{int(v)}")
            comp = span(cfgs, "ligerito_component_bits", lambda v: f"{int(v)}")
            gated = span(cfgs, "whole_protocol_bits", lambda v: f"{v:.1f}")
            union = span(cfgs, "union_bound_bits", lambda v: f"{v:.1f}")
            inner = [t for t in [f"{gated} bits achieved" if gated else "",
                                 f"the union bound over the same terms is {union} bits" if union else ""] if t]
            parts.append(f"rate $1/{1 << log_rate}$"
                         + (f" with {queries} level-0 queries" if queries else "")
                         + (f", {fold} bits of fold grinding" if fold else "")
                         + (f" and a {comp}-bit per-round target" if comp else "")
                         + (" (" + "; ".join(inner) + ")" if inner else ""))
        return ("Binius (Johnson) (the same Binius64 circuit and PIOP; every oracle committed and opened by "
                "ring switching and Johnson-regime Ligerito with Round~0 at " + " and at ".join(parts)
                + "; every error term is gated at $100$ bits on its own --- the round-by-round minimum the \\ftwoz-SNARK rows report)")
    fri_rows = [r for r in rows if r["backend"] == "plonky3-fri"]
    fri_note = ("Plonky3 (Goldilocks AIR, degree-$5$ extension, FRI at rate $1/2$, the smallest query count "
                "clearing a proven round-by-round $100$-bit report per size)")
    if fri_rows:
        fri_queries = sorted({int(r["config"]["num_queries"]) for r in fri_rows})
        qtext = f"${fri_queries[0]}$" if len(fri_queries) == 1 else f"${fri_queries[0]}$--${fri_queries[-1]}$"
        fri_note = ("Plonky3 (Goldilocks AIR, degree-$5$ extension, FRI at rate $1/2$ with the smallest query "
                    f"count clearing a proven round-by-round $100$-bit report per size: {qtext} queries)")
    limber_rows = [r for r in rows if r["backend"] == "limber"]
    limber_note = ("Limber (one independent integer-mod R1CS row per multiplication, IntEval/Brakedown, "
                   "Brakedown column-open target $100$ bits)")
    if limber_rows:
        targets = {int(r["config"]["target_bits"]) for r in limber_rows}
        if len(targets) != 1:
            raise ValueError(f"Limber rows mix Brakedown targets {sorted(targets)}")
        limber_note = ("Limber (one independent integer-mod R1CS row per multiplication, IntEval/Brakedown; "
                       f"Brakedown column-open target ${targets.pop()}$ bits, its native IntEval $128$-bit, "
                       "challenge $117$-bit and $2^{-114}$ fingerprint terms unchanged)")
    scheme_notes = {
        "bitz": bitz_caption(rows),
        "binius64": binius_note,
        "binius64-ligerito-rbr": ligerito_family_note("binius64-ligerito-rbr"),
        "plonky3-fri": fri_note,
        "plonky3-whir": "Plonky3 (shared Goldilocks mod32 AIR, multilinear zerocheck/sumcheck, WHIR with per-run tuning and at least $100$ bits under the recorded Johnson accounting)",
        "limber": limber_note,
    }
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
      + f"\\emph{{Witgen}} and \\emph{{peak mem.}} are from the {max(threads_list)}-thread runs; proof sizes do not depend on the thread count. "
      + "".join(f"{scheme_name(slug)} was not run at " + ", ".join(f"$2^{{{e}}}$" for e in gone) + ". " for slug, gone in missing.items())
      + "".join(wall_sentence(e, slugs) for e, slugs in sorted(walls.items()))
      + unsupported_sentence + paging_sentence
      + f"{machine_text}; threads per run: {thread_counts}; medians of {samples[0]} runs after one warm-up.}}")
    label = args.label or ("tab:native-mul" + ("" if args.workload == "u32-mod32" else "-" + args.workload))
    w("  \\label{" + label + "}")
    w("\\end{table}")
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text("\n".join(out) + "\n")
    print(f"wrote {args.out} ({len(exps)} sizes, {len(schemes)} schemes)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
