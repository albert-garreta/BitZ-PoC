#!/usr/bin/env python3
"""LaTeX tables of absolute numbers from SHA-256 + ECDSA campaigns (scripts/run_sha256_ecdsa_compare.py).

Reads the `*.result.json` records of one or more campaign output directories
(later directories override earlier ones for the same case) and writes one
self-documenting table per signature curve: one row group per message size,
one row per scheme, with the medians of witness generation, the complete prover
(commitment included) and the complete verifier, plus the complete proof
material and the worker's peak memory.

A campaign names a curve, and the two curves answer different questions, so
they never share a table:

    secp256k1  the head-to-head: Binius64 on its own upstream verifier, BitZ on
               a circuit matched to that schedule
               -> paper/sha256-ecdsa-secp256k1-table.tex
    p256       BitZ alone, on the circuit the paper documents; Binius64
               implements ECDSA over secp256k1 only
               -> paper/sha256-ecdsa-p256-table.tex

    python3 scripts/sha256_ecdsa_table.py bench_results/suite-sha256-ecdsa-<date> \\
        bench_results/suite-sha256-ecdsa-p256-<date>
"""
from __future__ import annotations

import argparse
import datetime
import json
import platform
import statistics
import subprocess
from pathlib import Path

SCHEMA = "bitz/sha256-ecdsa-compare/v1"
# Table rows are (scheme id, LaTeX label), in display order. A scheme id is
# `method@log_inv_rate`: the BitZ rows split by their Ligerito profile's
# level-0 rate, the Binius-family rows by their commitment rate. Naming
# follows the 2026-09-13 directive: no `(this work)`, no \cite after names.
SCHEMES = [
    ("bitz-split@1", "\\ftwoz-SNARK, rate $1/2$"),
    ("bitz-split@3", "\\ftwoz-SNARK, rate $1/8$"),
    ("binius64@1", "Binius (UDR), rate $1/2$"),
    ("binius64@3", "Binius (UDR), rate $1/8$"),
    ("binius64-ligerito@1", "Binius (Johnson), rate $1/2$"),
    ("binius64-ligerito@3", "Binius (Johnson), rate $1/8$"),
]
PLACEHOLDER = "--"
# Campaign curves (the runner's `--curve`), each with the file and label of its table.
CURVES = {
    "secp256k1": dict(name="secp256k1", file="sha256-ecdsa-secp256k1-table.tex", label="tab:sha256-ecdsa-secp256k1"),
    "p256": dict(name="P-256", file="sha256-ecdsa-p256-table.tex", label="tab:sha256-ecdsa-p256"),
}
# The ECDSA verifier a BitZ row ran, by the worker's `circuit_profile` (benches/sha256_ecdsa_compare.rs; the
# schedules are `LadderProfile` in crates/circuit/src/p256.rs). Rows from before the profile was recorded get no
# description: across revisions a bare curve token does not say which schedule the binary ran.
BITZ_CIRCUITS = {
    "sha256-chain-secp256k1/binius-matched/v1":
        "ECDSA verifier on Binius64's scalar-multiplication schedule, so that the two systems are measured on one "
        "schedule --- uniform $4$-bit windows over the four GLV bases, every table built in-circuit, accumulator "
        "starting at the identity",
    "sha256-chain-secp256k1/optimized/v1":
        "its own ECDSA verifier --- GLV endomorphism splitting, an $8$-bit window over the public generator table, "
        "$4$-bit windows over the committed tables, offset accumulator",
    "sha256-chain-p256/paper/v1":
        "ECDSA verifier with complete additions throughout and an accumulator starting at the identity",
    "sha256-chain-p256/optimized/v1":
        "its own ECDSA verifier --- an $8$-bit window over the public generator table, $4$-bit windows over the "
        "committed tables, offset accumulator",
}
# The ECDSA verifier behind the Binius-family rows. Upstream Binius64 implements ECDSA over secp256k1 only
# (`ecdsa::bitcoin_verify`); the pinned tree's P-256 verifier was ported into it for this comparison, which is why
# the runner no longer measures it. Campaigns that did are still rendered, and say so.
BINIUS_VERIFIERS = {
    "secp256k1": "its own secp256k1 ECDSA verifier",
    "p256": "a complete-arithmetic P-256 verifier ported into the pinned Binius64 tree for this comparison "
            "(Binius64 itself implements ECDSA over secp256k1 only)",
}


