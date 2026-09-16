# BitZ end-to-end parity — feasibility analysis (Spartan PIOP + virtual reduction on the parity PCS)

Date 2026-09-16. Box: Apple M5, 24 GB, 10 rayon threads (`threads=10` in every
run below). Our side: `~/f2z-pcs` branch `bitz-parity` at `f00c454` (code tip
`afbfd73` + docs). Oracle: `~/f2z-benchmark` branch `bitz-e2e-k4` at `90655c4`
(see §1). Companion prompts: `docs/bitz-piop-parity-prompt.md` (the task),
`docs/bitz-parity-continue-prompt.md` (the PCS half and its conventions).

> **Outcome (2026-09-16, later the same day):** the port is done on branch
> `bitz-e2e-parity` — byte-identical at 1, 8, 64 and 608 blocks across
> seeds, both verifiers both ways; see `docs/bitz-e2e-parity.md`.

**Verdict: feasible, and cheaper than the PCS port was.** Every question the
prompt raised has an answer with evidence, and the three that could have
blocked are resolved in our favour:

- the vendored `crates/circuit` reproduces their `M, A, B, C` and `M^T`
  **bit for bit** (constraint digest and map digest identical at 1, 8, 64 and
  608 blocks; §4);
- the one new PCS shape — the single-column `LinearClaim<F128>` over
  `Shape::new(22, 0)` that `transpose_query` hands the opening — is already
  **byte-identical** through our `bitz::Pcs` (narg and hints identical, root
  identical, both verifiers accept; §5);
- the byte surface is fully accounted for: the narg string of an e2e proof is
  the folds + GKR + PCS sumcheck + ring-switch claims + Ligerito narg, to the
  byte (§2), and nothing in the Spartan PIOP touches the narg at all (its
  round polynomials are public messages), so the parity set must include the
  Spartan proof bytes explicitly.

What is new to write: an `Fq<2^100−15>` type with their encodings and
challenge rule, the Spartan prover/verifier (two sumchecks, a sparse
bind-and-batch, a digest), and the virtual framing around our existing
fold/GKR/PCS (`VirtualParams`, `VirtualStatement`, `opening_claim`,
`transpose_query`, `prove_virtual`/`verify_virtual`). Estimated ≈ 8–10 working
days to byte identity at all four sizes (§9), most of it stage A.

---

## 1. The oracle branch

`~/f2z-benchmark`, branch `bitz-e2e-k4`, checked 2026-09-16 after `git fetch
origin` — **upstream has not moved**: `origin/feat/circuit-e2e` = `0013ce4`,
`origin/bitz-k4` = `344903c`, `origin/main` = `0c75fd8`, exactly as when the
branch was merged.

```
$ git log --oneline -3
90655c4 cli: derive the end-to-end shapes from the reference split
c0cac39 Merge remote-tracking branch 'origin/bitz-k4' into bitz-e2e-k4
0013ce4 refactor: separate circuit adapters from proving tools
```

The pinned commit for everything below is **`90655c4`** (= `0013ce4` +
`344903c` merged + our `shape_for` commit); the branch tip is now `6486ce8`,
this session's probe examples on top of it (A.4), no protocol code touched.
Tests: see §1.3.

### 1.1 What was added on the branch this session (their code untouched)

- `tooling/cli/examples/e2e_probe.rs` — the SHA-256 adapter of `d6b637e`
  (`tooling/cli/src/sha256.rs`) restored **verbatim** as a module (only
  `crate::end_to_end` → `bitz_cli::end_to_end`), seeded blocks (SplitMix64
  over the seed, 8 bytes per output, little-endian, 64 bytes per block; the
  expected chaining value by `sha2::compress256` as the old CLI did), then
  everything `Prepared::new` derives step by step through public APIs
  (`ConstraintGenerator` → `PreparedConstraintMatrices` + digest,
  `MTransposeGenerator` → map + digest, `shape_for` replicated,
  `Pcs::new(.., Fast, Blake3)` printed with `{:?}`), the transpose and the
  Spartan PIOP timed alone, the two costs around the transpose (§6), then
  the whole e2e through `bitz_cli::benchmark::run` and once more by hand for
  the proof anatomy. `--bench-only` runs nothing but `benchmark::run` (for the
  process peak); `--skip-e2e` stops after the Spartan-alone timing.
  Usage: `e2e_probe <sha256-compression|sha256-chain> <blocks> <seed>`.
- `crates/tests/examples/dump_lin.rs` — the transposed query shape alone: 2^22
  bits committed under `(14, 8)`, a `LinearClaim<F128>` over `Shape::new(22,
  0)` with random row weights and column weight one, proved with
  `pcs.prove_lin(.., StatementBinding::Bind, ..)` under the dump labels
  (`bitz-tests` / `fold-round-trip`), verified, dumped (`witness.bin`,
  `claim.bin` = u64 count ‖ rows ‖ u64 count ‖ cols ‖ target, F128 as 16 LE
  bytes; `narg.bin`, `hints.bin`, `meta.txt`). Usage: `dump_lin <seed> <dir>`.
- `tooling/cli/Cargo.toml`: `[dev-dependencies] sha2 = { version = "0.10",
  features = ["compress"] }`, `rayon.workspace = true` (examples only).

Note: `crates/pcs/src/statement.rs` (`Pcs::bitz_statement`, the
"bitz/statement/v1" frame) is **dead code** — `mod statement` is not declared
in `crates/pcs/src/lib.rs`. What is absorbed for the PCS is the 40-byte
`Encoding for Pcs` (§2.4), not that frame.

### 1.2 What `Prepared::new` derives per size (seed 7, chain = standard IV, no padding)

| blocks | A/B/C rows (→ 2^·) | h_len = M rows (→ 2^·) | f_len − 1 = committed bits | M nnz | A nnz | B nnz | C nnz | claim shape (h) | committed shape (f) |
|---|---|---|---|---|---|---|---|---|---|
| 1 (compression) | 952 → 2^10 | 21,225 → 2^15 | 6,888 | 42,556 | 768 | 768 | 54,527 | (14, 8) | (14, 8) |
| 8 (chain) | 5,824 → 2^13 | 168,001 → 2^18 | 55,104 | 342,660 | 4,352 | 4,352 | 435,151 | (14, 8) | (14, 8) |
| 64 | 44,800 → 2^16 | 1,342,209 → 2^21 | 440,832 | 2,743,492 | 33,024 | 33,024 | 3,480,280 | (14, 8) | (14, 8) |
| 608 | 423,424 → 2^19 | 12,748,801 → 2^24 | 4,187,904 | 26,065,860 | 311,552 | 311,552 | 33,060,541 | **(15, 9)** | (14, 8) |

