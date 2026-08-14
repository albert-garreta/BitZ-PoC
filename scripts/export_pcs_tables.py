#!/usr/bin/env python3
"""Validate a PCS profile sweep and export CSV and LaTeX tables.

The input files are the per-shape CSVs written by ``scripts/bench_csv.py``.
Headline and breakdown measurements are deliberately separate: the former is
captured without ``OBLONG_PROFILE`` while the latter is a tagged diagnostic
run whose phase times must not be subtracted from the headline medians.
Dashboard JSON is available only through the explicit ``--with-dashboard-data``
option.
"""

from __future__ import annotations

import argparse
import csv
import json
import math
import re
import sys
from pathlib import Path
from typing import Iterable


EXPECTED_SHAPES = {
    20: (13, 7, 1),
    21: (13, 8, 1),
    22: (14, 8, 1),
    23: (14, 9, 1),
    24: (15, 9, 1),
    25: (15, 10, 1),
    26: (16, 10, 1),
    27: (16, 11, 1),
    28: (17, 11, 1),
    29: (17, 12, 1),
    30: (18, 12, 1),
}
GEOMETRY_RE = re.compile(r"^(?P<tag>.+)@r1/(?P<rate_den>[0-9]+)k(?P<k>[0-9]+)$")
DETAIL_KEYS = (
    "pack_ms",
    "pow2_ms",
    "forest_core_ms",
    "fold_v_ms",
    "presum_tables_ms",
    "presum_run_ms",
    "rings_ms",
    "basis_combine_ms",
    "ligerito_recursive_ms",
)


class InputError(ValueError):
    """A benchmark artifact is incomplete or violates the series contract."""


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--headline",
        type=Path,
        nargs="+",
        help="headline CSV file(s) or directories containing per-shape CSVs",
    )
    parser.add_argument(
        "--breakdown",
        type=Path,
        nargs="+",
        help="profiled CSV file(s) or directories containing per-shape CSVs",
    )
    parser.add_argument("--out-dir", type=Path)
    parser.add_argument(
        "--expected-profile",
        help="optional caller-supplied guard for the requested Ligerito profile",
    )
    parser.add_argument(
        "--force",
        action="store_true",
        help="replace only this script's known generated report files",
    )
    parser.add_argument(
        "--with-dashboard-data",
        action="store_true",
        help="also write dashboard-data.json for an explicit dashboard build",
    )
    return parser.parse_args()


def expand_inputs(paths: Iterable[Path]) -> list[Path]:
    files: list[Path] = []
    for path in paths:
        if path.is_dir():
            files.extend(sorted(path.glob("*.csv")))
        elif path.is_file():
            files.append(path)
        else:
            raise InputError(f"input does not exist: {path}")
    if not files:
        raise InputError("no input CSV files found")
    return files


def read_rows(paths: Iterable[Path], kind: str) -> dict[int, dict[str, str]]:
    rows: dict[int, dict[str, str]] = {}
    for path in expand_inputs(paths):
        with path.open(newline="", encoding="utf-8") as handle:
            reader = csv.DictReader(handle)
            if reader.fieldnames is None:
                raise InputError(f"{kind}: missing CSV header in {path}")
            for line_number, row in enumerate(reader, start=2):
                try:
                    exponent = int(row["n"])
                except (KeyError, TypeError, ValueError) as exc:
                    raise InputError(
                        f"{kind}: invalid n at {path}:{line_number}"
                    ) from exc
                if exponent in rows:
                    raise InputError(
                        f"{kind}: duplicate n={exponent} in {path}; "
                        "keep exactly one completed part per exponent"
                    )
                row["_source"] = str(path)
                rows[exponent] = row
    missing = sorted(set(EXPECTED_SHAPES) - set(rows))
    extra = sorted(set(rows) - set(EXPECTED_SHAPES))
    if missing or extra:
        raise InputError(f"{kind}: exponent mismatch; missing={missing}, extra={extra}")
    return rows


