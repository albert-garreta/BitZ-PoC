# Falcon algebraic word binding — October 11, 2026

This change implements Item 4 for both Falcon-512 and Falcon-1024 algebraic
proofs. Production uses the v9 protocol, a word-domain binder and one weighted
BitZ bridge. The terminal merge uses the error budget released by the four
removed lane rounds. Compact physical commitments remain enabled; the separate
three-bridge compact-computation experiment remains disabled by default.

## Implementation

For each sixteen-bit word, the binder uses three uniform linear functions of
the original committed bits:

```text
D(w) = sum(l=0..13, 2^l*b(w,l)) - 2^14*b(w,14)
H(w) = b(w,15)
C(w) = sum(l=0..14, b(w,l)).
```

All activity, side and coefficient-position masks belong to the public weight
functions. The ring/norm coefficient weights multiply `D`; the slack decoder
and spare-lane padding weights multiply `H`; the inactive-instance padding
weight multiplies `C`. The raw word functions remain defined on every word,
including inactive instances and unused lanes.

Only the main `[Lambda,D]` pair is dense. The prover allocates that pair after
the first word fold, using native signed coefficients and wide mixed
accumulators in the initial round. During the coefficient and side rounds,
`AggregateSpare` collects lane-15 contributions across instances into at most
32 positions: `U[j] = sum_i slack_weight[i]*H[i,j]` and
`V[j] = sum_i H[i,j]`. After those rounds, the prover projects the original
sparse `H` entries to instance values and folds those values through the
instance rounds. Public lane-15 weights remain factored.

When the batch fills its logical capacity, the public `C` weight vanishes and
the table and terminal value are omitted. Otherwise, the prover delays `C`
until the instance rounds, evaluates the local popcounts once and folds a
capacity-sized table. The deterministic parallel source scan rejects
malformed padding; the verifier still checks the complete padding relation
and authenticates the uniform raw word functions.

The binder absorbs its two or three terminal values before sampling a fresh
prime-field `zeta`. It merges them with `[1,zeta]` or `[1,zeta,zeta^2]` and
opens the combined linear function through one weighted, two-limb BitZ
bridge. The bridge verifier derives the canonical row weights from the word
point and combined lane decoder. Its equality-opening wrappers retain their
existing transcript behavior for other protocols, including full Falcon.

The binder removes four lane rounds. At batch 1024 this changes Falcon-512
from 24 to 20 rounds and Falcon-1024 from 25 to 21. R7 already used a
word-sized dense suffix on its selected prefix-four paths; this change does
not newly halve that suffix domain. It removes the prefix work and its four
messages/challenges while avoiding three dense pairs of word tables.

The production v9 digest binds the decoder, terminal count and new grinding
schedule. The security ledger uses a degree-two word sumcheck over
`log2(N)+1+log2(capacity)` variables and a degree-one terminal merge when all
instances are active, or degree two otherwise. Production retains the old
bit-domain binding difficulty `g`. For `k` word variables and merge degree
`e` in `{1,2}`, the terminal difficulty `h` satisfies both its individual
security bound and `h >= max(0, g + log2(e) - 3)`. Consequently,

```text
2k/2^g + e/2^h <= (2k+8)/2^g.
```

The binder and terminal merge therefore consume no more error budget than
the old bit binder. The binary-field and PCS allocation uses this combined
ledger; compact-computation grinding remains unchanged.

## Validation

The recorded debug algebraic run completed with 108 passing tests, two failed
replay-fixture tests and two explicitly ignored large tests. The old fixture
rewrote a transcript domain that the new unground path no longer absorbed,
making the attempted replay mutation a no-op. Updating only that `cfg(test)`
fixture made both targeted replay tests pass. Thus all 110 individual
nonignored algebraic tests passed across that full run and targeted rerun.

- [Algebraic test log](algebraic-tests.txt): both degrees, dense mathematical
  references for word rounds and terminals, raw-mask and weighted-opening
  checks on malformed padding, terminal/nonce tampering, end-to-end tests,
  and complete proof/transcript parity across worker counts.
