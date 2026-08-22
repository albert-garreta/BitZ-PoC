# Integer inner products: F2Z vs Binius64 machinery — measured

2026-08-22, MacBook Air M4 16 GB, 10 rayon threads both sides, same GHASH
GF(2^128) field both sides. Binius64 = upstream `binius-zk/binius64` main
@ `7ad579a1` (worktree `~/binius64/.claude-worktrees/upstream-bench`),
built `--release --features rayon`, `-C target-cpu=native`. F2Z = this
repo, LTO+unchecked, private target dir. Box shared with another active
session; medians of 3 reps with first-rep warmup discounted, ±10–20 %
band; the 2^26 rows ran at 5.7–6.2 GB RSS (memory-pressured — which is
part of the finding).

Drivers (in the binius64 worktree, examples of `binius-ip-prover` /
`binius-prover`):

- `crates/ip-prover/examples/f2z_setting1.rs` — setting 1, monolithic
- `crates/ip-prover/examples/f2z_setting1_split.rs` — setting 1, split-gifted
- `crates/prover/examples/f2z_setting2.rs` — setting 2

All three assert the product root equals `g^μ` against a direct integer
computation, so the constructions are verified end-to-end. Reproduce:

```sh
cd ~/binius64/.claude-worktrees/upstream-bench
CARGO_TARGET_DIR=$PWD/target-bench RUSTFLAGS="-C target-cpu=native" \
  cargo build --release --features rayon \
  --example f2z_setting1 --example f2z_setting1_split -p binius-ip-prover
CARGO_TARGET_DIR=$PWD/target-bench RUSTFLAGS="-C target-cpu=native" \
  cargo build --release --features rayon --example f2z_setting2 -p binius-prover
./target-bench/release/examples/f2z_setting1 24 100 3 2        # n, weight_bits, reps, switchover
./target-bench/release/examples/f2z_setting1_split 24 14 100 3 2
./target-bench/release/examples/f2z_setting2 16 3
# F2Z arm (this repo):
F2Z_BENCH_SHAPES=14:10:1 F2Z_BENCH_REPS=3 OBLONG_PROFILE=1 \
  RUSTFLAGS="-C target-cpu=native" cargo bench --bench pcs --features unchecked
```

## Setting 1 — `g^{⟨v,w⟩} = g^μ`, v public (~100-bit, tensor), w committed bits

Three arms, same claim family (weight_bits = 100, matching our chunkless
q≈100-bit regime — F2Z bench headers confirm `chunks=1` at every shape):

1. **Binius64 machinery, structure-oblivious**: one eager `ProdcheckProver`
   over the bit-affine leaves `1 + w_i(g^{v_i}−1)`, leaf claim discharged
   by `SelectorMlecheckProver` (switchover 2). Leaf generation = ℓ
   exponentiations.
2. **Binius64 machinery, split handed for free**: `v = v² ⊗ v¹`, ℓ₁
   exponentiations only, their prodcheck run natively batched
   (k = r₁ product vars over 2^{r₂} columns), same selector discharge —
   but the shared `v¹` bases must be physically tiled into an ℓ-length
   `selected` buffer and folded densely every round (their machinery has
   no symbolic sharing).
3. **F2Z**: the production forest (case-LUT rounds, stash, JIT), phase
   `forest+presum` from `OBLONG_PROFILE=1`; shapes `t:s:1` with t≈0.6n
   matching the split arm's r₁.

### Prover time (median, MT)

| ℓ | B64 monolithic | B64 split-gifted | F2Z forest | F2Z full prove* |
|---|---|---|---|---|
| 2^20 | 98 ms (leafgen 90) | ~11 ms | **6.6 ms** | 10.1 ms |
| 2^22 | 399 ms (leafgen 361) | ~33 ms | **12.2 ms** | 18.3 ms |
| 2^24 | 1 790 ms (leafgen ~1 550) | ~133 ms | **38.0 ms** | 49.9 ms |
| 2^26 | 7 229 ms (leafgen ~6 025) | ~1 835 ms | **104.3 ms** | 129.0 ms |

