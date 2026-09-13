# Albert benchmark verification — requested campaign

Generated 2026-09-13T16:18:11.203309+00:00. **Complete:** 152/152 native comparison cases, 24/24 BitZ wide cases, 6/6 SHA+ECDSA cases. Each completed case has five measured repetitions; native and SHA warmups are retained separately. The native matrix includes the independently repeated Binius sweeps requested in both command groups.

## What this establishes

The original four intended Binius configurations are BaseFold and the Ligerito adapter, each at initial rates **1/2 and 1/8**. The historical prose/labels disagree; no historical rate-1/4 result was verified. This campaign adds rate 1/4 explicitly, producing **six current configurations**. A new rate-1/4 measurement does not authenticate an old rate-1/4 label.

The adapter replaces the Binius PCS opening path with ring switching plus Johnson-regime Ligerito, while retaining the supported Binius constraint system and PIOP. The compared implementations also differ in hash/transcript and folding parameters: standard Binius uses SHA-256, the adapter uses BLAKE3. Treat these as configured implementation comparisons, not an isolated experiment changing only the PCS algorithm.

A blanket claim of negligible prover or memory impact is not established. Use the measured changes by rate and size below. Proof size is stable for a given case; the timing data have appreciable variation and known machine contention.

## Execution and validation

Apple M1 Max, 64 GiB RAM, Rust 1.98.1, `-C target-cpu=native`, eight Rayon threads, release profile. Commands run sequentially. The requested `unchecked` feature applies to the performance sweeps; the selectable-rate unit test uses its requested `binius64-bench` feature. The unit test passed for all three rates, both committed oracles, and cross-rate proof rejection. All native timing and memory records included here report successful proof verification and matching corpus hashes. All SHA cases share the same fixture.

The first native attempts stopped before measurement because two new environment knobs were missing from the harness allowlist. The wide attempts stopped before measurement because `cargo run` needed both `--bin f2z` and the required `span-metrics` feature. Corrected retries preserve all original failures and logs. No failed group is silently relabeled as a successful run. The SHA runner tests passed (10 tests). Earlier circuit/statement validation passed 22 tests; see the linked earlier audit for the negative cases.

Only this campaign’s process group is monitored, with a 48 GiB RSS stop threshold on this 64 GiB host. No unrelated application was stopped. macOS StorageManagementService was observed using 152% CPU at 14:10:24 UTC. This is a timing limitation: five samples and these sequential duplicate sweeps do not establish precise small differences or statistical equivalence. During the final Plonky3 case, the host also had 36.6 GiB of swap in use. A warmup-time vmmap snapshot reported a 43.9G main-process footprint, while the separate memory-only child reported 29.2 GiB peak RSS. These are different measurement boundaries, and RSS excludes part of the compressed/swapped footprint. The largest-case timings and RSS must be interpreted under this memory pressure. See [memory-pressure.json](/Users/johnwu/code/zk/f2z-pcs/PerfRuns/albert-requested-_f62mead/memory-pressure.json).

Commands/provenance: [request.json](/Users/johnwu/code/zk/f2z-pcs/PerfRuns/albert-requested-_f62mead/request.json), [retry-commands.json](/Users/johnwu/code/zk/f2z-pcs/PerfRuns/albert-requested-_f62mead/retry-commands.json), [wide-retry-commands.json](/Users/johnwu/code/zk/f2z-pcs/PerfRuns/albert-requested-_f62mead/wide-retry-commands.json), [status.json](/Users/johnwu/code/zk/f2z-pcs/PerfRuns/albert-requested-_f62mead/status.json), [retry-status.json](/Users/johnwu/code/zk/f2z-pcs/PerfRuns/albert-requested-_f62mead/retry-status.json), [wide-retry-status.json](/Users/johnwu/code/zk/f2z-pcs/PerfRuns/albert-requested-_f62mead/wide-retry-status.json), [initial-source.patch](/Users/johnwu/code/zk/f2z-pcs/PerfRuns/albert-requested-_f62mead/initial-source.patch), [retry-source.patch](/Users/johnwu/code/zk/f2z-pcs/PerfRuns/albert-requested-_f62mead/retry-source.patch), [retry-source-sha256.json](/Users/johnwu/code/zk/f2z-pcs/PerfRuns/albert-requested-_f62mead/retry-source-sha256.json), [timing-contention.json](/Users/johnwu/code/zk/f2z-pcs/PerfRuns/albert-requested-_f62mead/timing-contention.json).

Earlier statement/negative-test audit: [albert-review-2026-09-13.md](/Users/johnwu/code/zk/f2z-pcs/docs/albert-review-2026-09-13.md).

Source stability: all 481 snapshotted source/build files match the recorded baseline plus the documented retry corrections. See [source-stability.json](/Users/johnwu/code/zk/f2z-pcs/PerfRuns/albert-requested-_f62mead/source-stability.json).

## Measurement boundaries

- Native **witness-to-proof** is the recorded interval from witness generation through proof completion, excluding reusable setup and verification. Adapter serialization is inside its proving interval and decoding is inside verification; standard Binius generates/consumes native transcript bytes. These are the existing benchmark boundaries. Phase intervals can overlap (for example witness packing inside the online prover); do not sum all columns.
- Native peak RSS is a separate fresh-process pass including corpus, setup, witness, one verified proof, and proof accounting. It is not prover-only memory.
- Wide CLI proving reuses a witness and reports witness construction separately. Its peak is tracked live heap in **MiB**, although its original output says MB. It is not comparable directly with RSS. The CLI retains aggregate medians, not all individual timing samples.
- SHA witness-to-proof excludes reusable setup, fixture/signature generation, verification, and transport codec work. Its RSS is the worker process peak across setup and all trials.
- Native/SHA proof figures count complete proof material including commitment. Wide CLI raw bytes omit its 32-byte BLAKE3 commitment root; an explicitly adjusted figure is also provided. Expected public inputs are separate. All KB below are decimal; GiB is binary.

## Focused Binius comparison at 2^22 multiplications

