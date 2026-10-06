//! Prime-field source evaluation reduced through the BitZ PCS's product GKR.
//!
//! Native and 128-bit shared proofs use two bounded limbs with an unmultiplied
//! limb coordinate. 100-bit shared proofs use one unsplit exponent per row.
//! Both terminal claims are authenticated by the shared binary PCS.
use super::hybrid_keccak::grinding::{
    ProverBlockGrindingTranscript, VerifierBlockGrindingTranscript,
};
use super::{FalconError, FalconSourceWitness};
use crate::{
    bitz::{
        FixedBasePow, Shape, WINDOW,
        column_sums::{ColumnSums, FoldValue, LargeNumber},
        fold::fold_columns,
        forest::Forest,
        gkr::{GkrProverTranscript, GkrVerifierTranscript, gpgkr_verify},
    },
    hybrid::BinaryClaim,
    ligerito::pack_columns_from_rows,
    pcs::IntegerMatrixLayout,
    piop::spartan::{SpartanBitzField, SpartanField, grinding::GrindingDomain, matrix::eq_table},
    poly::{univariate::binary_gf128::Gf128 as Gf, utils::build_eq_x_r_vec},
    transcript::traits::Transcript,
};
use field::Uint;
#[cfg(feature = "parallel")]
use rayon::prelude::*;

type F = SpartanBitzField;
type Cfg = <F as SpartanField>::Config;

/// Selected by the prepared protocol and security target, never by proof data.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum BridgeMode {
    Unsplit,
    TwoLimbs,
}

/// One arity-2 forest: root projection degree s, degree-three
/// sumcheck rounds, and one linear child fold per layer. See the bridge audit.
pub(super) fn error_numerator(layout: &super::FalconSourceLayout, mode: BridgeMode) -> usize {
    let p = layout.bitz_params();
    let d = p.row_vars;
    let s = p.col_vars + usize::from(mode == BridgeMode::TwoLimbs);
    s + 3 * (d * (d - 1) / 2 + d * s) + d
}

fn message_count(p: &IntegerMatrixLayout) -> usize {
    p.row_vars * (p.row_vars - 1) / 2 + p.row_vars * (p.col_vars + 1) + p.row_vars
}

/// Each bit-column fold is strictly below 2^126, so its exponent remains
/// injective in the multiplicative group of GF(2^128).
fn limb_width(p: &IntegerMatrixLayout) -> usize {
    126 - p.row_vars
}

/// Conservative preparation gate for every modulus in the selected family.
fn split_bounds(p: &IntegerMatrixLayout, modulus: u128) -> Result<LargeNumber, FalconError> {
    let width = 126usize
        .checked_sub(p.row_vars)
        .filter(|&w| w > 0 && w < 128)
        .ok_or(err("bridge limb width"))?;
    let q_bits = 128 - modulus.leading_zeros() as usize;
    if modulus < 2 || q_bits.div_ceil(width) != 2 {
        return Err(err("bridge needs two bounded prime limbs"));
    }
    let lower = ((1u128 << width) - 1)
        .checked_mul(p.rows() as u128)
        .ok_or(err("bridge sum bound"))?;
    let upper = ((modulus - 1) >> width)
        .checked_mul(p.rows() as u128)
        .ok_or(err("bridge sum bound"))?;
    LargeNumber::checked_new(lower, upper).ok_or(err("bridge upper sum exceeds u32"))
}

pub(super) fn validate_layout(
    p: &IntegerMatrixLayout,
    mode: BridgeMode,
    modulus: u128,
) -> Result<(), FalconError> {
    match mode {
        BridgeMode::Unsplit => unsplit_shape(p, modulus).map(|_| ()),
        BridgeMode::TwoLimbs => split_bounds(p, modulus).map(|_| ()),
    }
}

fn weight_limbs(weights: &[u128], width: usize) -> Result<Vec<LargeNumber>, FalconError> {
    let mask = (1u128 << width) - 1;
    weights
        .iter()
        .map(|&w| {
            LargeNumber::checked_new(w & mask, w >> width)
                .ok_or(err("bridge upper weight exceeds u32"))
        })
        .collect()
}

fn checked_split_weight_sum(weights: &[LargeNumber]) -> Result<LargeNumber, FalconError> {
    let bound = weights
        .iter()
        .try_fold(LargeNumber::ZERO, |a, &b| a.checked_add(b))
        .ok_or(err("bridge sum bound"))?;
    if bound.lower == u128::MAX {
        return Err(err("bridge sum bound"));
    }
    Ok(bound)
}

fn binary_claim(p: &IntegerMatrixLayout, images: &[Gf], point: &[Gf], value: Gf) -> BinaryClaim {
    // The lowest geometric row bit is the unmultiplied limb coordinate.
    // BitZ returns [column | limb | original row]; Falcon uses [row | column].
    let (column_point, tail) = point.split_at(p.col_vars);
    let (limb, row_point) = tail.split_first().expect("limb coordinate");
    let low = build_eq_x_r_vec(row_point, &())
        .expect("nonempty row point")
        .into_iter()
        .zip(images.chunks_exact(2))
        .map(|(weight, pair)| {
            let factor =
                (Gf::one() + *limb) * (pair[0] + Gf::one()) + *limb * (pair[1] + Gf::one());
            weight * factor
        })
        .collect();
    BinaryClaim {
        low,
        high_point: column_point.to_vec(),
        value: value + Gf::one(),
    }
}

struct BridgeGrinding;
impl GrindingDomain for BridgeGrinding {
    const DOMAIN: &'static [u8] = b"bitz/falcon-hybrid/bridge-grinding/v4";
}

#[derive(Clone, Debug)]
pub(super) struct Proof {
    pub sums: ColumnSums,
    pub forest: Vec<[Gf; 2]>,
    pub nonces: Vec<u64>,
}

fn absorb_pair(t: &mut impl Transcript, pair: &[Gf; 2]) {
    let mut bytes = [0u8; 32];
    bytes[..16].copy_from_slice(&pair[0].to_bytes());
    bytes[16..].copy_from_slice(&pair[1].to_bytes());
    t.absorb_slice(&bytes);
}

