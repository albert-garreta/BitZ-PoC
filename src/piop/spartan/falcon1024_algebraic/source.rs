use super::{BETA_SQUARED, COEFFICIENT_LOG, FalconError, N, Q, SLACK_BITS, error};
use crate::pcs::IntegerMatrixLayout;
#[cfg(test)]
use crate::piop::spartan::falcon_bit_layout::{coefficient_bit, slack_bit};
use crate::piop::spartan::falcon_polynomial::PolynomialWorkspace;
#[cfg(feature = "parallel")]
use rayon::prelude::*;

/// Public keys and externally computed targets, each in F_12289[X]/(X^N+1).
/// Coefficients are in ascending degree and must be canonical, in 0..12289.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FalconAlgebraicStatement {
    pub public_keys: Vec<[u16; N]>,
    pub targets: Vec<[u16; N]>,
}

impl FalconAlgebraicStatement {
    pub(super) fn validate(&self, batch: usize) -> Result<(), FalconError> {
        if self.public_keys.len() != batch || self.targets.len() != batch {
            return Err(FalconError::InvalidBatchCapacity);
        }
        for polynomial in self.public_keys.iter().chain(&self.targets) {
            if polynomial.iter().any(|&x| i64::from(x) >= Q) {
                return Err(error("noncanonical algebraic public coefficient"));
            }
        }
        Ok(())
    }
}

/// Signed integer coefficients, shared by the ring and norm constraints.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FalconAlgebraicWitness {
    pub s1: Vec<[i16; N]>,
    pub s2: Vec<[i16; N]>,
}

impl FalconAlgebraicWitness {
    /// Derive the centered s1 = t - h*s2 (mod q), then check the exact norm.
    /// No message hashing or signature decoding is performed here.
    pub fn from_s2(
        statement: &FalconAlgebraicStatement,
        s2: Vec<[i16; N]>,
    ) -> Result<Self, FalconError> {
        Layout::new(s2.len())?;
        statement.validate(s2.len())?;
        check_coefficients(&s2)?;
        let s1 = map_products(statement, &s2, |i, product| {
            Ok(centered_s1(&statement.targets[i], product))
        })?;
        let witness = Self { s1, s2 };
        check_norms(&witness)?;
        Ok(witness)
    }
}

