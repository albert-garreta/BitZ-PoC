use crate::{
    piop::{
        lookup::gkr_product::absorb_field_slice,
        sumcheck::{
            SumcheckProof,
            prover::{NatEvaluatedPolyWithoutConstant, ProverMsg},
        },
    },
    poly::{coefficient::PolynomialField, univariate::binary_gf128::Gf128 as Gf},
    transcript::traits::Transcript,
    utils::wide_mul::WideMulAcc,
};
use field::SumcheckKernels;

#[cfg(feature = "parallel")]
use rayon::prelude::*;

const PARALLEL_THRESHOLD: usize = 1 << 15;

#[derive(Clone, Copy)]
pub(super) enum ProductInput<'a> {
    Table(&'a [Gf]),
    AffineInterleaved {
        numerators: &'a [Gf],
        denominators: &'a [Gf],
        gamma: Gf,
        parity: usize,
        len: usize,
        padding: Gf,
    },
}

impl<'a> ProductInput<'a> {
    fn len(self) -> usize {
        match self {
            Self::Table(values) => values.len(),
            Self::AffineInterleaved { len, .. } => len,
        }
    }

    fn is_empty(self) -> bool {
        self.len() == 0
    }

    fn padding(self) -> Gf {
        match self {
            Self::Table(_) => Gf::ZERO,
            Self::AffineInterleaved { padding, .. } => padding,
        }
    }

    #[inline(always)]
    fn value(self, index: usize) -> Gf {
        match self {
            Self::Table(values) => values.get(index).copied().unwrap_or(Gf::ZERO),
            Self::AffineInterleaved {
                numerators,
                denominators,
                gamma,
                parity,
                len,
                padding,
            } => {
                if index >= len {
                    return padding;
                }
                let source = 2 * index + parity;
                numerators
                    .get(source)
                    .zip(denominators.get(source))
                    .map_or(padding, |(&num, &den)| den + gamma * num)
            }
        }
    }
}

#[derive(Clone, Copy)]
enum StateView<'a> {
    Dense { values: &'a [Gf], padding: Gf },
    Input(ProductInput<'a>),
}

impl StateView<'_> {
    fn len(self) -> usize {
        match self {
            Self::Dense { values, .. } => values.len(),
            Self::Input(input) => input.len(),
        }
    }

    fn padding(self) -> Gf {
        match self {
            Self::Dense { padding, .. } => padding,
            Self::Input(input) => input.padding(),
        }
    }

    #[inline(always)]
    fn value(self, index: usize) -> Gf {
        match self {
            Self::Dense { values, padding } => values.get(index).copied().unwrap_or(padding),
            Self::Input(input) => input.value(index),
        }
    }
}

