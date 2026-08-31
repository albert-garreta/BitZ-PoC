# Session prompt: unify the benchmark harness + make the IOP security profile configurable

Load the `f2z-pcs` skill. Work in `/Users/albertgarretafontelles/f2z-pcs`
(branch `master`; commits are unsigned, `git commit --no-gpg-sign`). Run
`git status` first — there is in-flight work in `.claude/worktrees/sha-map-opt`
and `.claude/worktrees/virt-gap`, and an untracked `paper/multiswap-table.tex`.

There are two coupled workstreams. Do **A** first (it defines the vocabulary
that B has to be measurable in), but design B's config type before finalizing
A's phase list, so the two agree.

---

## A. One uniform benchmark output across all protocol benches

### A.1 Current state — three benches, three incompatible accounting models

| bench | shape knob | setup lines | what `prove` excludes | phase split |
|---|---|---|---|---|
| `benches/u32_mul.rs` (857 L) | `F2Z_MUL_EXPONENTS`, `F2Z_BENCH_REPS`, `F2Z_BENCH_PASS`, `F2Z_SPARTAN_REDUCTION`, `F2Z_SPARTAN_OUTER_SKIP`, `F2Z_MUL_WORD_BITS`, `F2Z_BENCH_ORDER`, `F2Z_MUL_SEED` | **5**: witness / relation / projection / bit-pack / commitment (`:514–545`, printed `:735–738`) | witness gen, relation prep, bit-pack, commit, and a hoisted field projection that the default `delayed-barrett` path never performs | `prove split: Spartan | bitify | F2Z PCS | residual` (needs `OBLONG_PROFILE=1`) |
| `benches/sha256_compressions.rs` (993 L) | `F2Z_SHA_LOG2S`, `F2Z_SHA_REPS`, `F2Z_SHA_SEED`, `F2Z_SHA_TRACE_PATH`, `OBLONG_PROFILE[_INTERVALS]` | **1**: `setup` inline on the R1CS line (`:826–842`) | nothing — witness gen **and** commit are inside the headline `end-to-end prove` (`run_once`, `:703–753`) | `proof internals: outer Spartan | claim factorization | virtual F2Z` |
| `benches/multiswap.rs` (211 L) | none — hardcoded `MultiswapDims::multiswap(0)`; reads only `F2Z_MULTISWAP_REPS`; force-sets `OBLONG_PROFILE=1` at `:58` | **2**: `witness+shape build` / `relation setup`, both labelled one-time | witness gen + relation prep; bit-rows and commit are timed per-rep but reported *outside* `prove` | `prove phases: primes | projection | spartan | bitify | step5.0 lift+grind | f2z opening` — closest to what we want |

Other output surfaces to keep consistent (or explicitly exempt):
`benches/pcs.rs` (PCS-only), `benches/cm_and.rs`, `benches/u32_mul_inner_policy.rs`,
`benches/u32_mul_outer_skip.rs` (policy sweeps), `benches/field.rs`,
`benches/eq_tables.rs` (micro-kernels), `examples/reference_measure.rs`,
`src/bin/f2z.rs`.

### A.2 Required timing semantics (the user's decision — implement it, don't relitigate)

**End-to-end prover time INCLUDES**: bit-packing, commitment, field projection,
prime sampling + grinding, the PIOP, bitification, Step 5.0, and the F2Z
opening. Everything the prover does after it holds a witness.

**End-to-end prover time EXCLUDES**: witness generation, and one-time public
preprocessing (relation/index preparation, Ligerito config derivation). Report
those separately, clearly labelled one-time, and make sure any *untimed* work
is either attributed or removed — e.g. `multiswap.rs:66` currently runs a full
`circuit.is_sat_integer()` between the two setup timers, attributed to nothing.

Every bench must also print a per-step **prover** breakdown and a matching
per-step **verifier** breakdown, summing to their respective totals with an
explicit `residual` line so the split is auditable.

### A.3 The phase taxonomy: paper §2.1

`paper/main.tex:305–427` (`\subsection{A simple version of \ftwoz}`,
`\label{s:to_simple_bitz}`) is the spec. Read it (read-only — the paper carries
a "written without AI assistance" statement; do not edit `paper/` in this
session). Its steps are the canonical phase names, and every bench must report
in these terms:

