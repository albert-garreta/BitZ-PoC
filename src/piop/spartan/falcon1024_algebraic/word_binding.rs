//! Word sumcheck with one dense product and sparse/factored auxiliary terms.
use super::*;
use crate::piop::spartan::SpartanField;
use crate::sumcheck::boundary::{RoundBoundaryPolicy, SplitRoundBoundary};
use field::{FpProductAcc, PreparedLinearCombination};

#[cfg(test)]
#[path = "word_binding_tests.rs"]
mod tests;

const DOMAIN: &[u8] = b"bitz/falcon-algebraic/word-binding/v1";
const TERMINALS: &[u8] = b"bitz/falcon-algebraic/word-terminals/v1";
const MERGED_DOMAIN: &[u8] = b"bitz/falcon-algebraic/word-binding-merged-norm/v1";
crate::piop::spartan::grinding::grinding_domains! {
    TerminalMerge => b"bitz/falcon-algebraic/grinding/word-terminals/v1";
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in super::super) struct WordBridgeClaim {
    pub point: Vec<F>,
    pub modulus: u128,
    pub value: F,
    pub zeta: F,
    pub with_padding: bool,
}

impl WordBridgeClaim {
    pub fn column_point(&self, layout: &Layout) -> &[F] {
        &self.point[layout.row_vars() - 4..]
    }

    #[tracing::instrument(skip_all, name = "falcon_algebraic:word_row_weights")]
    pub fn row_weights(&self, layout: &Layout, field: &Cfg) -> Result<Vec<u128>, FalconError> {
        if self.point.len() != rounds(layout)
            || self.with_padding != (layout.batch() < layout.capacity())
        {
            return Err(failure("word bridge shape"));
        }
        let eq = eq_table(&self.point[..layout.row_vars() - 4], field).map_err(failure)?;
        let extra = if self.with_padding {
            field.mul(&self.zeta, &self.zeta)
        } else {
            field.zero()
        };
        let decoder: [F; 16] = std::array::from_fn(|lane| {
            if lane == 15 {
                self.zeta
            } else {
                field.add(
                    &small(if lane == 14 { -(1 << 14) } else { 1 << lane }, field),
                    &extra,
                )
            }
        });
        Ok(eq
            .iter()
            .flat_map(|weight| {
                decoder
                    .iter()
                    .map(move |digit| u128::from(field.to_integer(&field.mul(weight, digit))))
            })
            .collect())
    }
}

fn rounds(layout: &Layout) -> usize {
    source_rounds(layout) - 4
}
fn small(x: i64, f: &Cfg) -> F {
    let v = F::from_with_cfg(x.unsigned_abs() as u128, f);
    if x < 0 { f.neg(&v) } else { v }
}
fn lerp(a: F, b: F, r: F, f: &Cfg) -> F {
    f.add(&a, &f.mul(&r, &f.sub(&b, &a)))
}
fn word(source: &Source, index: usize) -> u16 {
    // Four lane bits and nine coefficient bits are in each physical row.
    (source.rows()[index >> 9][(index & 511) >> 2] >> (16 * (index & 3))) as u16
}
fn decode(bits: u16) -> i64 {
    i64::from(((bits << 1) as i16) >> 1)
}
fn coefficient(form: &BindingForm<'_>, index: usize) -> F {
    let instance = index / (2 * N);
    if instance >= form.layout.batch() {
        return form.field.zero();
    }
    form.coefficients[(index / N) & 1][instance * N + index % N]
}
fn fold_small(table: &mut Vec<F>, r: F, f: &Cfg) {
    for i in 0..table.len() / 2 {
        table[i] = lerp(table[2 * i], table[2 * i + 1], r, f);
    }
    table.truncate(table.len() / 2);
}
fn add_pair(a: [F; 2], b: [F; 2], f: &Cfg) -> [F; 2] {
    [f.add(&a[0], &b[0]), f.add(&a[1], &b[1])]
}

/// H's weights are gamma + 1_side0*(slack_i*pow_j - gamma*active_i*mask_j).
/// These small factors are multilinear in disjoint coordinate sets.
struct PublicWeights {
    powers: Vec<F>,
    padding_positions: Vec<F>,
    slacks: Vec<F>,
    active: Vec<F>,
    side: F,
    padding: F,
}
impl PublicWeights {
    #[tracing::instrument(skip_all, name = "falcon_algebraic:word_public_weights")]
    fn new(form: &BindingForm<'_>) -> Self {
        let f = form.field;
        let mut slacks = form.slacks.clone();
        slacks.resize(form.layout.capacity(), f.zero());
        Self {
            powers: (0..N)
                .map(|j| {
                    if j < SLACK_BITS {
                        small(1 << j, f)
                    } else {
                        f.zero()
                    }
                })
                .collect(),
            padding_positions: (0..N)
                .map(|j| {
                    if j < SLACK_BITS {
                        form.padding
                    } else {
                        f.zero()
                    }
                })
                .collect(),
            slacks,
            active: (0..form.layout.capacity())
                .map(|i| {
                    if i < form.layout.batch() {
                        f.one()
                    } else {
                        f.zero()
                    }
                })
                .collect(),
            side: f.one(),
            padding: form.padding,
        }
    }
    fn spare(&self, index: usize, round: usize, f: &Cfg) -> F {
        if round <= COEFFICIENT_LOG {
            let n = N >> round;
            let instance = index / (2 * n);
            // Side and activity are still public Boolean coordinates here.
            if (index / n) & 1 != 0 || self.active[instance] == f.zero() {
                return self.padding;
            }
            let j = index % n;
            return f.add(
                &f.mul(&self.slacks[instance], &self.powers[j]),
                &f.sub(&self.padding, &self.padding_positions[j]),
            );
        }
        f.add(
            &self.padding,
            &f.mul(
                &self.side,
                &f.sub(
                    &f.mul(&self.slacks[index], &self.powers[0]),
                    &f.mul(&self.active[index], &self.padding_positions[0]),
                ),
            ),
        )
    }
    fn count(&self, index: usize, f: &Cfg) -> F {
        f.mul(&self.padding, &f.sub(&f.one(), &self.active[index]))
    }
    fn fold(&mut self, round: usize, r: F, f: &Cfg) {
        if round < COEFFICIENT_LOG {
            fold_small(&mut self.powers, r, f);
            fold_small(&mut self.padding_positions, r, f);
        } else if round == COEFFICIENT_LOG {
            self.side = f.sub(&f.one(), &r);
        } else {
            fold_small(&mut self.slacks, r, f);
            fold_small(&mut self.active, r, f);
        }
    }
}

/// Sorted nonzero H entries; folding never increases their number.
struct Sparse(Vec<(usize, F)>);
impl Sparse {
    #[tracing::instrument(skip_all, name = "falcon_algebraic:word_source_scan")]
    fn from_source(source: &Source, layout: &Layout, f: &Cfg) -> Result<Self, FalconError> {
        let scan = |instances: std::ops::Range<usize>| {
            let active = instances
                .end
                .min(layout.batch())
                .saturating_sub(instances.start);
            let mut entries = Vec::with_capacity(active * SLACK_BITS);
            for instance in instances {
                let base = instance * 2 * N;
                if instance >= layout.batch() {
                    for c in base..base + 2 * N {
                        if word(source, c) != 0 {
                            return Err(failure("nonzero inactive word"));
                        }
                    }
                } else {
                    for j in 0..2 * N {
                        if word(source, base + j) & 0x8000 != 0 {
                            if j >= SLACK_BITS {
                                return Err(failure("nonzero spare-lane padding"));
                            }
                            entries.push((base + j, f.one()));
                        }
                    }
                }
            }
            Ok(entries)
        };
        #[cfg(feature = "parallel")]
        if layout.source_bits() / 16 >= 1 << 17 && rayon::current_num_threads() > 1 {
            let chunks: Vec<_> = (0..layout.capacity().div_ceil(8))
                .into_par_iter()
                .map(|chunk| scan(chunk * 8..((chunk + 1) * 8).min(layout.capacity())))
                .collect();
            let mut entries = Vec::with_capacity(layout.batch() * SLACK_BITS);
            // Indexed collection preserves source order, including which
            // malformed chunk produces the first error.
            for chunk in chunks {
                entries.extend(chunk?);
            }
            return Ok(Self(entries));
        }
        scan(0..layout.capacity()).map(Self)
    }
    fn pairs(&self, f: &Cfg, mut visit: impl FnMut(usize, F, F)) {
        let mut i = 0;
        while i < self.0.len() {
            let (index, value) = self.0[i];
            let (a, mut b) = if index & 1 == 0 {
                (value, f.zero())
            } else {
                (f.zero(), value)
            };
            i += 1;
            if index & 1 == 0 && i < self.0.len() && self.0[i].0 == index + 1 {
                b = self.0[i].1;
                i += 1;
            }
            visit(index / 2, a, b);
        }
    }
    #[tracing::instrument(skip_all, name = "falcon_algebraic:word_spare_sums", fields(round = round))]
    fn sums(&self, weights: &PublicWeights, round: usize, f: &Cfg) -> [F; 2] {
        let mut sums = [FpProductAcc::<2>::default(); 2];
        self.pairs(f, |i, a, b| {
            let w0 = weights.spare(2 * i, round, f);
            let w1 = weights.spare(2 * i + 1, round, f);
            f.mul_acc(&mut sums[0], &w0, &a);
            f.mul_acc(&mut sums[1], &f.sub(&w1, &w0), &f.sub(&b, &a));
        });
        sums.map(|s| f.reduce(s))
    }
    #[tracing::instrument(skip_all, name = "falcon_algebraic:word_spare_fold")]
    fn fold(&mut self, r: F, f: &Cfg) {
        // Compact in place; the write cursor never overtakes unread entries.
        let mut read = 0;
        let mut write = 0;
        while read < self.0.len() {
            let (index, value) = self.0[read];
            read += 1;
            let (a, mut b) = if index & 1 == 0 {
                (value, f.zero())
            } else {
                (f.zero(), value)
            };
            if index & 1 == 0 && read < self.0.len() && self.0[read].0 == index + 1 {
                b = self.0[read].1;
                read += 1;
            }
            let value = lerp(a, b, r, f);
            if value != f.zero() {
                self.0[write] = (index / 2, value);
                write += 1;
            }
        }
        self.0.truncate(write);
    }
    /// Project the original unit-valued slack entries after coefficient and
    /// side challenges. Entries stay grouped by instance throughout the scan.
    #[tracing::instrument(skip_all, name = "falcon_algebraic:word_spare_project")]
    fn project_instances(&mut self, local_point: &[F], f: &Cfg) -> Result<(), FalconError> {
        let weights = eq_table(local_point, f).map_err(failure)?;
        let mut read = 0;
        let mut write = 0;
        while read < self.0.len() {
            let instance = self.0[read].0 / (2 * N);
            let mut sum = f.zero();
            while read < self.0.len() && self.0[read].0 / (2 * N) == instance {
                let (index, bit) = self.0[read];
                debug_assert_eq!(bit, f.one());
                sum = f.add(&sum, &weights[index % (2 * N)]);
                read += 1;
            }
            if sum != f.zero() {
                self.0[write] = (instance, sum);
                write += 1;
            }
        }
        self.0.truncate(write);
        Ok(())
    }
    fn terminal(&self, f: &Cfg) -> F {
        self.0.first().map_or(f.zero(), |&(_, v)| v)
    }
}

