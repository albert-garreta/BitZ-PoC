# BitZ end-to-end parity — the port, its numbers, and how to continue (branch `bitz-e2e-parity`)

Date 2026-09-16. The end-to-end scheme of `worldfnd/BitZ` (their Spartan
PIOP over the circuit R1CS, the reduction to a claim on the virtual bits
`h = M(1 ‖ f)`, the fold + GKR on `h`, the transpose onto the committed `f`,
the PCS opening) re-implemented in `src/bitz/` over this crate's field and
engines, **producing the same bytes as their `tooling/cli` end-to-end
prover at every size the e2e runs** — 1, 8, 64 and 608 blocks, across
seeds — with their verifier accepting our proofs and ours accepting theirs.
The same holds for the **sampled-prime scheme** (the prime drawn from the
transcript per proof, their branch `bitz-k4-prime` composed end to end on
`bitz-e2e-k4-prime`): see "The sampled prime" below.
The feasibility analysis that preceded it is
`docs/bitz-piop-parity-feasibility.md`; the PCS half and its conventions are
`docs/bitz-parity-continue-prompt.md`.

## Where things are

- **This repo, branch `bitz-e2e-parity`** (off `bitz-parity` at `ddcb402`):
  `eca9218` stage A (the field and the Spartan PIOP), `411e58b` stage B (the
  virtual path and `Prepared`), then stage C/D (the sweep, the single-column
  commit without column packing, the Boolean inner-round kernels) and this
  document. Files:
  - `src/bitz/fq.rs` — their `Fq<2^100−15>` verbatim: schoolbook `mul_wide`,
    Barrett `reduce_wide`, 16-byte LE wire form, the rejection-sampled `u128`
    squeeze (`squeeze_fq` on both transcript halves). No inversions anywhere.
    The modulus is read at runtime (their `Fq<RUNTIME>`): `Q` until
    `set_modulus(prime)` installs a sampled one (`modulus()`, `ModulusError`,
    their 13-base Miller–Rabin `is_probable_prime` verbatim); process-wide.
  - `src/bitz/transcript.rs` — `squeeze_prime(bits)` on both halves (their
    rule: masked squeeze, top and low bits set, first probable prime).
  - `src/bitz/spartan/{poly,matrix,sumcheck,piop}.rs` — the little-endian eq
    table; the R1CS matrices lowered from the vendored circuit crate with the
    constraint digest streamed (`M` hashed, not kept), the 2^16-column chunk
    index, `bind_and_batch`, `evaluate_batched`, the products and the
    assignment; the outer (cubic, one dense eq table) and inner (quadratic)
    provers with fused fold-and-next passes and `c1` reconstruction, the
    Boolean round-0 kernels, the reusable verifier; the composition and the
    canonical out-of-band bytes `spartan.bin` (`to_bytes`/`from_bytes`).
    For the sampled prime: `PreparedIntegerMatrices` (the coefficients kept
    as integers — `IntegerCoefficient::{PowerOfTwo, Small, Big}` — with
    their integer digest, `lower()` under the installed modulus into the
    flat `SparseMatrix` in parallel), `prove/verify_spartan_piop_absorbed`
    (the digest already absorbed) and `prove/verify_spartan_piop_sampled`.
  - `src/bitz/map.rs` — the map digest (blake3 over the CSC arrays) and the
    `M^T` transpose (XOR gathers), through the `csc()` accessor added to the
    vendored `crates/circuit/src/matrix_transpose.rs`.
  - `src/bitz/virt.rs` — `VirtualParams` (64-byte frame), `VirtualStatement`,
    `transpose_query`, `VirtualTable` (h as per-column rows + column-lane
    packing at the claim shape), `BitZProver::prove_virtual`,
    `BitZVerifier::verify_virtual`.
  - `src/bitz/e2e.rs` — their `Prepared` (new/witness/commit/prove/verify,
    `bind`), `opening_claim`, `shape_for`, `generator`; `Proof { root,
    spartan, terminal, opening }`. Their `PreparedSampled` (`SESSION_SAMPLED`,
    `prime_bits` in 64..=100, `prime_for(root)`, `params()` under the
    installed modulus, `SampledWitness`, `SampledProof { root, prime, spartan,
    terminal, opening }`).
  - `src/bitz/statements.rs` — their d6b637e SHA-256 statements on the
    vendored trait, `Sha256Statement::seeded` (SplitMix64) and
    `from_public_bytes`.
  - `examples/bitz_spartan_parity.rs` (stage A harness),
    `examples/bitz_e2e_parity.rs` (the e2e harness, `--sweep`; dumps with
    `kind=e2e-sampled` go through `PreparedSampled`, `--sweep --sampled`),
    `examples/bitz_e2e_bench.rs` (the raw-metrics bench, `RESULT` line,
    `--sampled`),
    `examples/bitz_lin_probe.rs` (the single-column opening),
    `examples/bitz_circuit_probe.rs` (the digest guard).
