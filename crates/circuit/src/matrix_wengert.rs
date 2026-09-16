//! Modulus-independent reverse-mode tape for `r * (A + x B + x^2 C)`.
//!
//! The circuit is replayed once without witness values or a field modulus.
//! Z-side linear arithmetic is recorded as a Wengert graph, then dead nodes are
//! removed and the remaining graph is transposed and ordered by reverse depth.
//! Applying the finished tape is reverse-mode automatic differentiation of
//! `r * (A + x B + x^2 C) * w`. Nodes at one depth write disjoint adjoints, so
//! sufficiently wide depths are evaluated in parallel without atomics.

use field::ModRingCtx;

use std::collections::HashMap;
use std::error::Error;
use std::fmt::{self, Display};
use std::iter::Sum;
use std::mem::size_of;
use std::ops::{Add, AddAssign, Mul, Neg, Sub, SubAssign};
use std::sync::{Arc, Mutex};

use field::{FpCtx, IntegerEmbedding, RingOps, Uint, create_prime_field};
use num_traits::{One, Zero};
use rayon::prelude::*;

use crate::integer_storage::IntegerTable;
use crate::witgen::Z;
use crate::{BoolWitness, Circuit, HintResult, PackedBits, ScalarBits, WitnessContext};

const PARALLEL_LEVEL_THRESHOLD: usize = 1 << 16;
const PARALLEL_VECTOR_THRESHOLD: usize = 1 << 14;
const OUTPUT_CHUNK_SIZE: usize = 1 << 12;
const NO_NODE: u32 = u32::MAX;

/// A value-free Boolean handle used while recording the arithmetic tape.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct WengertBit;

impl From<bool> for WengertBit {
    fn from(_: bool) -> Self {
        Self
    }
}

impl BoolWitness for WengertBit {
    type Repr<const N: usize, const M: usize> = ScalarBits<Self, N>;
}

#[derive(Clone, Copy, Debug)]
struct RawTerm {
    node: u32,
    coefficient: u32,
}

impl RawTerm {
    fn is_zero(&self) -> bool {
        self.coefficient == 2
    }
}

#[derive(Debug)]
enum RawNode {
    Input,
    Sum(Box<[RawTerm]>),
}

#[derive(Clone, Copy, Debug)]
enum RootKind {
    A,
    B,
    C,
}

#[derive(Debug)]
struct RawRoot {
    row: u32,
    kind: RootKind,
    term: RawTerm,
}

/// Builds the symbolic integer-arithmetic graph as the circuit is replayed.
/// For example, `3*a + 2*b` records a sum pointing to `a` and `b` with weights
/// 3 and 2. Each constraint records which expressions form its A/B/C sides.
/// [`WengertGenerator::finish`] prunes unused nodes and arranges this graph into
/// a [`WengertTape`] for propagating row weights backward to witness columns.
#[derive(Debug)]
struct Recorder {
    /// Inputs and weighted sums in creation order; each node's ID is its index.
    nodes: Vec<RawNode>,
    /// Maps each integer-witness column to its input node; column 0 is constant one.
    input_nodes: Vec<u32>,
    /// Three roots per constraint: its A/B/C expressions, row index, and coefficients.
    roots: Vec<RawRoot>,
    /// Bit-column ranges and their full/low packed sums, for later power-of-two expansion.
    power_groups: Vec<RawPowerGroup>,
    /// Number of constraints recorded so far; also the next constraint's row index.
    constraints: usize,
    coefficients: IntegerTable,
    coefficient_map: HashMap<Vec<u64>, u32>,
}

#[derive(Debug)]
struct RawPowerGroup {
    first_column: u32,
    len: u32,
    low_len: u32,
    full_node: u32,
    low_node: u32,
}

impl Recorder {
    fn new() -> Self {
        let mut recorder = Self {
            // Integer-witness column zero is the implicit constant one.
            nodes: vec![RawNode::Input],
            input_nodes: vec![0],
            roots: Vec::new(),
            power_groups: Vec::new(),
            constraints: 0,
            coefficients: IntegerTable::default(),
            coefficient_map: HashMap::new(),
        };
        recorder.intern(Z::<1>::ONE);
        recorder.intern(-Z::<1>::ONE);
        recorder.intern(Z::<1>::ZERO);
        recorder
    }

    // Coefficients are public circuit structure. Normalize only the hash key to
    // deduplicate equivalent values from different gadget widths; storage keeps L.
    fn intern<const L: usize>(&mut self, value: Z<L>) -> u32 {
        let words = value.as_words();
        let mut len = L;
        while len > 1 {
            let sign = 0u64.wrapping_sub(words[len - 2] >> 63);
            if words[len - 1] != sign {
                break;
            }
            len -= 1;
        }
        let key = &words[..len];
        if let Some(index) = self.coefficient_map.get(key) {
            return *index;
        }
        let index = u32::try_from(self.coefficients.len()).expect("too many tape coefficients");
        self.coefficients.push(value);
        self.coefficient_map.insert(key.to_vec(), index);
        index
    }

    fn push_input(&mut self) -> u32 {
        let node = u32::try_from(self.nodes.len()).expect("too many Wengert nodes");
        self.nodes.push(RawNode::Input);
        self.input_nodes.push(node);
        node
    }

    fn push_sum(&mut self, terms: Box<[RawTerm]>) -> u32 {
        debug_assert!(terms.len() >= 2);
        let node = u32::try_from(self.nodes.len()).expect("too many Wengert nodes");
        debug_assert!(terms.iter().all(|term| term.node < node));
        self.nodes.push(RawNode::Sum(terms));
        node
    }
}

#[derive(Debug)]
struct SharedRecorder(Mutex<Option<Recorder>>);

impl SharedRecorder {
    fn with_mut<R>(&self, apply: impl FnOnce(&mut Recorder) -> R) -> R {
        let mut guard = self.0.lock().expect("Wengert recorder lock poisoned");
        apply(
            guard
                .as_mut()
                .expect("the Wengert generator has already been finished"),
        )
    }
}

/// A scaled Wengert handle with the gadget-local coefficient width.
#[derive(Clone, Debug)]
pub struct WengertValue<const LIMBS: usize> {
    location: ValueLocation,
    coefficient: Z<LIMBS>,
}

#[derive(Clone, Debug)]
enum ValueLocation {
    /// A coefficient times the implicit constant-one input.
    Constant,
    Node {
        recorder: Arc<SharedRecorder>,
        node: u32,
    },
}

impl<const LIMBS: usize> WengertValue<LIMBS> {
    fn attached(recorder: Arc<SharedRecorder>, node: u32, coefficient: Z<LIMBS>) -> Self {
        Self {
            location: ValueLocation::Node { recorder, node },
            coefficient,
        }
    }

    fn recorder(&self) -> Option<&Arc<SharedRecorder>> {
        match &self.location {
            ValueLocation::Constant => None,
            ValueLocation::Node { recorder, .. } => Some(recorder),
        }
    }

    fn raw_term(&self, recorder: &mut Recorder) -> RawTerm {
        let node = match self.location {
            ValueLocation::Constant => 0,
            ValueLocation::Node { node, .. } => node,
        };
        RawTerm {
            node,
            coefficient: recorder.intern(self.coefficient),
        }
    }

    fn same_recorder(left: &Arc<SharedRecorder>, right: &Arc<SharedRecorder>) {
        assert!(
            Arc::ptr_eq(left, right),
            "cannot combine values from different Wengert generators"
        );
    }
}

impl<const LIMBS: usize> From<Z<LIMBS>> for WengertValue<LIMBS> {
    fn from(coefficient: Z<LIMBS>) -> Self {
        Self {
            location: ValueLocation::Constant,
            coefficient,
        }
    }
}

impl<const LIMBS: usize> Zero for WengertValue<LIMBS> {
    fn zero() -> Self {
        Self::from(Z::zero())
    }

    fn is_zero(&self) -> bool {
        self.coefficient.is_zero()
    }
}

impl<const LIMBS: usize> Add for WengertValue<LIMBS> {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        if self.is_zero() {
            return rhs;
        }
        if rhs.is_zero() {
            return self;
        }

        match (&self.location, &rhs.location) {
            (ValueLocation::Constant, ValueLocation::Constant) => {
                Self::from(self.coefficient + rhs.coefficient)
            }
            (
                ValueLocation::Node {
                    recorder: left,
                    node: left_node,
                },
                ValueLocation::Node {
                    recorder: right,
                    node: right_node,
                },
            ) if left_node == right_node => {
                Self::same_recorder(left, right);
                Self::attached(left.clone(), *left_node, self.coefficient + rhs.coefficient)
            }
            _ => {
                let recorder = self
                    .recorder()
                    .or_else(|| rhs.recorder())
                    .expect("nonconstant Wengert sum needs a recorder")
                    .clone();
                if let Some(other) = self.recorder() {
                    Self::same_recorder(&recorder, other);
                }
                if let Some(other) = rhs.recorder() {
                    Self::same_recorder(&recorder, other);
                }
                let node = recorder.with_mut(|tape| {
                    let terms = [self.raw_term(tape), rhs.raw_term(tape)].into();
                    tape.push_sum(terms)
                });
                Self::attached(recorder, node, Z::one())
            }
        }
    }
}

