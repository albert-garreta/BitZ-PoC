# Outer sumcheck versus a3450385

Baseline: `a3450385e2a90115add370a3c05e5127db0b3a87`, before outer-sumcheck unification. Current: `ef7a8935` plus the retained generic first-fold parallelism and this benchmark. This comparison must not be confused with the earlier generic-before/generic-after optimization result.

## Result: faster-everywhere qualification failed

The public generic API does **not** match the legacy specialized kernels across these workloads. Its u64 skip path improves substantially, but ordinary multiplication is slower for all three widths and u32 skip regresses badly. This is a protocol/API unification, not yet a performance-equivalent replacement for every specialized entrypoint.

Representative medians below use confirmation session 2, 131,072 rows, 10 threads, 21 measured proofs. The CSVs preserve both sessions and every skip width.

| Multiplication | Ordinary: historical → generic | Skip K=3: historical → generic |
| --- | ---: | ---: |
| u32 × u32 | 1.756 → 2.538 ms (45% slower) | 1.053 → 13.028 ms (12.4× latency) |
| u64 × u64 | 1.931 → 2.545 ms (32% slower) | 25.336 → 14.266 ms (44% lower latency) |
| u128 × u128 | 2.619 → 3.229 ms (23% slower) | 25.595 → 25.689 ms (approximately unchanged) |

Production-compatible paths are much closer to the historical code, but a clean universal no-regression claim is also unwarranted: the primary run flagged three >3% slowdowns, none repeated in a 101-sample focused follow-up; the subsequent per-case alternating full matrix flagged u32 ordinary at 32,768 rows (6.6% / 13.7% slower), which had inconsistent signs in earlier runs. These are retained as unresolved timing warnings, not erased by a favorable average. The paired matrix found only 1/60 production-compatible cases at least 5% faster in both sessions. All proof and transcript fingerprints matched.

- `confirmation/comparison.csv`: full 60-case matrix, historical/production/generic, two sessions, 21 measured repetitions; 7,560 measured verified proofs.
- `production-followup/comparison.csv`: eight focused production cases, 101 repetitions, opposite process order.
- `paired-production/comparison.csv`: full production matrix with revisions alternating for each individual case, 21 repetitions; 5,040 measured verified proofs.
- Each directory's `gate.json` records failures and cases without a demonstrated improvement. The public generic API fails both the no-regression and faster gates.

## Code-level causes of the generic gap

1. Ordinary first-round coefficients project four A/B values per pair with `reduce(mul_wide(one, integer))`, perform field subtractions and multiplication, and then accumulate. Native kernels compute exact integer differences/products before the weighted delayed MAC.
2. Generic first folding does not return the next round's coefficients; continuation is called with `pending=None` and rereads the folded tables. The native fold computes those coefficients while producing the tables.
3. Generic prefix interpolation uses field-weighted dot products with short-lived accumulators. For K=3, six finite nodes require three reductions each, and the leading term requires two more: 20 reductions per eight-row block for message construction. Native u32 interpolation/residual arithmetic uses bounded signed integers and delays modular reduction across equality buckets.
4. Generic prefix message construction and prefix folding are serial. The retained native u32 path parallelizes them. The earlier parallel-prefix prototype was rejected after small-case regressions; this benchmark exposes the much larger remaining deficit relative to the historical specialized baseline.
5. Generic integer pair folding currently takes the general remainder path. The specialized u32 native fold uses a bounded Montgomery-reduction/conversion sequence.

The u64/u128 skip comparison has a different historical baseline: field projection plus a field skip kernel. The generic path avoids materializing those projected tables and reuses prepared interpolation weights. Wider u128/Uint<4> MACs do more limb work; whether that precisely offsets avoided preparation requires phase-level attribution. The totals alone cannot assign percentages of time to these causes. No dynamic trait dispatch cost has been demonstrated.

## Measurement contract