- **f2z-benchmark, branch `bitz-e2e-k4`** (`~/f2z-benchmark`, local,
  unpushed; protocol pin `90655c4` = `0013ce4` + `344903c` + the reference
  split): our examples on top — `6486ce8` `e2e_probe` + `dump_lin`, `eb4ea94`
  `dump_spartan` + the shared `examples/common/sha256.rs`, `ec33847`
  `dump_e2e` + `verify_e2e`. Their code untouched; their tests green
  (bitz-cli 4, circuit 48, common 43, spartan 23, tests 26).
- **f2z-benchmark, branch `bitz-e2e-k4-prime`** (worktree
  `~/f2z-benchmark-prime`, local, unpushed): the merge `71f1b4f` of
  `bitz-e2e-k4` with `bitz-k4-prime` (`0049646`, the sampled prime with a
  minimal diff: `Fq<RUNTIME>`, `squeeze_prime`, `PreparedIntegerMatrices`,
  `prove/verify_spartan_piop_sampled`), then `45f84a6`: `PreparedSampled`
  in `tooling/cli/src/end_to_end.rs` (the composition: bind, squeeze the
  prime, install, absorb `BitZParams<RUNTIME>`, lower, Spartan, opening),
  its round-trip/tamper test, `dump_e2e --sampled` (`kind=e2e-sampled`,
  `prime_bits`, `prime`, the integer digest as `constraint_digest`) and
  `verify_e2e --sampled`. bitz-cli tests: 2 + 3 green.

## How to run (the loop that guards every change)

```sh
# their side, once per checkout, native flags, their own target dir
cd ~/f2z-benchmark && git checkout bitz-e2e-k4
CARGO_TARGET_DIR=$HOME/f2z-benchmark/target RUSTFLAGS="-C target-cpu=native" \
  cargo build --release -p tests -p bitz-cli --examples
E=$HOME/f2z-benchmark/target/release/examples
# our side (CARGO_TARGET_DIR is globally ~/zinc-plus/target)
cd ~/f2z-pcs && git checkout bitz-e2e-parity
RUSTFLAGS="-C target-cpu=native" cargo build --release --features bitz-parity \
  --example bitz_e2e_parity --example bitz_e2e_bench --example bitz_spartan_parity
RUSTFLAGS="-C target-cpu=native" cargo test --release --features bitz-parity,parallel --lib bitz   # 49 tests
O=$CARGO_TARGET_DIR/release/examples

# one instance: their dump, our re-prove + byte diff + both verifiers, their verifier on ours
$E/dump_e2e sha256-chain 8 7 $SCRATCH/e2e_ch8
BITZ_REPEAT=5 BITZ_TRACE=1 $O/bitz_e2e_parity $SCRATCH/e2e_ch8
$E/verify_e2e $SCRATCH/e2e_ch8 ours.
# the sweep: every block count x seeds (a count draws from a printed base;
# BITZ_SWEEP_BASE=<base> reproduces; --keep keeps passing dumps)
$O/bitz_e2e_parity --sweep $E $SCRATCH/sweep sha256-chain 8,64,608 3
# the PIOP alone (stage A): the abc fixture or the e2e prefix
$E/dump_spartan abc $SCRATCH/sp_abc; $O/bitz_spartan_parity $SCRATCH/sp_abc
# the paper-style metrics for one size (medians of reps, phases, RSS)
$O/bitz_e2e_bench sha256-chain 608 --reps 5
# their canonical timings for the same statement
/usr/bin/time -l $E/e2e_probe sha256-chain 608 7 --bench-only

# the sampled prime: their prime worktree (its own target dir), --sampled everywhere
cd ~/f2z-benchmark-prime && git checkout bitz-e2e-k4-prime
CARGO_TARGET_DIR=$HOME/f2z-benchmark-prime/target RUSTFLAGS="-C target-cpu=native" \
  cargo build --release -p bitz-cli --examples
P=$HOME/f2z-benchmark-prime/target/release/examples
$P/dump_e2e sha256-chain 8 7 $SCRATCH/e2ep_ch8 --sampled   # prints the prime
BITZ_REPEAT=3 $O/bitz_e2e_parity $SCRATCH/e2ep_ch8          # kind=e2e-sampled → PreparedSampled
$P/verify_e2e $SCRATCH/e2ep_ch8 ours. --sampled
$O/bitz_e2e_parity --sweep $P $SCRATCH/sweep_p sha256-chain 1,8,64,608 3 --sampled
$O/bitz_e2e_bench sha256-chain 608 --sampled --reps 3
```

