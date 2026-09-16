# U32 multiplication transcript

Run one commitment, proof, and verification of 32,768 independent U32
multiplications:

```bash
RUSTFLAGS="-C target-cpu=native" \
cargo run --release --example u32_mul_transcript -- \
  --out /tmp/f2z-u32-transcript
```

The output directory must be fresh. Omit `--out` to create a unique directory
under `target/transcript-logs/`. The executable prints absolute artifact paths.
`--fiat-shamir-transcript-logs=false` disables transcript capture; the default is true.

## Statement and inputs

For every `i` in `[0, 32768)`, the committed witness satisfies
`x_i * y_i = z_i + 2^32 * w_i`, with all four limbs in `[0, 2^32)`.
`z` and `w` are the low and high limbs of the full U64 product.

The public instance is the commitment root **and its parameters**. The public
configuration is W1 packing, Lambda100, and Johnson Ligerito at rate 1/2 with
initial `k=4`. The verifier receives the prepared relation, commitment, and
existing `U32MulProof`. It receives no separate operand or expected-product arrays.
The prime, challenges, and evaluation points come from the existing protocol.

The example reproduces the regression witness:

```rust
let x = (i as u32).wrapping_mul(0x9e37_79b9) | 1;
let y = (i as u32).wrapping_mul(0x85eb_ca6b) | 1;
let product = u64::from(x) * u64::from(y);
let z = product as u32;
let w = (product >> 32) as u32;
```

There is no RNG seed. These values occupy private-witness inputs even though
the fixture is reproducible. The proof establishes the multiplication relation
under the commitment; it does not additionally prove this generation recipe.

Each row has 128 little-endian bit slots, `x32 | y32 | product64`.
There are 4,194,304 committed bits, `row_vars=15`, `col_vars=7`, `word_bits=1`,
and no padded multiplication rows.

## Artifacts and reading

| File | Contents |
| --- | --- |
| `statement.json` | Relation, ranges, dimensions, resolved configuration, commitment, assignment-binding digest |
| `run.json` | Fixture recipe, revision/dirty state, verification result, separate prover/verifier digests, artifact paths |
| `transcript.jsonl` | One record per logical absorb or squeeze, with complete values, caller spans, and nested byte operations |
| `transcript.txt` | Readable rendering of the JSONL records, including all values, caller context, and the public statement behind the initial digest |

At `statement.assignment_binding_digest`, the example shows the matching
`statement.json` contents for both prover and verifier: commitment root and
parameters, relation, dimensions, and resolved configuration. This is a human
description of the public statement, not the binary hash preimage. The absorbed
payload is still the 32-byte digest shown above it. The JSONL capture is unchanged.

Each role starts with a fresh `Blake3Transcript`. Diagnostic labels and JSON are
never absorbed. Existing framing, domain separators, generator, sampling,
grinding, and feedback bytes are unchanged. The existing prover/verifier final
states differ and remain pinned separately.

Read logical records as a stream:

```rust
use f2z::transcript::logging::{read_events, Event};

for event in read_events("/tmp/f2z-u32-transcript/transcript.jsonl")? {
    match event? {
        Event::Logical(record) => println!(
            "{} {:?} {}: {}", record.role, record.operation,
            record.purpose, record.value,
        ),
        Event::Legacy(record) => println!("{:?}", record.fields),
    }
}
```

Filter the file by meaning and role:

```bash
jq -c 'select(.role == "prover" and (.purpose | startswith("sumcheck.round_"))) |
  {operation, purpose, context, value, draw_count}' \
  /tmp/f2z-u32-transcript/transcript.jsonl
```

For byte replay, use `read_byte_events(path)`. It expands logical records into
exact ordered byte events and also reads older raw captures. Start a fresh
BLAKE3 hasher per role, apply every `absorb`, and check every `squeeze` against
`finalize_xof()`. The nested `wire` list retains framing, retries, grinding, and
accepted-challenge feedback. Each byte step also retains its current spans.
Malformed, unsupported, or incomplete records are errors, with line numbers.
These files recover the transcript state; they do not serialize the proof.

## Logical message interface

`Absorbable` supplies `kind`, `visit_chunks`, and `log_value`. Its encoder can
emit byte slices but cannot access a transcript or sample a challenge.
`transcript.absorb(&object)` calls `absorb_frame` once, resulting in one row.
Existing typed helpers use borrowed adapters with their original encodings.
`SpartanRoundPolynomial` displays canonical monomial coefficients,
constant-first. A whole proof is processed round by round:

```text
ABSORB sumcheck.round_polynomial   coefficients_hex: [c0, c1, c2, c3]
SQUEEZE sumcheck.round_challenge   value_hex: r0
ABSORB sumcheck.round_polynomial   coefficients_hex: [d0, d1, d2, d3]
SQUEEZE sumcheck.round_challenge   value_hex: r1
```

Absorbed field values and returned field challenges use canonical numeric hex.
Wire hex preserves byte order and, where the protocol uses them, internal
Montgomery representations. Each field type provides its explicit diagnostic
conversion through `TranscriptField`; this conversion never changes encoding.
Opaque messages retain their complete byte value. There is no truncation.

## Capture lifecycle and context

Compose `TranscriptLogs::layer()` with profiling in one subscriber. The layer
keeps all caller spans and only writes events from wrapped transcripts.
`begin_trial` rejects overwrites and overlapping trials; `wrap` identifies each
role; `finish` flushes and returns the absolute JSONL path or an I/O error.
`render_text(input, output)` streams the readable view into a new file and checks
its writes and flush. Capture is synchronous, buffered, and retains only active
logical operations, not the entire run. Incomplete operations are marked and
rejected by the reader; an interrupted run is not a complete transcript.
`render_text_with_annotations(input, output, annotate)` also accepts a callback
that supplies a label and borrowed JSON description for selected logical records.
The example matches both the operation's purpose and the statement digest before
adding the public statement to the readable output.

Contexts are captured anew at each operation. The full enclosing span names and
fields appear in `spans`; `context` also exposes recorded stage, round, layer,
group, and coordinate fields. Semantic labels include statement commitments,
sumcheck polynomials/challenges, product-tree roots, opening evaluations, and
Ligerito roots, folds, queries, and out-of-domain points. Prime-sampling and
rejection contexts remain inside the nested byte steps.

The example and U32/U64/U128 benchmarks share these utilities. Each benchmark
trial, including warmups, gets a unique JSONL file and readable text companion.
The existing runner forwards `--fiat-shamir-transcript-logs=true|false`; default
is true. Disabled capture produces neither transcript artifact.

Review these outputs before attempting `f2z-benchmark` parity. This repository
is authoritative, and `src/bitz/` remains removed.
