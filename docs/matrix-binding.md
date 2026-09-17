# Shared matrix binding and linear maps

Matrix binding computes `v = Σ M_kᵀ w_k`. Terminal verification computes the same bilinear form `Σ w_kᵀ M_k eq(y)` directly, without constructing `v`. Neither operation consumes a transcript or derives an initial claim from the witness.

## Implementation map

| Layer | Location | Responsibility |
|---|---|---|
| Validated CSC | `crates/circuit/src/linear_map/sparse.rs` | Topology, coefficient storage, borrowed columns and canonical constructors |
| Construction | `linear_map/builder.rs`, `coefficients.rs` | Owned node handles, exact coefficient labels, packed input ranges |
| Compilation | `linear_map/compile.rs` | Reachability, compact indices, independent-node levels, forward/reverse schedules |
| Execution | `linear_map/evaluate.rs`, `contraction.rs` | Generic arithmetic, mapped seeds, typed output, independent direction workspaces |
| Circuit adapter | `linear_map/circuit.rs` | Circuit recording and ordinary A/B/C output registration |
| Binary adjoints | `linear_map/binary.rs`, `binary_adjoint.rs` | Implicit-one maps, digests, streamed ranges, repeated/chained factors and corrections |
| MLE bridge | `src/sumcheck/bridge/` | Ordinary/prefix/independent row functionals, dense/block/repeated/composite coefficients and direct terminal evaluation |
| Opening protocol | `src/ligerito_flock.rs` | Transcripts, dual-basis conversion, bit planes, ring switching and Flock integration |

`src/f2map.rs`, `src/sparse_matrix.rs` and `crates/circuit/src/matrix_wengert.rs` retain compatibility exports. There is one production graph compiler and one shared forward/adjoint traversal. Existing dense, native/block, factored and composite MLE adapters remain the inputs to unified inner sumcheck.

## Construction and preparation

```rust
use circuit::linear_map::{WengertBuilder, FieldCoefficients, DenseColumns};
use field::{IntegerEmbedding, RingOps, Uint, create_prime_field};

let field = create_prime_field(Uint::<2>::from((1u128 << 127) - 1));
let mut builder = WengertBuilder::new(Vec::<u64>::new());
let x = builder.input();
let y = builder.input();
let expression = builder.linear_combination([(x, 3u64), (y, 7u64)]);
builder.output(expression);
let tape = builder.finish();
let mut prepared = tape.prepare(&field);
let weights = [field.from_integer(&5u64)];
let mut coefficients = field.zero_vec(2);
prepared.adjoint_into(&weights, &mut coefficients)?;
let columns = [field.from_integer(&11u64), field.from_integer(&13u64)];
let claim = prepared.evaluate_bilinear(&weights, &DenseColumns::new(&field, &columns))?;
```

The public construction API is:

```rust
pub trait CoefficientStore {
    type Ref<'a> where Self: 'a;
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool;
    fn get(&self, index: usize) -> Self::Ref<'_>;
    fn compact(&mut self, used: &[bool]) -> Option<Vec<usize>>;
}
pub trait StoreCoefficient<C>: CoefficientStore {
    fn store(&mut self, coefficient: C) -> usize;
}

pub struct WengertBuilder<S> { /* private */ }
pub struct WengertTape<S> { /* private */ }
pub struct NodeId { /* builder ownership and compact index */ }
pub struct OutputId(/* index */);
pub struct PackedInputs {
    pub columns: std::ops::Range<usize>,
    pub full: NodeId,
    pub low: NodeId,
}

impl<S: CoefficientStore> WengertBuilder<S> {
    pub fn new(coefficients: S) -> Self;
    pub fn input(&mut self) -> NodeId;
    pub fn zero(&mut self) -> NodeId;
    pub fn add(&mut self, lhs: NodeId, rhs: NodeId) -> NodeId;
    pub fn sub(&mut self, lhs: NodeId, rhs: NodeId) -> NodeId;
    pub fn sum(&mut self, inputs: &[NodeId]) -> NodeId;
    pub fn scale<C>(&mut self, input: NodeId, coefficient: C) -> NodeId
    where S: StoreCoefficient<C>;
    pub fn linear_combination<C>(&mut self, terms: impl IntoIterator<Item=(NodeId,C)>) -> NodeId
    where S: StoreCoefficient<C>;
    pub fn packed_inputs(&mut self, bits: usize, low_bits: usize) -> PackedInputs;
    pub fn output(&mut self, value: NodeId) -> OutputId;
    pub fn finish(self) -> WengertTape<S>;
}
```

`Vec<u64>` and `Vec<Z<L>>` store homogeneous coefficients. `IntegerTable` supports distinct declared `Z<L>` widths within one graph. `FieldCoefficients::new(&field)` retains the provider and accepts its elements. Integer tapes use `prepare(&field)` and may be reused across moduli; field-backed tapes use `prepare()` and borrow their coefficients. Separate storage types avoid overlapping integer/field preparation implementations.

Compilation never hashes coefficients or simplifies them using wrapping arithmetic. Storage owns optional interning/compaction. Circuit recording retains its integer interner and uses checked simplification, recording a graph operation if an attached coefficient expression exceeds its declared width. A constant-only overflowing expression fails explicitly.

Runtime elements still have a matching-context caller contract: `Fp<2>` alone does not identify a particular modulus. Keeping the provider in field-backed storage prevents preparing the tape against a different provider, but callers must supply elements from the correct context.