impl<const LIMBS: usize> AddAssign for WengertValue<LIMBS> {
    fn add_assign(&mut self, rhs: Self) {
        *self = self.clone() + rhs;
    }
}

impl<const LIMBS: usize> Neg for WengertValue<LIMBS> {
    type Output = Self;

    fn neg(mut self) -> Self::Output {
        self.coefficient = -self.coefficient;
        self
    }
}

impl<const LIMBS: usize> Sub for WengertValue<LIMBS> {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        self + -rhs
    }
}

impl<const LIMBS: usize> SubAssign for WengertValue<LIMBS> {
    fn sub_assign(&mut self, rhs: Self) {
        *self = self.clone() - rhs;
    }
}

impl<const LIMBS: usize> Mul<Z<LIMBS>> for WengertValue<LIMBS> {
    type Output = Self;

    fn mul(mut self, rhs: Z<LIMBS>) -> Self::Output {
        self.coefficient = self.coefficient * rhs;
        self
    }
}

impl<const LIMBS: usize> Sum for WengertValue<LIMBS> {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(Self::zero(), Add::add)
    }
}

#[derive(Clone, Copy, Debug)]
struct Edge {
    dependent: u32,
    coefficient: u32,
}

#[derive(Clone, Copy, Debug)]
struct Root {
    row: u32,
    coefficient: u32,
    kind: RootKind,
}

/// One term `coefficient · value[source]` of a sum node's forward program.
#[derive(Clone, Copy, Debug)]
struct Term {
    source: u32,
    coefficient: u32,
}

#[derive(Clone, Copy, Debug)]
struct PowerGroup {
    first_column: u32,
    len: u32,
    low_len: u32,
    full_node: u32,
    low_node: u32,
}

/// A compact, modulus-independent reverse-mode program.
#[derive(Debug)]
pub struct WengertTape {
    /// Number of constraint rows, hence the number of input row weights.
    constraints: usize,
    /// Witness column -> node ID, including constant column 0; `NO_NODE` if pruned.
    input_nodes: Box<[u32]>,
    /// Internal node ranges per reverse-pass level; nodes in a level can run in parallel.
    level_offsets: Box<[u32]>,
    /// Node `n` reads `edges[edge_offsets[n]..edge_offsets[n + 1]]`.
    edge_offsets: Box<[u32]>,
    /// Dependent nodes and coefficient indices for propagating weights backward.
    edges: Box<[Edge]>,
    /// Node `n` reads `roots[root_offsets[n]..root_offsets[n + 1]]`.
    root_offsets: Box<[u32]>,
    /// A/B/C row seeds and coefficient indices, grouped by receiving node.
    roots: Box<[Root]>,
    /// Packed bit sums expanded into column weights using powers of two.
    power_groups: Box<[PowerGroup]>,
    /// Shared integer coefficients, before reduction; entries 0 and 1 are +1 and -1.
    coefficients: IntegerTable,
    /// Forward program (local F2Z addition): per live sum node, in the
    /// reverse-level numbering, its `(source, coefficient)` terms over live
    /// sources — the same edges as `edges`, grouped by dependent instead of
    /// by source. A power group's sum has no terms here: its bit inputs are
    /// dead in the graph, and [`PreparedWengertEvaluator::apply_forward_weighted`]
    /// takes the group's geometric input sum from the caller instead.
    forward_offsets: Box<[u32]>,
    forward_terms: Box<[Term]>,
}

/// Column values of a forward pass, in the evaluator's Montgomery form.
///
/// The caller owns the integer-witness column values `x`; the tape only ever
/// needs a scalar column on its own, or the geometric sum
/// `Σ_{k < len} 2^k · x_{first + k}` of one power group's columns. Local F2Z
/// addition; not part of upstream f2z-benchmark.
pub trait ForwardColumns: Sync {
    /// `x_column` for a scalar (non-power-group) column.
    fn scalar(&self, column: usize) -> [u64; 2];

    /// `Σ_{k < len} 2^k · x_{first + k}`.
    fn power_sum(&self, first: usize, len: usize) -> [u64; 2];
}

/// Evaluates the transpose of the tape's linear map modulo a fixed prime `q`.
/// Let `A, B, C ∈ Z^(m×n)` denote the implicit coefficient maps encoded by the
/// graph. For row weights `w_A, w_B, w_C ∈ F_q^m`, [`Self::apply_weighted`]
/// computes `v ∈ F_q^n`:
///
/// ```text
/// v = Aᵀ w_A + Bᵀ w_B + Cᵀ w_C,
/// v_j = Σ_i (w_A[i] A[i,j] + w_B[i] B[i,j] + w_C[i] C[i,j]) mod q.
/// ```
///
/// Equivalently, for a symbolic assignment `h`, the reverse pass computes
///
/// ```text
/// Φ(h) = ⟨w_A, Ah⟩ + ⟨w_B, Bh⟩ + ⟨w_C, Ch⟩,
/// v = ∇_h Φ(h).
/// ```
///
/// Since `Φ` is linear, its gradient is independent of `h`; no witness values
/// are needed. [`Self::apply`] specializes to `(w_A, w_B, w_C) = (r, x r, x² r)`,
/// giving `v = (A + x B + x² C)ᵀ r` through the same graph.
///
/// [`WengertTape::prepare`] caches each distinct graph coefficient as
/// `c̄ = c R mod q`, where `R = 2^128`. Inputs and outputs likewise use
/// Montgomery form: `w̄ = R w mod q` and `v̄ = R v mod q`.
/// Repeated calls reuse the coefficient conversion and reverse-pass storage.
#[derive(Debug)]
pub struct PreparedWengertEvaluator<'a> {
    tape: &'a WengertTape,
    field: FpCtx<2>,
    coefficients: Vec<[u64; 2]>,
    weighted_challenges: Vec<[[u64; 2]; 3]>,
    adjoints: Vec<[u64; 2]>,
    /// The reverse pass's column outputs; allocated by the first reverse
    /// pass (a forward-only evaluator never materializes it).
    output: Vec<[u64; 2]>,
    powers_of_two: Vec<[u64; 2]>,
    /// The forward pass's node values (every live node); allocated by the
    /// first forward pass.
    forward_values: Vec<[u64; 2]>,
}