| Rate | Backend | Witness→proof ms | P10–P90 ms | Verify ms | Proof KB | Peak RSS GiB |
| --- | --- | --- | --- | --- | --- | --- |
| 1/2 | Binius64 / BaseFold | 2,799.52 | 2,693.78–3,301.22 | 57.39 | 606.80 | 22.20 |
| 1/2 | Binius64 / Ligerito | 3,757.16 | 3,675.50–3,998.36 | 59.48 | 477.94 | 24.30 |
| 1/4 | Binius64 / BaseFold | 3,574.59 | 3,371.10–4,580.40 | 427.67 | 446.78 | 21.04 |
| 1/4 | Binius64 / Ligerito | 3,865.91 | 3,811.46–4,104.07 | 57.56 | 316.02 | 23.57 |
| 1/8 | Binius64 / BaseFold | 8,131.45 | 7,242.03–9,461.11 | 463.84 | 408.21 | 20.72 |
| 1/8 | Binius64 / Ligerito | 4,465.84 | 4,308.10–4,778.72 | 54.15 | 256.50 | 19.12 |

Change from BaseFold to Ligerito, using the five-sample medians (negative means smaller/faster):

| Rate | Witness→proof | Commit | PIOP | Opening | Verifier | Proof bytes | Peak RSS |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 1/2 | +34.2% | +17.4% | +28.8% | +270.8% | +3.6% | -21.2% | +9.4% |
| 1/4 | +8.1% | +11.8% | +4.2% | +109.9% | -86.5% | -29.3% | +12.0% |
| 1/8 | -45.1% | -5.6% | -50.0% | -32.2% | -88.3% | -37.2% | -7.7% |

Stage changes are observations for the configured backends, including their different hashing, serialization, and security models. The full exponent sweep is retained below and in [binius-changes.csv](/Users/johnwu/code/zk/f2z-pcs/PerfRuns/albert-requested-_f62mead/binius-changes.csv).

Independent repeat within the broader comparison at 2^22; percentage change from BaseFold to Ligerito:

| Rate | Witness→proof | Verifier | Proof bytes | Peak RSS |
| --- | --- | --- | --- | --- |
| 1/2 | +30.2% | +4.3% | -21.2% | +17.8% |
| 1/4 | +35.8% | -14.0% | -29.3% | +1.0% |
| 1/8 | +21.1% | -85.9% | -37.2% | +22.8% |

**Verifier timing is not stable across the repeated runs.** Rate-1/4 BaseFold at 2^22 measured 427.67 ms in the focused pass and 59.53 ms in the broader pass with identical protocol configuration and proof size. The large apparent verifier reduction in the focused table is not sufficient evidence for a precise speedup claim. Keep both observations and confirm verifier performance on an idle machine.

Interactive phase intervals with sample distributions: [focused-intervals/intervals.html](/Users/johnwu/code/zk/f2z-pcs/PerfRuns/albert-requested-_f62mead/focused-intervals/intervals.html). The focused traces passed structural validation: 288 warmup/measured runs and 3,456 spans. Derived traces normalize legacy trial-index labels, record the actual release build profile and clarify measurement boundaries; all original measured intervals and raw traces are preserved. Primary totals use interval unions, not sums of nested spans.

The broader comparison is available in [all-provers-intervals/intervals.html](/Users/johnwu/code/zk/f2z-pcs/PerfRuns/albert-requested-_f62mead/all-provers-intervals/intervals.html). Its traces passed structural validation for 624 warmup/measured runs and 6,816 spans. Across both visualizations, all 1,064 phase-median checks match the original measurements. The browser URL policy blocked visual preview of the local HTML; the structural and numerical checks passed.

## Actual Binius parameters

| Initial rate | BaseFold queries | BaseFold grinding | BaseFold security scope |
| --- | --- | --- | --- |
| 1/2 | 241 | 0 | FRI query phase only |
| 1/4 | 148 | 0 | FRI query phase only |
| 1/8 | 121 | 0 | FRI query phase only |

BaseFold query counts use the implementation’s target-100 formula; the label does not include PIOP or FRI folding soundness. The reported regime is unique decoding. Fold arities, message lengths and final challenges are recorded per shape.

Ligerito uses Johnson OOD proximity with eta=0.02, 32 initial lanes, per-level query/fold grinding, and Round-0 OOD grinding. Its reported `whole_protocol_bits` is the implementation’s modeled composition bound including its grinding model, gated at at least 100. It is not an independently established unconditional Fiat–Shamir security theorem. Rates in the following table describe the initial commitment rate; deeper levels can use different rates.

| Rate | Component target | Modeled composition bits | Initial queries | Initial fold grind | Initial query grind | Round-0 grind (oracle 0) |
| --- | --- | --- | --- | --- | --- | --- |
| 1/2 | 105 | 100.100277 | 194 | 23 | 16 | 14 |
| 1/4 | 105 | 100.224242 | 95 | 24 | 16 | 15 |
| 1/8 | 105 | 100.196286 | 63 | 24 | 16 | 16 |

Every oracle’s full level schedule, error terms, and Round-0 grinding are preserved in `config` in [analysis.json](/Users/johnwu/code/zk/f2z-pcs/PerfRuns/albert-requested-_f62mead/analysis.json) and the original `samples.jsonl` records. A flat table of all 189 oracle-level parameter rows from the focused runs is available in [binius-ligerito-parameters.csv](/Users/johnwu/code/zk/f2z-pcs/PerfRuns/albert-requested-_f62mead/binius-ligerito-parameters.csv). BitZ’s native comparison reports round-by-round economic accounting. Plonky3 reports its native proven-security calculation, including AIR/FRI, with queries increased as required. Limber uses its native Brakedown parameter set and is a single baseline, not a rate-1/2, 1/4, or 1/8 experiment.

| Plonky3 initial rate | Queries | Reported proven bits | Unique-decoding bits | List-decoding bits |
| --- | --- | --- | --- | --- |
| 1/2 | 201 | 100 | 83 | 100 |
| 1/4 | 101 | 100 | 68 | 100 |
| 1/8 | 100 | 127 | 83 | 127 |

The Plonky3 table shows the recorded 2^15 configuration; every case retains its full report. The existing minimum of 100 queries is preserved. It produces a reported 127-bit bound at rate 1/8, so these rate sweeps are not uniformly tuned to exactly 100 bits even within Plonky3.

## SHA-256 followed by P-256 ECDSA

The measured workload is **one chain of 128 total compressions including padding**, over **8,128 message bytes**, followed by one P-256 ECDSA verification. It is not 128 independent chains. A 4,032-byte message requires 64 total compressions; exactly 4,096 bytes requires 65. The current power-of-two interface does not select 65 compressions.

