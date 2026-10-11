#!/usr/bin/env python3
"""Require exact proof identities for every case shared by the two v7 runs."""
import argparse
import itertools
import json
from pathlib import Path


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--reference", type=Path, required=True)
    parser.add_argument("--candidate", type=Path, nargs="+", required=True)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--scope", choices=("all", "split"), default="all",
                        help="Require full proof identity for all cases, or only Split cases above the 2^19-bit source floor; still check every Single input and verification.")
    args = parser.parse_args()
    keys = ["input_digest", "protocol_id", "trial", "iteration", "source_root", "proof_debug_digest", "proof_payload_bytes"]
    reference_arguments = json.loads((args.reference / "metadata.json").read_text())["arguments"]
    required = {key: reference_arguments[key] for key in ["degrees", "securities", "batches", "threads", "seeds"]}
    trials = reference_arguments["iterations"] + 1
    checked = []
    changed_layout = []
    for degree, security, batch, threads, seed in itertools.product(*(required[key] for key in ["degrees", "securities", "batches", "threads", "seeds"])):
        name = f"candidate-n{degree}-s{security}-b{batch}-t{threads}-seed{seed}.jsonl"
        candidates = [directory / name for directory in args.candidate if (directory / name).exists()]
        assert len(candidates) == 1, f"expected exactly one candidate run for {name}, found {len(candidates)}"
        old = [json.loads(line) for line in (args.reference / name).read_text().splitlines()]
        new = [json.loads(line) for line in candidates[0].read_text().splitlines()]
        assert len(old) == len(new) == trials, f"wrong trial count for {name}"
        full_identity = args.scope == "all" or old[0]["committed_source_bits"] > 1 << 19
        required_keys = keys if full_identity else ["input_digest", "protocol_id", "trial", "iteration"]
        for before, after in zip(old, new):
            assert before["verified"] and after["verified"], f"verification missing for {name}"
            assert before["committed_source_bits"] == after["committed_source_bits"], f"physical source size changed for {name}"
            for key in required_keys:
                assert before[key] == after[key], f"proof identity mismatch: {name}: {key}"
        record = {"file": name, "candidate_path": str(candidates[0]), "proofs": len(new)}
        if full_identity:
            checked.append(record)
        else:
            assert old[0]["committed_source_bits"] == 1 << 19, f"unexpected non-Split source size for {name}"
            changed_layout.append(record)
    result = {"passed": True, "reference": str(args.reference), "candidate": [str(path) for path in args.candidate],
              "required_cases": required,
              "scope": args.scope,
              "checked_keys": keys, "checked_proofs": sum(row["proofs"] for row in checked), "checked_runs": checked,
              "single_layout_change_input_checks": sum(row["proofs"] for row in changed_layout),
              "single_layout_change_runs": changed_layout,
              "single_layout_check_keys": ["input_digest", "protocol_id", "trial", "iteration", "verified", "committed_source_bits"],
              "interpretation": "Every required case must exist exactly once. Full proof identity applies to the selected scope; Single cases outside the Split-only scope retain input/verification/physical-size checks but may change their witness layout and proof."}
    output = args.output or args.candidate[0] / "reference-proof-identity.json"
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps({"identity_audit_passed": True, "checked_proofs": result["checked_proofs"],
                      "single_layout_change_input_checks": result["single_layout_change_input_checks"]}))


if __name__ == "__main__":
    main()
