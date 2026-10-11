# Falcon proofs

The `falcon-hybrid` feature proves batches of 1–1024 Falcon-512 or Falcon-1024
signatures using SharedPrime. The proof checks SHAKE256, HashToPoint rejection
and ordered compaction, the Falcon ring equation, and each signature's norm.
It is not zero knowledge. Public inputs include each public key, 32-byte message,
and exact constant-time signature encoding (nonce and `s2`).

## API

```rust,ignore
use bitz::piop::spartan::falcon_profiles::{
    Falcon1024_128, FalconPublicStatement, PreparedFalconHybrid,
};

let prepared = PreparedFalconHybrid::<Falcon1024_128>::new(public_keys.len())?;
let public = FalconPublicStatement::<Falcon1024_128>::from_bytes(
    &public_keys, &messages, &signatures,
)?;
let committed = prepared.commit(public)?;
let statement = committed.statement.clone();
let proof = prepared.prove(committed)?;
prepared.verify(&statement, &proof)?;
```

Preparation is reusable for the same live batch. Proving consumes the committed
witness so large buffers can be folded without cloning. The four presets are
`Falcon512_100`, `Falcon512_128`, `Falcon1024_100`, and `Falcon1024_128`.
[PROFILES.md](PROFILES.md) describes explicit extension selection and validation.
The `falcon` feature retains direct signature verification and reference helpers.

There is one proving protocol and no NativeCarry or old-proof verifier. The
protocol identifier is `bitz/falcon/shared-prime/non-zk/v7`. Proofs must be
regenerated. Falcon currently exposes in-memory proofs without a full transport
codec. `payload_size_bytes()` counts stored messages, excluding the public
statement and transport framing; `payload_size_breakdown()` reports disjoint
components. Column sums have their own canonical encoding.

## Reductions

1. Generate live Keccak computations and arithmetic bits. Inactive Keccak
   instances have constant wire zero and an all-zero witness; active instances
   have constant wire one. Commit one Merkle tree over complete canonical
   encoded rows of the joint source. Logical source projections do not create
   separate source trees. Bind the statement, root, layout, activity policy,
   circuit identifiers, and security parameters before challenges.
2. Send canonical packed public selection masks with exactly N ones each.
   Bind them before the ring challenges. They specify linear routing; the
   arithmetic proof checks them against the SHAKE-authenticated rejection bits.
   In `E = F_12289[T]/(T^k + T + c_k)`, prove the ring identity using its quotient
   polynomial, a signature sumcheck, and an operand batch over `C,H,S2,S1`.
   Transpose affine decoders into bit coefficients, combining aliases before
   canonical integer lifting. Keep signature and local-position coefficients
   factored.
3. Fix the `2k−1` coefficients of an integer polynomial and check its evaluation
   in `E`. Sample the arithmetic prime and fresh projection point. Check the
   polynomial against the projected committed bits. The inclusive prime
   intervals are `[2^114, 2^115−2^102−1]` at 100 bits and `[2^125, 2^126−1]`
   at 128 bits. Coefficient and source-residual bounds prevent wraparound.
4. Run one cubic integer outer sumcheck combining the norm and quadratic
   rejection rows. Compute the public S2 norm directly; project the S1
   endpoint, slack, and three rejection endpoints to the committed source.
   Public-mask validity and selected coefficient routing are linear constraints
   on existing committed bits. There are no intermediate HashToPoint polynomial
   witnesses, and no HashToPoint grand-product/GKR proof.
   Store only compact sumcheck messages and endpoint values; derive
   challenge points during verification. The binder combines exact public
   equalities and authenticated endpoints with the factored ring projection,
   then reduces them to one arithmetic-source evaluation.
5. The BitZ bridge checks integer column sums and authenticates the same bits
   in the binary field. Use one unsplit sum per column at 100 bits; at 128 bits
   use two bounded limbs, encoding the upper limb in four bytes. Both modes
   produce one binary evaluation claim.
6. Binary Keccak proofs, public wiring, activity and padding checks, and the
   bridge claim join one binary sumcheck. Ring switching and recursive PCS
   folding authenticate its endpoint against the joint source root. There is
   one initial source multiproof; recursive folding commitments remain.

See [ONE_SOURCE.md](ONE_SOURCE.md) for the projection geometry,
[COMPACTION_SOUNDNESS.md](COMPACTION_SOUNDNESS.md) for ordered compaction, and
[BRIDGE_GRINDING_AUDIT.md](BRIDGE_GRINDING_AUDIT.md) for bridge bounds.

## Layout and performance

Falcon-1024 uses 100,482 live arithmetic bits and 6,504 linear rows per signature,
padded to strides of 131,072 bits and 8,192 rows. The first `4*N*16` positions
form aligned coefficient blocks ordered `S1,S2,C,H`. The coefficient encodings
remain bounded14 minus 6144, signed12, unsigned14, and unsigned14 respectively.
Norm slack bits occupy lane 15 of the first 27 `S1` coefficient rows. Signature
header/nonce bytes and the remaining HashToPoint columns follow these blocks;
the exact CT payload is reconstructed from the aligned `S2` bits without a
second copy. SHAKE nonce and sample links use the same address map.