| step | paper name | notes |
|---|---|---|
| 1 | Commitment in `K = GF(2^128)` | bit-packing + RS encode + Merkle. In-scope for prove. |
| 2 | Projection to `F` | reduction mod a random prime for `R = Z`; identity for `R = K`. May be a no-op → report `n/a`, not `0.00`. |
| 3 | PIOP over `F` | Spartan outer/bind/inner. Sub-split allowed (keep the existing outer/bind/inner). |
| 4 | Bitification of the linear claim | `u = pi(L)^T v`, tensor-preserving. |
| 5.0 | Reduce the prime modulus *if necessary* | triggers iff `|F| >= (|K|-1)/(2*n_1)`; **always** when `F_ext` is a proper extension. Lift to exact integer `mu'` + fresh grinded prime `q'`. |
| 5.1 | Integer claims `eta_j` | low-magnitude integer claims per column. |
| 5.2 | Exponentiation + batched grand product in `K` | the forest GKR — the dominant cost. |
| 5.3 | Linear claims over `K` | ring switch + Ligerito. |

Note the paper's own open item at the Step-5.0 remark: *"In the actual protocol
this step is done after Step 5.1. I think they are equivalent but should
unify."* The code follows the after-5.1 order. Report phases in **code order**
and say so, rather than silently reordering to match the prose.

Existing `crate::utils::prof::scope` labels are already close to this
(`multiswap:relation_projection_prove`, `multiswap:integer_lift_prove`,
`sha256-paper128:opening_prepare_prover`, `spartan-f2z:bitify_prover`, …).
Prefer **renaming/aliasing the scopes to a shared step vocabulary** over
per-bench string matching in the benches. Scopes are thread-local and must stay
on the control-flow thread (`src/utils/prof.rs:24–27`).

### A.4 Deliverable

A shared bench-harness module (e.g. `benches/common/mod.rs` or a
`#[cfg(feature = "bench-internals")]` module in the crate) providing:

- one `Timings`/`PhaseBreakdown` type keyed by the §2.1 steps;
- one printer producing an identical human block and an identical
  machine-readable `RESULT ...` key=value line across benches (the `RESULT`
  line is what feeds the paper's Experiments tables — `paper/main.tex:2526`,
  `paper/multiswap-table.tex` — so settle the key names once);
- uniform env-var conventions. Today `F2Z_BENCH_REPS`, `F2Z_SHA_REPS` and
  `F2Z_MULTISWAP_REPS` are three names for one concept, and `multiswap` has no
  shape knob at all. Pick one scheme (suggest `F2Z_BENCH_REPS`,
  `F2Z_BENCH_SHAPES`, `F2Z_BENCH_SEED`, `F2Z_BENCH_PASS`), keep the old names
  as deprecated aliases, and **fail loudly on an unknown `F2Z_*` variable** so
  a typo'd knob can never silently do nothing again.
- Decide and state: which benches adopt the full schema (the three protocol
  benches, `pcs.rs`) versus which stay micro-benchmarks with only the shared
  `RESULT` convention (`field.rs`, `eq_tables.rs`, the two policy sweeps).

Keep: warm-up rep excluded, medians over reps, `black_box` on proofs, every
measured proof verified.

---

## B. A configurable IOP security profile

### B.1 Current state — three hardcoded, mutually inconsistent schemes

- **u32_mul path**: no prime sampling at all. `FQ_MOD = 2^100 - 15`,
  `FQ_BITS = 100`, fixed (`src/pcs.rs:345–347`), fed through
  `spartan_f2z_field_config()` (`src/piop/spartan/f2z.rs:180`). No grinding.
- **SHA path**: `Sha256PrimeProfile` (`src/piop/spartan/sha256/prime.rs`) —
  ONE prime, interval derived from batch size
  (`113.min(128 - log_compressions)` bits, capped by the no-wrap inequality
  `(2^t + 1)(q-1) <= 2^128 - 1`), with grinding bits as hardcoded lookup
  tables: `initial 20|21|22`, `outer 18|19`, `terminal = outer` (`:82–103`).
  Target security is a comment, not a parameter.
- **MultiSwap path**: `MultiswapPrimeProfile`
  (`src/piop/spartan/multiswap/prime.rs`) — TWO primes ("Strategy 2"):
  fingerprint `Q in [2^127, 2^128)` un-grinded, Step-5.0 reduction
  `q' in [2^112, 2^113)` with a const `reduction_grinding_bits() = 10`.
  Const intervals, const grind, 114-bit floors argued in a doc comment.
