# Falcon-512 algebraic: Aerie comparison and bench-only experiments (2026-10-16/17)

Base: `origin/falcon-poc` e140a3f3. Every switch below exists only in
`bench-internals` builds. Without that feature, the transcripts and the batch
limit are those of e140a3f3. The one default-path change is that statement
validation and byte packing run in parallel; the hashed bytes are the same.

The algebraic tests pass on the default feature set:
- falcon-aerie-experiments: 92 passed;
- falcon-ring-succinct: 98 passed, including the keyed ring and wider bridge rows.

Both have 1 ignored test (the large batches).

## Switches

| Variable | Values | Effect |
|---|---|---|
| `BITZ_FALCON_GRINDING_MODEL` | `union` (default), `per-site` | Per-site grinds every challenge site to its own `2^-128`, as Aerie does (Thm 5.9: `(Q+R)·2^-128`). Union is the existing union-bound allocator. |
| `BITZ_FALCON_PRIME` | `default`, `small` | `small` samples `2^114 <= p <= 2^115 - 2^103`, so `(rows+1)(p-1) < 2^128 - 1` at 2^13 bridge rows. The BitZ bridge then runs unsplit, with one forest leaf per bit instead of two. Requires `per-site`. |
| `BITZ_FALCON_LIGERITO` | `udr` (default), `johnson` | `johnson` currently fails at 128 bits: out-of-domain samples give 118.8 bits < 128. |
| `BITZ_FALCON_PRINT_GRINDING` | any | Prints the per-site difficulty report to stderr. |
| `BITZ_ARITHMETIC_BINDING_STRATEGY` | `optimized` | Existing switch. Production enables the optimized binder only at 1, 8, 12 or 16 workers. |
| `BITZ_ARITHMETIC_RING` | `canonical` (default), `factored`, `keyed` | `keyed`: the integer ring check of `keyed_ring.rs` (below). |
| `BITZ_FALCON_BRIDGE_ROWS` | `13` (default) … `16` | Bridge row variables `t`. The proof carries `2^(n-t)` column sums instead of `2^(n-13)`. With `PRIME=small` the family becomes `2^(127-t) <= p <= 2^(128-t) - 2^(116-t)`. |

- The batch cap is 65536 in `bench-internals` builds and stays at 1024 otherwise.
- `falcon_algebraic:verify_{statement,sample_field,arithmetic,bridge,joint,pcs}` are tracing spans only.

## Build and run

```
RUSTFLAGS='-C target-cpu=native' cargo bench --no-run --profile release \
  --features falcon-hybrid,bench-internals --bench falcon_algebraic
BITZ_FALCON_GRINDING_MODEL=per-site BITZ_FALCON_PRIME=small \
BITZ_ARITHMETIC_BINDING_STRATEGY=optimized RAYON_NUM_THREADS=10 \
  target/release/deps/falcon_algebraic-* --degree 512 --batch 32768 --security 128 \
  --threads 10 --warmup 1 --iterations 3
```

## Results

Machine and method:
- Apple M5, 10 threads, AC power.
- Each run is gated: idle at least 80 % for 60 s, under one machine-wide lock.
- Median of 3 samples after a warm-up.
- All proofs verify, with composed security 128.00 bits.

Falcon-512, 128-bit, 32768 signatures:

| Configuration | Prove (incl. commit) | Verify | Proof | Peak RSS |
|---|---|---|---|---|
| e140a3f3 as configured (10 workers, legacy binder) | 1681 ms | 590 ms | 1.77 MB | 3.9 GiB |
| + `BINDING_STRATEGY=optimized` | 1070 ms | 201 ms | 1.77 MB | 3.9 GiB |
| + `per-site` | 990 ms | 200 ms | 1.77 MB | 3.9 GiB |
| + `PRIME=small` | **770 ms** | 200 ms | 1.51 MB | **2.8 GiB** |
| Aerie v0.2.0 on the same machine (plain release, as published) | 964 ms | 14.9 ms | 1.38 MB (68 KB with one shared salt) | 2.0 GiB |

Other batch sizes (per-site, optimized binder):

| Batch | Default prime | Small prime | Notes |
|---|---|---|---|
| 1024 | 41.7 ms | 42.9 ms | Extra prime-field PoW offsets the smaller forest. |
| 16384 | 500 ms | 399 ms | |
| 32768, 1 thread | 3.92–4.12 s | 3.16–3.32 s | Aerie: 4.09 s. |

Prover phases at 32K (traced):

