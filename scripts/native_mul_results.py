"""Identity checks shared by the multiplication runner and paper exporter."""
import hashlib
import json
import math
from ligerito_results import validate_ligerito

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


# Configuration keys that later harness revisions record for information only;
# when one side of a comparison predates them they are ignored, when both sides
# carry them they must agree (they name the u64 BitZ split).
INFORMATIONAL_CONFIG_KEYS = ("u64_split_shift", "bitz_t", "bitz_s")


def protocol_identity(row, other):
    """The fingerprint's identity minus the source tree, on configuration keys
    both rows record (see INFORMATIONAL_CONFIG_KEYS)."""
    config = dict(row["config"])
    for key in INFORMATIONAL_CONFIG_KEYS:
        if key not in row["config"] or key not in other["config"]:
            config.pop(key, None)
    identity = {"workload": row["workload"], "config": config,
                "measurement_policy": row["measurement_policy"], "threads": row["threads"],
                "build": row["provenance"]["build"]}
    return hashlib.sha256(json.dumps(identity, sort_keys=True, separators=(",", ":")).encode()).hexdigest()


def require_compatible(left, right, allow_source_drift=False):
    """Only combine the same protocol, source, corpus, machine and timing policy.

    With `allow_source_drift` the two rows may come from different source
    trees: they must still agree on protocol identity (configuration, policy,
    threads, build), and the caller is expected to verify that their proof
    bytes agree wherever they overlap."""
    if any(row.get("protocol_fingerprint") != fingerprint(row) for row in (left, right)):
        raise ValueError("result fingerprint does not match its configuration")
    keys = ["schema", "workload", "backend", "log_multiplications", "corpus_digest",
            "threads", "measurement_policy"]
    if allow_source_drift:
        if protocol_identity(left, right) != protocol_identity(right, left):
            raise ValueError("incompatible multiplication results: protocol identity differs across source trees")
    else:
        keys.append("protocol_fingerprint")
    if any(left.get(key) != right.get(key) for key in keys):
        raise ValueError("incompatible multiplication results: protocol/configuration/corpus differs")
    if left.get("provenance", {}).get("machine") != right.get("provenance", {}).get("machine"):
        raise ValueError("incompatible multiplication results: measurement machines differ")


def validate_summary(row):
    if row.get("backend") == "bitz":
        validate_ligerito(row.get("config", {}).get("ligerito"), 100)
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
