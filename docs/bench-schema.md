# Unified benchmark output schema (`schema=f2z/1`)

One accounting model, one printer, one machine-readable line across the
protocol benches. The shared implementation lives in `benches/common/mod.rs`;
the per-step umbrella profiler scopes (`step2:*` … `step5:*`) live in the
crate's protocol prove/verify functions.

**Status: v1, provisional.** The key names below feed the paper's Experiments
tables (`paper/main.tex` §Experiments, `paper/multiswap-table.tex`), so they
are versioned: any rename bumps `schema=` and this file. They have not yet
received explicit user sign-off — settle them before wiring tables to them.

## Timing semantics

**End-to-end prover time (`prove_ms`) INCLUDES** everything the prover does
once it holds a witness: bit-packing, commitment, field projection, prime
sampling + grinding, the PIOP, bitification, Step 5.0, and the F2Z opening.

**`prove_ms` EXCLUDES** witness generation (`witness_ms`) and one-time public
preprocessing — relation/index preparation and Ligerito config derivation
(`setup_ms`). Those are reported separately and labelled one-time.

Conventions kept from the old benches: one untimed warm-up rep, medians over
`reps` measured reps, `black_box` on proofs, and **every measured proof is
verified**.

## Phase taxonomy — paper §2.1 (`s:to_simple_bitz`)

Steps are reported **in code order** (the code runs Step 5.0 *after* the
Step 5.1 lift where it fires, per the paper's own open remark; we do not
reorder to match the prose). A step that does not run in a path is `na`,
never `0.00`.

| key | paper step | contents |
|---|---|---|
| `s1_commit` | 1 Commitment in K | bit-packing + RS encode + Merkle (bench-timed wall clock) |
| `s2_project` | 2 Projection to F | pre-draw grinding + prime sample(s) + relation/witness projection mod q; `na` when the projection is the identity / a fixed legacy modulus |
| `s3_piop` | 3 PIOP over F | Spartan prepare + outer (+ per-round grinding) + bind + inner |
| `s4_bitify` | 4 Bitification | terminal-claim grinding + claim factorization through the bit adjoint + claim absorb |
| `s5_0_reduce` | 5.0 Reduce the prime modulus | exact integer lift μ′ + reduction grinding + fresh prime q′ draw + re-projection; `na` when the condition does not fire |
| `s5_open` | 5.1–5.3 | the F2Z opening: integer claims + exponent-fold GKR forest + ring switch + Ligerito |
| `prove_residual` | — | `prove_ms − s1 − … − s5` (signed, so the split sums exactly) |

Detail keys (optional, `na` when unavailable): `s3_outer_ms`, `s3_bind_ms`,
`s3_inner_ms` (Spartan sub-split), `s5_forest_ms`, `s5_opener_ms`
(Step 5.2 forest+presum vs Step 5.3 ring-switch+Ligerito).

The verifier gets the same split with `v*` keys (no `v1`: the verifier holds
only the commitment) and `verify_residual_ms`.

### Umbrella scope labels (crate-side)

`step2:project_prove|verify`, `step3:piop_prove|verify`,
`step4:bitify_prove|verify`, `step5_0:reduce_prove|verify`,
`step5:open_prove|verify` — thread-local `crate::utils::prof` scopes wrapped
around the existing finer-grained labels, which are unchanged (they keep the
SHA trace writer and older tooling working). The harness sums only the
umbrella labels for the step totals and uses a shared label table for detail.

## The RESULT line

One line per measured configuration, `key=value` separated by single spaces,
no free text. Common keys in fixed order; bench-specific keys sit between
`shape=` and `lambda=`.

```
RESULT schema=f2z/1 bench=<multiswap|sha256|u32_mul|pcs|...> shape=<token>
  [bench-specific keys]
  lambda=<bits|na> lambda_achieved=<bits|na> lambda_bind=<term|na>
  threads=<n> reps=<n> warmups=1 seed=<0x…|na>
  witness_ms= setup_ms=
  prove_ms= s1_commit_ms= s2_project_ms= s3_piop_ms= s4_bitify_ms=
  s5_0_reduce_ms= s5_open_ms= prove_residual_ms=
  s3_outer_ms= s3_bind_ms= s3_inner_ms= s5_forest_ms= s5_opener_ms=
  verify_ms= v2_project_ms= v3_piop_ms= v4_bitify_ms= v5_0_reduce_ms=
  v5_open_ms= verify_residual_ms=
  proof_bytes= proof_piop_bytes= proof_open_bytes= verified_samples=
```

- `lambda` is the security target the run was measured at (100 is the
  default profile; 114 pins the MultiSwap/Limber comparison; `na` for
  paths without a sampled projection prime). `lambda_achieved` is the
  instantiated profile's achieved bits — the minimum over every soundness
  term, GF(2^128) floors included — and `lambda_bind` names the binding
  term (`src/piop/spartan/profile.rs` accounting).
- `proof_piop_bytes` = Spartan payload + nonces + Step-5.0 lift;
  `proof_open_bytes` = the serialized F2Z opening; `proof_bytes` = their sum.
- All `*_ms` values are medians over the measured reps; `prove_residual_ms`
  and `verify_residual_ms` are defined as total-median minus the sum of the
  step medians, so each split sums to its total exactly.

## Environment variables

Canonical names (aliases are honored with a deprecation warning; setting both
an alias and the canonical name to different values is an error):

| canonical | replaces | meaning |
|---|---|---|
| `F2Z_BENCH_REPS` | `F2Z_SHA_REPS`, `F2Z_MULTISWAP_REPS` | measured reps (plus 1 warm-up) |
| `F2Z_BENCH_SHAPES` | `F2Z_MUL_EXPONENTS`, `F2Z_SHA_LOG2S`, `F2Z_BABY_BEAR_MUL_EXPONENTS`, `F2Z_CM_EXPONENTS` | bench-specific shape list |
| `F2Z_BENCH_SEED` | `F2Z_MUL_SEED`, `F2Z_SHA_SEED`, `F2Z_CM_SEED` | root seed |
| `F2Z_BENCH_PASS` | — | `latency|memory|both` |

**Unknown `F2Z_*` variables abort the bench** with the full known-knob list,
so a typo'd knob can never silently do nothing. The registry lives in
`benches/common/mod.rs` (`KNOWN_F2Z_ENV`); add new knobs there.

Protocol benches force `OBLONG_PROFILE=1` at startup so the step split is
always populated (scope overhead is µs-class; the medians carry it). The
`pcs` bench keeps profiling opt-in — its opener phases are µs-scale, so the
headline timings stay scope-free and its forest/opener detail keys are `na`
unless `OBLONG_PROFILE=1` is set.

## Which benches adopt what

- **Full schema** (uniform block + RESULT line + step scopes):
  `multiswap`, `sha256_compressions`, `u32_mul`, `pcs` (PCS-only: steps
  2/3/4/5.0 are `na`).
- **Micro/policy benches — exempt** (own output, strict-env check only):
  `field`, `eq_tables`, `u32_mul_inner_policy`, `u32_mul_outer_skip`,
  `cm_and`, `baby_bear_mul` (its RESULT keys are pinned by
  `scripts/baby_bear_mul_bench_report.py`; port it together with that script).
- `examples/reference_measure.rs`, `src/bin/f2z.rs`: unchanged output,
  documented here as exempt.