Arithmetic (all verified by the probe): rows = 184·blocks (the compressions)
+ 512·blocks (one `assert_r1c` per public input bit) + 256 (the digest bits)
= 696·blocks + 256, so the row domain is 2^19 at 608 blocks, not the
2^8·blocks the prompt guessed; h = 1 + blocks·(20,456 + 512) + 256; f =
blocks·6,888 (512 inputs + `COMPRESSION_HINT_BITS` = 6,376 per block), so
608 blocks give 4,187,904 ≤ 2^22 committed bits — the last count that fits
the 2^22 floor, which is why 608 is the e2e size. `shape_for` on the branch
(`end_to_end.rs:217-224`) = `Shape::for_log_bits(max(⌈log2⌉, 22))`
(`common/src/shape.rs:60-69`, `t = ⌈3n/5⌉`), so the claim shape over h is
(14, 8) up to 64 blocks and **(15, 9) at 608**, and the committed shape is
(14, 8) at every size — both inside the sweep our parity PCS is pinned on.

Digests (recomputed identically on our side, §4): constraint digest
`6673025d…` (1), `e0d4ba58…` (8), `1758f9e1…` (64), `6416b50d…` (608); map
digest `29d30fed…` (1), `4d70acbb…` (8), `dfab9b34…` (64), `5996a397…` (608)
(full hex in the appendix).

**The Ligerito ladder** `Pcs::new(&committed_shape, LigeritoProfile::Fast,
HashKind::Blake3)` resolves to, printed from the `Pcs` Debug view at m = 22:
`log_inv_rates [1, 2, 3], recursive_steps 2, initial_log_msg_cols 11,
initial_log_num_interleaved 4, initial_k 4, recursive_log_msg_cols [8, 5],
recursive_ks [3, 3], queries [183, 90, 60], grinding_bits [16, 16, 16],
fold_grinding_bits [9, 6, 3], ood_samples [0, 1, 1]` — the k = 4 ladder
(`crates/pcs/src/ligerito.rs:39-66`: `Fast` → `include_str!` of
`configs/ligerito-k4/m{m}_fast.toml`), i.e. what `src/bitz` embeds; the
byte-identical PCS runs at n = 22 and 24 in §5 are the runtime proof.

### 1.3 The e2e end to end (clean sequential runs, seed 7, `benchmark::run`)

| blocks | setup | witness | commit | prove | verify | narg | hints | Spartan (out of band) | peak RSS (`--bench-only`) |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 14.4 ms | 0.2 | 0.68 | **131.4** | **70.0** | 15,396 B | 102,844 B | 1,408 B | 0.49 GB |
| 8 | 94.3 | 1.3 | 0.58 | **143.1** | **75.2** | 15,396 | 103,516 | 1,744 | 0.58 GB |
| 64 | 734 | 4.0 | 0.54 | **183.1** | **102.5** | 15,396 | 103,516 | 2,080 | 1.27 GB |
| 608 | 7,052 | 36.9 | 0.53 | **590.5** | **275.8** | 20,708 | 103,932 | 2,416 | 9.29 GB |

(`setup` = `Prepared::new`: two synthesis passes through the BigInt/BTreeMap
`ConstraintGenerator` and `MTransposeGenerator`; it is what the 9.3 GB at
608 blocks is, not the prover. Repeat runs agree within a few %:
608-block prove 590–620 ms, verify 276–284 ms across four runs.)

Measured components of their prover and verifier at 608 blocks (each timed
alone by the probe): Spartan PIOP prove **122–133 ms**, verify **27 ms**; the
row⊗column expansion `transpose_query` does before applying `M^T` (a serial
`flat_map` over 2^24 products) **31.6 ms**; `map.transpose` itself **5.2–5.5
ms** (26.1 M XOR gathers, parallel); `poly::eq_table` of 24 coordinates 28.8
ms (first touch of 256 MB); the sponge absorbing the single-column claim (67
MB) **55–76 ms**. Their PCS alone at the two claim shapes (their `dump_bitz`,
seed 7): (14, 8) prove 95.2 ms / verify 1.6 ms, (15, 9) prove 273.5 ms /
verify 2.4 ms; the single-column opening alone (`dump_lin`): prove 95.3 ms,
verify 60.3 ms. So at 608 blocks their 590 ms prover is roughly Spartan 125 +
fold/GKR on h at (15, 9) ≈ 240 + expansion 32 + transpose 5 + single-column
opening 95 (55–76 of which is the absorb) + eq tables and first touches; and
their 276 ms verifier is Spartan verify 27 + expansion 32 + transpose 5 +
absorb ≈ 60 + the serial 2^22-weight folds of their `pcs::sumcheck::verify`
+ GKR/fold verification + first touches. **Their e2e verifier is linear in
the circuit** (2^24-weight expansion, `M^T` over every nonzero, a 67 MB
absorb) by design; parity means we pay the same asymptotics, though our
constants are smaller (§5–6).

Tests on the branch: green today (bitz-cli 4, circuit 48, common 43, spartan
23, tests 26; A.5).

---

## 2. The byte surface

Transcript: spongefish 0.7.4, `StdHash = Shake128` (Keccak-f[1600] through
the `sha3` crate; `spongefish/src/lib.rs:238`), protocol id `"bitz/v1"`
(`transcript/src/domain.rs:6`), session `b"bitz/circuit-e2e/v1"`, instance
`statement.domain()` (`b"sha256-compression/v1"` | `b"sha256-chain/v1"`) —
domain tags, never in the narg string. Every prover message is absorbed
through its canonical `Encoding` and written to the narg string; every
`public_message` is absorbed and not written; hints bypass the sponge.

### 2.1 Event list of `Prepared::prove` (`end_to_end.rs:154-185`), in order

**(a) statement binding** (`bind`, `end_to_end.rs:200-208`) — public:
`u64 public.len()` (8 B LE) · the public bytes (chain: 8 + 4·(8 + 16·blocks + 8)
B) · root (32 B) · `BitZParams` (48 B) · `Pcs` (40 B) · map digest (32 B).

