//! Compressed committed bits for the aligned signed15 arithmetic witness.
//!
//! The arithmetic binder retains its regular sixteen-bit coefficient lanes.
//! Large batches commit two dense bit planes and an exception/slack tail.
//! Minimum-size commitments retain the original signed15 lanes because
//! compression cannot reduce their physical size. Split-source exceptions are
//! public and must be bound before the first arithmetic challenge. Each denotes
//! one coefficient whose top three bits replace the extension of bit eleven.

use super::{BETA_SQUARED, FalconError, Layout, N, SLACK_BITS, Source, WitnessData, error};
use crate::{
    hybrid::{
        BinaryClaim,
        joint_sumcheck::{Coefficients, Gather, Tensor},
        opening::Geometry,
        sumcheck::eq_table,
    },
    pcs::IntegerMatrixLayout,
};
use flock_core::field::Gf128 as Gf;
#[cfg(feature = "parallel")]
use rayon::prelude::*;

pub(super) const BASE_BITS: usize = 12;
pub(super) const MAX_EXCEPTIONS: usize = (BETA_SQUARED / (1 << 22)) as usize;
pub(super) const TAIL_STRIDE: usize = (3 * MAX_EXCEPTIONS + SLACK_BITS).next_power_of_two();
const SLACK_MASK: u64 = 0x8000_8000_8000_8000;
const UPPER_MASK: u64 = 0x7000_7000_7000_7000;

pub(super) struct CompactSource {
    pub packed: [Vec<Gf>; 3],
    pub exceptions: Vec<Vec<u16>>,
}

pub(super) fn is_split(layout: &Layout) -> bool {
    layout.source_bits() >= 1 << 20
}

/// Logical packed-word logarithms; only the first entry is used in single mode.
pub(super) fn source_logs(layout: &Layout) -> [usize; 3] {
    if is_split(layout) {
        [
            (16 * N * layout.capacity() / 128).ilog2() as usize,
            (8 * N * layout.capacity() / 128).ilog2() as usize,
            (TAIL_STRIDE * layout.capacity()).max(1 << 16).ilog2() as usize - 7,
        ]
    } else {
        [layout.source_bits().ilog2() as usize - 7, 0, 0]
    }
}

/// Matrix layouts of the physical source planes' logical value bits.
/// The dense planes use smaller row tables to reduce the bridge forest work.
/// The unused entries in single-source mode have no variables.
pub(super) fn bridge_layouts(layout: &Layout) -> [IntegerMatrixLayout; 3] {
    if !is_split(layout) {
        return [
            layout.bitz_params(),
            IntegerMatrixLayout {
                row_vars: 0,
                col_vars: 0,
            },
            IntegerMatrixLayout {
                row_vars: 0,
                col_vars: 0,
            },
        ];
    }
    let logs = source_logs(layout);
    std::array::from_fn(|branch| {
        let row_vars = [12, 11, 10][branch];
        IntegerMatrixLayout {
            row_vars,
            col_vars: logs[branch] + 7 - row_vars,
        }
    })
}

pub(super) fn validate_exceptions(
    layout: &Layout,
    exceptions: &[Vec<u16>],
) -> Result<(), FalconError> {
    if !is_split(layout) {
        return if exceptions.is_empty() {
            Ok(())
        } else {
            Err(error("unexpected signed15 algebraic exception indices"))
        };
    }
    if exceptions.len() != layout.batch()
        || exceptions.iter().any(|indices| {
            indices.len() > MAX_EXCEPTIONS
                || indices.iter().any(|&index| usize::from(index) >= 2 * N)
                || indices.windows(2).any(|pair| pair[0] >= pair[1])
        })
    {
        return Err(error("invalid compact algebraic exception indices"));
    }
    Ok(())
}

fn insert(packed: &mut [Gf], index: usize, value: u64) {
    let word = &mut packed[index / 128];
    if index % 128 < 64 {
        word.lo |= value << (index % 64);
    } else {
        word.hi |= value << (index % 64);
    }
}

