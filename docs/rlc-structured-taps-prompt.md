# Session prompt — RLC families over structured-sparse maps: ROT/SHIFT taps

You are in `/Users/albertgarretafontelles/f2z-pcs` (the `f2z-pcs` skill
applies: build/test/bench conventions, measurement protocol, commit style —
unsigned commits). Read FIRST:

1. `docs/rlc-family-note/main.tex` — the vetted family construction with
   measured results; especially the case-collapse (Lemma `lem:zeta`), the
   vanishing-channel elision (Prop. `prop:vanish`), the leaf-bit discharge
   and its 2-level cascade (Rem. `rem:dischargeleaf` + the j ≥ 3 status in
   the open-problems section), and the carries-are-conserved boundary.
2. The landed code (commits `8fbd155..cf2bebe`):
   `prove/verify_mle_eval_mod_q_ligerito_rlc_family` and the cascade
   (`rlc_channel_sides`, `rlc_prove/verify_eqf_level`) in
   `src/ligerito_flock.rs`; `extract_virtual_xor_rows` and the
   `ShaF2Layout` index math in `src/pcs.rs`; the ring/basis machinery
   (`embed_xor_index/point`, the sparse `s_v` walks); the forests in
   `src/merged_forest.rs` (`prove_merged_forest_lazy_rlc2`,
   `..._rlc_general`).
3. The README's dated RLC notes (measurement protocol + pitfalls:
   deterministic FS grinding luck, churned box, statement averaging).

## The setting

Committed columns `a_1, …, a_j` (same dimension: `N = 2^{num_vars}`
entries of `W = 2^{bit_vars}`-bit words, under a `ShaF2Layout`). The
claims are on derived vectors `b_1, …, b_k` where `b_i = M_i · (a_1 … a_j)`
over `F_2`, with `M_i` **sparse and highly structured**: its entries are
sums of operands `ROT^c` (cyclic bit-rotation within a word) and
`SHIFT^c` (logical shift, bits drop out), applied to column entries at
**offset entry indices** (convolution-style taps `k−1, k−2, …` along the
entry axis, truncated at the boundary). The goal: run the grand-product
machinery (the mod-q integer-MLE claims — the forest IS the grand
product) over the `b_i` at family cost, not at k-forests cost.

## The concrete target instance (j = 2, k = 6)

Entries indexed `k = 1..N`; boundary convention: out-of-range entries are
zero (entry-offset = SHIFT along the entry axis). `⊕` is wordwise XOR.

```
b_1 = a_1
b_2 = a_2
b_3[k] = ROT^1(a_1[k]) ⊕ ROT^2(a_1[k−1]) ⊕ ROT^3(a_1[k−2])
b_4[k] = ROT^2(a_2[k]) ⊕ ROT^5(a_2[k−1]) ⊕ ROT^7(a_2[k−2])
b_5[k] = ROT^1(a_1[k]) ⊕ ROT^4(a_2[k]) ⊕ ROT^6(a_1[k−1])
b_6[k] = SHIFT^3(a_1[k]) ⊕ SHIFT^5(a_2[k−1]) ⊕ ROT^2(a_2[k−2])
```

(For k = 1, 2 the negative-index taps vanish — e.g.
`b_3[1] = ROT^1(a_1[1])`, `b_3[2] = ROT^1(a_1[2]) ⊕ ROT^2(a_1[1])` —
matching the user's boundary spec. NOTE two transcription fixes made
here, TO BE CONFIRMED with the user: the original spec's `a_{3,k−2}` is
read as `a_1[k−2]` (no `a_3` exists at j = 2), and the bare `ROT` in
`b_32` as `ROT^2`. `b_4..b_6` were left to our choice; the above pick
exercises both columns, cross-column mixing, and the lossy SHIFT.)

Claims: `MLE[INT(b_i)](r_i) = c_i mod q`, shared column point (row points
free; the all-points-equal case connects to
`docs/rlc-shared-point-prompt.md`).

## The key reduction to investigate first

The current family requires each claim to be a per-position function of
the j committed bits AT THAT POSITION; the taps break that. But every
operand is `F_2`-linear and index-structured, so define the **derived
streams**: for each distinct (column, ROT/SHIFT amount, entry-offset)
tap, the stream `s = op(column)` — e.g. `s_1 = ROT^1 a_1`,
`s_2 = ROT^2·off^1 a_1`, `s_3 = ROT^3·off^2 a_1`, giving
`b_3[p] = s_1[p] ⊕ s_2[p] ⊕ s_3[p]` — a **per-position XOR of streams**.
With the streams as the family's base columns, the ENTIRE landed
construction applies verbatim (case-collapse, presum channels, elision,
cascade discharge) with `j_eff = #distinct streams` (here: count them —
a_1, a_2 plus the tap streams; dedupe shared taps like `ROT^1 a_1` in
b_3/b_5). Everything reduces to two questions:

1. **Stream extraction** (engineering): extend
   `extract_virtual_xor_rows`-style derivation with ROT (a permutation of
   the bit-position coordinate — in the layout, run reindexing), word
   SHIFT (masked run shift), and entry-offset (trace-row shift with
   zero boundary). One pass per stream; the committed code is
   `F_2`-linear so no new commitment.
2. **Stream openings** (the research crux): the presum/discharge exit at
   stream-MLE claims `ŝ(ρ)`, which must reduce to COMMITTED openings.
   - `ROT^c` alone permutes the bit-position variables: the embedded
     point permutes coordinates — the existing `embed_xor_point`/ring
     machinery generalises trivially. PROVE this first; it may already
     cover b-vectors built from pure in-place rotations.
   - Entry-offset and SHIFT are NOT variable permutations (shift ≠
     coordinate map on the hypercube). The promising route: the ring
     machinery is an `F_2`-linear-functional evaluator — an in-pack
     marginal `s_v[j] = Σ_y eq[y]·bit_j(P[y])` against an arbitrary basis
     `B(y)`. A stream that is an `F_2`-linear image of the committed
     vector has openings equal to committed-side functionals with a
     TRANSFORMED basis (eq composed with the tap map). What must hold for
     deployment: (a) the prover-side walk stays near-linear (it does —
     it's a reindexed gather), and (b) the verifier's succinct residual
     closure (`residual_b_evals` / `tensor_eq_phi_eval`) still evaluates
     the transformed basis in `O(m·128²)` — work out for offset/SHIFT
     whether the transformed `B̂` factors (shift-eq has a known
     2-term-per-variable recursion; derive it, or find the counterexample
     and record it as the boundary). If closed-form fails, fall back to:
     open the streams as their own family columns and discharge the
     stream↔column relation by one extra sumcheck level (the cascade
     pattern — level-3) — cost it honestly.

Also verify the CHANNEL-COUNT story before building: with j_eff streams
the case space is `2^{j_eff}`, but each claim's form touches ≤ 3 streams,
so `α^W` factors as a product of per-claim functions of ≤ 3 bits — the
multilinear support (the ACTIVE channels, Prop. `prop:vanish` machinery)
is exponentially sparser than `2^{j_eff}`. Count the active channels of
the concrete instance BEFORE writing kernels; if the union blows past the
table budget, cluster (`{b_1,b_3,b_5}` / `{b_2,b_4,b_6}`-style) — the
note's clustering problem, now with real structure to exploit.

## Staging

1. **Phase 0 — analysis on paper/in the note's terms**: streams and their
   dedupe for the instance; active-channel count; the ROT-only opening
   argument; the offset/SHIFT basis-transform derivation OR its
   counterexample. STOP AND RECORD (README + note) if a soundness or
   completeness gap appears — before code.
2. **Phase 1 — extraction + baseline**: extend the extraction ops
   (ROT/SHIFT/offset) and run the instance through the BATCHED-VX path
   (k = 6 extracted claims) — this is both the correctness oracle and the
   today-baseline the family must beat (~6–8 forest bodies + shared
   tail). Roundtrip + tamper tests at test shapes.
3. **Phase 2 — the stream family**: family construction over the streams
   (eager forest first), with the opening reduction of Phase 0; the
   cascade for |S| ≥ 2 channels of stream monomials (the AND-of-streams
   rows are extractable — the cascade generalises unchanged). Tests:
   cross-check vs the Phase-1 baseline on the same statement; pure-ROT
   sub-instance separately (the easy opening case).
4. **Phase 3 — measure** per the repo protocol at n = 22–28 vs the
   Phase-1 baseline and independent proofs (extend `examples/rlc_ab.rs`
   or add a mode; the CLI `--family` pattern is there to copy). Expected
   shape of the win: one forest (with a `2^{j_eff}`-case or clustered
   leaf) + discharge cascade vs ~6–8 forest bodies; report honestly if
   the stream count or the opening overhead eats it.

## Conventions and traps (repo-specific)

- 83/83 tests green before you start; nothing may change existing proof
  bytes; new APIs only; flock-core is a LOCAL PATH dep — do not touch it.
- Measurement: alternated in-window medians, phase trees, `vm_stat`
  before n ≥ 28, and the deterministic-grinding pitfall — average over
  STATEMENTS at small n.
- The forest kernels are indifferent to leaf provenance (tables + bit
  streams in, values out) — reuse `Pair2TauSet`/`T4Bits` shapes for
  j_eff = 2-ish clusters before inventing kernels; the 8/16-case
  leaf-round kernels remain unbuilt (that lever is shared with the plain
  j ≥ 3 family — if you build it here, wire it there too).
- `INT(a⊕b) = INT(a)+INT(b)−2·INT(a∧b)` is why per-claim tap monomials
  (ANDs of streams) appear at all; the carries are conserved — the
  cascade only relocates them. Do not attempt to lift integers into
  `GF(2^128)` (parity collapse — the founding obstruction).
