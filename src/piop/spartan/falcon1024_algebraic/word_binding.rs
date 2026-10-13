//! Word sumcheck with one dense product and sparse/factored auxiliary terms.
use super::*;
use crate::piop::spartan::SpartanField;
use crate::sumcheck::boundary::RoundBoundaryPolicy;
use field::{FpProductAcc, PreparedLinearCombination};

#[cfg(test)]
#[path = "word_binding_tests.rs"]
mod tests;

const DOMAIN: &[u8] = b"bitz/falcon-algebraic/word-binding/v1";
const TERMINALS: &[u8] = b"bitz/falcon-algebraic/word-terminals/v1";
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

#[derive(Clone, Copy, Debug, Default)]
struct WordOptions {
    reuse_initial: bool,
    fused_fold: bool,
    direct_init: bool,
    scratch: bool,
    factored_initial_fused: bool,
    factored_fold_reduction: bool,
    dense_compaction: Option<usize>,
}

impl WordOptions {
    fn configured() -> Self {
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
            dense_compaction: dense_compaction_threshold(),
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
    // The fused initializer writes pairs; reject an odd tail before exposing
    // any initialized slice or changing the Vec length.
    assert_eq!(len % 2, 0);
    let parts: Vec<[F; 2]> = if direct {
        // Every output is overwritten. Keep length zero until all worker writes
        // finish so unwinding can never expose an uninitialized field element.
        values.clear();
        if values.capacity() < len {
            values.reserve_exact(len);
        }
        let spare = &mut values.spare_capacity_mut()[..len];
        let parts = crate::cfg_chunks_mut!(spare, 1024)
            .enumerate()
            .map(|(chunk, out)| {
                if fused {
                    let mut sums = [FpProductAcc::<2>::default(); 2];
                    for (j, pair) in out.chunks_exact_mut(2).enumerate() {
                        let a = slot(chunk * 1024 + 2 * j);
                        let b = slot(chunk * 1024 + 2 * j + 1);
                        pair[0].write(a);
                        pair[1].write(b);
                        f.mul_acc(&mut sums[0], &a[0], &a[1]);
                        f.mul_acc(&mut sums[1], &f.sub(&b[0], &a[0]), &f.sub(&b[1], &a[1]));
                    }
                    sums.map(|s| f.reduce(s))
                } else {
                    for (j, out) in out.iter_mut().enumerate() {
                        out.write(slot(chunk * 1024 + j));
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
        // size 1024 are even, so the fused pair loop has no unwritten tail.
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
        crate::cfg_chunks_mut!(values, 1024)
            .enumerate()
            .map(|(chunk, out)| {
                if fused {
                    let mut sums = [FpProductAcc::<2>::default(); 2];
                    for (j, pair) in out.chunks_exact_mut(2).enumerate() {
                        let a = slot(chunk * 1024 + 2 * j);
                        let b = slot(chunk * 1024 + 2 * j + 1);
                        pair[0] = a;
                        pair[1] = b;
                        f.mul_acc(&mut sums[0], &a[0], &a[1]);
                        f.mul_acc(&mut sums[1], &f.sub(&b[0], &a[0]), &f.sub(&b[1], &a[1]));
                    }
                    sums.map(|s| f.reduce(s))
                } else {
                    for (j, out) in out.iter_mut().enumerate() {
                        *out = slot(chunk * 1024 + j);
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
        WordOptions::configured(),
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
    let mut values = vec![dense[0][1], spare.terminal(f)];
    let mut terminal = f.add(
        &f.mul(&dense[0][0], &values[0]),
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
            binding: proof,
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
        .verify_with_round_boundary(t, target, rounds(layout), f, &mut boundary)
        .map_err(failure)?;
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
    let evaluate_instance = |instance: usize| {
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
    #[cfg(feature = "parallel")]
    let values: Option<Vec<_>> =
        if binding_parallel(binding_strategy(layout.batch(), BindingKernel::Evaluation)) {
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
