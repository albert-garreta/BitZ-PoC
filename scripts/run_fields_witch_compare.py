#!/usr/bin/env python3
"""BitZ vs fields-witch (Lev Soukhanov's characteristic-2 field switch).

fields-witch (github.com/morgana-proofs/fields-witch) proves the evaluation,
over F_p with p = 2^127 - 1, of the multilinear extension of 2^k integers in
[0, 2^127) committed densely over F_{2^128} (recursive Ligerito, UDR
100-bit queries, SHA-256 Merkle trees). BitZ proves the evaluation, over a
transcript-sampled prime q, of the MLE of 2^n W-bit cells committed as bits
(ring switch + recursive Ligerito, Johnson 100-bit target, BLAKE3).

The comparison is bit-matched: fields-witch at 2^k entries commits
2^k * 127 bits, which is the same commitment volume (16 bytes per entry) as
BitZ at n = k + 7 with W = 1. Every (scheme, shape, threads) cell runs in a
fresh process under /usr/bin/time -l (peak RSS), one untimed warm-up and
`--reps` measured prove+verify passes (medians), gated on measured CPU idle.

    python3 scripts/run_fields_witch_compare.py --sizes 14,16,18,20,22 --threads 1,8

Re-render an existing run: --render PerfRuns/<dir>/results.jsonl
"""
import argparse
import datetime as dt
import hashlib
import json
import math
import os
import platform
import re
import subprocess
import sys
import time

FIELD_BITS = 127
MAX_LIMB_BITS = 22
README_2_20 = [16, 16, 16, 15, 13, 13, 12, 11, 10, 9, 8, 8, 7, 6, 5, 4, 4, 3, 2, 2]


# --------------------------------------------------------------------------
# fields-witch limb schedules
# --------------------------------------------------------------------------

def _candidate_widths():
    """Minimal limb width for every reachable limb count ceil(127 / l).

    A wider limb than the minimal one for its limb count costs public-table
    entries without reducing the number of limb lookups, so the README's
    2^20 schedule only uses these widths."""
    minimal = {}
    for l in range(MAX_LIMB_BITS, 0, -1):
        minimal[math.ceil(FIELD_BITS / l)] = l
    return sorted(set(minimal.values()), reverse=True)


CANDIDATES = _candidate_widths()


def amortization(n, round_index, width):
    source_log = n if round_index == 0 else n - round_index - 1
    return math.ceil(FIELD_BITS / width) * 2.0 ** (source_log - width)


def limb_schedule(n, budget_log=None, min_amortization=16.0):
    """The README rule: a packed public table of at most 2^(n-2) entries
    while every round keeps >= 16 limb lookups per public table entry;
    reproduces the README's 2^20 schedule exactly (asserted below)."""
    budget = 1 << (n - 2 if budget_log is None else budget_log)
    schedule, used = [], 0
    for r in range(n):
        remaining_rounds = n - r - 1
        pick = 1
        for width in CANDIDATES:
            if amortization(n, r, width) < min_amortization:
                continue
            if used + (1 << width) + remaining_rounds * 2 > budget:
                continue
            pick = width
            break
        schedule.append(pick)
        used += 1 << pick
    return schedule


assert limb_schedule(20) == README_2_20, limb_schedule(20)


# --------------------------------------------------------------------------
# quiet-box gate and process runner
# --------------------------------------------------------------------------

def cpu_idle_percent():
    out = subprocess.run(["top", "-l", "2", "-n", "0", "-s", "1"],
                         capture_output=True, text=True, check=False).stdout
    idle = None
    for line in out.splitlines():
        m = re.search(r"CPU usage:.*?([\d.]+)% idle", line)
        if m:
            idle = float(m.group(1))
    return idle


def wait_for_quiet(min_idle, max_wait_s=600):
    start = time.time()
    while True:
        idle = cpu_idle_percent()
        if idle is None or idle >= min_idle:
            return idle
        if time.time() - start > max_wait_s:
            print(f"warning: box never reached {min_idle}% idle (last {idle}%)", flush=True)
            return idle
        time.sleep(5)


def run_timed(cmd, cwd=None, env=None):
    """Run under /usr/bin/time -l; return (stdout+stderr text, max RSS bytes, wall s)."""
    full = ["/usr/bin/time", "-l"] + cmd
    t0 = time.time()
    proc = subprocess.run(full, cwd=cwd, env=env, capture_output=True, text=True, check=False)
    wall = time.time() - t0
    text = proc.stdout + "\n" + proc.stderr
    if proc.returncode != 0:
        raise RuntimeError(f"command failed ({proc.returncode}): {' '.join(cmd)}\n{text[-4000:]}")
    m = re.search(r"(\d+)\s+maximum resident set size", text)
    rss = int(m.group(1)) if m else None
    return text, rss, wall


