//! Word-domain binder: Λ·signed15 + H·lane15. The source remains bit committed.
//!
//! The honest lane-15 table is supported on the slack positions. We check that
//! support against every source word before using its compact representation;
//! the verifier authenticates the projection of *all* lane-15 bits.
use super::*;
use crate::sumcheck::RoundBoundaryPolicy;
use field::{FpProductAcc, FpSignedLinearAcc, IntegerEmbedding, PreparedLinearCombination, Z};

const DOMAIN: &[u8] = b"bitz/falcon-algebraic/word-binding/v1";
const TERMINAL_DOMAIN: &[u8] = b"bitz/falcon-algebraic/word-binding-terminal/v1";
const SLACK_WIDTH: usize = SLACK_BITS.next_power_of_two();
const SLACK_LOG: usize = SLACK_WIDTH.ilog2() as usize;
type Row = [F; 2]; // Public weight, witness value.

crate::piop::spartan::grinding::grinding_domains! {
    TerminalMerge => b"bitz/falcon-algebraic/grinding/word-terminal/v1";
}

pub(super) struct Output {
    pub proof: SumcheckProof<F, 3>,
    pub terminal: [F; 2],
    pub nonces: Vec<u64>,
    pub merge_nonce: Option<u64>,
    pub claim: BridgeClaim,
}

/// Source words are ordered coefficient, side, then instance (low bit first).
fn word(source: &Source, index: usize) -> u16 {
    let words_per_column = 1usize << (source.layout().row_vars() - 4);
    (source.rows()[index / words_per_column][(index % words_per_column) / 4] >> (16 * (index % 4)))
        as u16
}
fn signed_word(source: &Source, index: usize) -> i64 {
    i64::from(((word(source, index) << 1) as i16) >> 1)
}
fn weight(form: &BindingForm<'_>, index: usize) -> F {
    form.coefficients[(index / N) % 2][(index / (2 * N)) * N + index % N]
}
fn interpolate(field: &Cfg, a: F, b: F, challenge: F) -> F {
    field.add(&a, &field.mul(&challenge, &field.sub(&b, &a)))
}
fn accumulate_product_pair(field: &Cfg, sums: &mut [FpProductAcc<2>; 2], a: Row, b: Row) {
    field.mul_acc(&mut sums[0], &a[0], &a[1]);
    field.mul_acc(
        &mut sums[1],
        &field.sub(&b[0], &a[0]),
        &field.sub(&b[1], &a[1]),
    );
}
fn product_coefficients(field: &Cfg, rows: &[Row], constant: F) -> [F; 2] {
    let zero = || [FpProductAcc::<2>::default(); 2];
    let accumulate = |mut sums: [FpProductAcc<2>; 2], pair: &[Row]| {
        field.mul_acc(
            &mut sums[0],
            &field.add(&constant, &pair[0][0]),
            &pair[0][1],
        );
        field.mul_acc(
            &mut sums[1],
            &field.sub(&pair[1][0], &pair[0][0]),
            &field.sub(&pair[1][1], &pair[0][1]),
        );
        sums
    };
    let sums = if rows.len() >= 4096 && binding_workers() > 1 {
        rows.par_chunks_exact(2)
            .fold(zero, accumulate)
            .reduce(zero, |a, b| [a[0] + b[0], a[1] + b[1]])
    } else {
        rows.chunks_exact(2).fold(zero(), accumulate)
    };
    let reduce = field.prepare_product_reduction(rows.len() / 2);
    sums.map(|sum| reduce.reduce(sum))
}

