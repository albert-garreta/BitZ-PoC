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
fn first_round(form: &BindingForm<'_>, source: &Source, f: &Cfg) -> ([F; 2], F) {
    let chunks = (form.layout.source_bits() / 32).div_ceil(1024);
    let parts: Vec<_> = crate::cfg_into_iter!(0..chunks)
        .map(|chunk| {
            let mut sums = [[FpLinearAcc::<2, 1>::default(); 2]; 3];
            let end = ((chunk + 1) * 1024).min(form.layout.source_bits() / 32);
            for i in chunk * 1024..end {
                let a = decode(word(source, 2 * i));
                let b = decode(word(source, 2 * i + 1));
                let w0 = coefficient(form, 2 * i);
                let w1 = coefficient(form, 2 * i + 1);
                signed_acc(&mut sums[0], &w0, a, f);
                signed_acc(&mut sums[1], &f.sub(&w1, &w0), b - a, f);
                signed_acc(&mut sums[2], &w0, a, f);
                signed_acc(&mut sums[2], &w1, b, f);
            }
            sums.map(|s| reduce_signed(s, f))
        })
        .collect();
    let sum = parts.into_iter().fold([f.zero(); 3], |a, b| {
        std::array::from_fn(|i| f.add(&a[i], &b[i]))
    });
    ([sum[0], sum[1]], sum[2])
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
#[tracing::instrument(skip_all, name = "falcon_algebraic:word_first_fold")]
fn first_fold(form: &BindingForm<'_>, source: &Source, r: F, f: &Cfg) -> (Vec<[F; 2]>, [F; 2]) {
    // Flipping bit 14 maps signed15 a to a+2^14. The interpolation weights
    // sum to one, so the third term removes this bias in the wide accumulator.
    // Prepared weights give one Montgomery reduction for the complete fold.
    let prepared = <Cfg as PreparedLinearCombination<u64>>::prepare_linear_combination(
        f,
        [f.sub(&f.one(), &r), r, f.neg(&small(1 << 14, f))],
    );
    let mut values = vec![[f.zero(); 2]; form.layout.source_bits() / 32];
    let parts: Vec<_> = crate::cfg_chunks_mut!(values, 1024)
        .enumerate()
        .map(|(chunk, out)| {
            for (j, slot) in out.iter_mut().enumerate() {
                let i = 2 * (chunk * 1024 + j);
                let words = [word(source, i), word(source, i + 1)];
                *slot = [
                    lerp(coefficient(form, i), coefficient(form, i + 1), r, f),
                    <Cfg as PreparedLinearCombination<u64>>::linear_combination(&prepared, |k| {
                        if k == 2 {
                            1
                        } else {
                            u64::from((words[k] ^ 0x4000) & 0x7fff)
                        }
                    }),
                ];
            }
            dense_sums(out, 1, f)
        })
        .collect();
    let sums = parts
        .into_iter()
        .fold([f.zero(); 2], |a, b| add_pair(a, b, f));
    (values, sums)
}

fn dense_fold(values: &mut [[F; 2]], stride: usize, r: F, f: &Cfg) -> [F; 2] {
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
    let _span = tracing::info_span!("falcon_algebraic:word_dense_fold").entered();
    #[cfg(feature = "parallel")]
    if values.len() / stride >= 4096 && rayon::current_num_threads() > 1 {
        return values
            .par_chunks_mut(1024 * stride)
            .map(fold)
            .reduce(|| [f.zero(); 2], |a, b| add_pair(a, b, f));
    }
    fold(values)
}

/// Padding is checked while scanning the source, so C contributes zero until
/// the instance rounds. Evaluate its local MLE once instead of folding a
/// third full-size field table. The verifier never assumes padding is zero.
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
    t.absorb_slice(DOMAIN);
    let mut weights = PublicWeights::new(&form);
    let mut spare = Sparse::from_source(source, layout, f)?;
    let aggregate_spare = AggregateSpare::new(&spare, &weights, f);
    let (mut next, initial) = first_round(&form, source, f);
    let spare_initial = aggregate_spare.initial(&weights, f);
    if f.add(&initial, &spare_initial) != target {
        return Err(failure("word binding initial claim"));
    }
    let mut aggregate_spare = Some(aggregate_spare);
    let mut form = Some(form);
    let mut dense = Vec::new();
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
        boundary.after_round(t, round).map_err(failure)?;
        let r = sample(t, f)?;
        claim = f.add(&c0, &f.mul(&r, &f.add(&c1, &f.mul(&r, &c2))));
        point.push(r);
        if round == 0 {
            let initial_form = form.take().unwrap();
            (dense, next) = first_fold(&initial_form, source, r, f);
        } else {
            next = dense_fold(&mut dense, stride, r, f);
            stride *= 2;
        }
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
    let terminal_merge_nonce = prove_nonce::<TerminalMerge>(t, schedule.terminal_merge_bits)?;
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
    let evaluate_instance = |instance: usize| {
        let mut sum = FpProductAcc::<2>::default();
        for side in 0..2 {
            for (coefficient, weight) in form.coefficients[side][instance * N..(instance + 1) * N]
                .iter()
                .zip(&local[side * N..(side + 1) * N])
            {
                f.mul_acc(&mut sum, coefficient, weight);
            }
        }
        reduction.reduce(sum)
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
    let mut weights = PublicWeights::new(form);
    for (round, &r) in point.iter().enumerate() {
        weights.fold(round, r, f);
    }
    let v = &proof.binding_terminal;
    let mut terminal = f.add(
        &f.mul(&f.reduce(coefficient_sum), &v[0]),
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
