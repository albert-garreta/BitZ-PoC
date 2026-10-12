# Falcon and SHA-256 + ECDSA proof-size checker

For quantified optimization candidates, see [the Falcon cost study](FALCON_COST_STUDY.md).

The `proof_size_comparison` example generates fresh signatures and proofs,
verifies them, and prints a byte breakdown. It does not read saved benchmark
results. Its defaults reproduce these two workloads at security target 128:

| Workload | Input |
|---|---|
| Falcon | 1,024 distinct Falcon-1024 signatures over 32-byte messages, seed 42; 20,480 logical Keccak permutations |
| SHA-256 + ECDSA | One 65,472-byte message, exactly 1,024 SHA-256 compressions including padding, one P-256 signature |

Both select rate-1/2 Ligerito UDR with initial fold parameter 4. The SHA path
uses Split mode and the same fixed signing key and message bytes as
`benches/sha256_ecdsa.rs`. Falcon uses the upstream `fn-dsa` fixture generator
shared with `benches/falcon_hybrid.rs`.

Run from the repository root:

```sh
RUSTFLAGS='-C target-cpu=native' cargo run --locked --release \
  --features falcon-hybrid,ecdsa,bench-internals \
  --example proof_size_comparison -- --threads 16
```

The command uses Cargo's normal dependency setup. Add `--offline` before `--`
if dependencies are already cached. This checker does not require the native
Perfetto trace processor. The full Falcon workload uses several GiB of RAM.

To save machine-readable results, append `--json`:

```sh
RUSTFLAGS='-C target-cpu=native' cargo run --locked --release \
  --features falcon-hybrid,ecdsa,bench-internals \
  --example proof_size_comparison -- --threads 16 --json > proof-sizes.json
```

Progress and Cargo output go to stderr. JSON goes to stdout after all selected
workloads succeed. Each result includes the exact workload counts, individual
component bytes, total bytes, and verification flags. The reported elapsed
time includes fixture generation and setup; it is not a prover benchmark.

The checker fails if native signature verification, proof verification, or a
component-sum assertion fails. It also serializes and decodes the SHA/ECDSA
proof, requires identical re-encoded bytes, and verifies the decoded proof.
Falcon currently has no complete wire codec, so it is verified through its
in-memory API.

## Expected default results

Measured from the implementation at `4491309f`, with reporting helpers added.
These are measured values, not hardcoded checks. Merkle paths can change size
when inputs, protocol code, or transcript configuration changes.

### Falcon: 1,024 signatures

| Component | Payload bytes |
|---|---:|
| Compaction forest evaluations | 720,896 |
| Compaction forest sumchecks and nonces | 4,128 |
| Compaction terminal points | 360,448 |
| Compaction roots and terminal claims | 65,536 |
| Integer-to-binary bridge column sums | 524,288 |
| Bridge GKR messages and nonces | 11,448 |
| Native ring proof | 22,844 |
| Norm proof | 2,680 |
| HashToPoint rejection proof | 1,904 |
| Compaction leaf reduction | 2,064 |
| Arithmetic binding proof | 1,984 |
| Two Keccak proof prefixes | 9,040 |
| Joint binary sumcheck | 1,256 |
| Ring switch | 2,048 |
| Ligerito opening | 360,456 |
| Source Merkle authentication paths | 352,128 |
| Remaining cached values and nonces | 600 |
| **Total** | **2,443,748 (2.33054 MiB)** |

The program prints finer subcomponents for the grouped entries above. Its sum
must equal `FalconHybridProof::payload_size_bytes()`.

This is stored payload accounting, including stored challenge points and
internal Keccak/Ligerito codec framing. It excludes the public statement,
the three separately supplied source commitment roots, and Falcon-level
transport headers, vector lengths, and tags. Internal Ligerito commitment
roots are included. It is not a Rust heap-size measurement.

### SHA-256: 1,024 compressions and one P-256 verification

| Component | Serialized bytes |
|---|---:|
| Outer sumcheck and terminal claims | 880 |
| Shared inner sumcheck | 1,200 |
| Initial, outer, and inner nonces | 320 |
| BitZ transcript, including Ligerito replay messages | 80,040 |
| Encoded Ligerito proof plus hint framing | 205,636 |
| Modulus, codec headers, stream lengths, and OOD tag | 81 |
| **Total** | **288,157 (281.403 KiB)** |