\* full prove includes the Ligerito opening; the Binius64 arms stop at
bit-column claims with no commitment opening, so **forest** is the
apples-to-apples column (it also stops pre-opener but includes the τ
builds and the transcript grind).

Ratios vs F2Z forest: monolithic **15× / 33× / 47× / 69×**; split-gifted
**1.7× / 2.7× / 3.5× / 17.6×**.

### Peak memory (max RSS vs bench heap peak)

| ℓ | B64 monolithic | B64 split | F2Z prove peak |
|---|---|---|---|
| 2^20 | 121 MB | 106 MB | **6.4 MB** |
| 2^22 | 480 MB | 412 MB | **24.7 MB** |
| 2^24 | 1.90 GB | 1.63 GB | **95.5 MB** |
| 2^26 | 6.22 GB | 5.66 GB | **378 MB** |

### Decomposition of the advantage

- **The split is the first-order term** (10–30× and growing): without it
  their prover pays ℓ exponentiations for the leaf values — ~86 ns/exp
  parallel, >90 % of their runtime at every size. This is the
  O(ℓ₁)-vs-O(ℓ) exponentiation-count argument, measured. (The verifier
  asymmetry is the same term: their verifier would evaluate the public
  leaf MLE in O(ℓ); ours is O(ℓ₁+ℓ₂).)
- **Tables + laziness are the second-order term** and grow with size:
  1.7× → 17.6× time and a steady ~15–19× memory. At 2^20–2^24 the gap is
  dense-fold vs case-LUT arithmetic; at 2^26 their eager layer chain
  (2× leaves) plus the tiled selected buffer fall out of cache and the
  memory wall feeds back into time, while F2Z's tree-shared tables keep
  the working set at 1/15th.
- **Expressibility ceiling** (not in the tables): the monolithic arm
  cannot even express our production claim — tensor weights
  `v²_j · v¹_i` at ~100-bit factors are ~200-bit ≥ ord(g). Only the
  split form fits the exponent budget; the split is not just faster, it
  is what makes the claim provable in GF(2^128) at all.

## Setting 2 — `g^{⟨v,w⟩} = g^μ`, both v and w committed 64-bit words, no tensor structure

Binius64 native machinery: per-row `g^{v_i·w_i}` via the full intmul
protocol (four product trees, Frobenius selector discharge, LogUp\*
power-table read with its pushforward oracle committed over a real
BaseFold channel, rate 1/2), plus one accumulation prodcheck over the n
per-row roots giving `g^{Σ v_i w_i mod 2^128−1}` — verified against a
direct mod-(2^128−1) computation.

| rows n | total (median) | witness | acc tree | intmul | per product | RSS |
|---|---|---|---|---|---|---|
| 2^12 | 16.0 ms | 1.6 | 0.1 | 14.3 | 3.9 µs | 42 MB |
| 2^14 | 22.7 ms | 3.9 | 0.2 | 16.8 | 1.39 µs | 104 MB |
| 2^16 | 51.6 ms | 15.9 | 0.7 | 34.9 | 0.79 µs | 373 MB |
| 2^18 | ~230 ms | ~120 | 2.0 | ~106 | 0.88 µs | 1.45 GB |
| 2^20 | ~2 100 ms | ~1 100 | 14 | ~985 | 2.0 µs | 4.62 GB |

Same-ℓ ladder as setting 1 (requested): measured through 2^20; the
per-product cost rises 0.79 → 2.0 µs between 2^16 and 2^20 as the
~5.5 KiB/row eager witness (four materialized tree-layer chains) leaves
cache. **2^21 needs ~9 GB and 2^22 ≈ 18–23 GB — past this 16 GB box**:
the same-ℓ points 2^22–2^26 are unrunnable in one shot. Time
extrapolates linearly at the ≥2^20 rate (2^22 ≈ 8.5 s, 2^24 ≈ 34 s,
2^26 ≈ 135 s, labeled EXTRAPOLATED); memory extrapolates to 18/74/296 GB,
which is the real bound. A production deployment would batch rows
(their M4 chip system is data-parallel batching), making the wall
per-batch — the time extrapolation then stands, the memory bound
becomes the batch size.

