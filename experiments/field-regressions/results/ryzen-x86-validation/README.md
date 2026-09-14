# Implementation validation — Ryzen 9 9950X3D

Validated on 2026-09-14, on the local `fast-arithmetic` branch. This record
establishes coverage, arithmetic correctness and working benchmark plumbing.
The short samples below do **not** select confirmed performance winners.

## Complete coverage sweep

```sh
python3 experiments/field-regressions/ryzen.py \
  --out /tmp/bitz-ryzen-validation-01 --streaming --scaling --runs 1 --samples 2
```

| Placement | Threads | Complete variant cases | Arithmetic correctness failures |
|---|---:|---:|---:|
| CPU 0, 96 MiB L3 | 1 | 1,257 / 1,257 | 0 |
| CPU 8, 32 MiB L3 | 1 | 1,257 / 1,257 | 0 |
| CPUs 0–7 | 8 | 119 / 119 | 0 |
| CPUs 8–15 | 8 | 119 / 119 | 0 |
| CPUs 0–15 | 16 | 119 / 119 | 0 |
| CPUs 0–31 | 32 | 119 / 119 | 0 |

The single-core matrix covers 39 distinct operation families; the four scaling
placements cover consumers. Reset-only controls deliberately have no arithmetic
correctness claim. Allocation-regressing alternatives remain visible and are
ineligible for selection. [coverage.json](coverage.json) records placement,
executable hashes and the local raw-artifact directories.

The benchmark kernels were fixed during this sweep; Python reporting and
diagnostic instrumentation received further changes. Every placement has its
own frozen sources. Start a fresh exploration with the final source tree before
attempting independent confirmation.

## NTT activation

All seven shapes exercised the candidate scalar multiplication in a separate
untimed counting build. [ntt-activation.json](ntt-activation.json) records the
counts. Specialized production kernels remain present in those transforms.
The counting build emits no timing samples.

```sh
python3 experiments/field-regressions/run.py --suite x86 --cpu-set 0 \
  --probe-ntt --out /tmp/bitz-ryzen-ntt-activation-01 \
  --target-dir /tmp/bitz-ryzen-target
```

## Full-prover smoke checks

Multiplication (`u32-mod32`, exponent 15), split SHA256+ECDSA (exponent 7,
security target 100), and PCS (`7:8:32`) each completed on CPU 0 and CPU 8.
All six workers verified their proofs. These are one-process baseline
characterizations; larger shapes and full-prover scaling were not timed in this
implementation check. [prover-results.json](prover-results.json) preserves the
structured output and host observations.

```sh
PERFETTO_TRACE_PROCESSOR=/tmp/bitz-trace-processor-shell \
python3 experiments/field-regressions/end_to_end.py \
  --out /tmp/bitz-prover-validation-03 \
  --cases pcs-7-8-32,sha-ecdsa-n7,u32-mod32-n15 --runs 1
```

The official Perfetto v58.2 native binary was downloaded into `/tmp` and checked
against the SHA-256 in its official launcher manifest:
`58042408e6cc861fb1a731c26bb082dc222285561eaa4e12a48a8b2b90dca7b9`.
It is also preserved in the local full-prover output directory. Initial smoke
attempts exposed missing Perfetto and an unnecessary feature union for PCS;
the driver now preflights Perfetto and builds only the selected benches' feature
union.

## Harness checks

- 33 Python policy tests pass, including incomplete coverage, changed artifacts,
  wrong affinity/backends, invalid selections, seed planning, allocation limits,
  paired-tail statistics, challenger retries, and exclusive benchmark leases.
- Native release builds and arithmetic correctness checks pass.
- A final smoke run completed all 31 requested variants in
  `/tmp/bitz-ryzen-final-smoke`. All 655 archived original/generated source-file
  hashes matched the frozen metadata, including the resolved build lockfile.
  The performance gate correctly rejected this exploration run as insufficient
  for independent confirmation.
- Both matrix drivers produce their complete workload plans with `--dry-run`.
- `git diff --check` passes.

See [the campaign instructions](../../X86.md) for fresh exploration followed by
five-process, 32-sample confirmation. Production arithmetic was not changed by
this benchmark implementation.