/// Before the instance rounds, H is supported on side zero of active
/// instances. U_j = sum_i slack_i*H_ij and V_j = sum_i H_ij therefore reduce
/// its contribution to P*U + (gamma-gamma*M)*V over at most 32 positions.
struct AggregateSpare(Vec<[F; 2]>);
impl AggregateSpare {
    #[tracing::instrument(skip_all, name = "falcon_algebraic:word_spare_setup")]
    fn new(source: &Sparse, weights: &PublicWeights, f: &Cfg) -> Self {
        let mut values = vec![[f.zero(); 2]; SLACK_BITS.next_power_of_two()];
        for &(index, bit) in &source.0 {
            debug_assert_eq!(bit, f.one());
            let instance = index / (2 * N);
            let j = index % (2 * N);
            debug_assert!(j < SLACK_BITS);
            values[j][0] = f.add(&values[j][0], &weights.slacks[instance]);
            values[j][1] = f.add(&values[j][1], &bit);
        }
        Self(values)
    }

    fn initial(&self, weights: &PublicWeights, f: &Cfg) -> F {
        // Every original H entry is an active slack bit, so its padding
        // weight is zero and the instance sum has already been collected.
        let mut sum = FpProductAcc::<2>::default();
        for (j, &[u, _]) in self.0.iter().enumerate() {
            f.mul_acc(&mut sum, &weights.powers[j], &u);
        }
        f.reduce(sum)
    }

    #[tracing::instrument(skip_all, name = "falcon_algebraic:word_spare_prefix_sums", fields(round = round))]
    fn sums(&self, weights: &PublicWeights, round: usize, f: &Cfg) -> [F; 2] {
        if round == COEFFICIENT_LOG {
            // H(x)=(1-x)V while its public weight has one more side factor.
            let [u, v] = self.0[0];
            let quadratic = f.sub(
                &f.mul(&weights.powers[0], &u),
                &f.mul(&weights.padding_positions[0], &v),
            );
            return [f.add(&quadratic, &f.mul(&weights.padding, &v)), quadratic];
        }
        let mut sums = [FpProductAcc::<2>::default(); 2];
        for (i, pair) in self.0.chunks(2).enumerate() {
            let [u0, v0] = pair[0];
            let [u1, v1] = pair.get(1).copied().unwrap_or([f.zero(); 2]);
            let p0 = weights.powers[2 * i];
            let p1 = weights.powers[2 * i + 1];
            let m0 = weights.padding_positions[2 * i];
            let m1 = weights.padding_positions[2 * i + 1];
            f.mul_acc(&mut sums[0], &p0, &u0);
            f.mul_acc(&mut sums[0], &f.sub(&weights.padding, &m0), &v0);
            f.mul_acc(&mut sums[1], &f.sub(&p1, &p0), &f.sub(&u1, &u0));
            f.mul_acc(&mut sums[1], &f.sub(&m0, &m1), &f.sub(&v1, &v0));
        }
        sums.map(|sum| f.reduce(sum))
    }

    fn fold(&mut self, r: F, f: &Cfg) {
        if self.0.len() == 1 {
            // Unrepresented high coefficient coordinates have a zero partner.
            let factor = f.sub(&f.one(), &r);
            self.0[0] = self.0[0].map(|value| f.mul(&factor, &value));
        } else {
            for i in 0..self.0.len() / 2 {
                self.0[i] =
                    std::array::from_fn(|k| lerp(self.0[2 * i][k], self.0[2 * i + 1][k], r, f));
            }
            self.0.truncate(self.0.len() / 2);
        }
    }
}

fn signed_acc(acc: &mut [FpLinearAcc<2, 1>; 2], w: &F, x: i64, f: &Cfg) {
    if x != 0 {
        f.mul_acc(&mut acc[usize::from(x < 0)], w, &x.unsigned_abs());
    }
}
fn reduce_signed(acc: [FpLinearAcc<2, 1>; 2], f: &Cfg) -> F {
    let [p, n] = acc;
    f.sub(&f.reduce(p), &f.reduce(n))
}

/// Before the first challenge D is signed15. Products need a wide accumulator,
/// not u128: a prime-field coefficient itself can already occupy 126 bits.
#[cfg(test)]
fn first_round(form: &BindingForm<'_>, source: &Source, f: &Cfg) -> ([F; 2], F) {
    first_round_with_factors(
        form,
        source,
        &WordFactors::default(),
        f,
        arithmetic_experiment("BITZ_ARITHMETIC_WORD_INITIAL", false),
    )
}

/// At most one dense first-fold allocation, owned by a prepared prover's lease.
#[derive(Default)]
pub(in super::super) struct WordWorkspace {
    dense: Vec<[F; 2]>,
}

impl WordWorkspace {
    pub(in super::super) fn capacity_slots(&self) -> usize {
        self.dense.capacity()
    }
}

pub(super) fn workspace_reuse_enabled() -> bool {
    arithmetic_experiment("BITZ_ARITHMETIC_WORD_SCRATCH", false)
}

fn dense_compaction_threshold() -> Option<usize> {
    #[cfg(feature = "bench-internals")]
    return match std::env::var("BITZ_ARITHMETIC_DENSE_COMPACTION") {
        Err(std::env::VarError::NotPresent) => None,
        Ok(value) => match value.as_str() {
            "off" => None,
            "2" => Some(2),
            "4" => Some(4),
            "8" => Some(8),
            _ => panic!("BITZ_ARITHMETIC_DENSE_COMPACTION must be off, 2, 4, or 8"),
        },
        Err(std::env::VarError::NotUnicode(_)) => {
            panic!("BITZ_ARITHMETIC_DENSE_COMPACTION must be valid Unicode")
        }
    };
    #[cfg(not(feature = "bench-internals"))]
    None
}

fn compact_dense_output_override() -> Option<bool> {
    #[cfg(feature = "bench-internals")]
    return match std::env::var("BITZ_ARITHMETIC_DENSE_OUTPUT") {
        Err(std::env::VarError::NotPresent) => None,
        Ok(value) => match value.as_str() {
            "inplace" => Some(false),
            "compact" => Some(true),
            _ => panic!("BITZ_ARITHMETIC_DENSE_OUTPUT must be inplace or compact"),
        },
        Err(std::env::VarError::NotUnicode(_)) => {
            panic!("BITZ_ARITHMETIC_DENSE_OUTPUT must be valid Unicode")
        }
    };
    #[cfg(not(feature = "bench-internals"))]
    None
}

fn select_compact_dense_output(
    batch: usize,
    workers: usize,
    modulus: u128,
    compaction_threshold: Option<usize>,
    explicit: Option<bool>,
) -> bool {
    assert!(
        explicit != Some(true) || compaction_threshold.is_none(),
        "BITZ_ARITHMETIC_DENSE_COMPACTION must be off when BITZ_ARITHMETIC_DENSE_OUTPUT=compact"
    );
    // Cold/repeated qualification selected only this full-batch geometry.
    // An explicitly selected older compaction experiment suppresses the
    // production default, so its strided kernel remains independently usable.
    explicit.unwrap_or(
        compaction_threshold.is_none()
            && N == 512
            && batch == 1024
            && workers == 12
            && modulus >= 1u128 << 125,
    )
}

#[derive(Clone, Copy, Debug, Default)]
struct WordOptions {
    reuse_initial: bool,
    fused_fold: bool,
    direct_init: bool,
    scratch: bool,
    factored_initial_fused: bool,
    factored_fold_reduction: bool,
    dense_compaction: Option<usize>,
    compact_dense_output: bool,
}

impl WordOptions {
    fn configured(layout: &Layout, f: &Cfg) -> Self {
        let dense_compaction = dense_compaction_threshold();
        #[cfg(feature = "parallel")]
        let workers = rayon::current_num_threads();
        #[cfg(not(feature = "parallel"))]
        let workers = 1;
        let compact_dense_output = select_compact_dense_output(
            layout.batch(),
            workers,
            f.modulus_u128(),
            dense_compaction,
            compact_dense_output_override(),
        );
        Self {
            reuse_initial: arithmetic_experiment("BITZ_ARITHMETIC_WORD_INITIAL", false),
            fused_fold: arithmetic_experiment("BITZ_ARITHMETIC_WORD_FUSION", false),
            // Qualification improved 8/16 workers; one worker keeps zero initialization.
            direct_init: arithmetic_experiment(
                "BITZ_ARITHMETIC_WORD_DIRECT_INIT",
                rayon::current_num_threads() > 1,
            ),
            scratch: workspace_reuse_enabled(),
            factored_initial_fused: arithmetic_experiment(
                "BITZ_ARITHMETIC_FACTORED_INITIAL_FUSED",
                false,
            ),
            factored_fold_reduction: arithmetic_experiment(
                "BITZ_ARITHMETIC_FACTORED_FOLD_REDUCTION",
                false,
            ),
            dense_compaction,
            compact_dense_output,
        }
    }
}

#[derive(Default)]
pub(super) struct WordFactors {
    pub norm: Option<FactoredNorm>,
    pub s1: Option<FactoredS1>,
}

pub(super) struct FactoredS1 {
    pub rows: Vec<F>,
    pub columns: Vec<F>,
}

pub(super) struct FactoredNorm {
    first: F,
    // Equality weights of the remaining coefficient bits, shared by all rows.
    columns: Vec<F>,
    // eta * eq(side) * eq(instance), with no entries for inactive instances.
    rows: [Vec<F>; 2],
}

impl FactoredNorm {
    pub(super) fn new(
        layout: &Layout,
        norm: &NormClaims,
        eta: F,
        f: &Cfg,
    ) -> Result<Self, FalconError> {
        let columns = eq_table(&norm.point[1..COEFFICIENT_LOG], f).map_err(failure)?;
        let instances = eq_table(&norm.point[COEFFICIENT_LOG + 1..], f).map_err(failure)?;
        let sides = [
            f.sub(&f.one(), &norm.point[COEFFICIENT_LOG]),
            norm.point[COEFFICIENT_LOG],
        ];
        Ok(Self {
            first: norm.point[0],
            columns,
            rows: sides.map(|side| {
                let scale = f.mul(&eta, &side);
                instances[..layout.batch()]
                    .iter()
                    .map(|i| f.mul(&scale, i))
                    .collect()
            }),
        })
    }

    fn first_round(&self, source: &Source, f: &Cfg) -> ([F; 2], F) {
        let parts: Vec<_> = crate::cfg_into_iter!(0..self.rows[0].len())
            .map(|instance| {
                let mut totals = [f.zero(); 2];
                for side in 0..2 {
                    let mut sums = [[FpLinearAcc::<2, 1>::default(); 2]; 2];
                    for (j, weight) in self.columns.iter().enumerate() {
                        let i = instance * 2 * N + side * N + 2 * j;
                        signed_acc(&mut sums[0], weight, decode(word(source, i)), f);
                        signed_acc(&mut sums[1], weight, decode(word(source, i + 1)), f);
                    }
                    for (total, sum) in totals.iter_mut().zip(sums) {
                        *total = f.add(
                            total,
                            &f.mul(&self.rows[side][instance], &reduce_signed(sum, f)),
                        );
                    }
                }
                totals
            })
            .collect();
        let [even, odd] = parts
            .into_iter()
            .fold([f.zero(); 2], |a, b| add_pair(a, b, f));
        let c0 = f.mul(&f.sub(&f.one(), &self.first), &even);
        let c2 = f.mul(
            &f.sub(&f.add(&self.first, &self.first), &f.one()),
            &f.sub(&odd, &even),
        );
        ([c0, c2], f.add(&c0, &f.mul(&self.first, &odd)))
    }