| Phase | Default prime | Small prime |
|---|---|---|
| BitZ forest | 537 ms | 299 ms |
| Word binder | 176 ms | 185 ms |
| Integer outer | 74 ms | 109 ms |
| Ring | 131 ms | 127 ms |
| Commit | 107 ms | 98 ms |
| PCS | 51 ms | 53 ms |

The integer outer includes prime-field PoW: with the small prime, every prime-field round grinds 14–18 bits.

Verifier at 32K, 10 threads (1 thread: 879 ms):

| Component | Time |
|---|---|
| Ring check (alpha coordinates + collapse per signature and coefficient) | 114 ms |
| Merged binding-form construction and MLE evaluation | 45 + ~10 ms |
| Statement validation and hashing | 34 ms |
| Bridge | 12 ms (8 ms one-limb) |
| PCS | 3 ms |

John's experimental three-plane compact binding (`BITZ_ARITHMETIC_COMPUTE_VARIANT=combined`) at 32K: 1.38–1.57 s prove, 3.14 MB proof.

## Keyed integer ring check and bridge rows (branch falcon-ring-succinct)

### The keyed ring check

The ring relation holds iff `q` divides every coefficient of the integer residual
`R_i = s1_i + (h_i s2_i mod X^N+1) - t_i`. The check runs in five steps:

1. The verifier samples independent uniform `A_i in F_q[Y]` with 11 coefficients per signature, from one seed. If some `R_i` is nonzero mod `q`, so is `sum_i A_i R_i`, except with probability `q^-11`.
2. The prover sends the exact quotient `K = sum_i A_i R_i / q`: `N + 10` integers, 4.2 KB for Falcon-512. It obtains each `R_i / q` from `h_i s2_i` modulo a 31-bit NTT prime.
3. `sum_i A_i R_i - qK` is a nonzero integer polynomial of degree `N + 9` unless the batch relation holds. The protocol checks it at random points of `F_p`:
   - one point when `p >= 2^120`, ground for `(N+9)/p`;
   - two independent points and a merge coefficient otherwise.
4. Weights: `s1[i][j]` gets `A_i(xi) xi^j` and `s2[i][j]` gets `A_i(xi) (h_i X^j mod X^N+1)(xi)`. The prover fills them with a two-term recurrence in `j`.
5. The verifier never materializes them. Per point it forms `sum_j e_s1[j] xi^j` and `G[k] = sum_j e_s2[j] sigma(j,k) xi^((j+k) mod N)`, then takes one dot product `<h_i, G>` per key. That dot product splits `G` into four 32-bit limbs, so each term is a widening multiply-accumulate.

The binder, norm and bridge are unchanged.

### Statement validation

Statement validation and byte packing are parallel now. The hashed byte stream is identical.

### Bridge rows above 13

Source rows stay one polynomial each (2^13 bits). The bridge then:
- groups `2^(t-13)` source rows per column;
- moves the lowest `t-13` column variables of the binder's claim into its row weights through their equality weights;
- maps the binary claim to the compact planes with `compact::coefficients(layout, parameters, ...)`, whose slack gather now also handles rows that span whole signatures.

### Results

Falcon-512 @128, 32K signatures, 10 threads, per-site grinding, optimized binder. Each cell is the median of 3, gated runs on the M5:

| Configuration | Prove | Verify | Proof | Bridge sums | Peak RSS |
|---|---|---|---|---|---|
| default prime, canonical ring, t=13 | 927 ms | 173 ms | 1.77 MB | 1.32 MB | 3.9 GiB |
| default prime, keyed ring, t=13 | 1071 ms (25 % spread) | 30.3 ms | 1.77 MB | 1.32 MB | 3.4 GiB |
| small prime, canonical ring, t=13 | 786 ms | 191 ms | 1.51 MB | 1.06 MB | 2.8 GiB |
| small prime, keyed ring, t=13 | 818 ms | 26.7 ms | 1.51 MB | 1.06 MB | 2.3 GiB |
| small prime, keyed ring, t=14 | 766 ms | 22.7 ms | 0.99 MB | 537 KB | 2.4 GiB |
| small prime, keyed ring, t=15 | 843 ms | 23.6 ms | 0.73 MB | 275 KB | 2.4 GiB |
| **small prime, keyed ring, t=16** | **807 ms** | **21.4 ms** | **0.59 MB** | **145 KB** | **2.4 GiB** |
| default prime, keyed ring, t=16 | 902 ms | 18.7 ms | 0.63 MB | 178 KB | 3.5 GiB |
| default prime, canonical ring, t=16 | 923 ms | 168.5 ms | 0.62 MB | 178 KB | 3.9 GiB |
| Aerie v0.2.0 (same machine) | 964 ms | 14.9 ms | 1.38 MB (68 KB with one salt) | — | 2.0 GiB |