**(b) `prove_spartan_piop`** (`spartan/src/piop.rs:66-114`) — public and
squeezes only, **nothing reaches the narg string**:
constraint digest (32 B) · squeeze `tau` (num_row_vars × `Fq`) · outer
sumcheck (`sumcheck.rs:254-389`): per round public `[c0,c1,c2,c3]` (64 B, the
array `Encoding` is plain concatenation, `spongefish/src/codecs.rs:101-109`)
then squeeze one `Fq` · public `[Ah(r_x), Bh(r_x), Ch(r_x)]` (48 B) · squeeze
`rho` · inner sumcheck (`sumcheck.rs:446-559`): per round public `[c0,c1,c2]`
(48 B) then squeeze one `Fq`. Output: `ScaledMleEvaluationClaim { point =
r_y (num_column_vars Fq), scale = D(r_y), value = final claim }` — carried out
of band; the verifier recomputes `scale` through `evaluate_batched`
(`matrix.rs:265-280`).

**(c) `opening_claim`** (`end_to_end.rs:236-255`): the point zero-extended to
the claim shape's `log_bits`, `rows = scale · eq_table(point[..t])` (2^t Fq),
`columns = eq_table(point[t..])` (2^s Fq), target = `value`; no transcript
event.

**(d) `prove_virtual`** (`prover/src/prove.rs:91-143`) — public:
`b"bitz/virtual-statement/v1"` (25 raw bytes) · root (32 B) · `VirtualParams`
(64 B) · map digest (32 B) · `LinearClaim<Fq>` on h (8 + 16·2^t + 8 + 16·2^s
+ 16 B: 266,272 B at (14, 8), 532,504 B at (15, 9)).

**(e) fold** (`prover/src/fold.rs:34-63`) — **narg: 2^s records of 16 B**
(each `fold.to_le_bytes()` as a `[u8; 16]` prover message: 4,096 B at s = 8,
8,192 B at s = 9); then s squeezes of `F128` (zeta).

**(f) GKR** (`gkr/src/lib.rs:15-125`) — t layers; layer i (i = 0..t−1, point
length s + i): (s + i) rounds each **narg `[F128; 2]` (32 B)** + one squeeze,
then **narg `[F128; 2]` (32 B)** (the two children) + one squeeze. Total
narg = 32·Σ_{i<t}(s+i+1) = 32·(t(s+1) + t(t−1)/2): **6,944 B at (14, 8),
8,160 B at (15, 9)**.

**(g) `transpose_query`** (`common/src/virtual_map.rs:158-194`): the GKR
query is `InnerProduct { rows u1 (2^t), columns u2 (2^s), target }`;
weights = `column ⊗ row` (index `column·2^t + row`, 2^{t+s} F128), `M^T`
applied (`matrix_transpose.rs:373-385`), the constant-column weight
subtracted from the target, the 2^22 committed weights zero-padded, a
single-column `LinearClaim<F128>` over `Shape::new(22, 0)` with column weight
one. No transcript event.

**(h) `pcs.prove_lin(.., InnerProduct, Bind, ..)`** (`pcs/src/opening.rs:85-122`) —
public `b"bitz/pcs/bit-inner-product/v2"` · root · `Pcs` (40 B) · **the
single-column claim: 8 + 16·2^22 + 8 + 16 + 16 = 67,108,904 B** · public
`b"bitz/pcs/inner-product-sumcheck/v1"` · sumcheck (`pcs/src/sumcheck.rs:33-81`):
22 rounds of **narg `[F128; 3]` (48 B)** + squeeze, then **narg the witness
evaluation (16 B)** = 1,072 B · public `b"bitz/pcs/mle-opening/v1"`, root,
`Pcs`, `u64 22`, the 22 coordinates, the target · public
`b"bitz/pcs/mle-claims/v1"` + **narg `[F128; 128]` (2,048 B)** · public
`b"bitz/pcs/ring-switch-challenges/v1"` + 7 squeezes · Ligerito through the
challenger: **narg 1,236 B at m = 22** (per-level sumcheck messages, OOD
samples, PoW nonces as u64 LE, the final message) and **hints ≈ 102.8–103.9
KB** (the serialised Ligerito proof, `hint_bytes`, the only hint).

The e2e `Proof` is `{ root, spartan: SpartanPiopProof (out of band), opening:
transcript::Proof { narg, hints } }`; `Prepared::verify` replays (a), then
`verify_spartan_proof` (absorbing the out-of-band round polynomials as public
messages, checking each `g(0) + g(1)`, the terminal `eq(tau, r_x)·(Ah·Bh −
Ch)`, and recomputing `scale`), (c), `verify_virtual` (`verifier/src/verify.rs:38-66`),
and `check_eof` on both streams.

**Byte accounting, exact**: 1 block (14, 8) narg = 4,096 + 6,944 + 1,072 +
2,048 + 1,236 = **15,396 ✓**; 608 blocks (15, 9)/(22, 0) = 8,192 + 8,160 +
1,072 + 2,048 + 1,236 = **20,708 ✓**; `dump_lin` alone = 1,072 + 2,048 +
1,236 = **4,356 ✓**; their `dump_bitz 24` = 8,192 + 8,160 + (24·48 + 16 =
1,168) + 2,048 + 1,216 (the m = 24 ladder) = 20,784 ✓. Spartan out-of-band
bytes = 16·(4·outer + 3 + 3·inner): 2,416 at 608 ✓.

### 2.2 The parity comparison set

1. `root` (32 B) — pins the commit (packing + flock).
2. `narg` — pins the fold, GKR, opening sumcheck, ring switch and Ligerito
   narg, hence every challenge after Spartan; but **NOT** Spartan itself: a
   wrong Spartan prover changes the challenges and would show up only
   indirectly (everything after it shifts) — never diagnose Spartan from
   narg.
3. `hints` — the Ligerito proof.
4. **`spartan.bin`** (proposed canonical encoding, none exists on their
   side): outer round polynomials in round order, each `[c0,c1,c2,c3]` as
   4 × 16 LE bytes; then `[Ah, Bh, Ch]` (48 B); then inner rounds `[c0,c1,c2]`
   (3 × 16 B); then the terminal claim `point (num_column_vars × 16 B) ‖
   scale ‖ value`. All `Fq` canonical residues (`lift().to_le_bytes()`).