- [Replay-fixture rerun](replay-tests.txt): both corrected domain-replay tests
  passed.
- [Integer bridge tests](bridge-tests.txt): all 16 passed, including arbitrary
  public row weights, malformed inputs and exact equality-wrapper proof and
  transcript compatibility.
- [Release checks](release-checks.txt): both explicit large algebraic tests
  passed, each covering batches 32/512/513/1024 and targets 100/128. Three full
  Falcon compatibility tests also passed: roundtrip/tampering, the 128-bit
  roundtrip and shared-prime complete-proof/protocol binding.
- [Final word-kernel tests](aggregate-tests.txt): all eight passed after the
  aggregate lane-15 optimization. Both degrees cover dense reference rounds
  and terminals, Boolean challenges, malformed-padding weights and bridge
  claims, and deterministic parallel source scanning.
- [Budget-preservation tests](budget-tests.txt): all four passed for both
  degrees. Exact integer arithmetic checks the combined binder/terminal
  budget and required difficulties for every batch 1–1024 at targets 100/128;
  it also checks that compact grinding is unchanged. Prepared configurations
  check the composed ledger and transcript binding of the schedule.
- [Final end-to-end tests](funded-e2e-tests.txt): both degree-specific
  roundtrip/tampering tests passed with the final kernels and schedule,
  covering batches 1/3/8/9/16/65 at both security targets.

The full algebraic suite and release checks predate the final equivalent
kernel optimizations: cached public masks, deterministic parallel source
scanning and the aggregate lane-15 prefix. The release checks also predate
the parallel verifier coefficient evaluation, which the later debug
worker-parity tests cover. These broad runs also predate the final grinding
budget adjustment. The eight targeted word-kernel tests check the final
kernels, the four budget tests check the adjusted schedule, and the two
end-to-end tests exercise their integration. A complete
suite rerun on that combination has not been recorded here.

At 100-bit security, the schedule remains unground, and matching native
benchmark cases must retain the initial v9 proof digests. At 128 bits, the
funded terminal-merge schedule changes transcript challenges, so equality
with earlier v9 proof digests is not required.

## Benchmark method and provenance

[run.py](run.py) compares the archived R7 qualification binary with the native
v9 word-binding example. The primary campaign uses these four shapes:

| Degree | Batch | Security | Workers |
|---|---:|---:|---:|
| Falcon-512 | 1024 | 100 | 1 |
| Falcon-1024 | 1024 | 100 | 1 |
| Falcon-512 | 1024 | 128 | 8 |
| Falcon-1024 | 1024 | 128 | 8 |

Each shape uses seeds 42/43/44, one warmup and two measured trials per seed
and binary: six measured samples per variant. Execution order alternates by
seed. The script sets `RAYON_NUM_THREADS=T`, assigns worker CPUs `0..T-1`
and main CPU 0, and runs the binaries sequentially. Primary timing runs do
not enable tracing. Every trial must verify. Before producing a summary,
the script asserts equal input digests and source roots for the paired
variants and trials at each shape and seed.
For each 100-bit word-binding run with a matching record in
[initial/large](initial/large/), it also asserts equality of
`proof_debug_digest` with that initial v9 implementation. This compares
kernel optimizations under the unchanged 100-bit schedule. The final
128-bit schedule and all v9 proofs relative to R7 have changed transcripts.

The summary takes the median of all six measured samples per variant for
proving time, total prover time, verification time, process peak RSS and
stored proof payload. Percent change is `100*(word/R7 - 1)`. Process peak
RSS is a process-level high-water mark, including warmup, rather than an
isolated binder allocation measurement. Total prover time includes witness
preparation and commitment; proving time excludes those phases.

[provenance.json](provenance.json) records the base commit, source hashes,
Linux/x86-64 platform and native release build flags. The
[R7 provenance](../arithmetic-falcon-compact-compute-20261011/candidate-r7-provenance.json)
identifies the archived baseline binary and its source snapshot. The
[campaign manifest](large/manifest.json) records both binary hashes and the
exact shapes, seeds and iteration counts. Per-run JSONL and stderr files are
preserved in [large](large/); warmup rows remain in the raw JSONL but are
excluded from the medians. The script can additionally run boundary shapes
or traced diagnostics; those are separate from the primary campaign and
must be identified separately when reported.