    fn folded_rows(&self, r: F, f: &Cfg) -> [Vec<F>; 2] {
        let eq = f.add(
            &f.mul(&self.first, &r),
            &f.mul(&f.sub(&f.one(), &self.first), &f.sub(&f.one(), &r)),
        );
        self.rows
            .each_ref()
            .map(|rows| rows.iter().map(|v| f.mul(v, &eq)).collect())
    }
}

/// The dense term's first polynomial and Boolean sum, computed before its
/// challenge. Sparse spare lanes and inactive-source validation remain in prove.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct InitialClaim {
    round: [F; 2],
    sum: F,
}

/// Construct adjacent coefficients and consume them while still in registers.
/// A task contains at most 1024 word pairs, matching the reference first-round
/// accumulator bound (two degree-512 instances or one degree-1024 instance).
#[allow(clippy::too_many_arguments)]
pub(super) fn construct_and_initial(
    coefficients: &mut [Vec<F>; 2],
    source: &Source,
    coefficient_weights: &[F],
    norm_instances: &[F],
    eta: F,
    side_weights: [F; 2],
    f: &Cfg,
    parallel: bool,
    reuse_product: bool,
) -> Result<InitialClaim, FalconError> {
    const INSTANCES_PER_CHUNK: usize = 1024 / N;
    let fill = |(chunk, (s1, s2)): (usize, (&mut [F], &mut [F]))| {
        let mut sums = [[FpLinearAcc::<2, 1>::default(); 2]; 3];
        for (local, (s1, s2)) in s1.chunks_mut(N).zip(s2.chunks_mut(N)).enumerate() {
            // Validate original projections before performing field arithmetic,
            // as the baseline optimized constructor does for each instance.
            validate_field_elements(s1, f).map_err(failure)?;
            validate_field_elements(s2, f).map_err(failure)?;
            let instance = chunk * INSTANCES_PER_CHUNK + local;
            for (side, values) in [s1, s2].into_iter().enumerate() {
                let scale = f.mul(&f.mul(&eta, &side_weights[side]), &norm_instances[instance]);
                for (j, pair) in values.chunks_exact_mut(2).enumerate() {
                    let w0 = f.add(&pair[0], &f.mul(&scale, &coefficient_weights[2 * j]));
                    let w1 = f.add(&pair[1], &f.mul(&scale, &coefficient_weights[2 * j + 1]));
                    pair[0] = w0;
                    pair[1] = w1;
                    let i = instance * 2 * N + side * N + 2 * j;
                    let a = decode(word(source, i));
                    let b = decode(word(source, i + 1));
                    signed_acc(&mut sums[0], &w0, a, f);
                    signed_acc(&mut sums[1], &f.sub(&w1, &w0), b - a, f);
                    if !reuse_product {
                        signed_acc(&mut sums[2], &w0, a, f);
                    }
                    signed_acc(&mut sums[2], &w1, b, f);
                }
            }
        }
        Ok::<_, FalconError>(sums.map(|s| reduce_signed(s, f)))
    };
    let [s1, s2] = coefficients;
    let parts: Vec<[F; 3]> = if parallel {
        s1.par_chunks_mut(INSTANCES_PER_CHUNK * N)
            .zip(s2.par_chunks_mut(INSTANCES_PER_CHUNK * N))
            .enumerate()
            .map(fill)
            .collect::<Result<_, _>>()?
    } else {
        s1.chunks_mut(INSTANCES_PER_CHUNK * N)
            .zip(s2.chunks_mut(INSTANCES_PER_CHUNK * N))
            .enumerate()
            .map(fill)
            .collect::<Result<_, _>>()?
    };
    let mut sums = parts.into_iter().fold([f.zero(); 3], |a, b| {
        std::array::from_fn(|i| f.add(&a[i], &b[i]))
    });
    if reuse_product {
        sums[2] = f.add(&sums[2], &sums[0]);
    }
    Ok(InitialClaim {
        round: [sums[0], sums[1]],
        sum: sums[2],
    })
}

fn first_round_with_factors(
    form: &BindingForm<'_>,
    source: &Source,
    factors: &WordFactors,
    f: &Cfg,
    reuse_product: bool,
) -> ([F; 2], F) {
    let chunks = (form.layout.source_bits() / 32).div_ceil(1024);
    let parts: Vec<_> = crate::cfg_into_iter!(0..chunks)
        .map(|chunk| {
            let mut sums = [[FpLinearAcc::<2, 1>::default(); 2]; 3];
            let end = ((chunk + 1) * 1024).min(form.layout.source_bits() / 32);
            for i in chunk * 1024..end {
                if factors.s1.is_some() && (2 * i / N) & 1 == 0 {
                    continue;
                }
                let a = decode(word(source, 2 * i));
                let b = decode(word(source, 2 * i + 1));
                let w0 = coefficient(form, 2 * i);
                let w1 = coefficient(form, 2 * i + 1);
                signed_acc(&mut sums[0], &w0, a, f);
                signed_acc(&mut sums[1], &f.sub(&w1, &w0), b - a, f);
                if !reuse_product {
                    signed_acc(&mut sums[2], &w0, a, f);
                }
                signed_acc(&mut sums[2], &w1, b, f);
            }
            sums.map(|s| reduce_signed(s, f))
        })
        .collect();
    let mut sum = parts.into_iter().fold([f.zero(); 3], |a, b| {
        std::array::from_fn(|i| f.add(&a[i], &b[i]))
    });
    if reuse_product {
        sum[2] = f.add(&sum[2], &sum[0]);
    }
    if let Some(s1) = &factors.s1 {
        let parts: Vec<_> = crate::cfg_into_iter!(0..form.layout.batch())
            .map(|instance| {
                let mut sums = [[FpLinearAcc::<2, 1>::default(); 2]; 3];
                for (j, pair) in s1.columns.chunks_exact(2).enumerate() {
                    let i = instance * 2 * N + 2 * j;
                    let a = decode(word(source, i));
                    let b = decode(word(source, i + 1));
                    signed_acc(&mut sums[0], &pair[0], a, f);
                    signed_acc(&mut sums[1], &f.sub(&pair[1], &pair[0]), b - a, f);
                    signed_acc(&mut sums[2], &pair[1], b, f);
                }
                let mut values = sums.map(|s| f.mul(&s1.rows[instance], &reduce_signed(s, f)));
                values[2] = f.add(&values[2], &values[0]);
                values
            })
            .collect();
        for part in parts {
            for i in 0..3 {
                sum[i] = f.add(&sum[i], &part[i]);
            }
        }
    }
    if let Some(norm) = &factors.norm {
        let (next, initial) = norm.first_round(source, f);
        sum[0] = f.add(&sum[0], &next[0]);
        sum[1] = f.add(&sum[1], &next[1]);
        sum[2] = f.add(&sum[2], &initial);
    }
    ([sum[0], sum[1]], sum[2])
}

fn first_round_factored_fused(
    form: &BindingForm<'_>,
    source: &Source,
    s1: &FactoredS1,
    norm: &FactoredNorm,
    f: &Cfg,
    reuse_product: bool,
) -> ([F; 2], F) {
    // These differences depend only on the shared columns, not the instance.
    let differences: Vec<_> = s1
        .columns
        .chunks_exact(2)
        .map(|pair| f.sub(&pair[1], &pair[0]))
        .collect();
    let parts: Vec<_> = crate::cfg_into_iter!(0..form.layout.batch())
        .map(|instance| {
            let mut totals = [f.zero(); 5];
            for side in 0..2 {
                let mut ring = [[FpLinearAcc::<2, 1>::default(); 2]; 3];
                let mut correction = [[FpLinearAcc::<2, 1>::default(); 2]; 2];
                for (j, norm_weight) in norm.columns.iter().enumerate() {
                    let i = instance * 2 * N + side * N + 2 * j;
                    let a = decode(word(source, i));
                    let b = decode(word(source, i + 1));
                    let (w0, w1, difference) = if side == 0 {
                        (s1.columns[2 * j], s1.columns[2 * j + 1], differences[j])
                    } else {
                        let w0 = form.coefficients[1][instance * N + 2 * j];
                        let w1 = form.coefficients[1][instance * N + 2 * j + 1];
                        (w0, w1, f.sub(&w1, &w0))
                    };
                    signed_acc(&mut ring[0], &w0, a, f);
                    signed_acc(&mut ring[1], &difference, b - a, f);
                    if side != 0 && !reuse_product {
                        signed_acc(&mut ring[2], &w0, a, f);
                    }
                    signed_acc(&mut ring[2], &w1, b, f);
                    signed_acc(&mut correction[0], norm_weight, a, f);
                    signed_acc(&mut correction[1], norm_weight, b, f);
                }
                let mut values = ring.map(|sum| reduce_signed(sum, f));
                if side == 0 || reuse_product {
                    values[2] = f.add(&values[2], &values[0]);
                }
                for k in 0..3 {
                    let value = if side == 0 {
                        f.mul(&s1.rows[instance], &values[k])
                    } else {
                        values[k]
                    };
                    totals[k] = f.add(&totals[k], &value);
                }
                for (k, sum) in correction.into_iter().enumerate() {
                    totals[3 + k] = f.add(
                        &totals[3 + k],
                        &f.mul(&norm.rows[side][instance], &reduce_signed(sum, f)),
                    );
                }
            }
            totals
        })
        .collect();
    let [c0, c2, initial, even, odd] = parts.into_iter().fold([f.zero(); 5], |a, b| {
        std::array::from_fn(|k| f.add(&a[k], &b[k]))
    });
    let norm_c0 = f.mul(&f.sub(&f.one(), &norm.first), &even);
    let norm_c2 = f.mul(
        &f.sub(&f.add(&norm.first, &norm.first), &f.one()),
        &f.sub(&odd, &even),
    );
    (
        [f.add(&c0, &norm_c0), f.add(&c2, &norm_c2)],
        f.add(&initial, &f.add(&norm_c0, &f.mul(&norm.first, &odd))),
    )
}

fn dense_sums(values: &[[F; 2]], stride: usize, f: &Cfg) -> [F; 2] {
    let mut sums = [FpProductAcc::<2>::default(); 2];
    for pair in values.chunks_exact(2 * stride) {
        let [w0, a] = pair[0];
        let [w1, b] = pair[stride];
        f.mul_acc(&mut sums[0], &w0, &a);
        f.mul_acc(&mut sums[1], &f.sub(&w1, &w0), &f.sub(&b, &a));
    }
    sums.map(|s| f.reduce(s))
}