impl WengertTape {
    fn from_recorder(recorder: Recorder) -> Self {
        let Recorder {
            nodes,
            input_nodes,
            roots: raw_roots,
            power_groups: raw_power_groups,
            constraints,
            coefficients: recorded_coefficients,
            coefficient_map: _,
        } = recorder;
        let node_count = nodes.len();

        // Reverse reachability removes arithmetic that cannot affect A/B/C.
        let mut live = vec![false; node_count];
        for root in &raw_roots {
            if !root.term.is_zero() {
                live[root.term.node as usize] = true;
            }
        }
        for node in (0..node_count).rev() {
            if !live[node] {
                continue;
            }
            if let RawNode::Sum(terms) = &nodes[node] {
                for term in terms {
                    if !term.is_zero() {
                        live[term.node as usize] = true;
                    }
                }
            }
        }

        // Packed lifts are differentiated as power groups, so their private
        // scalar inputs and per-bit edges are redundant in the reverse graph.
        for group in &raw_power_groups {
            let start = group.first_column as usize;
            let end = start + group.len as usize;
            for &input in &input_nodes[start..end] {
                live[input as usize] = false;
            }
        }

        // A source is one level after all dependents in the reverse pass.
        let mut depth = vec![0_u32; node_count];
        for target in (0..node_count).rev() {
            if !live[target] {
                continue;
            }
            if let RawNode::Sum(terms) = &nodes[target] {
                for term in terms {
                    if live[term.node as usize] && !term.is_zero() {
                        depth[term.node as usize] = depth[term.node as usize].max(
                            depth[target]
                                .checked_add(1)
                                .expect("Wengert depth overflow"),
                        );
                    }
                }
            }
        }

        let live_count = live.iter().filter(|value| **value).count();
        let live_internal_count = nodes
            .iter()
            .zip(&live)
            .filter(|(node, live)| **live && matches!(node, RawNode::Sum(_)))
            .count();
        let level_count = depth
            .iter()
            .zip(nodes.iter().zip(&live))
            .filter_map(|(depth, (node, live))| {
                (*live && matches!(node, RawNode::Sum(_))).then_some(*depth as usize + 1)
            })
            .max()
            .unwrap_or(0);
        let mut level_offsets = vec![0_u32; level_count + 1];
        for ((&node_depth, node), &is_live) in depth.iter().zip(&nodes).zip(&live) {
            if is_live && matches!(node, RawNode::Sum(_)) {
                level_offsets[node_depth as usize + 1] += 1;
            }
        }
        for level in 0..level_count {
            level_offsets[level + 1] += level_offsets[level];
        }
        debug_assert_eq!(
            level_offsets.last().copied().unwrap_or(0) as usize,
            live_internal_count
        );

        let mut ordering = vec![0_u32; live_count];
        let mut level_cursors = level_offsets[..level_count].to_vec();
        for (old, ((&node_depth, node), &is_live)) in
            depth.iter().zip(&nodes).zip(&live).enumerate()
        {
            if is_live && matches!(node, RawNode::Sum(_)) {
                let cursor = &mut level_cursors[node_depth as usize];
                ordering[*cursor as usize] = old as u32;
                *cursor += 1;
            }
        }
        let mut input_cursor = live_internal_count;
        for &old in &input_nodes {
            if live[old as usize] {
                ordering[input_cursor] = old;
                input_cursor += 1;
            }
        }
        debug_assert_eq!(input_cursor, live_count);
        let mut old_to_new = vec![NO_NODE; node_count];
        for (new, &old) in ordering.iter().enumerate() {
            old_to_new[old as usize] = new as u32;
        }

        let mut coefficients = IntegerTable::default();
        recorded_coefficients.copy_row_to(0, &mut coefficients);
        recorded_coefficients.copy_row_to(1, &mut coefficients);
        let mut remap = vec![u32::MAX; recorded_coefficients.len()];
        remap[0] = 0;
        remap[1] = 1;
        let mut intern = |index: u32| {
            let slot = &mut remap[index as usize];
            if *slot == u32::MAX {
                *slot = u32::try_from(coefficients.len()).expect("too many tape coefficients");
                recorded_coefficients.copy_row_to(index as usize, &mut coefficients);
            }
            *slot
        };

        let mut edge_counts = vec![0_u32; live_count];
        for (target, node) in nodes.iter().enumerate() {
            if !live[target] {
                continue;
            }
            if let RawNode::Sum(terms) = node {
                for term in terms {
                    if live[term.node as usize] && !term.is_zero() {
                        edge_counts[old_to_new[term.node as usize] as usize] += 1;
                    }
                }
            }
        }
        let edge_offsets = prefix_offsets(&edge_counts, "too many Wengert edges");
        let mut edge_cursors = edge_offsets[..live_count].to_vec();
        let mut edges = vec![
            Edge {
                dependent: 0,
                coefficient: 0,
            };
            edge_offsets[live_count] as usize
        ];
        for (target, node) in nodes.iter().enumerate() {
            if !live[target] {
                continue;
            }
            if let RawNode::Sum(terms) = node {
                for term in terms {
                    if !live[term.node as usize] || term.is_zero() {
                        continue;
                    }
                    let source = old_to_new[term.node as usize] as usize;
                    let cursor = &mut edge_cursors[source];
                    edges[*cursor as usize] = Edge {
                        dependent: old_to_new[target],
                        coefficient: intern(term.coefficient),
                    };
                    *cursor += 1;
                }
            }
        }

        let mut root_counts = vec![0_u32; live_count];
        for root in &raw_roots {
            if !root.term.is_zero() {
                root_counts[old_to_new[root.term.node as usize] as usize] += 1;
            }
        }
        let root_offsets = prefix_offsets(&root_counts, "too many Wengert roots");
        let mut root_cursors = root_offsets[..live_count].to_vec();
        let mut roots = vec![
            Root {
                row: 0,
                coefficient: 0,
                kind: RootKind::A,
            };
            root_offsets[live_count] as usize
        ];
        for root in raw_roots {
            if root.term.is_zero() {
                continue;
            }
            let node = old_to_new[root.term.node as usize] as usize;
            let cursor = &mut root_cursors[node];
            roots[*cursor as usize] = Root {
                row: root.row,
                coefficient: intern(root.term.coefficient),
                kind: root.kind,
            };
            *cursor += 1;
        }

        // The forward program: the same live, nonzero terms as `edges`,
        // grouped by their sum node (the power groups' inputs are dead, so a
        // power group's sum keeps no terms).
        let mut forward_offsets = Vec::with_capacity(live_internal_count + 1);
        forward_offsets.push(0_u32);
        let mut forward_terms = Vec::with_capacity(edges.len());
        for &old in &ordering[..live_internal_count] {
            if let RawNode::Sum(terms) = &nodes[old as usize] {
                for term in terms {
                    if !live[term.node as usize] || term.is_zero() {
                        continue;
                    }
                    forward_terms.push(Term {
                        source: old_to_new[term.node as usize],
                        coefficient: intern(term.coefficient),
                    });
                }
            }
            forward_offsets
                .push(u32::try_from(forward_terms.len()).expect("too many Wengert terms"));
        }
        debug_assert_eq!(forward_terms.len(), edges.len());

        Self {
            constraints,
            input_nodes: input_nodes
                .into_iter()
                .map(|node| old_to_new[node as usize])
                .collect(),
            level_offsets: level_offsets.into_boxed_slice(),
            edge_offsets: edge_offsets.into_boxed_slice(),
            edges: edges.into_boxed_slice(),
            root_offsets: root_offsets.into_boxed_slice(),
            roots: roots.into_boxed_slice(),
            power_groups: raw_power_groups
                .into_iter()
                .map(|group| PowerGroup {
                    first_column: group.first_column,
                    len: group.len,
                    low_len: group.low_len,
                    full_node: old_to_new[group.full_node as usize],
                    low_node: if group.low_node == NO_NODE {
                        NO_NODE
                    } else {
                        old_to_new[group.low_node as usize]
                    },
                })
                .collect(),
            coefficients,
            forward_offsets: forward_offsets.into_boxed_slice(),
            forward_terms: forward_terms.into_boxed_slice(),
        }
    }

    /// Number of R1CS rows, and therefore required `r` elements.
    pub const fn row_count(&self) -> usize {
        self.constraints
    }

    /// Number of integer-witness columns, including constant column zero.
    pub const fn column_count(&self) -> usize {
        self.input_nodes.len()
    }

    /// Number of live arithmetic and input nodes after pruning.
    pub const fn node_count(&self) -> usize {
        self.edge_offsets.len() - 1
    }

    /// Number of differentiated tape edges.
    pub const fn edge_count(&self) -> usize {
        self.edges.len()
    }

    /// Number of reverse-depth batches.
    pub const fn level_count(&self) -> usize {
        self.level_offsets.len() - 1
    }

    /// Number of distinct modulus-independent integer coefficients.
    pub fn coefficient_count(&self) -> usize {
        self.coefficients.len()
    }

    /// Bytes occupied by the tape's indexed payload and coefficient words.
    pub fn payload_bytes(&self) -> usize {
        self.input_nodes.len() * size_of::<u32>()
            + self.level_offsets.len() * size_of::<u32>()
            + self.edge_offsets.len() * size_of::<u32>()
            + self.edges.len() * size_of::<Edge>()
            + self.root_offsets.len() * size_of::<u32>()
            + self.roots.len() * size_of::<Root>()
            + self.power_groups.len() * size_of::<PowerGroup>()
            + self.coefficients.payload_bytes()
            + self.forward_offsets.len() * size_of::<u32>()
            + self.forward_terms.len() * size_of::<Term>()
    }