For Falcon-1024 there are 10,213 internal padding bits and 20,377 trailing padding
bits. The occupied extent is 110,695, distinct from the 100,482 live-bit count.
Every internal hole, trailing bit, and inactive signature is constrained to zero.
Use the layout's coefficient, signature-byte, and slack address methods rather
than assuming a contiguous live prefix. Falcon-512 uses 52,637 live bits in a
65,536-bit stride and 3,524 linear
rows in a 4,096-row stride. Quadratic rejection domains have 1,024 / 2,048
rows. Compared with the previous grand-product protocol, removing prefix
counters saves 7,180 / 14,432 live bits without changing the padded domains.
Masks add 90 / 164 bytes per signature to the proof payload, outside the source.
At batch 1,024 the Falcon-1024 column table occupies 262,144 bytes at 100 bits
and 327,680 bytes at 128 bits. It remains a substantial proof-size cost.

Falcon-1024 packs its twenty SHAKE permutations into slabs of 12, 6, and 2.
The first two use three independent Keccak permutations per 131,072-bit circuit
block; the last uses one permutation per 65,536-bit block. Public wiring checks
every chain edge, including edges inside triples and across slab boundaries.
Falcon-512 retains its 8+2+1 single-permutation slabs. Each circuit needs at least
eight blocks for byte-stripe lincheck, so the three Falcon-1024 slab capacities
at batch one are 2, 4, and 4 signatures. Inactive blocks remain all zero.

For Falcon-1024 capacities of at least four, the logical source sizes total
1,048,576 bits per signature, down from 1,441,792. The joint padded domain halves,
and all sixteen committed lanes are occupied. At batch one the eight-block
floors instead double the joint domain; batch two retains its previous joint
domain size. These are layout counts, not measured proving-time or proof-size
improvements. The extra Keccak prefix and changed PCS geometry also affect cost.

The implementation preserves packed source buffers, factored ring coefficients,
compiled binder templates, compact zero-lane and final-message encodings, and
SIMD Keccak/grinding kernels. Security targets remain 100 and 128 bits. PCS
configuration and grinding are derived from the new public geometry and the
full error ledger, including all three Keccak prefixes. Every native minimum
remains enforced, and the schedule is bound into the statement.

## Benchmarks

Full and arithmetic-only proofs have direct Cargo benchmark targets:

```sh
RAYON_NUM_THREADS=1 RUSTFLAGS='-C target-cpu=native' cargo bench --profile release --features falcon-hybrid --bench falcon_hybrid -- --degree 512 --security 128 --batch 1024 --threads 1 --warmup 1 --iterations 5
RAYON_NUM_THREADS=1 RUSTFLAGS='-C target-cpu=native' cargo bench --profile release --features falcon-hybrid --bench falcon_algebraic -- --degree 512 --security 128 --batch 1024 --threads 1 --warmup 1 --iterations 5
```

Use `--degree 1024` for Falcon-1024. Both runners generate original-Falcon
inputs outside proof timing, verify every proof, and print JSON metrics to stdout.
The full runner accepts environment defaults; explicit arguments take precedence.
The optional `falcon_field` target compares public field power-generation kernels.

For measurements on an unused Linux CPU, check that its SMT sibling is idle,
set both `BITZ_FALCON_MAIN_CPU` and `BITZ_FALCON_WORKER_CPUS` to that CPU ID,
and launch with `taskset -c <cpu>`. Keep `RAYON_NUM_THREADS` and `--threads` at 1.
The runners report observed main, worker, and auxiliary pool affinity. Build
before measuring and run the benchmark processes sequentially.

### Keccak3 packing comparison

A local comparison against `51b472c55` on 2026-10-10 used a Ryzen 9 9950X3D,
16 threads, native release codegen, Falcon-1024 at 128-bit security, and seed 42.
Each variant ran in a fresh process with one warmup and three measured trials;
processes ran sequentially, without pinned CPU affinity. Input digests matched
and every proof verified. Entries below are baseline → 12+6+2 packing.

| Batch | Median commit + prove (ms) | Peak RSS (MiB) | Median verify (ms) | Proof payload (bytes) |
|---:|---:|---:|---:|---:|
| 1 | 24.94 → 25.29 | 49.7 → 93.8 | 12.85 → 15.38 | 166,918 → 175,958 |
| 32 | 44.32 → 47.78 | 167.2 → 173.7 | 15.03 → 17.95 | 317,922 → 301,066 |
| 1024 | 675.52 → 602.44 | 3062.0 → 2525.2 | 53.24 → 57.97 | 971,378 → 967,234 |

Commit + prove excludes input generation, preparation, and verification. RSS is
process high-water memory across all trials. Proof payload excludes public
inputs and transport framing. These local measurements show a large-batch
prover benefit, with slower verification and no universal small-batch benefit.
At batch 1024, packed inputs are 176 → 128 MiB, initial codewords 352 → 256 MiB,
and the shared virtual domain 256 → 128 MiB. The smaller opening domain does not
imply a proportionally smaller proof.

[Raw measurements and reproduction settings](../../../../results/falcon_keccak3_packing_2026-10-10.json)
include all trial records and derived PCS configurations. Validation passed
203 Falcon unit tests, 21 profile tests including the qualification matrix,
and both new encoder regression tests. Two existing standalone vendor tests
(`all_zero_witness_rejected`, `prove_fast_roundtrip`) reproduce the same default
Ligerito configuration mismatch on the baseline and candidate: at m=22 the
embedded initial k is 4 while the caller requests 6. Full Falcon uses its own
prepared opening configuration and passes the complete proof tests.