Both circuits constrain the SHA output to the ECDSA digest input, match P-256/public-key/signature/padding semantics, and reject the tested message/signature/public-input mutations. The public statement contains the exponent, key and signature; message and digest are witnesses. These benchmarks do not promise zero knowledge. The Binius–Ligerito adapter does not support this composed circuit because it contains BMUL constraints; the SHA comparison here uses standard Binius64/BaseFold.

| Rate | Backend | Setup ms (once) | Witness→proof ms | P10–P90 ms | Verify ms | Proof KB | Peak RSS GiB |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 1/2 | Binius64 / BaseFold | 671.24 | 574.51 | 544.57–575.95 | 114.80 | 393.78 | 0.96 |
| 1/2 | BitZ Split | 1,619.79 | 162.84 | 106.51–165.73 | 23.92 | 135.34 | 1.21 |
| 1/4 | Binius64 / BaseFold | 642.21 | 553.46 | 525.68–610.80 | 120.44 | 303.15 | 0.97 |
| 1/4 | BitZ Split | 1,707.76 | 90.44 | 89.78–109.19 | 22.70 | 102.94 | 1.22 |
| 1/8 | Binius64 / BaseFold | 671.90 | 683.64 | 663.55–697.86 | 119.60 | 289.20 | 1.04 |
| 1/8 | BitZ Split | 1,611.07 | 112.27 | 105.52–131.97 | 19.87 | 92.43 | 1.21 |

These “target 100” settings have different scopes. BitZ reports an economic bound including grinding and separately a statistical bound without grinding; standard Binius reports the FRI-query-only target. Do not describe this as a comparison at a demonstrated identical complete-protocol statistical security level. Full security reports, phase timings and samples: [sha-summary.csv](/Users/johnwu/code/zk/f2z-pcs/PerfRuns/albert-requested-_f62mead/sha-summary.csv) and [analysis.json](/Users/johnwu/code/zk/f2z-pcs/PerfRuns/albert-requested-_f62mead/analysis.json).

The witness-to-proof comparison amortizes reusable setup. BitZ’s measured setup is larger here and is shown separately; the faster proof-generation interval must not be presented as the total latency of a cold invocation including setup.

## Full-width versus wrapping u32 multiplication

The CLI `--mul-sweep` constructs `u32 × u32 → u64` products. The shared comparison workload `u32-mod32` exposes the low 32-bit result. In BitZ, both use the same full-product relation `x*y = z_low + 2^32*z_high` with four committed 32-bit limbs (128 committed bits per multiplication). Thus the code supports equal BitZ relation/constraint cost. These requested commands use different input generators and measurement drivers, so their timings alone are not a paired experiment proving identical runtime. The shared benchmark has no full-width-u32 cross-backend mode; its `u64` mode would use 64-bit operands.

| Rate | Exponent | Witness ms | Prove after witness ms | Verify ms | Raw proof B | Proof B incl. root | Tracked heap MiB |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 1/2 | 15 | 0.10 | 25.84 | 4.12 | 119280 | 119312 | 29.43 |
| 1/2 | 16 | 0.14 | 36.93 | 3.82 | 138932 | 138964 | 55.23 |
| 1/2 | 17 | 0.26 | 69.03 | 4.81 | 151872 | 151904 | 110.00 |
| 1/2 | 18 | 0.79 | 110.04 | 4.92 | 169016 | 169048 | 212.82 |
| 1/2 | 19 | 1.08 | 162.52 | 5.52 | 187844 | 187876 | 424.75 |
| 1/2 | 20 | 1.65 | 285.24 | 5.89 | 209632 | 209664 | 835.19 |
| 1/2 | 21 | 3.48 | 617.98 | 8.38 | 226676 | 226708 | 1,668.79 |
| 1/2 | 22 | 5.32 | 1,158.98 | 8.13 | 262308 | 262340 | 3,309.27 |
| 1/4 | 15 | 0.07 | 25.15 | 3.76 | 83328 | 83360 | 30.68 |
| 1/4 | 16 | 0.18 | 37.47 | 3.55 | 99620 | 99652 | 57.73 |
| 1/4 | 17 | 0.18 | 58.72 | 4.35 | 107632 | 107664 | 115.06 |
| 1/4 | 18 | 0.55 | 99.92 | 4.74 | 120072 | 120104 | 222.82 |
| 1/4 | 19 | 1.02 | 174.30 | 5.45 | 136588 | 136620 | 444.75 |
| 1/4 | 20 | 1.66 | 310.38 | 5.82 | 154336 | 154368 | 875.19 |
| 1/4 | 21 | 3.31 | 642.80 | 8.10 | 162932 | 162964 | 1,748.79 |
| 1/4 | 22 | 6.29 | 1,226.66 | 7.85 | 196676 | 196708 | 3,469.27 |
| 1/8 | 15 | 0.04 | 25.23 | 3.94 | 70480 | 70512 | 33.18 |
| 1/8 | 16 | 0.16 | 36.65 | 3.26 | 84756 | 84788 | 62.79 |
| 1/8 | 17 | 0.23 | 60.22 | 3.96 | 92192 | 92224 | 125.06 |
| 1/8 | 18 | 0.50 | 107.06 | 4.97 | 102480 | 102512 | 242.82 |
| 1/8 | 19 | 1.14 | 176.38 | 6.51 | 116148 | 116180 | 484.75 |
| 1/8 | 20 | 2.00 | 294.35 | 5.41 | 131208 | 131240 | 955.19 |
| 1/8 | 21 | 4.17 | 616.73 | 7.56 | 139772 | 139804 | 1,908.79 |
| 1/8 | 22 | 6.14 | 1,166.63 | 7.23 | 171220 | 171252 | 3,789.27 |

For all 24 completed wide/wrapping counterpart pairs, the full recorded Ligerito configuration and matrix geometry match exactly. The proof-byte comparison includes the CLI’s omitted commitment root. See [wide-vs-wrapping.csv](/Users/johnwu/code/zk/f2z-pcs/PerfRuns/albert-requested-_f62mead/wide-vs-wrapping.csv). This verifies configuration equivalence, not equality of runtime under the different drivers and input generators.

## All native comparison measurements

Times are five-sample medians in milliseconds. Setup is a single untimed-for-proving setup measurement. Original samples and P10/P90 values are in [analysis.json](/Users/johnwu/code/zk/f2z-pcs/PerfRuns/albert-requested-_f62mead/analysis.json).

### binius-focused