impl CompactSource {
    /// Pack the native signed15 coefficients directly into committed planes,
    /// without allocating the aligned virtual arithmetic source.
    pub fn from_data(layout: &Layout, data: &WitnessData) -> Result<Self, FalconError> {
        validate_data(layout, data)?;
        if is_split(layout) {
            return Self::pack_split(layout, |signature, local| {
                native_word(data, signature, local)
            });
        }
        let mut packed = vec![Gf::ZERO; 1 << 12];
        for signature in 0..layout.batch() {
            for local in (0..2 * N).step_by(8) {
                packed[signature * (2 * N / 8) + local / 8] = Gf {
                    lo: native_word(data, signature, local),
                    hi: native_word(data, signature, local + 4),
                };
            }
        }
        Ok(Self {
            packed: [packed, Vec::new(), Vec::new()],
            exceptions: Vec::new(),
        })
    }

    pub fn new(layout: &Layout, source: &Source, data: &WitnessData) -> Result<Self, FalconError> {
        validate_data(layout, data)?;
        if source.layout() != layout {
            return Err(error("compact algebraic witness shape"));
        }
        if !is_split(layout) {
            let mut packed = Vec::with_capacity(layout.source_bits() / 128);
            for row in source.rows() {
                packed.extend(row.chunks_exact(2).map(|word| Gf {
                    lo: word[0],
                    hi: word[1],
                }));
            }
            // The logical source can be smaller than the PCS allocation floor.
            packed.resize(1 << 12, Gf::ZERO);
            // The arithmetic binder enforces both internal and inactive padding.
            // Retaining the exact source here lets that proof check either kind.
            return Ok(Self {
                packed: [packed, Vec::new(), Vec::new()],
                exceptions: Vec::new(),
            });
        }
        // Read the actual arithmetic source, including deliberately modified
        // sources in negative tests, rather than silently repacking native data.
        let columns_per_signature = layout.signature_stride() / layout.bitz_params().rows();
        if source.rows()[layout.batch() * columns_per_signature..]
            .iter()
            .flatten()
            .any(|&word| word != 0)
        {
            return Err(error("nonzero compact algebraic virtual padding"));
        }
        Self::pack_split(layout, |signature, local| {
            source.rows()[signature * columns_per_signature + local / 512][(local % 512) / 4]
        })
    }

    fn pack_split(
        layout: &Layout,
        word: impl Fn(usize, usize) -> u64 + Sync,
    ) -> Result<Self, FalconError> {
        let physical = Geometry::new(source_logs(layout))
            .map_err(error)?
            .physical_logs;
        let mut packed: [Vec<Gf>; 3] =
            std::array::from_fn(|branch| vec![Gf::ZERO; 1 << physical[branch]]);
        let mut exceptions = vec![Vec::new(); layout.batch()];
        let [low, high, tail] = &mut packed;
        pack_planes(word, &mut exceptions, low, high, tail)?;
        Ok(Self { packed, exceptions })
    }
}

fn validate_data(layout: &Layout, data: &WitnessData) -> Result<(), FalconError> {
    if data.witness.s1.len() != layout.batch()
        || data.witness.s2.len() != layout.batch()
        || data.slacks.len() != layout.batch()
    {
        return Err(error("compact algebraic witness shape"));
    }
    Ok(())
}

#[inline]
fn native_word(data: &WitnessData, signature: usize, local: usize) -> u64 {
    let polynomial = if local < N {
        &data.witness.s1[signature]
    } else {
        &data.witness.s2[signature]
    };
    let lanes = &polynomial[local % N..local % N + 4];
    let mut word = (u64::from(lanes[0] as u16) & 0x7fff)
        | ((u64::from(lanes[1] as u16) & 0x7fff) << 16)
        | ((u64::from(lanes[2] as u16) & 0x7fff) << 32)
        | ((u64::from(lanes[3] as u16) & 0x7fff) << 48);
    if local < SLACK_BITS {
        for lane in 0..(SLACK_BITS - local).min(4) {
            word |= ((data.slacks[signature] >> (local + lane)) & 1) << (16 * lane + 15);
        }
    }
    word
}