fn initial_view(input: ProductInput<'_>) -> StateView<'_> {
    match input {
        ProductInput::Table(values) => StateView::Dense {
            values,
            padding: Gf::ZERO,
        },
        input @ ProductInput::AffineInterleaved { .. } => StateView::Input(input),
    }
}

#[derive(Default)]
pub struct ProductWorkspace {
    tables: Vec<Vec<Gf>>,
    backs: Vec<Vec<Gf>>,
    paddings: Vec<Gf>,
    eq_low: Vec<Gf>,
    eq_high: Vec<Gf>,
    scales: Vec<Gf>,
    normalized_claims: Vec<Gf>,
    quadratics: Vec<[Gf; 3]>,
}

impl ProductWorkspace {
    pub fn reserve(
        &mut self,
        max_claims: usize,
        max_table_len: usize,
        max_dimension: usize,
    ) {
        let table_slots = 2 * max_claims;
        while self.tables.len() < table_slots {
            self.tables.push(Vec::new());
            self.backs.push(Vec::new());
        }
        let folded_len = max_table_len.div_ceil(2).max(1);
        for table in self.tables[..table_slots]
            .iter_mut()
            .chain(&mut self.backs[..table_slots])
        {
            reserve_vec(table, folded_len);
        }
        let (eq_low, eq_high) = split_eq_capacities(max_dimension, max_table_len.div_ceil(2));
        reserve_vec(&mut self.eq_low, eq_low);
        reserve_vec(&mut self.eq_high, eq_high);
        reserve_vec(&mut self.paddings, table_slots);
        reserve_vec(&mut self.scales, max_claims);
        reserve_vec(&mut self.normalized_claims, max_claims);
        reserve_vec(&mut self.quadratics, max_claims);
    }

    pub fn retained_bytes(&self) -> usize {
        let fields = self.tables.iter().map(Vec::capacity).sum::<usize>()
            + self.backs.iter().map(Vec::capacity).sum::<usize>()
            + self.paddings.capacity()
            + self.eq_low.capacity()
            + self.eq_high.capacity()
            + self.scales.capacity()
            + self.normalized_claims.capacity()
            + 3 * self.quadratics.capacity();
        fields * core::mem::size_of::<Gf>()
    }
}

pub(super) fn prove_products(
    transcript: &mut impl Transcript,
    claims: &[(Vec<Gf>, Gf)],
    inputs: &[(ProductInput<'_>, ProductInput<'_>)],
    workspace: &mut ProductWorkspace,
) -> (SumcheckProof<Gf>, Vec<Gf>, Vec<(Gf, Gf)>) {
    prove_products_impl(transcript, claims, inputs, workspace, None)
}

pub(super) fn prove_products_materialized(
    transcript: &mut impl Transcript,
    claims: &[(Vec<Gf>, Gf)],
    inputs: &[(ProductInput<'_>, ProductInput<'_>)],
    workspace: &mut ProductWorkspace,
    materialized: (&mut [Gf], &mut [Gf]),
) -> (SumcheckProof<Gf>, Vec<Gf>, Vec<(Gf, Gf)>) {
    assert_eq!(claims.len(), 1);
    assert_eq!(inputs.len(), 1);
    assert_eq!(materialized.0.len(), inputs[0].0.len());
    assert_eq!(materialized.1.len(), inputs[0].1.len());
    prove_products_impl(transcript, claims, inputs, workspace, Some(materialized))
}

fn prove_products_impl(
    transcript: &mut impl Transcript,
    claims: &[(Vec<Gf>, Gf)],
    inputs: &[(ProductInput<'_>, ProductInput<'_>)],
    workspace: &mut ProductWorkspace,
    mut materialized: Option<(&mut [Gf], &mut [Gf])>,
) -> (SumcheckProof<Gf>, Vec<Gf>, Vec<(Gf, Gf)>) {
    assert!(!claims.is_empty() && claims.len() == inputs.len());
    let dimension = claims[0].0.len();
    let full_len = 1usize << dimension;
    assert!(claims.iter().all(|claim| claim.0 == claims[0].0));
    assert!(inputs.iter().all(|&(left, right)| {
        !left.is_empty()
            && left.len() <= full_len
            && !right.is_empty()
            && right.len() <= full_len
    }));

    let beta = (claims.len() > 1).then(|| transcript.get_field_challenge::<Gf>(&()));
    workspace.scales.clear();
    workspace.normalized_claims.clear();
    let mut scale = Gf::ONE;
    let mut claimed_sum = Gf::ZERO;
    for claim in claims {
        workspace.scales.push(scale);
        workspace.normalized_claims.push(claim.1);
        claimed_sum += scale * claim.1;
        if let Some(beta) = beta {
            scale *= beta;
        }
    }

    if dimension == 0 {
        let evals = inputs
            .iter()
            .map(|&(left, right)| (left.value(0), right.value(0)))
            .collect::<Vec<_>>();
        debug_assert_eq!(
            claimed_sum,
            evals
                .iter()
                .zip(&workspace.scales)
                .map(|(&(left, right), &scale)| scale * left * right)
                .sum()
        );
        absorb_evals(transcript, &evals);
        return (
            SumcheckProof {
                messages: Vec::new(),
                claimed_sum,
            },
            Vec::new(),
            evals,
        );
    }

    let table_count = 2 * inputs.len();
    assert!(workspace.tables.len() >= table_count && workspace.backs.len() >= table_count);
    workspace.paddings.clear();
    for &(left, right) in inputs {
        for input in [left, right] {
            let folded_len = input.len().div_ceil(2).max(1);
            let slot = workspace.paddings.len();
            assert!(workspace.tables[slot].capacity() >= folded_len);
            assert!(workspace.backs[slot].capacity() >= folded_len);
            workspace.paddings.push(input.padding());
        }
    }
    assert!(workspace.scales.capacity() >= claims.len());
    assert!(workspace.normalized_claims.capacity() >= claims.len());
    assert!(workspace.quadratics.capacity() >= claims.len());

    let q = &claims[0].0;
    let mut transcript_buf = [0u8; 16];
    transcript.absorb_random_field(
        &Gf::interpolation_node(dimension as u64, &()),
        &mut transcript_buf,
    );
    transcript.absorb_random_field(&Gf::interpolation_node(3, &()), &mut transcript_buf);

    let mut messages = Vec::with_capacity(dimension);
    let mut point = Vec::with_capacity(dimension);
    let mut running_claim = claimed_sum;
    let mut prefix = Gf::ONE;
    let mut domain_len = full_len;
    let mut prefetched_round = None;

    for round in 0..dimension {
        let initial = round == 0;
        let s = q[round];
        workspace.quadratics.resize(claims.len(), [Gf::ZERO; 3]);
        if let Some(quadratic) = prefetched_round.take() {
            debug_assert_eq!(claims.len(), 1);
            workspace.quadratics[0] = quadratic;
        } else {
            let max_pairs = (0..claims.len())
                .map(|claim| {
                    let left = input_view(claim, initial, inputs, workspace);
                    let right = input_view(claim + claims.len(), initial, inputs, workspace);
                    active_len(left).max(active_len(right)).div_ceil(2)
                })
                .max()
                .unwrap();
            let low_domain = prepare_eq_split(
                &q[round + 1..],
                max_pairs,
                &mut workspace.eq_low,
                &mut workspace.eq_high,
            );
            let at_one = s != Gf::ONE;
            let linear0_inv = if at_one {
                (Gf::ONE + s).inverse_or_zero()
            } else {
                Gf::ZERO
            };
            let total_work = (0..claims.len())
                .map(|claim| {
                    active_len(input_view(claim, initial, inputs, workspace))
                        .max(active_len(input_view(
                            claim + claims.len(),
                            initial,
                            inputs,
                            workspace,
                        )))
                        .div_ceil(2)
                })
                .sum::<usize>();

            let tables = &workspace.tables;
            let paddings = &workspace.paddings;
            let normalized = &workspace.normalized_claims;
            let eq_low = &workspace.eq_low;
            let eq_high = &workspace.eq_high;
            let calculate = |claim: usize, quadratic: &mut [Gf; 3]| {
                let left = if initial {
                    initial_view(inputs[claim].0)
                } else {
                    StateView::Dense {
                        values: &tables[2 * claim],
                        padding: paddings[2 * claim],
                    }
                };
                let right = if initial {
                    initial_view(inputs[claim].1)
                } else {
                    StateView::Dense {
                        values: &tables[2 * claim + 1],
                        padding: paddings[2 * claim + 1],
                    }
                };
                let parallel_kernel = claims.len() == 1;
                let [finite, infinity] = product_round(
                    left,
                    right,
                    eq_low,
                    eq_high,
                    low_domain,
                    domain_len,
                    at_one,
                    parallel_kernel,
                );
                let (h0, h1) = if at_one {
                    let h1 = finite;
                    let h0 = (normalized[claim] + s * h1) * linear0_inv;
                    (h0, h1)
                } else {
                    (finite, normalized[claim])
                };
                *quadratic = [h0, h0 + h1 + infinity, infinity];
            };

            if initial && materialized.is_some() {
                let (left_out, right_out) = materialized.as_mut().unwrap();
                let [finite, infinity] = product_round_materialize(
                    initial_view(inputs[0].0),
                    initial_view(inputs[0].1),
                    left_out,
                    right_out,
                    eq_low,
                    eq_high,
                    low_domain,
                    at_one,
                );
                let (h0, h1) = if at_one {
                    let h1 = finite;
                    let h0 = (normalized[0] + s * h1) * linear0_inv;
                    (h0, h1)
                } else {
                    (finite, normalized[0])
                };
                workspace.quadratics[0] = [h0, h0 + h1 + infinity, infinity];
            } else {
                #[cfg(feature = "parallel")]
                if claims.len() > 1 && total_work >= PARALLEL_THRESHOLD {
                    workspace.quadratics[..claims.len()]
                        .par_iter_mut()
                        .enumerate()
                        .for_each(|(claim, quadratic)| calculate(claim, quadratic));
                } else {
                    workspace.quadratics[..claims.len()]
                        .iter_mut()
                        .enumerate()
                        .for_each(|(claim, quadratic)| calculate(claim, quadratic));
                }
                #[cfg(not(feature = "parallel"))]
                workspace.quadratics[..claims.len()]
                    .iter_mut()
                    .enumerate()
                    .for_each(|(claim, quadratic)| calculate(claim, quadratic));
            }
        }

        let mut cofactor = [Gf::ZERO; 3];
        for ((quadratic, &scale), _) in workspace.quadratics[..claims.len()]
            .iter()
            .zip(&workspace.scales)
            .zip(claims)
        {
            let weight = prefix * scale;
            for i in 0..3 {
                cofactor[i] += weight * quadratic[i];
            }
        }
        debug_assert_eq!(
            running_claim,
            (Gf::ONE + s) * cofactor[0]
                + s * (cofactor[0] + cofactor[1] + cofactor[2])
        );
        let message = ProverMsg(NatEvaluatedPolyWithoutConstant::new(vec![
            cofactor[1],
            cofactor[2],
        ]));
        transcript.absorb_random_field_slice(&message.0.tail_evaluations, &mut transcript_buf);
        let challenge = transcript.get_field_challenge::<Gf>(&());
        transcript.absorb_random_field(&challenge, &mut transcript_buf);
        messages.push(message);
        point.push(challenge);

        let cofactor_at = eval_quadratic(cofactor, challenge);
        let eq_at = Gf::ONE + s + challenge;
        running_claim = eq_at * cofactor_at;
        for (claim, quadratic) in workspace
            .normalized_claims
            .iter_mut()
            .zip(&workspace.quadratics)
        {
            *claim = eval_quadratic(*quadratic, challenge);
        }
        prefix *= eq_at;

        let can_prefetch = claims.len() == 1
            && round + 1 < dimension
            && active_len(input_view(0, initial, inputs, workspace))
                == active_len(input_view(1, initial, inputs, workspace));
        if can_prefetch {
            let current_len = active_len(input_view(0, initial, inputs, workspace));
            let folded_len = current_len.div_ceil(2);
            let next_pairs = folded_len.div_ceil(2);
            let low_domain = prepare_eq_split(
                &q[round + 2..],
                next_pairs,
                &mut workspace.eq_low,
                &mut workspace.eq_high,
            );
            let next_s = q[round + 1];
            let next_at_one = next_s != Gf::ONE;
            let [finite, infinity] = if initial {
                let (left, right) = if let Some((left, right)) = materialized.as_ref() {
                    (
                        StateView::Dense { values: left, padding: inputs[0].0.padding() },
                        StateView::Dense { values: right, padding: inputs[0].1.padding() },
                    )
                } else {
                    (initial_view(inputs[0].0), initial_view(inputs[0].1))
                };
                let (left_out, right_out) = two_mut(&mut workspace.tables, 0, 1);
                fused_fold_product_round(
                    left,
                    right,
                    left_out,
                    right_out,
                    &workspace.eq_low,
                    &workspace.eq_high,
                    low_domain,
                    domain_len,
                    challenge,
                    next_at_one,
                )
            } else {
                let left = StateView::Dense {
                    values: &workspace.tables[0],
                    padding: workspace.paddings[0],
                };
                let right = StateView::Dense {
                    values: &workspace.tables[1],
                    padding: workspace.paddings[1],
                };
                let (left_out, right_out) = two_mut(&mut workspace.backs, 0, 1);
                let message = fused_fold_product_round(
                    left,
                    right,
                    left_out,
                    right_out,
                    &workspace.eq_low,
                    &workspace.eq_high,
                    low_domain,
                    domain_len,
                    challenge,
                    next_at_one,
                );
                core::mem::swap(&mut workspace.tables[0], &mut workspace.backs[0]);
                core::mem::swap(&mut workspace.tables[1], &mut workspace.backs[1]);
                message
            };
            let (h0, h1) = if next_at_one {
                let h1 = finite;
                let h0 = (workspace.normalized_claims[0] + next_s * h1)
                    * (Gf::ONE + next_s).inverse_or_zero();
                (h0, h1)
            } else {
                (finite, workspace.normalized_claims[0])
            };
            prefetched_round = Some([h0, h0 + h1 + infinity, infinity]);
        } else if initial {
            if let Some((left, right)) = materialized.as_ref() {
                let (left_out, right_out) = two_mut(&mut workspace.tables, 0, 1);
                initialize_view(
                    left_out,
                    StateView::Dense {
                        values: left,
                        padding: inputs[0].0.padding(),
                    },
                    challenge,
                    false,
                );
                initialize_view(
                    right_out,
                    StateView::Dense {
                        values: right,
                        padding: inputs[0].1.padding(),
                    },
                    challenge,
                    false,
                );
            } else {
                initialize_tables(inputs, workspace, challenge);
            }
        } else {
            fold_tables(workspace, table_count, challenge);
        }
        domain_len >>= 1;
    }

    let evals = (0..claims.len())
        .map(|claim| {
            (
                workspace.tables[2 * claim][0],
                workspace.tables[2 * claim + 1][0],
            )
        })
        .collect::<Vec<_>>();
    for (claim, &(left, right)) in evals.iter().enumerate() {
        debug_assert_eq!(workspace.normalized_claims[claim], left * right);
    }
    debug_assert_eq!(
        running_claim,
        evals
            .iter()
            .zip(&workspace.scales)
            .map(|(&(left, right), &scale)| prefix * scale * left * right)
            .sum()
    );
    absorb_evals(transcript, &evals);
    (
        SumcheckProof {
            messages,
            claimed_sum,
        },
        point,
        evals,
    )
}

fn absorb_evals(transcript: &mut impl Transcript, evals: &[(Gf, Gf)]) {
    let flat = evals
        .iter()
        .flat_map(|&(left, right)| [left, right])
        .collect::<Vec<_>>();
    absorb_field_slice(transcript, &flat);
}

fn input_view<'a>(
    slot: usize,
    initial: bool,
    inputs: &'a [(ProductInput<'a>, ProductInput<'a>)],
    workspace: &'a ProductWorkspace,
) -> StateView<'a> {
    let claim = slot % inputs.len();
    let input = if slot < inputs.len() {
        inputs[claim].0
    } else {
        inputs[claim].1
    };
    if initial {
        initial_view(input)
    } else {
        let table = 2 * claim + usize::from(slot >= inputs.len());
        StateView::Dense {
            values: &workspace.tables[table],
            padding: workspace.paddings[table],
        }
    }
}