def integer(row: dict[str, str], key: str, kind: str, exponent: int) -> int:
    try:
        return int(row[key])
    except (KeyError, TypeError, ValueError) as exc:
        raise InputError(f"{kind}: n={exponent} has invalid {key!r}") from exc


def number(row: dict[str, str], key: str, kind: str, exponent: int) -> float:
    try:
        value = float(row[key])
    except (KeyError, TypeError, ValueError) as exc:
        raise InputError(f"{kind}: n={exponent} has invalid {key!r}") from exc
    if not math.isfinite(value) or value < 0:
        raise InputError(f"{kind}: n={exponent} has non-finite/negative {key!r}")
    return value


def validate_geometry(geometry: str, exponent: int, kind: str) -> str:
    match = GEOMETRY_RE.fullmatch(geometry)
    if match is None:
        raise InputError(f"{kind}: n={exponent} has malformed lig_geometry={geometry!r}")
    rate_den = int(match.group("rate_den"))
    initial_k = int(match.group("k"))
    if rate_den <= 0 or initial_k <= 0:
        raise InputError(f"{kind}: n={exponent} has invalid lig_geometry={geometry!r}")
    return f"{match.group('tag')} (rate 1/{rate_den}, k={initial_k})"


def validate_row(
    row: dict[str, str], exponent: int, kind: str
) -> tuple[str, str, str]:
    expected_t, expected_s, expected_w = EXPECTED_SHAPES[exponent]
    actual = (
        integer(row, "t", kind, exponent),
        integer(row, "s", kind, exponent),
        integer(row, "W", kind, exponent),
    )
    if actual != (expected_t, expected_s, expected_w):
        raise InputError(
            f"{kind}: n={exponent} shape {actual} != "
            f"{(expected_t, expected_s, expected_w)}"
        )
    requested = row.get("profile_arg", "").strip()
    if not requested or "," in requested:
        raise InputError(
            f"{kind}: n={exponent} must contain one non-empty profile_arg"
        )
    resolved = row.get("lig_geometry", "").strip()
    return requested, resolved, validate_geometry(resolved, exponent, kind)


def normalized_headline(rows: dict[int, dict[str, str]]) -> list[dict[str, object]]:
    output: list[dict[str, object]] = []
    for exponent, row in sorted(rows.items()):
        requested, resolved, resolved_label = validate_row(row, exponent, "headline")
        proof_bytes = integer(row, "proof_bytes", "headline", exponent)
        if proof_bytes <= 0:
            raise InputError(f"headline: n={exponent} has non-positive proof_bytes")
        if any(row.get(key, "") for key in ("forest_ms", "open_ms", *DETAIL_KEYS)):
            raise InputError(
                f"headline: n={exponent} contains profiler fields; "
                "capture it without OBLONG_PROFILE"
            )
        output.append(
            {
                "i": exponent,
                "witness_bits": 1 << exponent,
                "t": integer(row, "t", "headline", exponent),
                "s": integer(row, "s", "headline", exponent),
                "word_bits": integer(row, "W", "headline", exponent),
                "requested_ligerito": requested,
                "resolved_ligerito": resolved,
                "resolved_ligerito_label": resolved_label,
                "threads": row.get("threads", ""),
                "reps": integer(row, "reps", "headline", exponent),
                "prover_ms": number(row, "prove_ms", "headline", exponent),
                "verifier_ms": number(row, "verify_ms", "headline", exponent),
                "proof_bytes": proof_bytes,
                "proof_kib": proof_bytes / 1024.0,
                # bench_csv's *_mb columns divide bytes by 1024^2: they are MiB.
                "prover_peak_heap_mib": number(
                    row, "prove_peak_mb", "headline", exponent
                ),
                "commit_ms": number(row, "commit_ms", "headline", exponent),
                "commit_peak_heap_mib": number(
                    row, "commit_peak_mb", "headline", exponent
                ),
                "timestamp": row.get("timestamp", ""),
                "source": row["_source"],
            }
        )
    reps = {int(row["reps"]) for row in output}
    threads = {str(row["threads"]) for row in output}
    if len(reps) != 1 or next(iter(reps)) <= 0:
        raise InputError(f"headline: inconsistent repetition counts: {sorted(reps)}")
    if len(threads) != 1 or not next(iter(threads)):
        raise InputError(f"headline: inconsistent thread counts: {sorted(threads)}")
    profiles = {str(row["requested_ligerito"]) for row in output}
    if len(profiles) != 1:
        raise InputError(f"headline: inconsistent requested profiles: {sorted(profiles)}")
    return output


