# Matched F2Z / Limber MultiSwap campaign

This campaign compares the same canonical integer relation and integer
assignment across three prover configurations:

- F2Z with its virtual F2Z/Ligerito opening;
- Limber with Hyrax;
- Limber with Brakedown.

For every workload value `k`, each prover runs once with one Rayon thread and
once with all physical **performance** cores: six cells per `k`. The default
sweep is `k = 0, 1, 2, 4, 8`, so the default campaign has 30 cells. The sampling
policy is one excluded warmup and five measured trials per cell. On an Apple M1
Max the multi-worker count is eight, not the ten-core
performance-plus-efficiency total. Rayon workers are **not affinity-pinned** to
those cores; the worker count is merely set equal to the detected physical
performance-core count.

The workload is the wired MultiSwap/RSA cost-model selected by
`F2Z_BENCH_SHAPES=<k>` and `MSCFG=paper MATCHED_K=<k>`. `k=0` is the quotable,
source-backed reference configuration. Every `k>0` row adds modeled per-swap
`H_delta` groups and is labeled as scaling evidence, not an additional source
measurement. Modeled hashes and Poseidon are protocol-cost accounting; they
are not native hash executions. Reports repeat this disclosure so the result
cannot be mistaken for an application-level RSA or hashing benchmark.

## Preview the exact campaign

The dry run performs no compilation, benchmark execution, or artifact writes:

```sh
python3 scripts/run_matched_multiswap_campaign.py --dry-run --all-threads 8
```

Omit `--all-threads` on supported Apple Silicon machines to query
`hw.perflevel0.physicalcpu`, with `system_profiler` as the fallback. If the
machine cannot expose a performance-core count, the runner fails and asks for
an explicit value instead of silently benchmarking logical or efficiency
cores.

## Run

From the F2Z repository root:

```sh
python3 scripts/run_matched_multiswap_campaign.py --all-threads 8
```

Useful explicit controls are:

```text
--f2z-root PATH          default: this repository
--limber-root PATH       default: ../limber-impl
--output-dir PATH        default: PerfRuns/<UTC>-matched-multiswap-f2z-limber
--campaign-id ID         stable ID embedded in F2Z traces
--samples N              default: 5
--warmups 1              fixed at one for the matched F2Z interface
--k-values LIST          default: 0,1,2,4,8 (comma or whitespace separated)
--expected-digest K=HEX  repeatable hard expectation for one workload k
--rustflags VALUE        default: -Ctarget-cpu=native
--profiler PATH          canonical zk-proof-profiler validator/reporter
```

The two command shapes are intentionally fixed and instantiated six times per
workload `k`:

```text
F2Z:    cargo bench --bench multiswap --features unchecked
Limber: rustup run nightly-2026-07-01 cargo bench --bench multiswap_modp
```

The runner supplies each benchmark's trace path, workload `k`, backend, trial
count, thread count, and matched configuration through its documented
environment interface. Every cell receives a distinct trace file. Limber uses
a shared writable `build/limber-target` directory inside the campaign because
its repository may be mounted read-only.

Execution order is deterministic but counterbalanced: the starting
implementation rotates by the ordinal position of `k`, and the single/multiple
worker order alternates between successive `k` groups. Every cell stores its
`execution_index`, and the manifest records the complete order. This reduces
the chance that thermal or time drift is confused with one implementation or
one end of the `k` sweep; it does not replace host stabilization or affinity
control.

## Artifact layout

Existing run directories and traces are never overwritten.

```text
PerfRuns/<campaign>/
├── raw/                         # original zkperf.trace/v1 JSONL, one per cell
├── logs/                        # exact command plus merged stdout/stderr
├── metadata/campaign.json       # matched-multiswap-campaign/v1 manifest
├── traces/combined.jsonl        # byte-for-byte concatenation in manifest order
└── reports/
    ├── canonical/               # stock zk-proof-profiler report
    └── combined/
        ├── summary.json         # matched-multiswap-report/v1
        ├── metrics.csv
        └── intervals.html       # combined interactive comparison
```

The manifest records commands, the non-secret environment overrides, both Git
revisions and dirty flags, CPU/core detection, sampling policy, source trace
hashes, and the canonical statement digest. Raw JSONL and logs remain in place
even when a later cell is rejected.

