"""Identity checks shared by the multiplication runner and paper exporter."""
import hashlib
import json
import math

SAMPLE_SCHEMA = "native-mul-sample/v2"
SUMMARY_SCHEMA = "native-mul-summary/v2"
POLICY = "warm-process/v1"
CORE_METRICS = ("witness_ms", "online_prover_ms", "witness_to_proof_ms", "verify_ms", "proof_bytes")


def finite_number(value, name, positive=False):
    if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(value):
        raise ValueError(f"invalid {name}: {value!r}")
    if value < 0 or (positive and value == 0):
        raise ValueError(f"invalid {name}: {value!r}")
    return value


def fingerprint(row):
    source = row["provenance"]
    identity = {key: row[key] for key in ("workload", "config", "measurement_policy", "threads")}
    identity["build"] = source["build"]
    identity["source_sha256"] = source["source_sha256"]
    return hashlib.sha256(json.dumps(identity, sort_keys=True, separators=(",", ":")).encode()).hexdigest()


def require_compatible(left, right):
    """Only combine the same protocol, source, corpus, machine and timing policy."""
    if any(row.get("protocol_fingerprint") != fingerprint(row) for row in (left, right)):
        raise ValueError("result fingerprint does not match its configuration")
    keys = ("schema", "workload", "backend", "log_multiplications", "corpus_digest",
            "threads", "measurement_policy", "protocol_fingerprint")
    if any(left.get(key) != right.get(key) for key in keys):
        raise ValueError("incompatible multiplication results: protocol/configuration/corpus differs")
    if left.get("provenance", {}).get("machine") != right.get("provenance", {}).get("machine"):
        raise ValueError("incompatible multiplication results: measurement machines differ")


def validate_summary(row):
    if row.get("schema") != SUMMARY_SCHEMA or row.get("measurement_policy") != POLICY:
        raise ValueError("historical or unsupported multiplication results; regenerate the comparison")
    if row.get("proof_verified") is not True or row.get("warmups") != 1:
        raise ValueError("summary lacks a verified warm-process measurement")
    if row.get("multiplications") != 1 << row["log_multiplications"] or row.get("samples", 0) < 1:
        raise ValueError("invalid multiplication count or sample count")
    if row.get("protocol_fingerprint") != fingerprint(row):
        raise ValueError("summary protocol fingerprint does not match its configuration")
    for name in CORE_METRICS:
        finite_number(row["medians"].get(name), name, positive=name == "proof_bytes")