#[inline]
fn observe_word(
    word: u64,
    local: usize,
    indices: &mut Vec<u16>,
    exception_bits: &mut u64,
) -> Result<(), FalconError> {
    let live = SLACK_BITS.saturating_sub(local).min(4);
    if live < 4 && word & (SLACK_MASK << (16 * live)) != 0 {
        return Err(error("nonzero compact algebraic virtual padding"));
    }
    // Adjacent equality of bits11..14 characterizes ordinary signed12 values.
    let different = (word ^ (word << 1)) & UPPER_MASK;
    if different != 0 {
        for lane in 0..4 {
            if different & (0x7000 << (16 * lane)) == 0 {
                continue;
            }
            if indices.len() == MAX_EXCEPTIONS {
                return Err(error("too many compact algebraic exceptions"));
            }
            let upper = (word >> (16 * lane + BASE_BITS)) & 7;
            *exception_bits |= upper << (3 * indices.len());
            indices.push((local + lane) as u16);
        }
    }
    Ok(())
}

fn pack_planes(
    word: impl Fn(usize, usize) -> u64 + Sync,
    exceptions: &mut [Vec<u16>],
    low: &mut [Gf],
    high: &mut [Gf],
    tail: &mut [Gf],
) -> Result<(), FalconError> {
    #[cfg(feature = "parallel")]
    if exceptions.len() >= 128 && rayon::current_num_threads() > 1 {
        // Sixteen signatures keep the dense output planes near L1 size while
        // providing enough independent tasks for the large-batch prover.
        const CHUNK: usize = 16;
        return exceptions
            .par_chunks_mut(CHUNK)
            .zip(low.par_chunks_mut(CHUNK * 16 * N / 128))
            .zip(high.par_chunks_mut(CHUNK * 8 * N / 128))
            .zip(tail.par_chunks_mut(CHUNK * TAIL_STRIDE / 128))
            .enumerate()
            .try_for_each(|(chunk, (((exceptions, low), high), tail))| {
                pack_signatures(&word, chunk * CHUNK, exceptions, low, high, tail)
            });
    }
    pack_signatures(&word, 0, exceptions, low, high, tail)
}

fn pack_signatures(
    word: &impl Fn(usize, usize) -> u64,
    first_signature: usize,
    exceptions: &mut [Vec<u16>],
    low_plane: &mut [Gf],
    high_plane: &mut [Gf],
    tail_plane: &mut [Gf],
) -> Result<(), FalconError> {
    for (signature, indices) in exceptions.iter_mut().enumerate() {
        let low_start = signature * 16 * N;
        let high_start = signature * 8 * N;
        let tail_start = signature * TAIL_STRIDE;
        let mut exception_bits = 0_u64;
        for local in (0..2 * N).step_by(4) {
            let word = word(first_signature + signature, local);
            observe_word(word, local, indices, &mut exception_bits)?;
            // Compress four aligned sixteen-bit lanes in one operation
            // per plane, without branching on each ordinary coefficient.
            let low = (word & 0xff)
                | ((word >> 8) & 0xff00)
                | ((word >> 16) & 0xff0000)
                | ((word >> 24) & 0xff000000);
            let high = ((word >> 8) & 0xf)
                | ((word >> 20) & 0xf0)
                | ((word >> 32) & 0xf00)
                | ((word >> 44) & 0xf000);
            insert(low_plane, low_start + local * 8, low);
            insert(high_plane, high_start + local * 4, high);

            if local < SLACK_BITS {
                let slack = ((word >> 15) & 1)
                    | ((word >> 30) & 2)
                    | ((word >> 45) & 4)
                    | ((word >> 60) & 8);
                insert(tail_plane, tail_start + 3 * MAX_EXCEPTIONS + local, slack);
            }
        }
        // All 24/48 exception bits fit into the first half of the tail.
        insert(tail_plane, tail_start, exception_bits);
    }
    Ok(())
}

