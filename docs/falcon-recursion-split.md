# Falcon recursion: SHA batch + glue in one commitment (BitZ paper §2.4.1)

Status (2026-10-21): implemented, tested and measured (`c07b28ebc`); it does not
beat FlockRec (see Results). User
directives: follow §2.4.1 (one commitment of the concatenation, each
sub-SNARK run until an inner-product claim over the binary field, the claims
proved together); no second commitment, no linking step; accept the SHA
batch's slower witness generation and report it separately.

Code: `src/piop/spartan/split.rs` (the relation pair), `src/bitz/shared.rs`
(the shared opening), `FlockRecRelation::new_stacked` + `Stacked`
(`src/piop/spartan/flockrec.rs`), the PIOP/opening splits of the linear PIOP
(`protocol/linear.rs`: `bind_linear_statement`, `prove_linear_piop`,
`verify_linear_piop`) and of FlockRec (`bind_statement`, `prove_piop`,
`verify_piop`), the SHA batch's entry points (`sha256::shared`), the Falcon
constructors (`falcon_verifier::split_relation`, `replay_split_relation`),
the bench mode (`BITZ_REC_SPLIT=1`).

## Relations

- Part A (SHA batch): every circuit compression as one instance of the
  Ch-only CM relation, `N = 2^k` instances (`k = ⌈log₂ n⌉`, at least 7): the
  circuit's `n`, then dummy compressions of zero inputs that nothing reads.
  Private I/O, block layout `2^(13+r) × 2^(k−r)` (instance `i` = the
  `2^13`-row chunk at flat `i·2^13`; local column 0 its constant, block
  words 1..512, state 513..768, output word `w` bit `b` at `6881 + 33w + b`,
  live width 7,145). Linear PIOP + product sumcheck, stopped before the
  opening: a claim on the product tensor `2^(14+r) × 2^(k−r)`, which the
  block map transposes to `⟨R ⊗ u₂, f⟩`. `BITZ_REC_SHA_T = 14 + r`
  (unset: the batch's own split, `r = 2` at `k = 13`; 20 exceeds the
  projection-draw grinding cap at λ=128).
- Part B (glue, `Glue`): the recording with each compression block replaced
  by a stub template — hints 0..768 the inputs, 768..1024 the outputs (both
  Part A's cells of that instance), 1024..1280 the output copies; cells
  `input_s ⊕ slot_s` and `output_k ⊕ copy_k`, each with the row `cell = 0`;
  outputs the copies. Committed bits renumbered (stubs `i·1280..`, then the
  tail in order; a compression's output hint becomes its stub's copy; no
  other block hint may be read from outside), cells and rows likewise
  (stubs', then the tail's; tail rows read tail cells only). Its witness:
  inputs from the wiring evaluated on `g`, outputs and copies from the
  output hints, the tail's bits; `h'` the constant, zeros, the tail cells.
  Proved by FlockRec on the shared grid (`Stacked`), stopped before the
  opening: a claim on its derived grid, transposed through the stacked map
  to structured weights.

## Shared grid (`Stacked`)

Part A's block layout. Stub hint `h` of instance `i` at flat `i·2^13 +
pos[h]`: inputs and outputs at Part A's columns, copy `k` at
`2^13 − 2^b + k`. The box (local rows `2^13 − 2^b .. 2^13`, `b` the
smallest of 8..13 clear of the live width with room) holds the copies, then
— block after block, whole boxes past the stubs — the glue's constant and
the tail's committed bits. Part A's map reads nothing there.

Structured weights: a committed column holds `2^r` consecutive blocks; with
`hr − ha ≥ r` they share an opening column and have `2^r` consecutive
classes, so the column is `col_w·S_κ` for one of `2^(hr−ha−r)` stacked class
vectors (one more for a partly filled last column); every external read and
the constant land in `2^r` row boxes (one per stacked block, `2^(b+s)`
entries each); no column box (a zero one on column 0); `term2` is free.
The verifier's lazy MLE is FlockRec's with the block chunk `2^fa = 2^13`.

## Protocol

`protocol` frame; Part A's statement (frames incl. the commitment, opener
config); Part B's; one Round 0; Part A's PIOP (its prime, grinding,
batching, product sumcheck, bound claim); Part B's PIOP (its prime,
grinding, batching, inner sumcheck, bound claim); BitZ state forked; the
shared statement (root, both frames, PCS, both claims); each claim's fold +
GKR under an integer-only native schedule; Part A's block transpose, Part
B's structured weights; `γ`; one structured sumcheck over `w_B + γ·(R ⊗
u₂)` with target `t_B + γ·t_A`, ring switch, Ligerito (Round-0 claim
batched). Verifier: the same, the batched weights' MLE lazily.

## Tests

`split::tests`: the stub circuit holds on the glue witness (flattened
`check_witness`) and a wrong copy breaks it; the stacked map's fast
transpose = the generic column-by-column one (sampled), structured weights
= dense, lazy MLE = dense MLE, `derive_rows` on the shared rows = the glue
witness (`r = 0` and `r = 2`); prove/verify at λ=100 `custom:3:4` and
λ=128 `ext:3:4` with tampered opening narg/hint, glue sumcheck, batch
product sumcheck and Round-0 value rejected. `falcon_verifier::tests::
falcon_recursion_split_step_proves_and_verifies` (ignored): the Falcon
step end to end, final verifier from the public stream, tampered opening.

## Results (gated campaign, `results/falcon-recursion-split-20261021/`)

λ=128, ext:3:4 base and outer, 10 threads, medians (summary.md has the
full table). Step without the batch's witness generation / π:

| batch | FlockRec t18 | FlockRec t17 | FlockRec t16 | FlockRec t15 | split 17-15 | split 16-15 |
|---|---|---|---|---|---|---|
| 1,024 | 0.47 s / 170.6 KB | 0.39 s / 178.4 KB | 0.37 s / 192.8 KB | 0.34 s / 226.0 KB | 0.55 s / 192.7 KB | 0.47 s / 208.4 KB |
| 4,096 | 0.50 s / 169.8 KB | 0.41 s / 177.4 KB | 0.38 s / 193.9 KB | 0.38 s / 225.9 KB | 0.53 s / 193.2 KB | 0.52 s / 208.2 KB |
| 32,768 | 0.66 s / 178.1 KB | 0.64 s / 195.0 KB | 0.56 s / 226.3 KB | 0.54 s / 291.5 KB | 0.71 s / 197.9 KB | 0.69 s / 213.6 KB |

The batch's witness generation adds 0.55–0.62 s to the split. The split's
protocol (0.20–0.27 s) beats FlockRec t18's (0.29–0.35 s), but its setup
and verifier cost more and its proofs are larger; FlockRec at T = 16–17
dominates it on time and size at every batch. FlockRec already prices a CM
block like the block opener prices a compression (GKR over derived cells),
the 2^13 batch pads with 37 / 25 / 6 % dummies, and the split pays a second
PIOP's proof of work and a second fold + GKR. The lever this found is the
opening rows: each PIOP round grinds ≈ rows + 2 bits at λ=128 (the prime is
at most min(113, 128 − rows) bits), so FlockRec at T = 17 or 16 is 15–17 %
faster per step for 5–27 % more proof.

Not done (would not change the comparison): a word-level packer for the
batch's witness (two serial bit loops, ~0.5 s of the 0.55–0.62 s), a shared
prime and initial boundary for the two PIOPs, a faster stub/relation build.