/// Keep BitZ's optimized arithmetic and variable order while drawing every
/// challenge through Falcon's domain-separated, per-block grinding schedule.
struct ForestProver<'a, T> {
    transcript: &'a mut T,
    messages: Vec<[Gf; 2]>,
}
impl<T: Transcript> GkrProverTranscript for ForestProver<'_, T> {
    fn write_pair(&mut self, pair: [Gf; 2]) {
        absorb_pair(self.transcript, &pair);
        self.messages.push(pair);
    }
    fn challenge(&mut self) -> Gf {
        self.transcript.get_field_challenge(&())
    }
}
struct ForestVerifier<'a, T> {
    transcript: &'a mut T,
    messages: &'a [[Gf; 2]],
    cursor: usize,
}
impl<T: Transcript> GkrVerifierTranscript for ForestVerifier<'_, T> {
    fn read_pair(&mut self) -> Option<[Gf; 2]> {
        let pair = *self.messages.get(self.cursor)?;
        self.cursor += 1;
        absorb_pair(self.transcript, &pair);
        Some(pair)
    }
    fn challenge(&mut self) -> Option<Gf> {
        Some(self.transcript.get_field_challenge(&()))
    }
}

fn err(message: &'static str) -> FalconError {
    FalconError::Piop(message.into())
}

fn weights(point: &[F], field: &Cfg) -> Vec<u128> {
    eq_table(point, field)
        .expect("validated bridge point")
        .into_iter()
        .map(|x| u128::from(field.to_integer(&x)))
        .collect()
}

fn bind(t: &mut impl Transcript, sums: &[LargeNumber]) {
    t.absorb_slice(b"bitz/falcon-hybrid/integer-folds/v2");
    t.absorb_slice(&2u64.to_le_bytes());
    // Preserve the original limb-major, 16-byte transcript representation.
    // The compact column-major transport encoding is deliberately separate.
    for upper in [false, true] {
        t.absorb_slice(&(sums.len() as u64).to_le_bytes());
        for sum in sums {
            let value = if upper {
                u128::from(sum.upper)
            } else {
                sum.lower
            };
            t.absorb_slice(&value.to_le_bytes());
        }
    }
}

fn root_point(t: &mut impl Transcript, p: &IntegerMatrixLayout) -> Vec<Gf> {
    // Roots are indexed [column | limb]. This frame starts the initial
    // grinding block after both sets of integer sums have been bound.
    t.absorb_slice(b"bitz/falcon-hybrid/wfbitz-joint-limbs/v1");
    t.absorb_slice(&(p.row_vars as u64).to_le_bytes());
    t.absorb_slice(&(p.col_vars as u64).to_le_bytes());
    t.get_field_challenges(p.col_vars + 1, &())
}

fn row_images(chunks: &[LargeNumber], comb: &FixedBasePow) -> Vec<Gf> {
    chunks
        .iter()
        .flat_map(|w| [comb.pow(w.lower), comb.pow(u128::from(w.upper))])
        .collect()
}

pub(super) fn prove(
    t: &mut impl Transcript,
    source: &FalconSourceWitness,
    mode: BridgeMode,
    point: &[F],
    modulus: u128,
    grinding_bits: u32,
    row_weights: &[u128],
) -> Result<(Proof, Vec<BinaryClaim>), FalconError> {
    if mode == BridgeMode::Unsplit {
        return prove_unsplit(t, source, point, modulus, grinding_bits, row_weights);
    }
    let p = source.layout().bitz_params();
    if point.len() != p.row_vars + p.col_vars {
        return Err(err("bridge point shape"));
    }
    F::make_cfg(&Uint::from(modulus)).map_err(|_| err("bridge modulus"))?;
    if row_weights.len() != p.rows() || row_weights.iter().any(|&weight| weight >= modulus) {
        return Err(err("bridge row weights"));
    }
    split_bounds(&p, modulus)?;
    let chunks = weight_limbs(row_weights, limb_width(&p))?;
    checked_split_weight_sum(&chunks)?;
    let fold_span = tracing::info_span!("falcon_bridge:integer_folds").entered();
    let shape = Shape::new(p.row_vars, p.col_vars).map_err(|_| err("bridge fold shape"))?;
    let sums = fold_columns(&shape, source.rows(), &chunks);
    drop(fold_span);
    bind(t, &sums);
    let packing_span = tracing::info_span!("falcon_bridge:column_packing").entered();
    let mut packed_cols = pack_columns_from_rows(&p, source.rows());
    // Interleave a repeated source bit at row 2*r+limb. Highest-row-bit
    // products reduce only original rows; after d layers the limb survives.
    crate::utils::cfg_iter_mut!(&mut packed_cols).for_each(|column| {
        let rows = column.len();
        column.resize(2 * rows, 0);
        for r in (0..rows).rev() {
            let word = column[r];
            column[2 * r] = word;
            column[2 * r + 1] = word;
        }
    });
    drop(packing_span);
    let comb = FixedBasePow::new(crate::pcs::smallest_generator(), 128, WINDOW);
    let power_span = tracing::info_span!("falcon_bridge:power_tables").entered();
    let images = row_images(&chunks, &comb);
    drop(power_span);
    let mut grinder = ProverBlockGrindingTranscript::<_, BridgeGrinding>::new(t, grinding_bits);
    let zeta = root_point(&mut grinder, &p);
    let forest_span = tracing::info_span!("falcon_bridge:wfbitz_forest").entered();
    let mut state = ForestProver {
        transcript: &mut grinder,
        messages: Vec::with_capacity(message_count(&p)),
    };
    let (point, value) = Forest::new(p.row_vars + 1, p.col_vars, &packed_cols, &images)
        .prove_depth(&mut state, &zeta, p.row_vars);
    let forest = state.messages;
    drop(forest_span);
    let claim = binary_claim(&p, &images, &point, value);
    let nonces = grinder.finish();
    Ok((
        Proof {
            sums: ColumnSums::Split(sums),
            forest,
            nonces,
        },
        vec![claim],
    ))
}