fn initialize_tables(
    inputs: &[(ProductInput<'_>, ProductInput<'_>)],
    workspace: &mut ProductWorkspace,
    challenge: Gf,
) {
    let tables = &mut workspace.tables[..2 * inputs.len()];
    let total_len = inputs
        .iter()
        .flat_map(|&(left, right)| [left, right])
        .map(|input| input.len().div_ceil(2))
        .sum::<usize>();
    #[cfg(feature = "parallel")]
    let parallel_within_table = total_len >= PARALLEL_THRESHOLD
        && tables.len() < rayon::current_num_threads();
    #[cfg(not(feature = "parallel"))]
    let parallel_within_table = false;
    let fill = |(slot, table): (usize, &mut Vec<Gf>)| {
        let claim = slot / 2;
        let input = if slot & 1 == 0 {
            inputs[claim].0
        } else {
            inputs[claim].1
        };
        initialize_table(table, input, challenge, parallel_within_table);
    };
    #[cfg(feature = "parallel")]
    if total_len >= PARALLEL_THRESHOLD && tables.len() > 1 {
        tables.par_iter_mut().enumerate().for_each(fill);
    } else {
        tables.iter_mut().enumerate().for_each(fill);
    }
    #[cfg(not(feature = "parallel"))]
    tables.iter_mut().enumerate().for_each(fill);
}