- **Ligerito layer**: `sha_paper128_lig_configs` hardcodes
  `custom_udr_grind_config_bits(m, 1, 4, Some(128))`
  (`src/ligerito_flock.rs:431`); `sha_lig_configs` picks the embedded audited
  FAST profile only at `m = m_p + 7 >= 22`.
- **Grinding machinery already exists and is good**: typed
  `GrindingDomain` / `GrindingRound<D>` / `grind_and_absorb` /
  `verify_and_absorb` (`src/piop/spartan/grinding.rs`), with
  transcript-stable parallel search. Prime sampling: `sample_prime_in_interval`
  (`src/ext_proj.rs:262`), rejects intervals reaching `2^126`.

### B.2 What to build

One shared profile type (name it, e.g. `IopSecurityProfile`) that supersedes
`Sha256PrimeProfile` and `MultiswapPrimeProfile`, exposing:

1. **Target security bits** `lambda` as an explicit input (today: implicitly
   114 for MultiSwap, ~128 elsewhere).
2. **Sampleable prime intervals per role** — the Step-2 projection/fingerprint
   prime and the Step-5.0 reduction prime — configurable, with validation
   against the hard geometric constraints:
   - `sample_prime_in_interval` refuses `max >= 2^126`;
   - the exponent-fold chunk width `c_w = 127 - t - W`
     (`src/pcs.rs:1055`) and `L = ceil(q_bits / c_w)` chunks;
   - the no-wrap lift bound `(2^t + 1)(q - 1) <= 2^128 - 1`.
3. **Per-round grinding bits** — initial, per-outer-round, terminal,
   Step-5.0 reduction, plus the Ligerito query/fold grinding — as configurable
   fields, not `match` tables.
4. **Derivation, not tabulation**: given `lambda` plus the instance facts
   (defect bit-bound, `d`, `t`, `W`, round count, degree), *compute* the
   required interval sizes and grinding bits, and expose the resulting
   per-step soundness-error accounting programmatically (the argument that
   today lives only in the `prime.rs` doc comments). A profile must be able to
   report "this configuration achieves >= lambda bits, here is the binding
   term" — and refuse to build otherwise.
5. **Step 5.0 fires on its condition, not on which module you are in**:
   trigger iff `|F| >= (|K| - 1) / (2 * n_1)` (paper's
   `\fieldboundsizeexponent`), and always when `F_ext` is a proper extension.
   With `L = 1` mandatory (see B.5), Step 5.0 is the *only* mechanism for an
   oversized `F` — there is no policy choice left to expose.
6. Wire all three protocol paths (u32_mul, SHA-256, MultiSwap) onto it, with
   the current hardcoded values reproduced exactly as named default profiles
   so today's numbers stay reproducible.

### B.3 Decisions (settled by the user — implement these, do not re-ask)

1. **The u32_mul path gains a real sampled Step-2 prime.** It has none today
   (fixed `FQ_MOD = 2^100 - 15`, `src/pcs.rs:345`). Give it a transcript-derived
   prime drawn after the F2Z commitment is bound (the Zaratan order already used
   by `Sha256PrimeProfile` and `MultiswapPrimeProfile`). The fixed modulus may
   survive only as an explicitly named legacy profile for reproducing old
   numbers — not as the default.

2. **The profile is compile-time**, not a runtime value.

3. **Ligerito is configured to attain the same `lambda`** as the rest of the
   protocol — one security target for the whole system, no independently pinned
   PCS level.

4. **`lambda` is selectable between 100 and 128**, with `100` the default.
   The MultiSwap / Limber comparison pins `114`. Ship three named profiles:
   `Lambda100` (default), `Lambda128`, `Limber114` (MultiSwap only). Reaching a
   *genuine* 128 requires new grinding in the forest — see B.6; today's
   configuration does not have it. Optionally keep a `Legacy` profile that
   reproduces today's exact parameters so existing numbers stay checkable, but
   do not call it 128.

### B.4 Consequences of those three decisions

**On (2) — what "compile-time" can and cannot cover.** Split the profile in two
layers; the type is compile-time, the shape instantiation is not:

- *Compile-time policy* (const generics / associated consts on a
  zero-sized profile type): `lambda`, per-round grinding bits or the rule that
  derives them, prime-interval **widths in bits**, the number of primes
  (one-prime vs the two-prime Strategy 2), and the oversized-`F` policy
  (weight chunking `L > 1` vs Step-5.0 re-projection).