/// Transpose the signed15 decoder onto the final integer-bridge claim.
/// Unused tail bits do not decode to any arithmetic variable. Their values
/// therefore cannot affect the ring equation, norm, or virtual padding check.
pub(super) fn coefficients(
    layout: &Layout,
    parameters: &IntegerMatrixLayout,
    exceptions: &[Vec<u16>],
    claim: BinaryClaim,
) -> Result<[Coefficients; 3], FalconError> {
    validate_exceptions(layout, exceptions)?;
    let source = layout.bitz_params();
    if parameters.row_vars < source.row_vars
        || parameters.row_vars + parameters.col_vars != source.row_vars + source.col_vars
        || claim.low.len() != parameters.rows()
        || claim.high_point.len() != parameters.col_vars
    {
        return Err(error("compact algebraic binary claim shape"));
    }
    let mut coefficients: [Coefficients; 3] = std::array::from_fn(|_| Coefficients::default());
    if !is_split(layout) {
        coefficients[0].tensors.push(Tensor {
            low: claim.low,
            high_point: claim.high_point,
            marginals: None,
        });
        return Ok(coefficients);
    }
    coefficients[0].tensors.push(Tensor {
        low: claim
            .low
            .chunks_exact(16)
            .flat_map(|row| row[..8].iter().copied())
            .collect(),
        high_point: claim.high_point.clone(),
        marginals: None,
    });
    coefficients[1].tensors.push(Tensor {
        low: claim
            .low
            .chunks_exact(16)
            .flat_map(|row| {
                [
                    row[8],
                    row[9],
                    row[10],
                    row[11] + row[12] + row[13] + row[14],
                ]
            })
            .collect(),
        high_point: claim.high_point.clone(),
        marginals: None,
    });

    // Slack occupies column zero inside each signature. A packed tail
    // word holds two Falcon-512 instances or one Falcon-1024 instance.
    let packed_instances = 7 - TAIL_STRIDE.ilog2() as usize;
    let signature_vars = layout.signature_stride().ilog2() as usize;
    if parameters.row_vars < signature_vars {
        let instance_vars = layout.capacity().ilog2() as usize;
        let local_columns = claim.high_point.len() - instance_vars;
        let column_zero = claim.high_point[..local_columns]
            .iter()
            .fold(Gf::ONE, |weight, &point| weight * (Gf::ONE + point));
        let instance_point = &claim.high_point[local_columns..];
        let local_instance_weights = eq_table(&instance_point[..packed_instances]);
        let slack_weights: Vec<_> = (0..SLACK_BITS)
            .map(|bit| column_zero * claim.low[16 * bit + 15])
            .collect();
        let mut slack = Vec::with_capacity(SLACK_BITS * local_instance_weights.len());
        for (instance, instance_weight) in local_instance_weights.into_iter().enumerate() {
            for (bit, &weight) in slack_weights.iter().enumerate() {
                slack.push((
                    instance * TAIL_STRIDE + 3 * MAX_EXCEPTIONS + bit,
                    weight * instance_weight,
                ));
            }
        }
        coefficients[2].gathers.push(Gather::repeated_at(
            slack,
            instance_point[packed_instances..].to_vec(),
            7,
        ));
    } else {
        // Bridge rows hold 2^local whole signatures; their slack weights come
        // from the row factor, and the column point indexes higher instances.
        let local = parameters.row_vars - signature_vars;
        let extra = packed_instances.saturating_sub(local);
        let extra_weights = eq_table(&claim.high_point[..extra]);
        let mut slack = Vec::with_capacity(SLACK_BITS << (local + extra));
        for (outer, &outer_weight) in extra_weights.iter().enumerate() {
            for inner in 0..1 << local {
                let instance = inner + (outer << local);
                for bit in 0..SLACK_BITS {
                    slack.push((
                        instance * TAIL_STRIDE + 3 * MAX_EXCEPTIONS + bit,
                        claim.low[(inner << signature_vars) + 16 * bit + 15] * outer_weight,
                    ));
                }
            }
        }
        coefficients[2].gathers.push(Gather::repeated_at(
            slack,
            claim.high_point[extra..].to_vec(),
            TAIL_STRIDE.ilog2() as usize + local + extra,
        ));
    }

    let exception_count = exceptions.iter().map(Vec::len).sum::<usize>();
    if exception_count != 0 {
        let high = eq_table(&claim.high_point);
        let weight =
            |index: usize| claim.low[index % parameters.rows()] * high[index / parameters.rows()];
        let mut corrections = Vec::with_capacity(exception_count);
        let mut tail = Vec::with_capacity(3 * exception_count);
        for (signature, indices) in exceptions.iter().enumerate() {
            let base = signature * layout.signature_stride();
            for (rank, &index) in indices.iter().enumerate() {
                let index = usize::from(index);
                let weights = std::array::from_fn::<_, 3, _>(|bit| {
                    weight(base + index * 16 + BASE_BITS + bit)
                });
                // Characteristic two: adding these weights cancels the default
                // sign extension and replaces it with three explicit tail bits.
                corrections.push((
                    (signature * 2 * N + index) * 4 + 3,
                    weights[0] + weights[1] + weights[2],
                ));
                for (bit, value) in weights.into_iter().enumerate() {
                    tail.push((signature * TAIL_STRIDE + 3 * rank + bit, value));
                }
            }
        }
        coefficients[1]
            .gathers
            .push(Gather::repeated(corrections, Vec::new()));
        coefficients[2]
            .gathers
            .push(Gather::repeated(tail, Vec::new()));
    }
    Ok(coefficients)
}