This is their home turf and it is genuinely strong at cache-resident
sizes: ~0.8 µs per 64×64→128 product-with-accumulation at 2^16 rows,
with a ~15 ms LogUp\*/table fixed floor. **F2Z has no native mechanism for this claim**
— the exponent is bilinear in witness, so there is no public factor to
tabulate and no low-entropy leaf property; per the muls audit, the
import path (their variable-base + Frobenius machinery on top of our
commitment) prices a word×word Hadamard opening at ≈3–4× a plain
opening.

### Cross-setting comparison — normalize by committed witness bits (ℓ = 2^{i+7})

A setting-2 instance at 2^i rows commits 128·2^i = 2^{i+7} statement-
witness bits (v and w, 64 each per row); a setting-1 instance at
ℓ = 2^{i+7} commits the same. Since both deliver one scalar claim over
their committed input, equal committed bits is the right normalization
for the task "prove one inner product over committed data" — this is
the headline cross-setting table (F2Z shapes measured directly at
n = 23, 25, 27, t = 0.6n, chunks=1):

| committed bits | Setting 2 (2^i rows) | Setting 1 F2Z forest (ℓ=2^{i+7}) | time ratio | memory |
|---|---|---|---|---|
| 2^23 | 2^16 rows: 51.6 ms | n=23: 19.4 ms | **2.7×** | 373 MB vs 49 MB |
| 2^25 | 2^18 rows: ~230 ms | n=25: 58.1 ms | **4.0×** | 1.45 GB vs 190 MB |
| 2^27 | 2^20 rows: ~2 100 ms | n=27: 188.3 ms | **11.2×** | 4.62 GB vs 748 MB |

Per committed bit: setting 2 runs 6.2 → 6.9 → 15.6 ns/bit (degrading —
the eager working set leaves cache) while the F2Z forest runs 2.3 →
1.7 → 1.4 ns/bit (improving — table amortization); hence the widening
ratio. With the F2Z full prove (Ligerito opening included, which the
setting-2 arm lacks) the ratios are 1.9× / 3.2× / 9.5×.

Conventions and caveats for this table: (i) 128 bits/row counts the
STATEMENT witness (v, w); their realization also commits the c_lo/c_hi
advice columns (256 bits/row) and the LogUp\* pushforward — counting
those would match ℓ = 2^{i+8} and move the factor ~2× in their favor;
state the convention when quoting. (ii) The claims still differ in
kind — public tensor-structured weights vs witness weights — which is
the structural point being illustrated, not an unfairness. (iii) The
per-position (equal-ℓ) framing answers a different question: at
ℓ = 2^20 the witness×witness claim costs ~320× the time and ~740× the
memory of the public×bits claim per claim dimension.

## Verdict

The two settings measure the two sides of the same structural fact. On
⟨public-tensor, bits⟩ claims (setting 1) the F2Z mechanism is 15–69×
faster and ~15–19× leaner than Binius64's machinery used naively, and
still 1.7–17.6× faster when their machinery is handed the tensor split
for free — the residual being exactly the case-LUT/stash/JIT layer they
cannot replicate without public shared leaf values. On
⟨witness, witness⟩ claims (setting 2) their machinery does sub-µs/word
products that F2Z cannot express at all. The claim shapes select the
machinery; neither side's optimizations transfer across the boundary.

## Caveats

- Shared box (another session active); medians, warm reps; 2^26 rows
  memory-pressured, affecting both sides alike.
- Binius64 built with their stock release profile (no fat LTO; F2Z uses
  LTO+cu=1 per its own conventions) — each project's canonical perf
  profile; LTO would not change the orders of magnitude.
- Their arms carry no Fiat-Shamir grinding; the F2Z prove includes its
  grind. Favors them slightly.
- Selector switchover fixed at 2 (their intmul-default neighborhood);
  varying it moves the discharge by ±tens of ms at 2^24+, not the story.
- Setting-2's accumulation prodcheck and intmul run on one channel but
  are not point-glued; intmul samples its own opening point, and its
  cost is point-independent, so timing equals the composed protocol.
