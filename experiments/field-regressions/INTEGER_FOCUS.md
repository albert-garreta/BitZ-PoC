# Two-limb MAC and bounded wide multiplication

This focused experiment removes the two regressions identified in the
[initial arithmetic campaign](results/arithmetic-arm-01/REPORT.md). Its selected
implementations remain experimental; production integration is a separate step.
The confirmation results are in
[integer-focus-arm-03/REPORT.md](results/integer-focus-arm-03/REPORT.md).

## Selected implementations

`campaign::integer::mac::<2, false>` uses two independent native `u128`
accumulators, seeded directly from the first pair of products. Later pairs add
their products to the corresponding lanes, then the lanes merge with wrapping
addition. Empty and one-term inputs return directly. This avoids the long high-limb
dependency chain in the earlier fused implementation. Thin forwarding functions
inline into the caller. Other limb counts and the checked diagnostic retain the
previous implementation.

The operation is a full-width dot product modulo 2^128. It also gives the exact
signed result when the caller's capacity bound excludes overflow. It does not
specialize on the signed-16-bit benchmark fixture or inspect operand values to
select a kernel. Equal input lengths are checked once. The unchecked two-limb
entry accepts arbitrary slice lengths; the previous implementation's 2^20-term
fixture limit is unnecessary for its wrapping contract.

`campaign::bounded_product` exposes the experimental batch interface:

```rust
enum PublicProductBound { FourLimbs, NineLimbs }
enum ProductBatchError {
    InputLengthMismatch,
    OutputLengthMismatch,
    ExceedsPublicBound,
}
struct PreparedProducts<'a> { /* borrowed inputs and exclusive outputs */ }

impl<'a> PreparedProducts<'a> {
    fn try_new(
        a: &'a [[u64; 9]], b: &'a [[u64; 9]],
        out: &'a mut [[u64; 18]], bound: PublicProductBound,
    ) -> Result<Self, ProductBatchError>;
    fn execute(&mut self);
    fn outputs(&self) -> &[[u64; 18]];
}

fn multiply_batch(
    a: &[[u64; 9]], b: &[[u64; 9]],
    out: &mut [[u64; 18]], bound: PublicProductBound,
) -> Result<(), ProductBatchError>;
```

Preparation checks lengths and the caller-supplied public bound once. Four limbs
means unsigned inputs below 2^256; it is not a signed four-limb representation.
Validation scans every upper limb of both inputs before reporting an error.
It verifies a declared width instead of discovering a width from operand values.
Immutable input borrows preserve that validation across executions.

Execution dispatches once per batch: four-limb schoolbook multiplication for
`FourLimbs`, direct-output nine-limb multiplication for `NineLimbs`. Both write
all 18 output limbs, including upper zeros for four-limb products. Neither
allocates or builds converted input buffers. One-shot validation plus execution is measured
and gated independently from already-prepared execution.

## Reproduce

```sh
python3 experiments/field-regressions/run.py \
  --suite integer-focus --seed-offset 3141593 \
  --out experiments/field-regressions/results/new-integer-focus-run
python3 experiments/field-regressions/gate.py \
  experiments/field-regressions/results/new-integer-focus-run
python3 -m unittest discover -s experiments/field-regressions -p 'test_*.py'
```

On the x86-64 measurement host, use the repository's pinned Rust toolchain and
populate the dependency cache before running the offline benchmark build:

```sh
cargo fetch --locked --manifest-path experiments/field-regressions/Cargo.toml
python3 experiments/field-regressions/run.py \
  --suite integer-focus --arch x86_64 --seed-offset 3141593 \
  --out experiments/field-regressions/results/integer-focus-x86-01 \
  --target-dir /tmp/f2z-integer-focus-target
python3 experiments/field-regressions/gate.py \
  experiments/field-regressions/results/integer-focus-x86-01
```

Use a new output directory for each run. The runner builds a native executable
on that host and freezes an x86-only coverage manifest. The integer kernels are
portable Rust and this focused suite requires no particular x86 instruction
extension. The wider `--suite arithmetic` campaign separately requires PCLMUL
for its GF comparisons. The ARM-selected candidates remain fixed for this first
x86 comparison; ARM timings do not establish x86 performance. A blocked gate is
a recorded result requiring investigation, not a reason to relax its threshold.

The runner freezes sources, executable, compiler flags, selections, and the
host-scoped manifest before timing. It uses five fresh processes with 32 paired
samples and shuffled variant order. Selected inconclusive cases receive one
predeclared retry of five processes with 64 samples. The existing 1% regression
gate applies to median/P95 confidence bounds and every process median. See the
[measurement policy](README.md) for definitions and limits.

`integer_focus_cases.json` requires 217 variant cases and 28 selected cases:
two MAC input families at eight sizes (including one term, odd tails, and
1,048,576 terms), and two product bounds at three sizes with both batch APIs.
The old slow kernels, alternate accumulator schedules, direct four-limb output,
and Comba multiplication remain controls. Candidate selection uses development
data; confirmation uses fresh seeds and a frozen executable.

Every timed input is checked against independent BigUint arithmetic. Additional
checks cover carry boundaries, all-one words, signed boundaries, empty and odd
batches, rejected bounds, length mismatches, and repeated output overwrite.
Signed-16-bit MAC fixtures additionally use an exact BigInt oracle. Allocation
audits run outside timing. These checks establish arithmetic correctness for the
tested cases, not a constant-time certification or full-prover speedup.