#[cfg(test)]
mod tests {
    use super::super::{FalconAlgebraicStatement, FalconAlgebraicWitness};
    use super::*;
    use crate::{
        hybrid::{joint_sumcheck as joint, sumcheck::Scratch},
        transcript::Blake3Transcript,
    };

    fn fixture(batch: usize) -> (Layout, WitnessData, Source) {
        let layout = Layout::new(batch).unwrap();
        let public = FalconAlgebraicStatement {
            public_keys: vec![[0; N]; batch],
            targets: vec![[0; N]; batch],
        };
        let mut data = WitnessData::new(
            &public,
            FalconAlgebraicWitness {
                s1: vec![[0; N]; batch],
                s2: vec![[0; N]; batch],
            },
        )
        .unwrap();
        // The decoder is tested on all base bits, independently of the norm.
        // The supplied values intentionally exercise noncentered signed15 too.
        for signature in 0..batch {
            for (index, coefficient) in data.witness.s1[signature]
                .iter_mut()
                .chain(&mut data.witness.s2[signature])
                .enumerate()
            {
                *coefficient = ((index * 117 + signature * 223) % 4096) as i16 - 2048;
            }
            for rank in 0..MAX_EXCEPTIONS {
                let index = rank * (2 * N - 1) / (MAX_EXCEPTIONS - 1);
                let value = [-16384, -2049, 2048, 16383][rank % 4];
                if index < N {
                    data.witness.s1[signature][index] = value;
                } else {
                    data.witness.s2[signature][index - N] = value;
                }
            }
            data.slacks[signature] = (0x55aa_55aa >> (signature % 7)) & ((1 << SLACK_BITS) - 1);
        }
        let source = Source::new(layout, &data);
        (layout, data, source)
    }

    fn bit(source: &CompactSource, branch: usize, index: usize) -> bool {
        let word = source.packed[branch][index / 128];
        (if index % 128 < 64 { word.lo } else { word.hi }) >> (index % 64) & 1 != 0
    }

