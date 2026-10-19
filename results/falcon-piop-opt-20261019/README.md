# Algebraic Falcon-512: shrinking the word-level PIOP (2026-10-19)

Branch `falcon-piop-opt` (from `falcon-pcs-levers` 0c5d7e130). Setup G throughout:
32,768 Falcon-512 signatures at 128 bits, `BITZ_FALCON_GRINDING_MODEL=per-site
BITZ_FALCON_PRIME=small BITZ_ARITHMETIC_RING=keyed BITZ_FALCON_BRIDGE_ROWS=16`,
inputs from the cached fn-dsa corpus (seed 42). Apple M5 on AC power, 1-thread
runs at 4.4 GHz (`cycles elapsed / user`), 10-thread runs at 3.8 GHz, nothing
else running. `run.zsh <bin> <tag>` does one 1-thread traced run (two proves)
and one 10-thread run (warm-up + five proves); `summarize.py <tags>` tabulates
per-prove span times (ms) and medians.

## Question

The single-thread flamegraph of setup G had the ring and norm arguments at
1.74 s against 0.88 s for the bit-to-integer bridge (the grand product): the
word-level PIOP cost twice the grand product, for a statement that is one
linear ring identity and one sum of squares per signature.

## Builds

| Build | Change | Proof digest |
|---|---|---|
| baseline | falcon-pcs-levers tip | 160f85fb8989 |
| A | keyed-ring recurrence interleaved over 8 signatures; 32-bit quotient taps | 160f85fb8989 (identical) |
| B | **merged norm**: `BITZ_ARITHMETIC_NORM=merged` folds the square sums into the word binder as a cubic term over the instance variables; no integer-outer sumcheck | 2c56f89f6b2a |
| C | B + s1 ring weights kept factored (one rank per projection point), parallel coefficient validation | 2c56f89f6b2a (identical to B) |
| D | C + branchless, vectorizable NTT butterflies in the exact quotient; one fewer accumulation per word pair in the merged first round | 2c56f89f6b2a (identical to B) |

## Results

Milliseconds per prove; one-thread rows are span times from the traced run,
the last rows are medians of the untraced proves. Full table: `summary.txt`.

| | baseline | B merged norm | C + factored s1 | D + NTT |
|---|---:|---:|---:|---:|
| 1 thr: keyed ring (weights / quotient) | 712 (458 / 251) | 685 (434 / 247) | 547 (295 / 248) | 472 (296 / 172) |
| 1 thr: integer-outer sumcheck | 549 | - | - | - |
| 1 thr: word binder (of which grinding) | 968 (140) | 884 (211) | 937 (211) | 913 (211) |
| 1 thr: binder construction | 154 | 14 | 7 | 7 |
| 1 thr: bridge GKR forest | 849 | 827 | 846 | 838 |
| 1 thr: prove | 3,488 | 2,832 | 2,761 | 2,655 |
| 1 thr: witness + commit + prove | 3,821 | 3,161 | 3,093 | 2,986 |
| 10 thr: prove | 697 | 616 | 591 | 583 |
| 10 thr: witness + commit + prove | 770 | 687 | 663 | 656 |
| 10 thr: verify | 20.8 | 19.3 | 19.6 | 20.1 |
| peak RSS, 10 thr (MiB) | 2,481 | 2,090 | 1,830 | 1,834 |
| proof bytes | 593,232 | 593,096 | 593,096 | 593,096 |

Word-level PIOP without its grinding, one thread: 1,728 ms at the baseline
(ring 712 + norm 335 + binder 681) against 849 for the forest; 1,225 ms in D
(ring 472 + binder 702 + instance point 50) against 838. The remaining PIOP is
the two dense s2 weight chains (296), the exact quotient (172), and the binder
passes over 2^25 words: native first round 96, first fold 241, nine strided
dense rounds 288, auxiliaries about 80.

Ten threads, build D, span times of a traced prove (`runs/t10trace-D.err`,
688 ms traced against 583 untraced): forest 262 (bitz 291), word binder 196
(dense rounds 51, first fold 47, round grinding 42, first round 19), keyed
ring 104 (weights 72, quotient 31), instance point 9, commit 80, PCS 44. At
ten threads the PIOP without grinding is 268 ms against 262 for the forest;
the same split of the baseline configuration was 400 against 274.

## Notes

- The merged binder's messages have four coefficients; the ten coefficient and
  side rounds are quadratic (the verifier rejects a nonzero cubic term there)
  and the fifteen instance rounds are cubic. The two halves are ground at
  their own difficulty (18 and 19 bits here) through `SplitRoundBoundary`.
  This removes the 25 norm rounds' grinding sites: prime-field grinding per
  prove drops from 25 x 2^19 + 25 x 2^18 to 15 x 2^19 + 10 x 2^18 hashes.
- The norm weight `eta * eq(rho, instance)` is constant inside a signature, so
  the coefficient rounds accumulate each instance's squares in a deferred
  accumulator and multiply once per instance and round.
- A prover-batched X^N+1 quotient to make the s2 weights rank-1 was examined
  and rejected as unsound (a single-CRT-component residual passes with
  probability 1/q once the batched identity is reduced modulo X^N+1). The
  dense s2 weights, two projection points at this prime size, are inherent to
  the compact-quotient ring check; see the session notes.
- Interleaving the recurrence does not help: each step already has two
  multiplications, so it runs at multiplier throughput, and the mixed-width
  product `reduce(mul_wide)` is slower than a Montgomery multiply by a
  tabulated residue.
