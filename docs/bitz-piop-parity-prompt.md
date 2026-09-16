# Continue: BitZ end-to-end parity — the Spartan PIOP and the virtual reduction on top of the parity PCS — session prompt

Paste this into a fresh session in `/Users/albertgarretafontelles/f2z-pcs`
(branch `bitz-parity`, tip `afbfd73` or later; read
`docs/bitz-parity-continue-prompt.md` first — it is the PCS half of this
work and fixes the conventions this prompt reuses). `docs/` is gitignored:
stage docs with `git add -f`.

---

We have a **parity PCS**: `src/bitz/` (feature `bitz-parity`) reproduces
`worldfnd/BitZ`'s BitZ PCS (their clean-room F2Z; the repo was
`worldfnd/f2z-benchmark` — the old URL redirects, the local clone
`~/f2z-benchmark` still names it) byte for byte on their `bitz-k4` branch:
narg strings and hint streams IDENTICAL on n = 22..28 across random seeds,
both verifiers accept both proofs, and our prover is 5–20× faster than
theirs. The next goal is an **end-to-end parity scheme**: their Spartan
PIOP over the circuit R1CS, its reduction to a claim on the virtual bits
`h = M (1 ‖ f)`, the fold + GKR on `h`, the transpose of the reduced claim
onto the committed bits `f`, and the PCS opening — all re-implemented here
over this crate's field and engines, producing the same bytes as their
`tooling/cli` end-to-end prover. Their code is the oracle; we never change
a byte. **The first session's deliverable is a feasibility analysis**
(`docs/bitz-piop-parity-feasibility.md`), not code; the port follows the
methodology the PCS used (stage by stage, each stage pinned by byte
identity on dumped vectors before anything is optimised).

## The oracle: what their end-to-end proof is