## Performance results

The [final primary summary](large/summary.json) records these medians at
batch 1024. All 24 executions verified and the paired inputs and source
roots matched. The six 100-bit v9 executions also retained their initial
v9 proof digests.

| Degree | Security | Workers | Prove ms, R7 → word | Prove change | Total prover change |
|---|---:|---:|---:|---:|---:|
| Falcon-512 | 100 | 1 | 168.47 → 146.68 | −12.93% | −10.60% |
| Falcon-1024 | 100 | 1 | 327.71 → 289.29 | −11.73% | −8.95% |
| Falcon-512 | 128 | 8 | 43.28 → 41.98 | −3.00% | −2.51% |
| Falcon-1024 | 128 | 8 | 78.93 → 77.35 | −2.00% | −1.35% |

Verification time decreased 1.40% for Falcon-1024 at eight workers; the other
three verification medians changed by at most 0.34%. Process peak RSS fell
3.43% for Falcon-512 at one worker and by 0.17–0.24% for the other shapes.
The arithmetic payload is 192 bytes smaller at 100-bit security and 216
bytes smaller at 128-bit security for these full batches. Total proof
payload medians fell 0.02–0.38%; the changed transcript also changes PCS
query patterns, so the arithmetic reduction alone does not predict the
total payload change for an individual proof.

These six samples across three seeds per variant provide a native comparison
on four shapes, not a full performance qualification. The measured
eight-worker gains are modest and need broader sampling to establish their
stability. Other security/worker combinations and hardware are outside the
primary campaign.

The [stage diagnostics](stage-summary.json) locate the large one-worker saving
in the binder; eight-worker stage changes are smaller. These are seed-42-only
medians of two measured trials, excluding warmup. Tracing adds overhead, so
these timings support attribution rather than performance qualification.

| Degree | Security | Workers | Binder ms, R7 → word | BitZ ms, R7 → word |
|---|---:|---:|---:|---:|
| Falcon-512 | 100 | 1 | 59.91 → 40.10 | 53.66 → 53.07 |
| Falcon-1024 | 100 | 1 | 117.81 → 80.90 | 100.76 → 99.36 |
| Falcon-512 | 128 | 8 | 9.49 → 8.83 | 16.24 → 16.25 |
| Falcon-1024 | 128 | 8 | 17.89 → 17.82 | 26.45 → 25.95 |

### Boundary batches

The [boundary summary](boundaries/summary.json) covers batches 3 and 513,
which both require the third popcount terminal. It uses the same three
seeds and six measured samples per variant and shape. All 96 executions
completed with 288 verified proofs, including warmups; paired input digests
and source roots matched.

The table reports proving-time changes, with negative values indicating
less time:

| Degree / batch | 100 bits / 1 worker | 100 bits / 8 workers | 128 bits / 1 worker | 128 bits / 8 workers |
|---|---:|---:|---:|---:|
| Falcon-512 / 3 | −6.36% | +0.89% | −3.72% | −4.58% |
| Falcon-512 / 513 | −11.27% | −4.66% | −11.24% | −3.73% |
| Falcon-1024 / 3 | −6.11% | −3.82% | −7.26% | −2.67% |
| Falcon-1024 / 513 | −9.82% | −0.75% | −10.03% | +0.57% |

Two measured shapes were slower by less than 1%: Falcon-512 batch 3 at
100 bits/eight workers and Falcon-1024 batch 513 at 128 bits/eight workers.
These results do not establish a gain for every configuration; small
changes need more sampling to distinguish a repeatable effect from timing
variation.

All recorded 128-bit primary and boundary cases reported the same composed
security value as their paired R7 cases, including all 24 word-binding
128-bit boundary executions. The final schedule funds the new merge
without reducing those reported security totals.

Earlier primary, stage and boundary campaigns remain under
[per-group-schedule](per-group-schedule/). Their 128-bit measurements use a
different grinding schedule and do not qualify the final configuration.