| Rate | e | Backend | Setup | Witness | Commit | PIOP | Opening | Witness→proof | Verify | Proof KB | RSS GiB |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 1/2 | 15 | Binius64 / BaseFold | 80.20 | 0.89 | 1.69 | 44.67 | 5.32 | 53.35 | 2.40 | 324.08 | 0.28 |
| 1/2 | 15 | Binius64 / Ligerito | 73.68 | 0.86 | 1.43 | 39.36 | 9.52 | 50.53 | 4.70 | 362.08 | 0.28 |
| 1/2 | 16 | Binius64 / BaseFold | 170.67 | 1.90 | 2.77 | 79.52 | 6.38 | 93.08 | 2.69 | 355.39 | 0.54 |
| 1/2 | 16 | Binius64 / Ligerito | 174.74 | 1.65 | 6.70 | 117.69 | 69.17 | 197.46 | 6.16 | 373.57 | 0.53 |
| 1/2 | 17 | Binius64 / BaseFold | 361.64 | 4.02 | 5.45 | 127.59 | 10.68 | 150.30 | 4.02 | 387.89 | 0.92 |
| 1/2 | 17 | Binius64 / Ligerito | 342.53 | 3.30 | 4.79 | 104.12 | 14.10 | 126.20 | 5.82 | 388.35 | 0.90 |
| 1/2 | 18 | Binius64 / BaseFold | 734.93 | 7.00 | 7.34 | 200.10 | 12.99 | 228.02 | 4.41 | 431.50 | 1.80 |
| 1/2 | 18 | Binius64 / Ligerito | 767.53 | 7.25 | 11.65 | 191.90 | 32.71 | 241.23 | 6.97 | 408.66 | 1.77 |
| 1/2 | 19 | Binius64 / BaseFold | 1,552.08 | 14.21 | 14.58 | 393.97 | 17.22 | 440.71 | 11.80 | 472.26 | 3.67 |
| 1/2 | 19 | Binius64 / Ligerito | 1,457.83 | 13.38 | 18.84 | 375.42 | 32.47 | 447.91 | 10.11 | 424.82 | 3.35 |
| 1/2 | 20 | Binius64 / BaseFold | 3,261.91 | 28.07 | 29.18 | 703.11 | 29.62 | 793.04 | 20.44 | 518.86 | 7.32 |
| 1/2 | 20 | Binius64 / Ligerito | 3,064.83 | 26.45 | 39.97 | 794.32 | 59.53 | 918.92 | 15.14 | 438.19 | 6.66 |
| 1/2 | 21 | Binius64 / BaseFold | 6,209.76 | 55.39 | 61.16 | 1,318.30 | 48.79 | 1,484.81 | 32.22 | 559.20 | 14.73 |
| 1/2 | 21 | Binius64 / Ligerito | 6,309.79 | 52.83 | 78.07 | 1,628.05 | 147.16 | 1,906.12 | 26.39 | 461.87 | 13.30 |
| 1/2 | 22 | Binius64 / BaseFold | 13,188.43 | 114.42 | 138.62 | 2,458.06 | 89.49 | 2,799.52 | 57.39 | 606.80 | 22.20 |
| 1/2 | 22 | Binius64 / Ligerito | 13,617.07 | 106.67 | 162.74 | 3,164.79 | 331.82 | 3,757.16 | 59.48 | 477.94 | 24.30 |
| 1/4 | 15 | Binius64 / BaseFold | 75.55 | 0.88 | 2.77 | 42.03 | 4.83 | 50.18 | 2.21 | 253.01 | 0.29 |
| 1/4 | 15 | Binius64 / Ligerito | 74.77 | 0.84 | 2.75 | 41.62 | 9.80 | 55.16 | 3.79 | 232.82 | 0.27 |
| 1/4 | 16 | Binius64 / BaseFold | 163.95 | 1.80 | 3.73 | 59.17 | 5.69 | 70.17 | 2.53 | 272.42 | 0.54 |
| 1/4 | 16 | Binius64 / Ligerito | 161.38 | 1.68 | 4.78 | 59.90 | 14.63 | 81.21 | 4.19 | 241.07 | 0.52 |
| 1/4 | 17 | Binius64 / BaseFold | 342.47 | 3.61 | 7.37 | 100.31 | 7.89 | 118.96 | 2.96 | 297.26 | 0.94 |
| 1/4 | 17 | Binius64 / Ligerito | 339.35 | 3.36 | 8.84 | 97.67 | 16.78 | 126.63 | 5.13 | 250.19 | 0.92 |
| 1/4 | 18 | Binius64 / BaseFold | 698.82 | 7.19 | 13.54 | 167.54 | 10.89 | 198.94 | 4.36 | 329.25 | 1.85 |
| 1/4 | 18 | Binius64 / Ligerito | 723.02 | 6.64 | 18.06 | 169.35 | 27.10 | 221.32 | 6.11 | 266.29 | 1.81 |
| 1/4 | 19 | Binius64 / BaseFold | 1,421.54 | 14.05 | 28.31 | 301.90 | 17.50 | 361.16 | 10.48 | 354.19 | 3.74 |
| 1/4 | 19 | Binius64 / Ligerito | 1,441.43 | 13.25 | 33.06 | 351.57 | 39.05 | 438.56 | 8.65 | 277.46 | 3.41 |
| 1/4 | 20 | Binius64 / BaseFold | 3,085.11 | 28.10 | 55.48 | 682.95 | 29.75 | 796.88 | 20.18 | 382.59 | 7.51 |
| 1/4 | 20 | Binius64 / Ligerito | 3,013.90 | 27.09 | 76.51 | 762.03 | 66.61 | 929.42 | 13.89 | 287.31 | 6.82 |
| 1/4 | 21 | Binius64 / BaseFold | 6,166.78 | 56.50 | 115.68 | 1,314.82 | 51.34 | 1,537.86 | 31.80 | 412.30 | 14.90 |
| 1/4 | 21 | Binius64 / Ligerito | 6,121.51 | 54.87 | 141.19 | 1,542.60 | 180.56 | 1,919.40 | 23.33 | 304.34 | 13.60 |
| 1/4 | 22 | Binius64 / BaseFold | 13,425.66 | 169.37 | 259.46 | 3,053.65 | 118.99 | 3,574.59 | 427.67 | 446.78 | 21.04 |
| 1/4 | 22 | Binius64 / Ligerito | 13,106.91 | 107.41 | 290.13 | 3,182.67 | 249.77 | 3,865.91 | 57.56 | 316.02 | 23.57 |
| 1/8 | 15 | Binius64 / BaseFold | 75.60 | 0.90 | 3.57 | 41.76 | 4.74 | 51.19 | 2.20 | 242.93 | 0.29 |
| 1/8 | 15 | Binius64 / Ligerito | 73.27 | 0.86 | 4.10 | 40.93 | 8.38 | 54.29 | 3.60 | 186.48 | 0.27 |
| 1/8 | 16 | Binius64 / BaseFold | 160.26 | 1.81 | 6.37 | 60.58 | 5.93 | 74.93 | 2.45 | 259.33 | 0.56 |
| 1/8 | 16 | Binius64 / Ligerito | 158.60 | 1.70 | 8.62 | 60.60 | 10.20 | 80.98 | 4.01 | 194.06 | 0.53 |
| 1/8 | 17 | Binius64 / BaseFold | 344.28 | 3.59 | 12.98 | 97.89 | 8.39 | 122.50 | 2.92 | 283.31 | 1.00 |
| 1/8 | 17 | Binius64 / Ligerito | 339.73 | 3.37 | 16.38 | 99.47 | 15.78 | 133.72 | 5.06 | 200.53 | 0.95 |
| 1/8 | 18 | Binius64 / BaseFold | 707.07 | 7.08 | 26.11 | 169.58 | 12.61 | 217.32 | 4.04 | 306.03 | 1.97 |
| 1/8 | 18 | Binius64 / Ligerito | 720.28 | 6.74 | 31.81 | 170.96 | 27.96 | 237.81 | 5.79 | 216.59 | 1.86 |
| 1/8 | 19 | Binius64 / BaseFold | 1,453.99 | 14.11 | 51.89 | 311.44 | 21.07 | 400.63 | 11.34 | 330.50 | 3.93 |
| 1/8 | 19 | Binius64 / Ligerito | 1,445.54 | 13.31 | 61.02 | 362.33 | 44.60 | 484.83 | 9.06 | 223.38 | 3.57 |
| 1/8 | 20 | Binius64 / BaseFold | 2,990.20 | 27.98 | 107.80 | 661.73 | 34.53 | 829.70 | 20.70 | 355.44 | 7.83 |
| 1/8 | 20 | Binius64 / Ligerito | 3,077.70 | 27.13 | 131.10 | 779.96 | 175.40 | 1,120.78 | 15.12 | 233.04 | 7.10 |
| 1/8 | 21 | Binius64 / BaseFold | 6,269.85 | 56.10 | 229.15 | 1,312.66 | 64.39 | 1,670.54 | 32.70 | 383.42 | 15.80 |
| 1/8 | 21 | Binius64 / Ligerito | 6,312.14 | 54.14 | 256.71 | 1,550.07 | 190.24 | 2,058.41 | 25.78 | 247.64 | 14.17 |
| 1/8 | 22 | Binius64 / BaseFold | 13,473.69 | 305.11 | 580.32 | 6,830.84 | 523.51 | 8,131.45 | 463.84 | 408.21 | 20.72 |
| 1/8 | 22 | Binius64 / Ligerito | 13,541.40 | 113.36 | 547.79 | 3,416.19 | 354.88 | 4,465.84 | 54.15 | 256.50 | 19.12 |