## Acceptance gates

A campaign is rejected before aggregation when any of these conditions holds:

1. canonical `zkperf.trace/v1` structure or interval nesting is invalid;
2. a run is not `status="ok"` and `trace_complete=true`;
3. `validation.proof_verified` is absent/false, or any relation, digest,
   preflight, validity, or verification assertion is not true (informational
   booleans are not interpreted as assertions);
4. the warmup/sample counts or workload `k` differ from the manifest;
5. within the same `k`, the canonical statement domain or BLAKE3 relation
   digest differs between any F2Z, Limber Hyrax, or Limber Brakedown cell;
6. within the same `k`, a canonical integer-assignment digest is absent or
   differs across cells;
7. `parameters.input.workload_id` differs from the shared
   `multiswap-rsa-wired-cost-model-v1` identity, or the trace's implementation
   or measured Rayon thread count does not match its manifest cell;
8. normalized live/padded row/column dimensions or A/B/C nonzero counts differ
   between implementations at the same `k`;
9. a raw trace's recomputed SHA-256 differs from `cell.trace_sha256` in the
   manifest, or a required headline interval is absent.

Both BLAKE3 digests must use the canonical producer representation: exactly 64
lowercase hexadecimal characters with no `0x` or algorithm prefix.

The first accepted trace for each `k` establishes that group's canonical digest
when no matching `--expected-digest K=HEX` is supplied. Any later F2Z
invocation for that `k` receives the established digest through
`F2Z_MULTISWAP_EXPECTED_CONSTRAINT_DIGEST`; all 30 default files are also
checked again as one campaign before report generation. Different `k` groups
are expected to have different statement and assignment digests.

Completed artifacts may be revalidated or rerendered without rerunning a
benchmark:

```sh
python3 scripts/matched_multiswap_report.py validate \
  PerfRuns/<campaign>/metadata/campaign.json

python3 scripts/matched_multiswap_report.py report \
  PerfRuns/<campaign>/metadata/campaign.json \
  --out-dir /tmp/matched-multiswap-report
```

## Timing definitions

All totals are overlap-safe unions within one measured run. Only after forming
the per-run total does the reporter compute the median and Hyndman–Fan Type 7
P10/P90 across measured trials. Warmups never enter distributions.

| Report row | F2Z operation | Limber operation(s) |
|---|---|---|
| Witness generation | `multiswap-trace.witness_generation` | `multiswap.witness_generation` |
| Commit | `multiswap-trace.commit` | `multiswap.commit` |
| Projection / field reduction | `step2.project_prove` | `limber.projection` (contains sample-prime, reduction, and SpMV drilldowns) |
| PIOP / relation reduction | `step3.piop_prove` | `limber.piop` (contains outer sumcheck, inner setup/sumcheck, and evaluation recovery) |
| PCS opening proof | `step5.open_prove` | `limber.pcs.opening` |
| Total prover | `multiswap-trace.end_to_end_prove` | `multiswap.prover` |
| PCS total | overlap-safe union of commit and PCS opening | same union |
| Application total | overlap-safe union of witness and total prover | same union |
| Verify | `multiswap-trace.verification` | `multiswap.verify` |
| Verified trial | `multiswap-trace.verified_trial` | `multiswap.trial` |

The HTML headline section contains one six-column table per workload `k`; the
CSV carries explicit `workload_k` and `k_semantics` columns. The detailed selector shows a
chronological representative run (the measured sample nearest the cell's
median total-prover time), while each row's tooltip reports the cross-run
median and P10–P90. Tooltips render producer-supplied `math_latex`; stable
fallback equations cover the shared witness, Spartan outer and inner
sumchecks, LogUp, commitments, and the batched backend opening claim.

## Fast tests

Tests synthesize tiny canonical JSONL fixtures; they do not compile or run Rust
benchmarks:

```sh
PYTHONDONTWRITEBYTECODE=1 python3 -m unittest -v \
  scripts/test_matched_multiswap_campaign.py
```

The fixtures cover the default 30-cell dry-run plan, exact Type 7 quantiles,
combined HTML/math rendering, invalid-proof rejection, within-`k` digest
mismatch rejection, valid distinct digests across different `k` groups,
canonical-domain enforcement, normalized relation-shape parity, and manifest
trace-hash tamper detection.