def scheme_id(case: dict, sample: dict) -> str:
    """`method@rate` for suite methods; the bare method otherwise."""
    method = case["method"]
    security = sample.get("security") or {}
    if method.startswith("bitz"):
        lig = security.get("ligerito") or {}
        levels = (lig.get("configuration") or lig).get("levels") or [{}]
        return f"{method}@{int(levels[0].get('log_inv_rate', 1))}"
    if method.startswith("binius64"):
        rate = case.get("log_inv_rate", security.get("log_inv_rate", 1))
        return f"{method}@{int(rate)}"
    return method


def campaign_curve(token) -> str:
    """The campaign curve behind a row's `curve` token (`secp256k1-matched` -> `secp256k1`).

    Rows from before the curve split carry no token; every campaign was P-256 then.
    """
    curve = "p256" if token is None else str(token).split("-")[0]
    if curve not in CURVES:
        raise ValueError(f"unknown curve {token!r}")
    return curve


def family(scheme: str) -> str:
    """`bitz` or `binius64`: the rows that share one circuit within a campaign."""
    return scheme.split("@")[0].split("-")[0]


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


def load_cases(directory: Path) -> tuple:
    """(campaign curve, complete cases keyed by (log_compressions, security_target, threads, scheme))."""
    manifest = directory / "manifest.json"
    declared = json.loads(manifest.read_text()).get("curve") if manifest.exists() else None
    result, curves = {}, set()
    for path in sorted(directory.glob("*.result.json")):
        record = json.loads(path.read_text())
        if record["status"] != "complete":
            continue
        case = record["case"]
        samples = [r for r in record["rows"] if r.get("trial") == "sample"]
        if not samples or any(r.get("schema") != SCHEMA or r.get("verified") is not True for r in samples):
            raise ValueError(f"{path}: complete case without verified samples")
        ran = {(r.get("curve"), r.get("circuit_profile")) for r in samples}
        if len(ran) != 1:
            raise ValueError(f"{path}: samples of one case ran different circuits: {sorted(map(str, ran))}")
        token, profile = next(iter(ran))
        curves.add(campaign_curve(token))
        key = (case["log_compressions"], case["security_target"], case["threads"], scheme_id(case, samples[0]))
        if key in result:
            raise ValueError(f"duplicate case {key} in {directory}")
        med = lambda k: statistics.median(r[k] for r in samples)  # noqa: E731
        result[key] = dict(
            key=key, run_dir=str(directory), file=path.name, samples=len(samples),
            seed=case["seed"], compressions=samples[0]["compressions"], message_bytes=samples[0]["message_bytes"],
            fixture_id=samples[0]["fixture_id"], security=samples[0].get("security", {}),
            curve_token=token, circuit_profile=profile,
            circuit=samples[0].get("circuit"), peak_rss_bytes=record["peak_rss_bytes"],
            witness_ms=med("witness_ms"), prove_ms=med("prove_ms"), e2e_prover_ms=med("e2e_prover_ms"),
            verify_ms=med("verify_ms"), proof_bytes=med("proof_material_bytes"), setup_ms=med("setup_ms"),
            opening_ms=med("opening_ms") if all(isinstance(r.get("opening_ms"), (int, float)) for r in samples) else None,
        )
    if len(curves) > 1:
        raise ValueError(f"{directory}: one campaign directory holds rows of several curves: {sorted(curves)}")
    if declared is not None and curves and curves != {declared}:
        raise ValueError(f"{directory}: the manifest names curve {declared!r} but the rows ran {sorted(curves)}")
    return (next(iter(curves)) if curves else declared), result