    /// Prepares this tape for repeated evaluation modulo `modulus`.
    pub fn prepare(
        &self,
        modulus: &ModRingCtx<2>,
    ) -> Result<PreparedWengertEvaluator<'_>, WengertApplyError> {
        self.prepare_inner(modulus, None)
    }

    fn prepare_inner(
        &self,
        modulus: &ModRingCtx<2>,
        force_parallel: Option<bool>,
    ) -> Result<PreparedWengertEvaluator<'_>, WengertApplyError> {
        let modulus_words = *modulus.modulus().as_words();
        if modulus_words[0] & 1 == 0 {
            return Err(WengertApplyError::EvenModulus);
        }
        let field = create_prime_field(Uint::from_words(modulus_words));
        let projection =
            field::PreparedSignedProjection::new(field.clone(), self.coefficients.max_limbs());
        let view = self.coefficients.view();
        let project = |index: usize| {
            *projection
                .project(&view[index])
                .as_montgomery_integer()
                .as_words()
        };
        let coefficients: Vec<_> =
            if force_parallel.unwrap_or(self.coefficients.len() >= PARALLEL_VECTOR_THRESHOLD) {
                (0..self.coefficients.len())
                    .into_par_iter()
                    .map(project)
                    .collect()
            } else {
                (0..self.coefficients.len()).map(project).collect()
            };
        debug_assert_eq!(
            coefficients[0],
            *field.one().as_montgomery_integer().as_words()
        );
        let max_power_group_len = self
            .power_groups
            .iter()
            .map(|group| group.len as usize)
            .max()
            .unwrap_or(0);
        let mut powers_of_two = Vec::with_capacity(max_power_group_len);
        if max_power_group_len != 0 {
            let mut power = coefficients[0];
            for _ in 0..max_power_group_len {
                powers_of_two.push(power);
                power = add_representatives(power, power, &field);
            }
        }
        Ok(PreparedWengertEvaluator {
            tape: self,
            field,
            coefficients,
            weighted_challenges: vec![[[0; 2]; 3]; self.row_count()],
            adjoints: vec![[0; 2]; self.level_offsets.last().copied().unwrap_or(0) as usize],
            output: Vec::new(),
            powers_of_two,
            forward_values: Vec::new(),
        })
    }

    /// Computes `r * (A + x B + x^2 C)` modulo a runtime two-limb prime.
    pub fn apply(
        &self,
        challenges: &[[u64; 2]],
        x: [u64; 2],
        modulus: &ModRingCtx<2>,
    ) -> Result<Vec<[u64; 2]>, WengertApplyError> {
        let mut output = Vec::new();
        self.apply_into(challenges, x, modulus, &mut output)?;
        Ok(output)
    }

    /// Computes the product into a reusable output allocation.
    pub fn apply_into(
        &self,
        challenges: &[[u64; 2]],
        x: [u64; 2],
        modulus: &ModRingCtx<2>,
        output: &mut Vec<[u64; 2]>,
    ) -> Result<(), WengertApplyError> {
        self.apply_inner(challenges, x, modulus, output, None)
    }

    fn apply_inner(
        &self,
        challenges: &[[u64; 2]],
        x: [u64; 2],
        modulus: &ModRingCtx<2>,
        output: &mut Vec<[u64; 2]>,
        force_parallel: Option<bool>,
    ) -> Result<(), WengertApplyError> {
        // Prepare arithmetic modulo q = modulus: cache each tape coefficient c
        // as c̄ = R c mod q, with R = 2^128, and allocate reverse-pass buffers.
        let mut evaluator = self.prepare_inner(modulus, force_parallel)?;

        // Keep the row weights r_i = challenges[i] mod q in canonical form.
        // Multiplying by a cached coefficient c̄ = R c preserves this form:
        //   MontMul(r_i, c̄) = r_i (R c) R⁻¹ mod q = r_i c mod q.
        let reduced_challenges = parallel_map(challenges, force_parallel, |challenge| {
            *evaluator
                .field
                .reduce_integer(&Uint::from_words(*challenge))
                .as_words()
        });
        // Encode the matrix-batching challenge as x̄ = R x mod q.
        let montgomery_x = evaluator.to_montgomery(x);

        // Seed each row with canonical (r_i, r_i x, r_i x²) and propagate these
        // weights backward through the linear tape. Its additions and scaling by
        // Montgomery coefficients keep every intermediate and output canonical:
        //   output[j] = Σ_i r_i (A[i,j] + x B[i,j] + x² C[i,j]) mod q.
        evaluator.apply_inner(&reduced_challenges, montgomery_x, force_parallel)?;

        // Copy the canonical results into the caller's reusable allocation.
        output.clone_from(&evaluator.output);
        Ok(())
    }
}

struct ReverseContext<'a> {
    tape: &'a WengertTape,
    field: &'a FpCtx<2>,
    coefficients: &'a [[u64; 2]],
    weighted_challenges: &'a [[[u64; 2]; 3]],
}

impl ReverseContext<'_> {
    #[inline(always)]
    fn scale(&self, source: [u64; 2], coefficient: u32) -> [u64; 2] {
        if coefficient == 0 {
            source
        } else if coefficient == 1 {
            neg_representative(source, &self.field)
        } else {
            mul_representatives(source, self.coefficients[coefficient as usize], &self.field)
        }
    }

    #[inline(always)]
    fn evaluate(&self, node: usize, adjoints: &[[u64; 2]]) -> [u64; 2] {
        let root_start = self.tape.root_offsets[node] as usize;
        let root_end = self.tape.root_offsets[node + 1] as usize;
        let roots = &self.tape.roots[root_start..root_end];
        let edge_start = self.tape.edge_offsets[node] as usize;
        let edge_end = self.tape.edge_offsets[node + 1] as usize;
        let edges = &self.tape.edges[edge_start..edge_end];
        if roots.is_empty() && edges.len() == 1 {
            let edge = edges[0];
            debug_assert!((edge.dependent as usize) < adjoints.len());
            return self.scale(adjoints[edge.dependent as usize], edge.coefficient);
        }

        let mut value = [0_u64; 2];
        let mut initialized = false;
        for root in roots {
            let seed = self.weighted_challenges[root.row as usize][match root.kind {
                RootKind::A => 0,
                RootKind::B => 1,
                RootKind::C => 2,
            }];
            let contribution = self.scale(seed, root.coefficient);
            if initialized {
                value = add_representatives(value, contribution, &self.field);
            } else {
                value = contribution;
                initialized = true;
            }
        }
        for edge in edges {
            debug_assert!((edge.dependent as usize) < adjoints.len());
            let contribution = self.scale(adjoints[edge.dependent as usize], edge.coefficient);
            if initialized {
                value = add_representatives(value, contribution, &self.field);
            } else {
                value = contribution;
                initialized = true;
            }
        }
        value
    }
}