def file_sha256(path):
    h = hashlib.sha256()
    with open(path, "rb") as fh:
        for chunk in iter(lambda: fh.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def fields_witch_provenance(binary):
    """Base revision of the checkout the binary was built in (read from its
    metadata files, no VCS command) and content hashes of its sources and of
    the binary itself."""
    root = binary.split(os.sep + "target", 1)[0]
    head = None
    try:
        meta = os.path.join(root, ".git")
        if os.path.isfile(meta):
            meta = open(meta).read().split(":", 1)[1].strip()
        common = meta
        if os.path.exists(os.path.join(meta, "commondir")):
            common = os.path.normpath(os.path.join(meta, open(os.path.join(meta, "commondir")).read().strip()))
        ref = open(os.path.join(meta, "HEAD")).read().strip()
        if ref.startswith("ref: "):
            name, sha = ref[5:], None
            loose = os.path.join(common, name)
            if os.path.exists(loose):
                sha = open(loose).read().strip()
            elif os.path.exists(os.path.join(common, "packed-refs")):
                for line in open(os.path.join(common, "packed-refs")):
                    parts = line.split()
                    if len(parts) == 2 and parts[1] == name:
                        sha = parts[0]
            head = f"{name.rsplit('/', 1)[-1]}@{sha}"
        else:
            head = ref
    except OSError:
        pass
    h = hashlib.sha256()
    for entry in ("Cargo.toml", "Cargo.lock", "src", "examples"):
        path = os.path.join(root, entry)
        files = [path] if os.path.isfile(path) else sorted(
            os.path.join(d, f) for d, _, fs in os.walk(path) for f in fs)
        for f in files:
            h.update(os.path.relpath(f, root).encode() + b"\0")
            with open(f, "rb") as fh:
                h.update(fh.read())
    return {"fw_root": root, "fw_head": head, "fw_tree_sha256": h.hexdigest()[:16],
            "fw_binary_sha256": file_sha256(binary)[:16]}


# --------------------------------------------------------------------------
# fields-witch
# --------------------------------------------------------------------------

_UNIT = {"ns": 1e-6, "us": 1e-3, "ms": 1.0, "s": 1e3}


def _dur_ms(value, unit):
    return float(value) * _UNIT[unit]


def parse_fields_witch(text):
    """Medians (ms) of every profile row, keyed by section/row label."""
    rows = {}
    section = None
    row_re = re.compile(r"^\s+(.+?)\s+best\s+([\d.]+) (ns|us|ms|s), median\s+([\d.]+) (ns|us|ms|s), worst\s+([\d.]+) (ns|us|ms|s)")
    for line in text.splitlines():
        stripped = line.strip()
        if stripped == "prover":
            section = "prover"
            continue
        if stripped == "verifier":
            section = "verifier"
            continue
        m = row_re.match(line)
        if m and section:
            label = m.group(1).strip()
            rows[f"{section}/{label}"] = {
                "best_ms": _dur_ms(m.group(2), m.group(3)),
                "median_ms": _dur_ms(m.group(4), m.group(5)),
                "worst_ms": _dur_ms(m.group(6), m.group(7)),
            }
    m = re.search(r"proof bytes\s+(\d+)", text)
    proof_bytes = int(m.group(1)) if m else None
    m = re.search(r"lookup_used=(\d+), lookup_len=(\d+)", text)
    lookup = (int(m.group(1)), int(m.group(2))) if m else (None, None)
    return rows, proof_bytes, lookup


def run_fields_witch(binary, k, threads, reps, warmups, min_idle, log_inv_rate=1, scheme=None):
    schedule = README_2_20 if k == 20 else limb_schedule(k)
    cmd = [binary, "--limb-sizes", ",".join(map(str, schedule)), "--threads", str(threads),
           "--warmups", str(warmups), "--repeats", str(reps), "--allow-large"]
    if log_inv_rate != 1:
        cmd += ["--log-inv-rate", str(log_inv_rate)]
    idle = wait_for_quiet(min_idle)
    text, rss, wall = run_timed(cmd, cwd=os.path.dirname(binary))
    rows, proof_bytes, lookup = parse_fields_witch(text)
    med = lambda key: rows[key]["median_ms"] if key in rows else None
    return {
        "scheme": scheme or "fields-witch",
        "log_inv_rate": log_inv_rate,
        "pcs": next((line[5:] for line in text.splitlines() if line.startswith("pcs: ")), None),
        "ood_rounds_ms": med("prover/round 0"),
        **fields_witch_provenance(binary),
        "k": k, "n_bits_log2": k + 7, "threads": threads, "reps": reps, "warmups": warmups,
        "limb_sizes": schedule, "lookup_used": lookup[0], "lookup_len": lookup[1],
        "idle_before": idle, "wall_s": wall, "max_rss_bytes": rss,
        "prove_total_ms": med("prover/protocol total"),
        "claim_check_ms": med("prover/claim check"),
        "setup_ms": med("prover/setup wall"),
        "witness_ms": med("prover/witness branch"),
        "dense_commit_ms": med("prover/dense commits"),
        "aux_commit_ms": med("prover/auxiliary"),
        "core_ms": med("prover/core"),
        "main_loop_ms": med("prover/main loop"),
        "logup_ms": med("prover/Logup*"),
        "ringswitch_ms": med("prover/ringswitch"),
        "openings_ms": med("prover/openings"),
        "verify_total_ms": med("verifier/protocol total"),
        "verify_core_ms": med("verifier/core"),
        "verify_openings_ms": med("verifier/PCS openings"),
        "proof_bytes": proof_bytes,
        "cmd": " ".join(cmd),
        "raw": text,
    }


# --------------------------------------------------------------------------
# BitZ
# --------------------------------------------------------------------------

def parse_bitz_result(text):
    line = None
    for candidate in text.splitlines():
        if candidate.startswith("RESULT schema=bitz-cli/"):
            line = candidate
    if line is None:
        raise RuntimeError("no RESULT line in BitZ output:\n" + text[-3000:])
    fields = {}
    for token in line.split()[1:]:
        key, _, value = token.partition("=")
        if key == "ligerito_hex":
            continue
        fields[key] = value
    return fields


def _num(value):
    try:
        return int(value)
    except ValueError:
        try:
            return float(value)
        except ValueError:
            return value


def run_bitz(binary, n, threads, reps, min_idle, word_bits=1, profile=None):
    cmd = [binary, str(n), "--threads", str(threads), "--reps", str(reps)]
    if word_bits != 1:
        cmd += ["--word-bits", str(word_bits)]
    if profile:
        cmd += ["--profile", profile]
    idle = wait_for_quiet(min_idle)
    text, rss, wall = run_timed(cmd)
    f = {k: _num(v) for k, v in parse_bitz_result(text).items()}
    sec = None
    for line in text.splitlines():
        if line.startswith("security:"):
            sec = line
    bitz_commit = subprocess.run(["git", "describe", "--always", "--dirty", "--abbrev=9"],
                                capture_output=True, text=True).stdout.strip() or None
    return {
        "scheme": "bitz", "bitz_commit": bitz_commit, "bitz_binary_sha256": file_sha256(binary)[:16],
        "n": n, "word_bits": word_bits, "n_bits_log2": n + int(math.log2(word_bits)),
        "threads": threads, "reps": reps, "warmups": 1, "profile": f.get("lig"),
        "t": f.get("t"), "s": f.get("s"), "q_bits": f.get("q_bits"),
        "lig_achieved_bits": f.get("lig_achieved_bits"), "lig_hash": f.get("lig_hash"),
        "idle_before": idle, "wall_s": wall, "max_rss_bytes": rss,
        "commit_ms": f.get("commit_ms"), "prove_ms": f.get("prove_ms"),
        "prove_total_ms": (f.get("commit_ms") or 0) + (f.get("prove_ms") or 0),
        "prove_gp_ms": f.get("prove_gp_ms"), "prove_rs_ms": f.get("prove_rs_ms"),
        "prove_lig_ms": f.get("prove_lig_ms"), "prove_residual_ms": f.get("prove_residual_ms"),
        "commit_peak_mb": f.get("commit_peak_mb"), "prove_peak_mb": f.get("prove_peak_mb"),
        "verify_total_ms": f.get("verify_ms"),
        "proof_bytes": f.get("proof_bytes"), "proof_nonlig_bytes": f.get("proof_nonlig_bytes"),
        "proof_lig_bytes": f.get("proof_lig_bytes"),
        "security": sec,
        "cmd": " ".join(cmd),
        "raw": text,
    }


# --------------------------------------------------------------------------
# rendering
# --------------------------------------------------------------------------

def scheme_label(r):
    if r["scheme"] == "bitz":
        return f"BitZ {r.get('profile') or ''}".strip()
    rate = f"rate 1/{2 ** (r.get('log_inv_rate') or 1)}"
    if r["scheme"] == "fields-witch-asm":
        return f"fields-witch ({rate}, sha2 asm)"
    return f"fields-witch ({rate})"


def fmt_ms(v):
    if v is None:
        return "--"
    if v >= 1000:
        return f"{v / 1000:.2f} s"
    if v >= 100:
        return f"{v:.0f} ms"
    return f"{v:.1f} ms"


def fmt_kb(b):
    return "--" if b is None else f"{b / 1024:.0f} KB"


def fmt_mb(b):
    return "--" if b is None else f"{b / 2**20:.0f} MB"


def render_markdown(records, machine):
    fw = [r for r in records if r["scheme"] in ("fields-witch", "fields-witch-asm")]
    fz = [r for r in records if r["scheme"] == "bitz" and r["word_bits"] == 1]
    fzw = [r for r in records if r["scheme"] == "bitz" and r["word_bits"] != 1]
    threads = sorted({r["threads"] for r in records})
    out = []
    out.append(f"Machine: {machine}\n")
    out.append("## Bit-matched end-to-end (fields-witch 2^k x 127-bit entries vs BitZ n = k+7, W = 1)\n")
    out.append("Prover = everything from the data to the proof, commitment included "
               "(fields-witch `protocol total`; BitZ `commit + prove`). Medians; peak = process max RSS.\n")
    out.append("| bits | threads | scheme | shape | prover | verifier | proof | peak RSS |")
    out.append("|---|---|---|---|---|---|---|---|")
    for bits in sorted({r["n_bits_log2"] for r in fw} | {r["n_bits_log2"] for r in fz}):
        for th in threads:
            for r in fw:
                if r["n_bits_log2"] == bits and r["threads"] == th:
                    out.append(f"| 2^{bits} | {th} | {scheme_label(r)} | 2^{r['k']} x 127 b | {fmt_ms(r['prove_total_ms'])} | "
                               f"{fmt_ms(r['verify_total_ms'])} | {fmt_kb(r['proof_bytes'])} | {fmt_mb(r['max_rss_bytes'])} |")
            for r in fz:
                if r["n_bits_log2"] == bits and r["threads"] == th:
                    out.append(f"| 2^{bits} | {th} | {scheme_label(r)} | n={r['n']} (t={r['t']}, s={r['s']}), q {r['q_bits']} b | "
                               f"{fmt_ms(r['prove_total_ms'])} | {fmt_ms(r['verify_total_ms'])} | "
                               f"{fmt_kb(r['proof_bytes'])} | {fmt_mb(r['max_rss_bytes'])} |")
    out.append("")
    out.append("## Prover phase split\n")
    out.append("| bits | threads | scheme | commit | fold / core | opener | other |")
    out.append("|---|---|---|---|---|---|---|")
    for bits in sorted({r["n_bits_log2"] for r in fw} | {r["n_bits_log2"] for r in fz}):
        for th in threads:
            for r in fw:
                if r["n_bits_log2"] == bits and r["threads"] == th:
                    commit = (r["dense_commit_ms"] or 0) + (r["aux_commit_ms"] or 0)
                    other = (r["prove_total_ms"] or 0) - commit - (r["core_ms"] or 0) - (r["openings_ms"] or 0)
                    out.append(f"| 2^{bits} | {th} | {scheme_label(r)} | {fmt_ms(commit)} (dense {fmt_ms(r['dense_commit_ms'])} + aux {fmt_ms(r['aux_commit_ms'])}) | "
                               f"{fmt_ms(r['core_ms'])} (main loop {fmt_ms(r['main_loop_ms'])}, Logup* {fmt_ms(r['logup_ms'])}, ring switch {fmt_ms(r['ringswitch_ms'])}) | "
                               f"{fmt_ms(r['openings_ms'])} | {fmt_ms(other)} (witness {fmt_ms(r['witness_ms'])}, claim check {fmt_ms(r['claim_check_ms'])}"
                               + (f", round 0 {fmt_ms(r['ood_rounds_ms'])}" if r.get("ood_rounds_ms") else "") + ") |")
            for r in fz:
                if r["n_bits_log2"] == bits and r["threads"] == th:
                    out.append(f"| 2^{bits} | {th} | {scheme_label(r)} | {fmt_ms(r['commit_ms'])} | "
                               f"{fmt_ms(r['prove_gp_ms'])} (GKR forest) + ring switch {fmt_ms(r['prove_rs_ms'])} | "
                               f"{fmt_ms(r['prove_lig_ms'])} | {fmt_ms(r['prove_residual_ms'])} |")
    if fzw:
        out.append("")
        out.append("## BitZ word-cell rows (same entry count as fields-witch; the one-chunk rule shrinks q)\n")
        out.append("| entries | W | threads | shape | q bits | prover | verifier | proof | peak RSS |")
        out.append("|---|---|---|---|---|---|---|---|---|")
        for r in sorted(fzw, key=lambda r: (r["n"], r["word_bits"], r["threads"])):
            out.append(f"| 2^{r['n']} | {r['word_bits']} | {r['threads']} | t={r['t']}, s={r['s']} | {r['q_bits']} | "
                       f"{fmt_ms(r['prove_total_ms'])} | {fmt_ms(r['verify_total_ms'])} | {fmt_kb(r['proof_bytes'])} | {fmt_mb(r['max_rss_bytes'])} |")
    out.append("")
    return "\n".join(out)


def _fmt_ms_tex(v):
    if v is None:
        return "--"
    if v >= 1000:
        return f"{v:.0f}"
    if v >= 100:
        return f"{v:.0f}"
    if v >= 10:
        return f"{v:.1f}"
    return f"{v:.2f}"


def render_latex(records, machine, cmdline, threads=8, bitz_commit_override=None):
    """Paper-style table (same layout as the paper's native-multiplication
    table): rows grouped by committed bits, one row per scheme, the better
    value of each column in bold; KB = 1000 bytes, GB = 2^30 bytes."""
    fw = [r for r in records if r["scheme"] == "fields-witch" and r["threads"] == threads]
    fz = [r for r in records if r["scheme"] == "bitz" and r["word_bits"] == 1 and r["threads"] == threads]
    sizes = sorted({r["n_bits_log2"] for r in fw} & {r["n_bits_log2"] for r in fz})
    if not sizes:
        return "% no size has both a fields-witch and an BitZ row; nothing to tabulate\n"
    fw_commit = subprocess.run(["git", "-C", os.path.expanduser("~/fields-witch"), "rev-parse", "--short", "HEAD"],
                               capture_output=True, text=True).stdout.strip() or "?"
    bitz_commit = bitz_commit_override or next((r.get("bitz_commit") for r in fz if r.get("bitz_commit")), None) \
        or (subprocess.run(["git", "describe", "--always", "--dirty", "--abbrev=9"],
                           capture_output=True, text=True).stdout.strip() + " (tree at render time; the run predates provenance capture)")
    lines = []
    lines.append("% BitZ vs fields-witch table -- GENERATED FILE, do not edit by hand.")
    lines.append(f"% Generated by scripts/run_fields_witch_compare.py on {dt.datetime.utcnow():%Y-%m-%d} (UTC); BitZ at {bitz_commit},")
    lines.append(f"%   fields-witch (github.com/morgana-proofs/fields-witch) at {fw_commit}, built with RUSTFLAGS=\"-C target-cpu=native\".")
    lines.append(f"% Command: python3 {cmdline}")
    lines.append(f"% Machine: {machine}; {threads} rayon threads in both provers; one untimed warm-up, medians of the measured")
    lines.append("%   prove+verify passes; every measured proof is verified; one fresh process per cell under /usr/bin/time -l.")
    lines.append("% fields-witch: 2^k entries of F_{2^127} read as integers in [0, 2^127), committed densely over F_{2^128}, MLE")
    lines.append("%   evaluated over F_p with p = 2^127 - 1 (the README's limb schedule at 2^20; the README rule elsewhere); prover =")
    lines.append("%   its `protocol total` (commitments included). BitZ: n = k + 7 cells of W = 1 bit (the same 16 bytes per entry),")
    lines.append("%   profile custom:1:4; prover = commit_ms + prove_ms. Proof KB = 1000 bytes; peak mem. = process max RSS, GB = 2^30 bytes.")
    lines.append("% Include with \\input{fields-witch-table} (relative to paper/). Regenerate from a run directory with")
    lines.append("%   python3 scripts/run_fields_witch_compare.py --render-latex PerfRuns/<run>/results.jsonl --latex paper/fields-witch-table.tex")
    lines.append("\\begin{table}[H]")
    lines.append("  \\centering")
    lines.append("  \\small")
    lines.append("  \\setlength{\\tabcolsep}{4.5pt}")
    lines.append("  \\begin{tabular}{@{}rlrrrr@{}}")
    lines.append("    \\toprule")
    lines.append("    Committed bits & Scheme & Prover (ms) & Verifier (ms) & Proof (KB) & Peak mem.\\ (GB) \\\\")
    lines.append("    \\midrule")
    for i, bits in enumerate(sizes):
        a = next(r for r in fw if r["n_bits_log2"] == bits)
        b = next(r for r in fz if r["n_bits_log2"] == bits)
        cols = []
        for key, fmt in (("prove_total_ms", _fmt_ms_tex), ("verify_total_ms", _fmt_ms_tex),
                         ("proof_bytes", lambda v: f"{v / 1000:.0f}"), ("max_rss_bytes", lambda v: f"{v / 2**30:.2f}" if v / 2**30 < 10 else f"{v / 2**30:.1f}")):
            va, vb = a[key], b[key]
            ta, tb = fmt(va), fmt(vb)
            if va is not None and vb is not None:
                if va < vb:
                    ta = "\\textbf{" + ta + "}"
                elif vb < va:
                    tb = "\\textbf{" + tb + "}"
            cols.append((ta, tb))
        if i:
            lines.append("    \\addlinespace")
        lines.append(f"    $2^{{{bits}}}$ & fields-witch~\\cite{{lev}} ($2^{{{a['k']}}}$ entries) & " + " & ".join(c[0] for c in cols) + " \\\\")
        lines.append(f"      & \\ftwoz\\ (this work), $\\log \\codedim = {b['n']}$ & " + " & ".join(c[1] for c in cols) + " \\\\")
    lines.append("    \\bottomrule")
    lines.append("  \\end{tabular}")
    q_lo = min(r["q_bits"] for r in fz)
    q_hi = max(r["q_bits"] for r in fz)
    lines.append("  \\caption{Cost of opening a commitment of $2^{m}$ bits, $m = " + f"{sizes[0]}, \\ldots, {sizes[-1]}" + "$, with "
                 "fields-witch (Soukhanov's implementation of \\cite{lev}) and with \\ftwoz. fields-witch commits $2^{m-7}$ entries of "
                 "$\\FF_{2^{127}}$, read as integers in $[0, 2^{127})$, densely over $\\FF_{2^{128}}$ (16 bytes per entry) and proves the "
                 "evaluation of their multilinear extension at a random point of $\\FF_p$, $p = 2^{127} - 1$, with unique-decoding "
                 "Ligerito openings (rate $1/2$ at the first level, SHA-256 Merkle trees) at $100$ bits of security. \\ftwoz\\ commits "
                 "the same volume as $\\codedim = 2^{m}$ bits at cell width $W = 1$ and proves one claim $\\langle \\vv, \\bff\\rangle = \\mu$ "
                 "over $\\FF_q$ for a prime $q$ sampled after the commitment from $[2^{b-1}, 2^{b})$, $b = " + f"{q_lo}, \\ldots, {q_hi}" + "$, "
                 "exactly as in \\cref{tab:f2z-raw-performance} (rate $1/2$, Johnson regime, BLAKE3, $100$ bits). Prover time includes "
                 "the commitment in both cases; both provers use their own retained scratch memory; peak memory is the maximum resident "
                 "set of the whole process; KB $= 1000$ bytes, $1$\\,GB $= 2^{30}$ bytes. Apple M5 (10 cores: 4 performance + 6 "
                 "efficiency), 24\\,GB, " + f"{threads}" + " threads; medians of 5 runs after one warm-up.}")
    lines.append("  \\label{tab:fields-witch}")
    lines.append("\\end{table}")
    return "\n".join(lines) + "\n"




# --------------------------------------------------------------------------
# the fields-witch suite (Binius-suite shape: BitZ and fields-witch at two rates)
# --------------------------------------------------------------------------

SUITE_ROWS = [
    ("bitz", 1, "\\ftwoz-SNARK, $\\rho = 1/2$"),
    ("bitz", 3, "\\ftwoz-SNARK, $\\rho = 1/8$"),
    ("fields-witch", 1, "Fields-Witch, $\\rho = 1/2$"),
    ("fields-witch", 3, "Fields-Witch, $\\rho = 1/8$"),
]


def record_rate(r):
    """Level-0 inverse-rate exponent of a record: BitZ from its `custom:r:k`
    profile, fields-witch from its recorded `log_inv_rate`."""
    if r["scheme"] == "bitz":
        m = re.match(r"custom:(\d+):", r.get("profile") or "")
        return int(m.group(1)) if m else None
    return r.get("log_inv_rate") or 1


def _is_suite(records):
    rates = {record_rate(r) for r in records
             if r["scheme"] in ("fields-witch", "fields-witch-asm")}
    return len(rates) > 1


def _pick_renderer(records):
    if _is_suite(records):
        return render_latex_suite
    return render_latex


def render_latex_suite(records, machine, cmdline, threads=None, bitz_commit_override=None, caption_note=""):
    """SHA+ECDSA-style table of the fields-witch suite: \\ftwoz-SNARK and fields-witch
    at rates 1/2 and 1/8, grouped by committed bits
    and thread count (every thread count present; `threads` is ignored); bold = best row of
    a group per column. The native fields-witch rows are the sha2-asm build when present."""
    native = "fields-witch-asm" if any(r["scheme"] == "fields-witch-asm" for r in records) else "fields-witch"
    cell = {}
    for r in records:
        if r.get("word_bits", 1) != 1:
            continue
        if r["scheme"] in ("fields-witch", "fields-witch-asm"):
            if r["scheme"] != native:
                continue
            scheme = "fields-witch"
        else:
            scheme = r["scheme"]
        cell[(scheme, record_rate(r), r["n_bits_log2"], r["threads"])] = r
    sizes = sorted({key[2] for key in cell})
    if not sizes:
        return "% no suite rows; nothing to tabulate\n"
    thread_counts = sorted({key[3] for key in cell})
    fz = [r for key, r in cell.items() if key[0] == "bitz"]
    fw_all = [r for key, r in cell.items() if key[0] != "bitz"]
    bitz_commit = bitz_commit_override or next((r.get("bitz_commit") for r in fz if r.get("bitz_commit")), "?")
    lines = []
    lines.append("% fields-witch suite: BitZ vs fields-witch (sha2 asm build), rates 1/2 and 1/8 -- GENERATED FILE, do not edit by hand.")
    lines.append(f"% Generated by scripts/run_fields_witch_compare.py on {dt.datetime.utcnow():%Y-%m-%d} (UTC); BitZ at {bitz_commit} "
                 f"(binaries {sorted({r.get('bitz_binary_sha256') for r in fz})}, profiles {sorted({r.get('profile') for r in fz})}).")
    for prov in sorted({f"{r['scheme']}: {r.get('fw_head')} tree {r.get('fw_tree_sha256')} binary {r.get('fw_binary_sha256')}" for r in fw_all}):
        lines.append(f"% fields-witch (github.com/morgana-proofs/fields-witch @ 30cca8c + local branch) {prov}")
    lines.append(f"% Command: python3 {cmdline}")
    lines.append(f"% Machine: {machine}; one untimed warm-up, medians of the measured prove+verify passes; every measured proof")
    lines.append("%   is verified; one fresh process per cell under /usr/bin/time -l. Prover = commitment + proof generation;")
    lines.append("%   proof KB = 1000 bytes; peak mem. = process max RSS, GB = 2^30 bytes. Bold = best row of the (bits, threads) group.")
    lines.append("% Include with \\input{fields-witch-table} (relative to paper/). Regenerate from a run directory with")
    lines.append("%   python3 scripts/run_fields_witch_compare.py --render-latex PerfRuns/<run>/results.jsonl --latex paper/fields-witch-table.tex")
    lines.append("% Medians as recorded (ms unless noted):")
    for bits in sizes:
        for th in thread_counts:
            for scheme, rate, _ in SUITE_ROWS:
                r = cell.get((scheme, rate, bits, th))
                if r:
                    lines.append(f"%   2^{bits} threads={th} {r['scheme']}@{rate} prove={r['prove_total_ms']:.3f} verify={r['verify_total_ms']:.3f} "
                                 f"proof_bytes={r['proof_bytes']} peak_rss={r['max_rss_bytes']} idle_before={r.get('idle_before')}")
    lines.append("\\begin{table}[H]")
    lines.append("  \\centering")
    lines.append("  \\small")
    lines.append("  \\setlength{\\tabcolsep}{4.5pt}")
    lines.append("  \\begin{tabular}{@{}rrlrrrr@{}}")
    lines.append("    \\toprule")
    lines.append("    Committed bits & Threads & Scheme & Prover (ms) & Verifier (ms) & Proof (KB) & Peak mem.\\ (GB) \\\\")
    lines.append("    \\midrule")
    fmts = (("prove_total_ms", _fmt_ms_tex), ("verify_total_ms", _fmt_ms_tex),
            ("proof_bytes", lambda v: f"{v / 1000:.0f}"),
            ("max_rss_bytes", lambda v: f"{v / 2**30:.2f}" if v / 2**30 < 10 else f"{v / 2**30:.1f}"))
    first_group = True
    for bits in sizes:
        for gi, th in enumerate(thread_counts):
            group = [(label, cell.get((scheme, rate, bits, th))) for scheme, rate, label in SUITE_ROWS]
            group = [(label, r) for label, r in group if r]
            if not group:
                continue
            best = {}
            for key, _ in fmts:
                values = [r[key] for _, r in group if r[key] is not None]
                best[key] = min(values) if values else None
            if not first_group:
                lines.append("    \\addlinespace")
            first_group = False
            for ri, (label, r) in enumerate(group):
                cells = []
                for key, fmt in fmts:
                    text = fmt(r[key]) if r[key] is not None else "--"
                    if r[key] is not None and r[key] == best[key]:
                        text = "\\textbf{" + text + "}"
                    cells.append(text)
                size_cell = f"$2^{{{bits}}}$" if (gi == 0 and ri == 0) else ""
                thread_cell = f"{th}" if ri == 0 else ""
                lines.append(f"    {size_cell} & {thread_cell} & {label} & " + " & ".join(cells) + " \\\\")
    lines.append("    \\bottomrule")
    lines.append("  \\end{tabular}")
    q_bits = [r["q_bits"] for r in fz if r.get("q_bits")]
    q_clause = f", $b = {min(q_bits)}, \\ldots, {max(q_bits)}$" if q_bits else ""
    size_list = ", ".join(str(b) for b in sizes)
    caption = (
        f"Cost of opening a commitment of $2^{{m}}$ bits, $m \\in \\{{{size_list}\\}}$, at Reed--Solomon rates $\\rho = 1/2$ and $\\rho = 1/8$. "
        "\\ftwoz-SNARK commits the $2^{m}$ bits at cell width $W = 1$ and proves one claim over $\\FF_q$ for a prime $q$ sampled after the "
        f"commitment from $[2^{{b-1}}, 2^{{b}})${q_clause} (round-by-round economic security model, every error term at most $2^{{-100}}$; "
        "Johnson regime, early Round-0 OOD, fold and query grinding, BLAKE3 Merkle trees). Fields-Witch (Soukhanov's implementation of "
        "\\cite{lev}) commits $2^{m-7}$ entries of $\\FF_{2^{127}}$, read as integers in $[0, 2^{127})$, densely over $\\FF_{2^{128}}$ and "
        "proves the evaluation of their multilinear extension at a random point of $\\FF_p$, $p = 2^{127} - 1$, with three Ligerito openings "
        "in the unique-decoding regime ($100$-bit query target, SHA-256 Merkle trees on the \\texttt{sha2} crate's assembly backend) whose "
        "first level runs at rate $\\rho$. Prover time includes the commitment; peak memory is the maximum resident set of the "
        "whole process; KB $= 1000$ bytes, $1$\\,GB $= 2^{30}$ bytes. Bold: best row of the size and thread count. Apple M5 (10 cores: "
        "4 performance + 6 efficiency), 24\\,GB; medians of 5 runs after one warm-up."
    )
    if caption_note:
        caption += " " + caption_note
    lines.append("  \\caption{" + caption + "}")
    lines.append("  \\label{tab:fields-witch}")
    lines.append("\\end{table}")
    return "\n".join(lines) + "\n"


def machine_string():
    try:
        brand = subprocess.run(["sysctl", "-n", "machdep.cpu.brand_string"], capture_output=True, text=True).stdout.strip()
        mem = int(subprocess.run(["sysctl", "-n", "hw.memsize"], capture_output=True, text=True).stdout.strip())
        cores = subprocess.run(["sysctl", "-n", "hw.ncpu"], capture_output=True, text=True).stdout.strip()
        return f"{brand}, {mem // 2**30} GB, {cores} cores, {platform.platform()}"
    except Exception:
        return platform.platform()


# --------------------------------------------------------------------------
# main
# --------------------------------------------------------------------------

def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    home = os.path.expanduser("~")
    default_bitz = os.path.join(os.environ.get("CARGO_TARGET_DIR", os.path.join(os.getcwd(), "target")), "release", "bitz")
    ap.add_argument("--fw-bin", default=os.path.join(home, "fields-witch", "target", "release", "examples", "protocol_profile"))
    ap.add_argument("--bitz-bin", default=default_bitz)
    ap.add_argument("--sizes", default="14,16,18,20,22", help="fields-witch log2 entry counts k; BitZ runs n = k + 7")
    ap.add_argument("--threads", default="1,8")
    ap.add_argument("--reps", type=int, default=3)
    ap.add_argument("--warmups", type=int, default=1)
    ap.add_argument("--min-idle", type=float, default=90.0)
    ap.add_argument("--cooldown", type=float, default=3.0, help="seconds between processes")
    ap.add_argument("--word-rows", default="", help="extra BitZ rows `n:W,...` (e.g. 20:32,20:64)")
    ap.add_argument("--bitz-profile", default=None, help="override the BitZ opener profile (e.g. udr:1:4)")
    ap.add_argument("--schemes", default="fields-witch,bitz",
                    help="comma-separated: fields-witch (default build), fields-witch-asm (sha2 asm build), bitz")
    ap.add_argument("--fw-log-inv-rate", type=int, default=1,
                    help="commitment inverse-rate exponent of every fields-witch Ligerito PCS (values != 1 need the patched binary)")
    ap.add_argument("--fw-asm-bin", default=os.path.join(home, "fields-witch-f2z", "target-asm", "release", "examples", "protocol_profile"),
                    help="scheme fields-witch-asm: fields-witch built with --features sha2-asm (hardware SHA-256 Merkle trees)")
    ap.add_argument("--rates", default=None,
                    help="comma-separated inverse-rate exponents swept inside one campaign (e.g. 1,3): every fields-witch "
                         "scheme runs --log-inv-rate r and BitZ runs custom:r:4; overrides --fw-log-inv-rate and --bitz-profile")
    ap.add_argument("--out-dir", default=None)
    ap.add_argument("--latex", default=None, help="write the LaTeX table here (default <out-dir>/fields-witch-table.tex)")
    ap.add_argument("--render", default=None, help="re-render an existing results.jsonl as markdown")
    ap.add_argument("--render-latex", default=None, help="write the paper table from an existing results.jsonl to --latex")
    ap.add_argument("--latex-threads", type=int, default=8, help="thread count of the rows in the LaTeX table")
    ap.add_argument("--bitz-commit", default=None, help="BitZ revision to record in the LaTeX header (default: the run's recorded revision)")
    args = ap.parse_args()

    machine = machine_string()
    if args.render:
        with open(args.render) as fh:
            records = [json.loads(line) for line in fh if line.strip()]
        print(render_markdown(records, machine))
        return
    if args.render_latex:
        with open(args.render_latex) as fh:
            records = [json.loads(line) for line in fh if line.strip()]
        cmd = next((r["cmd"] for r in records if r.get("cmd")), "")
        first = json.loads(open(args.render_latex).readline())
        tex = _pick_renderer(records)(records, machine, "scripts/run_fields_witch_compare.py " + (open(os.path.join(os.path.dirname(args.render_latex), "summary.md")).readline().split("`")[1].split(" ", 1)[1] if os.path.exists(os.path.join(os.path.dirname(args.render_latex), "summary.md")) else ""), args.latex_threads, args.bitz_commit)
        path = args.latex or "paper/fields-witch-table.tex"
        with open(path, "w") as fh:
            fh.write(tex)
        print(f"wrote {path}")
        return

    stamp = dt.datetime.utcnow().strftime("%Y-%m-%dT%H-%M-%SZ")
    out_dir = args.out_dir or os.path.join("PerfRuns", f"{stamp}-fields-witch-compare")
    os.makedirs(out_dir, exist_ok=False)
    jsonl = os.path.join(out_dir, "results.jsonl")
    sizes = [int(x) for x in args.sizes.split(",") if x]
    threads = [int(x) for x in args.threads.split(",") if x]
    schemes = args.schemes.split(",")
    word_rows = [tuple(int(v) for v in item.split(":")) for item in args.word_rows.split(",") if item]
    cmdline = " ".join(sys.argv)
    print(f"run dir: {out_dir}\nmachine: {machine}\nfields-witch: {args.fw_bin}\nbitz: {args.bitz_bin}", flush=True)

    records = []

    def emit(rec):
        records.append(rec)
        with open(jsonl, "a") as fh:
            fh.write(json.dumps(rec) + "\n")
        brief = {k: v for k, v in rec.items() if k not in ("raw", "cmd", "security")}
        print(json.dumps(brief), flush=True)
        time.sleep(args.cooldown)

    rates = [int(x) for x in args.rates.split(",") if x] if args.rates else [None]
    for k in sizes:
        for th in threads:
            for rate in rates:
                fw_rate = args.fw_log_inv_rate if rate is None else rate
                bitz_profile = args.bitz_profile if rate is None else f"custom:{rate}:4"
                if "bitz" in schemes:
                    emit(run_bitz(args.bitz_bin, k + 7, th, args.reps, args.min_idle, profile=bitz_profile))
                if "fields-witch" in schemes:
                    emit(run_fields_witch(args.fw_bin, k, th, args.reps, args.warmups, args.min_idle,
                                          log_inv_rate=fw_rate))
                if "fields-witch-asm" in schemes:
                    emit(run_fields_witch(args.fw_asm_bin, k, th, args.reps, args.warmups, args.min_idle,
                                          log_inv_rate=fw_rate, scheme="fields-witch-asm"))
    for (n, w) in word_rows:
        for th in threads:
            emit(run_bitz(args.bitz_bin, n, th, args.reps, args.min_idle, word_bits=w, profile=args.bitz_profile))

    md = render_markdown(records, machine)
    with open(os.path.join(out_dir, "summary.md"), "w") as fh:
        fh.write(f"Command: `{cmdline}`\n\n" + md)
    tex = _pick_renderer(records)(records, machine, cmdline, args.latex_threads, args.bitz_commit)
    latex_path = args.latex or os.path.join(out_dir, "fields-witch-table.tex")
    with open(latex_path, "w") as fh:
        fh.write(tex)
    print("\n" + md)
    print(f"wrote {jsonl}, {os.path.join(out_dir, 'summary.md')}, {latex_path}")


if __name__ == "__main__":
    main()