5. The recomputed digests: constraint digest (SHA-256) and map digest
   (blake3), compared as hex, not copied.
6. Cheap extras worth dumping because they pin intermediate stages
   (`claim_h.bin` = the `LinearClaim<Fq>` on h in `claim.bin`'s format; at
   small sizes `products.bin` = Az‖Bz‖Cz residues, 3 × 2^rows × 16 B).

### 2.3 The field encodings (`field/src/codec.rs`)

`Fq`: `lift().to_le_bytes()`, 16 bytes of the canonical residue (`:42-46`);
`NargDeserialize` rejects any value ≥ Q (`:48-60`) — irrelevant to the e2e
since no `Fq` is ever a prover message. `F128`: `to_bytes()` = `lo` LE ‖ `hi`
LE (`:22-26`). Integers: `to_le_bytes` (`spongefish codecs.rs:111-142`);
`[u8]` and `[u8; N]`: raw bytes (`:181-185`, `:101-109`); `[T; N]`:
concatenation, no length prefix. Squeezes: `verifier_message::<u128>()` =
16 squeezed bytes read LE; `F128` = `from_bytes` of 16 squeezed bytes.

### 2.4 Every absorbed struct

| what | where | bytes |
|---|---|---|
| `BitZParams<Q>` | `common/src/params.rs:87-102` | 48: `log_rows` u64 ‖ `log_columns` u64 ‖ `Q` u128 ‖ generator 16 B |
| `VirtualParams<Q>` | `params.rs:172-180` | 64: `BitZParams` ‖ committed `log_rows` u64 ‖ committed `log_columns` u64 |
| `Pcs` | `pcs/src/commitment.rs:136-161` | 40: `m` ‖ `log_inv_rate` (1) ‖ `log_batch_size` (4) ‖ profile tag (Fast = 0) ‖ hash tag (Blake3 = 1), u64 LE each |
| `LinearClaim<F>` | `common/src/claim.rs:43-56` | u64 rows ‖ rows ‖ u64 cols ‖ cols ‖ target |
| round polynomial `[Fq; k]` | array `Encoding` | 16·k |
| digests, root | `[u8; 32]` | 32 raw |
| labels | `&[u8; N]` / `&[u8]` | raw, no length |

Our `src/bitz/params.rs` already carries the 48-byte `BitZParams` frame and
both `LinearClaim` encodings (`:140-151`, `:214-226`, `:271-283`); the
64-byte `VirtualParams` frame and the `Fq` round-polynomial absorptions are
the only new encodings.

---

## 3. The field `Fq<Q100>`

Their type (`field/src/fq.rs`): `Fq<const Q: u128>(u128)` held reduced;
`Q100 = 2^100 − 15` (`:19`); `From<u128>` reduces by `%` (`:284-289`);
`from_limbs(lo, hi)` (`:199-201`); `add`/`sub` by conditional subtraction
(`:318-338`); `mul` = `mul_wide` (schoolbook 128×128→256, `:37-48`) then
`reduce_wide` = Barrett with `MU = ⌊2^200/Q⌋` and two conditional
subtractions (`:203-229`); `neg` (`:310-316`). Challenges
(`transcript/src/challenge.rs:37-55`): squeeze u128s, reject any candidate
above `u128::MAX − ((u128::MAX % Q + 1) % Q)` (rejection probability ≈
2^−28 per squeeze — rare but it MUST be implemented, one skipped squeeze
shifts every later challenge), then `Fq::from(candidate)`.

**No inversions anywhere**: `Inv`, `Div`, `DivAssign` and `Pow` are
`unimplemented!()` (`fq.rs:431-483`); the e2e runs, so neither prover nor
verifier ever divides. The sumcheck verifier evaluates by Horner
(`sumcheck.rs:103-107`), the prover reconstructs `c1 = claim − 2c0 − c2 − …`
(`:782-803`), `eq_eval` is multiplicative (`poly/src/eq.rs:23-41`),
`opening_claim` puts `scale` into the row factor "avoiding division even
when the scale is zero" (`end_to_end.rs:244-253`).

**Decision**: implement `bitz::fq` as canonical `u128` residues with their
exact algorithms first (their `mul_wide` + Barrett is ~30 lines and lets the
first pin land in hours; our `fold.rs` already has the crypto-bigint
`U128`/wide remainder for the fold reconstruction, but Barrett is faster
than a generic 256-by-128 `rem`), the encodings and the squeeze rule
verbatim. Any exact field arithmetic gives the same values — only the
encoding and the squeeze rule can break parity — so a Montgomery-internal
variant (residues converted at the edges: coefficients on entry, `encode`
and `lift` on exit) is a stage-D option to measure, not a stage-A decision.
`Az/Bz/Cz` residues: their `build_product_mles` reduces the exact integer
products of `ProductWitgen` modulo Q (`matrix.rs:170-197`); we can equally
compute `Σ_j A[i,j]·h_j mod Q` from the sparse rows (h is 0/1, so it is a
sum of coefficient residues) — exact either way, identical values.

---

## 4. Matrices and the map: the vendored crate reproduces theirs bit for bit

Drift measured: `diff -ru` of `crates/circuit/src` (ours vs the oracle) =
12 files, +351/−520 lines, but it is (i) the `bitz`→`f2z` rename of the
`Circuit` trait methods (`bitz`/`bitz_unsigned` → `f2z`/`f2z_unsigned`),
(ii) constants moved (`ABC_BLOCK`/`ABC_DIGEST` inlined into a test on our
side), `PackedWitness::from_bits` → `packed_words`, (iii) APIs they added
that we lack (`SparseMatrix::try_from_rows`, `map_coefficients`,
`validate_shape`, `SparseMatrixError`, and the whole `impl VirtualMap for
MaterializedMTranspose` with the blake3 digest — 141 lines), (iv) our
SHA+ECDSA Wengert tape (`matrix_wengert.rs` +272). Nothing in the
generators' logic differs.