pub(super) fn verify(
    t: &mut impl Transcript,
    layout: &super::FalconSourceLayout,
    mode: BridgeMode,
    point: &[F],
    value: F,
    modulus: u128,
    proof: &Proof,
    grinding_bits: u32,
) -> Result<Vec<BinaryClaim>, FalconError> {
    if mode == BridgeMode::Unsplit {
        return verify_unsplit(t, layout, point, value, modulus, proof, grinding_bits);
    }
    let p = layout.bitz_params();
    if point.len() != p.row_vars + p.col_vars {
        return Err(err("bridge point shape"));
    }
    if proof.forest.len() != message_count(&p) {
        return Err(err("bridge forest message count"));
    }
    let field = F::make_cfg(&Uint::from(modulus)).map_err(|_| err("bridge modulus"))?;
    split_bounds(&p, modulus)?;
    let width = limb_width(&p);
    let chunks = weight_limbs(&weights(&point[..p.row_vars], &field), width)?;
    let ColumnSums::Split(sums) = &proof.sums else {
        return Err(err("bridge chunk shape"));
    };
    let bound = checked_split_weight_sum(&chunks)?;
    if sums.len() != p.cols() || sums.iter().any(|&sum| !sum.within(bound)) {
        return Err(err("bridge integer fold magnitude"));
    }
    let cols = weights(&point[p.row_vars..], &field);
    let partial = sums
        .iter()
        .zip(&cols)
        .fold([0, 0], |[low, high], (sum, &c)| {
            [
                field.add_u128(low, field.mul_u128(field.reduce_u128(sum.lower), c)),
                field.add_u128(
                    high,
                    field.mul_u128(field.reduce_u128(u128::from(sum.upper)), c),
                ),
            ]
        });
    let result = field.add_u128(partial[0], field.mul_u128(1u128 << width, partial[1]));
    if result != u128::from(field.to_integer(&value)) {
        return Err(err("bridge prime read-off"));
    }
    bind(t, sums);
    let comb = FixedBasePow::new(crate::pcs::smallest_generator(), 128, WINDOW);
    let mut grinder =
        VerifierBlockGrindingTranscript::<_, BridgeGrinding>::new(t, grinding_bits, &proof.nonces);
    let zeta = root_point(&mut grinder, &p);
    let eq = build_eq_x_r_vec(&zeta, &()).expect("nonempty column and limb point");
    let root_claim = sums
        .iter()
        .map(|sum| sum.lower)
        .chain(sums.iter().map(|sum| u128::from(sum.upper)))
        .zip(eq)
        .fold(Gf::zero(), |acc, (sum, weight)| {
            acc + weight * comb.pow(sum)
        });
    let mut state = ForestVerifier {
        transcript: &mut grinder,
        messages: &proof.forest,
        cursor: 0,
    };
    let (point, value) = gpgkr_verify(&mut state, root_claim, &zeta, p.row_vars as u32)
        .ok_or(err("bridge forest"))?;
    if state.cursor != state.messages.len() {
        return Err(err("bridge forest trailing messages"));
    }
    let images = row_images(&chunks, &comb);
    let claim = binary_claim(&p, &images, &point, value);
    grinder.finish().map_err(|_| err("bridge grinding"))?;
    Ok(vec![claim])
}

struct UnsplitBridgeGrinding;
impl GrindingDomain for UnsplitBridgeGrinding {
    const DOMAIN: &'static [u8] = b"bitz/falcon-hybrid/shared-prime/unsplit-bridge-grinding/v2";
}

fn unsplit_message_count(p: &IntegerMatrixLayout) -> usize {
    p.row_vars * (p.row_vars - 1) / 2 + p.row_vars * p.col_vars + p.row_vars
}

fn unsplit_shape(p: &IntegerMatrixLayout, modulus: u128) -> Result<Shape, FalconError> {
    let shape = Shape::new(p.row_vars, p.col_vars).map_err(|_| err("bridge fold shape"))?;
    // Retain BitZ's conservative (R + 1)(p - 1) < 2^128 - 1 gate. The
    // shared prime family must satisfy this at its upper endpoint as well.
    if !shape.supports_modulus_bound(modulus) {
        return Err(err("unsplit bridge modulus bound"));
    }
    Ok(shape)
}

fn checked_weight_sum(row_weights: &[u128]) -> Result<u128, FalconError> {
    let bound = row_weights
        .iter()
        .try_fold(0u128, |sum, &weight| sum.checked_add(weight))
        .ok_or(err("bridge sum bound"))?;
    // The multiplicative group has order 2^128 - 1, not 2^128. Zero
    // and the group order must never be two allowed integer read-offs.
    if bound == u128::MAX {
        return Err(err("bridge sum bound"));
    }
    Ok(bound)
}

fn bind_unsplit(t: &mut impl Transcript, sums: &[u128]) {
    t.absorb_slice(b"bitz/falcon-hybrid/shared-prime/unsplit-integer-folds/v2");
    t.absorb_slice(&(sums.len() as u64).to_le_bytes());
    for sum in sums {
        t.absorb_slice(&sum.to_le_bytes());
    }
}

fn unsplit_root_point(t: &mut impl Transcript, p: &IntegerMatrixLayout) -> Vec<Gf> {
    t.absorb_slice(b"bitz/falcon-hybrid/shared-prime/wfbitz-unsplit/v2");
    t.absorb_slice(&(p.row_vars as u64).to_le_bytes());
    t.absorb_slice(&(p.col_vars as u64).to_le_bytes());
    t.get_field_challenges(p.col_vars, &())
}

fn unsplit_binary_claim(
    p: &IntegerMatrixLayout,
    images: &[Gf],
    point: &[Gf],
    value: Gf,
) -> Result<BinaryClaim, FalconError> {
    // BitZ returns [column | original row]. The leaf at (row, column)
    // is 1 + (images[row] + 1) * b[row, column], in characteristic two.
    if point.len() != p.col_vars + p.row_vars || images.len() != p.rows() {
        return Err(err("unsplit bridge terminal shape"));
    }
    let (column_point, row_point) = point.split_at(p.col_vars);
    let low = build_eq_x_r_vec(row_point, &())
        .map_err(|_| err("unsplit bridge terminal point"))?
        .into_iter()
        .zip(images)
        .map(|(weight, image)| weight * (*image + Gf::one()))
        .collect();
    Ok(BinaryClaim {
        low,
        high_point: column_point.to_vec(),
        value: value + Gf::one(),
    })
}

fn prove_unsplit(
    t: &mut impl Transcript,
    source: &FalconSourceWitness,
    point: &[F],
    modulus: u128,
    grinding_bits: u32,
    row_weights: &[u128],
) -> Result<(Proof, Vec<BinaryClaim>), FalconError> {
    let p = source.layout().bitz_params();
    let shape = unsplit_shape(&p, modulus)?;
    if point.len() != p.row_vars + p.col_vars {
        return Err(err("bridge point shape"));
    }
    F::make_cfg(&Uint::from(modulus)).map_err(|_| err("bridge modulus"))?;
    if row_weights.len() != p.rows() || row_weights.iter().any(|&weight| weight >= modulus) {
        return Err(err("bridge row weights"));
    }
    // All terms are nonnegative. Bounding their full sum also bounds every
    // partial sum in the digit-table and parallel folding implementations.
    checked_weight_sum(row_weights)?;
    let fold_span = tracing::info_span!("falcon_bridge:integer_folds").entered();
    let sums = fold_columns(&shape, source.rows(), row_weights);
    drop(fold_span);
    bind_unsplit(t, &sums);
    let packing_span = tracing::info_span!("falcon_bridge:column_packing").entered();
    let packed_cols = pack_columns_from_rows(&p, source.rows());
    drop(packing_span);
    let comb = FixedBasePow::new(crate::pcs::smallest_generator(), 128, WINDOW);
    let power_span = tracing::info_span!("falcon_bridge:power_tables").entered();
    let images: Vec<_> = row_weights.iter().map(|&weight| comb.pow(weight)).collect();
    drop(power_span);
    let mut grinder =
        ProverBlockGrindingTranscript::<_, UnsplitBridgeGrinding>::new(t, grinding_bits);
    let zeta = unsplit_root_point(&mut grinder, &p);
    let forest_span = tracing::info_span!("falcon_bridge:wfbitz_forest").entered();
    let mut state = ForestProver {
        transcript: &mut grinder,
        messages: Vec::with_capacity(unsplit_message_count(&p)),
    };
    let (point, value) = Forest::new(p.row_vars, p.col_vars, &packed_cols, &images)
        .prove_depth(&mut state, &zeta, p.row_vars);
    let forest = state.messages;
    drop(forest_span);
    let claim = unsplit_binary_claim(&p, &images, &point, value)?;
    let nonces = grinder.finish();
    Ok((
        Proof {
            sums: ColumnSums::Unsplit(sums),
            forest,
            nonces,
        },
        vec![claim],
    ))
}

