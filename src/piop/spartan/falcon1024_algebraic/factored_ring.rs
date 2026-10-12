//! Experimental degree-20 integer lift of the native Falcon ring relation.
//!
//! Instance weights remain separate from the canonical alpha-basis coordinates
//! of the two operands. The integer polynomial is read at alpha in the native
//! extension before its independent prime-field projection is authenticated by
//! the caller's binding proof.

use field::{
    PreparedLinearCombination, RingOps,
    q12289::{PowerBasis, PowerCoordinates, PreparedPublicDotProducts},
};
use rayon::prelude::*;

use crate::{
    piop::spartan::{
        SpartanField,
        falcon_extension::{
            FalconExtension, absorb_extensions, equality_weights, sample_coordinates,
        },
        grinding::{GrindingRound, grind_and_absorb, verify_and_absorb},
        squeeze_field,
    },
    transcript::traits::Transcript,
};

use super::{Cfg, F, FalconAlgebraicStatement, FalconError, Layout, N, Q, WitnessData};

pub(super) const EXTENSION_DEGREE: usize = 11;
pub(super) const LIFT_COEFFICIENTS: usize = 2 * EXTENSION_DEGREE - 1;
type Ext = FalconExtension<EXTENSION_DEGREE>;
type CoordinatesInAlpha = PowerCoordinates<EXTENSION_DEGREE>;
const COEFFICIENT_ABS_BOUND: u128 = 1 << 14;
const DOMAIN: &[u8] = b"bitz/falcon-algebraic/native-ring/factored-alpha-lift/v1";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Proof {
    pub lift: [i128; LIFT_COEFFICIENTS],
    pub projection_nonce: Option<u64>,
}

