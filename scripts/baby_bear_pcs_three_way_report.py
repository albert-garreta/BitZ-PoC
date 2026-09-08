#!/usr/bin/env python3
"""Combine matched F2Z, WHIR degree-4, and WHIR degree-5 PCS campaigns."""

from __future__ import annotations

import argparse
import csv
import html
import json
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Sequence


EXPONENTS = tuple(range(16, 25))
PROFILES = (
    ("f2z", "F2Z / Ligerito"),
    ("whir-d4", "WHIR degree-4 / unique decoding"),
    ("whir-d5", "WHIR degree-5 / Johnson bound"),
)


class ReportError(Exception):
    pass


def parse_args(argv: Sequence[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--primary-summary", type=Path, required=True)
    parser.add_argument("--primary-trace", type=Path, required=True)
    parser.add_argument("--degree4-summary", type=Path, required=True)
    parser.add_argument("--degree4-trace", type=Path, required=True)
    parser.add_argument("--out-dir", type=Path, required=True)
    parser.add_argument("--force", action="store_true")
    return parser.parse_args(argv)


def load_json(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ReportError(f"cannot read {path}: {error}") from error
    if not isinstance(value, dict):
        raise ReportError(f"{path} must contain a JSON object")
    return value


def load_security(trace: Path) -> dict[tuple[str, int], dict[str, Any]]:
    found: dict[tuple[str, int], dict[str, Any]] = {}
    try:
        lines = trace.open(encoding="utf-8")
    except OSError as error:
        raise ReportError(f"cannot read {trace}: {error}") from error
    with lines:
        for line_number, line in enumerate(lines, 1):
            if '"record":"run"' not in line:
                continue
            try:
                record = json.loads(line)
            except json.JSONDecodeError as error:
                raise ReportError(f"{trace}:{line_number}: {error}") from error
            if record.get("record") != "run" or record.get("status") != "ok":
                continue
            implementation = record.get("benchmark", {}).get("implementation")
            parameters = record.get("parameters", {})
            exponent = parameters.get("input", {}).get("log_multiplications")
            security = parameters.get("security")
            if not isinstance(implementation, str) or not isinstance(exponent, int):
                raise ReportError(f"{trace}:{line_number}: malformed run identity")
            if not isinstance(security, dict):
                raise ReportError(f"{trace}:{line_number}: missing security metadata")
            key = (implementation, exponent)
            previous = found.get(key)
            if previous is not None and previous != security:
                raise ReportError(f"{trace}: inconsistent security metadata for {key}")
            found[key] = security
    return found


def summary_groups(summary: dict[str, Any]) -> dict[tuple[str, int], dict[str, Any]]:
    groups = summary.get("groups")
    if not isinstance(groups, list):
        raise ReportError("comparison summary is missing groups")
    result: dict[tuple[str, int], dict[str, Any]] = {}
    for group in groups:
        if not isinstance(group, dict):
            raise ReportError("comparison summary group must be an object")
        implementation = group.get("implementation")
        exponent = group.get("log_multiplications")
        if isinstance(implementation, str) and isinstance(exponent, int):
            result[(implementation, exponent)] = group
    return result


def validate_security(
    profile: str, exponent: int, security: dict[str, Any]
) -> tuple[list[int], int]:
    if security.get("target_bits") != 100:
        raise ReportError(f"{profile} 2^{exponent} does not target 100-bit security")
    schedule = security.get("query_schedule")
    total = security.get("total_query_openings")
    if (
        not isinstance(schedule, list)
        or not schedule
        or any(not isinstance(value, int) or value <= 0 for value in schedule)
        or not isinstance(total, int)
        or total != sum(schedule)
    ):
        raise ReportError(f"{profile} 2^{exponent} has invalid query accounting")
    if profile == "f2z":
        if security.get("commitment_field") != "GF(2^128)":
            raise ReportError(f"F2Z 2^{exponent} is not the GF(2^128) profile")
    else:
        wanted_degree = 4 if profile == "whir-d4" else 5
        wanted_assumption = "UniqueDecoding" if wanted_degree == 4 else "JohnsonBound"
        if security.get("challenge_extension_degree") != wanted_degree:
            raise ReportError(f"{profile} 2^{exponent} has the wrong extension degree")
        if security.get("security_assumption") != wanted_assumption:
            raise ReportError(f"{profile} 2^{exponent} has the wrong soundness assumption")
    return schedule, total


def build_rows(args: argparse.Namespace) -> list[dict[str, Any]]:
    primary_summary = summary_groups(load_json(args.primary_summary))
    degree4_summary = summary_groups(load_json(args.degree4_summary))
    primary_security = load_security(args.primary_trace)
    degree4_security = load_security(args.degree4_trace)
    rows: list[dict[str, Any]] = []
    for exponent in EXPONENTS:
        row: dict[str, Any] = {
            "log_multiplications": exponent,
            "multiplications": 1 << exponent,
            "profiles": {},
        }
        sources = {
            "f2z": (primary_summary, primary_security, "f2z"),
            "whir-d4": (degree4_summary, degree4_security, "plonky3-whir"),
            "whir-d5": (primary_summary, primary_security, "plonky3-whir"),
        }
        for profile, (groups, security_by_key, implementation) in sources.items():
            key = (implementation, exponent)
            group = groups.get(key)
            security = security_by_key.get(key)
            if group is None or security is None:
                raise ReportError(f"missing {profile} 2^{exponent}")
            if group.get("cell_status") != "measured" or group.get("eligible_count") != 5:
                raise ReportError(f"{profile} 2^{exponent} is not a complete five-sample cell")
            schedule, total = validate_security(profile, exponent, security)
            row["profiles"][profile] = {
                "query_schedule": schedule,
                "total_query_openings": total,
                "timings_ms": group["timings_ms"],
                "artifacts_bytes": group["artifacts_bytes"],
                "eligible_count": group["eligible_count"],
                "security": security,
            }
        rows.append(row)
    return rows


def median(row: dict[str, Any], profile: str, family: str, key: str) -> float:
    return float(row["profiles"][profile][family][key]["median"])


def fmt_ms(value: float) -> str:
    return f"{value:,.3f}"


def fmt_bytes(value: float) -> str:
    return f"{value:,.0f}"


def markdown_triplet(values: list[float], formatter) -> str:
    winner = min(range(len(values)), key=values.__getitem__)
    return " / ".join(
        f"**{formatter(value)}**" if index == winner else formatter(value)
        for index, value in enumerate(values)
    )


def html_triplet(values: list[float], formatter) -> str:
    winner = min(range(len(values)), key=values.__getitem__)
    return " / ".join(
        f"<strong>{html.escape(formatter(value))}</strong>"
        if index == winner
        else html.escape(formatter(value))
        for index, value in enumerate(values)
    )


def render_markdown(rows: list[dict[str, Any]], sources: dict[str, str]) -> str:
    lines = [
        "# Baby Bear PCS — 100-bit query and performance comparison",
        "",
        "Every comparison cell is **F2Z/Ligerito / WHIR degree-4 / WHIR degree-5**. "
        "Bold is the lowest value. One warmup and five measured verified proofs per cell.",
        "",
        "## Query openings",
        "",
        "| Multiplications | F2Z/Ligerito | WHIR d4 | WHIR d5 |",
        "|---:|---:|---:|---:|",
    ]
    for row in rows:
        cells = []
        totals = [row["profiles"][key]["total_query_openings"] for key, _ in PROFILES]
        winner = min(totals)
        for key, _ in PROFILES:
            profile = row["profiles"][key]
            total = profile["total_query_openings"]
            schedule = "+".join(map(str, profile["query_schedule"]))
            text = f"{total} ({schedule})"
            cells.append(f"**{text}**" if total == winner else text)
        lines.append(f"| $2^{{{row['log_multiplications']}}}$ | " + " | ".join(cells) + " |")
    lines += [
        "",
        "## Measured results",
        "",
        "| Multiplications | PCS prover (ms) | Commit (ms) | Opening (ms) | Verify (ms) | Opening proof (B) |",
        "|---:|---:|---:|---:|---:|---:|",
    ]
    for row in rows:
        time_cell = lambda key: markdown_triplet(
            [median(row, profile, "timings_ms", key) for profile, _ in PROFILES], fmt_ms
        )
        proof_cell = markdown_triplet(
            [median(row, profile, "artifacts_bytes", "opening_proof_bytes") for profile, _ in PROFILES],
            fmt_bytes,
        )
        lines.append(
            f"| $2^{{{row['log_multiplications']}}}$ | {time_cell('pcs_total_prover')} | "
            f"{time_cell('commit')} | {time_cell('opening')} | {time_cell('verify')} | {proof_cell} |"
        )
    lines += ["", "## Provenance", ""]
    lines.extend(f"- {label}: `{path}`" for label, path in sources.items())
    return "\n".join(lines) + "\n"


def render_html(rows: list[dict[str, Any]], sources: dict[str, str]) -> str:
    query_rows = []
    performance_rows = []
    for row in rows:
        totals = [row["profiles"][key]["total_query_openings"] for key, _ in PROFILES]
        winner = min(totals)
        query_cells = []
        for key, _ in PROFILES:
            profile = row["profiles"][key]
            total = profile["total_query_openings"]
            schedule = " + ".join(map(str, profile["query_schedule"]))
            value = f"<strong>{total}</strong>" if total == winner else str(total)
            query_cells.append(f'<td title="query schedule: {html.escape(schedule)}">{value}<small>{html.escape(schedule)}</small></td>')
        query_rows.append(
            f"<tr><th>2<sup>{row['log_multiplications']}</sup><small>{row['multiplications']:,}</small></th>"
            + "".join(query_cells)
            + "</tr>"
        )
        time_cell = lambda key: html_triplet(
            [median(row, profile, "timings_ms", key) for profile, _ in PROFILES], fmt_ms
        )
        proof_cell = html_triplet(
            [median(row, profile, "artifacts_bytes", "opening_proof_bytes") for profile, _ in PROFILES],
            fmt_bytes,
        )
        performance_rows.append(
            f"<tr><th>2<sup>{row['log_multiplications']}</sup></th>"
            f"<td>{time_cell('pcs_total_prover')}</td><td>{time_cell('commit')}</td>"
            f"<td>{time_cell('opening')}</td><td>{time_cell('verify')}</td><td>{proof_cell}</td></tr>"
        )
    source_items = "".join(
        f"<li><strong>{html.escape(label)}:</strong> <code>{html.escape(path)}</code></li>"
        for label, path in sources.items()
    )
    return f"""<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>Baby Bear PCS — 100-bit query comparison</title>
<style>
:root{{--ink:#18212f;--muted:#667085;--line:#e4e7ec;--blue:#175cd3;--wash:#f8fafc}}
*{{box-sizing:border-box}}body{{margin:0;background:#fff;color:var(--ink);font-family:Inter,ui-sans-serif,system-ui,-apple-system,sans-serif}}
main{{max-width:1480px;margin:0 auto;padding:42px 28px 72px}}h1{{font-size:34px;margin:0 0 10px}}h2{{margin-top:38px}}
p{{color:var(--muted);line-height:1.55;max-width:1000px}}.legend{{padding:14px 18px;border:1px solid #bfd4ff;background:#f3f7ff;border-radius:12px;color:#344054}}
.wrap{{overflow-x:auto;border:1px solid var(--line);border-radius:14px}}table{{border-collapse:collapse;width:100%;min-width:900px}}
th,td{{padding:13px 15px;border-bottom:1px solid var(--line);text-align:right;white-space:nowrap}}thead th{{background:var(--wash);color:#475467;font-size:13px}}
tbody th{{text-align:left}}tbody tr:last-child th,tbody tr:last-child td{{border-bottom:0}}strong{{color:var(--blue)}}small{{display:block;color:var(--muted);font-size:11px;margin-top:4px}}
code{{white-space:normal;word-break:break-all}}ul{{line-height:1.6}}.note{{font-size:13px}}
</style></head><body><main>
<h1>Baby Bear PCS — 100-bit query and performance comparison</h1>
<p>Same deterministic integer witnesses, Apple M1 Max, 10 Rayon threads, native code generation, one warmup and five measured verified proofs per cell.</p>
<p class="legend"><strong>Ordering:</strong> F2Z/Ligerito / WHIR degree-4 unique decoding / WHIR degree-5 Johnson bound. Bold marks the lowest value in each comparison.</p>
<h2>Instantiated query openings</h2>
<p>Totals are derived from the exact protocol configuration instantiated for each proof. The smaller line shows the recursive or round-plus-final schedule.</p>
<div class="wrap"><table><thead><tr><th>Multiplications</th><th>F2Z / Ligerito</th><th>WHIR degree-4</th><th>WHIR degree-5</th></tr></thead><tbody>{''.join(query_rows)}</tbody></table></div>
<h2>Measured performance</h2>
<p>Times are five-sample medians in milliseconds; proof size is the median serialized opening proof in bytes.</p>
<div class="wrap"><table><thead><tr><th>Multiplications</th><th>PCS prover (ms)</th><th>Commit (ms)</th><th>Opening (ms)</th><th>Verify (ms)</th><th>Opening proof (B)</th></tr></thead><tbody>{''.join(performance_rows)}</tbody></table></div>
<p class="note">Query counts are protocol-native openings, not equal-cost operations: Ligerito and WHIR authenticate different data along each query path. The degree-4 campaign ran immediately before the matched F2Z/degree-5 campaign, so timings are matched in configuration but not interleaved across all three implementations.</p>
<h2>Provenance</h2><ul>{source_items}</ul>
</main></body></html>"""


def write_csv(path: Path, rows: list[dict[str, Any]]) -> None:
    fields = [
        "log_multiplications", "multiplications", "profile", "target_bits",
        "query_schedule", "total_query_openings", "pcs_total_prover_median_ms",
        "commit_median_ms", "opening_median_ms", "verify_median_ms",
        "opening_proof_median_bytes", "eligible_samples",
    ]
    with path.open("x", newline="", encoding="utf-8") as output:
        writer = csv.DictWriter(output, fieldnames=fields)
        writer.writeheader()
        for row in rows:
            for profile, _ in PROFILES:
                data = row["profiles"][profile]
                writer.writerow({
                    "log_multiplications": row["log_multiplications"],
                    "multiplications": row["multiplications"],
                    "profile": profile,
                    "target_bits": data["security"]["target_bits"],
                    "query_schedule": "+".join(map(str, data["query_schedule"])),
                    "total_query_openings": data["total_query_openings"],
                    "pcs_total_prover_median_ms": data["timings_ms"]["pcs_total_prover"]["median"],
                    "commit_median_ms": data["timings_ms"]["commit"]["median"],
                    "opening_median_ms": data["timings_ms"]["opening"]["median"],
                    "verify_median_ms": data["timings_ms"]["verify"]["median"],
                    "opening_proof_median_bytes": data["artifacts_bytes"]["opening_proof_bytes"]["median"],
                    "eligible_samples": data["eligible_count"],
                })


def main(argv: Sequence[str] | None = None) -> int:
    args = parse_args(argv)
    rows = build_rows(args)
    sources = {
        "F2Z and WHIR degree-5 summary": str(args.primary_summary.resolve()),
        "F2Z and WHIR degree-5 trace": str(args.primary_trace.resolve()),
        "WHIR degree-4 summary": str(args.degree4_summary.resolve()),
        "WHIR degree-4 trace": str(args.degree4_trace.resolve()),
    }
    args.out_dir.mkdir(parents=True, exist_ok=True)
    targets = {
        "summary.json": json.dumps({
            "schema": "baby-bear-pcs-three-way/v1",
            "generated_at": datetime.now(timezone.utc).isoformat(),
            "profiles": [key for key, _ in PROFILES],
            "sources": sources,
            "rows": rows,
        }, indent=2) + "\n",
        "comparison.md": render_markdown(rows, sources),
        "comparison.html": render_html(rows, sources),
    }
    for name, content in targets.items():
        path = args.out_dir / name
        if path.exists() and not args.force:
            raise ReportError(f"refusing to overwrite {path}; pass --force")
        path.write_text(content, encoding="utf-8")
    csv_path = args.out_dir / "metrics.csv"
    if csv_path.exists():
        if not args.force:
            raise ReportError(f"refusing to overwrite {csv_path}; pass --force")
        csv_path.unlink()
    write_csv(csv_path, rows)
    print(args.out_dir / "comparison.html")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except ReportError as error:
        print(f"error: {error}", file=__import__("sys").stderr)
        raise SystemExit(2) from error