def circuits(by: dict) -> dict:
    """The one circuit each scheme family ran, as (curve token, circuit_profile).

    The runner refuses to record a mismatched pair, but merging run directories
    can still assemble one: rows that ran two circuits never share a table.
    """
    ran = {}
    for key, row in by.items():
        ran.setdefault(family(key[3]), set()).add((row["curve_token"], row["circuit_profile"]))
    for name, ids in sorted(ran.items()):
        if len(ids) != 1:
            raise SystemExit(f"{name} rows ran different circuits and cannot share a table: {sorted(map(str, ids))}")
    return {name: next(iter(ids)) for name, ids in ran.items()}


def bitz_security(row: dict) -> str:
    """One caption clause for an BitZ row's Ligerito policy at level 0."""
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


def opener_security(row: dict) -> str:
    """One caption clause per binius64-ligerito rate."""
    s = row["security"]
    return (f"rate $1/{1 << int(s.get('log_inv_rate', 1))}$: opener component target {s.get('component_bits')} bits, "
            f"{s.get('round_by_round_bits', 0):.1f} bits achieved")


def render(curve: str, by: dict, run_dirs: list, out: Path, label: str, regenerate: str) -> None:
    """Write the table of one campaign curve."""
    fixtures = {}
    for row in by.values():
        fixtures.setdefault((row["key"][0], row["seed"]), set()).add(row["fixture_id"])
    if any(len(ids) != 1 for ids in fixtures.values()):
        raise SystemExit("cases at one size and seed used different fixtures")
    ran = circuits(by)

    exponents = sorted({k[0] for k in by})
    targets = sorted({k[1] for k in by if k[1] is not None})
    threads = sorted({k[2] for k in by})
    methods = [m for m, _ in SCHEMES if any(k[3] == m for k in by)]
    show_n, show_t = len(exponents) > 1, len(targets) > 1

    name = CURVES[curve]["name"]
    any_row = next(iter(by.values()))
    bitz_rows = [r for r in by.values() if r["key"][3].startswith("bitz")]
    binius_rows = [r for r in by.values() if r["key"][3].split("@")[0] == "binius64"]
    opener_rows = [r for r in by.values() if r["key"][3].split("@")[0] == "binius64-ligerito"]
    systems = [s for s, rows in [("BitZ", bitz_rows), ("Binius64", binius_rows),
                                 ("Binius64 with the BitZ opener", opener_rows)] if rows]
    # P-256 is BitZ alone by design; say why rather than leave the absence unexplained.
    solo = ("Binius64 implements ECDSA over secp256k1 only and has no row here"
            if curve == "p256" and not binius_rows and not opener_rows else None)

    stamp = datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%d")
    rev = probe(["git", "rev-parse", "HEAD"], "unknown")
    dirty = "+dirty" if probe(["git", "status", "--porcelain", "--untracked-files=no"], "") else ""
    machine = probe(["sysctl", "-n", "machdep.cpu.brand_string"], platform.processor() or platform.machine())
    for run_dir in run_dirs:
        manifest = run_dir / "manifest.json"
        if manifest.exists():
            cpu = json.loads(manifest.read_text()).get("cpu")
            if cpu and cpu not in ("arm", "arm64", "x86_64", "i386"):
                machine = cpu
    dirs = " ".join(str(d) for d in run_dirs)
    header = [
        f"% SHA-256 chain + {name} ECDSA " + ("comparison (" + " vs ".join(systems) + ")" if len(systems) > 1 else f"({systems[0]} alone; {solo})" if solo else f"({systems[0]} alone)") + " — GENERATED FILE, do not edit by hand.",
        f"% Generated by scripts/sha256_ecdsa_table.py on {stamp} (UTC) at {rev}{dirty} from {dirs}",
        "%   (benches/sha256_ecdsa_compare.rs + benchmarks/binius64, driven by scripts/run_sha256_ecdsa_compare.py;",
        "%   see docs/sha256-ecdsa-comparison.md; later run directories override earlier ones per case).",
        "% Regenerate (from the repo root; this file is overwritten):",
        f"%   {regenerate}",
        f"% Machine: {machine}; medians of the measured samples after one warm-up; one worker process per case.",
        f"% Curve: {curve} (the runner's --curve). A table holds one curve, and every row of a scheme family ran one circuit,",
        "%   recorded per row as (curve token, circuit_profile); `None` = from before the field was recorded:",
    ]
    header += [f"%   {fam:<9} {ran[fam][0]}  {ran[fam][1]}" for fam in sorted(ran)]
    header += [
        "% Columns: witgen = witness_ms (witness generation from the message and signature; the binius64 rows count their",
        "%   witness packing here, while the binius64-ligerito rows pack inside their prover, so their witgen is the wire",
        "%   assignment alone); prover = prove_ms = commit_ms + protocol_ms (the complete prover after witness generation:",
        "%   commitment + PIOP + PCS opening); verifier = verify_ms (statement validation + verification of the decoded proof);",
        "%   proof = proof_material_bytes (complete proof material incl. commitments and auxiliary public inputs, statement",
        "%   excluded), KB = 1000 bytes; peak mem. = peak_rss_bytes of the whole worker process (setup, warm-up and all",
        "%   samples included), GB = 2^30 bytes.",
        "% Scheme ids are method@log_inv_rate (@1 = rate 1/2, @3 = rate 1/8); binius64-ligerito rows are gated round-by-round at 100 bits.",
        "% Bold = best of the schemes for that (size, target) group and column (each thread count is its own column).",
        "% Medians as recorded (ms unless noted):",
    ]
    for key in sorted(by, key=lambda k: (k[0], k[1] if k[1] is not None else -1, k[2], methods.index(k[3]))):
        r = by[key]
        header.append(
            f"%   2^{key[0]} target={key[1]} threads={key[2]} {key[3]:<22} witness={r['witness_ms']:.3f} prove={r['prove_ms']:.3f} "
            f"e2e_prover={r['e2e_prover_ms']:.3f} verify={r['verify_ms']:.3f} setup={r['setup_ms']:.3f} "
            f"opening={r['opening_ms'] if r['opening_ms'] is None else round(r['opening_ms'], 3)} proof_bytes={int(r['proof_bytes'])} "
            f"peak_rss={r['peak_rss_bytes']} samples={r['samples']} file={r['file']}")
    circuit_dims = {}
    for key in sorted(by):
        circuit_dims.setdefault(key[3], json.dumps(by[key].get("circuit"), sort_keys=True))
    for method, circuit in circuit_dims.items():
        header.append(f"%   circuit {method}: {circuit}")

    ref_th = max(threads)
    order_th = [ref_th] + [x for x in threads if x != ref_th]
    # The size column names the message and, under it, the compression count -- the paper's own layout.
    messages = {}
    for key, row in by.items():
        messages.setdefault(key[0], row["message_bytes"])
    size_label = [lambda n: f"{messages[n]}\\,B", lambda n: f"$2^{{{n}}}$ comp."]
    lead = ([("Message", "r")] if show_n else []) + ([("$\\lambda$ (bits)", "r")] if show_t else [])
    span = len(threads)
    thr = " & ".join(f"{x} thr" for x in threads)
    c0 = len(lead) + 3
    lines = ["", "\\begin{table}[H]", "  \\centering", "  \\small", "  \\setlength{\\tabcolsep}{4pt}",
             "  \\begin{tabular}{@{}" + "".join(a for _, a in lead) + "l" + "r" * (1 + 2 * span + 2) + "@{}}", "    \\toprule",
             "    " + " & ".join([""] * len(lead) + ["", "Witgen"]) + f" & \\multicolumn{{{span}}}{{c}}{{Prover (ms)}} & \\multicolumn{{{span}}}{{c}}{{Verifier (ms)}} & Proof & Peak mem. \\\\",
             f"    \\cmidrule(lr){{{c0}-{c0 + span - 1}}} \\cmidrule(lr){{{c0 + span}-{c0 + 2 * span - 1}}}",
             "    " + " & ".join([h for h, _ in lead] + ["Scheme", "(ms)"]) + f" & {thr} & {thr} & (KB) & (GB) \\\\", "    \\midrule"]
    cols = ([("witness_ms", None, fmt_ms)] + [("prove_ms", x, fmt_ms) for x in threads]
            + [("verify_ms", x, fmt_ms) for x in threads] + [("proof_bytes", None, fmt_kb), ("peak_rss_bytes", None, fmt_gb)])
    ratios = []
    first_group = True
    for n in exponents:
        first_n = True
        for t in targets + ([None] if any(k[1] is None for k in by) else []):
            present = [m for m in methods if any((n, t, x, m) in by for x in threads)]
            if not present:
                continue
            if not first_group:
                lines.append("    \\addlinespace")
            first_group = False
            def value(m, k, x):
                for y in (order_th if x is None else [x]):
                    r = by.get((n, t, y, m))
                    if r is not None:
                        return r.get(k)
                return None
            grid = {m: [value(m, k, x) for k, x, _ in cols] for m in present}
            for rate in (1, 3):
                ours, theirs = grid.get(f"bitz-split@{rate}"), grid.get(f"binius64@{rate}")
                if ours and theirs:
                    x = [f"{b / a:.2f}x" if a and b else PLACEHOLDER for a, b in zip(ours, theirs)]
                    ratios.append(f"%   2^{n}" + (f" target={t}" if show_t else "") + f" rate 1/{1 << rate}: prover "
                                  + " | ".join(x[1:1 + span]) + ", verifier " + " | ".join(x[1 + span:1 + 2 * span])
                                  + f"; proof {x[-2]}; peak mem. {x[-1]}")
            best = []
            for ci, (_, _, f) in enumerate(cols):
                vals = [grid[m][ci] for m in present if grid[m][ci] is not None]
                best.append(f(min(vals)) if len(vals) > 1 else None)
            first_t = True
            for index, m in enumerate(present):
                cells = []
                if show_n:
                    cells.append(size_label[index](n) if first_n and index < len(size_label) else "")
                if show_t:
                    cells.append(("$%d$" % t if t is not None else "n/a") if first_t else "")
                first_t = False
                cells.append(dict(SCHEMES)[m])
                for ci, (_, _, f) in enumerate(cols):
                    v = grid[m][ci]
                    txt = PLACEHOLDER if v is None else f(v)
                    cells.append(f"\\textbf{{{txt}}}" if v is not None and best[ci] is not None and txt == best[ci] else txt)
                lines.append("    " + " & ".join(cells) + " \\\\")
            first_n = False  # the size column labels the first target group of a size, not every one
    lines += ["    \\bottomrule", "  \\end{tabular}"]
    if ratios:
        header += ["% Binius (UDR) over \\ftwoz-SNARK at equal rate, from the medians above: >1 favours \\ftwoz-SNARK, <1 favours Binius.",
                   "%   The ratios move with the message size and the thread count, so quote one together with both ("
                   + " | ".join(f"{x} thr" for x in threads) + "):"] + ratios

    clauses = []
    if bitz_rows:
        policies = []
        for r in sorted(bitz_rows, key=lambda r: r["key"]):
            clause = bitz_security(r)
            if clause not in policies:
                policies.append(clause)
        circuit = BITZ_CIRCUITS.get(ran["bitz"][1])
        clauses.append("\\ftwoz-SNARK (integer R1CS with $\\FF_2$-virtualization; " + (circuit + "; " if circuit else "")
                       + "round-by-round economic security model; " + "; ".join(policies) + ")")
    if binius_rows:
        policies = []
        for r in sorted(binius_rows, key=lambda r: r["key"]):
            clause = binius_security(r)
            if clause not in policies:
                policies.append(clause)
        s = binius_rows[0]["security"]
        clauses.append(f"Binius (UDR) (Binius64's fixed SHA-256 circuit and {BINIUS_VERIFIERS[curve]}, ring switching and BaseFold with "
                       f"{s.get('merkle_hash', 'SHA-256')} Merkle hashing; FRI query target only, " + "; ".join(policies) + ")")
    if opener_rows:
        policies = []
        for r in sorted(opener_rows, key=lambda r: r["key"]):
            clause = opener_security(r)
            if clause not in policies:
                policies.append(clause)
        circuit = "the same Binius64 circuit and PIOP" if binius_rows else f"Binius64's fixed SHA-256 circuit, {BINIUS_VERIFIERS[curve]} and its PIOP"
        clauses.append(f"Binius (Johnson) ({circuit}, every oracle committed and opened by the "
                       "\\ftwoz\\ opener --- Johnson regime, early Round-0 OOD, fold and query grinding --- gated at $100$ bits "
                       "under the round-by-round model, every error term at most $2^{-100}$ on its own; " + "; ".join(policies) + ")")
    compressions = any_row["compressions"]
    n_desc = f"$2^{{{exponents[0]}}} = {compressions}$" if len(exponents) == 1 else "$N$"
    msg = f"{any_row['message_bytes']:,}".replace(",", "{,}") + " bytes" if len(exponents) == 1 else "$64(N-1)$ bytes"
    threads_desc = " and ".join(str(t) for t in threads)
    reps = sorted({r["samples"] for r in by.values()})
    caption = (f"Proving one SHA-256 hash of a message of {msg} ({n_desc} compressions, padding block included) followed by one {name} ECDSA "
               f"signature verification of the digest, non-ZK: " + "; ".join(clauses) + ". "
               + (f"{solo}. " if solo else "")
               + ("Native security targets are reported separately; these are not a uniform complete-protocol bound. " if len(clauses) > 1 else "")
               + "\\emph{Witgen} is the witness generation from the message and signature; \\emph{prover} is the complete prover after "
               "witness generation, commitment included; \\emph{verifier} is the complete verification of the decoded proof; "
               "\\emph{proof} is the complete proof material, commitments included ($1$\\,KB $= 1000$ bytes); \\emph{peak mem.} is the "
               "high-water resident set of the worker process, setup and warm-up included ($1$\\,GB $= 2^{30}$ bytes). "
               f"Apple M5; threads per run: {threads_desc}; medians of {' or '.join(map(str, reps))} runs after one warm-up.")
    lines += ["  \\caption{" + caption + (" Prover and verifier times are given for %s threads; witgen and peak memory are from the %d-thread runs, and proof size does not depend on the thread count." % (" and ".join(map(str, threads)), max(threads))) + "}", f"  \\label{{{label}}}", "\\end{table}", ""]
    out.write_text("\n".join(header + lines))
    print(f"wrote {out} ({curve}, {len(by)} cases)")