Acceptance for every change: the sweep passes (root, `spartan.bin`, the
claim on `h`, narg, hints IDENTICAL; ours→theirs, ours→ours and theirs→ours
accept) at 1, 8, 64 and 608 blocks, fixed and `--sampled`; the unit tests
pass. (`sha256-compression` is one block only; sweep it at `1`.)

## What is pinned (2026-09-16)

- Stage A, the PIOP alone (`dump_spartan` / `bitz_spartan_parity`): the
  abc compression (their `sha256_piop` fixture) and the e2e transcript prefix
  at 1 and 8 blocks — `spartan.bin` identical, every challenge identical (32,
  36 and 45 compared), the bound table `D` identical, the claim on `h`
  identical, the post-PIOP sponge state identical, both verifiers accept.
- Stage B/C, the whole scheme (`dump_e2e` / `bitz_e2e_parity --sweep`):
  1 block × 3 seeds, 8 × 3, 64 × 3, 608 × 2 (seed bases 20260916/17/18):
  root, `spartan.bin`, `claim_h`, narg and hints IDENTICAL, all three
  verifier directions accept — **14/14**.
- The single-column opening alone (`dump_lin` / `bitz_lin_probe`, 2^22 bits,
  `Shape::new(22, 0)`): identical, and the root under the single-row commit
  equals theirs under the (14, 8) commit.
- The sampled prime (`dump_e2e --sampled` / `bitz_e2e_parity`): seed 7 at 1
  block (prime 885573671305466740332365494013), 8 (1031328402746909102491161449767)
  and 608 (1258275306762215810449910764789), then the sweep — chain 1 × 3,
  8 × 3, 64 × 3, 608 × 3 (seed base 1789555292157467000) and compression
  1 × 2 (base 1789555387581027000): the integer digest, the map digest, the
  shapes, the root, the derived prime, `spartan.bin`, `claim_h`, narg and
  hints IDENTICAL, all three verifier directions accept — **17/17**. Fixed
  path re-checked identical alongside.

## The numbers (M5, 10 threads, seed 7; theirs = `benchmark::run` clean run, ours = `bitz_e2e_bench` medians)

| blocks | shapes h / f | their setup | their prove | their verify | our setup | our prove | our verify | narg | hints | Spartan out of band (theirs / ours with terminal) | peak RSS theirs / ours |
|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | (14,8) / (14,8) | 14 ms | **131** | **70** | 13 ms | **95** | **64** | 15,396 B | 102,844 B | 1,408 / 1,680 B | 0.49 / 0.37 GB |
| 8 | (14,8) / (14,8) | 94 | **143** | **75** | 100 | **104** | **71** | 15,396 | 103,516 | 1,744 / 2,064 | 0.58 / 0.46 |
| 64 | (14,8) / (14,8) | 734 | **183** | **103** | 743 | **112** | **71** | 15,396 | 103,516 | 2,080 / 2,448 | 1.27 / 1.20 |
| 608 | (15,9) / (14,8) | 7,052 | **591** | **276** | 6,828–6,990 | **210** | **108–113** | 20,708 | 103,932 | 2,416 / 2,832 | 9.29 / 9.05 |

(Setup is the vendored BigInt/BTreeMap constraint generator on both sides
— `ConstraintGenerator` and `MTransposeGenerator` — and it is also what the
9 GB at 608 blocks is; both digests are over the final arrays, so a compact
generator that yields the same arrays is byte-safe. The witness step is
27–37 ms at 608 on both sides. Commit: theirs 0.5 ms, ours 0.6.)