/// Allocate the sole dense product after the first word fold. Each slot holds
/// [Lambda, D]. Later folds reuse these slots at a doubling stride.
#[cfg(test)]
fn first_fold(form: &BindingForm<'_>, source: &Source, r: F, f: &Cfg) -> (Vec<[F; 2]>, [F; 2]) {
    first_fold_with_factors(
        form,
        source,
        r,
        &WordFactors::default(),
        f,
        arithmetic_experiment("BITZ_ARITHMETIC_WORD_FUSION", false),
    )
}

#[cfg(test)]
fn first_fold_with_factors(
    form: &BindingForm<'_>,
    source: &Source,
    r: F,
    factors: &WordFactors,
    f: &Cfg,
    fused: bool,
) -> (Vec<[F; 2]>, [F; 2]) {
    let mut values = Vec::new();
    let options = WordOptions {
        fused_fold: fused,
        ..WordOptions::default()
    };
    let sums = first_fold_into(form, source, r, factors, f, options, &mut values);
    (values, sums)
}

#[tracing::instrument(skip_all, name = "falcon_algebraic:word_first_fold")]
fn first_fold_into(
    form: &BindingForm<'_>,
    source: &Source,
    r: F,
    factors: &WordFactors,
    f: &Cfg,
    options: WordOptions,
    values: &mut Vec<[F; 2]>,
) -> [F; 2] {
    // Flipping bit14 maps signed15 a to a+2^14. Prepared weights include the bias.
    let prepared = <Cfg as PreparedLinearCombination<u64>>::prepare_linear_combination(
        f,
        [f.sub(&f.one(), &r), r, f.neg(&small(1 << 14, f))],
    );
    let norm_rows = factors.norm.as_ref().map(|norm| norm.folded_rows(r, f));
    let s1_columns: Option<Vec<_>> = factors.s1.as_ref().map(|s1| {
        s1.columns
            .chunks_exact(2)
            .map(|p| lerp(p[0], p[1], r, f))
            .collect()
    });
    let product_reduction = options
        .factored_fold_reduction
        .then(|| f.prepare_product_reduction(2));
    let slot = |index: usize| {
        let i = 2 * index;
        let instance = i / (2 * N);
        let side = (i / N) & 1;
        let weight = if options.factored_fold_reduction
            && instance < form.layout.batch()
            && factors.norm.is_some()
        {
            let norm = factors.norm.as_ref().unwrap();
            let rows = norm_rows.as_ref().unwrap();
            let mut sum = FpProductAcc::<2>::default();
            f.mul_acc(&mut sum, &rows[side][instance], &norm.columns[(i % N) / 2]);
            if side == 0 && factors.s1.is_some() {
                f.mul_acc(
                    &mut sum,
                    &factors.s1.as_ref().unwrap().rows[instance],
                    &s1_columns.as_ref().unwrap()[(i % N) / 2],
                );
                product_reduction.as_ref().unwrap().reduce(sum)
            } else {
                let left = coefficient(form, i);
                f.mul_acc(&mut sum, &r, &f.sub(&coefficient(form, i + 1), &left));
                f.add(&left, &product_reduction.as_ref().unwrap().reduce(sum))
            }
        } else {
            let mut weight = if let (Some(s1), Some(columns)) = (&factors.s1, &s1_columns) {
                if side == 0 {
                    if instance < form.layout.batch() {
                        f.mul(&s1.rows[instance], &columns[(i % N) / 2])
                    } else {
                        f.zero()
                    }
                } else {
                    lerp(coefficient(form, i), coefficient(form, i + 1), r, f)
                }
            } else {
                lerp(coefficient(form, i), coefficient(form, i + 1), r, f)
            };
            if instance < form.layout.batch() {
                if let (Some(norm), Some(rows)) = (&factors.norm, &norm_rows) {
                    weight = f.add(
                        &weight,
                        &f.mul(&rows[side][instance], &norm.columns[(i % N) / 2]),
                    );
                }
            }
            weight
        };
        let words = [word(source, i), word(source, i + 1)];
        [
            weight,
            <Cfg as PreparedLinearCombination<u64>>::linear_combination(&prepared, |k| {
                if k == 2 {
                    1
                } else {
                    u64::from((words[k] ^ 0x4000) & 0x7fff)
                }
            }),
        ]
    };
    initialize_dense(
        values,
        form.layout.source_bits() / 32,
        f,
        options.direct_init,
        options.fused_fold,
        slot,
    )
}

fn initialize_dense(
    values: &mut Vec<[F; 2]>,
    len: usize,
    f: &Cfg,
    direct: bool,
    fused: bool,
    slot: impl Fn(usize) -> [F; 2] + Sync,
) -> [F; 2] {
    initialize_dense_chunks::<1024>(values, len, f, direct, fused, slot)
}

fn initialize_dense_chunks<const CHUNK: usize>(
    values: &mut Vec<[F; 2]>,
    len: usize,
    f: &Cfg,
    direct: bool,
    fused: bool,
    slot: impl Fn(usize) -> [F; 2] + Sync,
) -> [F; 2] {
    // The fused initializer writes pairs; reject an odd tail before exposing
    // any initialized slice or changing the Vec length.
    assert_eq!(len % 2, 0);
    assert!(CHUNK > 0 && CHUNK % 2 == 0);
    let parts: Vec<[F; 2]> = if direct {
        // Every output is overwritten. Keep length zero until all worker writes
        // finish so unwinding can never expose an uninitialized field element.
        values.clear();
        if values.capacity() < len {
            values.reserve_exact(len);
        }
        let spare = &mut values.spare_capacity_mut()[..len];
        let parts = crate::cfg_chunks_mut!(spare, CHUNK)
            .enumerate()
            .map(|(chunk, out)| {
                if fused {
                    let mut sums = [FpProductAcc::<2>::default(); 2];
                    for (j, pair) in out.chunks_exact_mut(2).enumerate() {
                        let a = slot(chunk * CHUNK + 2 * j);
                        let b = slot(chunk * CHUNK + 2 * j + 1);
                        pair[0].write(a);
                        pair[1].write(b);
                        f.mul_acc(&mut sums[0], &a[0], &a[1]);
                        f.mul_acc(&mut sums[1], &f.sub(&b[0], &a[0]), &f.sub(&b[1], &a[1]));
                    }
                    sums.map(|s| f.reduce(s))
                } else {
                    for (j, out) in out.iter_mut().enumerate() {
                        out.write(slot(chunk * CHUNK + j));
                    }
                    // SAFETY: this worker initialized every element of its disjoint
                    // chunk above. Only a shared slice is exposed, after all writes;
                    // MaybeUninit<T> has T's layout and no mutable T slice is formed.
                    let initialized = unsafe {
                        std::slice::from_raw_parts(out.as_ptr().cast::<[F; 2]>(), out.len())
                    };
                    dense_sums(initialized, 1, f)
                }
            })
            .collect();
        // SAFETY: chunks cover exactly len slots, and each slot is initialized
        // before the parallel iterator returns. The checked length and chunk
        // size CHUNK are even, so the fused pair loop has no unwritten tail.
        // On panic length stays zero; [F;2] is Copy and owns no drop resources.
        unsafe {
            values.set_len(len);
        }
        parts
    } else {
        if values.capacity() < len {
            *values = vec![[f.zero(); 2]; len];
        } else {
            // Existing initialized slots will all be overwritten below. Only
            // initialize a new tail if this lease previously held fewer slots.
            values.resize(len, [f.zero(); 2]);
        }
        crate::cfg_chunks_mut!(values, CHUNK)
            .enumerate()
            .map(|(chunk, out)| {
                if fused {
                    let mut sums = [FpProductAcc::<2>::default(); 2];
                    for (j, pair) in out.chunks_exact_mut(2).enumerate() {
                        let a = slot(chunk * CHUNK + 2 * j);
                        let b = slot(chunk * CHUNK + 2 * j + 1);
                        pair[0] = a;
                        pair[1] = b;
                        f.mul_acc(&mut sums[0], &a[0], &a[1]);
                        f.mul_acc(&mut sums[1], &f.sub(&b[0], &a[0]), &f.sub(&b[1], &a[1]));
                    }
                    sums.map(|s| f.reduce(s))
                } else {
                    for (j, out) in out.iter_mut().enumerate() {
                        *out = slot(chunk * CHUNK + j);
                    }
                    dense_sums(out, 1, f)
                }
            })
            .collect()
    };
    parts
        .into_iter()
        .fold([f.zero(); 2], |a, b| add_pair(a, b, f))
}

fn dense_fold(values: &mut [[F; 2]], stride: usize, r: F, f: &Cfg) -> [F; 2] {
    let _span = tracing::info_span!("falcon_algebraic:word_dense_fold", stride).entered();
    if values.len() / stride == 2 {
        values[0] = std::array::from_fn(|k| lerp(values[0][k], values[stride][k], r, f));
        return [f.zero(); 2];
    }
    let fold = |block: &mut [[F; 2]]| {
        for pair in block.chunks_exact_mut(2 * stride) {
            pair[0] = std::array::from_fn(|k| lerp(pair[0][k], pair[stride][k], r, f));
        }
        dense_sums(block, 2 * stride, f)
    };
    #[cfg(feature = "parallel")]
    if values.len() / stride >= 4096 && rayon::current_num_threads() > 1 {
        return values
            .par_chunks_mut(1024 * stride)
            .map(fold)
            .reduce(|| [f.zero(); 2], |a, b| add_pair(a, b, f));
    }
    fold(values)
}

/// Write folded survivors consecutively, without a second compaction pass.
/// The alternate allocation is local to this proof: at most half the initial
/// dense allocation, in addition to that allocation. It is not cached in Prepared.
fn dense_fold_compact_output(
    values: &mut Vec<[F; 2]>,
    output: &mut Vec<[F; 2]>,
    stride: &mut usize,
    r: F,
    f: &Cfg,
) -> [F; 2] {
    let _span = tracing::info_span!(
        "falcon_algebraic:word_dense_fold",
        stride = *stride,
        compact = true
    )
    .entered();
    assert!(*stride > 0 && values.len() % *stride == 0);
    let live = values.len() / *stride;
    assert!(live >= 2 && live.is_power_of_two());
    if live == 2 {
        values[0] = std::array::from_fn(|k| lerp(values[0][k], values[*stride][k], r, f));
        values.truncate(1);
        *stride = 1;
        return [f.zero(); 2];
    }
    let slot = |i: usize| {
        let left = values[2 * i * *stride];
        let right = values[(2 * i + 1) * *stride];
        std::array::from_fn(|k| lerp(left[k], right[k], r, f))
    };
    // Match dense_fold's 1024 live input entries per task and its parallel
    // cutoff. Existing initialization writes each output before exposing it.
    #[cfg(feature = "parallel")]
    let parallel = live >= 4096 && rayon::current_num_threads() > 1;
    #[cfg(not(feature = "parallel"))]
    let parallel = false;
    let sums = if parallel {
        initialize_dense_chunks::<512>(output, live / 2, f, true, false, slot)
    } else {
        output.clear();
        output.extend((0..live / 2).map(slot));
        dense_sums(output, 1, f)
    };
    std::mem::swap(values, output);
    *stride = 1;
    sums
}