fn verify_unsplit(
    t: &mut impl Transcript,
    layout: &super::FalconSourceLayout,
    point: &[F],
    value: F,
    modulus: u128,
    proof: &Proof,
    grinding_bits: u32,
) -> Result<Vec<BinaryClaim>, FalconError> {
    let p = layout.bitz_params();
    unsplit_shape(&p, modulus)?;
    if point.len() != p.row_vars + p.col_vars {
        return Err(err("bridge point shape"));
    }
    if proof.forest.len() != unsplit_message_count(&p) {
        return Err(err("bridge forest message count"));
    }
    let ColumnSums::Unsplit(sums) = &proof.sums else {
        return Err(err("unsplit bridge sum shape"));
    };
    if sums.len() != p.cols() {
        return Err(err("unsplit bridge sum shape"));
    }
    let field = F::make_cfg(&Uint::from(modulus)).map_err(|_| err("bridge modulus"))?;
    let row_weights = weights(&point[..p.row_vars], &field);
    let bound = checked_weight_sum(&row_weights)?;
    if sums.iter().any(|&sum| sum > bound) {
        return Err(err("bridge integer fold magnitude"));
    }
    let cols = weights(&point[p.row_vars..], &field);
    let result = sums.iter().zip(&cols).fold(0, |acc, (&sum, &weight)| {
        field.add_u128(acc, field.mul_u128(field.reduce_u128(sum), weight))
    });
    if result != u128::from(field.to_integer(&value)) {
        return Err(err("bridge prime read-off"));
    }
    bind_unsplit(t, sums);
    let comb = FixedBasePow::new(crate::pcs::smallest_generator(), 128, WINDOW);
    let mut grinder = VerifierBlockGrindingTranscript::<_, UnsplitBridgeGrinding>::new(
        t,
        grinding_bits,
        &proof.nonces,
    );
    let zeta = unsplit_root_point(&mut grinder, &p);
    let eq = build_eq_x_r_vec(&zeta, &()).expect("nonempty column point");
    let root_claim = sums.iter().zip(eq).fold(Gf::zero(), |acc, (&sum, weight)| {
        acc + weight * comb.pow(sum)
    });
    let mut state = ForestVerifier {
        transcript: &mut grinder,
        messages: &proof.forest,
        cursor: 0,
    };
    let (point, value) = gpgkr_verify(&mut state, root_claim, &zeta, p.row_vars as u32)
        .ok_or(err("bridge forest"))?;
    if state.cursor != state.messages.len() {
        return Err(err("bridge forest trailing messages"));
    }
    let images: Vec<_> = row_weights.iter().map(|&weight| comb.pow(weight)).collect();
    let claim = unsplit_binary_claim(&p, &images, &point, value)?;
    grinder.finish().map_err(|_| err("bridge grinding"))?;
    Ok(vec![claim])
}

#[cfg(test)]
mod tests {
    use super::super::{FalconSourceLayout, verification_trace};
    use super::*;
    use crate::transcript::Blake3Transcript;

    const UNSPLIT_PRIME_MAX: u128 = (1u128 << 115) - (1u128 << 102) - 1;

    fn unsplit_sums(proof: &mut Proof) -> &mut Vec<u128> {
        let ColumnSums::Unsplit(sums) = &mut proof.sums else {
            panic!("unsplit sums");
        };
        sums
    }

    fn split_sums(proof: &mut Proof) -> &mut Vec<LargeNumber> {
        let ColumnSums::Split(sums) = &mut proof.sums else {
            panic!("split sums");
        };
        sums
    }

    #[test]
    fn compact_sums_preserve_legacy_absorbed_bytes() {
        use crate::transcript::traits::ConstTranscribable;
        #[derive(Default)]
        struct Recording(Vec<Vec<u8>>);
        impl Transcript for Recording {
            fn get_challenge<T: ConstTranscribable>(&mut self) -> T {
                panic!("binding draws no challenges");
            }
            fn fill_sampling_bytes(&mut self, _: &mut [u8]) {
                panic!("binding draws no sampling bytes");
            }
            fn absorb_inner(&mut self, bytes: &[u8]) {
                self.0.push(bytes.to_vec());
            }
        }
        let lower = [0, 1, (1u128 << 113) + 7, (1u128 << 126) - 8192];
        let upper = [11u128, 0, 1, (1u128 << 26) - 8192];
        let sums: Vec<_> = lower
            .iter()
            .zip(upper)
            .map(|(&lo, hi)| LargeNumber::checked_new(lo, hi).unwrap())
            .collect();
        let mut actual = Recording::default();
        bind(&mut actual, &sums);
        let mut legacy = Recording::default();
        legacy.absorb_slice(b"bitz/falcon-hybrid/integer-folds/v2");
        legacy.absorb_slice(&2u64.to_le_bytes());
        for limb in [lower, upper] {
            legacy.absorb_slice(&4u64.to_le_bytes());
            for value in limb {
                legacy.absorb_slice(&value.to_le_bytes());
            }
        }
        assert_eq!(actual.0, legacy.0);
    }

    #[test]
    fn reject_layouts_whose_upper_sum_does_not_fit() {
        let mut p = FalconSourceLayout::new_shared_prime(1024)
            .unwrap()
            .bitz_params();
        assert_eq!(
            split_bounds(&p, (1u128 << 126) - 1).unwrap().upper,
            (1u32 << 26) - 8192
        );
        p.row_vars = 17;
        assert!(split_bounds(&p, (1u128 << 126) - 1).is_err());
        p.row_vars = 126;
        assert!(split_bounds(&p, (1u128 << 126) - 1).is_err());
    }