fn initialize_table(
    table: &mut Vec<Gf>,
    input: ProductInput<'_>,
    challenge: Gf,
    parallel: bool,
) {
    initialize_view(table, initial_view(input), challenge, parallel);
}

fn initialize_view(
    table: &mut Vec<Gf>,
    input: StateView<'_>,
    challenge: Gf,
    parallel: bool,
) {
    let folded_len = input.len().div_ceil(2);
    assert!(table.capacity() >= folded_len);
    table.clear();
    let spare = &mut table.spare_capacity_mut()[..folded_len];
    let fill = |(index, output): (usize, &mut core::mem::MaybeUninit<Gf>)| {
        let low = input.value(2 * index);
        let high = input.value(2 * index + 1);
        output.write(low + challenge * (low + high));
    };
    #[cfg(feature = "parallel")]
    if parallel && folded_len >= PARALLEL_THRESHOLD {
        spare.par_iter_mut().enumerate().for_each(fill);
    } else {
        spare.iter_mut().enumerate().for_each(fill);
    }
    #[cfg(not(feature = "parallel"))]
    spare.iter_mut().enumerate().for_each(fill);
    unsafe { table.set_len(folded_len) };
}

fn fold_tables(
    workspace: &mut ProductWorkspace,
    table_count: usize,
    challenge: Gf,
) {
    let tables = &mut workspace.tables[..table_count];
    let backs = &mut workspace.backs[..table_count];
    let paddings = &workspace.paddings[..table_count];
    let total_len = tables.iter().map(Vec::len).sum::<usize>();
    #[cfg(feature = "parallel")]
    let inner_parallel = total_len >= PARALLEL_THRESHOLD
        && table_count < rayon::current_num_threads();
    #[cfg(not(feature = "parallel"))]
    let inner_parallel = false;
    let fold = |((table, back), &padding): ((&mut Vec<Gf>, &mut Vec<Gf>), &Gf)| {
        let folded_len = table.len().div_ceil(2);
        assert!(back.capacity() >= folded_len);
        back.clear();
        let spare = &mut back.spare_capacity_mut()[..folded_len];
        let fill = |(index, output): (usize, &mut core::mem::MaybeUninit<Gf>)| {
            let low = table[2 * index];
            let high = table.get(2 * index + 1).copied().unwrap_or(padding);
            output.write(low + challenge * (low + high));
        };
        #[cfg(feature = "parallel")]
        if inner_parallel && folded_len >= PARALLEL_THRESHOLD {
            spare.par_iter_mut().enumerate().for_each(fill);
        } else {
            spare.iter_mut().enumerate().for_each(fill);
        }
        #[cfg(not(feature = "parallel"))]
        spare.iter_mut().enumerate().for_each(fill);
        unsafe { back.set_len(folded_len) };
        core::mem::swap(table, back);
    };
    #[cfg(feature = "parallel")]
    if total_len >= PARALLEL_THRESHOLD && table_count > 1 {
        tables
            .par_iter_mut()
            .zip(backs.par_iter_mut())
            .zip(paddings.par_iter())
            .for_each(fold);
    } else {
        tables.iter_mut().zip(backs).zip(paddings).for_each(fold);
    }
    #[cfg(not(feature = "parallel"))]
    tables.iter_mut().zip(backs).zip(paddings).for_each(fold);
}