/// Move the live strided entries to the front without allocating or changing
/// the retained capacity. The forward write cursor never overtakes an unread
/// entry. This extra serial copy is experimental: saved sparse accesses in
/// later rounds must outweigh it before enabling a threshold by default.
fn compact_dense(values: &mut Vec<[F; 2]>, stride: usize) {
    assert!(stride > 0 && values.len() % stride == 0);
    let live = values.len() / stride;
    let _span =
        tracing::info_span!("falcon_algebraic:word_dense_compaction", stride, live).entered();
    for i in 1..live {
        values[i] = values[i * stride];
    }
    values.truncate(live);
}

#[inline]
fn dense_fold_with_compaction(
    values: &mut Vec<[F; 2]>,
    stride: &mut usize,
    r: F,
    f: &Cfg,
    threshold: Option<usize>,
) -> [F; 2] {
    let sums = dense_fold(values, *stride, r, f);
    *stride *= 2;
    if threshold.is_some_and(|threshold| *stride >= threshold) {
        compact_dense(values, *stride);
        *stride = 1;
    }
    sums
}

/// Padding is checked while scanning the source, so C contributes zero until
/// the instance rounds. Evaluate its local MLE once instead of folding a
/// third full-size field table. The verifier never assumes padding is zero.
#[tracing::instrument(skip_all, name = "falcon_algebraic:word_padding_counts")]
fn counts(
    source: &Source,
    layout: &Layout,
    local_point: &[F],
    f: &Cfg,
) -> Result<Vec<F>, FalconError> {
    let weights = eq_table(local_point, f).map_err(failure)?;
    Ok(crate::cfg_into_iter!(0..layout.capacity())
        .map(|i| {
            if i >= layout.batch() {
                return f.zero();
            }
            let mut sum = FpLinearAcc::<2, 1>::default();
            for (j, w) in weights.iter().enumerate() {
                f.mul_acc(
                    &mut sum,
                    w,
                    &u64::from((word(source, i * 2 * N + j) & 0x7fff).count_ones()),
                );
            }
            f.reduce(sum)
        })
        .collect())
}
fn count_sums(values: &[F], weights: &PublicWeights, f: &Cfg) -> [F; 2] {
    let mut sums = [FpProductAcc::<2>::default(); 2];
    for (i, pair) in values.chunks_exact(2).enumerate() {
        let a = weights.count(2 * i, f);
        let b = weights.count(2 * i + 1, f);
        f.mul_acc(&mut sums[0], &a, &pair[0]);
        f.mul_acc(&mut sums[1], &f.sub(&b, &a), &f.sub(&pair[1], &pair[0]));
    }
    sums.map(|s| f.reduce(s))
}

fn bind_terminal(
    t: &mut impl Transcript,
    point: Vec<F>,
    values: &[F],
    zeta: F,
    f: &Cfg,
) -> WordBridgeClaim {
    let mut value = f.zero();
    for v in values.iter().rev() {
        value = f.add(v, &f.mul(&zeta, &value));
    }
    t.absorb_slice(b"bitz/falcon-algebraic/word-source-opening/v1");
    t.absorb_slice(&f.modulus_u128().to_le_bytes());
    absorb_field_elements(t, &point, f);
    absorb_field_elements(t, &[zeta, value], f);
    WordBridgeClaim {
        point,
        modulus: f.modulus_u128(),
        value,
        zeta,
        with_padding: values.len() == 3,
    }
}


/// The dense coefficient's multilinear extension at the terminal point:
/// dense ring weights, factored s1 columns, or keyed ring weights evaluated
/// from the public keys, plus the deferred norm correction when that is
/// factored. Shared by the separate-norm and merged-norm verifiers.
fn terminal_coefficient_value(
    layout: &Layout,
    form: &BindingForm<'_>,
    factors: &WordFactors,
    keyed: Option<&KeyedRing<'_>>,
    point: &[F],
    f: &Cfg,
) -> Result<F, FalconError> {
    let local = eq_table(&point[..COEFFICIENT_LOG + 1], f).map_err(failure)?;
    let instances = eq_table(&point[COEFFICIENT_LOG + 1..], f).map_err(failure)?;
    let reduction = f.prepare_product_reduction(2 * N);
    let s1_local = factors.s1.as_ref().map(|s1| {
        let mut sum = FpProductAcc::<2>::default();
        for (a, b) in s1.columns.iter().zip(&local[..N]) {
            f.mul_acc(&mut sum, a, b);
        }
        reduction.reduce(sum)
    });
    #[cfg(any(test, feature = "bench-internals"))]
    let keyed_factors = keyed.map(|ring| {
        (
            ring,
            super::super::keyed_ring::coefficient_factors(ring.points, &local[..N], &local[N..2 * N], f),
        )
    });
    #[cfg(not(any(test, feature = "bench-internals")))]
    let _ = keyed;
    let evaluate_instance = |instance: usize| {
        #[cfg(any(test, feature = "bench-internals"))]
        if let Some((ring, coefficient_factors)) = &keyed_factors {
            return super::super::keyed_ring::instance_value(
                ring.rows,
                coefficient_factors,
                &ring.keys[instance],
                instance,
                f,
            );
        }
        let mut sum = FpProductAcc::<2>::default();
        for side in 0..2 {
            if side == 0 && factors.s1.is_some() {
                continue;
            }
            for (coefficient, weight) in form.coefficients[side][instance * N..(instance + 1) * N]
                .iter()
                .zip(&local[side * N..(side + 1) * N])
            {
                f.mul_acc(&mut sum, coefficient, weight);
            }
        }
        let value = reduction.reduce(sum);
        if let (Some(s1), Some(column)) = (&factors.s1, s1_local) {
            f.add(&value, &f.mul(&s1.rows[instance], &column))
        } else {
            value
        }
    };
    // Keyed ring values read one public key each; spread them whenever there
    // are workers, independently of the dense binder's qualified worker counts.
    #[cfg(feature = "parallel")]
    let values: Option<Vec<_>> =
        if binding_parallel(binding_strategy(layout.batch(), BindingKernel::Evaluation))
            || (keyed.is_some() && rayon::current_num_threads() > 1)
        {
            Some(
                (0..layout.batch())
                    .into_par_iter()
                    .map(evaluate_instance)
                    .collect(),
            )
        } else {
            None
        };
    #[cfg(not(feature = "parallel"))]
    let values: Option<Vec<F>> = None;
    let mut coefficient_sum = FpProductAcc::<2>::default();
    for (instance, weight) in instances.iter().take(layout.batch()).enumerate() {
        let value = values
            .as_ref()
            .map_or_else(|| evaluate_instance(instance), |values| values[instance]);
        f.mul_acc(&mut coefficient_sum, &value, weight);
    }
    let mut coefficient_value = f.reduce(coefficient_sum);
    if let Some(norm) = &factors.norm {
        let mut correction = f.zero();
        for side in 0..2 {
            let mut row_sum = FpProductAcc::<2>::default();
            for (a, b) in norm.rows[side].iter().zip(&instances) {
                f.mul_acc(&mut row_sum, a, b);
            }
            let mut column_sum = FpProductAcc::<2>::default();
            for (j, weight) in local[side * N..(side + 1) * N].iter().enumerate() {
                let first = if j & 1 == 0 {
                    f.sub(&f.one(), &norm.first)
                } else {
                    norm.first
                };
                let coefficient = f.mul(&first, &norm.columns[j / 2]);
                f.mul_acc(&mut column_sum, &coefficient, weight);
            }
            correction = f.add(
                &correction,
                &f.mul(&f.reduce(row_sum), &f.reduce(column_sum)),
            );
        }
        coefficient_value = f.add(&coefficient_value, &correction);
    }
    Ok(coefficient_value)
}

/// The norm's term of the merged binder, `eta * eq(rho, instance) * D^2`:
/// `weights[i]` is the per-instance factor over the whole capacity (inactive
/// instances hold zero words, so their weight is immaterial).
pub(super) struct MergedNorm {
    pub eta: F,
    pub instance_point: Vec<F>,
    pub weights: Vec<F>,
}

impl MergedNorm {
    pub(super) fn new(
        layout: &Layout,
        eta: F,
        instance_point: Vec<F>,
        instance_weights: &[F],
        f: &Cfg,
    ) -> Result<Self, FalconError> {
        if instance_point.len() != layout.capacity().ilog2() as usize
            || instance_weights.len() != layout.capacity()
        {
            return Err(failure("merged norm dimensions"));
        }
        Ok(Self {
            eta,
            instance_point,
            weights: instance_weights.iter().map(|w| f.mul(&eta, w)).collect(),
        })
    }

    /// `eta * eq(rho, r)` at the binder's instance coordinates.
    fn terminal_weight(&self, instance_point: &[F], f: &Cfg) -> F {
        debug_assert_eq!(instance_point.len(), self.instance_point.len());
        let mut value = self.eta;
        for (rho, r) in self.instance_point.iter().zip(instance_point) {
            let both = f.mul(rho, r);
            // (1 - rho)(1 - r) + rho r
            let e = f.add(
                &f.sub(&f.sub(&f.one(), rho), r),
                &f.add(&both, &both),
            );
            value = f.mul(&value, &e);
        }
        value
    }
}

/// The s1 weights of the merged binder in factored form: the weight of
/// `s1[i][j]` is `sum_p rows[p][i] * columns[p][j]` (one rank per ring
/// projection point). The binder never materializes them.
pub(super) struct MergedS1 {
    rows: Vec<Vec<F>>,
    columns: Vec<Vec<F>>,
}

impl MergedS1 {
    pub(super) fn new(
        layout: &Layout,
        rows: Vec<Vec<F>>,
        columns: Vec<Vec<F>>,
        f: &Cfg,
    ) -> Result<Self, FalconError> {
        if rows.is_empty()
            || rows.len() != columns.len()
            || rows.iter().any(|rows| rows.len() != layout.batch())
            || columns.iter().any(|columns| columns.len() != N)
        {
            return Err(failure("factored s1 weight dimensions"));
        }
        for values in rows.iter().chain(&columns) {
            validate_field_elements(values, f).map_err(failure)?;
        }
        Ok(Self { rows, columns })
    }

    /// Column factors after the first challenge, `lerp` over adjacent pairs.
    fn folded_columns(&self, r: F, f: &Cfg) -> Vec<Vec<F>> {
        self.columns
            .iter()
            .map(|columns| {
                columns
                    .chunks_exact(2)
                    .map(|pair| lerp(pair[0], pair[1], r, f))
                    .collect()
            })
            .collect()
    }
}