impl PreparedWengertEvaluator<'_> {
    /// Converts a canonical element into Montgomery form for this modulus.
    pub fn to_montgomery(&self, canonical: [u64; 2]) -> [u64; 2] {
        *self
            .field
            .from_integer(&Uint::from_words(canonical))
            .as_montgomery_integer()
            .as_words()
    }

    /// Converts a Montgomery element into canonical form for this modulus.
    pub fn from_montgomery(&self, montgomery: [u64; 2]) -> [u64; 2] {
        *self
            .field
            .to_integer(
                &self
                    .field
                    .from_montgomery_integer(Uint::from_words(montgomery)),
            )
            .as_words()
    }

    /// Bytes occupied by reduced coefficients and reusable apply vectors
    /// (the column output and the forward values count once allocated by
    /// their first pass).
    pub fn workspace_bytes(&self) -> usize {
        self.coefficients.len() * size_of::<[u64; 2]>()
            + self.weighted_challenges.len() * size_of::<[[u64; 2]; 3]>()
            + self.adjoints.len() * size_of::<[u64; 2]>()
            + self.output.len() * size_of::<[u64; 2]>()
            + self.powers_of_two.len() * size_of::<[u64; 2]>()
            + self.forward_values.len() * size_of::<[u64; 2]>()
    }

    /// Computes `r * (A + x B + x^2 C)` using Montgomery inputs and output.
    ///
    /// Every challenge and `x` must already be in Montgomery form for the
    /// prepared modulus. The output remains in Montgomery form. The returned
    /// slice remains valid until the next mutable use of this evaluator.
    pub fn apply(
        &mut self,
        challenges: &[[u64; 2]],
        x: [u64; 2],
    ) -> Result<&[[u64; 2]], WengertApplyError> {
        self.apply_inner(challenges, x, None)?;
        Ok(&self.output)
    }

    /// Row weights must be reduced modulo q; they may be canonical or Montgomery.
    /// The output retains their representation. The batching challenge x always
    /// uses Montgomery form, so MontMul(r_i, x̄) preserves the row weights' form.
    /// Public `apply` supplies Montgomery weights; `WengertTape` supplies canonical
    /// weights to its temporary evaluator and copies out the canonical results.
    fn apply_inner(
        &mut self,
        challenges: &[[u64; 2]],
        x: [u64; 2],
        force_parallel: Option<bool>,
    ) -> Result<(), WengertApplyError> {
        let Self {
            tape,
            field,
            weighted_challenges,
            ..
        } = self;
        if challenges.len() != tape.constraints {
            return Err(WengertApplyError::ChallengeLength {
                expected: tape.constraints,
                actual: challenges.len(),
            });
        }
        let x_squared = mul_representatives(x, x, field);
        let prepare_challenge = |challenge: &[u64; 2]| {
            let r = *challenge;
            let rx = mul_representatives(r, x, field);
            [r, rx, mul_representatives(r, x_squared, field)]
        };
        let parallel = force_parallel.unwrap_or_else(|| {
            rayon::current_num_threads() > 1
                && weighted_challenges.len() >= PARALLEL_VECTOR_THRESHOLD
        });
        if parallel {
            weighted_challenges
                .par_iter_mut()
                .zip(challenges.par_iter())
                .for_each(|(weighted, challenge)| *weighted = prepare_challenge(challenge));
        } else {
            weighted_challenges
                .iter_mut()
                .zip(challenges.iter())
                .for_each(|(weighted, challenge)| *weighted = prepare_challenge(challenge));
        }

        self.run_reverse(force_parallel);
        Ok(())
    }

    /// Computes `Σ_row (w_A · A_row + w_B · B_row + w_C · C_row)` from one Montgomery
    /// weight triple per row, in the Montgomery form of [`Self::apply`]. The triple
    /// replaces the `r, r·x, r·x²` that [`Self::apply`] derives from one challenge,
    /// so rows can be weighted per matrix (a linear row weighted on `C` alone, say).
    /// Local F2Z addition; not part of upstream f2z-benchmark.
    pub fn apply_weighted(
        &mut self,
        weights: &[[[u64; 2]; 3]],
    ) -> Result<&[[u64; 2]], WengertApplyError> {
        if weights.len() != self.tape.constraints {
            return Err(WengertApplyError::ChallengeLength {
                expected: self.tape.constraints,
                actual: weights.len(),
            });
        }
        self.weighted_challenges.copy_from_slice(weights);
        self.run_reverse(None);
        Ok(&self.output)
    }

    /// `Σ_row (w_A · A_row + w_B · B_row + w_C · C_row) · x` for the column
    /// values `x` of `columns`, by a FORWARD pass: every sum node takes
    /// `Σ coefficient · value[source]` over its terms, a power group's sum
    /// takes the caller's geometric input sum, and the roots close the sum
    /// with the row weights. It is the same bilinear form
    /// `⟨w, (A + …) x⟩ = ⟨(A + …)ᵀ w, x⟩` that [`Self::apply_weighted`]
    /// followed by a dot product with `x` computes (the reverse pass is the
    /// transpose of this program, edge for edge), so the two agree
    /// exactly; the forward pass never materializes the column vector.
    /// Weights, column values and the result are in the evaluator's
    /// Montgomery form. Local F2Z addition; not part of upstream
    /// f2z-benchmark.
    pub fn apply_forward_weighted<V: ForwardColumns>(
        &mut self,
        weights: &[[[u64; 2]; 3]],
        columns: &V,
    ) -> Result<[u64; 2], WengertApplyError> {
        if weights.len() != self.tape.constraints {
            return Err(WengertApplyError::ChallengeLength {
                expected: self.tape.constraints,
                actual: weights.len(),
            });
        }
        let Self {
            tape,
            field,
            coefficients,
            forward_values: values,
            ..
        } = self;
        values.clear();
        values.resize(tape.node_count(), [0; 2]);
        for (column, &node) in tape.input_nodes.iter().enumerate() {
            if node != NO_NODE {
                values[node as usize] = columns.scalar(column);
            }
        }
        for group in tape.power_groups.iter() {
            let first = group.first_column as usize;
            if group.full_node != NO_NODE {
                values[group.full_node as usize] = columns.power_sum(first, group.len as usize);
            }
            if group.low_node != NO_NODE {
                values[group.low_node as usize] = columns.power_sum(first, group.low_len as usize);
            }
        }
        let scale = |source: [u64; 2], coefficient: u32| -> [u64; 2] {
            if coefficient == 0 {
                source
            } else if coefficient == 1 {
                neg_representative(source, field)
            } else {
                mul_representatives(source, coefficients[coefficient as usize], field)
            }
        };
        // Sources sit at strictly higher levels (or are inputs), so a level
        // reads only past its own end; its nodes are independent.
        for level in (0..tape.level_count()).rev() {
            let start = tape.level_offsets[level] as usize;
            let end = tape.level_offsets[level + 1] as usize;
            let (current_and_before, later) = values.split_at_mut(end);
            let current = &mut current_and_before[start..end];
            let evaluate = |offset: usize, slot: &mut [u64; 2]| {
                let node = start + offset;
                let terms = &tape.forward_terms
                    [tape.forward_offsets[node] as usize..tape.forward_offsets[node + 1] as usize];
                if terms.is_empty() {
                    // A power group's sum: already holds its geometric input sum.
                    return;
                }
                let mut value = [0_u64; 2];
                let mut initialized = false;
                for term in terms {
                    let source = term.source as usize;
                    debug_assert!(source >= end);
                    let contribution = scale(later[source - end], term.coefficient);
                    if initialized {
                        value = add_representatives(value, contribution, field);
                    } else {
                        value = contribution;
                        initialized = true;
                    }
                }
                *slot = value;
            };
            if rayon::current_num_threads() > 1 && current.len() >= PARALLEL_LEVEL_THRESHOLD {
                current
                    .par_iter_mut()
                    .enumerate()
                    .for_each(|(offset, slot)| evaluate(offset, slot));
            } else {
                current
                    .iter_mut()
                    .enumerate()
                    .for_each(|(offset, slot)| evaluate(offset, slot));
            }
        }
        let mut result = [0_u64; 2];
        for (node, &value) in values.iter().enumerate() {
            let roots =
                &tape.roots[tape.root_offsets[node] as usize..tape.root_offsets[node + 1] as usize];
            for root in roots {
                let weight = weights[root.row as usize][match root.kind {
                    RootKind::A => 0,
                    RootKind::B => 1,
                    RootKind::C => 2,
                }];
                let term = mul_representatives(weight, scale(value, root.coefficient), field);
                result = add_representatives(result, term, field);
            }
        }
        Ok(result)
    }

    /// The reverse pass over the prepared `weighted_challenges`, into `output`.
    /// Addition and multiplication by Montgomery coefficients preserve the
    /// weights' representation, including through the power-of-two groups.
    fn run_reverse(&mut self, force_parallel: Option<bool>) {
        let Self {
            tape,
            field,
            coefficients,
            weighted_challenges,
            adjoints,
            output,
            powers_of_two,
            forward_values: _,
        } = self;
        output.resize(tape.column_count(), [0; 2]);
        let context = ReverseContext {
            tape,
            field,
            coefficients,
            weighted_challenges,
        };

        for level in 0..tape.level_count() {
            let start = tape.level_offsets[level] as usize;
            let end = tape.level_offsets[level + 1] as usize;
            let (prior, current_and_later) = adjoints.split_at_mut(start);
            let current = &mut current_and_later[..end - start];
            let evaluate = |new_node: usize| context.evaluate(start + new_node, prior);
            let parallel = force_parallel.unwrap_or_else(|| {
                rayon::current_num_threads() > 1 && current.len() >= PARALLEL_LEVEL_THRESHOLD
            });
            if parallel {
                current
                    .par_iter_mut()
                    .enumerate()
                    .for_each(|(node, value)| *value = evaluate(node));
            } else {
                current
                    .iter_mut()
                    .enumerate()
                    .for_each(|(node, value)| *value = evaluate(node));
            }
        }

        let evaluate_output_chunk = |chunk_index: usize, chunk: &mut [[u64; 2]]| {
            let chunk_start = chunk_index * OUTPUT_CHUNK_SIZE;
            let chunk_end = chunk_start + chunk.len();
            let groups = &tape.power_groups;
            let mut group_index = groups.partition_point(|group| {
                group.first_column as usize + group.len as usize <= chunk_start
            });
            let fill_scalars = |start: usize, values: &mut [[u64; 2]]| {
                for (offset, value) in values.iter_mut().enumerate() {
                    let node = tape.input_nodes[start + offset];
                    if node == NO_NODE {
                        *value = [0; 2];
                    } else {
                        *value = context.evaluate(node as usize, adjoints);
                    }
                }
            };
            let mut column = chunk_start;
            while column < chunk_end {
                let Some(group) = groups.get(group_index) else {
                    fill_scalars(column, &mut chunk[column - chunk_start..]);
                    break;
                };
                let group_start = group.first_column as usize;
                let group_end = group_start + group.len as usize;
                if column < group_start {
                    let scalar_end = group_start.min(chunk_end);
                    fill_scalars(
                        column,
                        &mut chunk[column - chunk_start..scalar_end - chunk_start],
                    );
                    column = scalar_end;
                    continue;
                }

                let low_end = group_start + group.low_len as usize;
                let segment_end = if column < low_end {
                    low_end.min(group_end).min(chunk_end)
                } else {
                    group_end.min(chunk_end)
                };
                let read_adjoint = |node: u32| {
                    if node == NO_NODE {
                        [0; 2]
                    } else {
                        adjoints[node as usize]
                    }
                };
                let full = read_adjoint(group.full_node);
                let mut base = full;
                if column < low_end {
                    let low = read_adjoint(group.low_node);
                    base = add_representatives(base, low, field);
                }
                let power = column - group_start;
                let mut value = if power == 0 {
                    base
                } else {
                    mul_representatives(base, powers_of_two[power], field)
                };
                for output in &mut chunk[column - chunk_start..segment_end - chunk_start] {
                    *output = value;
                    value = add_representatives(value, value, field);
                }
                column = segment_end;
                if column == group_end {
                    group_index += 1;
                }
            }
        };
        let parallel = force_parallel.unwrap_or_else(|| {
            rayon::current_num_threads() > 1 && output.len() >= PARALLEL_VECTOR_THRESHOLD
        });
        if parallel {
            output
                .par_chunks_mut(OUTPUT_CHUNK_SIZE)
                .enumerate()
                .for_each(|(chunk, output)| evaluate_output_chunk(chunk, output));
        } else {
            output
                .chunks_mut(OUTPUT_CHUNK_SIZE)
                .enumerate()
                .for_each(|(chunk, output)| evaluate_output_chunk(chunk, output));
        }
    }
}

fn prefix_offsets(counts: &[u32], message: &'static str) -> Vec<u32> {
    let mut offsets = Vec::with_capacity(counts.len() + 1);
    offsets.push(0_u32);
    for count in counts {
        offsets.push(
            offsets
                .last()
                .copied()
                .unwrap()
                .checked_add(*count)
                .expect(message),
        );
    }
    offsets
}

fn parallel_map<T: Sync, U: Send>(
    values: &[T],
    force_parallel: Option<bool>,
    apply: impl Fn(&T) -> U + Sync + Send,
) -> Vec<U> {
    let parallel = force_parallel.unwrap_or_else(|| {
        rayon::current_num_threads() > 1 && values.len() >= PARALLEL_VECTOR_THRESHOLD
    });
    if parallel {
        values.par_iter().map(apply).collect()
    } else {
        values.iter().map(apply).collect()
    }
}

