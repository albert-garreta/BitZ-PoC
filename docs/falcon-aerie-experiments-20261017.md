# Falcon-512 algebraic: Aerie comparison and bench-only experiments (2026-10-16/17)

Base: `origin/falcon-poc` e140a3f3. Every switch below exists only in
`bench-internals` builds. Without that feature the code paths, transcripts and
batch limit are those of e140a3f3. John's algebraic tests pass on that build:
92 passed, 1 ignored (the large batches).

## Switches

| Variable | Values | Effect |
|---|---|---|
| `BITZ_FALCON_GRINDING_MODEL` | `union` (default), `per-site` | Per-site grinds every challenge site to its own `2^-128`, as Aerie does (Thm 5.9: `(Q+R)·2^-128`). Union is the existing union-bound allocator. |
| `BITZ_FALCON_PRIME` | `default`, `small` | `small` samples `2^114 <= p <= 2^115 - 2^103`, so `(rows+1)(p-1) < 2^128 - 1` at 2^13 bridge rows. The BitZ bridge then runs unsplit, with one forest leaf per bit instead of two. Requires `per-site`. |
| `BITZ_FALCON_LIGERITO` | `udr` (default), `johnson` | `johnson` currently fails at 128 bits: out-of-domain samples give 118.8 bits < 128. |
| `BITZ_FALCON_PRINT_GRINDING` | any | Prints the per-site difficulty report to stderr. |
| `BITZ_ARITHMETIC_BINDING_STRATEGY` | `optimized` | Existing switch. Production enables the optimized binder only at 1, 8, 12 or 16 workers. |

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