/// Native check of the same algebraic statement proved by this API.
pub fn check_algebraic_statement(
    statement: &FalconAlgebraicStatement,
    witness: &FalconAlgebraicWitness,
) -> Result<(), FalconError> {
    WitnessData::new(statement, witness.clone()).map(|_| ())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Layout {
    batch: usize,
    capacity: usize,
}

impl Layout {
    pub fn new(batch: usize) -> Result<Self, FalconError> {
        if !(1..=1024).contains(&batch) {
            return Err(FalconError::InvalidBatchCapacity);
        }
        Ok(Self {
            batch,
            // The shared opener doubles the source; Ligerito requires at least
            // 2^20 committed bits, so retain at least 2^19 source bits.
            capacity: batch.next_power_of_two().max((1 << 19) / (32 * N)),
        })
    }
    pub const fn batch(&self) -> usize {
        self.batch
    }
    pub const fn capacity(&self) -> usize {
        self.capacity
    }
    pub const fn signature_stride(&self) -> usize {
        32 * N
    }
    pub const fn source_bits(&self) -> usize {
        self.signature_stride() * self.capacity
    }
    pub const fn row_vars(&self) -> usize {
        13
    }
    pub const fn bitz_params(&self) -> IntegerMatrixLayout {
        IntegerMatrixLayout {
            row_vars: self.row_vars(),
            col_vars: COEFFICIENT_LOG + 5 - self.row_vars() + self.capacity.ilog2() as usize,
        }
    }
    #[cfg(test)]
    pub fn is_live(&self, index: usize) -> bool {
        let local = index % self.signature_stride();
        index / self.signature_stride() < self.batch && (local % 16 < 15 || local / 16 < SLACK_BITS)
    }
}

#[derive(Clone, Debug)]
pub(super) struct WitnessData {
    pub witness: FalconAlgebraicWitness,
    pub slacks: Vec<u64>,
    pub quotients: Vec<Vec<u16>>,
}

impl WitnessData {
    pub fn from_s2(
        public: &FalconAlgebraicStatement,
        s2: Vec<[i16; N]>,
    ) -> Result<Self, FalconError> {
        let layout = Layout::new(s2.len())?;
        public.validate(layout.batch())?;
        check_coefficients(&s2)?;
        let signatures = map_products(public, &s2, |i, product| {
            let mut s1 = [0; N];
            let mut norm = 0;
            let mut quotient = vec![0; N - 1];
            for j in 0..N {
                let high = product[N + j];
                let centered = center(i64::from(public.targets[i][j]) - product[j] + high);
                s1[j] = centered;
                norm += i64::from(centered).unsigned_abs().pow(2);
                norm += i64::from(s2[i][j]).unsigned_abs().pow(2);
                if j < N - 1 {
                    quotient[j] = (-high).rem_euclid(Q) as u16;
                }
            }
            Ok((s1, checked_slack(norm)?, quotient))
        })?;
        let mut s1 = Vec::with_capacity(layout.batch());
        let mut slacks = Vec::with_capacity(layout.batch());
        let mut quotients = Vec::with_capacity(layout.batch());
        for (signature_s1, slack, quotient) in signatures {
            s1.push(signature_s1);
            slacks.push(slack);
            quotients.push(quotient);
        }
        Ok(Self {
            witness: FalconAlgebraicWitness { s1, s2 },
            slacks,
            quotients,
        })
    }

    pub fn new(
        public: &FalconAlgebraicStatement,
        witness: FalconAlgebraicWitness,
    ) -> Result<Self, FalconError> {
        let layout = Layout::new(witness.s1.len())?;
        public.validate(layout.batch())?;
        if witness.s2.len() != layout.batch() {
            return Err(FalconError::InvalidBatchCapacity);
        }
        check_coefficients(&witness.s1)?;
        check_coefficients(&witness.s2)?;
        let slacks = check_norms(&witness)?;
        let quotients = map_products(public, &witness.s2, |i, product| {
            let mut quotient = vec![0; N - 1];
            for j in 0..N {
                let high = product[N + j];
                let residual = i64::from(public.targets[i][j]) - product[j] + high
                    - i64::from(witness.s1[i][j]);
                if residual.rem_euclid(Q) != 0 {
                    return Err(error("invalid algebraic ring witness"));
                }
                if j < N - 1 {
                    quotient[j] = (-high).rem_euclid(Q) as u16;
                }
            }
            Ok(quotient)
        })?;
        Ok(Self {
            witness,
            slacks,
            quotients,
        })
    }
}

fn map_products<T: Send>(
    public: &FalconAlgebraicStatement,
    s2: &[[i16; N]],
    f: impl Fn(usize, &[i64]) -> Result<T, FalconError> + Send + Sync,
) -> Result<Vec<T>, FalconError> {
    #[cfg(feature = "parallel")]
    {
        (0..s2.len())
            .into_par_iter()
            .map_init(
                || PolynomialWorkspace::new(N),
                |workspace, i| f(i, workspace.product(&public.public_keys[i], &s2[i])),
            )
            .collect()
    }
    #[cfg(not(feature = "parallel"))]
    {
        let mut workspace = PolynomialWorkspace::new(N);
        (0..s2.len())
            .map(|i| f(i, workspace.product(&public.public_keys[i], &s2[i])))
            .collect()
    }
}

fn centered_s1(target: &[u16; N], product: &[i64]) -> [i16; N] {
    std::array::from_fn(|j| center(i64::from(target[j]) - product[j] + product[N + j]))
}

fn center(value: i64) -> i16 {
    let residue = value.rem_euclid(Q);
    (if residue > Q / 2 { residue - Q } else { residue }) as i16
}

fn check_coefficients(vectors: &[[i16; N]]) -> Result<(), FalconError> {
    if vectors
        .iter()
        .flatten()
        .any(|&x| !(-16384..=16383).contains(&x))
    {
        return Err(error("algebraic signature coefficient outside signed15"));
    }
    Ok(())
}

fn check_norms(witness: &FalconAlgebraicWitness) -> Result<Vec<u64>, FalconError> {
    witness
        .s1
        .iter()
        .zip(&witness.s2)
        .map(|(s1, s2)| norm_slack(s1, s2))
        .collect()
}

fn norm_slack(s1: &[i16; N], s2: &[i16; N]) -> Result<u64, FalconError> {
    let norm: u64 = s1
        .iter()
        .chain(s2)
        .map(|&x| i64::from(x).unsigned_abs().pow(2))
        .sum();
    checked_slack(norm)
}

fn checked_slack(norm: u64) -> Result<u64, FalconError> {
    if norm > BETA_SQUARED {
        return Err(FalconError::NormTooLarge {
            actual: norm,
            bound: BETA_SQUARED,
        });
    }
    Ok(BETA_SQUARED - norm)
}

#[derive(Clone, Debug)]
pub(super) struct Source {
    layout: Layout,
    rows: Vec<Vec<u64>>,
}

impl Source {
    pub fn new(layout: Layout, data: &WitnessData) -> Self {
        Self::pack(layout, data, false)
    }

    #[cfg(any(test, feature = "bench-internals"))]
    pub fn new_parallel(layout: Layout, data: &WitnessData) -> Self {
        Self::pack(layout, data, true)
    }

    fn pack(layout: Layout, data: &WitnessData, parallel: bool) -> Self {
        let p = layout.bitz_params();
        let mut source = Self {
            layout,
            rows: vec![vec![0; p.rows() / 64]; p.cols()],
        };
        let columns_per_signature = layout.signature_stride() / p.rows();
        let live = &mut source.rows[..layout.batch() * columns_per_signature];
        if parallel {
            #[cfg(feature = "parallel")]
            if rayon::current_num_threads() > 1 {
                live.par_chunks_mut(columns_per_signature)
                    .enumerate()
                    .for_each(|(i, columns)| pack_signature(columns, data, i));
                return source;
            }
        }
        for (i, columns) in live.chunks_mut(columns_per_signature).enumerate() {
            pack_signature(columns, data, i);
        }
        source
    }

    pub fn layout(&self) -> &Layout {
        &self.layout
    }
    pub fn rows(&self) -> &[Vec<u64>] {
        &self.rows
    }
    pub fn bit(&self, index: usize) -> bool {
        self.rows[index >> 13][(index & 8191) >> 6] >> (index & 63) & 1 != 0
    }
    #[cfg(test)]
    pub(super) fn flip(&mut self, index: usize) {
        self.rows[index >> 13][(index & 8191) >> 6] ^= 1 << (index & 63);
    }
}

fn pack_signature(columns: &mut [Vec<u64>], data: &WitnessData, signature: usize) {
    // A column contains 512 aligned signed15 lanes, in polynomial-major order.
    let coefficients = data.witness.s1[signature]
        .chunks_exact(512)
        .chain(data.witness.s2[signature].chunks_exact(512));
    for (column, coefficients) in columns.iter_mut().zip(coefficients) {
        for (word, lanes) in column.iter_mut().zip(coefficients.chunks_exact(4)) {
            *word = (u64::from(lanes[0] as u16) & 0x7fff)
                | ((u64::from(lanes[1] as u16) & 0x7fff) << 16)
                | ((u64::from(lanes[2] as u16) & 0x7fff) << 32)
                | ((u64::from(lanes[3] as u16) & 0x7fff) << 48);
        }
    }
    // Coefficient stores leave the high lane bits clear. Insert slack afterward.
    for bit in 0..SLACK_BITS {
        columns[0][bit / 4] |= ((data.slacks[signature] >> bit) & 1) << (16 * (bit % 4) + 15);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_encoding(source: &Source, data: &WitnessData) {
        let layout = source.layout();
        for signature in 0..layout.batch() {
            let base = signature * layout.signature_stride();
            for (polynomial, coefficients) in
                [&data.witness.s1[signature], &data.witness.s2[signature]]
                    .into_iter()
                    .enumerate()
            {
                for (j, &coefficient) in coefficients.iter().enumerate() {
                    let decoded = (0..15).fold(0_u16, |value, bit| {
                        value
                            | (u16::from(source.bit(base + coefficient_bit(N, polynomial, j, bit)))
                                << bit)
                    });
                    assert_eq!(decoded, coefficient as u16 & 0x7fff);
                }
            }
            let slack = (0..SLACK_BITS).fold(0_u64, |value, bit| {
                value | (u64::from(source.bit(base + slack_bit(bit))) << bit)
            });
            assert_eq!(slack, data.slacks[signature]);
        }
        for index in 0..layout.source_bits() {
            if !layout.is_live(index) {
                assert!(!source.bit(index));
            }
        }
    }

    #[test]
    fn packing_strategies_match_every_word() {
        let mut random = 0x5eeda11ce_u64;
        for batch in [
            1, 3, 8, 15, 16, 17, 31, 32, 33, 63, 64, 65, 127, 128, 129, 255, 256, 257, 511, 512,
            513, 1024,
        ] {
            let layout = Layout::new(batch).unwrap();
            let mut data = WitnessData {
                witness: FalconAlgebraicWitness {
                    s1: vec![[0; N]; batch],
                    s2: vec![[0; N]; batch],
                },
                slacks: (0..batch)
                    .map(|i| match i % 4 {
                        0 => 0,
                        1 => (1 << SLACK_BITS) - 1,
                        2 => 1 << (SLACK_BITS - 1),
                        _ => 0x2aaaaaa & ((1 << SLACK_BITS) - 1),
                    })
                    .collect(),
                quotients: Vec::new(),
            };
            for polynomial in data.witness.s1.iter_mut().chain(&mut data.witness.s2) {
                for (j, coefficient) in polynomial.iter_mut().enumerate() {
                    random ^= random << 13;
                    random ^= random >> 7;
                    random ^= random << 17;
                    *coefficient = match j % 17 {
                        0 => -16384,
                        1 => -1,
                        2 => 0,
                        3 => 16383,
                        _ => (random as i16) >> 1,
                    };
                }
            }
            let reference = Source::new(layout, &data);
            assert_encoding(&reference, &data);
            #[cfg(feature = "parallel")]
            for workers in [1, 2, 4, 8] {
                let pool = rayon::ThreadPoolBuilder::new()
                    .num_threads(workers)
                    .build()
                    .unwrap();
                let actual = pool.install(|| Source::new_parallel(layout, &data));
                assert_eq!(
                    reference.rows(),
                    actual.rows(),
                    "degree={N}, batch={batch}, workers={workers}"
                );
            }
        }
    }

    #[test]
    fn exact_ring_and_norm_match_independent_schoolbook() {
        let h = std::array::from_fn(|j| ((j * 71 + 9) % Q as usize) as u16);
        let s1 = std::array::from_fn(|j| j as i16 % 23 - 11);
        let s2 = std::array::from_fn(|j| j as i16 % 17 - 8);
        let mut t = s1.map(i64::from);
        let mut ordinary = [0_i64; 2 * N];
        for i in 0..N {
            for j in 0..N {
                let k = i + j;
                let term = i64::from(h[i]) * i64::from(s2[j]);
                ordinary[k] += term;
                t[k % N] += if k < N { term } else { -term };
            }
        }
        let public = FalconAlgebraicStatement {
            public_keys: vec![h],
            targets: vec![t.map(|x| x.rem_euclid(Q) as u16)],
        };
        let witness = FalconAlgebraicWitness {
            s1: vec![s1],
            s2: vec![s2],
        };
        check_algebraic_statement(&public, &witness).unwrap();
        let data = WitnessData::new(&public, witness.clone()).unwrap();
        let combined = WitnessData::from_s2(&public, witness.s2.clone()).unwrap();
        assert_eq!(combined.witness, witness);
        assert_eq!(combined.slacks, data.slacks);
        assert_eq!(combined.quotients, data.quotients);
        assert_eq!(combined.quotients[0].len(), N - 1);
        for j in 0..N - 1 {
            assert_eq!(
                combined.quotients[0][j],
                (-ordinary[N + j]).rem_euclid(Q) as u16
            );
        }
        for j in 0..2 * N {
            let residual = if j < N {
                i64::from(public.targets[0][j]) - i64::from(s1[j]) - ordinary[j]
            } else {
                -ordinary[j]
            };
            let expected = if j % N < N - 1 {
                i64::from(combined.quotients[0][j % N])
            } else {
                0
            };
            assert_eq!(residual.rem_euclid(Q), expected);
        }
        assert_eq!(
            data.slacks[0],
            BETA_SQUARED
                - s1.iter()
                    .chain(&s2)
                    .map(|&x| i64::from(x).unsigned_abs().pow(2))
                    .sum::<u64>()
        );
        let mut cyclic = public.clone();
        cyclic.targets[0][0] = ((u32::from(cyclic.targets[0][0]) + 1) % Q as u32) as u16;
        assert!(check_algebraic_statement(&cyclic, &witness).is_err());
    }

    #[test]
    fn batch_derivation_matches_distinct_schoolbook_witnesses() {
        let batch = 9;
        let mut public = FalconAlgebraicStatement {
            public_keys: Vec::new(),
            targets: Vec::new(),
        };
        let mut expected = FalconAlgebraicWitness {
            s1: Vec::new(),
            s2: Vec::new(),
        };
        let mut quotients = Vec::new();
        for instance in 0..batch {
            let h = std::array::from_fn(|j| ((j * 71 + instance * 311) % Q as usize) as u16);
            let s1 = std::array::from_fn(|j| ((j + instance * 3) % 23) as i16 - 11);
            let s2 = std::array::from_fn(|j| ((j * 7 + instance * 5) % 17) as i16 - 8);
            let mut product = vec![0_i64; 2 * N];
            for i in 0..N {
                for j in 0..N {
                    product[i + j] += i64::from(h[i]) * i64::from(s2[j]);
                }
            }
            public.public_keys.push(h);
            public.targets.push(std::array::from_fn(|j| {
                (i64::from(s1[j]) + product[j] - product[N + j]).rem_euclid(Q) as u16
            }));
            quotients.push(
                product[N..2 * N - 1]
                    .iter()
                    .map(|&x| (-x).rem_euclid(Q) as u16)
                    .collect::<Vec<_>>(),
            );
            expected.s1.push(s1);
            expected.s2.push(s2);
        }
        let derived = WitnessData::from_s2(&public, expected.s2.clone()).unwrap();
        assert_eq!(derived.witness, expected);
        assert_eq!(derived.quotients, quotients);
        assert_eq!(derived.slacks, check_norms(&expected).unwrap());
        let checked = WitnessData::new(&public, expected.clone()).unwrap();
        assert_eq!(checked.quotients, quotients);
        assert_eq!(checked.slacks, derived.slacks);
        assert_eq!(
            FalconAlgebraicWitness::from_s2(&public, expected.s2.clone()).unwrap(),
            expected
        );
    }

    #[test]
    fn signed15_source_decodes_and_padding_is_zero() {
        let public = FalconAlgebraicStatement {
            public_keys: vec![[0; N]; 3],
            targets: vec![[0; N]; 3],
        };
        let mut s2 = vec![[0; N]; 3];
        s2[0][0] = 3000;
        s2[1][8] = -4000;
        s2[2][N - 1] = if N == 1024 { 8000 } else { 5000 };
        let witness = FalconAlgebraicWitness::from_s2(&public, s2.clone()).unwrap();
        let data = WitnessData::new(&public, witness).unwrap();
        let combined = WitnessData::from_s2(&public, s2.clone()).unwrap();
        assert_eq!(combined.witness, data.witness);
        assert_eq!(combined.slacks, data.slacks);
        assert_eq!(combined.quotients, data.quotients);
        let layout = Layout::new(3).unwrap();
        let source = Source::new(layout, &data);
        assert_eq!(Source::new(layout, &combined).rows(), source.rows());
        for i in 0..3 {
            for j in 0..N {
                let offset = i * layout.signature_stride() + coefficient_bit(N, 1, j, 0);
                let low = (0..14)
                    .map(|b| i32::from(source.bit(offset + b)) * (1 << b))
                    .sum::<i32>();
                let decoded = low - i32::from(source.bit(offset + 14)) * (1 << 14);
                assert_eq!(decoded, i32::from(s2[i][j]));
            }
            let slack = (0..SLACK_BITS)
                .map(|bit| {
                    u64::from(source.bit(i * layout.signature_stride() + slack_bit(bit))) << bit
                })
                .sum::<u64>();
            assert_eq!(slack, data.slacks[i]);
        }
        for index in 0..layout.source_bits() {
            if !layout.is_live(index) {
                assert!(!source.bit(index));
            }
        }
        assert_eq!(
            (0..layout.source_bits())
                .filter(|&index| layout.is_live(index))
                .count(),
            3 * super::super::LIVE_BITS
        );
    }

    #[test]
    fn aligned_addresses_are_disjoint_and_preserve_signed15_boundaries() {
        let layout = Layout::new(1).unwrap();
        let mut data = WitnessData {
            witness: FalconAlgebraicWitness {
                s1: vec![[0; N]],
                s2: vec![[0; N]],
            },
            slacks: vec![(1 << SLACK_BITS) - 1],
            quotients: vec![vec![0; N - 1]],
        };
        // Encoding boundaries are tested independently of the norm gate.
        data.witness.s1[0][..4].copy_from_slice(&[-16384, -1, 0, 16383]);
        data.witness.s2[0][N - 4..].copy_from_slice(&[16383, 0, -1, -16384]);
        let source = Source::new(layout, &data);
        let mut seen = vec![false; layout.signature_stride()];
        for side in 0..2 {
            for j in 0..N {
                let mut value = 0i32;
                for bit in 0..15 {
                    let index = coefficient_bit(N, side, j, bit);
                    assert!(!std::mem::replace(&mut seen[index], true));
                    let digit = if bit == 14 { -(1 << 14) } else { 1 << bit };
                    value += i32::from(source.bit(index)) * digit;
                }
                assert_eq!(
                    value,
                    i32::from([&data.witness.s1, &data.witness.s2][side][0][j])
                );
            }
        }
        for bit in 0..SLACK_BITS {
            let index = slack_bit(bit);
            assert!(!std::mem::replace(&mut seen[index], true));
            assert!(source.bit(index));
        }
        assert_eq!(
            seen.iter().filter(|&&live| !live).count(),
            2 * N - SLACK_BITS
        );
        for (index, live) in seen.into_iter().enumerate() {
            assert_eq!(live, layout.is_live(index));
            if !live {
                assert!(!source.bit(index));
            }
        }
        for index in 0..layout.source_bits() {
            if !layout.is_live(index) {
                assert!(!source.bit(index));
            }
        }
    }

    #[test]
    fn exact_norm_boundary_and_coefficient_limits() {
        let public = FalconAlgebraicStatement {
            public_keys: vec![[0; N]],
            targets: vec![[0; N]],
        };
        let mut s2 = [0; N];
        let mut remaining = BETA_SQUARED;
        let mut used = 0;
        while remaining != 0 {
            let coefficient = remaining.isqrt();
            s2[used] = coefficient as i16;
            remaining -= coefficient * coefficient;
            used += 1;
        }
        let witness = FalconAlgebraicWitness {
            s1: vec![[0; N]],
            s2: vec![s2],
        };
        let data = WitnessData::new(&public, witness.clone()).unwrap();
        assert_eq!(data.slacks, vec![0]);
        let combined = WitnessData::from_s2(&public, witness.s2.clone()).unwrap();
        assert_eq!(combined.witness, witness);
        assert_eq!(combined.slacks, vec![0]);
        let mut excessive = witness;
        excessive.s2[0][used] = 1;
        assert!(
            matches!(WitnessData::from_s2(&public, excessive.s2.clone()),
            Err(FalconError::NormTooLarge { actual, bound }) if actual == bound + 1)
        );
        assert!(matches!(WitnessData::new(&public, excessive),
            Err(FalconError::NormTooLarge { actual, bound }) if actual == bound + 1));

        for coefficient in [-16384, 16383] {
            let mut values = [0; N];
            values[0] = coefficient;
            check_coefficients(&[values]).unwrap();
        }
        for coefficient in [-16385, 16384, i16::MIN, i16::MAX] {
            let mut values = [0; N];
            values[0] = coefficient;
            assert!(check_coefficients(&[values]).is_err());
            assert!(WitnessData::from_s2(&public, vec![values]).is_err());
        }
        // The algebraic relation also accepts non-centered s1 when its norm fits.
        let mut public = public;
        if N == 1024 {
            public.targets[0][0] = 7000;
            let mut s1 = [0; N];
            s1[0] = 7000;
            check_algebraic_statement(
                &public,
                &FalconAlgebraicWitness {
                    s1: vec![s1],
                    s2: vec![[0; N]],
                },
            )
            .unwrap();
            public.targets[0][0] = Q as u16;
            assert!(public.validate(1).is_err());
        }
    }

    #[test]
    fn combined_preparation_rejects_invalid_shapes_and_public_coefficients() {
        for batch in [0, 1025] {
            let public = FalconAlgebraicStatement {
                public_keys: vec![[0; N]; batch],
                targets: vec![[0; N]; batch],
            };
            assert!(matches!(
                WitnessData::from_s2(&public, vec![[0; N]; batch]),
                Err(FalconError::InvalidBatchCapacity)
            ));
        }

        for (keys, targets) in [(0, 1), (2, 1), (1, 0), (1, 2)] {
            let public = FalconAlgebraicStatement {
                public_keys: vec![[0; N]; keys],
                targets: vec![[0; N]; targets],
            };
            assert!(matches!(
                WitnessData::from_s2(&public, vec![[0; N]]),
                Err(FalconError::InvalidBatchCapacity)
            ));
        }

        for noncanonical_key in [false, true] {
            let mut public = FalconAlgebraicStatement {
                public_keys: vec![[0; N]],
                targets: vec![[0; N]],
            };
            if noncanonical_key {
                public.public_keys[0][N - 1] = Q as u16;
            } else {
                public.targets[0][N - 1] = Q as u16;
            }
            assert!(WitnessData::from_s2(&public, vec![[0; N]]).is_err());
        }
    }
}
