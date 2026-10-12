"""Diagnostic fold timings; run sequentially after builds and benchmarks stop."""
import hashlib
import json
import os
from pathlib import Path
import statistics
import subprocess
import sys

binary = Path(sys.argv[1]).resolve()
out = Path(sys.argv[2]).resolve()
out.mkdir(parents=True)
assert not any(out.iterdir())
cpus = [6, 7, 4, 5, 2, 3, 0, 1, 14, 15, 12, 13, 10, 11, 8, 9]
base_env = {k: v for k, v in os.environ.items() if not k.startswith(("BITZ_", "RAYON_", "OMP_"))}
identity_keys = ["input_digest", "source_root", "proof_debug_digest", "proof_payload_bytes", "proof_payload_breakdown"]
summary = []
for degree in [512, 1024]:
    for workers in [12, 16]:
        reference = None
        for mode in ["legacy", "prepared", "biased"]:
            name = f"n{degree}-t{workers}-{mode}"
            selected = ",".join(map(str, cpus[:workers]))
            configured = {
                "RAYON_NUM_THREADS": str(workers), "OMP_NUM_THREADS": str(workers),
                "BITZ_FALCON_WORKER_CPUS": selected, "BITZ_FALCON_MAIN_CPU": str(cpus[0]),
                "BITZ_FALCON_CASE_CACHE": "/tmp/bitz-falcon-speed-case-cache",
                "BITZ_ARITHMETIC_BINDING_STRATEGY": "optimized", "BITZ_ARITHMETIC_WORD_FOLD": mode,
            }
            command = ["taskset", "-c", selected, str(binary), "--degree", str(degree),
                       "--batch", "1024", "--security", "128", "--threads", str(workers),
                       "--seed", "42", "--warmup", "2", "--iterations", "9", "--trace"]
            (out / f"{name}.command.json").write_text(json.dumps({
                "argv": command, "environment": configured,
                "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
            }, indent=2) + "\n")
            with (out / f"{name}.jsonl").open("w") as stdout, (out / f"{name}.stages.jsonl").open("w") as stderr:
                subprocess.run(command, env={**base_env, **configured}, stdout=stdout, stderr=stderr, check=True)
            trials = [json.loads(x) for x in (out / f"{name}.jsonl").read_text().splitlines()]
            assert len(trials) == 11 and all(x["verified"] for x in trials)
            identity = {k: trials[0][k] for k in identity_keys}
            assert all({k: x[k] for k in identity_keys} == identity for x in trials)
            if reference is None:
                reference = identity
            assert identity == reference
            stages = [json.loads(x) for x in (out / f"{name}.stages.jsonl").read_text().splitlines() if x.startswith("{")]
            folded = [s["elapsed_ms"] for s in stages if s["name"] == "falcon_algebraic:word_first_fold"]
            assert len(folded) == 11
            row = dict(degree=degree, workers=workers, mode=mode, fold_ms=statistics.median(folded[2:]),
                       prove_ms=statistics.median(x["prove_ms"] for x in trials[2:]))
            summary.append(row)
            print(json.dumps(row), flush=True)
(out / "summary.json").write_text(json.dumps({
    "method": "Diagnostic traced seed42 medians: two warmups, nine samples; binding forced optimized in every variant. Not an untraced acceptance benchmark.",
    "cases": summary,
}, indent=2) + "\n")