    fn virtual_bit(layout: &Layout, source: &CompactSource, index: usize) -> bool {
        if !is_split(layout) {
            return bit(source, 0, index);
        }
        let coefficient = index / 16;
        let signature = coefficient / (2 * N);
        let local = coefficient % (2 * N);
        let lane = index % 16;
        let exception_rank = || {
            source
                .exceptions
                .get(signature)
                .and_then(|indices| indices.binary_search(&(local as u16)).ok())
        };
        match lane {
            0..=7 => bit(source, 0, coefficient * 8 + lane),
            8..=11 => bit(source, 1, coefficient * 4 + lane - 8),
            12..=14 => match exception_rank() {
                Some(rank) => bit(source, 2, signature * TAIL_STRIDE + 3 * rank + lane - 12),
                None => bit(source, 1, coefficient * 4 + 3),
            },
            _ if local < SLACK_BITS => bit(
                source,
                2,
                signature * TAIL_STRIDE + 3 * MAX_EXCEPTIONS + local,
            ),
            _ => false,
        }
    }

    #[test]
    fn compact_planes_reconstruct_every_virtual_bit() {
        for batch in [1, 3, 33, 65] {
            let (layout, data, source) = fixture(batch);
            let compact = CompactSource::new(&layout, &source, &data).unwrap();
            let direct = CompactSource::from_data(&layout, &data).unwrap();
            assert_eq!(compact.packed, direct.packed);
            assert_eq!(compact.exceptions, direct.exceptions);
            validate_exceptions(&layout, &compact.exceptions).unwrap();
            for index in 0..layout.source_bits() {
                assert_eq!(
                    virtual_bit(&layout, &compact, index),
                    source.bit(index),
                    "batch={batch}, index={index}"
                );
            }
        }
    }

    fn check_joint<const BRANCHES: usize>(
        geometry: &Geometry<BRANCHES>,
        packed: [&[Gf]; BRANCHES],
        coefficients: [&Coefficients; BRANCHES],
        target: Gf,
    ) {
        let (proof, point) = joint::prove(
            &mut Blake3Transcript::new(),
            geometry,
            packed,
            coefficients,
            target,
            0,
            &mut Scratch::default(),
        )
        .unwrap();
        let verified = joint::verify(
            &mut Blake3Transcript::new(),
            geometry,
            coefficients,
            target,
            0,
            &proof,
        )
        .unwrap();
        assert_eq!(point, verified);
        assert!(
            joint::verify(
                &mut Blake3Transcript::new(),
                geometry,
                coefficients,
                target + Gf::ONE,
                0,
                &proof,
            )
            .is_err()
        );
    }

    #[test]
    fn transposed_claim_matches_direct_virtual_dot_product() {
        for (batch, with_exceptions) in [(1, false), (1, true), (65, false), (65, true)] {
            let (layout, mut data, mut source) = fixture(batch);
            if !with_exceptions {
                for coefficient in data
                    .witness
                    .s1
                    .iter_mut()
                    .chain(&mut data.witness.s2)
                    .flatten()
                {
                    *coefficient /= 16;
                }
                source = Source::new(layout, &data);
            }
            let compact = CompactSource::new(&layout, &source, &data).unwrap();
            let field = |index: usize| Gf {
                lo: (index as u64 + 1).wrapping_mul(0x9e3779b97f4a7c15),
                hi: (index as u64 + 1).wrapping_mul(0x25b44a15d941a31b),
            };
            let base = layout.bitz_params();
            let total = base.row_vars + base.col_vars;
            // Wider bridge rows group consecutive source rows into one column.
            for row_vars in (base.row_vars..=16).filter(|&rows| rows < total) {
                let parameters = IntegerMatrixLayout {
                    row_vars,
                    col_vars: total - row_vars,
                };
                let mut claim = BinaryClaim {
                    low: (0..parameters.rows()).map(field).collect(),
                    high_point: (0..parameters.col_vars)
                        .map(|i| field(i + (1 << 16)))
                        .collect(),
                    value: Gf::ZERO,
                };
                let groups = source.rows().chunks(1 << (row_vars - base.row_vars));
                for (group, high) in groups.zip(eq_table(&claim.high_point)) {
                    let mut column = Gf::ZERO;
                    for (word_index, &word) in group.iter().flatten().enumerate() {
                        let mut remaining = word;
                        while remaining != 0 {
                            column +=
                                claim.low[64 * word_index + remaining.trailing_zeros() as usize];
                            remaining &= remaining - 1;
                        }
                    }
                    claim.value += column * high;
                }
                let target = claim.value;
                let coefficients =
                    coefficients(&layout, &parameters, &compact.exceptions, claim).unwrap();
                if is_split(&layout) {
                    check_joint(
                        &Geometry::new(source_logs(&layout)).unwrap(),
                        compact.packed.each_ref().map(Vec::as_slice),
                        coefficients.each_ref(),
                        target,
                    );
                } else {
                    check_joint(
                        &Geometry::with_allocation_logs([source_logs(&layout)[0]], [12]).unwrap(),
                        [compact.packed[0].as_slice()],
                        [&coefficients[0]],
                        target,
                    );
                }
            }
        }
    }

