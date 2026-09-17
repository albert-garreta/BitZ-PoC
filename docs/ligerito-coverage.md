# Ligerito policy and benchmark coverage

The default for the Ligerito component of production 100-bit F2Z proofs is
`custom:1:4`: Johnson, RS rate 1/2, initial folding exponent 4, with early
Round-0 OOD. Select `udrg:1:4` for the matched-geometry unique-decoding-radius
(UDR) configuration, with native fold grinding derived by Ligerito.

This selector configures **only Ligerito**. Binius64 BaseFold/FRI,
Plonky3-FRI, Plonky3-WHIR, and Limber-Brakedown retain their own policies.
In hybrid mode it configures the shared opener, not the Binius SHA PIOP.
The separate mode retains native Binius security and selects the F2Z opener.

`src/ligerito_flock/configuration.rs` is the checked resolver. Preparation
retains its resolved configuration; commitment, proving, verification, and
reporting use that configuration. Library callers pass `LigeritoSelection`.
Benchmarks read `F2Z_LIG_PROFILE`; applicable `--profile` options override it.
An absent selector chooses Johnson at target 100. Explicit historical
`udr:r:k` (queries only), `udrg:r:k` (fold grinding), and `custom:r:k`
geometries remain checked selections. Unknown profiles, conflicting explicit
`:bits` suffixes, and unsupported shapes fail instead of becoming `adhoc`.
Higher-security default profiles retain their previous UDR policy and
existing grinding-cap rejections; selecting a higher target does not make
every 100-bit shape eligible.

## Entrypoint matrix

`L` denotes the independent multiplication/gate exponent; `K` denotes the
SHA compression count exponent; `m` denotes committed-bit exponent. The
shape preflights do not allocate large witnesses or generate proofs.
The library maxima below describe configuration eligibility, not a memory
capacity guarantee. Comparison backends and runners may impose tighter limits.

| Workload / entrypoints | Preparation and policy | Shape preflight / proof validation |
|---|---|---|
| Full u32 product; mod32 relation; `mul_f2z proof`, `mul_compare proof`, `f2z --mul` | `PreparedRelation::<MulLayout<u32>>::new_with_profile_and_ligerito`; Lambda100 defaults Johnson | `coverage::multiplication_shapes_preflight_without_witnesses`: L=15..28, W=1 and W=8; u32 proof/tamper tests, both regimes |
| u64 / u128 full products; `mul_compare proof` | `PreparedRelation<MulLayout<u64>>` / `PreparedRelation<MulLayout<u128>>`; same resolver | u64 L=15..27, u128 L=15..26; native relation roundtrips and malformed-witness tests |
| BabyBear multiplication; `mul_f2z proof --workload baby-bear` | `PreparedRelation<BabyBearMulLayout>`; target inherited from Lambda100 or Lambda128 | L=15..28; paper-path proof tests and fixed-modulus compatibility tests |
| Terminal u32 / BabyBear PCS comparisons; `mul_compare pcs` | Prepared terminal opener retains resolved Ligerito policy; fixed-q BabyBear exposes explicit `_with_ligerito` APIs | Terminal and combined adapter roundtrips; same committed-source shape eligibility |
| CM-AND; `cm_and` | `PreparedCmAndRelation::with_ligerito`; 100-bit Ligerito policy | L=15..28 configuration preflight; both-regime `ligerito_protocols` proof/codec checks. Tiny algebra fixtures have no production security claim |
| SHA compression; `sha256_compressions`, `sha256_e2e_compare` F2Z arm | Prepared SHA compression policy, target inherited from enclosing profile | K=7..16; explicit inner-sumcheck, assignment-row-sized, and product layouts; proof/public-output tests |
| SHA chain; `sha256_chain` | Prepared SHA chain policy | K=7..16; both-regime chain proof and public-statement tampering tests |
| SHA layout sweep; `sha256_product_layout` | Same prepared SHA policy; explicitly labelled fixed-98 projection experiment | t=7..28 shape preflight and existing layout/forest-count tests; this is not a new complete-protocol 100-bit claim |
| `lambda_sweep` | Lambda100 arm defaults Johnson; higher-target arms preserve their UDR defaults | SHA profile preflights and higher-security roundtrips |
| Raw PCS; `pcs`, `f2z` / `f2z --sweep` | Resolver and early OOD before prime/point sampling; extension wrapper also binds OOD before projection | m=20..35 resolver tests; mod-q, extension, virtual proof/codec tests |
| SHA+ECDSA; `sha256_ecdsa`, `sha256_ecdsa_compare` F2Z arms | Prepared composed opener shared by execution/security derivation; split and all-rows outer modes | K=3..16 target preflights; both outer modes and both regimes, plus 128-bit roundtrip |
| Hybrid; `hybrid_u32_sha256`, `hybrid-u32-sha256` | Shared opener target 106; literal rate 1/2, 16 virtual lanes; both regimes enforce padding | Mul L=9..22 (prefix-only multiplication relation; the standalone 2^15 floor applies to `separate` mode), SHA K=1..16 geometry preflight; balanced `15:7` and equal-count `9:9` roundtrips, proof/codec tampering, and recommitted nonzero-padding rejection |
| Separate F2Z+Binius mode | F2Z component target 112, default Johnson; Binius configuration unchanged | Same u32 preparation and native Binius setup; selector isolation checks |
| All-Binius mode | No Ligerito resolver | Ligerito selector does not configure the Binius backend |
| `ligerito_bounds` | Dedicated Johnson/UDR comparison within F2Z | Compiled only in this pass; paired timing and RSS measurements are deferred |