/// The input/output words may be canonical residues or Montgomery encodings;
/// addition and scaling by a Montgomery coefficient preserve that representation.
#[inline(always)]
pub(crate) fn mul_representatives(
    left: [u64; 2],
    coefficient: [u64; 2],
    field: &FpCtx<2>,
) -> [u64; 2] {
    *field
        .mul_canonical(
            &Uint::from_words(left),
            &field.from_montgomery_integer(Uint::from_words(coefficient)),
        )
        .as_words()
}

#[inline(always)]
pub(crate) fn add_representatives(left: [u64; 2], right: [u64; 2], field: &FpCtx<2>) -> [u64; 2] {
    *field
        .add(
            &field.from_montgomery_integer(Uint::from_words(left)),
            &field.from_montgomery_integer(Uint::from_words(right)),
        )
        .as_montgomery_integer()
        .as_words()
}

#[inline(always)]
pub(crate) fn neg_representative(value: [u64; 2], field: &FpCtx<2>) -> [u64; 2] {
    *field
        .neg(&field.from_montgomery_integer(Uint::from_words(value)))
        .as_montgomery_integer()
        .as_words()
}

/// One geometric run of the last [`PreparedWengertEvaluator::apply`] /
/// [`PreparedWengertEvaluator::apply_weighted`] output: columns
/// `first_column .. first_column + len` hold `base · 2^k` for `k = 0 .. len`, in the
/// evaluator's Montgomery form. Local F2Z addition; not part of upstream f2z-benchmark.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PowerRun {
    pub first_column: usize,
    pub len: usize,
    pub base: [u64; 2],
}

impl PreparedWengertEvaluator<'_> {
    /// The tape's power groups as geometric runs of the current output, in
    /// column order: one run per `f2z_unsigned` lift, or two when the lift has a
    /// low part (the low columns carry the full and low adjoints summed, the rest
    /// continue the same power sequence from the full adjoint alone). Columns
    /// outside every run are scalar outputs. Valid until the next apply.
    pub fn power_runs(&self) -> Vec<PowerRun> {
        let read = |node: u32| {
            if node == NO_NODE {
                [0; 2]
            } else {
                self.adjoints[node as usize]
            }
        };
        let mut runs = Vec::with_capacity(2 * self.tape.power_groups.len());
        for group in self.tape.power_groups.iter() {
            let first_column = group.first_column as usize;
            let len = group.len as usize;
            let low_len = group.low_len as usize;
            let full = read(group.full_node);
            if low_len == 0 {
                runs.push(PowerRun {
                    first_column,
                    len,
                    base: full,
                });
            } else {
                let low = add_representatives(full, read(group.low_node), &self.field);
                runs.push(PowerRun {
                    first_column,
                    len: low_len,
                    base: low,
                });
                let base = mul_representatives(full, self.powers_of_two[low_len], &self.field);
                runs.push(PowerRun {
                    first_column: first_column + low_len,
                    len: len - low_len,
                    base,
                });
            }
        }
        runs
    }
}

/// Failure to apply a Wengert tape with the supplied runtime data.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WengertApplyError {
    ChallengeLength { expected: usize, actual: usize },
    EvenModulus,
}

impl Display for WengertApplyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ChallengeLength { expected, actual } => write!(
                formatter,
                "challenge vector has length {actual}, expected {expected}"
            ),
            Self::EvenModulus => formatter.write_str("Wengert evaluation requires an odd modulus"),
        }
    }
}

impl Error for WengertApplyError {}

/// Records and preprocesses the circuit's Z-side linear arithmetic.
#[derive(Debug)]
pub struct WengertGenerator {
    recorder: Arc<SharedRecorder>,
    inputs: Box<[WengertBit]>,
}

impl WengertGenerator {
    /// Creates a generator with the circuit's Boolean input shape.
    pub fn new(input_count: usize) -> Self {
        Self {
            recorder: Arc::new(SharedRecorder(Mutex::new(Some(Recorder::new())))),
            inputs: vec![WengertBit; input_count].into_boxed_slice(),
        }
    }

    /// Moves every Boolean input handle into a fixed-size boxed array.
    pub fn take_boxed_inputs<const N: usize>(&mut self) -> Box<[WengertBit; N]> {
        assert_eq!(N, self.inputs.len(), "input witness count mismatch");
        std::mem::take(&mut self.inputs)
            .try_into()
            .unwrap_or_else(|_| unreachable!("input length was checked"))
    }

    /// Moves dynamically sized Boolean input handles out.
    pub fn take_inputs(&mut self) -> Box<[WengertBit]> {
        std::mem::take(&mut self.inputs)
    }

    /// Prunes, transposes, and reverse-depth-orders the recorded graph.
    pub fn finish(self) -> WengertTape {
        let recorder = self
            .recorder
            .0
            .lock()
            .expect("Wengert recorder lock poisoned")
            .take()
            .expect("the Wengert generator has already been finished");
        WengertTape::from_recorder(recorder)
    }

    fn sum_terms<const LIMBS: usize>(
        &self,
        terms: impl IntoIterator<Item = WengertValue<LIMBS>>,
    ) -> WengertValue<LIMBS> {
        let mut terms: Vec<_> = terms.into_iter().filter(|term| !term.is_zero()).collect();
        match terms.len() {
            0 => WengertValue::zero(),
            1 => terms.pop().unwrap(),
            _ => {
                let node = self.recorder.with_mut(|recorder| {
                    let terms = terms
                        .iter()
                        .map(|value| value.raw_term(recorder))
                        .collect::<Vec<_>>();
                    recorder.push_sum(terms.into_boxed_slice())
                });
                WengertValue::attached(self.recorder.clone(), node, Z::one())
            }
        }
    }
}

impl Circuit for WengertGenerator {
    type Bool = WengertBit;
    type Coefficient<const LIMBS: usize> = Z<LIMBS>;
    type Z<const LIMBS: usize> = WengertValue<LIMBS>;

    fn coefficient_from_le_words<const LIMBS: usize>(words: &[u64]) -> Z<LIMBS> {
        crate::witgen::integer_from_words(words)
    }

    fn xor(&mut self, _: WengertBit, _: WengertBit) -> WengertBit {
        WengertBit
    }

    fn hint<const LIMBS: usize, const N: usize, const M: usize, H>(
        &mut self,
        _: H,
    ) -> ScalarBits<WengertBit, N>
    where
        H: Fn(
                &dyn WitnessContext<WengertValue<LIMBS>, WengertBit, Z<LIMBS>>,
            ) -> HintResult<PackedBits<N, M>>
            + Send
            + Sync
            + 'static,
    {
        assert_eq!(M, N.div_ceil(64), "incorrect packed limb count");
        ScalarBits([WengertBit; N])
    }

    fn f2z<const LIMBS: usize>(&mut self, _: WengertBit) -> WengertValue<LIMBS> {
        let node = self.recorder.with_mut(Recorder::push_input);
        WengertValue::attached(self.recorder.clone(), node, Z::one())
    }

    fn f2z_unsigned<const LIMBS: usize, const N: usize, const M: usize, const LOW: usize>(
        &mut self,
        _: &<WengertBit as BoolWitness>::Repr<N, M>,
    ) -> (WengertValue<LIMBS>, WengertValue<LIMBS>) {
        assert!(LOW <= N, "low part cannot be wider than the input");
        let first_column = self
            .recorder
            .with_mut(|recorder| recorder.input_nodes.len());
        let mut power = Z::<LIMBS>::one();
        let lifted: Vec<_> = (0..N)
            .map(|_| {
                let value = self.f2z::<LIMBS>(WengertBit) * power;
                power += power;
                value
            })
            .collect();
        let full = self.sum_terms(lifted.iter().cloned());
        let low = if LOW == N {
            full.clone()
        } else {
            self.sum_terms(lifted.into_iter().take(LOW))
        };
        if N >= 2 && LOW != 1 {
            let full_node = match &full.location {
                ValueLocation::Node { node, .. } => *node,
                ValueLocation::Constant => unreachable!("a nonempty lift is not constant"),
            };
            let low_node = if LOW == 0 || LOW == N {
                NO_NODE
            } else {
                match &low.location {
                    ValueLocation::Node { node, .. } => *node,
                    ValueLocation::Constant => unreachable!("a nonempty low lift is not constant"),
                }
            };
            self.recorder.with_mut(|recorder| {
                debug_assert_eq!(recorder.input_nodes.len(), first_column + N);
                recorder.power_groups.push(RawPowerGroup {
                    first_column: u32::try_from(first_column)
                        .expect("too many integer-witness columns"),
                    len: u32::try_from(N).expect("power group is too wide"),
                    low_len: if LOW == 0 || LOW == N {
                        0
                    } else {
                        u32::try_from(LOW).expect("low power group is too wide")
                    },
                    full_node,
                    low_node,
                });
            });
        }
        (full, low)
    }