Digests decide. `examples/bitz_circuit_probe.rs` (ours, uncommitted, see
A.4) drives the vendored `ConstraintGenerator`, `MTransposeGenerator` and
`Witgen` with the d6b637e statement (re-typed on our trait: `f2z` for
`bitz`), lowers coefficients like their `bigint_to_fq` (`matrix.rs:154-166`),
and recomputes their two digests with their formulas
(`constraint_matrix_digest`, `matrix.rs:416-449`: SHA-256 of
`"bitz/spartan/constraint-matrices/v1"`, `"M"`, counts and row positions as
u64 LE, then `"A"`/`"B"`/`"C"` with `u64 len ‖ per entry u64 column ‖ u64 16 ‖
16 residue bytes`; `MaterializedMTranspose::digest`,
`matrix_transpose.rs:387-404`: blake3 of `"bitz/virtual-map/csc/v1"`, row
count, column count, nnz as u64 LE, then `column_offsets` and `row_indices`
as u32 LE). Result, seed 7:

| blocks | M rows/cols/nnz | A rows / C nnz | constraint digest | map digest | ours == theirs |
|---|---|---|---|---|---|
| 1 | 21,225 / 6,889 / 42,556 | 952 / 54,527 | `6673025d1ff9…f485` | `29d30fedf2d5…f7b4` | **both identical** |
| 8 | 168,001 / 55,105 / 342,660 | 5,824 / 435,151 | `e0d4ba5845e1…062c` | `4d70acbb0a6f…6103` | **both identical** |
| 64 | 1,342,209 / 440,833 / 2,743,492 | 44,800 / 3,480,280 | `1758f9e1693d…62d6` | `dfab9b3459cc…7580` | **both identical** |
| 608 | 12,748,801 / 4,187,905 / 26,065,860 | 423,424 / 33,060,541 | `6416b50d9235…1f78` | `5996a397d579…4141` | **both identical** |

`f`/`h` bit lengths and `is_satisfied` agree too (6,888/21,225 …
4,187,904/12,748,801). Generation cost is the same on both sides (ours 5.9 s
at 608 blocks, theirs 5.8 s; both BigInt/BTreeMap generators; 7.3 GB RSS in
our probe).

**Decision (a′): keep the vendored crate as the generator; no re-vendoring,
no dumped matrices.** What stage A adds on our side: (1) the Fq lowering +
`PreparedConstraintMatrices` (column chunks of 2^16 columns, `bind_and_batch`,
`evaluate_batched`, the digest) in `src/bitz/spartan/`; (2) the map digest
and the transpose kernel over the CSC arrays — the vendored
`MaterializedMTranspose` keeps `column_offsets`/`row_indices` private and has
no `VirtualMap` impl, so it needs a `pub fn csc(&self) -> (&[u32], &[u32])`
(the probe adds exactly that, uncommitted; the port lands it properly, or
ports their `impl VirtualMap` minus the `common` dependency); (3) our
vendored crate gates `matrix_transpose` behind its `full` feature (it uses
`field::F128`), so the `bitz-parity` feature must pull `circuit/full` (the
`ecdsa` feature already does). Guard: rerun `bitz_circuit_probe` against
`e2e_probe` whenever either copy of `crates/circuit` moves — the digests are
the drift detector.

---

## 5. Our PCS on the e2e's queries

**(a) The fold + GKR on h at the claim shapes.** Both shapes the e2e uses
over h — (14, 8) up to 64 blocks, (15, 9) at 608 — are inside the sweep our
parity PCS is pinned on, and re-confirmed today on their `dump_bitz` dumps
(seed 7): root MATCH, **narg and hints IDENTICAL**, both verifiers accept,
at n = 22 (15,396 + 102,556 B) and n = 24 (20,784 + 131,068 B). The row
weights of the e2e's claim are `scale · eq_table(point[..t])`, the column
weights `eq_table(point[t..])`, all canonical residues — `LinearClaim::new`
takes them as is and the fold lifts them exactly as it lifts the random
weights the dumps use; nothing about the e2e's claim differs from the
pinned regime except its values.

Our cost at those shapes (`bitz_bench`, 5 reps, warm, 10 threads):

| shape | commit | prove (fold+images / gkr / sumcheck+ring switch / ligerito) | verify | proof |
|---|---|---|---|---|
| (14, 8) | 1.06 ms | **26.5 ms** (0.4 / 21.3 / 1.2 / 3.4) | 1.77 ms | 15,396 + 103,356 B |
| (15, 9) | 1.92 | **44.8** (0.9 / 34.8 / 2.1 / 6.5) | 2.27 | 20,784 + 131,836 B |

versus theirs 95.2 / 273.5 ms prove (§1.3) — 3.6–6.1× on the h side alone.
Their RSS on the dumps 210 MB / 823 MB, ours 44 / 108 MB.

**(b) The single-column opening — new, and already byte-identical.** Their
`dump_lin 7` (2^22 bits committed under (14, 8), `LinearClaim<F128>` over
`Shape::new(22, 0)`): our `bitz_lin_probe` re-lays the packed witness as one
row of 2^16 words, commits under `Shape::new(22, 0)` (`Pcs::new` accepts s =
0; the commit is over the flat packed words, so **the root is identical to
theirs committed under (14, 8)**: `6d830669…a52a`), runs `prove_lin` with
`StatementBinding::Bind`: **narg 4,356 B IDENTICAL, hints 102,876 B
IDENTICAL**, our verifier accepts their proof (58.4 ms) and ours (73.5 ms).
So `xi_combined_rows_packed` with one column, the 22 row rounds, and
`fold_rows_point` over all 22 coordinates all work at `col_vars = 0` with no
code change. Cost: our `prove_lin` 77.1 ms = sumcheck 19.3 (combine 3.1, row
rounds 15.4, fold rows 0.8) + ring switch 0.6 + Ligerito 2.5 + **≈ 55 ms of
SHAKE128 absorbing the 67 MB claim** (`bind_inner_product_statement`);
theirs 95.3 ms prove / 60.3 ms verify. The absorb is a floor both sides
pay (§6).

**(c) The upstream (7, n − 7) split, measured once so the difference is
known** (`bitz_bench --shape`, added this session):

| shape | prove | verify | narg | hints |
|---|---|---|---|---|
| (14, 8) reference | 26.5 ms | 1.8 ms | 15,396 B | 103,356 B |
| (7, 15) upstream | 34.9 | 5.0 | **532,900** | 101,852 |
| (15, 9) reference | 44.8 | 2.3 | 20,784 | 131,836 |
| (7, 17) upstream | 71.8 | 16.4 | **2,106,288** | 131,836 |

