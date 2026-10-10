# Falcon binder experiments

Run the existing `falcon_algebraic` and `falcon_hybrid` benchmark executables in
separate, sequential processes. Build baseline and candidate beforehand with the
same compiler, flags and features; candidate requires `bench-internals` for the
kernel controls. The runner never builds anything. Its defaults are Falcon-512,
batch 1024, security 128, 1/4/8/16 workers, three input seeds, one independent
process per configuration/seed, one warmup and one measured trial per process.

```sh
python3 scripts/binder_experiments.py run \
  --output results/binder-screen \
  --baseline-arithmetic /absolute/path/to/baseline-falcon_algebraic \
  --candidate-arithmetic /absolute/path/to/candidate-falcon_algebraic \
  --baseline-full /absolute/path/to/baseline-falcon_hybrid \
  --candidate-full /absolute/path/to/candidate-falcon_hybrid
```

Only executables needed by the selected configurations are required. For example,
an arithmetic-only campaign adds `--variants arithmetic` and needs no full binary.
Use `--case-cache /absolute/path/to/shared-cache` to reuse distinct validated
original-Falcon fixtures across screening, holdout and diagnostic campaigns. Input
generation is outside benchmark prover timers. Case caches contain the original
benchmark witness material, so keep them with local experiment artifacts.

The default screen compares the baseline executable, candidate `auto`, and:

| Configuration | Variant | Meaning |
|---|---|---|
| `r000` through `r111` | arithmetic | Three binary digits select replay, construction, evaluation, in that order |
| `replay-off`, `replay-on` | full | Original or optimized packed replay/grouping |
| `word` (opt in) | arithmetic | Word binder with all three kernel controls on |

`auto` keeps the existing kernel dispatch; all default screening configurations
explicitly set `BITZ_BENCH_WORD_BINDER=off`. The `word` configuration requires a
candidate that implements the new protocol and reports its actual protocol ID.
The baseline always uses its original kernel policy. For a small selected holdout:

```sh
python3 scripts/binder_experiments.py run \
  --output results/binder-holdout \
  --variants arithmetic --configs baseline,r111,word \
  --seeds 101,202,303,404,505 --repetitions 3 --iterations 2 \
  --baseline-arithmetic /absolute/path/to/baseline-falcon_algebraic \
  --candidate-arithmetic /absolute/path/to/candidate-falcon_algebraic \
  --case-cache /absolute/path/to/shared-cache
```

Use different seeds for screening and holdout. `--repetitions` creates independent
processes; `--iterations` creates multiple trials within each process. All
configurations in a matched block share variant, thread count, input seed and
repetition. Blocks and their configuration order are shuffled deterministically
with `--order-seed` (default `20261010`). The CPU list defaults to
`6,7,4,5,2,3,0,1,14,15,12,13,10,11,8,9`, truncated to the worker count. The main
thread and auxiliary serial pool use CPU 6. Override these with `--worker-cpus`
and `--main-cpu`; observed affinity must match the request.

For arbitrary selections, `--config-file configs.json` replaces the built-in
configuration list. Each object has `name`, `variant` (`arithmetic` or `full`),
and `binary` (`baseline` or `candidate`). Optional fields are `replay`,
`construction`, `evaluation` (`auto`, `on`, `off`; default `auto`), `word`
(`auto`, `off`, `on`; default `off`), `group_cap` (`auto` or positive integer), and
`threads` (worker counts where that configuration should run). Names must be
unique within a variant. Include a configuration named `baseline` for matched
ratios. Example:

```json
[
  {"name":"baseline","variant":"arithmetic","binary":"baseline"},
  {"name":"selected","variant":"arithmetic","binary":"candidate",
   "replay":"on","construction":"on","evaluation":"off","threads":[4,16]},
  {"name":"baseline","variant":"full","binary":"baseline"},
  {"name":"group-eight","variant":"full","binary":"candidate",
   "replay":"on","group_cap":8}
]
```

`--trace` enables algebraic `--trace` or full `BITZ_FALCON_STAGE_TIMINGS=1`.
Run diagnostic and latency campaigns in different output directories because
logging affects timings. Stage events retain their span path and ancestry;
nested wall times overlap and must not be summed blindly. The script does not
change the packed-prefix policy.

The output format is `bitz/falcon-binder-experiments/v1`:

- `manifest.json`: immutable settings, ordered jobs, CPU information, runner
  and imported helper SHA-256 hashes, executable paths and SHA-256 hashes.
- `runs/<job>/attempt-NNN/command.json`: exact argument list and experiment
  environment overrides. Inherited `BITZ_`, `F2_`, `RAYON_` and `BD` controls are
  removed before applying these overrides.
- `stdout.jsonl`, `stderr.log`, `process.json`: raw output and process status,
  start time and wall duration, including failures.
- `stages.jsonl`: parsed diagnostic stage events from stderr.
- `runs/<job>/result.json`: atomically saved successful verification, normalized
  warmup/sample metrics, raw-output digest and source/proof identity.
- `summary.json`, `summary.csv`: completed counts, process timing medians and matched
  candidate/baseline ratios. These are written after completion or by the
  summary command below.

Every warmup and measured trial must verify, report the requested degree, batch,
security, seed and workers, and have positive finite metrics. Input digests and
source roots must match across configurations and worker counts for each
variant/seed. Bit-protocol proofs must have identical complete debug digests,
payload sizes and protocol IDs, including against baseline. Word-protocol proofs
form their own equivalence group and must have a distinct protocol ID and digest.
Equivalence groups use the reported actual protocol ID, so custom `word: auto`
configurations remain comparable when the candidate's production default changes.
This digest is the existing benchmark's deterministic debug-content hash, not a
portable proof serialization format.

Rerun the identical command to resume. Completed raw outputs are rehashed and
revalidated; incomplete attempts are preserved and retried. Modified binaries,
runner, settings or CPU affinity reject resume: use a new output directory.

```sh
python3 scripts/binder_experiments.py summarize --output results/binder-screen
```

Each process contributes one timing median to matched ratios; iterations within one
process are not treated as independent bootstrap samples. Timing ratios below
one and positive `speedup_percent_*` indicate improvement. The paired 95%
bootstrap interval uses the repository's `bench_statistics.py`; a confidence
interval crossing 1 is inconclusive, not proof of nonregression. Report both
prove-only and total prover latency. RSS uses the maximum reported high-water
mark across all warmup and sample trials in each process,
including setup and proof-digest formatting; compare it separately from live
prover allocations. A screening winner still needs independent holdout evidence.