#[allow(clippy::too_many_arguments)]
fn fused_fold_product_round(
    left: StateView<'_>,
    right: StateView<'_>,
    left_out: &mut Vec<Gf>,
    right_out: &mut Vec<Gf>,
    eq_low: &[Gf],
    eq_high: &[Gf],
    low_domain: usize,
    domain_len: usize,
    challenge: Gf,
    at_one: bool,
) -> [Gf; 2] {
    debug_assert_eq!(left.len(), right.len());
    let folded_len = left.len().div_ceil(2);
    let pairs = folded_len.div_ceil(2);
    assert!(left_out.capacity() >= folded_len && right_out.capacity() >= folded_len);
    left_out.clear();
    right_out.clear();
    let left_spare = &mut left_out.spare_capacity_mut()[..folded_len];
    let right_spare = &mut right_out.spare_capacity_mut()[..folded_len];
    let high_len = pairs.div_ceil(low_domain);
    let baseline = left.padding() * right.padding();
    let challenge_mul = field::PreparedGf128Mul::new(challenge);

    let block = |(
        high,
        ((left_out, right_out), &high_weight),
    ): (
        usize,
        ((
            &mut [core::mem::MaybeUninit<Gf>],
            &mut [core::mem::MaybeUninit<Gf>],
        ),
        &Gf),
    )| {
        fused_fold_product_block(
            left,
            right,
            left_out,
            right_out,
            eq_low,
            high,
            low_domain,
            high_weight,
            domain_len,
            &challenge_mul,
            baseline,
            at_one,
        )
    };
    let zero = || {
        [
            <Gf as WideMulAcc>::wide_zero(&Gf::ZERO),
            <Gf as WideMulAcc>::wide_zero(&Gf::ZERO),
        ]
    };
    let merge = |
        mut left: [<Gf as WideMulAcc>::Wide; 2],
        right: [<Gf as WideMulAcc>::Wide; 2],
    | {
        <Gf as WideMulAcc>::wide_add_assign(&mut left[0], &right[0]);
        <Gf as WideMulAcc>::wide_add_assign(&mut left[1], &right[1]);
        left
    };
    #[cfg(feature = "parallel")]
    let correction = if folded_len >= PARALLEL_THRESHOLD && high_len > 1 {
        left_spare
            .par_chunks_mut(2 * low_domain)
            .zip(right_spare.par_chunks_mut(2 * low_domain))
            .zip(eq_high[..high_len].par_iter())
            .enumerate()
            .map(block)
            .reduce(zero, merge)
    } else {
        left_spare
            .chunks_mut(2 * low_domain)
            .zip(right_spare.chunks_mut(2 * low_domain))
            .zip(&eq_high[..high_len])
            .enumerate()
            .map(block)
            .fold(zero(), merge)
    };
    #[cfg(not(feature = "parallel"))]
    let correction = left_spare
        .chunks_mut(2 * low_domain)
        .zip(right_spare.chunks_mut(2 * low_domain))
        .zip(&eq_high[..high_len])
        .enumerate()
        .map(block)
        .fold(zero(), merge);

    unsafe {
        left_out.set_len(folded_len);
        right_out.set_len(folded_len);
    }
    [
        baseline + <Gf as WideMulAcc>::from_wide(correction[0].clone()),
        <Gf as WideMulAcc>::from_wide(correction[1].clone()),
    ]
}

