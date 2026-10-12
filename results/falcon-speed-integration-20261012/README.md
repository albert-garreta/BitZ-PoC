# Falcon preparation and statement hashing integration

This change ports the performance portions of `e568372fb`, `50c63e528`,
`5f2149a8f`, and `9b7b0d185` from `origin/falcon-grinding-opt2` onto the
existing working tree at `d865c9df5`, including its uncommitted v9 word binder.
The frozen baseline is that working tree, not plain `d865c9df5`.

**Result:** all three integrations are present and correctness/compatibility
checks pass. This is **not a zero-performance-regression result**: tiny serial
arithmetic proofs retain an approximately 1.3% total-prover slowdown. Larger
batches improve substantially. The final source exactly matches the candidate
used for the complete matrix and tests; diagnostic control edits were reverted.

## Changes and compatibility

- Hash canonical public statement bytes in bulk, using BLAKE3's parallel
  update for inputs at least 128 KiB. Field order, encodings, surrounding
  transcript fields, and protocol identifiers are unchanged.
- Replace allocating polynomial products with reusable, zero-padded `2N`
  NTT workspaces modulo 12289. All production consumers use the product
  modulo 12289; centered coefficients, reduced ring quotients, and exact
  integer norms are preserved. Explicitly supplied algebraic coefficients
  retain their original values.
- Consume generated SHAKE words directly and pack the supplied typed
  signatures without a serialization/deserialization round trip. The
  encoded signature remains part of the canonical statement digest.

The existing compact layout, quotient-free algebraic witness representation,
v9 word binder, grinding schedule, worker policy, and 1024 batch limit are
retained. `integration.patch` is the complete source delta against the frozen
baseline. Source hashes and binary hashes are recorded separately.

## Correctness validation

All 342 test executions passed:

| Suite | Passed |
|---|---:|
| Falcon library, including ignored large tests | 299 |
| Full Falcon profiles, including ignored qualification tests | 19 |
| Integer bridge | 16 |
| Grinding allocator | 3 |
| Upstream interoperability | 4 |
| Benchmark inputs | 1 |

The tests include independent schoolbook product and trace oracles, signed
and native integer boundaries, workspace reuse, distinct batch derivation,
SHAKE acceptance and byte order, typed/encoded source equivalence, malformed
witnesses, source padding, and transcript/proof tampering. The typed-source
test uses supplied signatures that differ from the trace copies to ensure
the supplied public signatures are still committed.

The native release benchmarks compile with `falcon-hybrid`, LTO and one
codegen unit. Correctness tests use release optimization, native CPU flags,
LTO disabled and 16 codegen units. Exact commands are recorded in
`library-test-commands.json` and `integration-test-commands.json`; summaries
are in `test-summary.json`. The serial library check with
`--no-default-features --features falcon` also passes.

An optimized native assembly inspection of the NTT found that remaining
integer divisions use public transform lengths and chunk dimensions; the
coefficient reductions by constant 12289 do not use integer division
instructions. This is a scoped kernel inspection, not a constant-time claim
for the whole prover or other architectures.

## Performance comparison

`qualify.py` compares frozen baseline and candidate binaries sequentially on
the AMD Ryzen 9 9950X3D. Each pair uses the same cached original-Falcon inputs,
CPU affinity, thread count, and native release build settings. Variant order
alternates. No compilation or correctness tests run during timing.

The primary matrix includes both full and algebraic Falcon, both degrees,
security targets 100/128, batches 1/3/32/513/1024, and one/eight workers:
80 configurations. Each uses seeds 42/43/44, one warmup and three measured
proofs per variant and seed. The runner requires identical input digests,
source roots, complete-proof debug digests, payload sizes and payload
breakdowns across variants. Debug digests are exact-build comparison aids,
not a stable wire format.

Ratios compare per-seed process medians. Bootstrap intervals resample paired
seeds; three seeds are only a screening sample. Total proving includes
witness preparation and commitment; proof-only timing excludes those phases.
Peak RSS measures the entire process, including cached input loading and
warmups. The largest observed relative increase was 4.2% in one small
eight-worker full-Falcon cell (about 1.7 MiB by the seed medians); these
process-level measurements do not establish an allocation improvement. The initial `identity-smoke` run overlaps compilation and is used
only for identity checks, never for performance conclusions.

Raw commands and trials are retained in each campaign directory, along with
its binary hashes, source provenance, pairing metadata, and summaries.
The initial matrix's observed increase threshold is 1%; flagged comparisons
require inspection and, where warranted, longer confirmation runs.


## Measured outcome

