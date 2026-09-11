# F2Z vs fields-witch (Lev Soukhanov's characteristic-2 field switch)

Date: 2026-09-11. Box: Apple M5, 24 GB, 10 cores, macOS 26.6.
fields-witch = `github.com/morgana-proofs/fields-witch` @ `30cca8c` (cloned to
`~/fields-witch`, built with `RUSTFLAGS="-C target-cpu=native"` under its own
release profile, `lto = "thin"` + `codegen-units = 1`; `cargo test --all-targets`
passes, 101 tests). F2Z = this crate @ `9d75b2b` plus the uncommitted working
tree, `--release --features unchecked`, `-C target-cpu=native`, opener
`custom:1:4` (the rate-1/2 Johnson default of every bench).

The paper's `\cite{lev}` paragraphs (§2 related work, line ≈340, and the
remark `r:comparison_grand_product_binius_lev`, lines ≈700–706) still say the
note "does not provide an implementation nor performance reports", and carry
two `\albertimportant` notes asking whether Lev has implemented it. He has:
fields-witch is Soukhanov's own implementation of the note (audited verifier
flow, optimized prover, August 2026). This document is the measured comparison
those notes asked for. §6 has drop-in replacement text.

Runner: `scripts/run_fields_witch_compare.py` (one fresh process per cell
under `/usr/bin/time -l`, CPU-idle gate ≥ 90 %, one untimed warm-up, medians
of 5 measured prove+verify passes). Raw records with the full program output
of every cell: `PerfRuns/2026-09-11T18-21-49Z-fields-witch-compare/results.jsonl`
(plus `-udr` and `-10thr` sibling directories for §3.4). LaTeX table:
`paper/fields-witch-table.tex` (not wired into `main.tex`).

## 1. What each scheme proves