Our forest runs fine at t = 7 (`Forest::new` needs t ≥ 4; the arena is
2^{t−4+s} = 2^18 / 2^20 entries), 30–60 % slower than the reference split,
but the point to make upstream is the proof: 2^s folds × 16 B put the narg
at 0.5–2.1 MB against 15–21 KB. The oracle branch's `shape_for` commit
(`90655c4`) is the right thing to push.

---

## 6. The virtual reduction (`transpose_query`)

Arithmetic (value-identical on our side by construction): `weights[c·2^t +
r] = u2[c] · u1[r]` over GF(2^128) (the serial `flat_map`,
`virtual_map.rs:173-177`); `M^T weights` = for every column j of M (= bit j
of `1 ‖ f`) the **XOR** of `weights[i]` over the rows i with `M[i, j] = 1`
(`matrix_transpose.rs:337-348`: `lo ^= …; hi ^= …` — addition in
characteristic 2, no multiplies), weights beyond `h_len` ignored (`:375-377`);
the constant column j = 0 is split off and its weight subtracted from the
target (`TransposedWeights::adjusted_target`, `virtual_map.rs:105-107`); the
remaining `f_len − 1` weights are zero-padded to `2^committed_bits`
(`:187-188`) and become the single-column claim with column weight
`F128::from(1)` (`:189-192`).

Cost, measured at 608 blocks (h = 2^24 padded, 26,065,860 nonzeros): the
expansion **31.6 ms** (serial in their code; trivially parallel, ≈ 4 ms on
our side), `map.transpose` **5.2–5.5 ms** (parallel gathers, 26.1 M XORs —
memory-bound, ≈ 0.2 ns/nonzero), memory 256 MB for the weights + 64 MB for
the output + the CSC payload 121 MB. At ≤ 64 blocks the transpose is
0.1–0.6 ms. It is **not** "a large share of their prover": the share is the
serial expansion (5 %) plus the absorb that follows.

The cost that IS large and unavoidable: `bind_inner_product_statement`
absorbs the 67,108,904-byte single-column claim on **both** sides
(prove and verify), 55–76 ms per absorb here (SHAKE128 via the `sha3` crate,
portable Keccak). It is part of the transcript, so parity requires it. The
one byte-identical lever is the `sha3`/`keccak` `asm` feature (ARMv8 SHA3
instructions; `spongefish` → `sha3 0.10` → `keccak`) — untested, worth one
measurement in stage D; a protocol-level fix (absorb a digest of the claim
instead) is theirs to make, not ours.

---

## 7. The Spartan prover

Their algorithms (`spartan/src/sumcheck.rs`, header lines 6-28): cubic
outer rounds sending `[c0, c2, c3]`-derived `[c0, c1, c2, c3]` with `c1`
reconstructed from the running claim (`:782-803`, `:825-854`), split `eq(tau,
·)` tables at `point.len()/2` (`poly/src/eq.rs:108-116`; the split point is
value-irrelevant), fold/round fusion (`fold_products_and_compute_next`,
`:1008-1073`), ping-pong scratch buffers, rayon above 2^12 items; the inner
sumcheck folds the dense batched matrix `D(j) = Σ_i eq(i, r_x)(A + ρB +
ρ²C)[i, j]` (`bind_and_batch`, `matrix.rs:283-332`, one 2^16-column chunk per
task) against the 0/1 assignment MLE, quadratic rounds `[c0, c1, c2]`. Every
coefficient is an exact field sum, so any correct implementation produces
identical bytes; the reduction order is free.

Mapping to what we have: our `src/piop/sumcheck/` is a different protocol
(eq-factored driver, case-LUT rounds, univariate skip) and its messages
cannot be reused, but its structure (row-weight tables, fused fold+round
passes over `[a0, a2]`) is the same shape; the `u128` wide multiply +
reduction is new (their `Fq` mul ≈ 10–20 ns scalar). Work at 608 blocks:
`bind_and_batch` 33.7 M nonzero multiply-adds (+ 2^19 row scales), the inner
sumcheck ≈ 3 mul per pair per round over 2^23, 2^22, … pairs ≈ 50 M
multiplies, the outer sumcheck ≈ 15 M over 2^19 × 3 tables; ≈ 100 M `Fq`
multiplies total, which their code does in **122–133 ms on 10 threads**
(≈ 1.3 ns amortised, ≈ 13 ns per multiply per thread — i.e. their kernels are
already reasonable). The verifier's `evaluate_batched` walks every nonzero
once (27 ms).

**Decision: port their code shape first** (the two provers, the reusable
`SumcheckProof::verify`, `PreparedConstraintMatrices`), pin on the abc
compression (their test `spartan/tests/sha256_piop.rs`: session
`spartan/piop/sha256-compression/v1`, instance `abc-single-compression`, M
20,457 × 7,145, 184 rows → 2^8, assignment 2^15, 8 + 15 rounds) and on 8
blocks, then optimise in stage D (Montgomery form, NEON `umulh` lanes, a
transposed `bind_and_batch`) under byte identity — exactly the PCS
methodology. Expected: parity at their speed in stage A (≈ 125 ms at 608),
then whatever the kernels give.

---

## 8. Verifier parity both ways: `dump_e2e` / `verify_e2e`

Their verifier for the e2e is `Prepared::verify` (`end_to_end.rs:187-198`),
not the spec verifier crate alone, so both examples live in
`tooling/cli/examples/` on the oracle branch with the d6b637e SHA-256
statement inside them (as `e2e_probe` already does):

- `dump_e2e <sha256-compression|sha256-chain> <blocks> <seed> <dir>`: the
  seeded statement (SplitMix64 derivation of §1.1 — both probes already
  agree on the resulting digest word, `b26773d6` at 1 block, `bccbd0f8` at
  608), `Prepared::new/witness/commit/prove/verify`, then the dump.
- `verify_e2e <circuit> <blocks> <seed> <dir> <ours.spartan.bin>
  <ours.narg.bin> <ours.hints.bin>`: rebuilds `Prepared`, reads our three
  files, constructs `Proof { root, spartan, opening }` (every field of
  `SpartanPiopProof`, `OuterSumcheckProof`, `SumcheckProof` is `pub`), runs
  `Prepared::verify`. The root is recomputed by committing (it must equal
  the dumped one).