    fn assert_r1c<const LIMBS: usize>(
        &mut self,
        a: WengertValue<LIMBS>,
        b: WengertValue<LIMBS>,
        c: WengertValue<LIMBS>,
    ) {
        for value in [&a, &b, &c] {
            if let Some(recorder) = value.recorder() {
                WengertValue::<LIMBS>::same_recorder(&self.recorder, recorder);
            }
        }
        self.recorder.with_mut(|recorder| {
            let row = u32::try_from(recorder.constraints).expect("too many R1CS rows");
            recorder.constraints += 1;
            let terms = [
                a.raw_term(recorder),
                b.raw_term(recorder),
                c.raw_term(recorder),
            ];
            recorder.roots.extend([
                RawRoot {
                    row,
                    kind: RootKind::A,
                    term: terms[0],
                },
                RawRoot {
                    row,
                    kind: RootKind::B,
                    term: terms[1],
                },
                RawRoot {
                    row,
                    kind: RootKind::C,
                    term: terms[2],
                },
            ]);
        });
    }

    fn sign_extend_z<const FROM_LIMBS: usize, const TO_LIMBS: usize>(
        &mut self,
        value: WengertValue<FROM_LIMBS>,
    ) -> WengertValue<TO_LIMBS> {
        assert!(
            TO_LIMBS >= FROM_LIMBS,
            "cannot sign-extend into fewer limbs"
        );
        WengertValue {
            location: value.location,
            coefficient: value.coefficient.sign_extend(),
        }
    }
}

#[cfg(test)]
mod tests {
    use num_bigint::{BigInt, BigUint};
    use num_traits::Signed;

    use super::*;
    use crate::constraints::{ConstraintGenerator, ConstraintMatrices};
    use crate::sha256::{COMPRESSION_INPUT_BITS, compression_circuit};
    use crypto_bigint::modular::{FixedMontyForm, FixedMontyParams};
    use crypto_bigint::{Odd, U128};

    fn example_circuit<CS: Circuit>(circuit: &mut CS, inputs: &[CS::Bool; 3]) {
        let a = circuit.f2z::<2>(inputs[0].clone());
        let b = circuit.f2z::<2>(inputs[1].clone());
        let c = circuit.f2z::<2>(inputs[2].clone());
        let seven = CS::Coefficient::<2>::from(7);
        let eleven = CS::Coefficient::<2>::from(11);
        let thirteen = CS::Coefficient::<2>::from(13);
        circuit.assert_r1c(
            a.clone() * seven.clone() - b.clone(),
            b.clone() * eleven + CS::Z::<2>::from(CS::Coefficient::<2>::from(5)),
            c.clone() * thirteen,
        );
        circuit.assert_r1c(
            a + c.clone(),
            -c,
            b + CS::Z::<2>::from(CS::Coefficient::<2>::from(19)),
        );
    }

    fn build_tape() -> WengertTape {
        let mut generator = WengertGenerator::new(3);
        let inputs = generator.take_boxed_inputs();
        example_circuit(&mut generator, &inputs);
        generator.finish()
    }

    fn direct_product(
        matrices: &ConstraintMatrices,
        challenges: &[[u64; 2]],
        x: [u64; 2],
        modulus: &BigUint,
    ) -> Vec<[u64; 2]> {
        let modulus_int = BigInt::from(modulus.clone());
        let as_bigint = |words: [u64; 2]| {
            BigInt::from(BigUint::from(words[0]) + (BigUint::from(words[1]) << 64_usize))
        };
        let x = as_bigint(x);
        let x_squared = &x * &x;
        let mut output = vec![BigInt::zero(); matrices.a.column_count()];
        for (row, challenge) in challenges.iter().enumerate() {
            let challenge = as_bigint(*challenge);
            for (column, coefficient) in matrices.a.row_entries(row) {
                let coefficient = BigInt::from_signed_bytes_le(
                    &coefficient
                        .iter()
                        .flat_map(|w| w.to_le_bytes())
                        .collect::<Vec<_>>(),
                );
                output[column] += &challenge * coefficient;
            }
            for (column, coefficient) in matrices.b.row_entries(row) {
                let coefficient = BigInt::from_signed_bytes_le(
                    &coefficient
                        .iter()
                        .flat_map(|w| w.to_le_bytes())
                        .collect::<Vec<_>>(),
                );
                output[column] += &challenge * &x * coefficient;
            }
            for (column, coefficient) in matrices.c.row_entries(row) {
                let coefficient = BigInt::from_signed_bytes_le(
                    &coefficient
                        .iter()
                        .flat_map(|w| w.to_le_bytes())
                        .collect::<Vec<_>>(),
                );
                output[column] += &challenge * &x_squared * coefficient;
            }
        }
        output
            .into_iter()
            .map(|mut value| {
                value %= &modulus_int;
                if value.is_negative() {
                    value += &modulus_int;
                }
                let words = value.to_biguint().unwrap().to_u64_digits();
                [
                    words.first().copied().unwrap_or(0),
                    words.get(1).copied().unwrap_or(0),
                ]
            })
            .collect()
    }

    #[test]
    fn tape_matches_materialized_matrices_for_primes_known_afterward() {
        let tape = build_tape();
        let mut generator = ConstraintGenerator::new(3);
        let inputs = generator.inputs();
        example_circuit(&mut generator, &inputs);
        let matrices = generator.into_matrices();
        let challenges = [[23, 0], [29, 0]];
        let x = [17, 0];

        for modulus in [
            (BigUint::one() << 127_usize) - BigUint::one(),
            (BigUint::one() << 128_usize) - BigUint::from(159_u64),
        ] {
            let runtime = ModRingCtx::<2>::new(field::Uint::from_words(
                crate::matrix_products::biguint_words(&(modulus.clone())),
            ))
            .unwrap();
            assert_eq!(
                tape.apply(&challenges, x, &runtime).unwrap(),
                direct_product(&matrices, &challenges, x, &modulus)
            );
        }
    }

    #[test]
    fn canonical_apply_handles_unreduced_inputs_and_reuses_output() {
        let tape = build_tape();
        let mut generator = ConstraintGenerator::new(3);
        let inputs = generator.inputs();
        example_circuit(&mut generator, &inputs);
        let matrices = generator.into_matrices();
        let words = |value: u128| [value as u64, (value >> 64) as u64];
        let mut output = vec![[u64::MAX; 2]; tape.column_count() + 3];
        let output_ptr = output.as_ptr();
        let output_capacity = output.capacity();

        for prime in [
            3,
            101,
            (1_u128 << 64) - 59,
            (1_u128 << 127) - 1,
            u128::MAX - 158,
        ] {
            let modulus = BigUint::from(prime);
            let runtime = ModRingCtx::<2>::new(field::Uint::from_words(
                crate::matrix_products::biguint_words(&(modulus.clone())),
            ))
            .unwrap();
            let mut prepared = tape.prepare(&runtime).unwrap();
            let cases = [0, 1, prime - 1, prime, prime + 1, u128::MAX];
            for (i, &r) in cases.iter().enumerate() {
                let challenges = [words(r), words(cases[(i + 1) % cases.len()])];
                let montgomery_challenges = challenges.map(|r| prepared.to_montgomery(r));
                for &x in &cases {
                    let x = words(x);
                    let expected = direct_product(&matrices, &challenges, x, &modulus);
                    for parallel in [false, true] {
                        output.resize(tape.column_count() + 3, [u64::MAX; 2]);
                        output.fill([u64::MAX; 2]);
                        tape.apply_inner(&challenges, x, &runtime, &mut output, Some(parallel))
                            .unwrap();
                        assert_eq!(
                            output, expected,
                            "q={prime}, r={r}, x={x:?}, parallel={parallel}"
                        );
                        assert_eq!(output.as_ptr(), output_ptr);
                        assert_eq!(output.capacity(), output_capacity);
                    }

                    let montgomery_x = prepared.to_montgomery(x);
                    let expected_encoded: Vec<_> = expected
                        .iter()
                        .map(|value| prepared.to_montgomery(*value))
                        .collect();
                    let encoded = prepared
                        .apply(&montgomery_challenges, montgomery_x)
                        .unwrap();
                    assert_eq!(encoded, expected_encoded);
                }
            }
        }
    }

    #[test]
    fn parallel_and_sequential_reverse_batches_agree() {
        let tape = build_tape();
        let modulus = ModRingCtx::<2>::new(field::Uint::from_words(
            crate::matrix_products::biguint_words(
                &((BigUint::one() << 128_usize) - BigUint::from(159_u64)),
            ),
        ))
        .unwrap();
        let challenges = [[0x1234_5678_9abc_def0, 7], [0x0fed_cba9_8765_4321, 11]];
        let mut sequential = Vec::new();
        let mut parallel = Vec::new();
        tape.apply_inner(&challenges, [31, 3], &modulus, &mut sequential, Some(false))
            .unwrap();
        tape.apply_inner(&challenges, [31, 3], &modulus, &mut parallel, Some(true))
            .unwrap();
        assert_eq!(parallel, sequential);
    }