#[allow(clippy::too_many_arguments)]
fn fused_fold_product_block(
    left: StateView<'_>,
    right: StateView<'_>,
    left_out: &mut [core::mem::MaybeUninit<Gf>],
    right_out: &mut [core::mem::MaybeUninit<Gf>],
    eq_low: &[Gf],
    high: usize,
    low_domain: usize,
    high_weight: Gf,
    domain_len: usize,
    challenge: &field::PreparedGf128Mul,
    baseline: Gf,
    at_one: bool,
) -> [<Gf as WideMulAcc>::Wide; 2] {
    debug_assert_eq!(left_out.len(), right_out.len());
    let block_pairs = left_out.len().div_ceil(2);
    let base_pair = high * low_domain;
    let mut finite = <Gf as WideMulAcc>::wide_zero(&Gf::ZERO);
    let mut infinity = <Gf as WideMulAcc>::wide_zero(&Gf::ZERO);
    for (low, &weight) in eq_low.iter().take(block_pairs).enumerate() {
        let pair = base_pair + low;
        let output = 2 * low;
        let [left0, right0] = folded_pair(left, right, 2 * pair, domain_len, challenge);
        let (left1, right1) = if output + 1 < left_out.len() {
            let folded = folded_pair(left, right, 2 * pair + 1, domain_len, challenge);
            (folded[0], folded[1])
        } else {
            (left.padding(), right.padding())
        };
        left_out[output].write(left0);
        right_out[output].write(right0);
        if output + 1 < left_out.len() {
            left_out[output + 1].write(left1);
            right_out[output + 1].write(right1);
        }

        let (left_finite, right_finite) = if at_one {
            (left1, right1)
        } else {
            (left0, right0)
        };
        let finite_product = left_finite * right_finite + baseline;
        let infinity_product = (left0 + left1) * (right0 + right1);
        <Gf as WideMulAcc>::wide_add_assign(
            &mut finite,
            &<Gf as WideMulAcc>::mul_wide(&weight, &finite_product),
        );
        <Gf as WideMulAcc>::wide_add_assign(
            &mut infinity,
            &<Gf as WideMulAcc>::mul_wide(&weight, &infinity_product),
        );
    }
    let finite = <Gf as WideMulAcc>::from_wide(finite);
    let infinity = <Gf as WideMulAcc>::from_wide(infinity);
    [
        <Gf as WideMulAcc>::mul_wide(&high_weight, &finite),
        <Gf as WideMulAcc>::mul_wide(&high_weight, &infinity),
    ]
}

fn folded_pair(
    left: StateView<'_>,
    right: StateView<'_>,
    output: usize,
    domain_len: usize,
    challenge: &field::PreparedGf128Mul,
) -> [Gf; 2] {
    debug_assert!(left.len() <= domain_len && right.len() <= domain_len);
    let fold = |state: StateView<'_>| {
        let low = state.value(2 * output);
        let high = state.value(2 * output + 1);
        low + challenge.mul(&(low + high))
    };
    [fold(left), fold(right)]
}

fn two_mut<T>(values: &mut [T], left: usize, right: usize) -> (&mut T, &mut T) {
    assert_ne!(left, right);
    if left < right {
        let (before_right, from_right) = values.split_at_mut(right);
        (&mut before_right[left], &mut from_right[0])
    } else {
        let (before_left, from_left) = values.split_at_mut(left);
        (&mut from_left[0], &mut before_left[right])
    }
}

fn product_round(
    left: StateView<'_>,
    right: StateView<'_>,
    eq_low: &[Gf],
    eq_high: &[Gf],
    low_domain: usize,
    domain_len: usize,
    at_one: bool,
    parallel: bool,
) -> [Gf; 2] {
    let baseline = left.padding() * right.padding();
    let pairs = active_len(left).max(active_len(right)).div_ceil(2);
    if pairs == 0 {
        return [baseline, Gf::ZERO];
    }
    let high_len = pairs.div_ceil(low_domain);
    let block = |high: usize| {
        product_round_block(
            left,
            right,
            eq_low,
            high,
            low_domain,
            eq_high[high],
            pairs,
            domain_len,
            baseline,
            at_one,
        )
    };
    let zero = || {
        [
            <Gf as WideMulAcc>::wide_zero(&Gf::ZERO),
            <Gf as WideMulAcc>::wide_zero(&Gf::ZERO),
        ]
    };
    let merge = |
        mut left: [<Gf as WideMulAcc>::Wide; 2],
        right: [<Gf as WideMulAcc>::Wide; 2],
    | {
        <Gf as WideMulAcc>::wide_add_assign(&mut left[0], &right[0]);
        <Gf as WideMulAcc>::wide_add_assign(&mut left[1], &right[1]);
        left
    };
    #[cfg(feature = "parallel")]
    let sum = if parallel && pairs >= PARALLEL_THRESHOLD && high_len > 1 {
        (0..high_len).into_par_iter().map(block).reduce(zero, merge)
    } else {
        (0..high_len).map(block).fold(zero(), merge)
    };
    #[cfg(not(feature = "parallel"))]
    let sum = (0..high_len).map(block).fold(zero(), merge);
    [
        baseline + <Gf as WideMulAcc>::from_wide(sum[0].clone()),
        <Gf as WideMulAcc>::from_wide(sum[1].clone()),
    ]
}

