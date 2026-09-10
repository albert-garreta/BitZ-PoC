# Hybrid vs all-Binius, with Binius64 at rate 1/2 and 1/8

`benches/hybrid_u32_sha256` proves `N` multiplications `x*y = z + 2^32*w` (four
u32 limbs) together with `M` chained SHA-256 compressions. SHA-256 is
Binius-arithmetized in **every** mode; the modes differ in how the two branches
are proved:

| mode | multiplication | SHA-256 | proof |
| --- | --- | --- | --- |
| `hybrid` | BitZ/F2Z | Binius64 | one, shared opening |
| `separate` | BitZ/F2Z | Binius64 | two independent proofs |
| `all-binius` | Binius64 | Binius64 | one (ring switch + BaseFold/FRI) |
| `binius-ligerito` | Binius64 | Binius64 | one (the all-Binius circuit and PIOP, every oracle committed at rate 1/8 and opened by the F2Z opener: Round 0, ring switch, Johnson-regime Ligerito with grinding; whole-protocol union bound gated at 100 bits) |

## F2Z-side prover optimizations — 2026-09-10 (byte-identical)

Prover-only changes on the F2Z side of the hybrid (see the "Prover-side
optimizations" section of the [protocol guide](hybrid-u32-sha256-protocol.md)):
bit marginals for the joint sumcheck's packed rounds; an eight-nonce NEON
BLAKE3 kernel plus a dynamic smallest-nonce scan for EVERY proof-of-work grind
(the Ligerito challenger's 16-byte seed and the Spartan/forest/Round-0
boundaries' 32-byte seed both go through `src/utils/blake3x4.rs`); and
parallel/zero-copy versions of three small serial passes. Proof bytes,
transcripts and the verifier are unchanged (same proof digests,
`tests/transcript_pins` 7/7).

**Final sweep, same box state as the morning campaign** (run
`PerfRuns/2026-09-10T11-46-57Z-hybrid-f2z-opt-final`, 11 verified iterations
per case, one process per case, `RAYON_NUM_THREADS=8`, medians; the untouched
phases — forest, Binius SHA PIOP, commit, verifier, peak RSS — agree with the
morning run within 3% at every shape, which is the same-state check). The
paper table `paper/hybrid-table.tex` was regenerated from this run with
`scripts/hybrid_table.py` (Binius rows from the morning run):

| N mul : M SHA | prover before → after (ms) | joint sumcheck | shared opening (grinds incl.) | forest (untouched) | verifier | proof (B) | peak (MiB) |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 2^15 : 2^7 | 42.9 → 24.0 (−44%) | 9.2 → 1.0 | 14.2 → 3.6 | 11.3 → 11.2 | 4.2 → 4.2 | 179,216 | 155 → 150 |
| 2^17 : 2^9 | 92.2 → 57.5 (−38%) | 21.6 → 2.3 | 23.3 → 8.2 | 29.0 → 29.1 | 6.0 → 6.2 | 219,208 | 706 → 698 |
| 2^19 : 2^11 | 312.0 → 192.5 (−38%) | 67.0 → 6.9 | 85.2 → 34.2 | 94.3 → 91.7 | 10.7 → 10.2 | 271,920 | 2498 → 2780 |
| 2^20 : 2^12 | 470.3 → 331.3 (−30%) | 108.6 → 12.0 | 72.9 → 35.0 | 171.9 → 171.8 | 16.3 → 15.6 | 304,992 | 5585 → 5490 |
| 2^21 : 2^13 | 965.6 → 673.5 (−30%) | 229.0 → 24.3 | 116.2 → 61.4 | 374.1 → 361.9 | 27.6 → 27.2 | 329,952 | 11129 → 11051 |
| 2^22 : 2^14 | 2186.2 → 1526.0 (−30%) | 437.2 → 51.4 | 326.1 → 171.5 | 762.7 → 751.0 | 54.7 → 54.1 | 371,256 | 13210 → 13310 |

The 22:14 case logged 2.8 M page compressions (other sessions' idle memory
being compressed, mostly during the 10 s Binius setup); its timed phases sit
at the morning's level, so it carries no dagger. An earlier afternoon
alternating old/new A/B on a thermally slower box gave the same relative
picture (15:7 −45%, 17:9 −37%, 19:11 −34%, 20:12 −30%, 21:13 −26%).

Tools: `scripts/rss_sampler.py` (external peak RSS, swap-outs and page
compressions per sweep case; cheap per-pid sampling, verified neutral at
20:12) and `scripts/hybrid_table.py` (regenerates `paper/hybrid-table.tex`
from a hybrid run directory and an all-Binius run directory; it reproduces the
previous table from its run directory exactly).

## Current hybrid: Johnson-regime opener at rate 1/8 with Round 0 (protocol v3)

The shared F2Z/Ligerito opening was 91–92% of the v2 hybrid proof, and it lost
on parameters alone: `Geometry::params` committed at rate 1/2 and the opener ran
in the unique-decoding regime, paying the whole target in codeword queries with
no query grinding (270 queries at 0.42 bits each). Protocol v3 commits both
branches at rate 1/8 and opens with a Johnson-regime configuration for the same
virtual geometry (`custom_johnson_config_bits(m, 3, virtual_lane_log, 106)`:
64 level-0 queries at 1.42 bits each plus 16 bits of query grinding; deeper
levels at rates 1/16 … 1/128). The paper's theorem covers that regime only with
Round 0, so the prover now sends the out-of-domain evaluation `y = Ṽ(ζ⃗)` of the
virtual packed witness right after the statement, with the audited
`src/ligerito_flock.rs` primitives and the `adopt_ood_round` accounting rule.
Details: [protocol guide](hybrid-u32-sha256-protocol.md).

Same machine, thread count, inputs and methodology as the tables below; medians
of 11 verified iterations, one process per case, executable hash
`27d5fbd09e617036` rebuilt from the v3 sources. Run:
`PerfRuns/2026-09-09T14-08-27Z-hybrid-johnson-ood` (peak RSS sampled externally
at 5 Hz; every case recorded zero swapouts). The v2 hybrid and all-binius rows
are the medians measured earlier today (runs `…12-45-12Z-hybrid-rates` and
`…13-51-09Z-hybrid-fri-bits`), not re-measured.

| N mul : M SHA | scheme | prover (ms) | verifier (ms) | proof (B) | peak (MiB) |
| ---: | --- | ---: | ---: | ---: | ---: |
| 2^15 : 2^7 | **hybrid v3** (Johnson 1/8 + Round 0) | 50.1 | 3.29 | **109,952** | 274 |
| | hybrid v2 (UDR 1/2) | 36.6 | 3.91 | 272,416 | – |
| | all-binius rate 1/8, FRI 112 | 49.5 | 2.58 | 265,264 | – |
| | all-binius rate 1/8, FRI 96 | 50.2 | 2.52 | 237,584 | 690 |
| 2^17 : 2^9 | **hybrid v3** | 101.3 | 4.91 | **134,848** | 922 |
| | hybrid v2 | 79.0 | 5.97 | 339,752 | – |
| | all-binius rate 1/8, FRI 112 | 128.2 | 7.32 | 314,192 | 2452 |
| | all-binius rate 1/8, FRI 96 | 127.6 | 7.12 | 277,008 | 2438 |
| 2^19 : 2^11 | **hybrid v3** | 302.7 | 10.79 | **164,760** | 3084 |
| | hybrid v2 | 230.4 | 12.51 | 423,432 | 2856 |
| | all-binius rate 1/8, FRI 112 | 444.2 | 33.80 | 362,464 | 9620 |
| | all-binius rate 1/8, FRI 96 | 445.5 | 33.15 | 322,544 | 9645 |
| 2^20 : 2^12 | **hybrid v3** | 516.7 | 17.14 | **182,440** | 5894 |
| | hybrid v2 | 415.0 | 19.48 | 467,000 | 5580 |
| | all-binius rate 1/8, FRI 112 | 891.9 | 69.64 | 389,648 | 16578 |
| | all-binius rate 1/8, FRI 96 | 859.5 | 66.25 | 346,080 | 14702 |
| 2^21 : 2^13 | **hybrid v3** | 1137.5 | 32.08 | **195,232** | 12201 |
| | hybrid v2 | 844.0 | 34.47 | 505,456 | 11320 |
| | all-binius rate 1/8, FRI 112 | 4718.2† | 1051.46† | 424,608 | 18614† |
| | all-binius rate 1/8, FRI 96 | 5292.6† | 815.10† | 373,360 | 18247† |

`†` paged (see the FRI-target section below).

- **Proof size**: −59.6% to −61.4% against the v2 hybrid at every shape
  (2^21: 505,456 → 195,232 B). Against `all-binius` at its best measured
  setting the hybrid is now 2.17x smaller at the 112-bit FRI target and 1.91x
  smaller at Binius64's own 96-bit default (2^21), 2.20x/1.96x at 2^19. The
  prior investigation's unverified estimate for this change (194,824 B at 2^21)
  was within 0.2% of the measured figure.
- **Prover**: +25% to +37% (2^21: 844 → 1138 ms). Two causes, both visible in
  the phase columns of `summary.csv`: the rate-1/8 commitments (`witness_commit_ms`
  67.8 → 152.0 ms at 2^21: 8x instead of 2x codeword expansion, four times the
  Merkle leaves) and the shared opening (`shared_opening_ms` 35.6 → 239.5 ms:
  the Johnson fold-challenge grinding — 2^23 hash evaluations before the first
  level-0 fold, tapered one bit per round, about 2^24 in total at 2^21 — plus
  the deeper levels' lower-rate encodings). Round 0 itself is 0.1–4.9 ms
  (`ood_round_ms`). Every other phase is unchanged within noise. Against
  `all-binius` the prover is still 1.5x (2^19) to 1.7x (2^20) faster; the 2^21
  all-binius rows paged and are not comparable.
- **Verifier**: 7–16% faster than v2 (fewer, cheaper queries), 3.1–4.1x faster
  than all-binius at 2^19–2^20.
- **Peak memory**: +8% at 2^21 (11.3 → 12.2 GB) from the four-times-longer
  codewords; still 1.5x under all-binius's 18.2–18.9 GB, and the only mode that
  did not page there.

### Security of the v3 composition

`prepared.security()` gates the whole composition at 100 bits and reports the
union bound. Measured at setup (`algebraic_security_bits` in each `.log`):

| N mul : M SHA | achieved bits | binding term | Round-0 bound (bits) | Round-0 grinding |
| ---: | ---: | --- | ---: | ---: |
| 2^15 : 2^7 | 101.83 | `step3:piop-round` (102.94) | 100.73 | 8 |
| 2^17 : 2^9 | 101.62 | `step3:piop-round` (102.80) | 98.73 | 10 |
| 2^19 : 2^11 | 101.36 | `step3:piop-round` (102.67) | 96.73 | 12 |
| 2^20 : 2^12 | 101.34 | `step3:piop-round` (102.61) | 95.73 | 13 |
| 2^21 : 2^13 | 101.31 | `step3:piop-round` (102.55) | 94.73 | 14 |

- The binding term is the Spartan outer-round union of the 108-bit integer
  prefix, as before; the opener's terms (five levels of `Ligerito proximity
  folds` at 2^-(106…107) each, `Ligerito queries` at 2^-106, the deeper levels'
  explicit `Ligerito OOD samples` at about 2^-112) sum to about 2^-102.2 at
  2^19–2^21 and are the second-largest contribution.
- **Component target 106.** The opener's terms sum to roughly 22·2^-target at
  five levels, so 104 fails the 100-bit gate, 105 clears it by about 0.6 bits
  and 106 by 1.3 bits. Each target bit doubles the level-0 fold grind (2^21
  hashes per fold at 106 for 2^19 multiplications, 2^27 at the profile's 112),
  which is why the shared opener does not reuse `CompositionProfile::LIGERITO_TARGET_BITS = 112`;
  that constant still governs the multiplication relation's standalone
  unique-decoding opener used by the `separate` mode. A one-off build at 105
  (run `PerfRuns/2026-09-09T14-12-50Z-hybrid-johnson-ood-target105`, same
  method) measured 100.61 / 100.58 achieved bits at 2^19 / 2^21 with proofs of
  162,136 / 192,544 B (−1.6% / −1.4%) and prover medians of 297.7 / 966.6 ms
  against 302.7 / 1137.5 ms at 106. Read the 2^21 prover difference with care:
  the expected grind halves (about 2^24 → 2^23 hash evaluations), but the
  benchmark's fixed inputs sample each configuration's proof-of-work search
  exactly once, and a smallest-nonce search has geometric variance. The fold
  nonces in the saved 106-bit proofs give the actual attempt counts: 2^21.44
  at 2^19 (0.67x the 2^22.01 expected) and 2^24.20 at 2^21 (1.15x the 2^24.01
  expected, i.e. 19.3 million BLAKE3 evaluations of 24 bytes on 8 threads —
  on the order of 120–160 ms of the 239.5 ms shared opening). The 105-bit
  instance's 97.4 ms shared opening at 2^21 (2 ms apart from 106 at 2^19)
  therefore reflects a short search as much as the smaller target. 106 stays
  the default for its full-bit margin over the gate; 105 is a legitimate
  100-bit configuration whose expected saving is roughly 2^23 hashes, some
  5–8% of the 2^21 prover time.
- **Round 0** is accounted exactly as `IopSecurityParams::adopt_ood_round`
  accounts it for the standalone relations: the theorem's collision bound
  `C(L_δ, 2)·(2^{m_p} − 1)/|K|` at level 0's Johnson parameters
  (`ood_round_bits`, column 4: `L_δ ≤ 1/(2η√ρ) ≈ 70.7` at η = 0.02, ρ = 1/8),
  topped up to the profile's λ = 108 by proof of work (`ceil(108 − bits)`,
  column 5) under the same 24-bit cap (`MAX_DERIVED_GRINDING_BITS`); the term
  is `step0:ood-draw`. It replaces flock's implicit level-0 binding, which
  would pin the list element only at the final opening — after the Spartan,
  GKR and SHA challenges.
- **Transcript and wire format.** v3 is incompatible with v2: the statement
  digest absorbs the (changed) opener configuration, the transcript domain is
  `f2z/hybrid-u32-mod32-sha256/non-zk/v3` / `hybrid/statement/v3`, Round 0's
  frame `f2z/core/ood-round/v1` and its grinding precede the multiplication
  prefix, and the codec magic is `BZSH\x03` with the Round-0 value and nonce
  first (the decoder replays Round 0 before deriving the prefix prime). The
  hybrid had no golden transcript pin; none of the repository's pins
  (`tests/transcript_pins.rs`) cover it, so no pin moved.

### Proof anatomy and the single-root option

Sections of one saved v3 proof (`--output`; sizes are deterministic):

| N mul : M SHA | total | Ligerito blob | two L0 path sets | rest (Spartan, GKR, sums, SHA, joint, ring switch, Round 0, nonces) |
| ---: | ---: | ---: | ---: | ---: |
| 2^15 : 2^7 | 109,952 | 60,048 | 30,608 (2 x 478 hashes) | 19,288 |
| 2^19 : 2^11 | 164,760 | 89,408 | 45,648 (2 x 713 hashes) | 29,696 |
| 2^21 : 2^13 | 195,232 | 100,608 | 54,416 (2 x 850 hashes) | 40,200 |

The two level-0 multiproofs authenticate the same positions in two trees of
identical shape, so a single tree over the virtual packed witness would carry
exactly one of them: −22,816 B at 2^19 (−13.8%) and −27,200 B at 2^21 (−13.9%).
It is not implemented: it changes the public statement from two roots to one,
which means the multiplication and SHA witnesses must be committed together by
one party, whereas the two-root statement lets each branch be committed
independently and combined later. That is a protocol decision, not an
optimization, and is left to the user.

## Baseline (protocol v2 hybrid): Binius64 at rate 1/2 and 1/8

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

## Binius64's FRI query target: 112 vs 100 vs 96 bits (rate 1/8)

`F2Z_HYBRID_BINIUS_SECURITY_BITS` defaulted to a local choice of 112. Binius64's
own default is `SECURITY_BITS = 96` (`vendor/binius64/crates/verifier/src/verify.rs`)
and the repository's other Binius64 harnesses use 100. Same machine, thread
count, build (executable hash `27d5fbd09e617036`), inputs and methodology as
above; `all-binius` only, rate 1/8, medians of 11 verified iterations. Run:
`PerfRuns/2026-09-09T13-51-09Z-hybrid-fri-bits` (`peak-rss-and-swap.tsv`
sampled externally at 5 Hz per child process). The 112-bit rows are the
rate-1/8 rows of the table above, repeated for reference.

| N mul : M SHA | FRI target | prover (ms) | verifier (ms) | proof (B) | peak (MiB) | swapouts |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 2^15 : 2^7 | 112 | 49.5 | 2.58 | 265,264 | – | 0 |
| | 100 | 50.5 | 2.59 | 245,104 | 681 | 0 |
| | 96 | 50.2 | 2.52 | **237,584** | 690 | 0 |
| 2^17 : 2^9 | 112 | 128.2 | 7.32 | 314,192 | 2452 | 0 |
| | 100 | 128.3 | 7.37 | 286,128 | 2429 | 0 |
| | 96 | 127.6 | 7.12 | **277,008** | 2438 | 0 |
| 2^19 : 2^11 | 112 | 444.2 | 33.80 | 362,464 | 9620 | 0 |
| | 100 | 449.5 | 34.19 | 333,344 | 9583 | 0 |
| | 96 | 445.5 | 33.15 | **322,544** | 9645 | 0 |
| 2^20 : 2^12 | 112 | 891.9 | 69.64 | 389,648 | 16578 | 0 |
| | 100 | 902.2 | 72.32 | 357,840 | 17107 | 0 |
| | 96 | 859.5 | 66.25 | **346,080** | 14702 | 0 |
| 2^21 : 2^13 | 112 | 4718.2† | 1051.46† | 424,608 | 18614 | 101,732 |
| | 100 | 4792.8† | 917.99† | 386,240 | 18865 | 85,420 |
| | 96 | 5292.6† | 815.10† | **373,360** | 18247 | 123,680 |

`†` Every 2^21 `all-binius` case paged (swapouts in the last column), so those
three timings are not comparable with each other or with the rows above them.
Proof sizes are deterministic and unaffected.

- **Proof size**: relative to the 112-bit rows, 100 bits removes 7.6–9.0% and
  96 bits removes 10.5–12.1% (2^21: 424.6 → 386.2 → 373.4 KB). Prover and
  verifier times are unchanged within run-to-run noise at every size that did
  not page, as expected: the target only sets the FRI query count.
- **These knobs are query-phase targets, not security levels.** Binius64's own
  documentation says so: "The target covers only the query phase, not the
  combined soundness error of the complete protocol." `security_bits` feeds
  `calculate_n_test_queries(security_bits, log_inv_rate)` and nothing else;
  the binary PIOPs, the ring switch and the sumchecks are not budgeted against
  it. F2Z's hybrid instead reports a union bound over every term of the whole
  composition (`prepared.security()`, gated at 100 bits with the per-component
  targets set ABOVE 100 so the union lands at 100). A Binius64 row at "96" or
  "100" and a hybrid row at "100" are therefore NOT commensurable: the Binius64
  number is one component's query-phase floor, the hybrid number is the
  end-to-end bound. The rate-1/8 comparison below keeps the 112-bit rows as
  the baseline and shows the 96/100-bit rows only as the range Binius64's own
  defaults would give it.

## Reading the table (v2 baseline)

These bullets describe the v2 hybrid rows above; the v3 opener at the top of
this document supersedes the proof-size conclusion.

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