Packed aggregates represent `Σ 2^i x_i`. Their scalar columns are private; a zero-width low part is zero, and equal full/low widths alias the same aggregate. Circuit recording creates these ranges directly. It no longer constructs per-bit nodes and coefficient products only to discard them during compilation. The constant-one coordinate is an explicit ordinary input, so terminal evaluation supplies its actual column-functional value.

## Execution interfaces

```rust
pub trait ColumnValues<E>: Sync {
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool;
    fn scalar(&self, column: usize) -> E;
    // Σ_{k<len} 2^k x[first+k]
    fn power_sum(&self, first: usize, len: usize) -> E;
}

impl<F: RingOps + Sync> PreparedWengert<'_, F> {
    pub fn adjoint_into(&mut self, weights: &[F::Elem], out: &mut [F::Elem])
        -> Result<(), LinearMapError>;
    pub fn adjoint_map_into(&mut self, count: usize,
        seed: impl Fn(usize) -> F::Elem + Sync, out: &mut [F::Elem])
        -> Result<(), LinearMapError>;
    pub fn adjoint_map_storage_into<O: Send>(&mut self, count: usize,
        seed: impl Fn(usize) -> F::Elem + Sync, out: &mut [O],
        write: impl Fn(F::Elem) -> O + Sync) -> Result<(), LinearMapError>;
    pub fn evaluate_bilinear(&mut self, weights: &[F::Elem],
        columns: &impl ColumnValues<F::Elem>) -> Result<F::Elem, LinearMapError>;
    pub fn evaluate_bilinear_map(&mut self, count: usize,
        seed: impl Fn(usize) -> F::Elem + Sync,
        columns: &impl ColumnValues<F::Elem>) -> Result<F::Elem, LinearMapError>;
    pub fn power_runs(&self) -> Vec<PowerRun<F::Elem>>;
    pub fn workspace_bytes(&self) -> usize;
}
```

Dimensions are checked before execution mutates outputs. Prepared coefficients are contiguous field elements. Heterogeneous storage lookup stays outside traversal. Mapped seeds avoid row-triple staging; mapped stores fuse output conversion. Forward and reverse scratch allocate independently and are reused.

CSC uses `SparseMatrix<C> { topology: CscTopology, coefficients: Box<[C]> }`. Topology constructors validate offsets and sorted unique indices. `PreparedVirtualMap::from_topology` constructs an implicit-one map without allocating coefficient storage. `PreparedSparse<F,C>` provides mixed-MAC adjoints and direct bilinear evaluation. Signed native contraction retains cheap singleton ±1 paths and handles `i64::MIN` through unsigned magnitude.

## Bridge and callers

```rust
pub(crate) trait PreparedBinding<F: RingOps, R: ?Sized> {
    type Bound;
    fn bind_rows(&mut self, rows: &R) -> Result<Self::Bound, BindingError>;
    fn bind_rows_into(&mut self, rows: &R, out: &mut Self::Bound)
        -> Result<(), BindingError>;
    fn evaluate_bound(&mut self, rows: &R, column_point: &[F::Elem])
        -> Result<F::Elem, BindingError>;
}
```

- `NativeBinding` accepts equality points, skipped-prefix factors, explicit weights and independently weighted A/B/C rows. It returns existing `NativeWeights::{Dense,Blocks}` and retains reusable row scratch. Ordinary/prefix PIOP dispatch passes the witness directly to inner sumcheck; binding no longer takes and returns it unchanged.
- Generic CSC binding shares column scheduling and coefficient-specific arithmetic. The explicit-row `PreparedSparse` bridge returns an owned field vector and evaluates against split equality factors directly.
- SHA local collapse lives in `bridge/repeated.rs`. SHA's existing factor owner keeps local coefficients separate from instance weights and exposes its factored MLE adapter.
- `CompositeBinding` owns no challenge-dependent result scratch. It fills a `CompositeCoefficients` owner containing local/instance factors, a single typed P-256 tail, constant correction and geometric-run metadata. Corrections accumulate at aliases and split affected runs. ECDSA delegates binding and direct evaluation here.
- Binary opening adjoints retain repeated/chained factors, prepared GF128 multipliers and factored tails. `fill_range` supports unaligned ranges with bounded scalar edges and the existing packed interior kernel. Opening-specific plane and transcript logic remains in the opening layer.

## Performance decisions

The circuit adapter preserves its existing FIOS Montgomery multiplication kernel while sharing generic traversal. Packed output spans of at least 32 elements use two independent doubling chains, exposing instruction parallelism without changing column order. Forward evaluation skips private packed-column ranges and uses scalar access for one-element aggregates.

Sparse delayed MACs use public column counts; columns are independent and worker accumulators do not merge. The optional graph delayed-adjoint method bounds each node, reduces chunks of at most 65,536 products, and reduces before dependent nodes consume results. The production circuit adapter retains scalar node reduction: measured delayed-node and terminal-dot replacements did not improve these workloads. No claim is made that delayed reduction always wins.

Transcript/proof formats, challenge order, matrix digests, GF128 basis, logical domains, padding, chain direction and additive aliases are unchanged. Differential matrix/bilinear tests, structured-vs-expanded oracles, forward/reverse checks and protocol tampering tests qualify the migration. Timing and memory evidence is recorded separately in [the MacBook benchmark report](benchmarks/matrix-binding-2026-09-16/README.md).