    #[test]
    fn unsplit_counts_and_conservative_modulus_gate() {
        for batch in 1..=1024 {
            let layout = FalconSourceLayout::new_shared_prime(batch).unwrap();
            let p = layout.bitz_params();
            let rounds: usize = (0..p.row_vars).map(|layer| layer + p.col_vars).sum();
            assert_eq!(unsplit_message_count(&p), rounds + p.row_vars);
            assert_eq!(
                error_numerator(&layout, BridgeMode::Unsplit),
                p.col_vars + 3 * rounds + p.row_vars
            );
            assert!((407..=807).contains(&error_numerator(&layout, BridgeMode::Unsplit)));
            assert_eq!(message_count(&p) - unsplit_message_count(&p), p.row_vars);
            assert!(unsplit_shape(&p, 1u128 << 114).is_ok());
            assert!(unsplit_shape(&p, UNSPLIT_PRIME_MAX).is_ok());
            // The top of the full 115-bit interval passes R(p-1) < M,
            // but not the stronger existing Shape gate (R+1)(p-1) < M.
            let too_large = (1u128 << 115) - 1;
            assert!((too_large - 1).checked_mul(p.rows() as u128).unwrap() < u128::MAX);
            assert!(unsplit_shape(&p, too_large).is_err());
        }
        assert!(checked_weight_sum(&[u128::MAX]).is_err());
        assert!(checked_weight_sum(&[u128::MAX - 1, 2]).is_err());
        assert_eq!(checked_weight_sum(&[0, 0]).unwrap(), 0);
        assert!(
            unsplit_shape(
                &IntegerMatrixLayout {
                    row_vars: 6,
                    col_vars: 4
                },
                3
            )
            .is_err()
        );
    }

    #[test]
    fn unsplit_full_width_folds_match_big_integer_reference() {
        use num_bigint::BigUint;

        let p = IntegerMatrixLayout {
            row_vars: 13,
            col_vars: 2,
        };
        let shape = unsplit_shape(&p, UNSPLIT_PRIME_MAX).unwrap();
        let weights: Vec<_> = (0..p.rows())
            .map(|row| UNSPLIT_PRIME_MAX - 1 - row as u128)
            .collect();
        let bound = checked_weight_sum(&weights).unwrap();
        assert!(bound > 1u128 << 127);
        let rows: Vec<Vec<u64>> = (0..p.cols())
            .map(|column| {
                (0..p.rows() / 64)
                    .map(|word| match column {
                        0 => u64::MAX,
                        1 => 0,
                        2 => 0xaaaa_aaaa_aaaa_aaaa,
                        _ => 1u64 << (word % 64),
                    })
                    .collect()
            })
            .collect();
        let folds = fold_columns(&shape, &rows, &weights);
        for (column, &fold) in rows.iter().zip(&folds) {
            let expected =
                weights
                    .iter()
                    .enumerate()
                    .fold(BigUint::from(0u8), |sum, (row, &weight)| {
                        if (column[row / 64] >> (row % 64)) & 1 == 1 {
                            sum + BigUint::from(weight)
                        } else {
                            sum
                        }
                    });
            assert_eq!(BigUint::from(fold), expected);
            assert!(fold <= bound);
        }
        assert_eq!(folds[0], bound);
        assert_eq!(folds[1], 0);
    }

    #[test]
    fn unsplit_terminal_coefficients_use_original_row_and_column_slots() {
        let p = IntegerMatrixLayout {
            row_vars: 3,
            col_vars: 2,
        };
        let point: Vec<_> = (0..p.row_vars + p.col_vars)
            .map(|i| Gf::from_polynomial_words([3 + 2 * i as u64, 1 + i as u64]))
            .collect();
        let images: Vec<_> = (0..p.rows())
            .map(|i| Gf::from_polynomial_words([19 + i as u64, 7 * i as u64]))
            .collect();
        let column_eq = build_eq_x_r_vec(&point[..p.col_vars], &()).unwrap();
        let row_eq = build_eq_x_r_vec(&point[p.col_vars..], &()).unwrap();
        // Check every one-bit witness independently; this catches transposed
        // coordinates and an accidentally retained limb-coordinate weight.
        for column in 0..p.cols() {
            for row in 0..p.rows() {
                let expected = column_eq[column] * row_eq[row] * (images[row] + Gf::one());
                let claim =
                    unsplit_binary_claim(&p, &images, &point, Gf::one() + expected).unwrap();
                assert_eq!(claim.low.len(), p.rows());
                assert_eq!(claim.high_point, point[..p.col_vars]);
                assert_eq!(column_eq[column] * claim.low[row], claim.value);
            }
        }
        assert!(unsplit_binary_claim(&p, &images[..p.rows() - 1], &point, Gf::zero()).is_err());
        assert!(unsplit_binary_claim(&p, &images, &point[..point.len() - 1], Gf::zero()).is_err());
        let mut extra_point = point.clone();
        extra_point.push(Gf::one());
        assert!(unsplit_binary_claim(&p, &images, &extra_point, Gf::zero()).is_err());
    }

    #[test]
    fn native_frames_remain_legacy_and_unsplit_domains_are_distinct() {
        use crate::piop::spartan::grinding::{GrindingRound, derive_grinding_seed};

        let p = FalconSourceLayout::new(1).unwrap().bitz_params();
        let sums = [[1u128, 17], [23, 31]];
        let packed = [
            LargeNumber {
                lower: 1,
                upper: 23,
            },
            LargeNumber {
                lower: 17,
                upper: 31,
            },
        ];
        let mut actual = Blake3Transcript::new();
        bind(&mut actual, &packed);
        let actual_point = root_point(&mut actual, &p);
        let mut expected = Blake3Transcript::new();
        expected.absorb_slice(b"bitz/falcon-hybrid/integer-folds/v2");
        expected.absorb_slice(&2u64.to_le_bytes());
        for column in &sums {
            expected.absorb_slice(&2u64.to_le_bytes());
            for sum in column {
                expected.absorb_slice(&sum.to_le_bytes());
            }
        }
        expected.absorb_slice(b"bitz/falcon-hybrid/wfbitz-joint-limbs/v1");
        expected.absorb_slice(&13u64.to_le_bytes());
        expected.absorb_slice(&4u64.to_le_bytes());
        assert_eq!(actual_point, expected.get_field_challenges::<Gf>(5, &()));
        assert_eq!(
            actual.get_challenge::<u128>(),
            expected.get_challenge::<u128>()
        );
        assert_eq!(
            BridgeGrinding::DOMAIN,
            b"bitz/falcon-hybrid/bridge-grinding/v4"
        );
        let native_seed = derive_grinding_seed(
            &mut Blake3Transcript::new(),
            GrindingRound::<BridgeGrinding>::new(0),
            2,
        )
        .unwrap();
        let shared_seed = derive_grinding_seed(
            &mut Blake3Transcript::new(),
            GrindingRound::<UnsplitBridgeGrinding>::new(0),
            2,
        )
        .unwrap();
        assert_ne!(native_seed, shared_seed);
        let mut shared = Blake3Transcript::new();
        bind_unsplit(&mut shared, &sums[0]);
        assert_eq!(unsplit_root_point(&mut shared, &p).len(), p.col_vars);
        assert_ne!(
            shared.get_challenge::<u128>(),
            actual.get_challenge::<u128>()
        );
    }

