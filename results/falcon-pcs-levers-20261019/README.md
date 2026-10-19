# Proof-size levers for algebraic Falcon (2026-10-19)

Three ways to shrink the algebraic Falcon proof, measured alone and together:

- **A**, a smaller Reed–Solomon rate for the Ligerito opening (1/4 to 1/32);
- **B**, Johnson list decoding at 128 bits, with every Ligerito challenge in GF(2^256);
- **C**, the polylog BitZ bridge, which commits the bridge's column sums instead of sending them.

Every run is algebraic Falcon-512 with 32,768 signatures at 128 bits, on 10 threads of an Apple M5 on AC power. The setup is G from the Aerie comparison: keyed ring, small prime, 2^16 bridge rows, per-site grinding and the optimized binder. Each run is a warm-up plus three proves. The tables give each run's median, then the mean over input seeds 42 and 1. Every campaign ran under `scripts/bench_gate.py`; the clock stayed at 3.8–3.9 GHz (`clock.log`).

## Code

Branch `falcon-pcs-levers`, on top of `origin/falcon-aerie-experiments` (`848d0a3d5`):

| Commit | What |
|---|---|
| `158f2b377` | `BITZ_FALCON_LIGERITO` takes any profile (`udrg:R:K`, `custom:R:K`, `ext:R:K`) |
| `411977004` | Ligerito over GF(2^256) and the `ext:R:K` profiles, ported from the SHA+ECDSA line |
| `3b16de597` | The joint opening over GF(2^256), with Round 0 right after the statement |
| `d83a49edc` | The polylog bridge, `BITZ_FALCON_BRIDGE=polylog` |

`3b16de597` also fixes a gap: before it, a Johnson profile over GF(2^128) (`custom:R:K`, reachable at 100 bits) ran the joint opening without Round 0, which the Johnson regime's soundness needs. Johnson profiles now run Round 0, and the plain joint opener refuses them.

## Switches

| Lever | Variables |
|---|---|
| Setup G | `BITZ_FALCON_GRINDING_MODEL=per-site BITZ_FALCON_PRIME=small BITZ_ARITHMETIC_RING=keyed BITZ_FALCON_BRIDGE_ROWS=16 BITZ_ARITHMETIC_BINDING_STRATEGY=optimized` |
| A (UDR at rate 2^-R) | `BITZ_FALCON_LIGERITO=udrg:R:4` |
| B, and A+B (rate 2^-R) | `BITZ_FALCON_LIGERITO=ext:R:4` |
| C | `BITZ_FALCON_BRIDGE=polylog`, fold opener `BITZ_FALCON_FOLD_LIGERITO` (default `ext:5:4`) |

## Results