- *Runtime instantiation against a shape*: the concrete interval endpoints and
  the validated parameter set, computed from the policy plus `t`, `W`, `n_1`,
  the defect bit-bound and the round count. This must stay runtime because the
  bench shape is an env input (`F2Z_SHA_LOG2S`, `F2Z_BENCH_SHAPES`) and
  `Sha256PrimeProfile::new(log_compressions)` already derives
  `113.min(128 - log_compressions)` and the no-wrap cap from it. Making the
  shape const-generic too would force an enumerated shape set and kill the
  shape env knobs — do not do that.
- The sampled prime `q` itself is necessarily runtime (transcript-derived), and
  `SpartanF2zField = F128 = MontyField<2>` carries a runtime
  `FixedMontyParams` built by `make_cfg`. Do **not** try to lift the modulus
  into a const-generic `ConstMontyForm`; only the *policy* is const.
- Benches sweep `lambda` by instantiating several named profile types in one
  binary (a generic bench body over the profile type), not by an env var.

**On (3) — wiring `lambda` into Ligerito.** `custom_udr_grind_config_bits(m,
r0, k0, target_bits)` (`src/ligerito_flock.rs:689`) already takes the target as
a parameter and rebuilds queries/grinding from it, so the wiring itself is
small: replace the hardcoded `Some(128)` at `:431`. Two hard constraints:

- **The GF(2^128) floor binds.** That function's own honest-scope note: 128
  means every term flock *tracks* clears `2^-128`; untracked field-limited
  rounds (each degree-`d` sumcheck message, error `~ d/2^128`) sit at ~126–127
  bits. So `lambda` above ~126 is unreachable over `K = GF(2^128)` at any
  parameter setting. The profile must reject or loudly cap such a `lambda`
  rather than silently reporting it as achieved.
- **Leaving `lambda = 128` leaves the audited regime.** `sha_lig_configs` /
  `sha_paper128_lig_configs` select embedded, audited profiles; a custom target
  routes through `custom_udr_grind_config_bits` instead. That is legitimate,
  but the code must say so explicitly (a named "unaudited custom target" state),
  and must never let a non-default `lambda` fall through into the UNAUDITED
  ad-hoc small-shape config path — see the trap below.

**On (4) — 100 is the near-zero-grinding regime, 128 is the expensive one.**
The paper already identifies both, so neither is improvised in code:

- `paper/main.tex:1610` — "Achieving `lambda = 100` bits of security is
  relatively straightforward and we do not discuss it"; the grinding machinery
  exists to reach 128.
- `:1657` — the GKR grand-product rounds have `eps <= 3/|K|`, which
  "immediately guarantees `lambda = 100`"; two bits of grinding **per round**
  are needed only for 128.
- `:1654` — WHIR/Ligerito "is configured at any desired `lambda <= 128`"; the
  ring-switch round needs 1 bit of grinding only at 128.
- `:744` — `q <= 2^128` is feasible at `lambda = 100` without PIOP grinding.

Expect the 100-vs-128 gap to be the single largest performance axis in the
whole task. The SHA path today grinds `2^20`–`2^22` before the initial draw
plus `2^18`–`2^19` before **every** outer-sumcheck round plus a terminal grind
(`src/piop/spartan/sha256/prime.rs:82–103`); at `lambda = 100` all of that
falls to zero, since `3/q <= 3/2^112 = 2^-110.4` already clears 100 un-grinded.
Ligerito also drops queries at a lower target, so proofs shrink too.
**Consequence for the measurement plan: do not conflate this with the harness
re-accounting.** Land A, re-baseline, and only then make the target selectable,
so the grinding delta is attributed to the right change.

Where the 128 cost actually sits, so it can be optimized rather than merely
paid: the expensive grinds are the ones compensating the *small PIOP field*
(`q` is only 112–113 bits because `c_w = 127 - t - W` caps it), at `2^18`–`2^22`
hashes each. The forest grinding that B.6 adds is by contrast almost free in
time — 2 bits is 4 hashes — and costs proof bytes instead.

The profile must report the achieved `lambda` as the **minimum over every
term** (fingerprint draw, PIOP rounds, Step-5.0 draw, GKR rounds, Ligerito)
together with which term binds. At 100 the `GF(2^128)` floor (~126) never
binds; the prime draws and Ligerito will.