The program prints these as 12 individual codec components. Their sum must
equal `Sha256EcdsaProof::to_bytes().len()`. The 205,636-byte hint stream includes
the 205,632-byte Ligerito blob and its four-byte length prefix.

The separate source commitment and public statement are excluded. The full
comparison benchmark's commitment/proof envelope adds 72 bytes for this
configuration. There is one shared proof for the hash chain and signature;
these are not separately encoded SHA and ECDSA proofs.

## Committed Falcon witness bits

The checker also reads the lengths of the actual packed inputs retained by
`CommittedFalconHybrid`, before proving consumes them. The JSON `details`
include `committed_source_bits`, `encoded_source_bits`, and their totals,
in arithmetic / first 16 Keccak permutations / last 4 permutations order.
These measurements describe committed data, not proof size or peak RAM.

For the default 1,024-signature batch:

| Source | Committed input bits | Packed MiB | Rate-1/2 codeword MiB |
|---|---:|---:|---:|
| Arithmetic | 134,217,728 | 16 | 32 |
| Keccak first 16 permutations | 1,073,741,824 | 128 | 256 |
| Keccak last 4 permutations | 268,435,456 | 32 | 64 |
| **Total** | **1,476,395,008** | **176** | **352** |

The counts include source padding. They exclude Merkle-tree storage and
other prover buffers. The three source roots themselves are only 768 bits
(96 bytes). The shared opening's larger virtual domain is not another
independently committed input.

The current implementation rejects a single batch of 1,025 signatures; its
limit is 1,024. To account for exactly 1,025 using supported batches, measure
1,024 and 1 separately. The singleton uses 1,703,936 committed input bits
(208 KiB), including the final Keccak slab's minimum eight-permutation
capacity. The combined total is 1,478,098,944 bits (176.203125 MiB), with six
source roots and two separate proofs.

## Committed SHA-256 + ECDSA witness bits

For 1,024 SHA-256 compressions in one chain plus one P-256 verification,
the checker measures the retained `FlockCommitHint::packed_message()` and
cross-checks its size against both the committed rows and PCS parameters.

| Component | Bits |
|---|---:|
| SHA block input slots: 1,024 × 512 | 524,288 |
| SHA auxiliary witness: 1,024 × 6,376 | 6,529,024 |
| ECDSA tail, sharing the SHA digest | 1,215,406 |
| Shared constant | 1 |
| Power-of-two padding | 119,889 |
| **Committed input** | **8,388,608 (1 MiB)** |
| **Rate-1/2 encoded codeword** | **16,777,216 (2 MiB)** |

The SHA input slots include the fixed final padding block. Derived linear
assignment values are not separately committed. The single source root is
32 bytes; Merkle-tree storage and other prover allocations are excluded.
The 1,024-signature Falcon batch commits 176 times as many input bits.

Run only this workload with `--workload sha256-ecdsa --sha-log-compressions 10`.
The JSON includes `live_source_bits`, `sha256_source_bits`,
`ecdsa_tail_source_bits`, `padding_bits`, `committed_source_bits_total`, and
`encoded_source_bits_total`.

## Select workloads or run a smaller check

Pass these options after Cargo's `--` separator:

| Option | Meaning |
|---|---|
| `--workload falcon` | Only generate and verify Falcon |
| `--workload sha256-ecdsa` | Only generate and verify SHA/P-256 |
| `--falcon-batch 32` | Use 32 Falcon signatures instead of 1,024 |
| `--sha-log-compressions 7` | Use 128 SHA compressions instead of 1,024 |
| `--seed 43` | Change Falcon inputs only |
| `--threads 4` | Use four Rayon workers |
| `--json` | Print one JSON report |

For a quick check of both paths:

```sh
RUSTFLAGS='-C target-cpu=native' cargo run --locked --release \
  --features falcon-hybrid,ecdsa,bench-internals \
  --example proof_size_comparison -- \
  --falcon-batch 1 --sha-log-compressions 3 --threads 4
```

Smaller or non-power-of-two Falcon batches include protocol padding; the
reported Keccak count is the logical count for live signatures. The default
1,024-signature batch is the workload behind the 2.33 MiB result.