| Configuration | Levers | Prove | Verify | Proof | PCS opening | Bridge | Peak RSS |
|---|---|---:|---:|---:|---:|---:|---:|
| UDR 1/2 (setup G) | – | 790 ms | 20.6 ms | 593 kB | 374 kB | 145 kB | 2.4 GiB |
| UDR 1/4 | A | 861 ms | 19.7 ms | 540 kB | 321 kB | 145 kB | 2.7 GiB |
| UDR 1/8 | A | 937 ms | 20.9 ms | 534 kB | 315 kB | 145 kB | 2.9 GiB |
| UDR 1/16 | A | 1,078 ms | 20.0 ms | 543 kB | 324 kB | 145 kB | 3.6 GiB |
| UDR 1/32 | A | 1,369 ms | 20.1 ms | 555 kB | 336 kB | 145 kB | 4.8 GiB |
| Johnson GF(2^256) 1/2 | B | 856 ms | 19.2 ms | 512 kB | 293 kB | 145 kB | 2.9 GiB |
| Johnson GF(2^256) 1/4 | A+B | 879 ms | 18.9 ms | 422 kB | 202 kB | 145 kB | 3.0 GiB |
| Johnson GF(2^256) 1/8 | A+B | 940 ms | 19.0 ms | 385 kB | 166 kB | 145 kB | 3.4 GiB |
| Johnson GF(2^256) 1/16 | A+B | 1,105 ms | 19.9 ms | 365 kB | 146 kB | 145 kB | 4.1 GiB |
| Johnson GF(2^256) 1/32 | A+B | 1,372 ms | 19.0 ms | 352 kB | 133 kB | 145 kB | 5.4 GiB |
| UDR 1/2 + polylog | C | 822 ms | 23.8 ms | 524 kB | 375 kB | 75 kB | 2.4 GiB |
| UDR 1/8 + polylog | A+C | 944 ms | 23.3 ms | 464 kB | 315 kB | 75 kB | 2.9 GiB |
| Johnson 1/2 + polylog | B+C | 892 ms | 22.7 ms | 444 kB | 294 kB | 76 kB | 2.9 GiB |
| Johnson 1/8 + polylog | A+B+C | 977 ms | 22.8 ms | 315 kB | 166 kB | 75 kB | 3.4 GiB |
| Johnson 1/16 + polylog | A+B+C | 1,098 ms | 22.4 ms | 295 kB | 146 kB | 75 kB | 4.1 GiB |
| Johnson 1/32 + polylog | A+B+C | 1,356 ms | 22.6 ms | 281 kB | 132 kB | 74 kB | 5.5 GiB |
| Johnson 1/32 + polylog, folds at 1/8 | A+B+C | 1,350 ms | 22.3 ms | 292 kB | 133 kB | 85 kB | 5.4 GiB |
| Johnson 1/32 + polylog, folds at 1/128 | A+B+C | 1,390 ms | 22.6 ms | 277 kB | 133 kB | 70 kB | 5.5 GiB |

`levers.py` prints this table and writes `levers.json`. "PCS opening" is the `pcs` payload entry, while the page's "PCS opening" adds the 1.2 kB binary source sumcheck. The two seeds stay within 4 % of each other on every row. The baselines re-run in the later binaries agree with the first ones: UDR 1/2 measured 780–795 ms in all three campaigns, and Johnson 1/32 1,349 ms in the polylog binary.

The rest of the proof does not change with these levers: 65.5 kB of coefficient exception counts (two bytes per signature, even when a signature has no exceptions), 7.5 kB of ring and norm messages, and the 32-byte commitment.

## What each lever does

**A, smaller rate.** Under unique decoding, each query buys at most one bit, since the decoding distance stays below 1/2. The query count therefore floors near 130: 309, 189, 155, 141 and 134 at the first level for rates 1/2 to 1/32. Meanwhile the codewords and Merkle paths keep growing. The proof is smallest at 1/8 (534 kB, −10 %, for +19 % prove). The commitment grows with every halving of the rate: 75, 105, 160, 275 and 494 ms from 1/2 to 1/32.

**B, Johnson over GF(2^256).** In the Johnson regime, Round 0 samples the committed message out of domain right after the statement, which pins one element of the decoding list. Its error is the list's pairs times the degree of the sampled polynomial over the field size. Here Round 0 evaluates at `(ζ, ζ², ζ⁴, …)`, a univariate polynomial of degree 2^22 − 1, so over GF(2^128) it keeps only 97 bits at 2^29 committed bits (flock's own level-0 check, with a multilinear point, stops at 118). So `custom:R:K` cannot reach 128 bits. With every Ligerito challenge drawn in GF(2^256) (`ext:R:K`), Round 0 has 225 bits, the fold challenges need no grinding, and the queries fall with the rate: 244, 119, 79, 60 and 48 at the first level for rates 1/2 to 1/32. Extension messages take 32 bytes, but the level-0 rows stay in GF(2^128). At rate 1/2 alone this gives 512 kB (−14 %) for +8 % prove; at 1/8, 385 kB (−35 %) for +19 %.