Our 608-block prover, warm (`BITZ_TRACE=1`), 210 ms:

| phase | ms | notes |
|---|---|---|
| Spartan | 83 | outer 8 (2^19 rows), `bind_and_batch` 29 (33.7 M nonzeros), inner 46 (2^24; round 0 multiply-free on the Boolean witness) |
| fold + images | 0.9 | 2^9 folds over 2^15 rows |
| GKR | 37 | the parity forest at (15, 9) |
| transpose | 12–15 | the row⊗column expansion (parallel) + 26.1 M XOR gathers; 64 MB first touch |
| opening | 73–80 | claim encode 3–6, **claim absorb 52–54** (64 MB through SHAKE128 ≈ 1.2 GB/s), opening sumcheck 10 (combine 2, row rounds 7), ring switch 0.5, Ligerito 5 |

Our 608-block verifier, 108–113 ms: Spartan verify 28 (`evaluate_batched`
over every nonzero), fold 0.3, GKR 0.8, transpose 12–15, opening 61–69 (the
same 52–54 ms absorb + the sumcheck replay + Ligerito). Theirs: 276 ms,
linear in the circuit by design; ours is the same asymptotics with smaller
constants.

## The sampled prime (M5, 10 threads, seed 7; theirs = `dump_e2e --sampled` on `bitz-e2e-k4-prime`, ours = `bitz_e2e_bench --sampled` medians of warm repeats, cold = the first proof after the witness)

The 100-bit prime is drawn after the root is bound, so it differs per
statement (608 blocks, seed 7: 1258275306762215810449910764789). What the
sampled scheme adds per proof is the lowering of the integer matrices under
that prime (and, on the prover, the products and the assignment), which the
fixed scheme does once at setup; everything after it is the same
computation.

| blocks | their prove sampled / fixed | their verify sampled / fixed | our prove sampled (cold) / fixed (cold) | our verify sampled / fixed | narg | hints | Spartan out of band |
|---|---|---|---|---|---|---|---|
| 1 | 157 / 131 | 70 / 70 | 98 / 95 | 65 / 64 | 15,396 B | 103,100 B | 1,680 B |
| 8 | 174 / 143 | 77 / 75 | 104 (123) / 99 (118) | 67 / 66 | 15,396 | 102,748 | 2,064 |
| 608 | **1,102** / 544 | **598** / 271 | **308** (332) / 232 (263) | **178** / 118 | 20,708 | 102,236 | 2,832 |