- Apple M1 Max, Rust 1.98.1, bench profile, LTO, one codegen unit; both builds use the same feature flags (`parallel`, `bench-internals`). The baseline uses its own `vendor/field` and Flock sources from the requested commit.
- Each table row proves a full-width unsigned multiplication: u32×u32→u64, u64×u64→u128, or u128×u128→Uint<4>. Fixtures include zero, one, and maximum operands among deterministic uniform values.
- Ordinary sumcheck and skip K=1,2,3,4, over 4,096 / 32,768 / 131,072 / 524,288 rows. Each method uses its valid rowwise-zero optimization where available. Protocol transcripts and proofs must match the historical method exactly.
- The timer includes equality-factor setup, required field projection, output allocations, and the complete outer protocol. It excludes fixture construction, field-context preparation, reusable generic skip preparation, verification, fingerprinting, and printing. New generic skip preparation is explicitly amortized across proofs; the old API has no matching preparation hook.
- One excluded warmup and 21 measured proofs per case, two process sessions with reversed implementation order. All runs are sequential: no concurrent compilation, allocator instrumentation, or tracing.
- Every proof verifies and the canonical proof coefficients, terminal evaluations, and continuing transcript digest are compared across versions and variants. Timing is analyzed only after this parity check passes.

## Compared paths

| Case | Historical baseline | Current production-compatible path | Current generic API |
| --- | --- | --- | --- |
| Ordinary, all widths | `raw_monty::prove_outer_native_raw` on borrowed native/split inputs | `outer::arithmetic::prove_native_zerocheck` on the same native/split inputs | `prove_outer_zerocheck_from_slices` |
| u32 skip | Native prefix message and prefix fold, then `prove_outer_field_raw` | `outer::native_skip::prove_native_skip` | `prove_outer_zerocheck_with_skip_from_slices` |
| u64/u128 skip | Project the inputs, then `prove_univariate_skip_outer_sumcheck_with_reducer` | Same projection, then `prove_field_skip_with_factors` | Mixed integer inputs consumed directly |

“Production-compatible” means the retained supported kernels. The high-level u64/u128 multiplication protocols currently select ordinary outer sumcheck and reject a skip selection with `UnsupportedKernel`. For wide inputs this benchmark exercises the available field-based outer skip kernel directly, not a high-level F2Z skip integration. It does not imply that a specialized native wide-integer skip kernel exists. Projection uses identical parallel conversion code in the two version adapters and is charged to both.

The benchmark module is compiled only under `bench-internals`; no arithmetic is copied into the current adapter. The legacy adapter reproduces the historical u32 skip composition because that code was embedded in the old PIOP driver. `prepare_baseline.py` adds only the shared driver, its historical adapter, and benchmark registration to an isolated source archive. It does not edit the historical kernels.

## Regression gate

`compare.py` writes per-case medians and P10–P90 sample deciles to `comparison.csv`. These deciles are not confidence intervals. `gate.json` separately records:

1. Repeatable regressions: more than 3% slower in **both** process sessions.
2. Cases that were not at least 5% faster in **both** sessions.

Unchanged/noisy timing is not classified as a demonstrated speedup. `--gate no-regression` exits unsuccessfully for (1); `--gate faster` exits unsuccessfully for (2). The default `--gate report` preserves findings without stopping at the first failed case. `--generic` includes the public generic API in the comparison and gate; omitting it checks retained production-compatible paths only.

## Reproduce

```sh
cargo +1.98.1 bench --offline --bench outer_regression --features bench-internals --no-run
# Copy the executable Cargo prints to /tmp/outer-current before building baseline.
python3 experiments/outer-regression-a3450385/prepare_baseline.py
# In the printed checkout, with the same target directory if desired:
cargo +1.98.1 bench --offline --bench outer_regression --features bench-internals --no-run
# Copy that executable to /tmp/outer-a3450385.
python3 experiments/outer-regression-a3450385/compare.py \
  --baseline /tmp/outer-a3450385 --current /tmp/outer-current \
  --out /tmp/outer-comparison --threads 10 --reps 21 --generic --paired --gate faster
```

The shared-target build must actually recompile each source tree. `prepare_baseline.py` touches its isolated library root to prevent stale archived timestamps from reusing a later build. When returning to the main tree, touch `src/lib.rs` before rebuilding if Cargo reused the historical artifact. Verify the source path in the compilation log, and freeze each executable before the next build overwrites it.

Small correctness runs also cover 1, 2, 4, 8, 16 and 16,384 rows, with all valid skip widths. x86 performance is not measured here.