Everything below is on their branch `feat/circuit-e2e` (`d6b637e`,
2026-09-16, "feat: generate inputs for circuit benchmark runs";
https://github.com/worldfnd/BitZ/blob/d6b637e855d0bdfaacde437ff49095b0912d63ad/tooling/cli/src/end_to_end.rs#L81
is the line the user's colleagues flagged as important: `Prepared::new`,
the setup of the whole scheme — read it first; then `0013ce4` "refactor:
separate circuit adapters from proving tools", which REMOVED the SHA-256
adapter (`tooling/cli/src/sha256.rs`) and the CLI's circuit selection —
the tip's `circuit-e2e` binary registers no adapter, `bitz_cli::end_to_end::
{CircuitStatement, Prepared}` + `bitz_cli::benchmark::run` remain, and the
only registered statement is the test's one-bit `PublicBit`). **Our oracle
is the local branch `bitz-e2e-k4` in `~/f2z-benchmark`** (see "The oracle
branch" below). Files:

- `tooling/cli/src/end_to_end.rs` (290 lines): `CircuitStatement` (a
  deterministic circuit with public inputs; `domain`, `public_bytes`,
  `input_bits`, `synthesize`), `Prepared::new` (**line 81**: synthesise the
  circuit ONCE through `ConstraintGenerator` into integer R1CS matrices
  `M, A, B, C`, lower `A, B, C` to `Q100` with `bigint_to_fq`, build
  `PreparedConstraintMatrices` — which computes the canonical
  **constraint digest**; synthesise AGAIN through `MTransposeGenerator`
  into the materialised map `M^T` — which has its own **map digest**; check
  `map.h_len() == a.column_count()`; claim shape = `shape_for(h_len)`,
  committed shape = `shape_for(f_len − 1)` where, ON OUR ORACLE BRANCH,
  `shape_for(bits) = Shape::for_log_bits(max(log2(next_pow2(bits)), 22))`
  — the reference split `t = ⌈0.6·n⌉`, the one our parity PCS is
  validated on (upstream `feat/circuit-e2e` has `Shape::new(7, log_bits −
  7)` there: t = 7 rows, everything else columns — we changed it, see
  below); `BitZParams<Q100>` with `smallest_generator()`;
  `Pcs::new(committed_shape, Fast, Blake3)`),
  `witness` (`ProductWitgen` replay → `f`, `h`, exact products
  `Az, Bz, Cz`; satisfaction check; `pack` into `F128` words zero-padded
  to the shapes), `commit`, `prove`, `verify`, `bind`, `opening_claim`.
- `prove` order: `build_prover(b"bitz/circuit-e2e/v1", statement.domain())`
  → `bind`: `public_message` of `public_bytes.len() as u64`, the public
  bytes, the root, the `BitZParams`, the `Pcs`, the map digest →
  `prove_spartan_piop` (crate `spartan`, `src/piop.rs`): absorb the
  constraint digest, squeeze `tau` (one `Fq` per row variable), outer
  sumcheck (cubic; each round `public_message(coefficients)` then squeeze),
  absorb `[Ah(r_x), Bh(r_x), Ch(r_x)]`, squeeze `rho`, inner sumcheck
  (quadratic) on the batched matrix against the assignment MLE →
  `ScaledMleEvaluationClaim { point, scale, value }` → `opening_claim`:
  zero-extend the point to the claim shape's `log_bits`, rows =
  `scale · eq_table(point[..t])`, columns = `eq_table(point[t..])` for the
  claim shape's `t`, target = `value` → `LinearClaim<Fq>` on `h` →
  `VirtualStatement::new(params,
  committed_shape, &map, &claim)` → `BitZProver::prove_virtual`
  (`crates/prover/src/prove.rs`): `public_message(b"bitz/virtual-statement/v1")`,
  root, `VirtualParams`, map digest, the claim; `fold_and_reduce` on the
  VIRTUAL bits `h` (the fold round + GKR we already mirror in
  `src/bitz/{fold,reduce,forest,gkr}.rs`, at the claim shape);
  `statement.transpose_query(query)` (`crates/common/src/virtual_map.rs`:
  the reduced query's `F128` weights over `h` — `eq_table(point)` for an
  MLE query, `row ⊗ column` for an inner-product query — pushed through
  `M^T`, the constant-one coordinate folded into the target, the result
  zero-padded to the committed bit count as a single-column
  `LinearClaim<F128>`); `pcs.prove_lin(data, committed_bits, &query,
  StatementBinding::Bind, transcript)` = the PCS opening we already mirror.
  The e2e `Proof` = `{ root, spartan: SpartanPiopProof (outer + inner round
  polynomials, carried OUT OF BAND), opening: transcript::Proof (narg +
  hints) }`.
- `verify`: `build_verifier` + `bind` + `verify_spartan_proof` (re-absorbs
  the out-of-band round polynomials as public messages, recomputes the
  terminal claim through `evaluate_batched`) + `opening_claim` +
  `verify_virtual` (`crates/verifier/src/verify.rs`) + `check_eof`.
- `crates/spartan/`: `piop.rs` (376), `matrix.rs` (627:
  `PreparedConstraintMatrices` — column-chunked nonzeros, `bind_and_batch`,
  `evaluate_batched`, `constraint_matrix_digest` at line 416 = SHA-256 of
  `"bitz/spartan/constraint-matrices/v1"`, `"M"` + counts + row positions,
  then `"A"/"B"/"C"` sparse rows with `Fq` coefficient bytes),
  `sumcheck.rs` (1761: the reusable `SumcheckProof<F, COEFFS>` verifier;
  prover optimisations listed in its header — missing-at-one `c1`
  reconstruction, split eq tables, parallel accumulation above 2^12,
  fold/round fusion, ping-pong scratch). Test `tests/sha256_piop.rs` (one
  `abc` compression: `M` 20,457 × 7,145, `A/B/C` 184 rows → 2^8, assignment
  2^15, 8 + 15 rounds; sessions `spartan/piop/sha256-compression/v1` /
  `abc-single-compression`); bench `benches/sha256.rs` (608 blocks = the
  e2e size: its bit witness fills the 2^22-bit committed shape).
- `crates/circuit/`: `constraints.rs` (`ConstraintGenerator`, `SparseMatrix`,
  `SparseBoolMatrix`, `ConstraintMatrices<BigInt>`, BTreeMap-ordered — the
  generation is deterministic), `witgen.rs` (`ProductWitgen`,
  `PackedWitness`, `Z<LIMBS>`), `matrix_transpose.rs` (`MTransposeGenerator`,
  `MaterializedMTranspose`, its blake3 digest over counts, column offsets
  and row indices), `sha256.rs` (`compress`, `compression_circuit`,
  `sha256_block_aligned_circuit`, `COMPRESSION_INPUT_BITS = 768`,
  `COMPRESSION_HINT_BITS`), `p256.rs`, `ecdsa_sha256.rs`.
- Field and transcript: `Fq<const Q: u128>(u128)` canonical residues,
  `Q100 = 2^100 − 15`, multiply = `mul_wide` + `reduce_wide`;
  `Encoding<[u8]> for Fq` = `lift().to_le_bytes()` (16 bytes);
  `TranscriptChallenge for Fq` (`crates/transcript/src/challenge.rs:37`) =
  **rejection sampling**: squeeze `u128`s, reject any above
  `u128::MAX − ((u128::MAX % Q + 1) % Q)`, reduce the first accepted one.
  Round polynomials and evaluations enter through `public_message` —
  they are NOT in the narg string.
- The CLI at `d6b637e` (`tooling/cli/src/main.rs` + `sha256.rs`):
  `circuit-e2e --circuit sha256-compression|sha256-chain [--num-blocks N]
  [--threads N]`; blocks came from `rand::rng()` (UNSEEDED — a dump example
  must derive them from a seed); it printed `setup_ms witness_ms commit_ms
  prove_ms total_prove_ms verify_ms`; nothing was dumped. At the tip
  (`0013ce4`) the SHA-256 adapter is gone: restore `sha256.rs` from
  `d6b637e` (`git show d6b637e:tooling/cli/src/sha256.rs`) as a fixture of
  the dump/verify examples on the oracle branch — it is their code, keep
  it verbatim. `crates/host` (the wire format) covers the PCS proof only;
  the e2e `SpartanPiopProof` has no canonical byte encoding on their side.

**The oracle branch (done 2026-09-16, local only).** `feat/circuit-e2e`
forks from `main` (`0c75fd8`) and does NOT contain `bitz-k4`'s commits —
`2882439` (the `Fast` profile on the k = 4 Ligerito ladder,
`crates/pcs/configs/ligerito-k4/`, which is what `src/bitz` mirrors),
`344903c` (`Shape::for_log_bits`, tests on `2^100 − 15`) and `61ad2c8`
(the `dump_bitz`/`verify_bitz` examples) — so upstream's e2e opens
through a DIFFERENT Ligerito ladder than our PCS. In `~/f2z-benchmark`
the branch **`bitz-e2e-k4`** = `origin/feat/circuit-e2e` (`0013ce4`) with
`origin/bitz-k4` (`344903c`) merged (clean, `c0cac39`) and one commit of
ours on top (`90655c4`, tip of the branch; their tests green on it:
`bitz-cli` 4, `circuit` 48, `common` 43, `spartan` 23, `tests` 26):
`tooling/cli/src/end_to_end.rs::shape_for` now uses
`Shape::for_log_bits` (the reference split) instead of `Shape::new(7,
log_bits − 7)`, with a unit test pinning `(14, 8)` up to `2^22` bits and
`(15, 9)` at `2^24`. Consequences: the claim shape over `h` and the
committed shape over `f` are both reference splits of their own padded
lengths (1 block: `h` 20,457 → `2^22` floor → `(14, 8)`, `f − 1` 7,144 →
`(14, 8)`; 608 blocks: `f` ≈ 608 × 6,888 ≈ `2^22` → `(14, 8)`, `h` ≈
`2^24` → `(15, 9)` — verify these counts in the feasibility doc), so our
PCS runs in its validated regime and the only new shape feature is the
single-column `InnerProduct` query on `f` after the transpose. The branch
is NOT pushed to `worldfnd/BitZ`; pushing it (or asking the colleagues to
adopt the split) is the user's call. `main` at `0c75fd8` differs from
`bitz-k4` only by those three commits; the PIOP crates are identical on
both.

## Our side

- `src/bitz/` at `afbfd73`: the PCS half (fold, reduce, forest/GKR,
  kernels, sumcheck, ring switch, Ligerito via flock, transcript, codec,
  params) — `prove` = their `BitZProver::prove` on committed bits; there is
  NO virtual path yet (`prove_virtual` / `verify_virtual` /
  `VirtualStatement` / `transpose_query` / `VirtualParams` are new), no
  Spartan, no `Fq` field arithmetic beyond the fold's `u128` residues and
  the wide-remainder reconstruction (`fold.rs`).
- The crate's OWN Spartan PIOP (`src/piop/spartan/`, 51k lines: the
  unified `protocol/` runner with `RelationSpec`, its own `prove_virtual`,
  a transcript-sampled prime, `raw_monty.rs`, the univariate skip, the
  SHA-256 and ECDSA relations) is a DIFFERENT protocol with a different
  transcript. It is not the oracle and its messages cannot be reused; its
  kernels (`src/piop/sumcheck/`, `src/utils/wide_mul.rs`, the NEON
  GF(2^128) pipeline) can, where the field values are identical.
- `crates/circuit/` is a VENDORED copy of their circuit crate (`8f91431`,
  2026-09-03) plus six local commits (SHA+ECDSA Wengert tape, Montgomery
  conversions, packed SHA products). Against their `bitz-k4` copy 12 files
  differ (+525/−351: `constraints.rs` +291, `matrix_transpose.rs` +146,
  `matrix_wengert.rs` −272, `sha256.rs`, `witgen.rs`, …). Do not assume it
  generates their matrices bit for bit.
- Tooling to reuse as is: `examples/bitz_parity.rs` (dump loader, byte
  diff, both verifiers, `BITZ_REPEAT`, `--sweep`), `examples/bitz_bench.rs`
  (the paper-metrics bench with `bitz::record_phases`/`take_phases`),
  `BITZ_TRACE` (phase timings + page faults per phase), the acceptance
  loop and the warm/cold methodology of the PCS prompt.

## Step 1 — the feasibility analysis (this session's deliverable)

Write `docs/bitz-piop-parity-feasibility.md` answering, with evidence
(commands run, numbers, file:line citations), at least:

0. **The oracle branch** exists: `bitz-e2e-k4` in `~/f2z-benchmark`
   (`git log --oneline -3` there; `git fetch origin` first and check
   whether `feat/circuit-e2e` or `bitz-k4` moved again — if so, re-merge
   and re-apply the `shape_for` commit; keep the packages `bitz-cli`,
   `spartan`, `common`, `tests`, `circuit` green: `CARGO_TARGET_DIR=
   $HOME/f2z-benchmark/target RUSTFLAGS="-C target-cpu=native" cargo test
   --release -p bitz-cli -p spartan -p common -p tests -p circuit`). Still
   to do here: restore the SHA-256 adapter from `d6b637e` as an example
   fixture, run the e2e at 1, 8, 64, 608 blocks through
   `bitz_cli::benchmark::run` (a small example: seeded blocks, expected
   digest via `sha2::compress256` as the old CLI did), record
   `setup/witness/commit/prove/verify` and peak RSS per size, confirm which
   Ligerito ladder `Pcs::new(.., Fast, ..)` resolves to (it must be
   `ligerito-k4`, what `src/bitz` embeds) and the `(t, s)` shapes
   `Prepared::new` derives per size. Pin the exact commit in the doc.
1. **The byte surface.** List every transcript event of the e2e prove in
   order with its encoding, split into (a) narg bytes (prover messages:
   the 2^s integer folds — note `s = log_bits(h) − 7`, i.e. up to 2^17
   folds × 16 B ≈ 2 MB at 608 blocks — the GKR messages, the PCS sumcheck
   and ring-switch messages, the PoW nonces), (b) hints (the Ligerito
   proof), (c) public messages (round polynomials, `[Ah, Bh, Ch]`, digests,
   params, the claim — they shape the challenges but are not transmitted
   in the transcript), (d) the out-of-band `SpartanPiopProof` and the
   terminal `ScaledMleEvaluationClaim`. Define the parity comparison set
   accordingly: narg, hints, root, the Spartan proof in a canonical dump
   encoding (propose: outer then inner round polynomials in round order,
   each coefficient as 16 LE bytes; the terminal claim as point ‖ scale ‖
   value), and the recomputed digests. Check what `VirtualParams`,
   `BitZParams`, `Pcs` and `LinearClaim<Fq>` encode to (`Encoding` impls)
   — every one of them is absorbed.
2. **The field.** `Fq<Q100>`: residues as `u128`, canonical encoding,
   rejection-sampled challenges (`challenge.rs:37`), `mul_wide` +
   `reduce_wide`, `Self::from(u128)` reduction. Decide how we implement it
   in `src/bitz` (a const-modulus type over `crypto_bigint::U128` widening
   ops like `fold.rs`'s `weighted_sum_mod`, or Montgomery internally with
   canonical bytes at the edges — any exact field arithmetic gives the
   same values; only encodings and the squeeze rule can break parity).
   Check whether the verifier needs inversions (`evaluate_round`-style
   divisions) and whether their prover ever divides.
3. **Matrices and the map.** Can our vendored `crates/circuit` reproduce
   their `M, A, B, C` and `M^T` bit for bit for the SHA-256 statements?
   Test it: dump from the oracle the constraint digest, the map digest, the
   counts (`M` rows/cols, `A/B/C` rows/cols/nonzeros, `h_len`, `f_len`) for
   1 and 8 blocks, and compute the same from our vendored crate. If they
   differ (likely, given the drift), choose and justify one of: (a)
   re-vendor their `crates/circuit` at the oracle commit as a second copy
   (`crates/bitz-circuit`, or a `bitz` feature of the existing crate) —
   check its dependencies (`num-bigint`, `num-traits`, `blake3`, …) and
   whether our crate's SHA+ECDSA path can keep its own copy; (b) consume
   dumped `M, A, B, C` (Fq coefficients) and `M^T` (offsets/indices) for the
   first stages and regenerate them later. Either way the digests must be
   recomputed on our side and compared, not copied.
4. **Our PCS on the e2e's queries.** With the reference split on the
   oracle branch, the fold + GKR over `h` and the opening over `f` run at
   the shapes our parity PCS is validated on ((14, 8) at 1 block and at
   the 608-block committed shape; (15, 9) for `h` at 608 blocks — inside
   the sweep's range). What is new: (a) the fold's claim on `h` is an
   `Fq` claim whose row weights are `scale · eq_table(point[..t])` and
   column weights `eq_table(point[t..])` (all residues; `LinearClaim::new`
   takes them as is); (b) after `transpose_query` the PCS opens a
   single-column `LinearClaim<F128>` over `2^22` bits (`OpeningQuery::
   InnerProduct` with `Shape::new(log_bits, 0)`) — our `pcs::prove_lin` /
   `sumcheck::prove` were only exercised with the two-sided shapes; check
   `col_vars = 0` end to end (`xi_combined_rows_packed` with one column,
   `fold_rows_point` over all bits) with a dumped vector; (c) the upstream
   `(7, n − 7)` split is not needed for parity any more, but if the
   colleagues keep it upstream, measure our PCS at `(7, 15)`/`(7, 17)` once
   (`Forest::new` needs t ≥ 4, the table-driven path t ≥ 6; the arena is
   `2^{t−4+s}` entries) so the difference is known.
5. **The virtual reduction.** `transpose_query` applies `M^T` to a dense
   `F128` weight vector of length `h_len` (2^24 × 16 B = 256 MB at 608
   blocks) — a sparse Boolean matrix–vector product over GF(2^128). Estimate
   its cost (nonzero count of `M`) and memory; it is likely a large share
   of their prover. Confirm the exact arithmetic (XOR accumulation, the
   constant-weight adjustment of the target, zero padding) so ours is
   value-identical.
6. **The Spartan prover.** Map their outer/inner sumcheck to what we have:
   cubic rounds with `[c0, c2, c3]` and `c1` from the claim, split eq
   tables, fold/round fusion — all exact field sums, so any correct
   implementation gives identical coefficients; the cost is in `Fq`
   multiplies (u128 wide multiply + reduction ≈ 10–20 ns scalar). Count the
   work (rows 2^8·blocks, nonzeros) and estimate. Decide whether to port
   their code shape first (fast to pin) and optimise after, as with the
   PCS.
7. **Verifier parity both ways.** Their verifier for the e2e is
   `Prepared::verify` (not the spec-derived `verifier` crate alone). We
   need, on the oracle branch, `dump_e2e <circuit> <blocks> <seed> <dir>`
   and `verify_e2e <circuit> <blocks> <seed> <dir> <ours.*>` examples
   under `tooling/cli/examples/` (the SHA-256 statement restored from
   `d6b637e` inside them), seeded, writing the statement's public bytes,
   the inputs, `meta.txt` (shapes, digests, session/domain), `f`/`h` (or
   enough to regenerate them), the products if we cannot regenerate, the
   Spartan proof bytes, narg, hints. Propose the file format; keep it
   byte-defined.
8. **Effort and risks.** Order the stages below, estimate each, and name
   what could block: upstream moving under the oracle branch (0), the
   circuit drift (3), the single-column query path (4), the 256 MB
   transpose (5), unknown `Encoding` details (1), seeding the blocks (7).

Do not write protocol code in this session beyond throwaway probes needed
to answer the questions (e.g. running our PCS at t = 7, computing digests).

## Step 2 and on — the port, staged like the PCS

Each stage ends with IDENTICAL bytes on dumped vectors, both verifiers
accepting both proofs, unit tests, and a commit (`--no-gpg-sign`; only
`src/bitz`, `examples/bitz_*`, `docs/bitz-*`, manifests; the paper edits
in the tree stay uncommitted).

- **A. Spartan alone**: `Fq<Q100>` + `Encoding`/challenge rules; prepared
  matrices + constraint digest (from dumps or the vendored crate per the
  analysis); outer + inner sumcheck provers and the reusable verifier;
  `SpartanPiopProof` bytes and the terminal claim IDENTICAL on the `abc`
  compression (`spartan/piop/sha256-compression/v1`), then on 8 blocks.
- **B. The virtual path**: `VirtualParams`/`VirtualStatement`, the map
  (`h_len`, `f_len`, `transpose`, digest), `opening_claim`,
  `prove_virtual`/`verify_virtual` framed their way, on top of our existing
  fold/GKR/PCS at the (7, ·) shapes; the whole e2e IDENTICAL (narg, hints,
  Spartan bytes, root) on 1 block, then 8.
- **C. Sizes and the sweep**: 64 and 608 blocks (`sha256-chain`), the
  sweep over seeds like `bitz_parity --sweep`; `BITZ_REPEAT` timing; our
  verifier on theirs and theirs on ours at every size.
- **D. Speed**, byte-identical only: profile (`BITZ_TRACE` phases + the
  new Spartan/transpose phases), then kernels — the `Fq` sumcheck rounds
  (u128 wide arithmetic; consider Montgomery internally), the `M^T`
  transpose (sparse, GF(2^128) XOR gathers), the fold on `h`
  (`fold_columns` at s = 17), the t = 7 GKR levels. Measure warm (repeats)
  and cold (single run) separately, one size per process, A/B against the
  previous binary in one thermal window.
- **E. Comparison**: their CLI vs ours at 1/8/64/608 blocks (prove split,
  verify, bytes, RSS), and against the crate's own SHA-256 SNARK numbers
  (`scripts/run_native_sha256_compare.sh`, the paper's SHA-256 rows) — the
  same table discipline as `docs/bitz-parity-continue-prompt.md`'s
  head-to-head section.

## How to run (carried over from the PCS loop)

```sh
# their side, once per checkout, native flags, their own target dir
cd ~/f2z-benchmark && git checkout bitz-e2e-k4      # the merged oracle + our shape_for commit
CARGO_TARGET_DIR=$HOME/f2z-benchmark/target RUSTFLAGS="-C target-cpu=native" \
  cargo build --release -p tests -p bitz-cli --examples   # dump_bitz/verify_bitz today; dump_e2e/verify_e2e once written
# our side
cd ~/f2z-pcs && git checkout bitz-parity
RUSTFLAGS="-C target-cpu=native" cargo build --release --features bitz-parity --example bitz_parity --example bitz_bench
RUSTFLAGS="-C target-cpu=native" cargo test --release --features bitz-parity,parallel --lib bitz   # 10 tests today
```
`CARGO_TARGET_DIR` is globally `~/zinc-plus/target` (our binaries land
there); dumps live in the session scratchpad and are regenerated every
session; `trace_processor_shell` must be on PATH for the crate's own
`f2z` CLI (`--features unchecked,span-metrics`, `--latex <scratch>` — never
let it overwrite `paper/raw-performance-table.tex`).

## Traps (the PCS ones still apply; new ones)

- Their point convention is little-endian externally and reversed inside
  the GKR; `eq_table` is little-endian; columns occupy the LOW index bits.
- The Gruen message is `[factor·Σ_endpoint, factor·Σ_∞]`; the arena is
  exactly `2^{t−4+s}` entries; `Buckets`/`SumBuckets` are per rayon split;
  pattern blocks are transposed (`col_of`).
- `public_message` ≠ `prover_message`: the Spartan round polynomials,
  the `[Ah, Bh, Ch]` evaluations, every digest and parameter frame are
  public messages — they change the challenges but never appear in narg.
  A byte diff of narg alone cannot pin the PIOP; compare the Spartan proof
  bytes and the terminal claim explicitly.
- `Fq` challenges are rejection-sampled from `u128` squeezes; one skipped
  squeeze shifts every later challenge. `Fq` encodes as 16 LE bytes of the
  canonical residue; `F128` as 16 LE bytes of the two words.
- The e2e's two shapes are derived separately (`h_len` vs `f_len − 1`,
  each padded to a power of two, floor `2^22`) — on our oracle branch
  through `Shape::for_log_bits`; upstream still has `(7, log_bits − 7)`.
  A proof made under one split does not verify under the other (the
  fold's row/column weights and the GKR's rounds follow the shape).
- Their CLI seeds nothing (`rand::rng()`): every dump example must take a
  seed and derive the blocks from it, as `dump_bitz` does.
- The vendored `crates/circuit` has drifted from theirs; digests decide,
  not eyeballing.
- `feat/circuit-e2e` lacks `bitz-k4` (different Ligerito ladder) — the
  merged oracle branch `bitz-e2e-k4` is not optional; it is local and
  unpushed, and upstream keeps moving (`0013ce4` landed the same day).
- Shell: `grep pattern $F` with an empty `$F` hangs the step; macOS has no
  `timeout`; never `git add -A` (the `.claude/worktrees` gitlinks).
- Never `git commit` without `--no-gpg-sign`; paper edits stay
  uncommitted; `docs/` needs `git add -f`.