**C, polylog bridge** (`src/hybrid/integer_bridge/polylog.rs`). The standard unsplit bridge sends its 2^13 integer column folds `S[c]` in the clear (131 kB). Instead, the prover commits `T[c] = S[c] + 1`, one GF(2^128) word per column, and then:

1. it sends `R = Σ_c eq(ζ, c)·g^{S[c]}`, which the source forest proves as before;
2. a second product tree over the bits of `T` (images `g^{2^k}`) proves `g·R`;
3. a small unsplit bridge on `T`, reshaped to 2^16 rows `(k, c_lo)` and 16 columns, proves the prime read-off `Σ_c cw[c]·T[c] = v + 1`. Since `Σ_c eq_p(·, c) = 1`, this is the original claim;
4. the two claims on `T` are batched, reduced by a joint sumcheck, and opened by `T`'s own Ligerito run at rate 1/32, with its Round 0.

The shift by one matters. `g` has order 2^128 − 1, so a committed word 2^128 − 1 and the value 0 have the same image. Without the shift, a prover could replace any zero column by 2^128 − 1 and move the read-off by a subset sum of known weights. Shifted folds lie in [1, 2^128 − 1), which rules this out.

At 32K signatures, the bridge drops from 145 kB to 75 kB. The parts are: the fold opening 46 kB, the source forest 14.7 kB (with `R` and nonces), the read-off 8.3 kB, the second tree 3.8 kB, the fold sumcheck 0.9 kB and the commitment 64 B. Verification takes 3 to 4 ms longer. The prove time changes by +7 to +37 ms: in the same binary, +35 ms at UDR 1/2 and +7 ms at Johnson 1/32. The new grinding sites change the transcript, and with it every later proof-of-work draw, so two seeds do not pin the overhead down further. Opening the folds at 1/128 instead of 1/32 saves another 4 kB for +35 ms.

## Caveats

- The `ext:` profiles inherit 16-bit query grinding per level from flock's slim template; the UDR profiles have none (the per-site plan adds 4 bits). Part of B's query reduction comes from that grinding.
- The GF(2^256) opener is a direct implementation, without the base opener's lookahead and deferred passes. At the same rate it costs about 60 ms more (790 → 856 ms at 1/2), and part of that is Round 0 itself.
- The polylog bridge opens `T` in a second Ligerito run. The polylog note batches both commitments into one merged run, which would save part of the 46 kB fold opening.
- The polylog bridge needs the small prime (unsplit bridge), per-site grinding and at least 2^7 columns. It is a bench-only switch.
- With `bench-internals`, four existing tests fail as before (they assert the 1024-signature cap that bench builds raise to 65,536). The default build passes all 495 Falcon, hybrid and Ligerito tests.

## Files

- `rates/`, `ext/`, `polylog/`: one `.jsonl` (bench records) and one `.err` (grinding report, `/usr/bin/time -l`) per run, plus `clock.log`, `build.rev` (the commit each binary was built from) and `binaries.txt` (md5).
- `rates.zsh`, `ext.zsh`, `polylog.zsh`: the campaigns, run from the gitignored `bench_results/pcs-levers-20261019/`.
- `summarize.py` (per run), `levers.py` (the table above), `page_levers.py` with `page_notes.json` (the flamegraph page's levers panel, `page.json`).
- Tests: `polylog_bridge_roundtrip_matches_claims_and_rejects_tampering`, `johnson_openings_run_round0_and_reject_tampering`, `residual_b_evals_ext_matches_dense_fold`, and flock's `compact_initial_extension_opening_roundtrip_and_tamper`. The env-driven end-to-end test runs with `BITZ_FALCON_PRIME=small BITZ_FALCON_GRINDING_MODEL=per-site BITZ_FALCON_BRIDGE=polylog cargo test --release --features falcon-hybrid,bench-internals --lib -- --ignored polylog_bridge_end_to_end`.