### all-provers

| Rate | e | Backend | Setup | Witness | Commit | PIOP | Opening | Witness→proof | Verify | Proof KB | RSS GiB |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 1/2 | 15 | Binius64 / BaseFold | 75.26 | 0.90 | 1.88 | 42.87 | 5.55 | 51.11 | 2.43 | 324.08 | 0.28 |
| 1/2 | 15 | Binius64 / Ligerito | 74.16 | 0.87 | 1.76 | 41.39 | 8.18 | 52.31 | 4.42 | 362.08 | 0.28 |
| 1/2 | 15 | BitZ | 11.22 | 0.07 | 1.13 | 2.25 | 20.67 | 24.72 | 4.11 | 118.45 | 0.06 |
| 1/2 | 15 | Plonky3 / FRI | 17.04 | 2.81 | 327.77 | 35.98 | 59.63 | 429.40 | 28.75 | 621.70 | 0.11 |
| 1/2 | 16 | Binius64 / BaseFold | 164.61 | 1.83 | 2.94 | 60.30 | 6.43 | 71.75 | 2.74 | 355.39 | 0.54 |
| 1/2 | 16 | Binius64 / Ligerito | 164.56 | 1.70 | 3.02 | 69.90 | 13.32 | 89.44 | 4.96 | 373.57 | 0.53 |
| 1/2 | 16 | BitZ | 22.01 | 0.14 | 1.62 | 3.26 | 32.49 | 38.15 | 3.58 | 138.32 | 0.11 |
| 1/2 | 16 | Plonky3 / FRI | 16.94 | 5.71 | 591.57 | 66.22 | 98.17 | 764.18 | 31.73 | 687.49 | 0.20 |
| 1/2 | 17 | Binius64 / BaseFold | 357.26 | 3.74 | 4.42 | 98.99 | 8.32 | 115.61 | 3.21 | 387.89 | 0.92 |
| 1/2 | 17 | Binius64 / Ligerito | 341.09 | 3.32 | 5.06 | 101.30 | 15.82 | 125.59 | 5.93 | 388.35 | 0.90 |
| 1/2 | 17 | BitZ | 43.86 | 0.25 | 2.61 | 5.02 | 45.21 | 54.10 | 4.43 | 150.34 | 0.16 |
| 1/2 | 17 | Plonky3 / FRI | 17.63 | 11.86 | 1,215.60 | 129.45 | 204.33 | 1,559.93 | 34.63 | 763.64 | 0.39 |
| 1/2 | 18 | Binius64 / BaseFold | 714.12 | 7.05 | 9.10 | 192.64 | 10.92 | 218.88 | 4.58 | 431.50 | 1.81 |
| 1/2 | 18 | Binius64 / Ligerito | 721.32 | 6.63 | 10.01 | 217.69 | 26.09 | 258.31 | 7.49 | 408.66 | 1.76 |
| 1/2 | 18 | BitZ | 88.91 | 0.53 | 4.54 | 7.38 | 75.19 | 87.68 | 4.80 | 168.34 | 0.30 |
| 1/2 | 18 | Plonky3 / FRI | 16.94 | 28.43 | 2,480.63 | 291.34 | 413.77 | 3,289.23 | 37.94 | 854.30 | 0.78 |
| 1/2 | 19 | Binius64 / BaseFold | 1,510.12 | 14.30 | 19.82 | 400.52 | 16.54 | 464.99 | 11.20 | 472.26 | 3.68 |
| 1/2 | 19 | Binius64 / Ligerito | 1,596.77 | 13.37 | 20.98 | 383.84 | 31.58 | 451.88 | 10.39 | 424.82 | 3.35 |
| 1/2 | 19 | BitZ | 174.03 | 0.98 | 9.22 | 10.94 | 178.99 | 209.43 | 4.67 | 188.13 | 0.52 |
| 1/2 | 19 | Plonky3 / FRI | 17.15 | 46.94 | 4,850.79 | 507.84 | 755.21 | 6,152.80 | 40.91 | 937.69 | 1.54 |
| 1/2 | 20 | Binius64 / BaseFold | 3,122.58 | 28.02 | 31.26 | 679.54 | 28.40 | 767.49 | 20.96 | 518.86 | 7.32 |
| 1/2 | 20 | Binius64 / Ligerito | 3,055.01 | 26.57 | 38.33 | 811.01 | 58.62 | 935.90 | 16.32 | 438.19 | 6.66 |
| 1/2 | 20 | BitZ | 348.39 | 2.04 | 16.18 | 20.74 | 244.25 | 284.71 | 5.74 | 211.14 | 1.01 |
| 1/2 | 20 | Plonky3 / FRI | 17.16 | 92.61 | 9,736.85 | 1,001.15 | 1,464.64 | 12,307.75 | 44.18 | 1,018.14 | 4.15 |
| 1/2 | 21 | Binius64 / BaseFold | 6,265.81 | 56.17 | 61.00 | 1,320.83 | 49.23 | 1,485.51 | 34.64 | 559.20 | 14.73 |
| 1/2 | 21 | Binius64 / Ligerito | 6,179.10 | 53.34 | 76.09 | 1,548.75 | 137.55 | 1,816.51 | 26.35 | 461.87 | 13.30 |
| 1/2 | 21 | BitZ | 699.66 | 4.19 | 30.82 | 36.42 | 463.19 | 537.37 | 8.06 | 226.84 | 1.89 |
| 1/2 | 21 | Plonky3 / FRI | 16.95 | 196.11 | 19,626.79 | 2,046.34 | 2,955.92 | 24,888.97 | 48.50 | 1,139.55 | 6.14 |
| 1/2 | 22 | Binius64 / BaseFold | 13,303.95 | 129.58 | 135.19 | 2,538.23 | 89.56 | 2,928.25 | 55.77 | 606.80 | 20.42 |
| 1/2 | 22 | Binius64 / Ligerito | 13,138.69 | 107.31 | 159.88 | 3,151.54 | 330.02 | 3,812.12 | 58.17 | 477.94 | 24.05 |
| 1/2 | 22 | BitZ | 1,418.71 | 7.75 | 62.43 | 74.54 | 936.17 | 1,082.33 | 7.33 | 262.12 | 3.72 |
| 1/2 | 22 | Plonky3 / FRI | 17.76 | 362.42 | 39,078.10 | 4,020.39 | 5,738.10 | 49,207.48 | 52.20 | 1,241.08 | 12.21 |
| 1/4 | 15 | Binius64 / BaseFold | 75.57 | 0.91 | 2.84 | 41.13 | 4.62 | 49.66 | 2.22 | 253.01 | 0.29 |
| 1/4 | 15 | Binius64 / Ligerito | 73.02 | 0.83 | 2.47 | 40.03 | 9.74 | 52.75 | 3.73 | 232.82 | 0.27 |
| 1/4 | 15 | BitZ | 11.34 | 0.07 | 1.42 | 2.19 | 20.14 | 24.23 | 3.57 | 83.84 | 0.06 |
| 1/4 | 15 | Plonky3 / FRI | 0.85 | 3.51 | 588.44 | 57.62 | 96.72 | 758.24 | 17.02 | 378.94 | 0.20 |
| 1/4 | 16 | Binius64 / BaseFold | 163.60 | 1.83 | 3.91 | 59.48 | 5.79 | 71.54 | 2.50 | 272.42 | 0.54 |
| 1/4 | 16 | Binius64 / Ligerito | 162.04 | 1.66 | 4.09 | 58.26 | 14.47 | 79.00 | 4.29 | 241.07 | 0.52 |
| 1/4 | 16 | BitZ | 21.99 | 0.14 | 2.41 | 2.99 | 28.40 | 34.45 | 3.31 | 99.91 | 0.11 |
| 1/4 | 16 | Plonky3 / FRI | 0.84 | 7.12 | 1,158.64 | 110.01 | 189.21 | 1,473.78 | 18.85 | 419.29 | 0.39 |
| 1/4 | 17 | Binius64 / BaseFold | 343.73 | 3.64 | 7.08 | 98.89 | 7.99 | 117.92 | 3.08 | 297.26 | 0.94 |
| 1/4 | 17 | Binius64 / Ligerito | 346.80 | 3.37 | 8.93 | 98.72 | 17.15 | 128.24 | 5.15 | 250.19 | 0.91 |
| 1/4 | 17 | BitZ | 43.88 | 0.25 | 3.72 | 4.90 | 44.60 | 54.09 | 4.20 | 108.40 | 0.17 |
| 1/4 | 17 | Plonky3 / FRI | 0.86 | 14.25 | 2,356.86 | 220.86 | 367.46 | 2,976.67 | 20.21 | 464.85 | 0.77 |
| 1/4 | 18 | Binius64 / BaseFold | 727.32 | 7.12 | 13.89 | 167.22 | 10.79 | 199.12 | 4.27 | 329.25 | 1.85 |
| 1/4 | 18 | Binius64 / Ligerito | 697.42 | 6.60 | 17.72 | 167.19 | 26.21 | 217.64 | 6.21 | 266.29 | 1.80 |
| 1/4 | 18 | BitZ | 86.67 | 0.48 | 6.32 | 7.08 | 73.76 | 88.31 | 4.78 | 120.49 | 0.31 |
| 1/4 | 18 | Plonky3 / FRI | 0.85 | 28.35 | 4,651.48 | 440.16 | 710.27 | 5,853.42 | 22.08 | 513.16 | 1.54 |
| 1/4 | 19 | Binius64 / BaseFold | 1,451.51 | 14.17 | 27.90 | 300.35 | 17.19 | 359.73 | 10.32 | 354.19 | 3.74 |
| 1/4 | 19 | Binius64 / Ligerito | 1,447.56 | 13.46 | 32.08 | 354.90 | 36.40 | 438.20 | 8.70 | 277.46 | 3.41 |
| 1/4 | 19 | BitZ | 173.78 | 0.91 | 11.82 | 11.38 | 124.93 | 151.03 | 4.77 | 136.01 | 0.55 |
| 1/4 | 19 | Plonky3 / FRI | 0.85 | 45.58 | 9,354.28 | 866.21 | 1,386.90 | 11,661.48 | 23.70 | 555.98 | 3.60 |
| 1/4 | 20 | Binius64 / BaseFold | 3,073.78 | 27.73 | 54.60 | 677.13 | 29.17 | 787.98 | 20.20 | 382.59 | 7.51 |
| 1/4 | 20 | Binius64 / Ligerito | 3,020.48 | 26.53 | 66.49 | 773.60 | 62.60 | 930.44 | 14.08 | 287.31 | 6.82 |
| 1/4 | 20 | BitZ | 351.06 | 2.09 | 24.08 | 19.72 | 230.20 | 277.87 | 5.48 | 153.70 | 1.05 |
| 1/4 | 20 | Plonky3 / FRI | 0.88 | 90.60 | 18,992.03 | 1,755.02 | 2,810.78 | 23,627.48 | 25.81 | 613.25 | 7.17 |
| 1/4 | 21 | Binius64 / BaseFold | 6,335.16 | 55.80 | 114.86 | 1,327.59 | 52.61 | 1,548.82 | 32.32 | 412.30 | 15.04 |
| 1/4 | 21 | Binius64 / Ligerito | 6,219.92 | 53.08 | 133.28 | 1,540.24 | 179.04 | 1,907.25 | 26.19 | 304.34 | 13.61 |
| 1/4 | 21 | BitZ | 703.00 | 4.16 | 45.71 | 36.80 | 466.07 | 557.96 | 7.33 | 163.83 | 1.97 |
| 1/4 | 21 | Plonky3 / FRI | 1.00 | 179.07 | 38,236.75 | 3,520.94 | 5,593.46 | 47,680.65 | 28.45 | 687.32 | 13.49 |
| 1/4 | 22 | Binius64 / BaseFold | 13,068.88 | 116.18 | 238.80 | 2,446.45 | 100.32 | 2,909.43 | 59.53 | 446.78 | 23.34 |
| 1/4 | 22 | Binius64 / Ligerito | 13,301.20 | 108.88 | 299.24 | 3,153.73 | 250.74 | 3,950.09 | 51.20 | 316.02 | 23.58 |
| 1/4 | 22 | BitZ | 1,418.84 | 7.69 | 96.10 | 69.69 | 891.12 | 1,069.61 | 7.11 | 197.12 | 3.91 |
| 1/4 | 22 | Plonky3 / FRI | 0.91 | 368.66 | 77,412.35 | 7,222.67 | 11,414.93 | 96,944.57 | 30.46 | 736.73 | 20.70 |
| 1/8 | 15 | Binius64 / BaseFold | 73.48 | 0.89 | 3.93 | 41.41 | 4.66 | 50.78 | 2.19 | 242.93 | 0.29 |
| 1/8 | 15 | Binius64 / Ligerito | 74.03 | 0.83 | 3.91 | 40.13 | 8.72 | 54.17 | 3.58 | 186.48 | 0.27 |
| 1/8 | 15 | BitZ | 11.20 | 0.07 | 1.74 | 2.26 | 19.73 | 24.17 | 3.41 | 70.42 | 0.06 |
| 1/8 | 15 | Plonky3 / FRI | 0.60 | 2.70 | 1,153.50 | 104.63 | 175.47 | 1,431.45 | 18.13 | 407.20 | 0.39 |
| 1/8 | 16 | Binius64 / BaseFold | 169.20 | 1.83 | 6.82 | 62.02 | 6.14 | 77.00 | 2.57 | 259.33 | 0.56 |
| 1/8 | 16 | Binius64 / Ligerito | 162.96 | 1.68 | 8.15 | 59.43 | 10.40 | 79.79 | 4.01 | 194.06 | 0.53 |
| 1/8 | 16 | BitZ | 22.02 | 0.14 | 3.35 | 3.27 | 29.60 | 36.37 | 3.28 | 85.24 | 0.11 |
| 1/8 | 16 | Plonky3 / FRI | 0.59 | 6.95 | 2,343.31 | 209.28 | 352.82 | 2,910.61 | 20.49 | 469.97 | 0.77 |
| 1/8 | 17 | Binius64 / BaseFold | 343.18 | 3.61 | 13.42 | 98.99 | 8.68 | 127.48 | 2.87 | 283.31 | 1.00 |
| 1/8 | 17 | Binius64 / Ligerito | 343.20 | 3.38 | 15.08 | 99.72 | 15.60 | 133.70 | 4.90 | 200.53 | 0.95 |
| 1/8 | 17 | BitZ | 43.27 | 0.26 | 5.16 | 4.30 | 43.42 | 53.16 | 3.69 | 92.00 | 0.18 |
| 1/8 | 17 | Plonky3 / FRI | 0.60 | 14.23 | 4,672.26 | 418.37 | 691.14 | 5,823.87 | 21.98 | 509.41 | 1.53 |
| 1/8 | 18 | Binius64 / BaseFold | 722.36 | 7.06 | 26.78 | 173.66 | 12.34 | 219.33 | 4.38 | 306.03 | 1.97 |
| 1/8 | 18 | Binius64 / Ligerito | 712.20 | 6.64 | 32.43 | 169.66 | 27.67 | 237.41 | 5.59 | 216.59 | 1.87 |
| 1/8 | 18 | BitZ | 86.97 | 0.48 | 11.42 | 7.14 | 73.15 | 92.33 | 4.29 | 102.29 | 0.32 |
| 1/8 | 18 | Plonky3 / FRI | 0.60 | 28.73 | 9,499.82 | 833.14 | 1,381.82 | 11,723.42 | 23.67 | 556.31 | 3.05 |
| 1/8 | 19 | Binius64 / BaseFold | 1,510.16 | 14.15 | 58.24 | 310.24 | 20.16 | 400.93 | 11.31 | 330.50 | 3.93 |
| 1/8 | 19 | Binius64 / Ligerito | 1,443.40 | 13.39 | 59.09 | 361.45 | 44.26 | 477.94 | 8.15 | 223.38 | 3.57 |
| 1/8 | 19 | BitZ | 179.07 | 1.06 | 20.43 | 12.08 | 139.11 | 173.12 | 5.00 | 116.05 | 0.58 |
| 1/8 | 19 | Plonky3 / FRI | 0.58 | 45.20 | 18,650.02 | 1,614.61 | 2,676.81 | 23,002.17 | 25.67 | 609.90 | 6.09 |
| 1/8 | 20 | Binius64 / BaseFold | 3,051.04 | 28.06 | 111.60 | 669.28 | 35.27 | 841.41 | 20.16 | 355.44 | 7.84 |
| 1/8 | 20 | Binius64 / Ligerito | 3,115.92 | 26.83 | 131.21 | 799.70 | 174.67 | 1,134.44 | 15.13 | 233.04 | 7.10 |
| 1/8 | 20 | BitZ | 350.15 | 1.84 | 37.72 | 19.28 | 233.72 | 294.72 | 5.63 | 131.14 | 1.13 |
| 1/8 | 20 | Plonky3 / FRI | 0.58 | 111.67 | 37,683.49 | 3,259.40 | 5,407.72 | 46,496.71 | 27.89 | 667.87 | 13.24 |
| 1/8 | 21 | Binius64 / BaseFold | 7,342.32 | 56.51 | 241.80 | 1,346.96 | 66.92 | 1,755.06 | 34.68 | 383.42 | 15.81 |
| 1/8 | 21 | Binius64 / Ligerito | 6,883.86 | 53.46 | 264.16 | 1,625.19 | 201.84 | 2,136.14 | 26.54 | 247.64 | 14.17 |
| 1/8 | 21 | BitZ | 709.86 | 4.18 | 78.46 | 39.27 | 488.05 | 623.91 | 7.55 | 139.80 | 2.13 |
| 1/8 | 21 | Plonky3 / FRI | 0.63 | 194.72 | 80,895.30 | 6,930.19 | 11,314.41 | 99,768.61 | 30.25 | 716.04 | 23.09 |
| 1/8 | 22 | Binius64 / BaseFold | 14,036.94 | 179.69 | 554.44 | 3,110.85 | 136.08 | 3,929.88 | 428.27 | 408.21 | 21.20 |
| 1/8 | 22 | Binius64 / Ligerito | 14,228.98 | 120.36 | 572.46 | 3,710.12 | 372.12 | 4,757.64 | 60.46 | 256.50 | 26.04 |
| 1/8 | 22 | BitZ | 1,414.20 | 8.26 | 169.79 | 71.44 | 964.53 | 1,222.01 | 7.43 | 170.13 | 4.27 |
| 1/8 | 22 | Plonky3 / FRI | 2.09 | 507.26 | 169,248.23 | 17,615.28 | 38,001.05 | 231,171.04 | 35.77 | 793.37 | 29.24 |
| native | 15 | Limber | 9.54 | 0.55 | 26.63 | 18.55 | 165.70 | 235.55 | 53.74 | 3,417.23 | 0.34 |
| native | 16 | Limber | 17.81 | 1.06 | 67.70 | 33.92 | 309.01 | 423.12 | 68.28 | 4,680.78 | 0.64 |
| native | 17 | Limber | 35.51 | 2.12 | 118.12 | 69.49 | 521.40 | 776.67 | 97.07 | 5,847.91 | 1.03 |
| native | 18 | Limber | 75.34 | 4.34 | 205.35 | 138.83 | 940.31 | 1,342.93 | 145.88 | 8,526.99 | 1.92 |
| native | 19 | Limber | 142.69 | 8.47 | 387.56 | 250.10 | 1,753.13 | 2,497.76 | 244.83 | 13,413.65 | 3.67 |
| native | 20 | Limber | 293.07 | 16.76 | 653.02 | 501.25 | 3,287.33 | 4,567.18 | 426.58 | 23,136.84 | 7.00 |
| native | 21 | Limber | 749.90 | 33.07 | 1,402.65 | 987.96 | 6,294.91 | 9,059.83 | 836.14 | 40,812.92 | 13.68 |
| native | 22 | Limber | 1,232.45 | 97.33 | 2,778.70 | 2,655.54 | 13,451.32 | 19,825.41 | 2,031.59 | 75,625.54 | 23.30 |