#[allow(clippy::too_many_arguments)]
fn product_round_materialize(
    left: StateView<'_>,
    right: StateView<'_>,
    left_out: &mut [Gf],
    right_out: &mut [Gf],
    eq_low: &[Gf],
    eq_high: &[Gf],
    low_domain: usize,
    at_one: bool,
) -> [Gf; 2] {
    assert_eq!(left_out.len(), left.len());
    assert_eq!(right_out.len(), right.len());
    assert_eq!(left.len(), right.len());
    let baseline = left.padding() * right.padding();
    let pairs = left.len().max(right.len()).div_ceil(2);
    if pairs == 0 {
        return [baseline, Gf::ZERO];
    }
    let high_len = pairs.div_ceil(low_domain);
    let block = |(
        high,
        ((left_out, right_out), &high_weight),
    ): (
        usize,
        ((&mut [Gf], &mut [Gf]), &Gf),
    )| {
        let base = high * low_domain;
        let block_len = (pairs - base).min(low_domain);
        let mut finite = <Gf as WideMulAcc>::wide_zero(&Gf::ZERO);
        let mut infinity = <Gf as WideMulAcc>::wide_zero(&Gf::ZERO);
        for low in 0..block_len {
            let pair = base + low;
            let index = 2 * pair;
            let left0 = left.value(index);
            let left1 = left.value(index + 1);
            let right0 = right.value(index);
            let right1 = right.value(index + 1);
            let output = 2 * low;
            if index < left.len() {
                left_out[output] = left0;
            }
            if index + 1 < left.len() {
                left_out[output + 1] = left1;
            }
            if index < right.len() {
                right_out[output] = right0;
            }
            if index + 1 < right.len() {
                right_out[output + 1] = right1;
            }
            let (left_finite, right_finite) = if at_one {
                (left1, right1)
            } else {
                (left0, right0)
            };
            let finite_product = left_finite * right_finite + baseline;
            let infinity_product = (left0 + left1) * (right0 + right1);
            let weight = eq_low[low];
            <Gf as WideMulAcc>::wide_add_assign(
                &mut finite,
                &<Gf as WideMulAcc>::mul_wide(&weight, &finite_product),
            );
            <Gf as WideMulAcc>::wide_add_assign(
                &mut infinity,
                &<Gf as WideMulAcc>::mul_wide(&weight, &infinity_product),
            );
        }
        let finite = <Gf as WideMulAcc>::from_wide(finite);
        let infinity = <Gf as WideMulAcc>::from_wide(infinity);
        [
            <Gf as WideMulAcc>::mul_wide(&high_weight, &finite),
            <Gf as WideMulAcc>::mul_wide(&high_weight, &infinity),
        ]
    };
    let zero = || {
        [
            <Gf as WideMulAcc>::wide_zero(&Gf::ZERO),
            <Gf as WideMulAcc>::wide_zero(&Gf::ZERO),
        ]
    };
    let merge = |
        mut left: [<Gf as WideMulAcc>::Wide; 2],
        right: [<Gf as WideMulAcc>::Wide; 2],
    | {
        <Gf as WideMulAcc>::wide_add_assign(&mut left[0], &right[0]);
        <Gf as WideMulAcc>::wide_add_assign(&mut left[1], &right[1]);
        left
    };
    #[cfg(feature = "parallel")]
    let sum = if pairs >= PARALLEL_THRESHOLD && high_len > 1 {
        left_out
            .par_chunks_mut(2 * low_domain)
            .zip(right_out.par_chunks_mut(2 * low_domain))
            .zip(eq_high[..high_len].par_iter())
            .enumerate()
            .map(block)
            .reduce(zero, merge)
    } else {
        left_out
            .chunks_mut(2 * low_domain)
            .zip(right_out.chunks_mut(2 * low_domain))
            .zip(&eq_high[..high_len])
            .enumerate()
            .map(block)
            .fold(zero(), merge)
    };
    #[cfg(not(feature = "parallel"))]
    let sum = left_out
        .chunks_mut(2 * low_domain)
        .zip(right_out.chunks_mut(2 * low_domain))
        .zip(&eq_high[..high_len])
        .enumerate()
        .map(block)
        .fold(zero(), merge);
    [
        baseline + <Gf as WideMulAcc>::from_wide(sum[0].clone()),
        <Gf as WideMulAcc>::from_wide(sum[1].clone()),
    ]
}

