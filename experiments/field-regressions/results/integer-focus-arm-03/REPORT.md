# Integer kernel optimization results

Both targeted prototype regressions are removed on the Apple M1 Max. All **28 selected cases pass** the existing 1% regression gate, with 217 required variant cases complete. The selected APIs allocate zero bytes in the measured hot paths. These are isolated arithmetic experiments; production integration and x86 performance remain pending.

## Representative paired comparisons

Times below are median batch times in microseconds. Ratios use the median of the paired process ratios, so rounding or pooled medians can give slightly different quotients. The original slower prototypes are measured in the same executable and processes as the replacements.

| Operation (1,024 terms/products) | Production baseline | Old slower prototype | Selected | Selected / production | Speedup over old prototype |
|---|---:|---:|---:|---:|---:|
| Two-limb MAC, signed-16-bit fixture | 0.689 | 1.612 | 0.657 | 0.952× | 2.46× |
| Two-limb MAC, full 128-bit inputs | 0.687 | 1.611 | 0.656 | 0.953× | 2.45× |
| Four-active-limb products, prepared | 21.329 | 65.457 | 6.778 | 0.318× | 9.67× |
| Four-active-limb products, including validation | 21.329 | 65.457 | 8.613 | 0.403× | 7.64× |
| Nine-active-limb products, prepared | 60.005 | 66.172 | 54.805 | 0.914× | 1.21× |

## Final implementation

- **Two-limb MAC:** seed two independent native `u128` accumulators with the first pair of products, accumulate subsequent pairs, then merge and handle the odd tail. Empty and one-term batches return directly. This avoids initial additions to zero and the previous fused kernel’s long high-limb dependency chain. Forwarding functions inline away. The generic experimental `integer::mac::<2, false>` entry uses this implementation.
- **Four-active-limb products:** validate the caller’s public unsigned bound once, then execute the existing fixed-four-limb kernel over nine-limb storage. Direct-output and Comba alternatives did not improve the development result. All 18 output limbs are written, including the ten upper zeros.
- **Nine-active-limb products:** write the first multiplication row directly into the output and build subsequent rows over initialized limbs. This removes the temporary 18-limb result and initial zeroing.
- **Batch interface:** `PreparedProducts::try_new` validates lengths and the declared public bound, retains immutable input borrows, and exposes repeated `execute`. `multiply_batch` includes preparation on every call. Both selected APIs are independently gated. No width is inferred from operand values.

The MAC computes the full-width dot product modulo 2^128. It is also exact signed arithmetic when a caller-established bound excludes overflow. The unchecked two-limb path no longer has the old diagnostic fixture’s arbitrary 2^20-term limit. Other widths and checked diagnostic paths retain the previous arithmetic.

## Coverage and validation

Each process checks every timed input against independent BigUint arithmetic; signed-16-bit fixtures additionally use an exact BigInt oracle. Edge checks cover empty batches, odd tails, all-one limbs, carries, signed boundaries, invalid public bounds, mismatched lengths, and reuse of output buffers. All correctness checks passed. The Python regression-policy suite passed **17 tests**. An independent review found no arithmetic or API integration issues.

| Selected family | Sizes | Median time / production range | Gate |
|---|---|---:|---|
| MAC: signed16 and full128 | 1, 3, 7, 16, 17, 1,024, 65,536, 1,048,576 | 0.818–0.953× | 16/16 pass |
| Four-limb: prepared | 16, 1,024, 65,536 | 0.317–0.326× | 3/3 pass |
| Four-limb: validation + execution | 16, 1,024, 65,536 | 0.374–0.434× | 3/3 pass |
| Nine-limb: prepared | 16, 1,024, 65,536 | 0.913–0.934× | 3/3 pass |
| Nine-limb: validation + execution | 16, 1,024, 65,536 | 0.914–0.937× | 3/3 pass |

Compared with the retained bare fixed-four-limb control, prepared execution has median ratios **0.998, 1.008, and 0.998** at 16, 1,024, and 65,536 products. The kernel itself is unchanged; the prepared API adds validation ownership and public-bound dispatch. This secondary comparison does not establish a 1% tail-latency bound against the bare kernel. Its [paired statistics](prepared-vs-bare-four-limb.json) are retained separately; the selected gate above is against production.

## Measurement policy and scope

This final run uses seed offset 3141593, five fresh processes, 32 paired samples per case, shuffled variant order, one caller thread, and `-C target-cpu=native`. Sources, candidate selection, manifest, lockfile, and executable were frozen before confirmation. Rust release settings are optimization level 3, fat LTO, one codegen unit, and debug information. Compiler details and hashes are in [metadata.json](metadata.json).

Automatically retried cases: none. Any retry uses the predeclared five-process, 64-sample policy and replaces the initial result for that case. The final 1% gate requires median and P95 upper confidence bounds at most 1.01, every process median at most 1.01, correct output, and no added allocations. Intervals are pointwise 95% hierarchical paired bootstrap intervals; P95 is for timed-batch averages, not individual-call tails.

The earlier arm-01 checkpoint exposed forwarding-call overhead at one term. Arm-02 removed that overhead but left one three-term case inconclusive. This final source changes the arithmetic initialization itself and uses a fresh seed; earlier reports remain intact. No cutoff was relaxed and the original slow controls remain visible.

Hardware: Apple M1 Max, 10 CPU cores, 64 GiB memory, macOS ARM64. The runner’s sandboxed hardware query was denied; the host observation was recorded separately in the earlier campaign on this machine. Core placement, temperature, and unrelated host load were uncontrolled. There are no x86 measurements or full-prover speedup claims. These tests do not certify constant-time behavior.

## Evidence

- [Gate result](gate.txt) and [all variant measurements](measurements.md).
- [Machine-readable statistics](summary.json), [frozen manifest](required_cases.json), and raw CSV/logs under `initial/` and optional `retry/`.
- [Selected ARM kernel assembly](selected-kernels-arm64.asm).
- [Frozen source and executable archive](frozen-inputs.tar.gz). Archive members are verified against the source and executable hashes in metadata; the archive is hashed in `attachments-sha256.json`.
- [API and reproduction instructions](../../INTEGER_FOCUS.md).