impl Proof {
    pub(super) fn payload_size_bytes(&self) -> usize {
        16 * LIFT_COEFFICIENTS + usize::from(self.projection_nonce.is_some()) * 8
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct PreparedClaim {
    /// Live instance factors A_i(xi), with padding supplied by the caller.
    pub s1_rows: Vec<F>,
    /// Shared P_j(xi), in coefficient order.
    pub s1_columns: Vec<F>,
    /// Live signature-major A_i(xi) W_ij(xi).
    pub weights_s2: Vec<F>,
    pub target: F,
}

struct Coordinates {
    rows: Vec<CoordinatesInAlpha>,
    columns: Vec<CoordinatesInAlpha>,
    s2: Vec<CoordinatesInAlpha>,
    alpha: Ext,
    target: Ext,
}

crate::piop::spartan::grinding::grinding_domains! {
    ProjectionGrinding => b"bitz/falcon-algebraic/factored-alpha-lift/projection-grinding/v1";
}

#[tracing::instrument(skip_all, name = "falcon_factored_ring:prove")]
pub(super) fn prove(
    transcript: &mut impl Transcript,
    layout: &Layout,
    public: &FalconAlgebraicStatement,
    data: &WitnessData,
    field: &Cfg,
    grinding_bits: u32,
) -> Result<(Proof, PreparedClaim), FalconError> {
    validate_context(layout, public, field, grinding_bits)?;
    validate_witness(layout, data)?;
    let coordinates = prepare_coordinates(transcript, layout, public, field, grinding_bits)?;
    let lift = calculate_lift(&coordinates, data);
    check_lift(&lift, layout.batch())?;
    check_read_off(&coordinates, &lift)?;
    absorb_lift(transcript, &lift);
    let projection_nonce = if grinding_bits == 0 {
        None
    } else {
        Some(
            grind_and_absorb(
                transcript,
                GrindingRound::<ProjectionGrinding>::new(0),
                grinding_bits,
            )
            .map_err(|e| error(e.to_string()))?,
        )
    };
    transcript.absorb_slice(b"coordinate-collapse");
    let xi = squeeze_field(transcript, field).map_err(|e| error(e.to_string()))?;
    let claim = collapse_claim(&coordinates, &lift, xi, field);
    Ok((
        Proof {
            lift,
            projection_nonce,
        },
        claim,
    ))
}

#[tracing::instrument(skip_all, name = "falcon_factored_ring:verify")]
pub(super) fn verify(
    transcript: &mut impl Transcript,
    layout: &Layout,
    public: &FalconAlgebraicStatement,
    proof: &Proof,
    field: &Cfg,
    grinding_bits: u32,
) -> Result<PreparedClaim, FalconError> {
    validate_context(layout, public, field, grinding_bits)?;
    if proof.projection_nonce.is_some() != (grinding_bits != 0) {
        return Err(error("invalid factored ring proof encoding"));
    }
    check_lift(&proof.lift, layout.batch())?;
    let coordinates = prepare_coordinates(transcript, layout, public, field, grinding_bits)?;
    check_read_off(&coordinates, &proof.lift)?;
    absorb_lift(transcript, &proof.lift);
    if let Some(nonce) = proof.projection_nonce {
        verify_and_absorb(
            transcript,
            GrindingRound::<ProjectionGrinding>::new(0),
            grinding_bits,
            nonce,
        )
        .map_err(|e| error(e.to_string()))?;
    }
    transcript.absorb_slice(b"coordinate-collapse");
    let xi = squeeze_field(transcript, field).map_err(|e| error(e.to_string()))?;
    Ok(collapse_claim(&coordinates, &proof.lift, xi, field))
}

fn validate_context(
    layout: &Layout,
    public: &FalconAlgebraicStatement,
    field: &Cfg,
    grinding_bits: u32,
) -> Result<(), FalconError> {
    if public.public_keys.len() != layout.batch() || public.targets.len() != layout.batch() {
        return Err(error("factored ring public input dimensions"));
    }
    if public
        .public_keys
        .iter()
        .chain(&public.targets)
        .flatten()
        .any(|&x| i64::from(x) >= Q)
    {
        return Err(error("factored ring public coefficient is not canonical"));
    }
    if field.modulus_u128() <= no_wrap_bound(layout.batch()) {
        return Err(error("factored ring integer lift exceeds the prime"));
    }
    if grinding_bits > 256 {
        return Err(error("invalid factored ring grinding difficulty"));
    }
    Ok(())
}

fn validate_witness(layout: &Layout, data: &WitnessData) -> Result<(), FalconError> {
    if data.witness.s1.len() != layout.batch() || data.witness.s2.len() != layout.batch() {
        return Err(error("factored ring witness dimensions"));
    }
    if data
        .witness
        .s1
        .iter()
        .chain(&data.witness.s2)
        .flatten()
        .any(|&s| !(-(1 << 14)..(1 << 14)).contains(&i32::from(s)))
    {
        return Err(error(
            "factored ring witness exceeds the signed 15-bit range",
        ));
    }
    Ok(())
}

fn prepare_coordinates(
    transcript: &mut impl Transcript,
    layout: &Layout,
    public: &FalconAlgebraicStatement,
    field: &Cfg,
    grinding_bits: u32,
) -> Result<Coordinates, FalconError> {
    transcript.absorb_slice(DOMAIN);
    transcript.absorb_slice(&(Q as u64).to_le_bytes());
    // The fixed extension encoding; the integer lift itself uses alpha's basis.
    for coefficient in [14u16, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1] {
        transcript.absorb_slice(&coefficient.to_le_bytes());
    }
    transcript.absorb_slice(&(N as u64).to_le_bytes());
    transcript.absorb_slice(&(layout.batch() as u64).to_le_bytes());
    transcript.absorb_slice(&(layout.capacity() as u64).to_le_bytes());
    transcript.absorb_slice(&field.modulus_u128().to_le_bytes());
    transcript.absorb_slice(&grinding_bits.to_le_bytes());
    transcript.absorb_slice(&lift_bound(layout.batch()).to_le_bytes());
    transcript.absorb_slice(b"instance-batching");
    let point = (0..layout.capacity().ilog2())
        .map(|_| sample_extension(transcript, false))
        .collect::<Result<Vec<_>, _>>()?;
    let lambda = equality_weights(&point);
    transcript.absorb_slice(b"polynomial-evaluation");
    let alpha = sample_extension(transcript, true)?;
    coordinates(public, &lambda, alpha)
}

fn sample_extension(transcript: &mut impl Transcript, non_base: bool) -> Result<Ext, FalconError> {
    transcript.begin_sampling();
    for _ in 0..128 {
        let coefficients = sample_coordinates::<EXTENSION_DEGREE>(transcript)
            .ok_or_else(|| error("factored ring field sampling exhausted"))?;
        if !non_base || coefficients[1..].iter().any(|&x| x != 0) {
            let value = Ext::new(coefficients);
            absorb_extensions(transcript, &[value]);
            return Ok(value);
        }
    }
    Err(error("factored ring non-base sampling exhausted"))
}

#[tracing::instrument(skip_all, name = "falcon_factored_ring:coordinates")]
fn coordinates(
    public: &FalconAlgebraicStatement,
    lambda: &[Ext],
    alpha: Ext,
) -> Result<Coordinates, FalconError> {
    let basis = PowerBasis::try_new(alpha)
        .map_err(|e| error(format!("invalid factored ring power basis: {e:?}")))?;
    let powers: Box<[Ext; N + 1]> = crate::piop::spartan::falcon_extension::powers(alpha);
    let batch = public.public_keys.len();
    let rows = lambda[..batch]
        .iter()
        .map(|&value| basis.encode(value))
        .collect();
    let mut columns = vec![CoordinatesInAlpha::ZERO; N];
    basis.fill_powers(basis.encode(Ext::ONE), &mut columns);
    let subtract = basis.encode(powers[N].add(Ext::ONE));
    let evaluator = PreparedPublicDotProducts::new(&powers[..N]);
    let mut s2 = vec![CoordinatesInAlpha::ZERO; batch * N];
    let mut targets = vec![Ext::ZERO; batch];
    let workers = rayon::current_num_threads();
    let simd = cfg!(all(
        target_arch = "x86_64",
        target_feature = "avx512f",
        target_feature = "avx512bw"
    ));
    let group_size = if simd && batch / 8 >= workers {
        8
    } else if simd && batch / 2 >= workers {
        2
    } else {
        1
    };
    s2.par_chunks_mut(group_size * N)
        .zip(targets.par_chunks_mut(group_size))
        .enumerate()
        .for_each(|(chunk, (out, targets))| {
            let first = chunk * group_size;
            let mut starts = [CoordinatesInAlpha::ZERO; 8];
            for (local, target) in targets.iter_mut().enumerate() {
                let i = first + local;
                let [h, t] = evaluator.evaluate_pair(&public.public_keys[i], &public.targets[i]);
                starts[local] = basis.encode(h);
                *target = lambda[i].mul(t);
            }
            if targets.len() == 8 {
                let mut outputs = out.chunks_mut(N);
                basis.fill_adjoint_pairs::<4>(
                    std::array::from_fn(|pair| [starts[2 * pair], starts[2 * pair + 1]]),
                    subtract,
                    std::array::from_fn(|pair| {
                        std::array::from_fn(|lane| {
                            public.public_keys[first + 2 * pair + lane].as_slice()
                        })
                    }),
                    std::array::from_fn(|_| [outputs.next().unwrap(), outputs.next().unwrap()]),
                );
                return;
            }
            for (pair, out) in out.chunks_mut(2 * N).enumerate() {
                let local = 2 * pair;
                let i = first + local;
                if out.len() == 2 * N {
                    let (a, b) = out.split_at_mut(N);
                    basis.fill_adjoint_pairs::<1>(
                        [[starts[local], starts[local + 1]]],
                        subtract,
                        [[&public.public_keys[i], &public.public_keys[i + 1]]],
                        [[a, b]],
                    );
                    continue;
                }
                let mut value = starts[local];
                for (j, slot) in out.iter_mut().enumerate() {
                    *slot = value;
                    if j + 1 == N {
                        break;
                    }
                    basis.advance_sub_scaled(
                        &mut value,
                        public.public_keys[i][N - 1 - j],
                        subtract,
                    );
                }
            }
        });
    Ok(Coordinates {
        rows,
        columns,
        s2,
        alpha,
        target: targets.into_iter().fold(Ext::ZERO, Ext::add),
    })
}

#[tracing::instrument(skip_all, name = "falcon_factored_ring:integer_lift")]
fn calculate_lift(coordinates: &Coordinates, data: &WitnessData) -> [i128; LIFT_COEFFICIENTS] {
    (0..data.witness.s1.len())
        .into_par_iter()
        .map(|i| {
            // Accumulate U_i first: only one K-by-K convolution per signature.
            // Each U coefficient is below 2*N*2^14*(q-1) < 2^39.
            let mut inner = [0i64; EXTENSION_DEGREE];
            for j in 0..N {
                for (k, sum) in inner.iter_mut().enumerate() {
                    *sum += i64::from(data.witness.s1[i][j])
                        * i64::from(coordinates.columns[j].coordinates()[k]);
                    *sum += i64::from(data.witness.s2[i][j])
                        * i64::from(coordinates.s2[i * N + j].coordinates()[k]);
                }
            }
            let mut lift = [0i128; LIFT_COEFFICIENTS];
            for (j, &a) in coordinates.rows[i].coordinates().iter().enumerate() {
                for (k, &u) in inner.iter().enumerate() {
                    lift[j + k] += i128::from(a) * i128::from(u);
                }
            }
            lift
        })
        .reduce(
            || [0; LIFT_COEFFICIENTS],
            |mut a, b| {
                for (a, b) in a.iter_mut().zip(b) {
                    *a += b;
                }
                a
            },
        )
}

/// Each product coefficient has at most K coordinate products, each bounded by
/// (q-1)^2. Sum over two signed15 operands, N positions and B live instances.
/// Both the true and claimed lifts obey H, so p > 2H preserves any nonzero
/// integer difference when the degree-20 polynomial is projected into F_p.
pub(super) fn lift_bound(batch: usize) -> u128 {
    2 * batch as u128
        * N as u128
        * COEFFICIENT_ABS_BOUND
        * EXTENSION_DEGREE as u128
        * (Q as u128 - 1).pow(2)
}

pub(super) fn no_wrap_bound(batch: usize) -> u128 {
    2 * lift_bound(batch)
}

fn check_lift(lift: &[i128; LIFT_COEFFICIENTS], batch: usize) -> Result<(), FalconError> {
    if lift
        .iter()
        .any(|value| value.unsigned_abs() > lift_bound(batch))
    {
        return Err(error("factored ring integer lift is out of bounds"));
    }
    Ok(())
}

fn read_off(lift: &[i128; LIFT_COEFFICIENTS], alpha: Ext) -> Ext {
    // Horner evaluation in E at alpha, not reduction by the fixed theta modulus.
    lift.iter().rev().fold(Ext::ZERO, |value, &coefficient| {
        let mut constant = Ext::ZERO;
        constant.0[0] = coefficient.rem_euclid(i128::from(Q)) as u16;
        value.mul(alpha).add(constant)
    })
}

fn check_read_off(
    coordinates: &Coordinates,
    lift: &[i128; LIFT_COEFFICIENTS],
) -> Result<(), FalconError> {
    if read_off(lift, coordinates.alpha) != coordinates.target {
        return Err(error(
            "factored ring integer lift does not match the native relation",
        ));
    }
    Ok(())
}

fn absorb_lift(transcript: &mut impl Transcript, lift: &[i128; LIFT_COEFFICIENTS]) {
    transcript.absorb_slice(b"integer-alpha-polynomial-lift");
    let mut bytes = [0u8; 16 * LIFT_COEFFICIENTS];
    for (bytes, value) in bytes.chunks_exact_mut(16).zip(lift) {
        bytes.copy_from_slice(&value.to_le_bytes());
    }
    transcript.absorb_slice(&bytes);
}

#[tracing::instrument(skip_all, name = "falcon_factored_ring:collapse")]
fn collapse_claim(
    coordinates: &Coordinates,
    lift: &[i128; LIFT_COEFFICIENTS],
    xi: F,
    field: &Cfg,
) -> PreparedClaim {
    let mut powers = [field.one(); EXTENSION_DEGREE];
    for k in 1..EXTENSION_DEGREE {
        powers[k] = field.mul(&powers[k - 1], &xi);
    }
    let prepared =
        <Cfg as PreparedLinearCombination<u64>>::prepare_linear_combination(field, powers);
    let evaluate = |v: &CoordinatesInAlpha| {
        <Cfg as PreparedLinearCombination<u64>>::linear_combination(&prepared, |k| {
            u64::from(v.coordinates()[k])
        })
    };
    let s1_rows: Vec<_> = coordinates.rows.iter().map(evaluate).collect();
    let s1_columns = coordinates.columns.iter().map(evaluate).collect();
    let mut weights_s2 = vec![field.zero(); coordinates.s2.len()];
    weights_s2
        .par_chunks_mut(N)
        .enumerate()
        .for_each(|(i, weights)| {
            // A_i(xi) is folded into the prepared kernel once per signature.
            let weighted = powers.map(|power| field.mul(&s1_rows[i], &power));
            let prepared = <Cfg as PreparedLinearCombination<u64>>::prepare_linear_combination(
                field, weighted,
            );
            for (weight, value) in weights.iter_mut().zip(&coordinates.s2[i * N..(i + 1) * N]) {
                *weight =
                    <Cfg as PreparedLinearCombination<u64>>::linear_combination(&prepared, |k| {
                        u64::from(value.coordinates()[k])
                    });
            }
        });
    let target = lift.iter().rev().fold(field.zero(), |value, &coefficient| {
        field.add(
            &field.mul(&value, &xi),
            &F::from_with_cfg(coefficient, field),
        )
    });
    PreparedClaim {
        s1_rows,
        s1_columns,
        weights_s2,
        target,
    }
}

fn error(message: impl Into<String>) -> FalconError {
    FalconError::Piop(message.into())
}

#[cfg(test)]
mod tests {
    use super::super::FalconAlgebraicWitness;
    use super::*;
    use crate::transcript::Blake3Transcript;
    use field::Uint;

    fn field() -> Cfg {
        F::make_cfg(&Uint::from((1u128 << 127) - 1)).unwrap()
    }

    fn fixture(batch: usize) -> (Layout, FalconAlgebraicStatement, WitnessData) {
        let layout = Layout::new(batch).unwrap();
        let mut public = FalconAlgebraicStatement {
            public_keys: vec![[0; N]; batch],
            targets: vec![[0; N]; batch],
        };
        let mut witness = FalconAlgebraicWitness {
            s1: vec![[0; N]; batch],
            s2: vec![[0; N]; batch],
        };
        for i in 0..batch {
            public.public_keys[i][N - 1] = 1;
            witness.s1[i][0] = -2;
            witness.s2[i][1] = (i + 1) as i16;
            public.targets[i][0] = (Q - 3 - i as i64) as u16;
        }
        let data = WitnessData::new(&public, witness).unwrap();
        (layout, public, data)
    }

    fn schoolbook_target(h: &[u16; N], s1: &[i16; N], s2: &[i16; N]) -> [u16; N] {
        let mut result = s1.map(i64::from);
        for (j, &b) in s2.iter().enumerate() {
            for (k, &a) in h.iter().enumerate() {
                let product = i64::from(a) * i64::from(b);
                if j + k < N {
                    result[j + k] += product;
                } else {
                    result[j + k - N] -= product;
                }
            }
        }
        result.map(|value| value.rem_euclid(Q) as u16)
    }

    fn shifted_evaluation(h: &[u16; N], shift: usize, powers: &[Ext]) -> Ext {
        let mut sums = [0i64; EXTENSION_DEGREE];
        for (k, &coefficient) in h.iter().enumerate() {
            let index = k + shift;
            let sign = if index < N { 1 } else { -1 };
            for (sum, &coordinate) in sums.iter_mut().zip(&powers[index % N].0) {
                *sum += sign * i64::from(coefficient) * i64::from(coordinate);
            }
        }
        Ext::new(sums.map(|x| x.rem_euclid(Q) as u16))
    }

    fn dot(claim: &PreparedClaim, data: &WitnessData, field: &Cfg) -> F {
        let mut sum = field.zero();
        for i in 0..data.witness.s1.len() {
            for j in 0..N {
                let s1_weight = field.mul(&claim.s1_rows[i], &claim.s1_columns[j]);
                sum = field.add(
                    &sum,
                    &field.mul(
                        &s1_weight,
                        &F::from_with_cfg(i64::from(data.witness.s1[i][j]), field),
                    ),
                );
                sum = field.add(
                    &sum,
                    &field.mul(
                        &claim.weights_s2[i * N + j],
                        &F::from_with_cfg(i64::from(data.witness.s2[i][j]), field),
                    ),
                );
            }
        }
        sum
    }

    #[test]
    fn factored_lift_matches_independent_shifted_polynomials_and_schoolbook_relation() {
        let (_, mut public, mut data) = fixture(3);
        for i in 0..3 {
            public.public_keys[i] = std::array::from_fn(|j| ((i * 37 + j * 101) as i64 % Q) as u16);
            data.witness.s1[i] = std::array::from_fn(|j| (j as i16 % 17) - 8);
            data.witness.s2[i] = std::array::from_fn(|j| (j as i16 % 11) - 5);
            public.targets[i] = schoolbook_target(
                &public.public_keys[i],
                &data.witness.s1[i],
                &data.witness.s2[i],
            );
        }
        let alpha = Ext::new(std::array::from_fn(|k| (97 * k + 321) as u16));
        let basis = PowerBasis::try_new(alpha).unwrap();
        let powers: Box<[Ext; N + 1]> = crate::piop::spartan::falcon_extension::powers(alpha);
        let lambda: Vec<_> = (0..3)
            .map(|i| Ext::new(std::array::from_fn(|k| ((i + 1) * 123 + k * 17) as u16)))
            .collect();
        let actual = coordinates(&public, &lambda, alpha).unwrap();
        let mut expected = [0i128; LIFT_COEFFICIENTS];
        for i in 0..3 {
            let a = basis.encode(lambda[i]);
            assert_eq!(actual.rows[i], a);
            for j in 0..N {
                let p = basis.encode(powers[j]);
                let w = basis.encode(shifted_evaluation(
                    &public.public_keys[i],
                    j,
                    powers.as_slice(),
                ));
                assert_eq!(actual.columns[j], p);
                assert_eq!(actual.s2[i * N + j], w);
                // Independent direct convolution for every witness coefficient.
                for k in 0..EXTENSION_DEGREE {
                    for l in 0..EXTENSION_DEGREE {
                        expected[k + l] += i128::from(a.coordinates()[k])
                            * (i128::from(data.witness.s1[i][j]) * i128::from(p.coordinates()[l])
                                + i128::from(data.witness.s2[i][j])
                                    * i128::from(w.coordinates()[l]));
                    }
                }
            }
        }
        assert_eq!(calculate_lift(&actual, &data), expected);
        assert!(check_read_off(&actual, &expected).is_ok());
        for xi in [0, 1, 1234567] {
            let field = field();
            let claim = collapse_claim(
                &actual,
                &expected,
                F::from_with_cfg(xi as u64, &field),
                &field,
            );
            assert_eq!(dot(&claim, &data, &field), claim.target);
        }
        data.witness.s2[2][N - 1] += 1;
        assert!(check_read_off(&actual, &calculate_lift(&actual, &data)).is_err());
    }

    #[test]
    fn prove_verify_agree_for_padded_batches_and_projection_grinding() {
        let field = field();
        for (batch, bits) in [(1, 0), (3, 0), (8, 0), (1, 3)] {
            let (layout, public, data) = fixture(batch);
            let mut prover = Blake3Transcript::new();
            let (proof, claim) = prove(&mut prover, &layout, &public, &data, &field, bits).unwrap();
            let mut verifier = Blake3Transcript::new();
            let checked = verify(&mut verifier, &layout, &public, &proof, &field, bits).unwrap();
            assert_eq!(claim, checked);
            assert_eq!(dot(&claim, &data, &field), claim.target);
            assert_eq!(claim.s1_rows.len(), batch);
            assert_eq!(claim.s1_columns.len(), N);
            assert_eq!(claim.weights_s2.len(), batch * N);
            assert_eq!(proof.payload_size_bytes(), 336 + usize::from(bits != 0) * 8);
            assert_eq!(
                prover.get_challenge::<u64>(),
                verifier.get_challenge::<u64>()
            );
        }
    }

    #[test]
    fn read_off_uses_alpha_basis_and_binding_rejects_extension_kernel_alterations() {
        let (layout, public, data) = fixture(3);
        let field = field();
        let (proof, _) = prove(
            &mut Blake3Transcript::new(),
            &layout,
            &public,
            &data,
            &field,
            0,
        )
        .unwrap();
        let coordinates =
            prepare_coordinates(&mut Blake3Transcript::new(), &layout, &public, &field, 0).unwrap();
        let basis = PowerBasis::try_new(coordinates.alpha).unwrap();
        let alpha_k = (0..EXTENSION_DEGREE).fold(Ext::ONE, |x, _| x.mul(coordinates.alpha));
        let feedback = basis.encode(alpha_k);
        let mut kernel = [0i128; LIFT_COEFFICIENTS];
        for (coefficient, &feedback) in kernel.iter_mut().zip(feedback.coordinates()) {
            *coefficient = -i128::from(feedback);
        }
        kernel[EXTENSION_DEGREE] = 1;
        assert_eq!(read_off(&kernel, coordinates.alpha), Ext::ZERO);
        let theta = Ext::new(std::array::from_fn(|k| u16::from(k == 1)));
        assert_ne!(
            read_off(&kernel, theta),
            Ext::ZERO,
            "fixed-basis read-off would be wrong"
        );
        let mut changed = proof.clone();
        for (coefficient, delta) in changed.lift.iter_mut().zip(kernel) {
            *coefficient += delta;
        }
        assert!(check_read_off(&coordinates, &changed.lift).is_ok());
        // Read-off alone cannot reject a multiple of alpha's minimal polynomial.
        // Its integer identity must still be checked by the authenticated binder.
        let claim = verify(
            &mut Blake3Transcript::new(),
            &layout,
            &public,
            &changed,
            &field,
            0,
        )
        .unwrap();
        assert_ne!(dot(&claim, &data, &field), claim.target);
        changed = proof;
        changed.lift[0] += i128::from(Q);
        let claim = verify(
            &mut Blake3Transcript::new(),
            &layout,
            &public,
            &changed,
            &field,
            0,
        )
        .unwrap();
        assert_ne!(dot(&claim, &data, &field), claim.target);
    }

    #[test]
    fn invalid_relation_and_malformed_lifts_are_rejected() {
        let (layout, public, mut data) = fixture(1);
        let field = field();
        let (proof, _) = prove(
            &mut Blake3Transcript::new(),
            &layout,
            &public,
            &data,
            &field,
            0,
        )
        .unwrap();
        for invalid in [
            i128::MIN,
            lift_bound(1) as i128 + 1,
            -(lift_bound(1) as i128) - 1,
        ] {
            let mut altered = proof.clone();
            altered.lift[0] = invalid;
            assert!(
                verify(
                    &mut Blake3Transcript::new(),
                    &layout,
                    &public,
                    &altered,
                    &field,
                    0
                )
                .is_err()
            );
        }
        let mut altered = proof.clone();
        altered.lift[0] += 1;
        assert!(
            verify(
                &mut Blake3Transcript::new(),
                &layout,
                &public,
                &altered,
                &field,
                0
            )
            .is_err()
        );
        altered = proof.clone();
        altered.projection_nonce = Some(0);
        assert!(
            verify(
                &mut Blake3Transcript::new(),
                &layout,
                &public,
                &altered,
                &field,
                0
            )
            .is_err()
        );
        assert!(
            verify(
                &mut Blake3Transcript::new(),
                &layout,
                &public,
                &proof,
                &field,
                1
            )
            .is_err()
        );
        let small = field::FpCtx::from_prime_u128(12289);
        assert!(validate_context(&layout, &public, &small, 0).is_err());
        let mut changed_public = public.clone();
        changed_public.targets[0][0] -= 1;
        assert!(
            verify(
                &mut Blake3Transcript::new(),
                &layout,
                &changed_public,
                &proof,
                &field,
                0,
            )
            .is_err()
        );
        changed_public.public_keys[0][0] = Q as u16;
        assert!(validate_context(&layout, &changed_public, &field, 0).is_err());
        assert!(coordinates(&public, &[Ext::ONE], Ext::ONE).is_err());
        data.witness.s2[0][1] += 1;
        assert!(
            prove(
                &mut Blake3Transcript::new(),
                &layout,
                &public,
                &data,
                &field,
                0
            )
            .is_err()
        );
    }

    #[test]
    fn signed_encoding_boundaries_and_partial_parallel_batches_are_exact() {
        let field = field();
        for batch in [1, 3, 129] {
            let (layout, mut public, mut data) = fixture(batch);
            // A sparse public key permits an independent O(N) schoolbook target.
            for i in 0..batch {
                public.public_keys[i].fill(0);
                public.public_keys[i][0] = (Q - 1) as u16;
                data.witness.s1[i] = std::array::from_fn(|j| [-16384, -1, 0, 16383][(i + j) % 4]);
                data.witness.s2[i] = std::array::from_fn(|j| [16383, 0, -16384][(i + j) % 3]);
                public.targets[i] = std::array::from_fn(|j| {
                    (i64::from(data.witness.s1[i][j]) - i64::from(data.witness.s2[i][j]))
                        .rem_euclid(Q) as u16
                });
            }
            let mut expected = None;
            for workers in [1, 4] {
                let pool = rayon::ThreadPoolBuilder::new()
                    .num_threads(workers)
                    .build()
                    .unwrap();
                let (proof, claim) = pool
                    .install(|| {
                        prove(
                            &mut Blake3Transcript::new(),
                            &layout,
                            &public,
                            &data,
                            &field,
                            0,
                        )
                    })
                    .unwrap();
                assert_eq!(dot(&claim, &data, &field), claim.target);
                if let Some((old_proof, old_claim)) = &expected {
                    assert_eq!(&proof, old_proof);
                    assert_eq!(&claim, old_claim);
                } else {
                    expected = Some((proof, claim));
                }
            }
            assert_eq!(no_wrap_bound(batch), 2 * lift_bound(batch));
            assert!(check_lift(&[lift_bound(batch) as i128; LIFT_COEFFICIENTS], batch).is_ok());
            assert!(check_lift(&[-(lift_bound(batch) as i128); LIFT_COEFFICIENTS], batch).is_ok());
            for invalid in [-16385, 16384] {
                data.witness.s1[0][0] = invalid;
                assert!(validate_witness(&layout, &data).is_err());
            }
        }
    }
}