| | fields-witch | F2Z (single-claim CLI path) |
|---|---|---|
| committed object | 2^k entries of F_{2^127}, read as integers in [0, 2^127); committed **densely** over F_{2^128} (16 B per entry) | 2^n cells of W bits, committed as **bits** (ring switch into F_{2^128} lanes; 2^n·W/8 bytes) |
| statement | MLE(d)(r) = v over F_p, **p = 2^127 − 1 fixed**. The Mersenne prime is the mechanism: F_{2^127}^× has prime order p, so every g ≠ 1 generates and k ↦ g^k is an isomorphism (Z/p, +) → (F_{2^127}^×, ·) | MLE[INT(D)](r) = y mod q for a prime q chosen by the application. The CLI samples q after the commitment from [2^{b−1}, 2^b), **b = min(113, 127 − t − W)** (the one-chunk rule); wider q needs L = ⌈q_bits / c_w⌉ exponent chunks at ≈ L× prover cost (library only, not in the CLI) |
| entry width | 127 bits of work per entry whatever the data: γ_i = r_i/(1−r_i) is a full F_p element, so every intermediate fold is full-width ("folds are full width", the note's own caveat) | cost ∝ total committed bits 2^n·W; the word grouping only enters through the one-chunk rule t + W ≤ 127 − q_bits |
| point | r ∈ F_p^k, r_i ≠ 1 | (r₁, r₂) ∈ F_q^{t+s} |

**Bit-matched shapes.** fields-witch at 2^k entries commits 2^k·127 bits,
the same 16 B/entry volume as F2Z at n = k + 7, W = 1; those are the rows of
§3.1. They are the same *service* (a binary commitment opened over a large
prime field) on the same data volume, not the same polynomial: fields-witch's
is the k-variate MLE of 127-bit words, F2Z's the (k+7)-variate MLE of the
bits. §3.3 adds F2Z rows at the same *entry count* with 32- and 64-bit cells,
which is the zkVM-shaped statement.

## 2. Mechanism

**fields-witch: the note, n rounds in the exponent of F_{2^127}.** One
variable per round, top index first, with the asymmetric fold
P_{i−1} = P_i(·,0) + γ_i·P_i(·,1). The prover keeps E_i = g^{P_i}; each round
is one pairwise-product sumcheck E_{k+1} = E_k^{(0)} · (E_k^{(1)})^{γ_k}
over F_{2^127}, after which the prover sends the plain evaluation of
E_k^{(1)} and a challenge glues the two halves into E_k. Every right branch
P_k^{(1)} (2^{k−1} full F_p elements, ≈ 2^{n−1} in total) is committed
densely as one concatenation and limb-decomposed with per-round limb widths
(the "schedule"). g^{limb} and g^{γ_k·limb} come from **indexed lookups**
into public tables: one packed Logup* over all 2·Σ limbs claims, with a
committed auxiliary polynomial the size of the packed table (2^18 entries at
2^20), fraction-tree sumchecks and an inner-product sumcheck against the
succinct table MLE. Limbs recombine by Frobenius (free) through balanced
**exponent product trees**, batched pairwise-product sumchecks per tree
level. A generalized **ring switch** (127 candidate bit evaluations per
source, one quadratic sumcheck) turns the F_{2^127} limb claims into dense
F_{2^128} openings, and **three recursive Ligerito openings** close the
proof: P_0, the concatenated tails, and the Logup auxiliary. The verifier
runs n product sumchecks + the tree levels + Logup* + the ring switch +
three openings; all of it polylogarithmic.

**F2Z: one round in the exponent of GF(2^128), read-off in the clear.** The
t row variables and the W word bits are folded once, in the exponent of a
transcript-sampled α with a generator check (bases α^{w_b·2^j}), certified by
the lazy bit-affine merged **GKR product forest**; the 2^s column values u_c
(integers below 2^{c_w+t+W}) are sent in the clear and the verifier recombines
y = Σ_c e_c u_c mod q itself. No lookups, no auxiliary commitments, no
per-round tables: the forest's leaf claims reduce to **one** ring-switched
Ligerito opening of the committed bits. Because the leaves are bits and the
bases are the 2^t public weights, the forest is low-entropy and the prover
runs on lookup-table rounds, the "low-entropy" optimizations of the paper.
The verifier is not polylogarithmic: it evaluates the lifted row-weight MLE
at the sumcheck point (2^t multiplications mod q, `mv:rhat`) and reads the
2^s column integers, i.e. O(2^t + 2^s) work on top of the opening.

| primitive | fields-witch | F2Z |
|---|---|---|
| exponent rounds | n (one variable each) | 1 (t + log₂W variables at once) |
| committed auxiliaries | ≈ 2^{n−1} dense F_{2^128} elements (all right branches) + the Logup table (2^18 entries at 2^20) | none (the 2^s clear-text column integers are proof bytes, 2 KB) |
| lookups | ≈ 2·N·⌈127/l⌉ limb lookups through Logup* (12.8 M at 2^20 with the README schedule) | none |
| product arguments | per-round pairwise product + per-source limb trees | one merged GKR forest over all folded variables |
| PCS openings | 3 (P_0, tails, aux), each recursive Ligerito | 1 |
| evaluation prime | 2^127 − 1 only | any q with q_bits ≤ 113 at one chunk (t + W ≤ 127 − q_bits) |
| Merkle hash | SHA-256 | BLAKE3 |
| opener regime | unique decoding, 100-bit query target, no grinding, no OOD; rates 1/2, 1/4, 1/8, 1/16 by level with 243 / 148 / 121 / 110 queries; 4 lanes folded per level | Johnson (provable list decoding), 100-bit round-by-round target, 16-bit query grinding, fold grinding, one OOD sample per recursive level plus the outer Round-0 OOD; at n = 27: rates 1/2 … 1/32 with 183 / 90 / 60 / 45 / 36 queries |
| verifier asymptotics | polylog(N) | O(2^t + 2^s) + polylog |

Both IOP layers run over ≈128-bit binary fields, so their sumcheck/GKR errors
(a few × n / 2^127) are negligible next to the 100-bit opener target; the two
systems are measured at the same nominal security (fields-witch's Ligerito
targets 100 bits by unique-decoding query count; F2Z's `security:` line
reports 100.0 achieved bits, minimum over levels and terms).

## 3. Measured

Prover = everything from the data to the proof, commitment included
(fields-witch `protocol total`; F2Z `commit + prove`). Peak = process max RSS
under `/usr/bin/time -l`. Every cell ran at ≥ 94.8 % measured CPU idle
(96–98 % for all but one).

### 3.1 Bit-matched (fields-witch 2^k × 127-bit entries vs F2Z n = k + 7, W = 1)

| committed bits | thr | scheme | shape | prover | verifier | proof | peak RSS |
|---|---|---|---|---|---|---|---|
| 2^21 | 1 | fields-witch | 2^14 entries | 25.9 ms | 7.1 ms | 420 KB | 78 MB |
| 2^21 | 1 | F2Z | n=21 (t=13, s=8), q 113 b | 16.2 ms | 1.3 ms | 105 KB | 36 MB |
| 2^21 | 8 | fields-witch | 2^14 entries | 18.5 ms | 7.8 ms | 420 KB | 79 MB |
| 2^21 | 8 | F2Z | n=21 | 10.4 ms | 2.0 ms | 105 KB | 36 MB |
| 2^23 | 1 | fields-witch | 2^16 entries | 81.9 ms | 8.4 ms | 561 KB | 242 MB |
| 2^23 | 1 | F2Z | n=23 (t=14, s=9), q 112 b | 53.3 ms | 1.7 ms | 137 KB | 117 MB |
| 2^23 | 8 | fields-witch | 2^16 entries | 40.3 ms | 9.3 ms | 561 KB | 245 MB |
| 2^23 | 8 | F2Z | n=23 | 23.1 ms | 2.6 ms | 137 KB | 112 MB |
| 2^25 | 1 | fields-witch | 2^18 entries | 293 ms | 9.4 ms | 693 KB | 855 MB |
| 2^25 | 1 | F2Z | n=25 (t=15, s=10), q 111 b | 185 ms | 3.0 ms | 170 KB | 406 MB |
| 2^25 | 8 | fields-witch | 2^18 entries | 109 ms | 10.8 ms | 693 KB | 864 MB |
| 2^25 | 8 | F2Z | n=25 | 59.6 ms | 3.8 ms | 170 KB | 452 MB |
| 2^27 | 1 | fields-witch | 2^20 entries (README schedule) | 1.02 s | 9.8 ms | 832 KB | 2.86 GB |
| 2^27 | 1 | F2Z | n=27 (t=17, s=10), q 109 b | 718 ms | 7.2 ms | 204 KB | 1.11 GB |
| 2^27 | 8 | fields-witch | 2^20 entries | 293 ms | 11.6 ms | 832 KB | 2.88 GB |
| 2^27 | 8 | F2Z | n=27 | 197 ms | 6.4 ms | 204 KB | 1.13 GB |
| 2^29 | 1 | fields-witch | 2^22 entries | 3.91 s | 11.6 ms | 980 KB | 10.55 GB |
| 2^29 | 1 | F2Z | n=29 (t=18, s=11), q 108 b | 2.90 s | 13.3 ms | 254 KB | 4.10 GB |
| 2^29 | 8 | fields-witch | 2^22 entries | 1.06 s | 13.7 ms | 980 KB | 10.69 GB |
| 2^29 | 8 | F2Z | n=29 | 759 ms | 10.0 ms | 254 KB | 4.06 GB |

Ratios fields-witch / F2Z:

| committed bits | prover 1 thr | prover 8 thr | verifier 1 thr | verifier 8 thr | proof | peak RSS |
|---|---|---|---|---|---|---|
| 2^21 | 1.60 | 1.78 | 5.5 | 3.9 | 4.0 | 2.2 |
| 2^23 | 1.54 | 1.74 | 4.9 | 3.6 | 4.1 | 2.1 |
| 2^25 | 1.58 | 1.83 | 3.1 | 2.8 | 4.1 | 2.1 |
| 2^27 | 1.42 | 1.49 | 1.4 | 1.8 | 4.1 | 2.6 |
| 2^29 | 1.35 | 1.39 | 0.87 | 1.37 | 3.9 | 2.6 |

### 3.2 Prover phase split (1 thread; 8-thread rows in `summary.md`)

| bits | scheme | commit | fold / core | opener | witness / other |
|---|---|---|---|---|---|
| 2^23 | fields-witch | 11.9 ms (dense 10.1 + aux 1.8) | 54.9 ms (main loop 23.5, Logup* 27.9, ring switch 3.5) | 3.1 ms | witness 11.8 (setup wall 21.9), claim check 0.2 |
| 2^23 | F2Z | 2.4 ms | 41.3 ms forest + 2.3 ms ring switch | 6.9 ms | 0.4 ms |
| 2^27 | fields-witch | 198 ms (dense 168 + aux 31) | 621 ms (main loop 224, Logup* 342, ring switch 55) | 37 ms | witness 156 (setup wall 324), claim check 3.8 |
| 2^27 | F2Z | 26.6 ms | 634 ms forest + 30 ms ring switch | 26 ms | 1.7 ms |
| 2^29 | fields-witch | 822 ms (dense 691 + aux 131) | 2.33 s (main loop 850, Logup* 1254, ring switch 217) | 151 ms | witness 601 (setup wall 1289), claim check 15 |
| 2^29 | F2Z | 71 ms | 2.67 s forest + 111 ms ring switch | 42 ms | 0.8 ms |

fields-witch's `setup wall` is the critical path of a `rayon::join` of the
witness branch (limb exponentiation tables, product-tree leaves, the tails)
and the dense commitments; at one thread it is their sum, at 8 threads they
overlap (2^27: witness 65 ms, dense 48 ms, wall 77 ms). Its `claim check`
is an O(N) validation of the public claim in F_p that a deployed prover would
skip (≤ 1.5 % of the total). F2Z's `prove` includes its α-power tables, the
forest, the fold integers, the ring switch and the opening.

Proof composition (instrumented `ProofWriter` offsets; local patch, reverted):

| | fields-witch 2^14 (430 KB) | fields-witch 2^20 (852 KB) | F2Z n=21 (105 KB) | F2Z n=27 (204 KB) |
|---|---|---|---|---|
| IOP messages (main loop + trees) | 60.4 KB | 43.5 KB | forest 12.1 KB | forest 29.0 KB |
| Logup* | 14.4 KB | 24.6 KB | — | — |
| ring switch | 31.1 KB (127 × 15 sources × 16 B) | 43.6 KB (127 × 21 sources × 16 B) | s_v 2.0 KB | s_v 2.0 KB |
| openings | 324 KB = P_0 130 + tails 108 + aux 85 | 740 KB = P_0 267 + tails 251 + aux 222 | Ligerito 88 KB | Ligerito 169 KB |

### 3.3 Same entry count, word cells (F2Z W = 32 / 64 at 2^20 entries)

fields-witch's cost at 2^20 entries is the 2^27 row above whatever the entry
width (1.02 s / 293 ms at 1 / 8 threads). F2Z at the same entry count:

| entries | W | thr | shape | q bits | prover | verifier | proof | peak RSS | vs fields-witch prover |
|---|---|---|---|---|---|---|---|---|---|
| 2^20 | 32 | 1 | t=12, s=8 | 83 | 192 ms | 6.8 ms | 159 KB | 301 MB | 5.3× faster |
| 2^20 | 32 | 8 | t=12, s=8 | 83 | 61.3 ms | 4.5 ms | 159 KB | 315 MB | 4.8× |
| 2^20 | 64 | 1 | t=12, s=8 | 51 | 407 ms | 26.0 ms | 178 KB | 570 MB | 2.5× |
| 2^20 | 64 | 8 | t=12, s=8 | 51 | 115 ms | 8.0 ms | 178 KB | 583 MB | 2.5× |

These rows are not at equal prime width: the CLI's one-chunk rule shrinks q
to 127 − t − W bits (83 and 51 bits here), whereas fields-witch always
evaluates over the 127-bit p. Keeping q ≥ 100 bits with 32-bit cells at
t = 12 would take L = 2 chunks (≈ 2× forest, ≈ 380 ms / 120 ms), still
2.4–2.7× ahead; with 64-bit cells L = 3 (≈ 1.2 s / 350 ms), about parity.
The bits-only route is cheaper: the library's read-off accepts arbitrary
public column weights, so a 2^20 × 64-bit word statement can be committed as
2^{22} 16-bit (or 2^{26} 1-bit) cells with the limb index among the s column
variables and the 2^{16j} place values folded into e_c in the clear; that
costs the W = 1 forest at 2^26 bits (≈ 400 / 120 ms) and keeps q at 111–113
bits. It is a harness change (the CLI only builds eq-tensor column weights),
not a protocol change.

### 3.4 Regime and thread checks at 2^27 bits

| variant | prover 1 thr | prover 8 thr | verifier | proof | note |
|---|---|---|---|---|---|
| F2Z `custom:1:4` (Johnson + grinding + OOD; the table above) | 718 ms | 197 ms | 7.2 / 6.4 ms | 204 KB (Ligerito 169) | |
| F2Z `udr:1:4` (unique decoding, no grinding, no OOD: fields-witch's regime) | 703 ms | 189 ms | 7.7 / 7.3 ms | 287 KB (Ligerito 250) | still 1.45× / 1.55× faster and 2.9× smaller than fields-witch |
| fields-witch, 10 threads | | 283 ms | 11.3 ms | 832 KB | |
| F2Z `custom:1:4`, 10 threads | | 182 ms | 6.4 ms | 204 KB | 1.56× |

The UDR row isolates the opener regime: F2Z's single UDR opening of 2^20
lanes costs 250 KB, fields-witch's P_0 opening of the same 2^20 elements
267 KB. Same opener family, same regime, same bytes; the 4× proof gap is
three openings against one, and Johnson + grinding then shaves another 30 %
off F2Z's.

## 4. Reading the numbers

- **Prover, equal bits: F2Z 1.35–1.8× faster.** F2Z is one pass: the GKR
  forest is 78–92 % of its time and everything else (commit 4 %, ring
  switch 4 %, Ligerito 1–4 %) is small. fields-witch spreads the same
  volume over four comparably expensive stages: the dense commitments of
  P_0 plus the 2^{n−1} tails plus the aux table (19–21 %, SHA-256 Merkle),
  the limb witness (15 %, overlapped under threads), the n product
  rounds (22 %), Logup* (34 %) and the ring switch (5 %). The ratio narrows
  with size because fields-witch's per-entry cost keeps falling (1.58 → 1.25
  → 1.12 → 0.97 → 0.93 µs per entry from 2^14 to 2^22: the amortization rule
  admits wider limbs at larger N, so round 0 needs 12 → 10 → 9 → 8 → 7 limbs
  per entry) while F2Z's per-bit cost is flat at ≈ 5.4 ns from 2^25 on.
  Both scale 3.5–3.8× from 1 to 8 threads; at 10 threads neither gains
  much more (memory bandwidth, as fields-witch's README warns).
- **Proof: 4× smaller.** Three openings against one (§3.2); the IOP parts are
  similar in size (fields-witch 112 KB of sumchecks + 44 KB of ring-switch
  α_i at 2^20; F2Z 31 KB).
- **Verifier: F2Z 3–5× faster up to 2^25 bits, then the O(2^t + 2^s) terms
  catch up.** fields-witch's verifier is ≈ 7–14 ms and nearly flat
  (core 5–7 ms: the ring switch's 127 α_i per source, the succinct table MLE,
  n product sumchecks; openings 2–6 ms). F2Z's lifted row-weight fold
  (`mv:rhat`) is 0.35 → 1.0 → 3.7 → 7.3 ms at t = 13, 15, 17, 18 (≈ 28 ns
  per row) plus the unattributed instance work, so the two cross between
  2^27 and 2^29 bits at one thread. This is the price of the single-round
  design (the weights are integer lifts of eq(r₁, ·) mod q, which has no
  tensor structure), not of the opener.
- **Memory: F2Z 2.2–2.6× less RSS.** fields-witch holds the exponent tables
  E_k and P_k of every round, the per-limb product trees of every source, the
  dense copies, the Logup histograms and three Ligerito scratches: 10.5 GB
  at 2^22 entries (64 MB of data). F2Z's forest is ≈ 4 B per committed bit
  plus the commit hint.
- **The word-width lever is F2Z's structural advantage.** fields-witch's
  intermediates are full 127-bit F_p elements, so 2^20 32-bit words cost it
  exactly what 2^20 127-bit words cost; F2Z pays per bit, hence 5× at 32-bit
  words and 2.5× at 64-bit words (§3.3, with the prime-width caveat and the
  limb-as-column route that removes it).
- **Prime flexibility is the other.** fields-witch is pinned to
  p = 2^127 − 1 by construction; F2Z takes any q (≤ 113 bits at one chunk),
  including the transcript-sampled primes the paper's PIOP relies on.

## 5. Caveats

- Different polynomials at equal bits (§1). The entry-count rows of §3.3 are
  the same statement shape but not the same prime width.
- fields-witch's limb schedules follow its README rule (packed table
  ≤ 2^{k−2} entries, ≥ 16 limb lookups per public table entry; minimal width
  per limb count), which reproduces the README's 2^20 schedule exactly. The
  rule is not tunable in a way that matters: limb counts are 8 for ≥ 16-bit
  limbs whatever the table budget, and lowering the amortization floor to 8
  changes only the last rounds.
- Hash functions differ (SHA-256 vs BLAKE3); both are the projects' defaults.
  fields-witch's commit phase is 7.5× F2Z's at 2^27 for 1.75× the data
  (dense 24 MB + aux 4 MB vs packed 16 MB).
- fields-witch's `retained scratch` mode reuses one shape-specific arena
  across repetitions (its sustained-load convention); F2Z allocates per
  proof. RSS is the maximum over the whole process either way.
- Both are single-machine numbers on a fanless laptop; the ±5–15 % band of
  the other campaigns applies, and the ratios above are well outside it.
- Neither implementation is audited beyond its own tests; fields-witch's
  README calls its verifier "audited" (message flow reviewed), its prover
  optimized separately.

## 6. Proposed paper text (not applied; paper edits are the user's)

Related work (§2, the sentence "The note does not provide an implementation
nor performance reports. We discuss this further in …"):

> Soukhanov has since released an implementation of the note
> (\texttt{fields-witch}, \url{https://github.com/morgana-proofs/fields-witch});
> we compare against it in \cref{r:comparison_grand_product_binius_lev}: at
> equal commitment size and a common 100-bit target, \ftwoz's prover is
> 1.4–1.8$\times$ faster, its proofs 4$\times$ smaller and its peak memory
> 2.2–2.6$\times$ lower, and the gap widens to 5$\times$ on 32-bit words
> because the note's folds are full-width regardless of the data.

Remark `r:comparison_grand_product_binius_lev`, replacing "though we do not
validate this hypothesis due to \cite{lev} not reporting performance nor
providing an implementation." and the two `\albertimportant` notes:

> We validated this against Soukhanov's implementation \texttt{fields-witch}
> on the same machine (Apple M5, 8 threads, \cref{tab:fields-witch}): opening
> $2^{20}$ committed $127$-bit words ($2^{27}$ bits) takes it 293 ms, 832 KB
> and 2.9 GB, against 197 ms, 204 KB and 1.1 GB for \ftwoz\ at the same
> $2^{27}$ committed bits; its verifier (10--14 ms, polylogarithmic) is
> 1.4--5$\times$ slower than ours up to $2^{27}$ bits and catches up at
> $2^{29}$, where our $O(2^t + 2^s)$ read-off dominates (13 vs 12 ms
> single-threaded). The proof gap is structural: the note
> commits every intermediate fold and opens three polynomials, we open one.
> Its cost is per $127$-bit word whatever the word width, ours is per bit,
> so on $2^{20}$ 32-bit words it stays at 293 ms while \ftwoz\ takes 61 ms.

Table: `paper/fields-witch-table.tex` (bit-matched rows, 1 and 8 threads).

## 7. Reproduction

```sh
git clone https://github.com/morgana-proofs/fields-witch ~/fields-witch   # 30cca8c
cd ~/fields-witch && CARGO_TARGET_DIR=~/fields-witch/target RUSTFLAGS="-C target-cpu=native" \
  cargo build --release --examples
cd ~/f2z-pcs && RUSTFLAGS="-C target-cpu=native" cargo build --release --features unchecked --bin f2z
python3 scripts/run_fields_witch_compare.py --sizes 14,16,18,20,22 --threads 1,8 --reps 5 --word-rows 20:32,20:64
python3 scripts/run_fields_witch_compare.py --sizes 20 --threads 1,8 --schemes f2z --f2z-profile udr:1:4   # §3.4
python3 scripts/run_fields_witch_compare.py --sizes 20 --threads 10                                        # §3.4
```

The runner derives fields-witch's limb schedules from the README rule and
asserts that it reproduces the README's 2^20 schedule
`16,16,16,15,13,13,12,11,10,9,8,8,7,6,5,4,4,3,2,2`; the 2^20 row uses that
schedule verbatim. The proof-size split of §3.2 came from a five-line local
patch printing `ProofWriter` offsets at the phase boundaries of
`Protocol::prove` (`FW_PROOF_SPLIT=1`), applied and reverted with
`git checkout`; the checkout is clean.

## 8. What in fields-witch could optimize F2Z (audit, 2026-09-11)

Method: two code inventories (fields-witch's prover techniques with file
pointers; F2Z's single-claim hot path incl. the flock-core opener), mapped
against F2Z's measured split at n = 27 (1 thread: forest 634 ms = 88 % of
718, of which LUT gather rounds ≈ 52 %, dense grid rounds ≈ 23 %, leaf
generation ≈ 16 %, level build ≈ 11 %; commit 27 ms, ring switch 30 ms,
Ligerito 26 ms). The ceilings below are shares of the n = 27 single-thread
prover; nothing here has been implemented.

| fields-witch technique (pointer) | F2Z today | verdict / ceiling |
|---|---|---|
| deferred carry-less reduction, 4-lane unreduced accumulators, unreduced rayon reduce (`field.rs:F2_254Unreduced`, `sumcheck.rs:product_round_gruen_high_block`) | `WideMulAcc`/`WideGf128` in every forest kernel, presum evaluator, basis fill; flock `F256Unreduced` in the lane folds | already there. Residual gap: flock's per-level `round_msg_lsb` / `introduce_new` / `glue` accumulate reduced products; F2Z's `eqf:close` combine. **< 1 %** |
| `mul_pair` two-lane VPCLMULQDQ products (`field.rs:mul_pair_vpclmul`) | NEON 4-PMULL + 2-stage `fold_x64`, vector-resident (Karatsuba/EOR3/b127 all measured worse) | x86-only. F2Z's non-NEON fallback is a Karatsuba `clmul_128x128`; matters only if F2Z is ever measured on x86. **0 % here** |
| Gruen two-accumulator round: only H(1) and H(∞) accumulated, H(0) from the running claim with a batched `(1+s)^{-1}` per round and the `s = 1` char-2 branch (`sumcheck.rs:product_round_gruen_sqrt` 1525–1587, `prepare_shared_linear0_inverses` 761–797) | three wide accumulators (A0, T11, A2) in every round body; Gruen on the wire only (verifier already reconstructs Ĥ0) | **the one real lever, 1–4 %** (the ledger's own unmeasured estimate: drops 1 of 3 wide products in the dense grid rounds and the `t_a0` gather stream in the leaf LUT rounds; byte-identical proofs). fields-witch's driver is a clean reference for the inverse bookkeeping |
| square-root eq split with per-block weighting and prefix-only materialization (`prepare_eq_split`, `fill_eq_prefix`) | eq-factored driver: prefix scalars + shared suffix-tensor arena per layer | equivalent or stronger. **0 %** |
| fused fold + next-message rounds, ping-pong buffers (`fused_fold_equal_product_tables_inner`) | pass fusion + double-fold (one pass per two rounds, 3×3 grid, arity-4 fixed-scalar fold) | F2Z is ahead. **0 %** |
| virtual zero/one padding, deviation tables, analytic constant tails (`ProductInput::PaddedTable`) | live-column elision (one synthetic all-ones group), zero-block skips in `sv_fold_mfr`, `xi_combined_rows_packed`, `fold_values_bits`, flock zero-twiddle path | equivalent for F2Z's shapes (all trees full-depth). **0 %** |
| two-sublimb windowed exponentiation, high window base by Frobenius (`witness.rs:exponentiate_value_limb_pair`) | `FixedBasePow` comb, 8-bit windows (12/16 measured worse) | ≈ 30 % fewer multiplies per ~110-bit exponent. Prover `mc:pow2` ≈ 0.5 % → **≤ 0.2 %**; verifier `mv:rhat` 7.3 → ≈ 5 ms at 2^29 (see below) |
| Four-Russians tables for F₂-linear maps: Φ_γ 8-bit windows, inverse-Frobenius 5-bit windows, 6-row weighted bit transpose (`ringswitch.rs:accumulate_weighted_rows`, `fill_phi_subsets`) | `sv_fold_mfr` (8×8 transpose + two 16-entry subset tables per 8 elements), `phi_byte_tables` (16 tables, η premultiplied), nibble LUT `fold_values_bits`, byte tables `xi_combined_rows_packed` | already there, same window sizes. **0 %** |
| additive NTT: rate-coset replicate-fill skipping the first r layers, fused two-layer top sweeps, cache-resident deep subtrees, sparse transposed NTT for the weighted basis row (`ntt.rs`, `ligerito.rs:prove_row_openings_and_build_basis`) | flock: replicate-fill + `from_layer(log_inv_rate)`, fused-2 top layers, 2 MB deep sub-groups, half-width/zero twiddles, `induce_sumcheck_poly_auto` sparse-prefix Fᵀ path | already there (commit = 4 % anyway). **0 %** |
| shape-simulated retained scratch arena, `spare_capacity_mut` writes, swap-in leaves (`protocol.rs:Scratch`, `sumcheck.rs:ProductSumcheckWorkspace`) | fresh `Vec`s per prove (≈ 1.6 GB first-touch at n = 27), flock 24-slot pool for lane folds | **measured dead**: `/usr/bin/time -l` page reclaims at n = 27 are 63.7 k for one proof and 68.6 k for three, i.e. ≈ 2.4 k pages (38 MB, ≈ 2–3 ms) per proof after the warm-up; macOS libmalloc already retains the large chunks. Remaining avoidable passes: `basis` zero-fill + `p_msg` zero-fill-then-copy (2 × 16 MB), per-layer clones of te/to, leaf_tau and tree bits (≈ 22 MB), flock's entry fold pass because `FoldLookahead` is passed as `None`. **≈ 1 % together** |
| `rayon::join` overlap of witness generation with transcript-independent commitments | not applicable: α is sampled after the commitment, so the forest cannot start before the root exists; the hybrid path tried overlapping its two commitments and dropped it (59.6 vs 61.3 ms) | **0 %** |
| quartet-fused SHA-256 leaf hashing, single-buffer leaf serialization, Merkle multi-proof | BLAKE3 `hash_many` 16-wide batches over 256 B leaves, octopus multi-proof per level; SHA-256 vs BLAKE3 measured a wash | already there. **0 %** |
| batch same-root openings, concatenated phase commitments, Logup* fraction trees | F2Z opens one polynomial and has no lookups (the limb-table idea was ruled out in the Binius64 audit: F2Z's bases are challenge-dependent) | not applicable |

Bottom line: fields-witch is a well-engineered dense product-sumcheck prover,
and F2Z already carries the generic part of its toolbox (deferred reduction,
eq factoring, fused passes, Four-Russians ring switch, coset-skipping NTT,
batched BLAKE3). The 52 % of F2Z's forest that is bit-driven LUT gathers has
no counterpart in fields-witch, so nothing in the repo touches F2Z's
dominant cost. The only prover item worth an experiment is the
two-accumulator Gruen body with batched inverses (1–4 %, byte-identical),
for which fields-witch is a reference implementation; the housekeeping
items (deferred reduction in flock's message passes, the two 16 MB
zero-fills and the `p_msg` copy, the per-layer clones, passing
`FoldLookahead`) add about 1 %.

**Verifier (the cell F2Z loses).** The comparison exposed F2Z's O(2^t + 2^s)
verifier terms at 2^29 bits: `mv:rhat` 7.3 ms (2^18 fixed-base comb
exponentiations, already parallel, plus a serial GF(2^128) eq table and dot
product) and the CLI's serial `eq_table_mod_q` instance tensor (≈ 4.4 ms,
the unattributed share). Two cheap, proof-invariant changes: build the
instance tensor as an outer product of two half tables (parallel), and take
fields-witch's two-sublimb exponentiation for the α^{w_b} bases. Estimate
at 2^29: 13.3 → ≈ 9 ms single-threaded, 10.0 → ≈ 6 ms at 8 threads, i.e.
below fields-witch's 11.6 / 13.7 ms at every measured size. Not applied.