## Coverage and evidence

Missing/incomplete cases (empty lists mean all requested cases were measured):

```json
{
  "missing": {
    "native": [],
    "wide": [],
    "sha": []
  },
  "incomplete": []
}
```

Machine-readable outputs: [native-summary.csv](/Users/johnwu/code/zk/f2z-pcs/PerfRuns/albert-requested-_f62mead/native-summary.csv), [binius-changes.csv](/Users/johnwu/code/zk/f2z-pcs/PerfRuns/albert-requested-_f62mead/binius-changes.csv), [wide-summary.csv](/Users/johnwu/code/zk/f2z-pcs/PerfRuns/albert-requested-_f62mead/wide-summary.csv), [sha-summary.csv](/Users/johnwu/code/zk/f2z-pcs/PerfRuns/albert-requested-_f62mead/sha-summary.csv), [analysis.json](/Users/johnwu/code/zk/f2z-pcs/PerfRuns/albert-requested-_f62mead/analysis.json).

Local draft response: [ALBERT-REPLY.md](/Users/johnwu/code/zk/f2z-pcs/PerfRuns/albert-requested-_f62mead/ALBERT-REPLY.md). It has not been sent to Slack.

Rebuild this report after the campaign finishes by running `analyze_results.py`, then `build_report.py` in this directory. Both only read recorded results and write summaries; they do not execute benchmarks.

Execution status: [completion.json](/Users/johnwu/code/zk/f2z-pcs/PerfRuns/albert-requested-_f62mead/completion.json) records 13/13 requested command groups complete and 910 measured proof repetitions. It preserves the original failed launches and the successful corrected attempts separately.

The four additional Plonky3 tests passed, covering arithmetic boundary/carry constraints, rejection of a field alias, proven-security accounting across supported shapes and rates, and proof roundtrip/tamper rejection at all three rates. Log: [logs/plonky3-rate-validation.retry2.log](/Users/johnwu/code/zk/f2z-pcs/PerfRuns/albert-requested-_f62mead/logs/plonky3-rate-validation.retry2.log).