    #[test]
    fn unsplit_bridge_authenticates_source_and_rejects_malformed_proofs() {
        const PK: &[u8] = include_bytes!("fixtures/public_key.bin");
        const MSG: &[u8] = include_bytes!("fixtures/message.bin");
        const SIG: &[u8] = include_bytes!("fixtures/signature_ct.bin");
        let trace = verification_trace(PK, MSG, SIG).unwrap();
        let field = crate::prime_sampling::sample_prime_context(
            &mut Blake3Transcript::new(),
            1u128 << 114,
            UNSPLIT_PRIME_MAX,
            128,
        )
        .unwrap();
        let modulus = field.modulus_u128();
        for (batch, grinding_bits) in [(1, 0), (3, 2)] {
            let layout = FalconSourceLayout::new_shared_prime(batch).unwrap();
            let source = FalconSourceWitness::from_traces(
                layout.clone(),
                &vec![MSG; batch],
                &vec![SIG; batch],
                &vec![trace.clone(); batch],
            )
            .unwrap();
            let p = layout.bitz_params();
            let point: Vec<_> = (0..p.row_vars + p.col_vars)
                .map(|i| F::from_with_cfg(Uint::from(3 + 2 * i as u128), &field))
                .collect();
            let row_weights = weights(&point[..p.row_vars], &field);
            let column_weights = weights(&point[p.row_vars..], &field);
            let mut value = 0;
            for (column, bits) in source.rows().iter().enumerate() {
                for row in 0..p.rows() {
                    if (bits[row / 64] >> (row % 64)) & 1 == 1 {
                        value = field.add_u128(
                            value,
                            field.mul_u128(row_weights[row], column_weights[column]),
                        );
                    }
                }
            }
            let value = F::from_with_cfg(Uint::from(value), &field);
            let mut pt = Blake3Transcript::new();
            let (mut proof, claims) = prove(
                &mut pt,
                &source,
                BridgeMode::Unsplit,
                &point,
                modulus,
                grinding_bits,
                &row_weights,
            )
            .unwrap();
            assert!(matches!(&proof.sums, ColumnSums::Unsplit(sums) if sums.len() == p.cols()));
            proof.sums = ColumnSums::decode(
                &proof.sums.encode(),
                p.cols(),
                crate::bitz::column_sums::ColumnSumBounds::Unsplit(
                    checked_weight_sum(&row_weights).unwrap(),
                ),
            )
            .unwrap();
            assert_eq!(proof.forest.len(), unsplit_message_count(&p));
            assert_eq!(
                proof.nonces.len(),
                if grinding_bits == 0 {
                    0
                } else {
                    1 + unsplit_message_count(&p)
                }
            );
            let mut vt = Blake3Transcript::new();
            let verified = verify(
                &mut vt,
                &layout,
                BridgeMode::Unsplit,
                &point,
                value,
                modulus,
                &proof,
                grinding_bits,
            )
            .unwrap();
            assert_eq!(claims.len(), 1);
            assert_eq!(verified.len(), 1);
            assert_eq!(claims[0].low, verified[0].low);
            assert_eq!(claims[0].high_point, verified[0].high_point);
            assert_eq!(claims[0].value, verified[0].value);
            assert_eq!(pt.get_challenge::<u128>(), vt.get_challenge::<u128>());
            let column_eq = build_eq_x_r_vec(&claims[0].high_point, &()).unwrap();
            let mut binary_value = Gf::zero();
            for (column, bits) in source.rows().iter().enumerate() {
                for row in 0..p.rows() {
                    if (bits[row / 64] >> (row % 64)) & 1 == 1 {
                        binary_value += column_eq[column] * claims[0].low[row];
                    }
                }
            }
            assert_eq!(binary_value, claims[0].value);
            let reject = |changed: &Proof| {
                assert!(
                    verify(
                        &mut Blake3Transcript::new(),
                        &layout,
                        BridgeMode::Unsplit,
                        &point,
                        value,
                        modulus,
                        changed,
                        grinding_bits
                    )
                    .is_err()
                );
            };
            let mut changed = proof.clone();
            changed.sums = ColumnSums::Unsplit(vec![]);
            reject(&changed);
            let mut changed = proof.clone();
            changed.sums = ColumnSums::Split(vec![LargeNumber::ZERO; p.cols()]);
            reject(&changed);
            let mut changed = proof.clone();
            unsplit_sums(&mut changed).pop();
            reject(&changed);
            let mut changed = proof.clone();
            unsplit_sums(&mut changed)[0] = u128::MAX;
            reject(&changed);
            let mut changed = proof.clone();
            unsplit_sums(&mut changed)[0] = checked_weight_sum(&row_weights).unwrap() + 1;
            reject(&changed);
            let mut changed = proof.clone();
            unsplit_sums(&mut changed)[0] ^= 1;
            reject(&changed);
            // A whole prime can be subtracted without changing the prime
            // read-off. The authenticated exponent forest must still reject.
            let mut changed = proof.clone();
            let column = unsplit_sums(&mut changed)
                .iter()
                .position(|&sum| sum >= modulus)
                .unwrap();
            unsplit_sums(&mut changed)[column] -= modulus;
            reject(&changed);
            let mut changed = proof.clone();
            changed.forest[0][0] += Gf::one();
            reject(&changed);
            let mut changed = proof.clone();
            changed.forest.last_mut().unwrap()[1] += Gf::one();
            reject(&changed);
            let mut changed = proof.clone();
            changed.forest.pop();
            reject(&changed);
            let mut changed = proof.clone();
            changed.forest.push([Gf::zero(); 2]);
            reject(&changed);
            let mut changed = proof.clone();
            changed.nonces.push(0);
            reject(&changed);
            if grinding_bits != 0 {
                let mut changed = proof.clone();
                changed.nonces.pop();
                reject(&changed);
                let mut changed = proof.clone();
                changed.nonces[0] ^= 1;
                reject(&changed);
            }
            assert!(
                verify(
                    &mut Blake3Transcript::new(),
                    &layout,
                    BridgeMode::Unsplit,
                    &point,
                    value,
                    modulus,
                    &proof,
                    grinding_bits + 1
                )
                .is_err()
            );
            let native = FalconSourceLayout::new(batch).unwrap();
            for other_layout in [&layout, &native] {
                assert!(
                    verify(
                        &mut Blake3Transcript::new(),
                        other_layout,
                        BridgeMode::TwoLimbs,
                        &point,
                        value,
                        modulus,
                        &proof,
                        grinding_bits
                    )
                    .is_err()
                );
            }
            let mut bad_weights = row_weights.clone();
            bad_weights[0] = modulus;
            assert!(
                prove(
                    &mut Blake3Transcript::new(),
                    &source,
                    BridgeMode::Unsplit,
                    &point,
                    modulus,
                    grinding_bits,
                    &bad_weights
                )
                .is_err()
            );
            assert!(
                prove(
                    &mut Blake3Transcript::new(),
                    &source,
                    BridgeMode::Unsplit,
                    &point,
                    (1u128 << 115) - 1,
                    grinding_bits,
                    &row_weights
                )
                .is_err()
            );
        }
    }

