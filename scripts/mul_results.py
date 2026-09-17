"""Validate the current multiplication format and aggregate raw, verified trials."""
from __future__ import annotations

from dataclasses import dataclass
import json
import math
from pathlib import Path
import statistics

SCHEMA = "mul-bench/v1"


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(",", ":"), allow_nan=False)


def integer(value, name, minimum=0):
    if type(value) is not int or value < minimum:
        raise ValueError(f"invalid {name}: {value!r}")
    return value


def finite_number(value, name):
    if type(value) not in (int, float) or not math.isfinite(value) or value < 0:
        raise ValueError(f"invalid {name}: {value!r}")
    return value


def percentile(values, probability):
    """Type-7 percentile, including the endpoints."""
    if not values or not 0 <= probability <= 1:
        raise ValueError("percentile requires samples and a probability in [0, 1]")
    values = sorted(values)
    position = (len(values) - 1) * probability
    index = math.floor(position)
    fraction = position - index
    return values[index] + fraction * (values[min(index + 1, len(values) - 1)] - values[index])


@dataclass
class Case:
    source: Path
    job: dict
    effective: dict
    provenance: dict
    status: str
    reason: str | None
    records: list[dict]

    @property
    def identity(self):
        return canonical(dict(case=self.job["case"], effective=self.effective,
                              provenance=self.provenance))