(Same-run pairs; the fixed path's 608-block prove measured 213–232 ms across
runs today, the Spartan passes swinging ±10 % with the box. Hints differ
from the fixed path's by the PoW nonces the different transcript draws.)

Our 608-block sampled prover, warm, 308 ms: lowering 70 (lower 52 —
33.7 M residues by table lookup into the kept buffer, 800 MB written;
products 13; assignment 4, parallel), Spartan 106 (outer 10, bind+batch 38,
inner 58 — the same as the fixed path's in the same run), GKR 37, transpose
13, opening 80. Verifier 178 = the fixed path's 118 + the 52 ms lowering
(+ a little for `evaluate_batched` over the fresh residues). Sampling the
prime is ≈ 1 ms (34 candidates through their 13-base Miller–Rabin).

Their sampled path is 2× their fixed one on both sides (their
`lower::<RUNTIME>()` per proof plus the runtime modulus in every operation;
their code is theirs to tune). Levers left on ours: flatten the integer
matrices too (the lowering reads scattered per-row vectors), and the
fixed-path levers.

## What stage D found (all byte-identical, all kept unless noted)

- The single-column commit of `f` skips the column-lane packing (`Pcs::commit`
  at `log_columns() == 0` calls `commit_rs_flock_from_rows` with no packed
  columns; the opening sumcheck combines from the rows): commit 13.7 → 0.6
  ms, `sc combine cols` 9 → 2 ms.
- The claim encodings are pre-sized (`Vec::with_capacity`): the 64 MB
  encode 6 → 3–6 ms (it was growing by doubling).
- The inner sumcheck's witness is `h`, 0/1 by construction: the initial
  `[c0, c2]` and the round-0 fold select instead of multiply
  (`inner_coefficients_boolean`, `fold_inner_and_next_boolean`; the general
  kernels are kept and a test pins equal bytes): inner 59 → 46 ms.
- The field's reductions are branchless (`conditional_subtract`, a masked
  subtraction after Barrett's estimate and in `add`/`sub`/`neg`): inner
  sumcheck 54 → 47 ms at 608 blocks on the fixed path, a little more under
  a generic prime where the estimate is short more often.
- Tried and dropped: `keccak`'s `asm` feature (the ARMv8 SHA3 permutation
  behind spongefish's SHAKE128) — no change to the 52–54 ms absorb (the
  portable permutation already runs at ≈ 130 ns; the absorb is a per-side
  floor of the protocol, a digest-of-the-claim would be their change).

Levers left, in order of expected gain at 608 blocks: `bind_and_batch` and
`evaluate_batched` (29 + 28 ms; a lazy 256-bit accumulation per column with
one reduction, or power-of-two coefficient shifts — the SHA coefficients are
`±2^i`); the inner sumcheck's rounds 1–2 (the folded witness stays in a
4-, then 16-value set); the GKR at (15, 9) (37 ms, the PCS work's kernels);
the transpose's 64 MB first touch (a reused buffer across repeats).

## Traps (new ones; the PCS and feasibility traps still apply)

- **Session and instance labels are byte strings on their side**
  (`b"bitz/circuit-e2e/v1"`, the domain, the Spartan test's labels);
  spongefish length-prefixes a `str`, so pass `&[u8]` — a `&str` desyncs the
  very first squeeze (`tau[0]`), and because the outer rounds of a satisfied
  SHA instance are all zeros (`Az = Bz = Cz = 0` on the 184 `sum_32` rows per
  block, `Bz = Cz` on the `constrain_bit` rows), the first visible difference
  lands rounds later. The PCS dumps use `str` labels on both sides, which is
  why this never showed there.
- `crates/pcs/src/statement.rs` (`Pcs::bitz_statement`) is dead code on their
  side; the 40-byte `Encoding for Pcs` is what is absorbed.
- Our `Proof` carries the terminal claim; their `Proof` does not (their
  verifier recomputes it). `spartan.bin` includes it on both sides (their
  dumps append it); their `verify_e2e` ignores the tail.
- `f` is committed as ONE row of `2^m` bits (`Prepared::commit_shape`), not
  under the (14, 8) split — the root is the same, and the opening's
  single-column claim needs the single-row layout.
- `VirtualTable::new` masks the last word of `h` beyond `bit_len`; the
  vendored `PackedWitness` already zeroes it, but a hand-built word vector
  might not.
- The 608-block `dump_e2e` takes ≈ 18 s and 9 GB (two `Prepared` setups: by
  `Prepared` and by hand, asserted equal); one size per process.
- **The modulus is process-wide** (ours and theirs): one sampled-prime proof
  or verification at a time; `Fq` values made under one prime mean nothing
  under another; decode a sampled proof's residues (`spartan.bin`, the
  terminal claim) only after `set_modulus(prime_for(root))` — the harness
  does, and resets `Q` afterwards. `PreparedSampled::params()` exists only
  after the install (the 48-byte `BitZParams` frame carries the prime, so it
  is absorbed after the squeeze). A `meta.txt` with `kind=e2e-sampled` has
  `q = prime` and `constraint_digest` = the integer digest (domain
  `bitz/spartan/integer-constraint-matrices/v1`), not the residue digest.
- **Fresh allocations fault under the kernel's map lock and every parallel
  pass after them runs slower.** The first sampled-prime `prove` lowered
  33.7 M residues into fresh per-row vectors, built products and assignment
  fresh, and its Spartan passes ran at half the fixed path's speed
  (`bind_and_batch` 30 → 58 ms at 608 blocks, 10 threads) — not the values
  (the prime forced to `2^100 − 15` timed the same), not the layout (flat
  made no difference), and at one thread the gap was 20 %. `SparseMatrix` is
  now flat (one entry buffer + row starts), the row starts and the column
  chunk index are built once from the integer matrices (prime-independent,
  shared through an `Arc`), and `PreparedSampled` keeps the lowered, product
  and assignment buffers between proofs (`LoweredScratch`, `lower_into`,
  `products_into`, `assignment_into`): the Spartan phases then time the same
  as the fixed path's, and the sampled path pays only the lowering.