/// Fuse the compact side/instance folds and the next round's accumulation.
fn fold_dense(
    field: &Cfg,
    output: &mut Vec<Row>,
    len: usize,
    read: impl Fn(usize) -> Row + Sync,
) -> [F; 2] {
    output.resize(len, [field.zero(); 2]);
    if len == 1 {
        output[0] = read(0);
        return [field.zero(); 2];
    }
    let zero = || [FpProductAcc::<2>::default(); 2];
    let fill = |mut sums: [FpProductAcc<2>; 2], (pair, rows): (usize, &mut [Row])| {
        rows[0] = read(2 * pair);
        rows[1] = read(2 * pair + 1);
        accumulate_product_pair(field, &mut sums, rows[0], rows[1]);
        sums
    };
    let sums = if len >= 4096 && binding_workers() > 1 {
        output
            .par_chunks_exact_mut(2)
            .enumerate()
            .fold(zero, fill)
            .reduce(zero, |a, b| [a[0] + b[0], a[1] + b[1]])
    } else {
        output.chunks_exact_mut(2).enumerate().fold(zero(), fill)
    };
    let reduce = field.prepare_product_reduction(len / 2);
    sums.map(|sum| reduce.reduce(sum))
}

/// Reuse the two Λ allocations for the coefficient rounds. Each signature has
/// an N-field stride: first, each weight pair becomes [folded Λ, folded A];
/// later rounds compress those rows into the beginning of the same chunk.
/// The writes proceed left to right and never reach an unread input row.
fn fold_coefficients(
    field: &Cfg,
    tables: &mut [Vec<F>; 2],
    source: &Source,
    round: usize,
    challenge: F,
) -> [F; 2] {
    let remaining_rows = N >> (round + 1);
    let native_fold = (round == 0).then(|| {
        <Cfg as PreparedLinearCombination<Z<1>>>::prepare_linear_combination(
            field,
            [field.sub(&field.one(), &challenge), challenge],
        )
    });
    let zero = || [FpProductAcc::<2>::default(); 2];
    let process = |mut sums: [FpProductAcc<2>; 2],
                   (instance, (s1, s2)): (usize, (&mut [F], &mut [F]))| {
        for (side, chunk) in [s1, s2].into_iter().enumerate() {
            // Read both rows before writing either compressed row. For round
            // zero each input is one weight; afterward it is [weight, value].
            let read_folded = |chunk: &[F], row: usize| -> Row {
                if let Some(native_fold) = &native_fold {
                    let first_word = instance * 2 * N + side * N + 2 * row;
                    let value = <Cfg as PreparedLinearCombination<Z<1>>>::linear_combination(
                        native_fold,
                        |j| Z::from(signed_word(source, first_word + j)),
                    );
                    [
                        interpolate(field, chunk[2 * row], chunk[2 * row + 1], challenge),
                        value,
                    ]
                } else {
                    std::array::from_fn(|j| {
                        interpolate(field, chunk[4 * row + j], chunk[4 * row + 2 + j], challenge)
                    })
                }
            };
            if remaining_rows == 1 {
                let row = read_folded(chunk, 0);
                chunk[..2].copy_from_slice(&row);
                continue;
            }
            for pair in 0..remaining_rows / 2 {
                let a = read_folded(chunk, 2 * pair);
                let b = read_folded(chunk, 2 * pair + 1);
                chunk[4 * pair..4 * pair + 2].copy_from_slice(&a);
                chunk[4 * pair + 2..4 * pair + 4].copy_from_slice(&b);
                accumulate_product_pair(field, &mut sums, a, b);
            }
        }
        sums
    };
    let [s1, s2] = tables;
    let sums = if binding_workers() > 1 {
        s1.par_chunks_exact_mut(N)
            .zip(s2.par_chunks_exact_mut(N))
            .enumerate()
            .fold(zero, process)
            .reduce(zero, |a, b| [a[0] + b[0], a[1] + b[1]])
    } else {
        s1.chunks_exact_mut(N)
            .zip(s2.chunks_exact_mut(N))
            .enumerate()
            .fold(zero(), process)
    };
    if remaining_rows == 1 {
        // The next variable is side, rather than another coefficient bit.
        let mut sums = zero();
        for (a, b) in s1.chunks_exact(N).zip(s2.chunks_exact(N)) {
            accumulate_product_pair(field, &mut sums, [a[0], a[1]], [b[0], b[1]]);
        }
        let reduce = field.prepare_product_reduction(s1.len() / N);
        sums.map(|sum| reduce.reduce(sum))
    } else {
        let reduce = field.prepare_product_reduction((s1.len() / N) * remaining_rows);
        sums.map(|sum| reduce.reduce(sum))
    }
}