    #[test]
    fn prepared_evaluator_reuses_storage_and_matches_one_shot_apply() {
        let tape = build_tape();
        let modulus = ModRingCtx::<2>::new(field::Uint::from_words(
            crate::matrix_products::biguint_words(
                &((BigUint::one() << 128_usize) - BigUint::from(159_u64)),
            ),
        ))
        .unwrap();
        let challenges = [[0x1234_5678_9abc_def0, 7], [0x0fed_cba9_8765_4321, 11]];
        let x = [31, 3];
        let expected = tape.apply(&challenges, x, &modulus).unwrap();
        let mut evaluator = tape.prepare(&modulus).unwrap();
        let montgomery_challenges: Vec<_> = challenges
            .into_iter()
            .map(|challenge| evaluator.to_montgomery(challenge))
            .collect();
        let montgomery_x = evaluator.to_montgomery(x);

        for _ in 0..2 {
            let output = evaluator
                .apply(&montgomery_challenges, montgomery_x)
                .unwrap()
                .to_vec();
            let output: Vec<_> = output
                .into_iter()
                .map(|value| evaluator.from_montgomery(value))
                .collect();
            assert_eq!(output, expected);
        }
    }

    /// Dense column values for the forward-pass test: every column value is
    /// stored, and a power group's sum is the plain doubling chain.
    struct DenseColumns {
        values: Vec<[u64; 2]>,
        field: FpCtx<2>,
    }

    impl ForwardColumns for DenseColumns {
        fn scalar(&self, column: usize) -> [u64; 2] {
            self.values[column]
        }

        fn power_sum(&self, first: usize, len: usize) -> [u64; 2] {
            let mut sum = [0; 2];
            for k in (0..len).rev() {
                sum = add_representatives(sum, sum, &self.field);
                sum = add_representatives(sum, self.values[first + k], &self.field);
            }
            sum
        }
    }

    /// The forward pass is the reverse pass's column vector dotted with the
    /// column values: on the example tape and the SHA-256 compression tape,
    /// random weight triples and random column values, at `2^127 − 1` and
    /// `2^128 − 159`.
    #[test]
    fn forward_pass_matches_reverse_dot_product() {
        let mut sha_generator = WengertGenerator::new(COMPRESSION_INPUT_BITS);
        let sha_inputs = sha_generator.take_boxed_inputs();
        let _ = compression_circuit(&mut sha_generator, &sha_inputs);
        let tapes = [build_tape(), sha_generator.finish()];
        let mut state = 0x2545_f491_4f6c_dd1d_u64;
        let mut random = || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        for modulus in [
            (BigUint::one() << 127_usize) - BigUint::one(),
            (BigUint::one() << 128_usize) - BigUint::from(159_u64),
        ] {
            let runtime = ModRingCtx::<2>::new(Uint::from_words(
                crate::matrix_products::biguint_words(&modulus),
            ))
            .unwrap();
            for tape in &tapes {
                let mut evaluator = tape.prepare(&runtime).unwrap();
                let element = |random: &mut dyn FnMut() -> u64| {
                    *evaluator
                        .field
                        .from_integer(&Uint::from_words([random(), random()]))
                        .as_montgomery_integer()
                        .as_words()
                };
                let weights: Vec<[[u64; 2]; 3]> = (0..tape.row_count())
                    .map(|_| {
                        [
                            element(&mut random),
                            element(&mut random),
                            element(&mut random),
                        ]
                    })
                    .collect();
                let columns = DenseColumns {
                    values: (0..tape.column_count())
                        .map(|_| element(&mut random))
                        .collect(),
                    field: evaluator.field.clone(),
                };
                assert!(evaluator.output.is_empty());
                let forward = evaluator
                    .apply_forward_weighted(&weights, &columns)
                    .unwrap();
                assert!(evaluator.output.is_empty());
                let reverse = evaluator.apply_weighted(&weights).unwrap().to_vec();
                let mut dot = [0; 2];
                for (output, value) in reverse.iter().zip(&columns.values) {
                    let term = mul_representatives(*output, *value, &evaluator.field);
                    dot = add_representatives(dot, term, &evaluator.field);
                }
                assert_eq!(forward, dot);
                assert_eq!(
                    evaluator.apply_forward_weighted(&weights[1..], &columns),
                    Err(WengertApplyError::ChallengeLength {
                        expected: tape.row_count(),
                        actual: tape.row_count() - 1,
                    })
                );
            }
        }
    }

    #[test]
    fn two_limb_montgomery_kernel_matches_crypto_bigint() {
        let modulus_words = [u64::MAX - 158, u64::MAX];
        let modulus = U128::from_words(modulus_words);
        let params = FixedMontyParams::new_vartime(Odd::new(modulus).unwrap());
        let field = create_prime_field(Uint::from_words(modulus_words));
        let mut state = 0x4d59_5df4_d0f3_3173_u64;
        let mut random = || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };

        for _ in 0..1_000 {
            let canonical_left = U128::from_words(
                *field
                    .reduce_integer(&Uint::from_words([random(), random()]))
                    .as_words(),
            );
            let canonical_right = U128::from_words(
                *field
                    .reduce_integer(&Uint::from_words([random(), random()]))
                    .as_words(),
            );
            let left = FixedMontyForm::new(&canonical_left, &params);
            let right = FixedMontyForm::new(&canonical_right, &params);
            let expected = (left * right).to_montgomery().to_words();
            let actual = mul_representatives(
                left.to_montgomery().to_words(),
                right.to_montgomery().to_words(),
                &field,
            );

            assert_eq!(actual, expected);
            assert_eq!(
                *field
                    .to_integer(&field.from_montgomery_integer(Uint::from_words(actual)))
                    .as_words(),
                (left * right).retrieve().to_words()
            );
        }
    }

    #[test]
    fn sha256_compression_tape_matches_materialized_sparse_matrices() {
        let mut tape_generator = WengertGenerator::new(COMPRESSION_INPUT_BITS);
        let tape_inputs = tape_generator.take_boxed_inputs();
        let _ = compression_circuit(&mut tape_generator, &tape_inputs);
        let tape = tape_generator.finish();

        let mut matrix_generator = ConstraintGenerator::new(COMPRESSION_INPUT_BITS);
        let matrix_inputs = matrix_generator.boxed_inputs();
        let _ = compression_circuit(&mut matrix_generator, &matrix_inputs);
        let matrices = matrix_generator.into_matrices();

        let challenges: Vec<_> = (0..tape.row_count())
            .map(|row| [(17 * row + 3) as u64, (5 * row + 1) as u64])
            .collect();
        let x = [0x1234_5678_9abc_def0, 0x0123_4567_89ab_cdef];
        let prime = (BigUint::one() << 128_usize) - BigUint::from(159_u64);
        let modulus = ModRingCtx::<2>::new(field::Uint::from_words(
            crate::matrix_products::biguint_words(&(prime.clone())),
        ))
        .unwrap();

        assert_eq!(tape.row_count(), matrices.a.row_count());
        assert_eq!(tape.column_count(), matrices.a.column_count());
        assert_eq!(
            tape.apply(&challenges, x, &modulus).unwrap(),
            direct_product(&matrices, &challenges, x, &prime)
        );
        assert!(tape.node_count() <= tape.edge_count());
        assert!(tape.payload_bytes() < 4 * 1024 * 1024);
    }

    #[test]
    fn rejects_wrong_challenge_length_and_even_modulus() {
        let tape = build_tape();
        let odd = ModRingCtx::<2>::new(field::Uint::from_words(
            crate::matrix_products::biguint_words(&(BigUint::from(101_u64))),
        ))
        .unwrap();
        assert_eq!(
            tape.apply(&[[1, 0]], [2, 0], &odd),
            Err(WengertApplyError::ChallengeLength {
                expected: 2,
                actual: 1,
            })
        );
        let even = ModRingCtx::<2>::new(field::Uint::from_words(
            crate::matrix_products::biguint_words(&(BigUint::from(100_u64))),
        ))
        .unwrap();
        assert_eq!(
            tape.apply(&[[1, 0], [2, 0]], [3, 0], &even),
            Err(WengertApplyError::EvenModulus)
        );
    }

    #[test]
    fn dead_arithmetic_is_pruned_and_unused_inputs_return_zero() {
        let mut generator = WengertGenerator::new(2);
        let inputs = generator.take_boxed_inputs::<2>();
        let used = generator.f2z::<1>(inputs[0]);
        let unused = generator.f2z::<1>(inputs[1]);
        let _dead = unused.clone() + unused;
        generator.assert_r1c(used, WengertValue::zero(), WengertValue::zero());
        let tape = generator.finish();
        let modulus = ModRingCtx::<2>::new(field::Uint::from_words(
            crate::matrix_products::biguint_words(&(BigUint::from(101_u64))),
        ))
        .unwrap();
        assert_eq!(
            tape.apply(&[[7, 0]], [3, 0], &modulus).unwrap(),
            [[0, 0], [7, 0], [0, 0]]
        );
        assert_eq!(tape.node_count(), 1);
    }
}