#[allow(clippy::too_many_arguments)]
fn product_round_block(
    left: StateView<'_>,
    right: StateView<'_>,
    eq_low: &[Gf],
    high: usize,
    low_len: usize,
    high_weight: Gf,
    pairs: usize,
    domain_len: usize,
    baseline: Gf,
    at_one: bool,
) -> [<Gf as WideMulAcc>::Wide; 2] {
    let base = high * low_len;
    let block_len = (pairs - base).min(low_len);
    let mut finite = <Gf as WideMulAcc>::wide_zero(&Gf::ZERO);
    let mut infinity = <Gf as WideMulAcc>::wide_zero(&Gf::ZERO);
    let kernel_len = match (left, right) {
        (
            StateView::Dense { values: left, .. },
            StateView::Dense { values: right, .. },
        ) => (left.len() / 2)
            .min(right.len() / 2)
            .saturating_sub(base)
            .min(block_len),
        _ => 0,
    };
    if kernel_len != 0 {
        let start = 2 * base;
        let end = start + 2 * kernel_len;
        let (left_values, right_values) = match (left, right) {
            (
                StateView::Dense { values: left, .. },
                StateView::Dense { values: right, .. },
            ) => (left, right),
            _ => unreachable!(),
        };
        let [mut finite_value, quadratic] = field::Gf128Ops.eqf_gruen_pair_round(
            &left_values[start..end],
            &right_values[start..end],
            eq_low,
            kernel_len,
            at_one,
        );
        if baseline != Gf::ZERO {
            let weight_sum = if kernel_len == low_len {
                Gf::ONE
            } else {
                eq_low[..kernel_len].iter().copied().sum()
            };
            finite_value += baseline * weight_sum;
        }
        finite = <Gf as WideMulAcc>::wide_of(&finite_value);
        infinity = <Gf as WideMulAcc>::wide_of(&quadratic);
    }
    for low in kernel_len..block_len {
        let pair = base + low;
        let (left0, left_delta) = pair_coeffs(left, pair, domain_len);
        let (right0, right_delta) = pair_coeffs(right, pair, domain_len);
        let (left_finite, right_finite) = if at_one {
            (left0 + left_delta, right0 + right_delta)
        } else {
            (left0, right0)
        };
        let finite_product = left_finite * right_finite + baseline;
        let infinity_product = left_delta * right_delta;
        let weight = eq_low[low];
        <Gf as WideMulAcc>::wide_add_assign(
            &mut finite,
            &<Gf as WideMulAcc>::mul_wide(&weight, &finite_product),
        );
        <Gf as WideMulAcc>::wide_add_assign(
            &mut infinity,
            &<Gf as WideMulAcc>::mul_wide(&weight, &infinity_product),
        );
    }
    let finite = <Gf as WideMulAcc>::from_wide(finite);
    let infinity = <Gf as WideMulAcc>::from_wide(infinity);
    [
        <Gf as WideMulAcc>::mul_wide(&high_weight, &finite),
        <Gf as WideMulAcc>::mul_wide(&high_weight, &infinity),
    ]
}

fn prepare_eq_split(
    point: &[Gf],
    prefix_len: usize,
    low: &mut Vec<Gf>,
    high: &mut Vec<Gf>,
) -> usize {
    if prefix_len == 0 {
        low.clear();
        high.clear();
        return 1;
    }
    let low_bits = point.len() / 2;
    let low_domain = 1usize << low_bits;
    let low_len = prefix_len.min(low_domain);
    let high_len = prefix_len.div_ceil(low_domain);
    assert!(
        low.capacity() >= low_len && high.capacity() >= high_len,
        "eq split under-reserved: tail_dim={}, pairs={}, split={}/{}, capacities={}/{}",
        point.len(),
        prefix_len,
        low_len,
        high_len,
        low.capacity(),
        high.capacity(),
    );
    fill_eq_prefix(&point[..low_bits], low_len, low);
    fill_eq_prefix(&point[low_bits..], high_len, high);
    low_domain
}

fn fill_eq_prefix(point: &[Gf], prefix_len: usize, output: &mut Vec<Gf>) {
    assert!(
        output.capacity() >= prefix_len,
        "eq prefix under-reserved: point_dim={}, prefix_len={}, capacity={}",
        point.len(),
        prefix_len,
        output.capacity(),
    );
    output.clear();
    if prefix_len == 0 {
        return;
    }
    output.push(Gf::ONE);
    let mut logical_len = 1usize;
    for &coordinate in point {
        let needed = prefix_len.min(logical_len << 1);
        let low_factor = Gf::ONE + coordinate;
        if needed <= logical_len {
            debug_assert_eq!(output.len(), needed);
            for value in output.iter_mut() {
                *value *= low_factor;
            }
        } else {
            debug_assert_eq!(output.len(), logical_len);
            let high_len = needed - logical_len;
            output.resize(needed, Gf::ZERO);
            let (low, high) = output.split_at_mut(logical_len);
            let (low_with_high, low_only) = low.split_at_mut(high_len);
            for (low, high) in low_with_high.iter_mut().zip(high) {
                let value = *low;
                *low = value * low_factor;
                *high = value * coordinate;
            }
            for value in low_only {
                *value *= low_factor;
            }
        }
        logical_len <<= 1;
    }
    debug_assert_eq!(output.len(), prefix_len);
}

fn pair_coeffs(state: StateView<'_>, pair: usize, domain_len: usize) -> (Gf, Gf) {
    debug_assert!(state.len() <= domain_len);
    let index = 2 * pair;
    let low = state.value(index);
    let high = state.value(index + 1);
    (low, low + high)
}

fn active_len(state: StateView<'_>) -> usize {
    state.len()
}

fn eval_quadratic(coefficients: [Gf; 3], point: Gf) -> Gf {
    coefficients[0] + point * (coefficients[1] + point * coefficients[2])
}

fn split_eq_capacities(max_dimension: usize, max_pairs: usize) -> (usize, usize) {
    let mut low = 0;
    let mut high = 0;
    for dimension in 1..=max_dimension {
        let mut pairs = max_pairs.min(1usize << (dimension - 1));
        for round in 0..dimension {
            let low_domain = 1usize << ((dimension - round - 1) / 2);
            low = low.max(pairs.min(low_domain));
            high = high.max(pairs.div_ceil(low_domain));
            pairs = pairs.div_ceil(2);
        }
    }
    (low, high)
}

fn reserve_vec<T>(values: &mut Vec<T>, capacity: usize) {
    if values.capacity() < capacity {
        values.reserve(capacity - values.len());
    }
}