struct SlackTable {
    rows: Vec<Row>, // H - padding, B; omitted coefficient/side coordinates are zero.
    scratch: Vec<Row>,
    padding: F,
}
impl SlackTable {
    fn new(form: &BindingForm<'_>, source: &Source) -> Result<Self, FalconError> {
        let field = form.field;
        let mut rows = vec![[field.zero(); 2]; form.layout.batch() * SLACK_WIDTH];
        // Scanning all packed high bits is essential: the compact representation
        // must never silently discard a nonzero padding bit in the commitment.
        for instance in 0..form.layout.batch() {
            let mut slack_weight = form.slacks[instance];
            for j in 0..2 * N {
                let bit = word(source, instance * 2 * N + j) >> 15;
                if j < SLACK_BITS {
                    rows[instance * SLACK_WIDTH + j] = [
                        field.sub(&slack_weight, &form.padding),
                        field.from_integer(&u64::from(bit)),
                    ];
                    slack_weight = field.add(&slack_weight, &slack_weight);
                } else if bit != 0 {
                    return Err(failure("nonzero word binder source padding"));
                }
            }
        }
        Ok(Self {
            rows,
            scratch: Vec::new(),
            padding: form.padding,
        })
    }
    fn omitted(round: usize) -> bool {
        (SLACK_LOG..COEFFICIENT_LOG + 1).contains(&round)
    }
    fn coefficients(&self, round: usize, field: &Cfg) -> [F; 2] {
        if !Self::omitted(round) {
            return product_coefficients(field, &self.rows, self.padding);
        }
        // For a missing coordinate, D(1)=B(1)=0, H=padding+D.
        let mut sums = [FpProductAcc::<2>::default(); 2];
        for &[d, b] in &self.rows {
            field.mul_acc(&mut sums[0], &field.add(&self.padding, &d), &b);
            field.mul_acc(&mut sums[1], &d, &b);
        }
        let reduce = field.prepare_product_reduction(self.rows.len());
        sums.map(|sum| reduce.reduce(sum))
    }
    fn fold(&mut self, round: usize, r: F, field: &Cfg) {
        if Self::omitted(round) {
            let complement = field.sub(&field.one(), &r);
            for row in &mut self.rows {
                *row = row.map(|value| field.mul(&value, &complement));
            }
        } else {
            self.scratch.resize(self.rows.len() / 2, [field.zero(); 2]);
            for (output, pair) in self.scratch.iter_mut().zip(self.rows.chunks_exact(2)) {
                *output = std::array::from_fn(|j| interpolate(field, pair[0][j], pair[1][j], r));
            }
            std::mem::swap(&mut self.rows, &mut self.scratch);
        }
    }
}

