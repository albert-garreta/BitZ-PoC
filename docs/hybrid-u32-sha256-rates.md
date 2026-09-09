# Hybrid vs all-Binius, with Binius64 at rate 1/2 and 1/8

`benches/hybrid_u32_sha256` proves `N` multiplications `x*y = z + 2^32*w` (four
u32 limbs) together with `M` chained SHA-256 compressions. SHA-256 is
Binius-arithmetized in **every** mode; the modes differ in how the two branches
are proved:

| mode | multiplication | SHA-256 | proof |
| --- | --- | --- | --- |
| `hybrid` | BitZ/F2Z | Binius64 | one, shared opening |
| `separate` | BitZ/F2Z | Binius64 | two independent proofs |
| `all-binius` | Binius64 | Binius64 | one |

Binius64's FRI inverse rate was previously hardcoded to `1/2`. It is now
`F2Z_HYBRID_BINIUS_LOG_INV_RATE` (default `1`; `3` selects rate 1/8), matching
the knob the native-multiplication tables use, and
`F2Z_HYBRID_BINIUS_SECURITY_BITS` (default `112`) exposes the FRI component
target. Both apply to the native Binius circuit, i.e. to `all-binius` and to
the Binius half of `separate`; `hybrid`'s internal geometry is set in
`src/hybrid/`.

Apple M5, 24 GB, 8 rayon threads, `-C target-cpu=native`, medians of 11
verified iterations, one process per case. Run:
`PerfRuns/2026-09-09T12-45-12Z-hybrid-rates` (peak RSS and swap counters in
`peak-rss-and-swap.tsv`).

| N mul : M SHA | scheme | witgen (ms) | prover (ms) | verifier (ms) | proof (KB) | peak (MiB) |
| ---: | --- | ---: | ---: | ---: | ---: | ---: |
| 2^15 : 2^7 | hybrid | 0.01 | 36.6 | 3.91 | 272.4 | – |
| | separate | 0.36 | **28.9** | 4.11 | 316.9* | – |
| | all-binius rate 1/2 | 1.26 | 44.2 | 2.78 | 361.0 | – |
| | all-binius rate 1/8 | 1.25 | 49.5 | **2.58** | **265.3** | – |
| 2^17 : 2^9 | hybrid | 0.03 | 79.0 | **5.97** | 339.8 | – |
| | separate | 1.50 | **64.1** | 6.28 | 404.1* | – |
| | all-binius rate 1/2 | 4.99 | 115.4 | 7.57 | 437.7 | 2394 |
| | all-binius rate 1/8 | 4.98 | 128.2 | 7.32 | **314.2** | 2452 |
| 2^19 : 2^11 | hybrid | 0.14 | 230.4 | **12.51** | 423.4 | **2856** |
| | separate | 6.17 | **183.9** | 14.47 | 521.8* | 2745 |
| | all-binius rate 1/2 | 19.92 | 393.2 | 34.12 | 517.9 | 8921 |
| | all-binius rate 1/8 | 19.99 | 444.2 | 33.80 | **362.5** | 9620 |
| 2^20 : 2^12 | hybrid | 0.27 | 415.0 | **19.48** | 467.0 | **5580** |
| | separate | 12.05 | **331.3** | 22.08 | 582.2* | 5738 |
| | all-binius rate 1/2 | 41.74 | 771.4 | 69.03 | 577.8 | 14985 |
| | all-binius rate 1/8 | 42.11 | 891.9 | 69.64 | **389.6** | 16578 |
| 2^21 : 2^13 | hybrid | 0.54 | 844.0 | **34.47** | 505.5 | **11320** |
| | separate | 27.48 | **683.8** | 43.06 | 629.0* | 11780 |
| | all-binius rate 1/2 | 186.90 | 4587.7 | 828.68 | 631.0 | 17476 |
| | all-binius rate 1/8 | 192.51 | 4718.2 | 1051.46 | **424.6** | 18614† |

`*` `separate` reports a payload estimate, not a serialized proof: it is two
independent proofs, and the standalone u32 API has no enclosing wire codec.
Not the same accounting as the other rows.

`†` The only case that paged: 101,732 swapouts during the run. Every other row
recorded zero.

## Reading the table

- **Proof size depends entirely on which rate Binius is given.** At rate 1/2
  hybrid looks 1.22–1.33x smaller; at rate 1/8 `all-binius` is smaller at every
  size (e.g. 424.6 KB vs 505.5 KB at 2^21). Rate 1/8 costs Binius 11–16% prover
  time and leaves its verifier unchanged. Any size claim must name the rate.
- **Prover and verifier favour hybrid, and the margin grows with N**: 1.7x/2.7x
  at 2^19, 1.9x/3.5x at 2^20 against rate 1/2.
- **Peak memory is the durable separation**: hybrid holds 2.7–3.4x less
  resident memory than `all-binius` at 2^19–2^20, and is the only mode with
  headroom at 2^21 (11.3 GB against 17.5–18.6 GB on a 24 GB machine).
- **The 2^21 all-binius jump is real, not paging.** Its rate-1/2 case recorded
  zero swapouts and still took 4587.7 ms — 5.9x the 2^20 time for 2x the work,
  with the verifier up 12x. Binius64 crosses something between 2^20 and 2^21
  that this harness does not explain.
- **The witgen column is not symmetric.** For `all-binius`/`separate` it is row
  construction plus the circuit's witness filling. `hybrid` synthesizes its
  assignment inside `commit_mod32`, so its witgen column is row construction
  only and the rest sits in the prover column. Do not read hybrid's 0.54 ms
  against all-binius's 192.51 ms as a like-for-like ratio.