The main matrix completed all 240 pairs. Longer confirmation runs added 136
pairs, using eight fresh seeds (45–52), three warmups, and nine measured
proofs per variant. Across these 376 pairs, **5,184 proof executions verified**
and every baseline/candidate pair had identical input digests, source roots,
complete-proof debug digests, payload sizes, and payload breakdowns.

For batch 1024, across both degrees, both security targets, and one/eight
workers, the measured reductions were:

| Mode | Total prover time | Proof-only time | Verification time |
|---|---:|---:|---:|
| Arithmetic Falcon | 16.4–27.3% | 4.2–18.6% | 15.0–39.8% |
| Full Falcon | 2.2–6.6% | 0.6–5.1% | 24.4–43.5% |

See [the complete matrix](qualification/report.md) for all 80 configurations.
These are observed timings on this host, not guarantees for other inputs,
compiler versions, or machines.

### Remaining small-case performance regression

The longer [small arithmetic confirmation](small-confirmation/report.md)
found the following consistent serial changes:

| Degree | Batch | Security | Total prover | Proof only |
|---|---:|---:|---:|---:|
| 512 | 1 | 100 | +1.24% | +1.51% |
| 512 | 1 | 128 | +1.30% | +1.53% |
| 512 | 3 | 100 | +0.61% | +1.35% |
| 512 | 3 | 128 | +0.51% | +1.10% |
| 1024 | 1 | 100 | +0.68% | +1.50% |
| 1024 | 1 | 128 | +0.34% | +1.01% |
| 1024 | 3 | 100 | −1.07% | +1.13% |
| 1024 | 3 | 128 | −0.99% | +0.76% |

For Falcon-512/batch1 this is approximately 0.15 ms. The 128-bit total-prover
paired-seed bootstrap interval is +1.15% to +1.48%. Small eight-worker cases
were neutral or faster in the longer run. The initially noisy full-Falcon
1024/batch1/128-bit/eight-worker result also settled to total −0.3% and proof
+0.2% in its [confirmation](full-small-confirmation/report.md).

The first small-confirmation cell began about 1.6 seconds before the main
matrix's report computation finished: seeds 45–49 partly overlap that CPU
work. The clean seeds 50–52 still show +1.34% total and +1.60% proof for that
cell. All other confirmation cells started after report generation. The
runner now writes completion status only after summaries finish; the exact
original runner is preserved as `qualify-original.py`.

Two controlled experiments did not remove the slowdown and are **not part of
the final source**: restoring per-coefficient hashing for batches up to three
(`streaming-control`), and halving NTT scratch storage by returning the `u32`
transform buffer directly (`scratch-control`). Each was checked in 64 paired
serial-small runs. Their patches and provenance remain for reproducibility.

Diagnostic traces attribute most of the additional time to the existing BitZ
forest outside measured grinding loops. That subsystem's source is unchanged;
the traces do not establish a source-level cause. Compiler layout and allocator
history are hypotheses, not findings. See `diagnostic-traces/analysis.json`.
Instrumented timings are excluded from the performance conclusions above.

`final-validation.json` records the final status. All 729 recorded source
hashes match the original tested candidate after reverting the experiments;
only the 10 integration files differ from the frozen starting workspace.
Existing user edits, including the v9 word binder, remain intact.


## Sixteen-worker follow-up

The [16-worker results](16-worker-results.md) extend the same frozen-binary
comparison to sixteen physical cores. All 164 pairs matched proof/payload
identities, with 2,496 verified proof executions. At batch 1024, total arithmetic
proving improved 15–17% and full Falcon improved 6–7%, using a longer confirmation
for the noisy arithmetic 512/128-bit cell. A separate confirmed regression
remains for full Falcon-1024/batch 1/security 100: total +2.82%, proof-only +3.53%.

Arithmetic's existing optimized binding selector enables only one/eight
workers, so its sixteen-worker measurements use a different binding strategy;
these are not pure scaling measurements. The follow-up report includes the
comparison with earlier eight-worker results, method, memory observations,
confirmation intervals, and links to every raw campaign.


## Committed raw evidence

Raw benchmark and diagnostic logs, per-process commands, paired-result records,
build artifact records, and the NTT assembly are preserved in
[raw-evidence.tar.gz](raw-evidence.tar.gz). The [archive metadata](raw-evidence.json)
records its SHA-256, file count and byte counts. Every archived file was checked
against the original bytes before committing. Reports, aggregate summaries,
validation logs and provenance remain directly readable in Git.

From this results directory, run `tar -xzf raw-evidence.tar.gz` to restore the
raw paths referenced by the reports and analysis scripts. Existing unpacked
files remain locally available and are ignored by Git.