pub(super) fn prove(
    transcript: &mut impl Transcript,
    form: BindingForm<'_>,
    target: F,
    source: &Source,
    schedule: &Schedule,
) -> Result<Output, FalconError> {
    let field = form.field;
    let layout = form.layout;
    if layout.batch() != layout.capacity() {
        return Err(failure("word binding requires a full-capacity batch"));
    }
    let rounds = source_rounds(layout) - 4;
    let mut slack = SlackTable::new(&form, source)?;
    let native_instance = |instance: usize| {
        let mut sums = [FpSignedLinearAcc::<2, 1>::default(); 2];
        for pair in 0..N {
            let i = instance * 2 * N + 2 * pair;
            let [a, b] = [signed_word(source, i), signed_word(source, i + 1)];
            let [wa, wb] = [weight(&form, i), weight(&form, i + 1)];
            field.mul_acc(&mut sums[0], &wa, &a);
            field.mul_acc(&mut sums[1], &field.sub(&wb, &wa), &(b - a));
        }
        sums
    };
    let sums = if binding_workers() > 1 {
        (0..layout.batch())
            .into_par_iter()
            .map(native_instance)
            .reduce(
                || [FpSignedLinearAcc::<2, 1>::default(); 2],
                |a, b| [a[0] + b[0], a[1] + b[1]],
            )
    } else {
        (0..layout.batch())
            .map(native_instance)
            .fold([FpSignedLinearAcc::<2, 1>::default(); 2], |a, b| {
                [a[0] + b[0], a[1] + b[1]]
            })
    };
    let mut coefficients = sums.map(|sum| field.reduce(sum));
    let mut coefficient_tables = Some(form.coefficients);
    let mut dense = Vec::<Row>::new();
    let mut scratch = Vec::<Row>::new();
    let mut claim = target;
    let mut point = Vec::with_capacity(rounds);
    let mut proof = SumcheckProof {
        round_polynomials: Vec::with_capacity(rounds),
    };
    let mut boundary =
        ProverGrindingRoundBoundary::<BindingRound>::with_round_offset(schedule.binding_bits, 0);
    boundary.validate(rounds).map_err(failure)?;
    transcript.absorb_slice(DOMAIN);
    for round in 0..rounds {
        let addition = slack.coefficients(round, field);
        let c0 = field.add(&coefficients[0], &addition[0]);
        let c2 = field.add(&coefficients[1], &addition[1]);
        let c1 = field.sub(&field.sub(&claim, &field.add(&c0, &c0)), &c2);
        let message = [c0, c1, c2];
        absorb_field_elements(transcript, &message, field);
        proof.round_polynomials.push(message);
        boundary.after_round(transcript, round).map_err(failure)?;
        let r = sample(transcript, field)?;
        point.push(r);
        claim = field.add(&c0, &field.mul(&r, &field.add(&c1, &field.mul(&r, &c2))));
        if round < COEFFICIENT_LOG {
            coefficients = fold_coefficients(
                field,
                coefficient_tables.as_mut().unwrap(),
                source,
                round,
                r,
            );
        } else if round == COEFFICIENT_LOG {
            let [s1, s2] = coefficient_tables.take().unwrap();
            coefficients = fold_dense(field, &mut dense, layout.batch(), |instance| {
                std::array::from_fn(|j| {
                    interpolate(field, s1[instance * N + j], s2[instance * N + j], r)
                })
            });
        } else {
            coefficients = fold_dense(field, &mut scratch, dense.len() / 2, |i| {
                std::array::from_fn(|j| interpolate(field, dense[2 * i][j], dense[2 * i + 1][j], r))
            });
            std::mem::swap(&mut dense, &mut scratch);
        }
        slack.fold(round, r, field);
    }
    let terminal = [dense[0][1], slack.rows[0][1]];
    if claim
        != field.add(
            &field.mul(&dense[0][0], &terminal[0]),
            &field.mul(&field.add(&slack.padding, &slack.rows[0][0]), &terminal[1]),
        )
    {
        return Err(failure("algebraic word binding terminal mismatch"));
    }
    transcript.absorb_slice(TERMINAL_DOMAIN);
    absorb_field_elements(transcript, &terminal, field);
    let merge_nonce = prove_nonce::<TerminalMerge>(transcript, schedule.terminal_merge_bits)?;
    let zeta = sample(transcript, field)?;
    let claim = opening(transcript, layout, &point, terminal, zeta, field)?;
    Ok(Output {
        proof,
        terminal,
        nonces: boundary.into_nonces(),
        merge_nonce,
        claim,
    })
}