/// Round-zero sums of the merged binder: the ring products read dense
/// coefficients (or the s1 factors) against signed15 words, and each
/// instance's square sums are native integers scaled by its norm weight.
/// Returns `([c0, c2, c3], total)` with `c3 = 0` (coefficient rounds are
/// quadratic) and `total` the Boolean sum of the dense and norm terms.
fn first_round_merged(
    form: &BindingForm<'_>,
    source: &Source,
    norm: &MergedNorm,
    s1: Option<&MergedS1>,
    f: &Cfg,
) -> ([F; 3], F) {
    let parts: Vec<[F; 3]> = crate::cfg_into_iter!(0..form.layout.batch())
        .map(|instance| {
            let mut ring = [[FpLinearAcc::<2, 1>::default(); 2]; 3];
            // sum a^2, sum (b-a)^2, sum a^2 + b^2: at most 2N * 2^30 < 2^41.
            let mut squares = [0u64; 3];
            let mut factored = [f.zero(); 3];
            for side in 0..2 {
                if side == 0 && let Some(s1) = s1 {
                    // sum_j (sum_p rows_p columns_p[j]) D_j = sum_p rows_p (sum_j columns_p[j] D_j)
                    let mut sums = vec![[[FpLinearAcc::<2, 1>::default(); 2]; 3]; s1.rows.len()];
                    for j in 0..N / 2 {
                        let i = instance * 2 * N + 2 * j;
                        let a = decode(word(source, i));
                        let b = decode(word(source, i + 1));
                        let d = b - a;
                        for (sums, columns) in sums.iter_mut().zip(&s1.columns) {
                            let (c0, c1) = (columns[2 * j], columns[2 * j + 1]);
                            signed_acc(&mut sums[0], &c0, a, f);
                            signed_acc(&mut sums[1], &f.sub(&c1, &c0), d, f);
                            // The Boolean total is c0 plus the odd half, added below.
                            signed_acc(&mut sums[2], &c1, b, f);
                        }
                        squares[0] += (a * a) as u64;
                        squares[1] += (d * d) as u64;
                        squares[2] += (a * a + b * b) as u64;
                    }
                    for (sums, rows) in sums.into_iter().zip(&s1.rows) {
                        let row = rows[instance];
                        let [c0, c2, odd] = sums.map(|sum| reduce_signed(sum, f));
                        let total = f.add(&c0, &odd);
                        for (k, value) in [c0, c2, total].into_iter().enumerate() {
                            factored[k] = f.add(&factored[k], &f.mul(&row, &value));
                        }
                    }
                    continue;
                }
                for j in 0..N / 2 {
                    let i = instance * 2 * N + side * N + 2 * j;
                    let a = decode(word(source, i));
                    let b = decode(word(source, i + 1));
                    let w0 = coefficient(form, i);
                    let w1 = coefficient(form, i + 1);
                    signed_acc(&mut ring[0], &w0, a, f);
                    signed_acc(&mut ring[1], &f.sub(&w1, &w0), b - a, f);
                    signed_acc(&mut ring[2], &w1, b, f);
                    let d = b - a;
                    squares[0] += (a * a) as u64;
                    squares[1] += (d * d) as u64;
                    squares[2] += (a * a + b * b) as u64;
                }
            }
            let weight = norm.weights[instance];
            let [c0, c2, odd] = ring.map(|sum| reduce_signed(sum, f));
            let dense = [c0, c2, f.add(&c0, &odd)];
            std::array::from_fn(|k| {
                f.add(
                    &f.add(&dense[k], &factored[k]),
                    &f.reduce(f.mul_wide(&weight, &squares[k])),
                )
            })
        })
        .collect();
    let [c0, c2, total] = parts.into_iter().fold([f.zero(); 3], |a, b| {
        std::array::from_fn(|k| f.add(&a[k], &b[k]))
    });
    ([c0, c2, f.zero()], total)
}

const MERGED_INSTANCES_PER_TASK: usize = 4;

/// The merged binder's first fold: `[Lambda', D']` per word pair, with the
/// s1 weights from their factors when given (`sum_p rows_p[i] columns'_p[j]`)
/// and dense coefficients otherwise. Returns the next round's ring sums.
fn first_fold_merged(
    form: &BindingForm<'_>,
    source: &Source,
    r: F,
    s1: Option<&MergedS1>,
    f: &Cfg,
    options: WordOptions,
    values: &mut Vec<[F; 2]>,
) -> [F; 2] {
    let _span = tracing::info_span!("falcon_algebraic:word_first_fold").entered();
    let Some(s1) = s1 else {
        return first_fold_into(form, source, r, &WordFactors::default(), f, options, values);
    };
    let prepared = <Cfg as PreparedLinearCombination<u64>>::prepare_linear_combination(
        f,
        [f.sub(&f.one(), &r), r, f.neg(&small(1 << 14, f))],
    );
    let columns = s1.folded_columns(r, f);
    let slot = |index: usize| {
        let i = 2 * index;
        let instance = i / (2 * N);
        let side = (i / N) & 1;
        let weight = if side == 0 {
            if instance < form.layout.batch() {
                let j = (i % N) / 2;
                s1.rows
                    .iter()
                    .zip(&columns)
                    .fold(f.zero(), |sum, (rows, columns)| {
                        f.add(&sum, &f.mul(&rows[instance], &columns[j]))
                    })
            } else {
                f.zero()
            }
        } else {
            lerp(coefficient(form, i), coefficient(form, i + 1), r, f)
        };
        let words = [word(source, i), word(source, i + 1)];
        [
            weight,
            <Cfg as PreparedLinearCombination<u64>>::linear_combination(&prepared, |k| {
                if k == 2 {
                    1
                } else {
                    u64::from((words[k] ^ 0x4000) & 0x7fff)
                }
            }),
        ]
    };
    initialize_dense(
        values,
        form.layout.source_bits() / 32,
        f,
        options.direct_init,
        options.fused_fold,
        slot,
    )
}

/// Weighted square sums of the next coefficient round over a dense table whose
/// instances own `N` consecutive slots, reading pairs `(pos, pos + stride)`.
fn norm_sums_strided(values: &[[F; 2]], stride: usize, norm: &MergedNorm, f: &Cfg) -> [F; 2] {
    debug_assert_eq!(values.len() % N, 0);
    let task = |(chunk, block): (usize, &[[F; 2]])| -> [F; 2] {
        let mut sums = [f.zero(); 2];
        for (local, block) in block.chunks_exact(N).enumerate() {
            let mut squares = [FpProductAcc::<2>::default(); 2];
            for pair in block.chunks_exact(2 * stride) {
                let a = pair[0][1];
                let d = f.sub(&pair[stride][1], &a);
                f.mul_acc(&mut squares[0], &a, &a);
                f.mul_acc(&mut squares[1], &d, &d);
            }
            let weight = norm.weights[chunk * MERGED_INSTANCES_PER_TASK + local];
            for k in 0..2 {
                sums[k] = f.add(&sums[k], &f.mul(&weight, &f.reduce(squares[k])));
            }
        }
        sums
    };
    #[cfg(feature = "parallel")]
    if values.len() / N >= 2 * MERGED_INSTANCES_PER_TASK && rayon::current_num_threads() > 1 {
        return values
            .par_chunks(MERGED_INSTANCES_PER_TASK * N)
            .enumerate()
            .map(task)
            .reduce(|| [f.zero(); 2], |a, b| add_pair(a, b, f));
    }
    values
        .chunks(MERGED_INSTANCES_PER_TASK * N)
        .enumerate()
        .map(task)
        .fold([f.zero(); 2], |a, b| add_pair(a, b, f))
}

/// Fold the dense `[Lambda, D]` table by `r` at `stride` and return the next
/// coefficient round's sums `[c0, c2, 0]`: ring products over the surviving
/// pairs plus each instance's weighted square sums. Instances own `N`
/// consecutive slots, so an instance's survivors never mix with another's.
fn dense_fold_merged(
    values: &mut [[F; 2]],
    stride: usize,
    r: F,
    norm: &MergedNorm,
    f: &Cfg,
) -> [F; 3] {
    let _span = tracing::info_span!("falcon_algebraic:word_dense_fold", stride).entered();
    debug_assert_eq!(values.len() % N, 0);
    let next = 2 * stride;
    let task = |(chunk, block): (usize, &mut [[F; 2]])| -> [F; 3] {
        let mut ring = [FpProductAcc::<2>::default(); 2];
        let mut sums = [f.zero(); 2];
        for (local, block) in block.chunks_exact_mut(N).enumerate() {
            for pair in block.chunks_exact_mut(2 * stride) {
                pair[0] = std::array::from_fn(|k| lerp(pair[0][k], pair[stride][k], r, f));
            }
            if 2 * next > N {
                // One survivor per instance: the instance rounds sum their own pairs.
                continue;
            }
            let mut squares = [FpProductAcc::<2>::default(); 2];
            for pair in block.chunks_exact(2 * next) {
                let [w0, a] = pair[0];
                let [w1, b] = pair[next];
                let d = f.sub(&b, &a);
                f.mul_acc(&mut ring[0], &w0, &a);
                f.mul_acc(&mut ring[1], &f.sub(&w1, &w0), &d);
                f.mul_acc(&mut squares[0], &a, &a);
                f.mul_acc(&mut squares[1], &d, &d);
            }
            let weight = norm.weights[chunk * MERGED_INSTANCES_PER_TASK + local];
            for k in 0..2 {
                sums[k] = f.add(&sums[k], &f.mul(&weight, &f.reduce(squares[k])));
            }
        }
        [
            f.add(&sums[0], &f.reduce(ring[0])),
            f.add(&sums[1], &f.reduce(ring[1])),
            f.zero(),
        ]
    };
    let combine = |a: [F; 3], b: [F; 3]| -> [F; 3] { std::array::from_fn(|k| f.add(&a[k], &b[k])) };
    #[cfg(feature = "parallel")]
    if values.len() / N >= 2 * MERGED_INSTANCES_PER_TASK && rayon::current_num_threads() > 1 {
        return values
            .par_chunks_mut(MERGED_INSTANCES_PER_TASK * N)
            .enumerate()
            .map(task)
            .reduce(|| [f.zero(); 3], combine);
    }
    values
        .chunks_mut(MERGED_INSTANCES_PER_TASK * N)
        .enumerate()
        .map(task)
        .fold([f.zero(); 3], combine)
}

/// Sums `[c0, c2, c3]` of one instance round before its challenge: the cubic
/// `Lambda(x) D(x) + e(x) D(x)^2` over pairs of live entries, with `e` the
/// folded norm weights (`eq` has one entry per live table entry).
fn instance_round_sums(values: &[[F; 2]], stride: usize, eq: &[F], f: &Cfg) -> [F; 3] {
    let live = values.len() / stride;
    debug_assert_eq!(live, eq.len());
    let mut sums = [f.zero(); 3];
    for m in 0..live / 2 {
        let [l0, d0] = values[2 * m * stride];
        let [l1, d1] = values[(2 * m + 1) * stride];
        let (e0, e1) = (eq[2 * m], eq[2 * m + 1]);
        let dl = f.sub(&l1, &l0);
        let dd = f.sub(&d1, &d0);
        let de = f.sub(&e1, &e0);
        let d0d0 = f.mul(&d0, &d0);
        let dddd = f.mul(&dd, &dd);
        let cross = f.mul(&d0, &dd);
        let cross2 = f.add(&cross, &cross);
        sums[0] = f.add(&sums[0], &f.add(&f.mul(&l0, &d0), &f.mul(&e0, &d0d0)));
        sums[1] = f.add(
            &sums[1],
            &f.add(
                &f.mul(&dl, &dd),
                &f.add(&f.mul(&e0, &dddd), &f.mul(&de, &cross2)),
            ),
        );
        sums[2] = f.add(&sums[2], &f.mul(&de, &dddd));
    }
    sums
}

fn fold_pairs(values: &mut [[F; 2]], stride: usize, r: F, f: &Cfg) {
    for pair in values.chunks_exact_mut(2 * stride) {
        pair[0] = std::array::from_fn(|k| lerp(pair[0][k], pair[stride][k], r, f));
    }
}