def normalized_breakdown(
    rows: dict[int, dict[str, str]],
    headline: list[dict[str, object]],
) -> list[dict[str, object]]:
    headline_by_i = {int(row["i"]): row for row in headline}
    output: list[dict[str, object]] = []
    for exponent, row in sorted(rows.items()):
        requested, resolved, resolved_label = validate_row(
            row, exponent, "breakdown"
        )
        if requested != headline_by_i[exponent]["requested_ligerito"]:
            raise InputError(f"n={exponent}: headline/breakdown profile differs")
        if resolved != headline_by_i[exponent]["resolved_ligerito"]:
            raise InputError(f"n={exponent}: headline/breakdown geometry differs")
        if integer(row, "reps", "breakdown", exponent) != 1:
            raise InputError(f"breakdown: n={exponent} must use exactly one timed rep")
        if row.get("threads", "") != str(headline_by_i[exponent]["threads"]):
            raise InputError(f"n={exponent}: headline/breakdown thread count differs")
        forest = number(row, "forest_ms", "breakdown", exponent)
        opening = number(row, "open_ms", "breakdown", exponent)
        untagged = number(row, "untagged_ms", "breakdown", exponent)
        profiled_total = number(row, "profiled_prove_ms", "breakdown", exponent)
        detail = {
            key: number(row, key, "breakdown", exponent) for key in DETAIL_KEYS
        }
        forest_detail = sum(detail[key] for key in DETAIL_KEYS[:6])
        opening_detail = sum(detail[key] for key in DETAIL_KEYS[6:])
        tolerance_ms = 0.035
        if abs(forest_detail - forest) > tolerance_ms:
            raise InputError(
                f"breakdown: n={exponent} detailed forest phases sum to "
                f"{forest_detail:.6f} ms, coarse value is {forest:.6f} ms"
            )
        if abs(opening_detail - opening) > tolerance_ms:
            raise InputError(
                f"breakdown: n={exponent} detailed opening phases sum to "
                f"{opening_detail:.6f} ms, coarse value is {opening:.6f} ms"
            )
        tagged_total = forest + opening
        if tagged_total <= 0:
            raise InputError(f"breakdown: n={exponent} has no tagged time")
        if abs(tagged_total + untagged - profiled_total) > tolerance_ms:
            raise InputError(
                f"breakdown: n={exponent} tagged + untagged is "
                f"{tagged_total + untagged:.6f} ms, profiled total is "
                f"{profiled_total:.6f} ms"
            )
        output.append(
            {
                "i": exponent,
                "witness_bits": 1 << exponent,
                "requested_ligerito": requested,
                "resolved_ligerito": resolved,
                "resolved_ligerito_label": resolved_label,
                "threads": row.get("threads", ""),
                "profiled_runs": 1,
                **detail,
                "forest_presum_ms": forest,
                "ligerito_open_ms": opening,
                "untagged_ms": untagged,
                "profiled_prove_ms": profiled_total,
                "tagged_total_ms": tagged_total,
                "forest_share_pct": forest / tagged_total * 100.0,
                "open_share_pct": opening / tagged_total * 100.0,
                "timestamp": row.get("timestamp", ""),
                "source": row["_source"],
            }
        )
    return output


def write_csv(path: Path, rows: list[dict[str, object]]) -> None:
    with path.open("w", newline="", encoding="utf-8") as handle:
        writer = csv.DictWriter(handle, fieldnames=list(rows[0]))
        writer.writeheader()
        writer.writerows(rows)