Dump format (byte-defined, every integer LE, every field element 16 B):

```
meta.txt      key=value lines: circuit, blocks, seed, session=bitz/circuit-e2e/v1,
              instance=<domain>, claim_t, claim_s, committed_t, committed_s,
              q, generator (hex), root (hex), constraint_digest (hex),
              map_digest (hex), h_len, f_len, rows (A row count),
              num_row_vars, num_column_vars, narg_len, hints_len, spartan_len
public.bin    statement.public_bytes()
inputs.bin    the input bits, packed LSB-first (input_bits/8 bytes)
f.bin         the committed packed witness, flat (2^committed_bits/8 bytes) — regenerable
h.bin         the virtual bits, flat (2^claim_bits/8 bytes)                — regenerable
products.bin  Az ‖ Bz ‖ Cz residues, 3 × 2^num_row_vars × 16 B               — small sizes only
claim_h.bin   the LinearClaim<Fq> on h: u64 rows ‖ rows ‖ u64 cols ‖ cols ‖ target
spartan.bin   §2.2 item 4
narg.bin      the narg string
hints.bin     the hint stream
```

Our harness (`examples/bitz_e2e_parity.rs`, the analogue of `bitz_parity`)
regenerates `f`, `h` and the products from `inputs.bin` through the vendored
crate, checks them against `f.bin`/`h.bin`, recomputes and compares both
digests, re-proves, diffs `spartan.bin`/`narg.bin`/`hints.bin`, verifies
theirs and ours, writes `ours.*` for `verify_e2e`; `--sweep` over sizes and
seeds like today's `bitz_parity --sweep`.

---

## 9. Effort and risks

Stages, in order, each ending with identical bytes on dumped vectors, both
verifiers accepting both proofs, unit tests and a commit (`--no-gpg-sign`;
only `src/bitz`, `examples/bitz_*`, `docs/bitz-*`, manifests):