SHA's algebraic prime-profile minimum remains K=4. Production Ligerito
requires m>=20, making K=7 the current compression/chain proof minimum.
K=4..6 is rejected at production preparation; extending it is separate work.
Hybrid's Binius SHA arithmetization supports its own smaller sizes.

## Historical and arithmetic-only entrypoints

- MultiSwap keeps its native Limber114 profile and rate-1/8 UDR opener; this
  change does not retarget that higher-security protocol.
- `proof_digest`, `prof_probe`, `reference_measure`, `rlc_ab`, `quad_ab`,
  `fuse_check`, and `taps_ab` explicitly use `historical_sha_lig_configs`.
  Their low-level/kernel results are historical experiments with no new
  production security claim. CLI family/tap experiments require explicit UDR.
- `field`, `eq_tables`, `u32_mul_inner_policy`, `mul_f2z piop`, and
  `mul_compare witness` measure arithmetic, witness generation, or PIOP
  kernels. They do not prove a complete statement with a security claim.
- `gen_lig_configs` generates checked research configurations; it is not a proof benchmark.
- `scripts/mul_report.py` reads only current multiplication campaigns; historical
  artifacts remain unchanged.

## Transcript and result identity

The changed proof paths bind statement, roots, configuration and security
parameters, then execute Johnson Round 0 before projection/PIOP challenges.
An opaque claim is consumed at the terminal opening without another Round-0
draw. UDR omits outer and recursive OOD payloads. Hybrid authenticates its
virtual source and adds the fresh padding check after ring-switch messages.

The policy identity is `f2z/ligerito-policy/early-ood/v1`. Mod-q, extension,
and virtual opening codecs have version-2 headers; SHA+ECDSA uses
`F2ZSE002`; hybrid uses codec version 5. Old late-OOD proofs are not new-protocol proofs.

`LIGERITO_CONFIG <JSON>` reports requested/resolved profile, regime, target,
full validated configuration (geometry, query/fold grinding and recursive
OOD), outer OOD presence/raw bits/grinding, protocol version, and a BLAKE3
configuration fingerprint. Keep the enclosing protocol target and its
composition/accounting report separate from this native opener target.
Raw statistical error is not discounted by economic grinding.

Common production records use `schema=f2z/2` and `ligerito_hex`: UTF-8 JSON
encoded as hexadecimal so whitespace within strings cannot corrupt the
key=value record. CLI records use `f2z-cli/2` and `f2z-cli-mul/2` with the
same field. SHA+ECDSA records use `f2z/sha256-ecdsa/v2` and nested JSON.
Hybrid prints metadata to stderr, preserving its CSV stream, and saves
`PROOF.ligerito.json` with a requested proof artifact. A saved UDR proof must
be verified with the matching `--profile udrg:3:4` selection.