Keep MultiSwap at 114 because that is Limber's own floor —
`LAMBDA_BOUND2 = 117`, fingerprint `~2^-114`, as documented in
`src/piop/spartan/multiswap/prime.rs:19–47`. Publishing an F2Z-at-100 number
against Limber-at-114 would be an unfair comparison and a reviewer would say
so. The MultiSwap bench must pin `Limber114`, and the unified `RESULT` line
must carry the `lambda` it was measured at so the table can state it.

**Paper consistency — flag to the user, do not edit `paper/`.** The abstract
and intro claim 128 bits (`main.tex:143`, `:205`, `:402`) and §Instantiation
(`:1610`) is written around 128. If the reported experiments move to
`lambda = 100`, the Experiments tables must say so explicitly and the headline
claim needs reconciling before submission. Raise this once the numbers exist;
it is the user's call, not a code change.

**On (1) — u32_mul's new Step 2 changes its numbers.** It currently pays no
prime sampling, no grinding, and (under the default `delayed-barrett`) no bulk
field projection. Adding a sampled prime puts Step 2 on the critical path and
makes the `F` modulus vary run to run, so `L = ceil(q_bits / c_w)` can change
with the draw. Fix the interval so `L` is stable, and re-baseline the bench.

### B.5 Hard requirement: `L = 1` always, hardcoded

**The weight-chunking path must be deleted, not merely defaulted off.** The
opener currently supports `L = ceil(q_bits / c_w) > 1` with
`c_w = 127 - t - W` (`src/pcs.rs:1055–1067`), carrying per-chunk folds and the
recombination `y = sum_c w'_c sum_l 2^{c_w*l} u_c^{(l)}`. All of it goes:
`mod_q_num_chunks`, `mod_q_chunk_width`, `ModQWeightChunks`, the per-chunk fold
loops, the chunked branches of the prover/verifier, and every `chunks=` field
in bench output and CLI plumbing.

Removal size, to scope the work: ~121 references across 8 files —
`src/ligerito_flock.rs` (87, the bulk), `src/pcs.rs` (17),
`src/piop/spartan/f2z.rs` (7), `src/bin/f2z.rs`, `benches/u32_mul.rs`,
`benches/pcs.rs`, `examples/reference_measure.rs`,
`examples/proof_digest.rs` (2 each).

What replaces it: `q_bits <= c_w = 127 - t - W` becomes a **hard
profile-construction invariant**, checked once and returning an error rather
than silently chunking. Consequences the profile must handle:

- The interval's upper endpoint is capped at `2^(127 - t - W)`, so the usable
  prime *shrinks as the shape grows*. At `W = 1`: `t = 13` gives `c_w = 113`
  (exactly MultiSwap's `q' in [2^112, 2^113)` — chosen for this reason);
  `t = 24` gives `c_w = 102`.
- That cap propagates into the security budget. A degree-3 PIOP round costs
  `3/q`, so `lambda = 100` un-grinded needs `q >= 2^101.6`, i.e.
  `c_w >= 102`, i.e. `t + W <= 25`. Past that the profile must either fire
  Step 5.0 or spend grinding bits — and must say which, not guess.
- Verify no currently benchmarked shape silently relied on `L > 1`. Check the
  `chunks=` values printed by `u32_mul` and `pcs` today before deleting, and
  state in the commit which shapes (if any) become unreachable without
  Step 5.0.

Deleting a protocol branch changes proof bytes and the codec surface; sequence
this with the transcript-changing work rather than as a separate "cleanup".

### B.6 New work: grinding the forest / GKR rounds (required for a genuine 128)

There is **no grinding anywhere in the forest today** — `grep -n grind
src/merged_forest.rs src/pcs.rs` returns nothing. Each GKR grand-product round
is a degree-3 sumcheck over `K = GF(2^128)`, so its error is
`3/2^128 = 2^-126.4`, and the ring-switch round adds `1/|K|`. That is the true
floor of the current SHA-256 configuration: every *grinded* term is tuned to
`2^-128.4` (outer rounds: `3/2^112 = 2^-110.4` plus 18 bits; at `log2 = 16`,
`3/2^111 = 2^-109.4` plus 19 — the exact fit shows 128 was the design target),
but end to end the system sits at **~126.4 bits**, not 128.

Per the paper this is closable:

- `main.tex:1657` — `eps_red <= 3/|K|` per GKR round "immediately guarantees
  `lambda = 100`. To achieve `lambda = 128`, one can do two bits of grinding
  per round."
- `main.tex:1654` — the ring-switch round's `1/|K|` term takes 1 bit of
  grinding at 128.

So implement it: a new `GrindingDomain` for the forest, with
`grind_and_absorb` / `verify_and_absorb` hooks at each GKR sumcheck round
boundary and at the ring-switch round, difficulty supplied by the profile
(0 bits at `lambda = 100`, 2 bits at 128). The existing typed machinery in
`src/piop/spartan/grinding.rs` is transcript-stable under `parallel` and is the
right thing to reuse.

Two things to get right:

- **Cost is bytes, not time.** 2 bits is 4 hashes, so prover time is
  unaffected. But every grinded boundary absorbs an 8-byte nonce, and
  `MergedForestProof` carries `depth` layers each with one or two
  `SumcheckProof<Gf>` whose round counts grow with the layer — order `10^2`
  rounds at typical shapes. Count the exact number of grinded boundaries for
  the shapes in the tables and report the proof-size delta; do not assert "free"
  before measuring it.
- **Byte-identity pins.** `lazy_matches_eager` / `lazy_multi_matches_eager` in
  `src/merged_forest.rs` compare prover paths; adding transcript writes must
  keep both paths in agreement. At `lambda = 100` (0 difficulty) the transcript
  must be **bit-identical to today's**, so the default profile changes no forest
  bytes at all. Make that a test.

---

## Constraints and traps

- **Every change here is transcript-changing.** Proof bytes move; the codec
  (`src/proof_codec.rs`) is canonical and tamper-rejecting; update its
  round-trip tests deliberately, not reflexively.
- **Do not break the byte-identity pins** `lazy_matches_eager` /
  `lazy_multi_matches_eager` in `src/merged_forest.rs` — they gate the forest.
- **The audited-config boundary is load-bearing**: `sha_lig_configs(m_p)` uses
  the embedded FAST profile only at `m = m_p + 7 >= 22`; the ad-hoc regime
  below is UNAUDITED and test-only. Never hardcode the ad-hoc config at big
  shapes (n=28 commit measured 292 s ad-hoc vs tens of ms embedded).
- **Build parity**: benches need `RUSTFLAGS="-C target-cpu=native"`,
  `--features unchecked`, and the release profile's `lto = true` +
  `codegen-units = 1`. Without all of them F2Z measures 1.2–1.5x slow.
- Baseline test suite is ~289 lib tests; `completeness_random` x2 is known
  pre-broken at HEAD. Watch the `QUAD_ENV_LOCK` env-toggle test race.
- Benchmarking protocol: idle-first, one shape per process at n >= 24, medians,
  ±5–15% band. Interleave A/B runs in one thermal window.
- Measure a before/after on the three protocol benches at fixed shapes and
  confirm the re-accounting explains every delta — a *reported* number will
  move (commit and bit-pack join prove); a *real* cost must not.

## Suggested order

1. Read paper §2.1 and fix the step vocabulary; get user sign-off on the
   `RESULT` key names (they feed the paper tables).
2. Land the shared bench-harness module + printer; port `multiswap` first (it
   is closest), then `sha256_compressions`, then `u32_mul`.
3. Verify the re-accounting on all three; record the new baselines.
4. Design `IopSecurityProfile` per the B.3 decisions and the B.4 layer split
   (compile-time policy + runtime shape instantiation).
5. Port MultiSwap and SHA onto it with byte-identical default profiles
   (transcript unchanged when defaults are selected — verify this explicitly),
   then u32_mul, whose new sampled Step-2 prime is a deliberate transcript
   change and needs a fresh baseline.
6. Switch the default target to `Lambda100` (MultiSwap stays on `Limber114`)
   and re-baseline again — separately from step 3, so the grinding delta is
   attributed to the profile change and not to the harness.
7. Add the forest/ring-switch grinding hooks (B.6) so `Lambda128` is a genuine
   128, with a test pinning the `lambda = 100` transcript bit-identical to
   today's.
8. Add sweep coverage: one bench binary instantiating `Lambda100`, `Limber114`
   and `Lambda128`, showing the prover-time / proof-size tradeoff in the
   unified format — this is the table that shows what 128 actually costs.