| stage | what | estimate | what could block |
|---|---|---|---|
| A | `bitz::fq` (Barrett, encodings, squeeze rule) + `PreparedConstraintMatrices` (chunks, bind-and-batch, evaluate, digest) + outer/inner provers + reusable verifier + `spartan.bin`; `dump_spartan` on the oracle (the abc test's session, then the e2e's transcript prefix at 8 blocks) | 4–5 days | an arithmetic slip in `Fq` (guard: unit tests against their vectors, dumped round polynomials); the products' residues (compute from the sparse rows, exact) |
| B | `VirtualParams`/`VirtualStatement`, `opening_claim`, `transpose_query` (expansion, CSC gathers, constant weight, padding), `prove_virtual`/`verify_virtual` framing, the `Prepared` equivalent; `dump_e2e`/`verify_e2e`; identical at 1 then 8 blocks | 3 days | nothing known: the PCS at (14, 8)/(15, 9) and the (22, 0) opening are pinned |
| C | 64 and 608 blocks, the seed sweep, `BITZ_REPEAT`, both verifiers both ways at every size | 1 day | memory at 608 (their e2e 9.3 GB is the generator; ours ≈ 2–3 GB: Fq matrices ≈ 0.8 GB, CSC 121 MB, expansion 256 MB, dense D 256 MB) |
| D | speed, byte-identical: the Fq kernels, the expansion in parallel, the `sha3` asm absorb, then profiles | 2–3 days | none (measure-only) |
| E | the comparison table (their CLI vs ours at 1/8/64/608; the crate's own SHA-256 SNARK rows) | 1 day | — |

Risk register, ordered as the prompt asked, with today's status:

0. **Upstream moving under the oracle branch** — not moved as of today
   (§1); if it moves: re-merge `origin/feat/circuit-e2e` + `origin/bitz-k4`
   into a fresh `bitz-e2e-k4`, re-apply `90655c4`'s `shape_for`, rerun the
   tests and both probes (digests + `e2e_probe` sizes). The branch is local
   and unpushed; pushing it or the split is the user's call.
1. **Unknown `Encoding` details** — resolved: every absorbed struct and every
   message is listed with file:line (§2.3–2.4); the only surprise was the
   dead `bitz_statement` frame (§1.1).
3. **Circuit drift** — resolved: identical digests at all four sizes (§4);
   the guard is the probe pair.
4. **The single-column query path** — resolved: byte-identical (§5b).
5. **The 256 MB transpose** — measured: 5.5 ms + a 32 ms serial expansion +
   256 + 64 MB; the real cost next to it is the 67 MB absorb (§6).
7. **Seeding the blocks** — resolved: SplitMix64 derivation implemented in
   `e2e_probe` and mirrored in `bitz_circuit_probe`; `dump_e2e` inherits it.
8. New: **Spartan is invisible in the narg** — the parity set carries
   `spartan.bin` and the terminal claim explicitly (§2.2); never claim PIOP
   parity from a narg diff.
9. New: **the rejection-sampled `Fq` squeeze** — implement the loop, test
   with their `fq_rejects_the_incomplete_final_interval` vector
   (`challenge.rs:74-91`).

Projected e2e at 608 blocks once A–C land (their algorithms for Spartan, our
PCS for the rest): prove ≈ 125 (Spartan) + 45 (fold/GKR at (15, 9)) + 4
(expansion, parallel) + 5 (transpose) + 77 (opening incl. the absorb) ≈
**≈ 260 ms vs their 590**, verify ≈ 27 + 2 + 4 + 5 + 58 ≈ **≈ 100 ms vs
their 276**, proofs identical (20,708 + 103,932 B + 2,416 B out of band),
RSS a few GB below theirs. Stage D moves Spartan and, if `asm` works, the
absorb.

---

## Appendix

### A.1 Commands (this session)

```sh
# oracle side (their target dir, native flags)
cd ~/f2z-benchmark && git checkout bitz-e2e-k4 && git fetch origin
CARGO_TARGET_DIR=$HOME/f2z-benchmark/target RUSTFLAGS="-C target-cpu=native" \
  cargo build --release -p tests -p bitz-cli --examples
E=$HOME/f2z-benchmark/target/release/examples
/usr/bin/time -l $E/e2e_probe sha256-compression 1 7        # also: sha256-chain 8|64|608 7
/usr/bin/time -l $E/e2e_probe sha256-chain 608 7 --bench-only   # the e2e's own RSS
$E/e2e_probe sha256-chain 608 7 --skip-e2e                  # Spartan alone + query costs
$E/dump_lin 7 $SCRATCH/lin_seed7                            # the single-column opening
$E/dump_bitz 22 7 $SCRATCH/dump_n22_seed7; $E/dump_bitz 24 7 $SCRATCH/dump_n24_seed7
# our side (CARGO_TARGET_DIR is globally ~/zinc-plus/target)
cd ~/f2z-pcs && git checkout bitz-parity
RUSTFLAGS="-C target-cpu=native" cargo build --release --features bitz-parity,ecdsa \
  --example bitz_circuit_probe --example bitz_lin_probe --example bitz_bench --example bitz_parity
O=$CARGO_TARGET_DIR/release/examples
$O/bitz_circuit_probe sha256-chain 608 7                    # digests vs e2e_probe's
BITZ_TRACE=1 $O/bitz_lin_probe $SCRATCH/lin_seed7           # single-column opening parity
BITZ_REPEAT=5 $O/bitz_parity $SCRATCH/dump_n24_seed7        # PCS parity at (15, 9)
$O/bitz_bench 24 --reps 5; $O/bitz_bench 24 --shape 7:17 --reps 5
```

### A.2 Full digests (seed 7)

```
blocks  constraint digest (SHA-256)                                        map digest (blake3)
1       6673025d1ff9539850e8e69c1af41f4e2068ae18a0abde4190c52b2ee590f485  29d30fedf2d58caccd328d0f369635cc3400462da4e723c1d8bde44aebfdf7b4
8       e0d4ba5845e1b35e9ac990942034ec4eb662dec6c18e6598c550381b8545062c  4d70acbb0a6fdcc420df61513df975f18bc97368e8c2169d11632bdea43f6103
64      1758f9e1693d9f1a378b6bed3fd78c9a453547842512cd03725603f5faf062d6  dfab9b3459cc3ec58a49bfbe434d9748c081de7f39569dd9770665aef8197580
608     6416b50d92350577c182ea75ff875fa697f2b6dce0e9eda007d743a62e7b1f78  5996a397d57956bfa664436a013aacb67d77d13f316d9d2d6c9282f3c18e4141
```
Roots of the e2e commits (seed 7): 1 block `9e984f64…b159`, 8 `f30602ac…95c7`,
64 `88959593…8b7f`, 608 `7f8fdac3…d047`; `dump_lin 7` root `6d830669…a52a`
(= ours at (22, 0)).

### A.3 Raw lines worth keeping

```
# e2e_probe, 608 blocks, clean run
spartan alone: prove 124.7 ms, verify 27.0 ms, outer_rounds=19 inner_rounds=24 narg_bytes=0 point_len=24
transpose: 5.2 ms (min of 3) over 16777216 weights, 26065860 nnz
query costs: row(x)column expansion 31.6 ms (serial, 16777216 products), eq_table(24) 28.8 ms, absorb of the single-column claim (67108904 B) 76.4 ms
benchmark::run: setup_ms=7052.497 witness_ms=36.866 commit_ms=0.533 prove_ms=590.548 total_prove_ms=591.081 verify_ms=275.848
by hand: ... narg=20708 B hints=103932 B spartan(out of band)=2416 B (outer 19 rounds x4, [Ah,Bh,Ch], inner 24 rounds x3)
# e2e_probe, 1 block: query costs: expansion 8.3 ms, eq_table(22) 3.1 ms, absorb (67108904 B) 55.1 ms
# dump_lin 7 (theirs): prove: 95.3ms  verify: 60.3ms  narg_len=4356 hints_len=102876
# bitz_lin_probe (ours, BITZ_TRACE=1):
shape=(22,0) commit 13.2ms root ours=6d83…a52a theirs=6d83…a52a MATCH
bitz:     sc combine cols          3.1ms   235.6 MB faulted
bitz:     sc row rounds           15.4ms   187.2 MB faulted
bitz:     sc fold rows           756.3µs
bitz:   sumcheck                  19.3ms
bitz:   ring switch              615.8µs
bitz:   ligerito                   2.5ms
our prove_lin: 77.1ms
narg:  ours=4356 B theirs=4356 B IDENTICAL
hints: ours=102876 B theirs=102876 B IDENTICAL
our verifier on THEIR proof: Ok(()) eof=true (58.4ms)
our verifier on OUR proof: Ok(()) eof=true (73.5ms)
# their dump_bitz 22/24 seed 7: prove 95.2 / 273.5 ms, verify 1.6 / 2.4 ms, RSS 210 / 823 MB
# our bitz_parity on them (BITZ_REPEAT=5): prove min/median 25.0/26.7 and 42.3/42.5 ms, IDENTICAL, RSS 44 / 108 MB
```

### A.4 Probe files (this session) and what is committed

Our tree (`bitz-parity`, the commit that adds this document): this document,
`examples/bitz_lin_probe.rs` (+ its `[[example]]` entry), the `--shape t:s`
flag in `examples/bitz_bench.rs`. **Uncommitted, on purpose** (throwaway, not
in the allowed commit set): `examples/bitz_circuit_probe.rs` (needs
`--features bitz-parity,ecdsa`) and the 3-line `pub fn csc(&self)` accessor
on the vendored `crates/circuit/src/matrix_transpose.rs`
`MaterializedMTranspose` (marked "FEASIBILITY PROBE ACCESSOR"); stage A lands
the accessor properly. Oracle branch `bitz-e2e-k4`: `tooling/cli/examples/
e2e_probe.rs`, `crates/tests/examples/dump_lin.rs` and the two
dev-dependencies committed as `6486ce8` on top of `90655c4` (their code
untouched; the branch stays local and unpushed). Scratch dumps live in the
session scratchpad and are regenerated each session.

### A.5 Oracle branch tests

`cargo test --release -p bitz-cli -p spartan -p common -p tests -p circuit`
on `90655c4`, 2026-09-16, exit 0: bitz-cli 2 + 2 (`end_to_end.rs`), circuit
48, common 43, spartan 22 + 1 (`sha256_piop.rs`), tests 10 (`fold`) + 2
(`host`) + 8 (`prove`) + 6 (`virtual_prove`) — all passed, the counts the
prompt recorded when the branch was made.