    #[test]
    fn bitz_joint_limb_forest_fits_security_budget() {
        for batch in 1..=1024 {
            let layout = FalconSourceLayout::new(batch).unwrap();
            let p = layout.bitz_params();
            let d = p.row_vars;
            let s = p.col_vars + 1;
            assert_eq!(d, 13);
            assert_eq!(s, 5 + layout.capacity().ilog2() as usize);
            // Count the accepted verifier rounds independently of the formula.
            let sumcheck_rounds: usize = (0..d).map(|ell| ell + s).sum();
            assert_eq!(
                error_numerator(&layout, BridgeMode::TwoLimbs),
                s + 3 * sumcheck_rounds + d
            );
            assert!((447..=847).contains(&error_numerator(&layout, BridgeMode::TwoLimbs)));
            // Security geometry follows the selected mode, independently of
            // whether these source slots also contain the shared public key.
            let shared = FalconSourceLayout::new_shared_prime(batch).unwrap();
            for mode in [BridgeMode::Unsplit, BridgeMode::TwoLimbs] {
                assert_eq!(
                    error_numerator(&layout, mode),
                    error_numerator(&shared, mode)
                );
            }
            assert_eq!(limb_width(&p), 113);
            for prime_bits in [126, 127] {
                assert_eq!(usize::div_ceil(prime_bits, limb_width(&p)), 2);
            }
            // Every canonical bit column has exponent < 2^126, strictly
            // below the order 2^128-1 of the fixed multiplicative generator.
            let max_limb = (1u128 << limb_width(&p)) - 1;
            assert!(max_limb * (p.rows() as u128) < (1u128 << 126));
        }
        assert!(crate::pcs::is_generator(crate::pcs::smallest_generator()));
    }

    #[test]
    fn bridge_rejects_nonces_from_other_domains_and_difficulties() {
        use crate::piop::spartan::grinding::{
            GrindingRound, derive_grinding_seed, derive_grinding_seed_in_domain,
            grinding_nonce_is_valid, verify_and_absorb,
        };

        const BITS: u32 = 3;
        let mut initial = Blake3Transcript::new();
        initial.absorb_slice(b"fixed bridge prefix");
        let current = derive_grinding_seed(
            &mut initial.clone(),
            GrindingRound::<BridgeGrinding>::new(0),
            BITS,
        )
        .unwrap();
        let other_domain =
            derive_grinding_seed_in_domain(&mut initial.clone(), b"unrelated-test-domain", 0, BITS)
                .unwrap();
        let other_difficulty = derive_grinding_seed(
            &mut initial.clone(),
            GrindingRound::<BridgeGrinding>::new(0),
            BITS + 1,
        )
        .unwrap();
        assert_ne!(current, other_domain);
        assert_ne!(current, other_difficulty);
        for (other_seed, other_bits) in [(&other_domain, BITS), (&other_difficulty, BITS + 1)] {
            // Choose a nonce whose rejection is certain, independent of a
            // chance nonce collision across the two independent seeds.
            let other_nonce = (0..u64::MAX)
                .find(|&nonce| {
                    grinding_nonce_is_valid(other_seed, nonce, other_bits).unwrap()
                        && !grinding_nonce_is_valid(&current, nonce, BITS).unwrap()
                })
                .unwrap();
            assert!(
                verify_and_absorb(
                    &mut initial.clone(),
                    GrindingRound::<BridgeGrinding>::new(0),
                    BITS,
                    other_nonce,
                )
                .is_err()
            );
        }
    }

    #[test]
    fn batched_limbs_authenticate_source_and_reject_compensating_changes() {
        assert_batched_limbs_authenticate_source(
            FalconSourceLayout::new(1).unwrap(),
            (1u128 << 127) - 1,
        );
    }

    #[test]
    fn shared_128_bit_bridge_uses_two_limbs_and_authenticates_padded_source() {
        let field = crate::prime_sampling::sample_prime_context(
            &mut Blake3Transcript::new(),
            1u128 << 125,
            (1u128 << 126) - 1,
            128,
        )
        .unwrap();
        for batch in [1, 3] {
            assert_batched_limbs_authenticate_source(
                FalconSourceLayout::new_shared_prime(batch).unwrap(),
                field.modulus_u128(),
            );
        }
    }

    #[test]
    fn shared_126_bit_weights_have_bounded_113_bit_limbs() {
        let p = FalconSourceLayout::new_shared_prime(1024)
            .unwrap()
            .bitz_params();
        let width = limb_width(&p);
        assert_eq!(width, 113);
        // Cover the entire allowed 126-bit family, including its largest
        // possible weight; this check does not assume those bounds are prime.
        for modulus in [1u128 << 125, (1u128 << 126) - 1] {
            let original = vec![modulus - 1; p.rows()];
            let limbs = weight_limbs(&original, width).unwrap();
            assert_eq!(limbs.len(), p.rows());
            for row in 0..p.rows() {
                assert_eq!(
                    limbs[row].lower + (u128::from(limbs[row].upper) << width),
                    original[row]
                );
                assert!(limbs[row].lower < 1u128 << 113);
                assert!(limbs[row].upper < 1u32 << 13);
            }
            let bound = checked_split_weight_sum(&limbs).unwrap();
            assert!(bound.lower < 1u128 << 126);
            assert!(bound.upper < 1u32 << 26);
            assert!(bound.within(split_bounds(&p, modulus).unwrap()));
            assert!(unsplit_shape(&p, modulus).is_err());
        }
    }