    #[test]
    fn exception_metadata_and_unrepresentable_padding_are_rejected() {
        let (layout, data, mut source) = fixture(65);
        let compact = CompactSource::new(&layout, &source, &data).unwrap();
        assert!(validate_exceptions(&layout, &[]).is_err());
        for indices in [
            vec![0, 0],
            vec![1, 0],
            vec![(2 * N) as u16],
            (0..=MAX_EXCEPTIONS as u16).collect(),
        ] {
            let mut invalid = compact.exceptions.clone();
            invalid[0] = indices;
            assert!(validate_exceptions(&layout, &invalid).is_err());
        }
        assert_eq!(compact.exceptions[0].len(), MAX_EXCEPTIONS);
        source.flip(16 * SLACK_BITS + 15);
        assert!(CompactSource::new(&layout, &source, &data).is_err());
    }

    #[test]
    fn minimum_size_source_is_exact_signed15_with_no_metadata() {
        let (layout, data, mut source) = fixture(3);
        assert!(!is_split(&layout));
        // Raw commitments preserve padding too; the arithmetic proof rejects it.
        source.flip(16 * SLACK_BITS + 15);
        source.flip(layout.batch() * layout.signature_stride());
        let compact = CompactSource::new(&layout, &source, &data).unwrap();
        assert!(compact.exceptions.is_empty());
        assert!(compact.packed[1].is_empty() && compact.packed[2].is_empty());
        let words = source.rows().iter().flatten().copied().collect::<Vec<_>>();
        for (packed, expected) in compact.packed[0].iter().zip(words.chunks_exact(2)) {
            assert_eq!((packed.lo, packed.hi), (expected[0], expected[1]));
        }
        assert_eq!(compact.packed[0].len(), 1 << 12);
        assert!(
            compact.packed[0][words.len() / 2..]
                .iter()
                .all(|&word| word == Gf::ZERO)
        );
        assert!(validate_exceptions(&layout, &[Vec::new()]).is_err());
        assert!(validate_exceptions(&layout, &[vec![0]]).is_err());
    }