def main(argv=None) -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("run_dirs", type=Path, nargs="+", metavar="RUN_DIR")
    ap.add_argument("--out-dir", type=Path, default=Path("paper"), help="where the per-curve tables go (default: paper)")
    ap.add_argument("--out", type=Path, help="one explicit output file; the run directories must then share a curve")
    ap.add_argument("--label", help="one explicit \\label; the run directories must then share a curve")
    args = ap.parse_args(argv)

    tables = {}
    for run_dir in args.run_dirs:
        try:
            curve, cases = load_cases(run_dir)
        except ValueError as error:
            raise SystemExit(str(error))
        if cases:
            table = tables.setdefault(curve, dict(by={}, run_dirs=[]))
            table["by"].update(cases)
            table["run_dirs"].append(run_dir)
    if not tables:
        raise SystemExit("no complete cases found")
    if len(tables) > 1 and (args.out or args.label):
        raise SystemExit(f"--out and --label name one table, but the run directories span curves {sorted(tables)}")
    options = "".join(f" {flag} {value}" for flag, value in [("--out-dir", args.out_dir if args.out_dir != Path("paper") else None),
                                                              ("--out", args.out), ("--label", args.label)] if value)
    for curve in sorted(tables, reverse=True):
        table = tables[curve]
        regenerate = "python3 scripts/sha256_ecdsa_table.py " + " ".join(str(d) for d in table["run_dirs"]) + options
        render(curve, table["by"], table["run_dirs"], args.out or args.out_dir / CURVES[curve]["file"],
               args.label or CURVES[curve]["label"], regenerate)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
