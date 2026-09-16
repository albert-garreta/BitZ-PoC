# The paper's schemes on BitZ's PCS (branch `bitz-pcs-schemes`)

Date 2026-09-16. Branch `bitz-pcs-schemes` stems from `bitz-parity` at
`afbfd73` — the parity prover of BitZ's PCS (`src/bitz/`, byte-identical
to `worldfnd/f2z-benchmark`'s opening at every size), before any of the
end-to-end transcript-parity work. The goal here is different: **run the
paper's experiments with the schemes of `master` — the Spartan PIOP over a
transcript-sampled prime, the bitification, the statement binding, the
grinding schedule — but with the polynomial commitment scheme swapped for
BitZ's**, so the two openers are compared inside the same protocol on the
same relations, at the same security accounting, with grinding where the
opener's ladder needs it. Transcript parity with their end-to-end prover is
not a goal of this branch.

## Where things are

- `src/piop/spartan/protocol/bitz_opener.rs` (feature `bitz-parity`) —
  the opener. `BitzOpener::new(layout, ladder, target_bits)`,
  `commit(rows)`, `prove(transcript, prefix, opener, witness, hint,
  options) -> Proof<BitzOpeningProof>`, `verify(...)`,
  `opening_bits()` (the opening's round-by-round accounting),
  `BitzLigerito::{Fast, Selected(LigeritoSelection)}`, `BitzOpeningProof`
  (the forked transcript's narg string and hints; implements
  `OpeningProof`).
- `src/bitz/pcs.rs` — `Pcs::with_security(shape, &LigeritoSecurityConfig,
  profile)`: BitZ's PCS over any validated flock ladder (the crate's
  resolved selections), not only the embedded `fast` one.
- `src/bin/f2z.rs` — `f2z --mul <e> --pcs bitz [--profile
  fast|udr|custom:<r>:<k>]` (and `--mul-sweep … --pcs bitz`): the same
  measurements and `RESULT schema=f2z-cli-mul/2` line as the F2Z opener,
  BitZ's phases in the opening buckets, `pcs=bitz` appended.
- `benches/mul_e2e_compare/f2z.rs` + `scripts/run_native_mul_compare.py`
  — `F2Z_PCS=bitz` runs the u32/u64/u128 workloads through the BitZ opener
  (`F2Z_LIG_PROFILE` = its ladder; the runner records the knob and adds the
  feature).

## How the opener plugs in

The unified runner (`protocol::prove_with_opener`) binds the statement,
grinds and draws the Step-2 prime, runs the Spartan PIOP under per-draw
grinding, bitifies the terminal claim and grinds the terminal boundary,
then discharges the bitified functional. The BitZ opener replaces only the
last step:

1. The bitified claim is a rank-one functional over the committed bit
   tensor: `Σ_r Σ_c w_r · v_c · D[r, c] ≡ claimed (mod q)` with dense row
   weights `w` (`bitify::dense_row_weights`: one factor per variable block
   times the slot weights times the equality table of the high gate
   coordinates) and column weights `v` (`bitify::column_weights`: the
   scaled equality table of the low gate coordinates). BitZ's
   `LinearClaim` is exactly that — row weights, column weights, a target
   — so no translation exists between the two openers.
2. The commitment is the same flock commitment over the same per-column
   bit rows (`f2z_bit_rows`), made under the ladder's level 0 (its rate
   and interleaving). The statement binding covers the ladder's prover and
   verifier configurations (`Opener::Custom`), and the fork's session tag
   and ladder digest are absorbed before the prime draw.
3. BitZ's fold bound, `(q − 1)(2^t + 1) < 2^127`, is the profile's
   direct-opening prime cap at word width one (`q_bits ≤ 126 − t`), so the
   profile's prime interval needs no change. Word width one only (BitZ
   commits bits).
4. BitZ runs its own transcript (spongefish with a hint channel). It is
   forked after the terminal boundary: the fork's instance tag is a
   32-byte squeeze of the outer state, the bridge digest is absorbed
   again inside, and nothing is drawn from the outer transcript after
   the fork. The fork's narg string and hints are the opening proof.

## Security and grinding

BitZ's opening draws only `GF(2^128)` challenges outside Ligerito — the
fold batching point, the GKR rounds, the reduction sumcheck, the ring
switch — so every one of its terms sits at the crate's field floor
(≈ 126.4 bits) and nothing outside Ligerito can be ground. The grinding
that matters is the ladder's:

- `fast` (BitZ as shipped): flock's embedded `fast` ladder for the size —
  a 100-bit Johnson ladder at rate 1/2, `k = 4`, 183 level-0 queries with
  16 bits of query grinding and 9 of fold grinding (at `m = 22`), the same
  family as the paper's `custom:1:4`. BitZ has no early Round 0 (the
  out-of-domain sample the crate binds right after the commitment to pin
  the list before the first fold challenge); this is reported, not
  patched: the row is "BitZ as it ships".
- `udr` (`udr:1:4`, or any `udr:<r>:<k>`): the crate's validated
  unique-decoding ladder with fold grinding at the profile's target — no
  Round 0 needed, so the opening is sound at the target as it stands
  (100.2 bits at the sizes below; larger proofs: 240 vs 163 KB of hints at
  `2^19` products).