def tex_escape(value: object) -> str:
    replacements = {
        "\\": r"\textbackslash{}",
        "&": r"\&",
        "%": r"\%",
        "$": r"\$",
        "#": r"\#",
        "_": r"\_",
        "{": r"\{",
        "}": r"\}",
        "~": r"\textasciitilde{}",
        "^": r"\textasciicircum{}",
    }
    return "".join(replacements.get(char, char) for char in str(value))


def label_slug(profile: str) -> str:
    slug = re.sub(r"[^a-z0-9]+", "-", profile.lower()).strip("-")
    return slug or "profile"


def headline_tex(rows: list[dict[str, object]], profile: str) -> str:
    profile_tex = tex_escape(profile)
    slug = label_slug(profile)
    lines = [
        r"\begin{table}[t]",
        r"\centering",
        rf"\caption{{F2Z PCS headline measurements with requested "
        rf"\texttt{{{profile_tex}}} Ligerito configuration. PCS opening/proving "
        r"(commitment excluded) and verifier times are medians; peak is the "
        r"tracked Rust-heap high-water mark "
        r"during a separate proving run.}",
        rf"\label{{tab:f2z-pcs-{slug}-headline}}",
        r"\begin{tabular}{r r r r r l}",
        r"\hline",
        r"$i$ & PCS prove (ms) & Verifier (ms) & Proof (KiB) & Peak heap (MiB) & Resolved Ligerito \\",
        r"\hline",
    ]
    for row in rows:
        resolved = tex_escape(row["resolved_ligerito_label"])
        lines.append(
            f'{row["i"]} & {row["prover_ms"]:.2f} & {row["verifier_ms"]:.2f} '
            f'& {row["proof_kib"]:.1f} & {row["prover_peak_heap_mib"]:.2f} '
            f'& {resolved} \\\\'
        )
    lines.extend(
        [
            r"\hline",
            r"\end{tabular}",
            r"\par\smallskip",
            r"{\footnotesize Resolved geometry is emitted by the Rust benchmark "
            r"and reconciled across the clean and profiled passes at every size. "
            r"Proof sizes are exact in the CSV.}",
            r"\end{table}",
            "",
        ]
    )
    return "\n".join(lines)


def breakdown_tex(rows: list[dict[str, object]], profile: str) -> str:
    profile_tex = tex_escape(profile)
    slug = label_slug(profile)
    lines = [
        r"\begin{table}[t]",
        r"\centering",
        rf"\caption{{Prover-time cost breakdown for F2Z PCS with requested "
        rf"\texttt{{{profile_tex}}} Ligerito configuration. Each row is "
        r"one separately profiled prove.}",
        rf"\label{{tab:f2z-pcs-{slug}-prover-breakdown}}",
        r"\scriptsize",
        r"\setlength{\tabcolsep}{3pt}",
        r"\begin{tabular}{r r r r r r r r r r}",
        r"\hline",
        r"$i$ & Pow2 & Forest & Fold & Pre-SC & Ring & Basis & Lig. & Other & Total \\",
        r" & \multicolumn{9}{c}{milliseconds} \\",
        r"\hline",
    ]
    for row in rows:
        pre_sc = row["presum_tables_ms"] + row["presum_run_ms"]
        lines.append(
            f'{row["i"]} & {row["pow2_ms"]:.2f} '
            f'& {row["forest_core_ms"]:.2f} & {row["fold_v_ms"]:.2f} '
            f'& {pre_sc:.2f} & {row["rings_ms"]:.2f} '
            f'& {row["basis_combine_ms"]:.2f} '
            f'& {row["ligerito_recursive_ms"]:.2f} '
            f'& {row["untagged_ms"]:.2f} '
            f'& {row["profiled_prove_ms"]:.2f} \\\\'
        )
    lines.extend(
        [
            r"\hline",
            r"\end{tabular}",
            r"\par\smallskip",
            r"{\footnotesize The commitment's already-packed columns make the "
            r"packing scope zero; it is preserved in the CSV but omitted here. "
            r"Pre-SC combines table construction and the pre-sumcheck. Other is "
            r"the profiled-prove total minus all disjoint tagged scopes. This "
            r"diagnostic run is independent of the headline median.}",
            r"\end{table}",
            "",
        ]
    )
    return "\n".join(lines)