pub(super) fn verify(
    transcript: &mut impl Transcript,
    form: &BindingForm<'_>,
    target: F,
    proof: &Proof,
    schedule: &Schedule,
) -> Result<BridgeClaim, FalconError> {
    let field = form.field;
    let layout = form.layout;
    if layout.batch() != layout.capacity() {
        return Err(failure("word binding requires a full-capacity batch"));
    }
    transcript.absorb_slice(DOMAIN);
    let mut boundary = VerifierGrindingRoundBoundary::<BindingRound>::new(
        schedule.binding_bits,
        &proof.binding_nonces,
    );
    let (point, claim) = proof
        .binding
        .verify_with_round_boundary(
            transcript,
            target,
            source_rounds(layout) - 4,
            field,
            &mut boundary,
        )
        .map_err(failure)?;
    validate_field_elements(&proof.binding_terminal, field).map_err(failure)?;
    let [lambda, h] = evaluate(form, &point)?;
    if claim
        != field.add(
            &field.mul(&lambda, &proof.binding_terminal[0]),
            &field.mul(&h, &proof.binding_terminal[1]),
        )
    {
        return Err(failure("algebraic word binding terminal mismatch"));
    }
    transcript.absorb_slice(TERMINAL_DOMAIN);
    absorb_field_elements(transcript, &proof.binding_terminal, field);
    verify_nonce::<TerminalMerge>(
        transcript,
        schedule.terminal_merge_bits,
        proof.terminal_merge_nonce,
    )?;
    let zeta = sample(transcript, field)?;
    opening(
        transcript,
        layout,
        &point,
        proof.binding_terminal,
        zeta,
        field,
    )
}

fn evaluate(form: &BindingForm<'_>, point: &[F]) -> Result<[F; 2], FalconError> {
    let field = form.field;
    if point.len() != source_rounds(form.layout) - 4 {
        return Err(failure("word binding point dimension mismatch"));
    }
    let local = eq_table(&point[..COEFFICIENT_LOG + 1], field).map_err(failure)?;
    let instances = eq_table(&point[COEFFICIENT_LOG + 1..], field).map_err(failure)?;
    let reduce = field.prepare_product_reduction(2 * N);
    let subtotal = |instance| reduce.reduce(form.coefficient_product_sum(instance, &local));
    let values: Vec<_> = if binding_workers() > 1 {
        (0..form.layout.batch())
            .into_par_iter()
            .map(subtotal)
            .collect()
    } else {
        (0..form.layout.batch()).map(subtotal).collect()
    };
    let mut lambda = FpProductAcc::<2>::default();
    let mut slack = FpProductAcc::<2>::default();
    for ((value, slack_weight), instance) in values.iter().zip(&form.slacks).zip(&instances) {
        field.mul_acc(&mut lambda, value, instance);
        field.mul_acc(&mut slack, slack_weight, instance);
    }
    let mut slack_positions = field.zero();
    let mut decoder = FpLinearAcc::<2, 1>::default();
    for (j, weight) in local[..SLACK_BITS].iter().enumerate() {
        slack_positions = field.add(&slack_positions, weight);
        field.mul_acc(&mut decoder, weight, &(1u64 << j));
    }
    let reduce = field.prepare_product_reduction(form.layout.batch());
    let h = field.add(
        &field.mul(&field.reduce(decoder), &reduce.reduce(slack)),
        &field.mul(&form.padding, &field.sub(&field.one(), &slack_positions)),
    );
    Ok([reduce.reduce(lambda), h])
}