- `custom:<r>:<k>` (a Johnson ladder of the crate's own): runs, but
  without Round 0; use it only to compare ladders, not as a security row.

`opening_bits()` reports the weakest ladder level (paper-predicted
proximity-gap and query terms plus their grinding) against the field
floor; the result line's `lambda_achieved` is the minimum of that and the
PIOP's accounting, `lambda_bind` names the binding term.

Adding Round 0 to the BitZ opener (bind the OOD sample after the
commitment, batch `η_ood` into its Ligerito basis) is the one piece that
would put a Johnson ladder under BitZ on the crate's footing; it is not
done here.

## Numbers (M5, 10 threads, `f2z --mul <e> --reps 3`, medians; prove = end to end incl. commit, excl. witness)

u32 products `x·y = z + 2^32 w`, the paper's u32 relation, `Lambda100`:

| e | F2Z prove | BitZ-fast prove | BitZ-udr prove | verify F2Z / BitZ | proof F2Z / BitZ-fast / BitZ-udr | peak F2Z / BitZ |
|---|---|---|---|---|---|---|
| 15 | 18.3 ms | 31.8 | 28.8 | 3.1 / 2.5 ms | 119 / 118 / 158 KB | 29 / 47 MB |
| 17 | 41.7 | 57.2 | 53.8 | 3.7 / 3.3 | 152 / 151 / 204 | 110 / 119 |
| 19 | 116 | 119 | 114 | 4.5 / 4.9 | 188 / 187 / 265 | 425 / 343 |
| 21 | 425 | 324 | 315 | 6.2 / 9.2 | 227 / 225 / 322 | 1,669 / 1,104 |
| 23 | 1,902 | 1,151 | 1,130 | 9.0 / 17.4 | 280 / 278 / 406 | 6,615 / 3,886 |

The PIOP, bitification and commit are identical on both rows; the
difference is the opening: F2Z's grand products (the chunked exponent-fold
forest) 11 / 27 / 84 / 338 / 1,579 ms against BitZ's folds + GKR 24 / 41 /
87 / 235 / 839 ms, then F2Z's ring switch + Ligerito 4 / 7.5 / 13.6 / 30.6
/ 108 against BitZ's 4.9 / 8.7 / 14.1 / 31.6 / 94.6. The same crossover as
the standalone head-to-head of the two PCSs: BitZ is slower up to `2^19`
(1.7× at `2^15`), even at `2^19`, faster from `2^21` (1.65× at `2^23`,
with 40 % less peak memory). BitZ's verifier is slower at the top sizes
(its GKR verification grows with the row count). Proof sizes are equal
under the shipped ladder; the `udr` ladder costs 35–45 % more hints.

u64 and u128 (the native-mul comparison bench, `F2Z_PCS=bitz`):

| workload | 2^e | F2Z online prover | BitZ-fast online prover | verify F2Z / BitZ | proof F2Z / BitZ |
|---|---|---|---|---|---|
| u64 (`x·y = z_lo + 2^64 z_hi`) | 15 | 27.2 ms | 47.2 | 3.1 / 3.3 ms | 137 / 138 KB |
| u64 | 17 | 68.1 | 84.3 | 4.4 / 5.0 | 164 / 164 |
| u128 (`x·y = z`, 256-bit z) | 15 | 43.2 | 69.1 | 4.1 / 4.7 | 151 / 149 |
| u128 | 17 | 122 | 136 | 5.7 / 8.3 | 186 / 184 |

(The bench's "online prover" is commit + prove after witness generation;
`F2Z_PCS=bitz` with the shipped `fast` ladder. The same shape as u32: the
BitZ opening costs more at these sizes and the gap closes as the tensor
grows.)

## What is not done

- SHA-256 and the SHA chain discharge through the virtual map
  (`prove_virtual`); the BitZ opener would need the transpose onto the
  committed rows (the e2e branch's `virt.rs` has that reduction, single
  column, verifier-linear) — not wired.
- MultiSwap discharges through Strategy 2 (`prove_reduced`, a 113-bit
  reduction prime): BitZ's fold bound at `t = 15` caps the opening prime at
  111 bits, so the reduction-prime interval (and its grind) would have to
  be re-derived for this opener — not wired.
- SHA-256 + ECDSA (the composite kernel) — not wired.
- Round 0 for a Johnson ladder under BitZ (see above).
- The `--mul-sweep` LaTeX writer labels rows as the F2Z opener; a `--pcs
  bitz` sweep writes correct numbers under that label.

## How to run

```sh
cd ~/f2z-pcs-bitzpcs   # the worktree of this branch (its own target dir)
export CARGO_TARGET_DIR=$HOME/f2z-pcs-bitzpcs/target
RUSTFLAGS="-C target-cpu=native" cargo build --release --features span-metrics,bitz-parity,parallel,unchecked --bin f2z
B=$CARGO_TARGET_DIR/release/f2z
$B --mul 19 --reps 3                          # F2Z opener (custom:1:4 Johnson + Round 0)
$B --mul 19 --reps 3 --pcs bitz               # BitZ as shipped (fast ladder)
$B --mul 19 --reps 3 --pcs bitz --profile udr # BitZ under the validated UDR ladder
$B --mul-sweep 15-23 --reps 3 --pcs bitz --latex /tmp/u32-bitz.tex
RUSTFLAGS="-C target-cpu=native" cargo test --release --features bitz-parity,parallel --lib bitz_opener
# u64 / u128 through the comparison bench
F2Z_PCS=bitz F2Z_MUL_COMPARE_WORKLOADS="u64 u128" F2Z_MUL_COMPARE_BACKENDS=f2z F2Z_BENCH_SHAPES="15 17" \
  F2Z_BENCH_REPS=3 RAYON_NUM_THREADS=10 F2Z_MUL_COMPARE_MEMORY=0 F2Z_MUL_COMPARE_OUTPUT_DIR=/tmp/mulcmp \
  RUSTFLAGS="-C target-cpu=native" cargo bench --bench mul_e2e_compare --features bench-internals,native-mul-compare,bitz-parity
F2Z_PCS=bitz F2Z_MUL_COMPARE_WORKLOADS="u64" F2Z_MUL_COMPARE_BACKENDS=f2z python3 scripts/run_native_mul_compare.py   # the campaign runner
```