def dashboard_payload(
    headline: list[dict[str, object]], breakdown: list[dict[str, object]]
) -> dict[str, object]:
    profile = str(headline[0]["requested_ligerito"])
    breakdown_by_i = {int(row["i"]): row for row in breakdown}
    points = []
    for row in headline:
        exponent = int(row["i"])
        points.append({**row, "breakdown": breakdown_by_i[exponent]})
    return {
        "schema_version": 3,
        "algorithm": f"F2Z PCS — requested {profile} Ligerito",
        "series": {
            "requested_config": profile,
            "exponents": [20, 30],
            "word_bits": 1,
            "measurement_boundary": "commit external; prover is PCS opening/proving",
            "peak_definition": (
                "absolute high-water of outstanding Rust heap bytes during one "
                "separate prove, including the live commitment hint; MiB = 2^20 bytes"
            ),
            "small_instance_derivation": (
                "the benchmark resolves and records the selected profile at each "
                "size; custom Johnson/OOD parameters are mechanically derived and "
                "validator-gated"
            ),
            "breakdown_boundary": (
                "one separately profiled prove; disjoint tagged phases plus "
                "measured untagged remainder, not the headline timing sample"
            ),
        },
        "points": points,
    }


def export_reports(
    headline_inputs: list[Path],
    breakdown_inputs: list[Path],
    out_dir: Path,
    *,
    expected_profile: str | None = None,
    force: bool = False,
    with_dashboard_data: bool = False,
) -> str:
    """Validate one complete series and write its known report artifacts."""

    headline_rows = normalized_headline(read_rows(headline_inputs, "headline"))
    breakdown_rows = normalized_breakdown(
        read_rows(breakdown_inputs, "breakdown"), headline_rows
    )
    profile = str(headline_rows[0]["requested_ligerito"])
    if expected_profile is not None and profile != expected_profile:
        raise InputError(
            f"captured profile {profile!r} != expected {expected_profile!r}"
        )
    outputs = {
        "headline.csv",
        "headline.tex",
        "prover-breakdown.csv",
        "prover-breakdown.tex",
    }
    if with_dashboard_data:
        outputs.add("dashboard-data.json")
    out_dir.mkdir(parents=True, exist_ok=True)
    existing = [out_dir / name for name in outputs if (out_dir / name).exists()]
    if existing and not force:
        joined = ", ".join(str(path) for path in sorted(existing))
        raise InputError(f"refusing to overwrite {joined}; pass --force")

    write_csv(out_dir / "headline.csv", headline_rows)
    write_csv(out_dir / "prover-breakdown.csv", breakdown_rows)
    (out_dir / "headline.tex").write_text(
        headline_tex(headline_rows, profile), encoding="utf-8"
    )
    (out_dir / "prover-breakdown.tex").write_text(
        breakdown_tex(breakdown_rows, profile), encoding="utf-8"
    )
    if with_dashboard_data:
        (out_dir / "dashboard-data.json").write_text(
            json.dumps(dashboard_payload(headline_rows, breakdown_rows), indent=2)
            + "\n",
            encoding="utf-8",
        )
    return profile


def main() -> int:
    args = parse_args()
    try:
        if args.headline is None or args.breakdown is None or args.out_dir is None:
            raise InputError(
                "--headline, --breakdown, and --out-dir are required for export"
            )
        profile = export_reports(
            args.headline,
            args.breakdown,
            args.out_dir,
            expected_profile=args.expected_profile,
            force=args.force,
            with_dashboard_data=args.with_dashboard_data,
        )
    except InputError as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 2

    print(f"validated i=20..30 {profile} series; wrote {args.out_dir}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