    #[test]
    fn norm_valid_boundaries_and_maximum_exception_count_are_preserved() {
        let cases = if N == 1024 { 5 } else { 3 };
        for batch in [cases, 65] {
            let layout = Layout::new(batch).unwrap();
            let public = FalconAlgebraicStatement {
                public_keys: vec![[0; N]; batch],
                targets: vec![[0; N]; batch],
            };
            let mut s2 = vec![[0; N]; batch];
            s2[0][..MAX_EXCEPTIONS].fill(2048);
            s2[1][N - 1] = -2048;
            s2[2][N - 1] = -2049;
            if N == 1024 {
                s2[3][N - 1] = 8382;
                s2[4][N - 1] = -8382;
            }
            let data = WitnessData::new(
                &public,
                FalconAlgebraicWitness {
                    s1: vec![[0; N]; batch],
                    s2,
                },
            )
            .unwrap();
            let source = Source::new(layout, &data);
            let compact = CompactSource::new(&layout, &source, &data).unwrap();
            if is_split(&layout) {
                assert_eq!(
                    compact.exceptions[0],
                    (N..N + MAX_EXCEPTIONS)
                        .map(|index| index as u16)
                        .collect::<Vec<_>>()
                );
                assert!(compact.exceptions[1].is_empty());
                for signature in 2..cases {
                    assert_eq!(compact.exceptions[signature], vec![(2 * N - 1) as u16]);
                }
                assert!(compact.exceptions[cases..].iter().all(Vec::is_empty));
            } else {
                assert!(compact.exceptions.is_empty());
            }
            for signature in 0..batch {
                for index in 0..N {
                    let base = (signature * 2 * N + N + index) * 16;
                    let bits = (0..15).fold(0_u16, |bits, lane| {
                        bits | (u16::from(virtual_bit(&layout, &compact, base + lane)) << lane)
                    });
                    assert_eq!(((bits << 1) as i16) >> 1, data.witness.s2[signature][index]);
                }
            }
            let mut overflow = data.clone();
            overflow.witness.s2[0][MAX_EXCEPTIONS] = 2048;
            assert!(WitnessData::new(&public, overflow.witness.clone()).is_err());
            let encoded = CompactSource::new(&layout, &Source::new(layout, &overflow), &overflow);
            assert_eq!(encoded.is_err(), is_split(&layout));
            assert_eq!(
                CompactSource::from_data(&layout, &overflow).is_err(),
                is_split(&layout)
            );
        }
    }

    #[cfg(feature = "parallel")]
    #[test]
    fn parallel_packing_preserves_all_bits_and_exception_indices() {
        // The final task contains one signature: in Falcon-512 its tail shares
        // a packed word with an inactive signature and must not change it.
        let (layout, data, source) = fixture(129);
        let encode = |threads| {
            rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap()
                .install(|| {
                    let encoded = CompactSource::new(&layout, &source, &data).unwrap();
                    let direct = CompactSource::from_data(&layout, &data).unwrap();
                    assert_eq!(encoded.packed, direct.packed);
                    assert_eq!(encoded.exceptions, direct.exceptions);
                    direct
                })
        };
        let serial = encode(1);
        for threads in [2, 8] {
            let parallel = encode(threads);
            assert_eq!(serial.packed, parallel.packed);
            assert_eq!(serial.exceptions, parallel.exceptions);
        }
    }

    #[test]
    fn every_signed15_coefficient_has_an_exact_compact_encoding() {
        for value in -16384_i16..=16383 {
            let bits = value as u16 & 0x7fff;
            let upper = if (-2048..=2047).contains(&value) {
                if bits & 0x800 == 0 { 0 } else { 7 }
            } else {
                bits >> BASE_BITS
            };
            let reconstructed = ((bits & 0xfff) | (upper << BASE_BITS)) as i16;
            assert_eq!((reconstructed << 1) >> 1, value);
        }
    }

    #[test]
    fn bridge_matrices_cover_exactly_the_logical_source_bits() {
        for batch in [1, 3, 16, 32, 33, 65, 129, 1024] {
            let layout = Layout::new(batch).unwrap();
            let parameters = bridge_layouts(&layout);
            if is_split(&layout) {
                for branch in 0..3 {
                    assert_eq!(parameters[branch].row_vars, [12, 11, 10][branch]);
                    assert_eq!(
                        parameters[branch].row_vars + parameters[branch].col_vars,
                        source_logs(&layout)[branch] + 7
                    );
                }
            } else {
                assert_eq!(parameters[0], layout.bitz_params());
                assert_eq!(
                    parameters[0].rows() * parameters[0].cols(),
                    layout.source_bits()
                );
                assert_eq!(parameters[1].row_vars + parameters[1].col_vars, 0);
                assert_eq!(parameters[2].row_vars + parameters[2].col_vars, 0);
            }
        }
    }
}