One thread, 32K:

| Configuration | Prove | Verify |
|---|---|---|
| default, canonical, t=13 | 3.94 s | 879 ms |
| small, keyed, t=13 | 3.24 s | 82 ms |
| small, keyed, t=16 | 3.68 s | 81 ms |
| Aerie | 4.09 s | 57 ms |

At t=16 the small prime grinds 18–19 bits per prime-field round, which costs about 0.4 s on one core.

1K, 10 threads (median of 10):

| Configuration | Prove | Verify | Proof |
|---|---|---|---|
| default, canonical | 39.3 ms | 9.2 ms | 258 KB |
| default, keyed | 39.1 ms | 4.2 ms | 262 KB |
| small, canonical | 41.6 ms | 8.6 ms | 248 KB |
| small, keyed | 41.1 ms | 4.0 ms | 252 KB |

Traced verifier at 32K (small, keyed, t=13), 26.5 ms in total:

| Component | Time |
|---|---|
| Statement validation and hashing | 9.0 ms |
| Bridge | 7.7 ms |
| Binder and norm | 3.6 ms |
| PCS | 2.8 ms |
| Keyed ring | 2.2 ms |

Prover phases, same configuration: forest 280 ms, binder 168 ms, integer outer 77 ms, keyed ring 159 ms (canonical: about 127 ms), PCS 49 ms, commit 80 ms.

## Full Falcon (hybrid prover, `08dee73eb`)

The hybrid (`benches/falcon_hybrid.rs`, which also proves SHAKE256 HashToPoint) reads the same variables:
- `BITZ_FALCON_GRINDING_MODEL`: wired in since `17482040f`.
- `BITZ_FALCON_BRIDGE_ROWS=13..16`: groups `2^(t-13)` source rows per bridge column and takes the row weights over the binder point's lowest `t` coordinates.
- `BITZ_FALCON_PRIME=small`: at 128 bits samples `[2^(127-t), 2^(128-t) - 2^(116-t)]` and runs the bridge unsplit. The prime-field schedule floors follow the sampled family. With more rows the 100-bit family follows `t` too.

Its binder replay is on by default only on Linux x86-64. `BITZ_BENCH_BINDER_REPLAY=on` forces it, like `BINDING_STRATEGY=optimized` for the algebraic binder.

Falcon-512 @128, 1024 signatures, 10 threads. The numbers are means over six input seeds, because with fixed inputs each large grinding site is a single draw (`results/falcon-full-20261018`, `seed_means.py`):

| Configuration | Prove | Range | Verify | Proof |
|---|---|---|---|---|
| as built on macOS | 552 ms | 524-601 | 28.3 ms | 698 KB |
| + binder replay | 516 ms | 497-540 | 28.2 ms | 698 KB |
| + per-site | 408 ms | 382-437 | 28.1 ms | 698 KB |
| + 2^16 bridge rows | 422 ms | 396-461 | 28.2 ms | 557 KB |
| + small prime | 1,651 ms | 591-3,257 | 28.1 ms | 550 KB |
| per-site, small prime, 2^13 rows | 434 ms | 383-519 | 27.4 ms | 665 KB |
| tight linear numerator: small prime, 2^13 rows | 379 ms | 373-393 | 27.5 ms | 664 KB |
| tight linear numerator: small prime, 2^16 rows | 468 ms | 446-495 | 28.4 ms | 551 KB |

Every prime-field site grinds 11 to 14 bits more with the small prime. The linear batch is the largest site.

Its numerator (`FalconSecurityNumerators::linear`, `m + d + B + 5`) includes the batch size `B`, but the block only draws a multilinear point over the linear rows and signatures (total degree `m + d`), `eta` (used up to its fifth power) and one merge coefficient. `BITZ_FALCON_LINEAR_NUMERATOR=tight` (bench-only) counts `m + d + 6` instead. At 1024 that takes the linear site from 19 to 13 bits under the union model and from 14 to 8 per site; with the small prime at 2^16 rows, from 28 to 22 bits.

The keyed ring check is not ported. The hybrid's ring check commits `h` and `s2` and costs 7.8 ms prove, under 2 ms verify and 12.6 KB at 1024.