/// The word binder with the norm's square sums as a cubic term: coefficient
/// and side rounds are quadratic (the instance weight is constant within a
/// signature), instance rounds are cubic. Messages carry four coefficients
/// throughout; the verifier requires a zero cubic term in the quadratic rounds.
#[allow(clippy::too_many_arguments)]
pub(super) fn prove_merged(
    t: &mut impl Transcript,
    layout: &Layout,
    source: &Source,
    form: BindingForm<'_>,
    norm: MergedNorm,
    s1: Option<MergedS1>,
    target: F,
    norm_proof: NormProof,
    merge_nonce: Option<u64>,
    schedule: &Schedule,
    f: &Cfg,
    workspace: &mut WordWorkspace,
) -> Result<(Proof, Vec<u128>, WordBridgeClaim), FalconError> {
    let _span = tracing::info_span!("falcon_algebraic:merged_sumcheck").entered();
    if s1.is_some() != form.coefficients[0].is_empty() {
        return Err(failure("merged binder s1 weights must be dense or factored"));
    }
    t.absorb_slice(MERGED_DOMAIN);
    let options = WordOptions::configured(layout, f);
    let mut weights = PublicWeights::new(&form);
    let mut spare = Sparse::from_source(source, layout, f)?;
    let aggregate_spare = AggregateSpare::new(&spare, &weights, f);
    let (mut next, initial) = {
        let _span = tracing::info_span!("falcon_algebraic:word_initial_sums").entered();
        first_round_merged(&form, source, &norm, s1.as_ref(), f)
    };
    let spare_initial = aggregate_spare.initial(&weights, f);
    if f.add(&initial, &spare_initial) != target {
        return Err(failure("merged word binding initial claim"));
    }
    let mut aggregate_spare = Some(aggregate_spare);
    let mut form = Some(form);
    let mut local_workspace = WordWorkspace::default();
    let dense = if options.scratch {
        &mut workspace.dense
    } else {
        &mut local_workspace.dense
    };
    let mut stride = 1;
    let mut eq = norm.weights.clone();
    let mut count = None;
    let with_padding = layout.batch() < layout.capacity();
    let rounds = rounds(layout);
    let quadratic_rounds = COEFFICIENT_LOG + 1;
    let mut point = Vec::with_capacity(rounds);
    let mut proof = SumcheckProof {
        round_polynomials: Vec::with_capacity(rounds),
    };
    let mut boundary = SplitRoundBoundary {
        split: quadratic_rounds,
        first: ProverGrindingRoundBoundary::<BindingRound>::with_round_offset(
            schedule.binding_bits,
            0,
        ),
        second: ProverGrindingRoundBoundary::<BindingRound>::with_round_offset(
            schedule.cubic_binding_bits,
            quadratic_rounds,
        ),
    };
    let mut claim = target;
    for round in 0..rounds {
        if with_padding && round == COEFFICIENT_LOG + 1 {
            count = Some(counts(source, layout, &point, f)?);
        }
        if round >= quadratic_rounds {
            next = instance_round_sums(&dense[..], stride, &eq, f);
        }
        let spare_sums = aggregate_spare.as_ref().map_or_else(
            || spare.sums(&weights, round, f),
            |aggregate| aggregate.sums(&weights, round, f),
        );
        let mut c0 = f.add(&next[0], &spare_sums[0]);
        let mut c2 = f.add(&next[1], &spare_sums[1]);
        let c3 = next[2];
        if let Some(c) = &count {
            let sums = count_sums(c, &weights, f);
            c0 = f.add(&c0, &sums[0]);
            c2 = f.add(&c2, &sums[1]);
        }
        let c1 = f.sub(&f.sub(&f.sub(&claim, &f.add(&c0, &c0)), &c2), &c3);
        let polynomial = [c0, c1, c2, c3];
        absorb_field_elements(t, &polynomial, f);
        proof.round_polynomials.push(polynomial);
        {
            let _span =
                tracing::info_span!("falcon_algebraic:word_round_grinding", round).entered();
            boundary.after_round(t, round).map_err(failure)?;
        }
        let r = sample(t, f)?;
        claim = f.add(
            &c0,
            &f.mul(&r, &f.add(&c1, &f.mul(&r, &f.add(&c2, &f.mul(&r, &c3))))),
        );
        point.push(r);
        if round == 0 {
            let initial_form = form.take().unwrap();
            let ring = first_fold_merged(&initial_form, source, r, s1.as_ref(), f, options, dense);
            let squares = norm_sums_strided(&dense[..], 1, &norm, f);
            next = [
                f.add(&ring[0], &squares[0]),
                f.add(&ring[1], &squares[1]),
                f.zero(),
            ];
        } else if round < quadratic_rounds {
            next = dense_fold_merged(&mut dense[..], stride, r, &norm, f);
            stride *= 2;
        } else {
            fold_pairs(&mut dense[..], stride, r, f);
            stride *= 2;
            fold_small(&mut eq, r, f);
        }
        {
            let _span =
                tracing::info_span!("falcon_algebraic:word_auxiliary_fold", round).entered();
            if round < COEFFICIENT_LOG {
                aggregate_spare.as_mut().unwrap().fold(r, f);
            } else if round == COEFFICIENT_LOG {
                spare.project_instances(&point, f)?;
                aggregate_spare = None;
            } else {
                spare.fold(r, f);
            }
            if let Some(c) = &mut count {
                fold_small(c, r, f);
            }
            weights.fold(round, r, f);
        }
    }
    debug_assert_eq!(eq.len(), 1);
    let dense_terminal = dense[0];
    let mut values = vec![dense_terminal[1], spare.terminal(f)];
    let mut terminal = f.add(
        &f.mul(&dense_terminal[0], &values[0]),
        &f.mul(&weights.spare(0, rounds, f), &values[1]),
    );
    terminal = f.add(&terminal, &f.mul(&eq[0], &f.mul(&values[0], &values[0])));
    if let Some(c) = count {
        values.push(c[0]);
        terminal = f.add(&terminal, &f.mul(&weights.count(0, f), &c[0]));
    }
    if claim != terminal {
        return Err(failure("merged word binding terminal mismatch"));
    }
    t.absorb_slice(TERMINALS);
    absorb_field_elements(t, &values, f);
    let terminal_merge_nonce = {
        let _span = tracing::info_span!("falcon_algebraic:word_terminal_grinding").entered();
        prove_nonce::<TerminalMerge>(t, schedule.terminal_merge_bits)?
    };
    let zeta = sample(t, f)?;
    let claim = bind_terminal(t, point, &values, zeta, f);
    let row_weights = claim.row_weights(layout, f)?;
    let mut binding_nonces = boundary.first.into_nonces();
    binding_nonces.extend(boundary.second.into_nonces());
    Ok((
        Proof {
            norm: norm_proof,
            merge_nonce,
            binding: BindingProof::Cubic(proof),
            binding_terminal: values,
            binding_nonces,
            terminal_merge_nonce,
        },
        row_weights,
        claim,
    ))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn verify_merged(
    t: &mut impl Transcript,
    layout: &Layout,
    form: &BindingForm<'_>,
    keyed: Option<&KeyedRing<'_>>,
    norm: &MergedNorm,
    target: F,
    proof: &Proof,
    schedule: &Schedule,
    f: &Cfg,
) -> Result<WordBridgeClaim, FalconError> {
    let n = 2 + usize::from(layout.batch() < layout.capacity());
    if proof.binding_terminal.len() != n {
        return Err(failure("word binding terminal count"));
    }
    validate_field_elements(&proof.binding_terminal, f).map_err(failure)?;
    let sumcheck = proof.binding.cubic()?;
    let rounds = rounds(layout);
    let quadratic_rounds = COEFFICIENT_LOG + 1;
    if sumcheck.round_polynomials.len() != rounds {
        return Err(failure("merged word binding round count"));
    }
    // Coefficient and side rounds are quadratic; a cubic message there would
    // exceed the error those rounds are budgeted for.
    if sumcheck.round_polynomials[..quadratic_rounds.min(rounds)]
        .iter()
        .any(|message| message[3] != f.zero())
    {
        return Err(failure("cubic message in a quadratic binder round"));
    }
    t.absorb_slice(MERGED_DOMAIN);
    let split_at = if schedule.binding_bits == 0 {
        0
    } else {
        quadratic_rounds.min(proof.binding_nonces.len())
    };
    let (first_nonces, second_nonces) = proof.binding_nonces.split_at(split_at);
    let mut boundary = SplitRoundBoundary {
        split: quadratic_rounds,
        first: VerifierGrindingRoundBoundary::<BindingRound>::new(schedule.binding_bits, first_nonces),
        second: VerifierGrindingRoundBoundary::<BindingRound>::with_round_offset(
            schedule.cubic_binding_bits,
            second_nonces,
            quadratic_rounds,
        ),
    };
    let (point, claim) = sumcheck
        .verify_with_round_boundary(t, target, rounds, f, &mut boundary)
        .map_err(failure)?;
    let coefficient_value =
        terminal_coefficient_value(layout, form, &WordFactors::default(), keyed, &point, f)?;
    let norm_weight = norm.terminal_weight(&point[quadratic_rounds.min(rounds)..], f);
    let mut weights = PublicWeights::new(form);
    for (round, &r) in point.iter().enumerate() {
        weights.fold(round, r, f);
    }
    let v = &proof.binding_terminal;
    let mut terminal = f.add(
        &f.mul(&coefficient_value, &v[0]),
        &f.mul(&weights.spare(0, rounds, f), &v[1]),
    );
    terminal = f.add(&terminal, &f.mul(&norm_weight, &f.mul(&v[0], &v[0])));
    if n == 3 {
        terminal = f.add(&terminal, &f.mul(&weights.count(0, f), &v[2]));
    }
    if claim != terminal {
        return Err(failure("merged word binding terminal mismatch"));
    }
    t.absorb_slice(TERMINALS);
    absorb_field_elements(t, v, f);
    verify_nonce::<TerminalMerge>(t, schedule.terminal_merge_bits, proof.terminal_merge_nonce)?;
    let zeta = sample(t, f)?;
    Ok(bind_terminal(t, point, v, zeta, f))
}

#[cfg(test)]
pub(super) fn prove(
    t: &mut impl Transcript,
    layout: &Layout,
    source: &Source,
    form: BindingForm<'_>,
    target: F,
    norm: NormProof,
    merge_nonce: Option<u64>,
    schedule: &Schedule,
    f: &Cfg,
) -> Result<(Proof, Vec<u128>, WordBridgeClaim), FalconError> {
    prove_with_correction(
        t,
        layout,
        source,
        form,
        WordFactors::default(),
        target,
        norm,
        merge_nonce,
        schedule,
        f,
    )
}

pub(super) fn prove_with_correction(
    t: &mut impl Transcript,
    layout: &Layout,
    source: &Source,
    form: BindingForm<'_>,
    factors: WordFactors,
    target: F,
    norm: NormProof,
    merge_nonce: Option<u64>,
    schedule: &Schedule,
    f: &Cfg,
) -> Result<(Proof, Vec<u128>, WordBridgeClaim), FalconError> {
    prove_with_workspace(
        t,
        layout,
        source,
        form,
        factors,
        target,
        norm,
        merge_nonce,
        schedule,
        f,
        &mut WordWorkspace::default(),
    )
}

pub(super) fn prove_with_workspace(
    t: &mut impl Transcript,
    layout: &Layout,
    source: &Source,
    form: BindingForm<'_>,
    factors: WordFactors,
    target: F,
    norm: NormProof,
    merge_nonce: Option<u64>,
    schedule: &Schedule,
    f: &Cfg,
    workspace: &mut WordWorkspace,
) -> Result<(Proof, Vec<u128>, WordBridgeClaim), FalconError> {
    prove_with_initial(
        t,
        layout,
        source,
        form,
        factors,
        target,
        norm,
        merge_nonce,
        schedule,
        f,
        workspace,
        None,
    )
}

pub(super) fn prove_with_initial(
    t: &mut impl Transcript,
    layout: &Layout,
    source: &Source,
    form: BindingForm<'_>,
    factors: WordFactors,
    target: F,
    norm: NormProof,
    merge_nonce: Option<u64>,
    schedule: &Schedule,
    f: &Cfg,
    workspace: &mut WordWorkspace,
    initial: Option<InitialClaim>,
) -> Result<(Proof, Vec<u128>, WordBridgeClaim), FalconError> {
    prove_with_options_and_initial(
        t,
        layout,
        source,
        form,
        factors,
        target,
        norm,
        merge_nonce,
        schedule,
        f,
        workspace,
        WordOptions::configured(layout, f),
        initial,
    )
}

#[cfg(test)]
fn prove_with_options(
    t: &mut impl Transcript,
    layout: &Layout,
    source: &Source,
    form: BindingForm<'_>,
    factors: WordFactors,
    target: F,
    norm: NormProof,
    merge_nonce: Option<u64>,
    schedule: &Schedule,
    f: &Cfg,
    workspace: &mut WordWorkspace,
    options: WordOptions,
) -> Result<(Proof, Vec<u128>, WordBridgeClaim), FalconError> {
    prove_with_options_and_initial(
        t,
        layout,
        source,
        form,
        factors,
        target,
        norm,
        merge_nonce,
        schedule,
        f,
        workspace,
        options,
        None,
    )
}

fn prove_with_options_and_initial(
    t: &mut impl Transcript,
    layout: &Layout,
    source: &Source,
    form: BindingForm<'_>,
    factors: WordFactors,
    target: F,
    norm: NormProof,
    merge_nonce: Option<u64>,
    schedule: &Schedule,
    f: &Cfg,
    workspace: &mut WordWorkspace,
    options: WordOptions,
    initial: Option<InitialClaim>,
) -> Result<(Proof, Vec<u128>, WordBridgeClaim), FalconError> {
    t.absorb_slice(DOMAIN);
    let mut weights = PublicWeights::new(&form);
    let mut spare = Sparse::from_source(source, layout, f)?;
    let aggregate_spare = AggregateSpare::new(&spare, &weights, f);
    let (mut next, initial) = {
        let _span = tracing::info_span!("falcon_algebraic:word_initial_sums").entered();
        if let Some(initial) = initial {
            if factors.s1.is_some() || factors.norm.is_some() {
                return Err(failure(
                    "precomputed initial claim requires dense coefficients",
                ));
            }
            (initial.round, initial.sum)
        } else if options.factored_initial_fused {
            if let (Some(s1), Some(norm)) = (&factors.s1, &factors.norm) {
                first_round_factored_fused(&form, source, s1, norm, f, options.reuse_initial)
            } else {
                first_round_with_factors(&form, source, &factors, f, options.reuse_initial)
            }
        } else {
            first_round_with_factors(&form, source, &factors, f, options.reuse_initial)
        }
    };
    let spare_initial = aggregate_spare.initial(&weights, f);
    if f.add(&initial, &spare_initial) != target {
        return Err(failure("word binding initial claim"));
    }
    let mut aggregate_spare = Some(aggregate_spare);
    let mut form = Some(form);
    let mut local_workspace = WordWorkspace::default();
    let dense = if options.scratch {
        &mut workspace.dense
    } else {
        &mut local_workspace.dense
    };
    let mut stride = 1;
    let mut compact_output = Vec::new();
    let mut count = None;
    let with_padding = layout.batch() < layout.capacity();
    let mut point = Vec::with_capacity(rounds(layout));
    let mut proof = SumcheckProof {
        round_polynomials: Vec::with_capacity(rounds(layout)),
    };
    let mut boundary =
        ProverGrindingRoundBoundary::<BindingRound>::with_round_offset(schedule.binding_bits, 0);
    let mut claim = target;
    for round in 0..rounds(layout) {
        if with_padding && round == COEFFICIENT_LOG + 1 {
            count = Some(counts(source, layout, &point, f)?);
        }
        let spare_sums = aggregate_spare.as_ref().map_or_else(
            || spare.sums(&weights, round, f),
            |aggregate| aggregate.sums(&weights, round, f),
        );
        let mut sums = add_pair(next, spare_sums, f);
        if let Some(c) = &count {
            sums = add_pair(sums, count_sums(c, &weights, f), f);
        }
        let [c0, c2] = sums;
        let c1 = f.sub(&f.sub(&claim, &f.add(&c0, &c0)), &c2);
        let polynomial = [c0, c1, c2];
        absorb_field_elements(t, &polynomial, f);
        proof.round_polynomials.push(polynomial);
        {
            let _span =
                tracing::info_span!("falcon_algebraic:word_round_grinding", round).entered();
            boundary.after_round(t, round).map_err(failure)?;
        }
        let r = sample(t, f)?;
        claim = f.add(&c0, &f.mul(&r, &f.add(&c1, &f.mul(&r, &c2))));
        point.push(r);
        if round == 0 {
            let initial_form = form.take().unwrap();
            next = first_fold_into(&initial_form, source, r, &factors, f, options, dense);
        } else if options.compact_dense_output {
            next = dense_fold_compact_output(dense, &mut compact_output, &mut stride, r, f);
        } else {
            next = dense_fold_with_compaction(dense, &mut stride, r, f, options.dense_compaction);
        }
        {
            let _span =
                tracing::info_span!("falcon_algebraic:word_auxiliary_fold", round).entered();
            if round < COEFFICIENT_LOG {
                aggregate_spare.as_mut().unwrap().fold(r, f);
            } else if round == COEFFICIENT_LOG {
                spare.project_instances(&point, f)?;
                aggregate_spare = None;
            } else {
                spare.fold(r, f);
            }
            if let Some(c) = &mut count {
                fold_small(c, r, f);
            }
            weights.fold(round, r, f);
        }
    }
    let dense_terminal = dense[0];
    // A lease retains only the original (largest) allocation. Its contents are
    // overwritten by the next first fold; the terminal was copied above.
    if compact_output.capacity() > dense.capacity() {
        std::mem::swap(dense, &mut compact_output);
    }
    let mut values = vec![dense_terminal[1], spare.terminal(f)];
    let mut terminal = f.add(
        &f.mul(&dense_terminal[0], &values[0]),
        &f.mul(&weights.spare(0, rounds(layout), f), &values[1]),
    );
    if let Some(c) = count {
        values.push(c[0]);
        terminal = f.add(&terminal, &f.mul(&weights.count(0, f), &c[0]));
    }
    if claim != terminal {
        return Err(failure("word binding terminal mismatch"));
    }
    t.absorb_slice(TERMINALS);
    absorb_field_elements(t, &values, f);
    let terminal_merge_nonce = {
        let _span = tracing::info_span!("falcon_algebraic:word_terminal_grinding").entered();
        prove_nonce::<TerminalMerge>(t, schedule.terminal_merge_bits)?
    };
    let zeta = sample(t, f)?;
    let claim = bind_terminal(t, point, &values, zeta, f);
    let row_weights = claim.row_weights(layout, f)?;
    Ok((
        Proof {
            norm,
            merge_nonce,
            binding: BindingProof::Quadratic(proof),
            binding_terminal: values,
            binding_nonces: boundary.into_nonces(),
            terminal_merge_nonce,
        },
        row_weights,
        claim,
    ))
}

pub(super) fn verify(
    t: &mut impl Transcript,
    layout: &Layout,
    form: &BindingForm<'_>,
    target: F,
    proof: &Proof,
    schedule: &Schedule,
    f: &Cfg,
) -> Result<WordBridgeClaim, FalconError> {
    verify_with_factors(
        t,
        layout,
        form,
        &WordFactors::default(),
        target,
        proof,
        schedule,
        f,
    )
}

pub(super) fn verify_with_factors(
    t: &mut impl Transcript,
    layout: &Layout,
    form: &BindingForm<'_>,
    factors: &WordFactors,
    target: F,
    proof: &Proof,
    schedule: &Schedule,
    f: &Cfg,
) -> Result<WordBridgeClaim, FalconError> {
    verify_with_ring(t, layout, form, factors, None, target, proof, schedule, f)
}

/// Keyed ring weights in place of dense ring coefficients: per point, the
/// scaled instance factors, evaluated against the public keys at the terminal.
#[cfg(any(test, feature = "bench-internals"))]
pub(super) struct KeyedRing<'a> {
    pub points: &'a [F],
    pub rows: &'a [Vec<F>],
    pub keys: &'a [[u16; N]],
}