fn opening(
    transcript: &mut impl Transcript,
    layout: &Layout,
    point: &[F],
    terminal: [F; 2],
    zeta: F,
    field: &Cfg,
) -> Result<BridgeClaim, FalconError> {
    let low_variables = layout.row_vars() - 4;
    let coefficient_weights = eq_table(&point[..low_variables], field).map_err(failure)?;
    let mut lane_weights = [field.zero(); 16];
    let mut power = field.one();
    for (lane, value) in lane_weights.iter_mut().enumerate().take(15) {
        *value = if lane == 14 { field.neg(&power) } else { power };
        power = field.add(&power, &power);
    }
    lane_weights[15] = zeta;
    let row_weights: Vec<_> = coefficient_weights
        .iter()
        .flat_map(|coefficient| {
            lane_weights.map(|lane| u128::from(field.to_integer(&field.mul(coefficient, &lane))))
        })
        .collect();
    let column_point = point[low_variables..].to_vec();
    let value = field.add(&terminal[0], &field.mul(&zeta, &terminal[1]));
    transcript.absorb_slice(b"bitz/falcon-algebraic/word-source-opening/v1");
    transcript.absorb_slice(&field.modulus_u128().to_le_bytes());
    for weight in &row_weights {
        transcript.absorb_slice(&weight.to_le_bytes());
    }
    absorb_field_elements(transcript, &column_point, field);
    absorb_field_elements(transcript, &[value], field);
    Ok(BridgeClaim {
        column_point,
        modulus: field.modulus_u128(),
        value,
        row_weights,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{sumcheck::inner::prove_inner_sumcheck, transcript::Blake3Transcript};

    fn schedule() -> Schedule {
        Schedule {
            norm_instance_bits: 0,
            norm_round_bits: 0,
            merge_bits: 0,
            binding_bits: 2,
            word_binding: true,
            terminal_merge_bits: 2,
        }
    }
    fn fixture<'a>(layout: &'a Layout, field: &'a Cfg) -> (BindingForm<'a>, Source) {
        let batch = layout.batch();
        let values = [-16384, -1, 0, 1, 16383];
        let data = WitnessData {
            witness: super::super::super::FalconAlgebraicWitness {
                s1: (0..batch)
                    .map(|i| std::array::from_fn(|j| values[(i + j) % values.len()]))
                    .collect(),
                s2: (0..batch)
                    .map(|i| std::array::from_fn(|j| values[(2 * i + j + 1) % values.len()]))
                    .collect(),
            },
            slacks: (0..batch)
                .map(|i| (0x5a69c3du64 ^ i as u64) & ((1 << SLACK_BITS) - 1))
                .collect(),
            quotients: Vec::new(),
        };
        let form = BindingForm {
            layout,
            field,
            coefficients: std::array::from_fn(|side| {
                (0..batch * N)
                    .map(|i| {
                        field.from_integer(&(field.modulus_u128() - 1 - (i + side * 31) as u128))
                    })
                    .collect()
            }),
            slacks: (0..batch)
                .map(|i| field.from_integer(&(field.modulus_u128() - 101 - i as u128)))
                .collect(),
            padding: field.from_integer(&123456789u64),
            options: BindingOptions::default(),
            folded_decoder: Default::default(),
        };
        (form, Source::new(*layout, &data))
    }
    fn dense_tables(form: &BindingForm<'_>, source: &Source) -> (Vec<F>, Vec<F>, F) {
        let field = form.field;
        let words = form.layout.source_bits() / 16;
        let mut values = Vec::with_capacity(2 * words);
        let mut weights = Vec::with_capacity(2 * words);
        for i in 0..words {
            values.push(field.from_integer(&signed_word(source, i)));
            weights.push(weight(form, i));
        }
        for i in 0..words {
            values.push(field.from_integer(&u64::from(word(source, i) >> 15)));
            let j = i % (2 * N);
            weights.push(if j < SLACK_BITS {
                field.reduce(field.mul_wide(&form.slacks[i / (2 * N)], &(1u64 << j)))
            } else {
                form.padding
            });
        }
        let target = values
            .iter()
            .zip(&weights)
            .fold(field.zero(), |sum, (a, b)| {
                field.add(&sum, &field.mul(a, b))
            });
        (values, weights, target)
    }
    fn proof(output: &Output, field: &Cfg) -> Proof {
        Proof {
            norm: NormProof {
                instance_nonce: None,
                slack: field.zero(),
                sumcheck: SumcheckProof {
                    round_polynomials: Vec::new(),
                },
                terminal: field.zero(),
                grinding_nonces: Vec::new(),
            },
            merge_nonce: None,
            binding: output.proof.clone(),
            binding_terminal: output.terminal,
            binding_nonces: output.nonces.clone(),
            terminal_merge_nonce: output.merge_nonce,
        }
    }

    #[test]
    fn word_sumcheck_matches_dense_two_product_oracle_and_bit_opening() {
        let field = F::make_cfg(&field::Uint::from((1u128 << 127) - 1)).unwrap();
        let batch = Layout::new(1).unwrap().capacity();
        let layout = Layout::new(batch).unwrap();
        let (form, source) = fixture(&layout, &field);
        let (values, weights, target) = dense_tables(&form, &source);
        let rounds = source_rounds(&layout) - 4;
        let mut oracle_transcript = Blake3Transcript::new();
        oracle_transcript.absorb_slice(DOMAIN);
        let mut boundary = ProverGrindingRoundBoundary::<BindingRound>::with_round_offset(
            schedule().binding_bits,
            0,
        );
        // A final selector variable puts both products into one ordinary dot
        // product. Its first `rounds` messages are an independent dense oracle.
        let oracle = prove_inner_sumcheck(
            &field,
            &mut oracle_transcript,
            target,
            values,
            weights,
            &mut boundary,
        )
        .unwrap();
        let output = prove(
            &mut Blake3Transcript::new(),
            form,
            target,
            &source,
            &schedule(),
        )
        .unwrap();
        assert_eq!(
            output.proof.round_polynomials,
            oracle.proof.round_polynomials[..rounds]
        );
        assert_eq!(output.nonces, boundary.into_nonces()[..rounds]);
        let (form, _) = fixture(&layout, &field);
        assert_eq!(
            verify(
                &mut Blake3Transcript::new(),
                &form,
                target,
                &proof(&output, &field),
                &schedule()
            )
            .unwrap(),
            output.claim
        );

        let column_weights = eq_table(&output.claim.column_point, &field).unwrap();
        let row_weights = &output.claim.row_weights;
        let mut opening = field.zero();
        for (column, column_weight) in column_weights.iter().enumerate() {
            let mut sum = field.zero();
            for (row, weight) in row_weights.iter().enumerate() {
                if source.bit(column * row_weights.len() + row) {
                    sum = field.add(&sum, &field.from_integer(weight));
                }
            }
            opening = field.add(&opening, &field.mul(&sum, column_weight));
        }
        assert_eq!(opening, output.claim.value);
        for index in 0..2 {
            let mut bad = proof(&output, &field);
            bad.binding_terminal[index] = field.add(&bad.binding_terminal[index], &field.one());
            assert!(
                verify(
                    &mut Blake3Transcript::new(),
                    &form,
                    target,
                    &bad,
                    &schedule()
                )
                .is_err()
            );
        }
        let mut bad = proof(&output, &field);
        bad.terminal_merge_nonce = None;
        assert!(
            verify(
                &mut Blake3Transcript::new(),
                &form,
                target,
                &bad,
                &schedule()
            )
            .is_err()
        );
        let mut bad = proof(&output, &field);
        bad.binding_nonces.pop();
        assert!(
            verify(
                &mut Blake3Transcript::new(),
                &form,
                target,
                &bad,
                &schedule()
            )
            .is_err()
        );
    }

    #[test]
    fn word_source_mutations_and_partial_batches_are_rejected() {
        let field = F::make_cfg(&field::Uint::from((1u128 << 127) - 1)).unwrap();
        let batch = Layout::new(1).unwrap().capacity();
        let layout = Layout::new(batch).unwrap();
        let (form, source) = fixture(&layout, &field);
        let (_, _, target) = dense_tables(&form, &source);
        for index in [
            0,
            14,
            15,
            16 * SLACK_BITS + 15,
            16 * N + 15,
            layout.source_bits() - 1,
        ] {
            let (form, mut source) = fixture(&layout, &field);
            source.flip(index);
            assert!(
                prove(
                    &mut Blake3Transcript::new(),
                    form,
                    target,
                    &source,
                    &schedule()
                )
                .is_err(),
                "changed bit {index}"
            );
        }
        let layout = Layout::new(batch - 1).unwrap();
        let (form, source) = fixture(&layout, &field);
        assert!(
            prove(
                &mut Blake3Transcript::new(),
                form,
                field.zero(),
                &source,
                &schedule()
            )
            .is_err()
        );
    }
}