def load(directory: Path) -> list[Case]:
    manifest = json.loads((directory / "manifest.json").read_text())
    if manifest.get("schema") != SCHEMA or manifest.get("status") != "complete":
        raise ValueError(f"{directory}: expected a complete {SCHEMA} campaign")
    provenance = manifest.get("provenance")
    if not isinstance(provenance, dict) or not provenance:
        raise ValueError("missing campaign provenance")
    cases = {}
    identities = set()
    for entry in manifest["cases"]:
        job = entry["job"]
        identifier = job["id"]
        descriptor = job["case"]
        if identifier in cases or canonical(descriptor) in identities:
            raise ValueError(f"duplicate case {identifier}")
        for name, minimum in [("log_n", 4), ("seed", 0), ("threads", 1)]:
            integer(descriptor.get(name), name, minimum)
        if descriptor["seed"] >= 2**64 or descriptor["log_n"] > 63:
            raise ValueError("invalid seed or size")
        if not all(descriptor.get(k) for k in ("mode", "workload", "backend")):
            raise ValueError("incomplete case identity")
        if descriptor["backend"] == "f2z" and descriptor["mode"] not in ("outer", "piop"):
            config = descriptor.get("f2z", {})
            if not all(k in config for k in ("w", "split", "profile", "bound", "ligerito")):
                raise ValueError("incomplete F2Z configuration")
            if descriptor["mode"] != "witness" and not all(config.get(k) for k in ("profile", "bound", "ligerito")):
                raise ValueError("missing F2Z proof configuration")
        integer(job.get("reps"), "reps", 1)
        integer(job.get("warmups"), "warmups")
        if job.get("memory") not in ("none", "rss", "heap"):
            raise ValueError("unknown memory mode")
        status = entry.get("status")
        if status not in ("measured", "skipped"):
            raise ValueError(f"invalid case status {status}")
        if status == "skipped" and not entry.get("reason"):
            raise ValueError("skip has no reason")
        effective = entry.get("effective", {})
        if status == "measured" and (not effective.get("boundary") or not
                (effective.get("corpus_digest") or effective.get("fixture_digest"))):
            raise ValueError("missing measured configuration or corpus identity")
        cases[identifier] = Case(directory, job, effective, provenance, status,
                                 entry.get("reason"), [])
        identities.add(canonical(descriptor))
    seen = set()
    for number, line in enumerate((directory / "samples.jsonl").read_text().splitlines(), 1):
        record = json.loads(line)
        identifier = record["case_id"]
        if identifier not in cases or cases[identifier].status != "measured":
            raise ValueError(f"sample {number} references an absent/skipped case")
        index = integer(record.get("index"), "sample index")
        kind = record.get("kind")
        key = (identifier, kind, index)
        if key in seen:
            raise ValueError(f"duplicate sample {key}")
        if record.get("verified") is not True:
            raise ValueError(f"unverified sample {key}")
        metrics = record.get("metrics")
        if not isinstance(metrics, dict) or not metrics:
            raise ValueError(f"missing metrics {key}")
        for name, value in metrics.items():
            finite_number(value, name)
        seen.add(key)
        cases[identifier].records.append(record)
    for identifier, case in cases.items():
        job = case.job
        expected = set()
        if case.status == "measured":
            if job["memory"] != "heap":
                expected |= {(identifier, "sample", i) for i in range(job["reps"])}
                expected |= {(identifier, "warmup", i) for i in range(job["warmups"])}
            if job["memory"] != "none":
                expected.add((identifier, job["memory"], 0))
        actual = {key for key in seen if key[0] == identifier}
        if actual != expected:
            raise ValueError(f"{identifier}: missing or unexpected samples: {actual ^ expected}")
        measured = [r for r in case.records if r["kind"] == "sample"]
        if measured and any(set(r["metrics"]) != set(measured[0]["metrics"]) for r in measured):
            raise ValueError(f"{identifier}: inconsistent sample metrics")
        required = {
            "standalone-proving": {"commit_ms", "online_prover_ms", "verify_ms", "proof_bytes"},
            "witness-to-proof": {"witness_ms", "commit_ms", "online_prover_ms", "witness_to_proof_ms", "verify_ms", "verified_trial_ms", "proof_bytes"},
            "witness-generation": {"witness_ms"},
            "pcs-opening": {"materialize_ms", "commit_ms", "claim_ms", "opening_ms", "verify_ms", "verified_trial_ms", "pcs_ms", "commitment_bytes", "claim_bytes", "opening_bytes", "proof_bytes"},
            "whole-piop": {"piop_ms", "verify_ms", "analytical_piop_bytes"},
            "outer-kernel": {"outer_ms"},
            "outer-regression": {"production_ms", "generic_ms"},
        }.get(case.effective.get("boundary"))
        if case.status == "measured" and required is None:
            raise ValueError(f"{identifier}: unknown measurement boundary")
        for record in case.records:
            if record["kind"] in ("sample", "warmup"):
                if not required <= record["metrics"].keys():
                    raise ValueError(f"{identifier}: missing required metrics: {required - record['metrics'].keys()}")
                if "proof_bytes" in required and record["metrics"]["proof_bytes"] <= 0:
                    raise ValueError(f"{identifier}: missing verified proof size")
            if record["kind"] in ("rss", "heap"):
                metric = "peak_rss_bytes" if record["kind"] == "rss" else "peak_heap_bytes"
                if set(record["metrics"]) != {metric} or record["metrics"][metric] <= 0:
                    raise ValueError(f"{identifier}: invalid memory record")
    return list(cases.values())


def select(cases, filters):
    for case in cases:
        values = {**case.job["case"], **case.job["case"].get("f2z", {})}
        if all(str(values.get(key)) == value for key, value in filters.items()):
            yield case


def aggregate(cases):
    rows = []
    seen = set()
    for case in cases:
        # Re-reading the same campaign is an error, not another repetition.
        key = (str(case.source.resolve()), case.job["id"])
        if key in seen:
            raise ValueError(f"duplicate campaign case {key}")
        seen.add(key)
        measurements = [r["metrics"] for r in case.records if r["kind"] == "sample"]
        stats = {}
        if measurements:
            for metric in measurements[0]:
                values = [m[metric] for m in measurements]
                stats[metric] = dict(median=statistics.median(values),
                                     p05=percentile(values, .05), p95=percentile(values, .95),
                                     minimum=min(values), maximum=max(values), count=len(values))
        memory = {k: v for r in case.records if r["kind"] in ("rss", "heap")
                  for k, v in r["metrics"].items()}
        rows.append(dict(source=str(case.source), case_id=case.job["id"],
                         case=case.job["case"], effective=case.effective,
                         provenance=case.provenance, status=case.status,
                         reason=case.reason, metrics=stats, memory=memory))
    return rows