#[cfg(not(any(test, feature = "bench-internals")))]
pub(super) struct KeyedRing<'a> {
    _keys: std::marker::PhantomData<&'a ()>,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn verify_with_ring(
    t: &mut impl Transcript,
    layout: &Layout,
    form: &BindingForm<'_>,
    factors: &WordFactors,
    keyed: Option<&KeyedRing<'_>>,
    target: F,
    proof: &Proof,
    schedule: &Schedule,
    f: &Cfg,
) -> Result<WordBridgeClaim, FalconError> {
    let n = 2 + usize::from(layout.batch() < layout.capacity());
    if proof.binding_terminal.len() != n {
        return Err(failure("word binding terminal count"));
    }
    validate_field_elements(&proof.binding_terminal, f).map_err(failure)?;
    t.absorb_slice(DOMAIN);
    let mut boundary = VerifierGrindingRoundBoundary::<BindingRound>::new(
        schedule.binding_bits,
        &proof.binding_nonces,
    );
    let (point, claim) = proof
        .binding
        .quadratic()?
        .verify_with_round_boundary(t, target, rounds(layout), f, &mut boundary)
        .map_err(failure)?;
    let coefficient_value = terminal_coefficient_value(layout, form, factors, keyed, &point, f)?;
    let mut weights = PublicWeights::new(form);
    for (round, &r) in point.iter().enumerate() {
        weights.fold(round, r, f);
    }
    let v = &proof.binding_terminal;
    let mut terminal = f.add(
        &f.mul(&coefficient_value, &v[0]),
        &f.mul(&weights.spare(0, rounds(layout), f), &v[1]),
    );
    if n == 3 {
        terminal = f.add(&terminal, &f.mul(&weights.count(0, f), &v[2]));
    }
    if claim != terminal {
        return Err(failure("word binding terminal mismatch"));
    }
    t.absorb_slice(TERMINALS);
    absorb_field_elements(t, v, f);
    verify_nonce::<TerminalMerge>(t, schedule.terminal_merge_bits, proof.terminal_merge_nonce)?;
    let zeta = sample(t, f)?;
    Ok(bind_terminal(t, point, v, zeta, f))
}