Readers reject missing/historical identities. Multiplication tables derive
the caption from recorded Ligerito policy and reject mixed Johnson/UDR
policies within one F2Z series. The controlled paired experiment deliberately
uses separate regime series; it is a **Ligerito-bound comparison within F2Z**.

## Reproducible commands

These examples run benchmarks only when explicitly invoked; no performance
campaign was run for this implementation pass.

```sh
# Default Johnson+OOD, or matched UDR for the same workload.
RUSTFLAGS='-C target-cpu=native' RAYON_NUM_THREADS=8 F2Z_BENCH_SHAPES=15 \
  cargo +1.98.1 bench --bench u32_mul
F2Z_LIG_PROFILE=udrg:3:4 RUSTFLAGS='-C target-cpu=native' RAYON_NUM_THREADS=8 \
  F2Z_BENCH_SHAPES=15 cargo +1.98.1 bench --bench u32_mul

# CLI selection overrides the environment; selector affects only the shared opener.
cargo +1.98.1 run --release --features hybrid --bin hybrid-u32-sha256 -- \
  --mode hybrid --mul-log 15 --sha-log 2 --profile udrg:3:4 --iterations 5

# Configuration/shape and proof tests, not performance measurements.
RUSTFLAGS='-C target-cpu=native' RAYON_NUM_THREADS=8 \
  cargo +1.98.1 test --release --lib --features ecdsa,hybrid,bench-internals coverage::
RUSTFLAGS='-C target-cpu=native' RAYON_NUM_THREADS=8 \
  cargo +1.98.1 test --release --test native_mul_compare \
  --features bench-internals,native-mul-compare ligerito_isolation_tests
```

## Validation record (2026-09-10)

Validation used Rust 1.97.1 on the integrated mod32 worktree based on
`72f4fe7ae7f1`, including the uncommitted policy/reporting changes, with native
CPU flags and eight Rayon threads for proof tests. No benchmark campaign
was executed. The final source was checked with all CLI, benchmark and test
targets enabled for `ecdsa`, `hybrid`, `bench-internals`,
`native-mul-compare`, `native-sha256-compare`, and `sha256-ecdsa-compare`.

Correctness coverage includes resolver and shape boundaries; early-OOD
order/payload checks; mod-q and extension codecs; u32/u64/u128/BabyBear
proofs; CM-AND; SHA compression and chain; SHA+ECDSA outer modes; and
balanced/padded hybrid proofs. The normally ignored u32 W=8, BabyBear terminal and
combined-adapter correctness tests were run explicitly. Reader tests cover
identity rejection, resume compatibility, Johnson/UDR captions and runner
normalization. Configuration-only child-process probes compare actual
Binius64, Plonky3-FRI, Plonky3-WHIR and Limber metadata under both selectors.

Final results: 91 selected library tests, five production integration tests,
four backend-isolation harness tests, and 22 Python tests passed. The
all-target feature check and `git diff --check` passed. These counts include
the explicitly enabled correctness tests above; they do not imply a full
workspace test run or a performance measurement.

Additional reproducible correctness commands:

```sh
RUSTFLAGS='-C target-cpu=native' RAYON_NUM_THREADS=8 \
  cargo +1.98.1 test --release --test ligerito_protocols \
  --features ecdsa,hybrid,bench-internals -- --test-threads=1
RUSTFLAGS='-C target-cpu=native' RAYON_NUM_THREADS=8 \
  cargo +1.98.1 test --release --test native_sha256_compare \
  --features bench-internals,native-sha256-compare ligerito_isolation_tests
python3 -m unittest discover -s scripts -p test_ligerito_results.py
python3 -m unittest discover -s scripts -p test_sha256_ecdsa_compare.py
python3 -m unittest discover -s scripts -p test_native_mul_runner.py
python3 -m unittest discover -s scripts -p test_native_sha256_runner.py
```

Deferred: paired Johnson/UDR timing and fresh-process RSS runs, published
sweep regeneration, smaller SHA production configurations, wider-protocol
security changes, Spartan2 comparisons, and a uniform complete-protocol
100-bit derivation. No paired performance claim follows from configuration
or correctness tests alone.