    fn assert_batched_limbs_authenticate_source(layout: FalconSourceLayout, modulus: u128) {
        const PK: &[u8] = include_bytes!("fixtures/public_key.bin");
        const MSG: &[u8] = include_bytes!("fixtures/message.bin");
        const SIG: &[u8] = include_bytes!("fixtures/signature_ct.bin");
        let trace = verification_trace(PK, MSG, SIG).unwrap();
        let source = FalconSourceWitness::from_traces(
            layout,
            &vec![MSG; layout.batch()],
            &vec![SIG; layout.batch()],
            &vec![trace; layout.batch()],
        )
        .unwrap();
        let p = layout.bitz_params();
        let field = F::make_cfg(&Uint::from(modulus)).unwrap();
        let point: Vec<_> = (0..p.row_vars + p.col_vars)
            .map(|i| F::from_with_cfg(Uint::from(3 + 2 * i as u128), &field))
            .collect();
        let row_weights = weights(&point[..p.row_vars], &field);
        let column_weights = weights(&point[p.row_vars..], &field);
        // Independent prime-field evaluation over the original committed bits.
        let mut value = 0;
        for (c, row) in source.rows().iter().enumerate() {
            let mut column = 0;
            for r in 0..p.rows() {
                if (row[r >> 6] >> (r & 63)) & 1 == 1 {
                    column = field.add_u128(column, row_weights[r]);
                }
            }
            value = field.add_u128(value, field.mul_u128(column, column_weights[c]));
        }
        let value = F::from_with_cfg(Uint::from(value), &field);
        let mut pt = Blake3Transcript::new();
        let (mut proof, claims) = prove(
            &mut pt,
            &source,
            BridgeMode::TwoLimbs,
            &point,
            modulus,
            2,
            &row_weights,
        )
        .unwrap();
        assert!(matches!(&proof.sums, ColumnSums::Split(sums) if sums.len() == p.cols()));
        let split_weights = weight_limbs(&row_weights, limb_width(&p)).unwrap();
        let decoded = ColumnSums::decode(
            &proof.sums.encode(),
            p.cols(),
            crate::bitz::column_sums::ColumnSumBounds::Split(
                checked_split_weight_sum(&split_weights).unwrap(),
            ),
        )
        .unwrap();
        assert_eq!(decoded, proof.sums);
        proof.sums = decoded;
        assert_eq!(claims.len(), 1);
        let s = p.col_vars + 1;
        let rounds: usize = (0..p.row_vars).map(|ell| ell + s).sum();
        assert_eq!(proof.nonces.len(), 1 + rounds + p.row_vars);
        assert_eq!(proof.forest.len(), rounds + p.row_vars);
        let mut vt = Blake3Transcript::new();
        let verified = verify(
            &mut vt,
            &layout,
            BridgeMode::TwoLimbs,
            &point,
            value,
            modulus,
            &proof,
            2,
        )
        .unwrap();
        for (claim, verified) in claims.iter().zip(&verified) {
            assert_eq!(claim.low, verified.low);
            assert_eq!(claim.high_point, verified.high_point);
            assert_eq!(claim.value, verified.value);
            let column_eq = build_eq_x_r_vec(&claim.high_point, &()).unwrap();
            let mut binary_value = Gf::zero();
            for (c, row) in source.rows().iter().enumerate() {
                let mut column = Gf::zero();
                for r in 0..p.rows() {
                    if (row[r >> 6] >> (r & 63)) & 1 == 1 {
                        column += claim.low[r];
                    }
                }
                binary_value += column_eq[c] * column;
            }
            assert_eq!(binary_value, claim.value);
        }
        assert_eq!(pt.get_challenge::<u128>(), vt.get_challenge::<u128>());

        let reject = |changed: &Proof| {
            assert!(
                verify(
                    &mut Blake3Transcript::new(),
                    &layout,
                    BridgeMode::TwoLimbs,
                    &point,
                    value,
                    modulus,
                    changed,
                    2,
                )
                .is_err()
            );
        };
        // Container shape cannot silently select the unsplit protocol.
        let mut changed = proof.clone();
        changed.sums = ColumnSums::Unsplit(vec![0; p.cols()]);
        reject(&changed);
        let mut changed = proof.clone();
        changed.sums = ColumnSums::Split(vec![]);
        reject(&changed);
        let mut changed = proof.clone();
        split_sums(&mut changed).push(LargeNumber::ZERO);
        reject(&changed);
        let mut changed = proof.clone();
        split_sums(&mut changed).pop();
        reject(&changed);
        assert!(
            verify(
                &mut Blake3Transcript::new(),
                &layout,
                BridgeMode::Unsplit,
                &point,
                value,
                modulus,
                &proof,
                2,
            )
            .is_err()
        );
        assert!(
            prove(
                &mut Blake3Transcript::new(),
                &source,
                BridgeMode::Unsplit,
                &point,
                modulus,
                2,
                &row_weights,
            )
            .is_err()
        );
        let mut changed = proof.clone();
        changed.nonces.pop();
        reject(&changed);
        let mut changed = proof.clone();
        changed.nonces.push(0);
        reject(&changed);
        let mut changed = proof.clone();
        changed.nonces[0] ^= 1;
        reject(&changed);
        assert!(
            verify(
                &mut Blake3Transcript::new(),
                &layout,
                BridgeMode::TwoLimbs,
                &point,
                value,
                modulus,
                &proof,
                3,
            )
            .is_err()
        );
        for limb in 0..2 {
            let mut changed = proof.clone();
            let sum = &mut split_sums(&mut changed)[0];
            if limb == 0 {
                sum.lower ^= 1;
            } else {
                sum.upper ^= 1;
            }
            reject(&changed);
        }
        // Preserve the prime read-off exactly while shifting mass between
        // limbs. Only the authenticated forest can detect this forgery.
        let mut changed = proof.clone();
        let radix = 1u128 << limb_width(&p);
        let sum = split_sums(&mut changed)
            .iter_mut()
            .find(|sum| sum.lower >= radix)
            .unwrap();
        sum.lower -= radix;
        sum.upper += 1;
        let error = verify(
            &mut Blake3Transcript::new(),
            &layout,
            BridgeMode::TwoLimbs,
            &point,
            value,
            modulus,
            &changed,
            2,
        )
        .err()
        .unwrap();
        assert!(error.to_string().contains("bridge forest"));
        let mut changed = proof.clone();
        split_sums(&mut changed)[0].upper =
            checked_split_weight_sum(&split_weights).unwrap().upper + 1;
        reject(&changed);
        let mut changed = proof.clone();
        changed.forest[0][0] += Gf::one();
        reject(&changed);
        let mut changed = proof.clone();
        changed.forest.last_mut().unwrap()[1] += Gf::one();
        reject(&changed);
        let mut changed = proof.clone();
        changed.forest.pop();
        reject(&changed);
        let mut changed = proof.clone();
        changed.forest.push([Gf::one(); 2]);
        reject(&changed);
    }
}
