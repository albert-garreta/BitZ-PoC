use super::{BETA_SQUARED, FalconError, LIVE_BITS, N, Q, error};
use crate::pcs::IntegerMatrixLayout;
use crate::piop::spartan::falcon_polynomial::integer_polynomial_product;
#[cfg(feature = "parallel")]
use rayon::prelude::*;

/// Public keys and externally computed targets, each in F_12289[X]/(X^1024+1).
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
        let s1 = crate::utils::cfg_into_iter!(0..s2.len())
            .map(|i| {
                let product = product(&statement.public_keys[i], &s2[i]);
                centered_s1(&statement.targets[i], &product)
            })
            .collect();
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
            // 2^20 committed bits, so retain at least eight 2^16-bit slots.
            capacity: batch.next_power_of_two().max(8),
        })
    }
    pub const fn batch(&self) -> usize {
        self.batch
    }
    pub const fn capacity(&self) -> usize {
        self.capacity
    }
    pub const fn signature_stride(&self) -> usize {
        1 << 16
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
            col_vars: 3 + self.capacity.ilog2() as usize,
        }
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
        let signatures = crate::utils::cfg_into_iter!(0..layout.batch())
            .map(|i| {
                let product = product(&public.public_keys[i], &s2[i]);
                let s1 = centered_s1(&public.targets[i], &product);
                let slack = norm_slack(&s1, &s2[i])?;
                Ok((s1, slack, quotient(&product)))
            })
            .collect::<Result<Vec<_>, FalconError>>()?;
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
        let quotients = crate::utils::cfg_into_iter!(0..layout.batch())
            .map(|i| {
                let product = product(&public.public_keys[i], &witness.s2[i]);
                for j in 0..N {
                    let residual = i64::from(public.targets[i][j]) - product[j] + product[N + j]
                        - i64::from(witness.s1[i][j]);
                    if residual.rem_euclid(Q) != 0 {
                        return Err(error("invalid algebraic ring witness"));
                    }
                }
                Ok(quotient(&product))
            })
            .collect::<Result<Vec<_>, FalconError>>()?;
        Ok(Self {
            witness,
            slacks,
            quotients,
        })
    }
}

fn product(h: &[u16; N], s2: &[i16; N]) -> Vec<i64> {
    let left: Vec<_> = h.iter().map(|&x| i64::from(x)).collect();
    let right: Vec<_> = s2.iter().map(|&x| i64::from(x)).collect();
    integer_polynomial_product(&left, &right)
}

fn centered_s1(target: &[u16; N], product: &[i64]) -> [i16; N] {
    std::array::from_fn(|j| {
        let residue = (i64::from(target[j]) - product[j] + product[N + j]).rem_euclid(Q);
        (if residue > Q / 2 {
            residue - Q
        } else {
            residue
        }) as i16
    })
}

fn quotient(product: &[i64]) -> Vec<u16> {
    // t-h*s2-s1 = (X^N+1)D modulo q, with D = -high(h*s2).
    product[N..2 * N - 1]
        .iter()
        .map(|&v| (-v).rem_euclid(Q) as u16)
        .collect()
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
        let p = layout.bitz_params();
        let mut source = Self {
            layout,
            rows: vec![vec![0; p.rows() / 64]; p.cols()],
        };
        for i in 0..layout.batch() {
            let base = i * layout.signature_stride();
            for j in 0..N {
                for (offset, coefficient) in
                    [(0, data.witness.s1[i][j]), (15 * N, data.witness.s2[i][j])]
                {
                    source.put(
                        base + offset + 15 * j,
                        15,
                        (i32::from(coefficient) & 0x7fff) as u64,
                    );
                }
            }
            source.put(base + 30 * N, 27, data.slacks[i]);
        }
        source
    }
    fn put(&mut self, offset: usize, width: usize, value: u64) {
        for b in 0..width {
            if value >> b & 1 != 0 {
                let index = offset + b;
                self.rows[index >> 13][(index & 8191) >> 6] |= 1 << (index & 63);
            }
        }
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

#[cfg(test)]
mod tests {
    use super::*;

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
    fn signed15_source_decodes_and_padding_is_zero() {
        let public = FalconAlgebraicStatement {
            public_keys: vec![[0; N]; 3],
            targets: vec![[0; N]; 3],
        };
        let mut s2 = vec![[0; N]; 3];
        s2[0][0] = 3000;
        s2[1][8] = -4000;
        s2[2][N - 1] = 8000;
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
                let offset = i * 65536 + 15 * N + 15 * j;
                let low = (0..14)
                    .map(|b| i32::from(source.bit(offset + b)) * (1 << b))
                    .sum::<i32>();
                let decoded = low - i32::from(source.bit(offset + 14)) * (1 << 14);
                assert_eq!(decoded, i32::from(s2[i][j]));
            }
        }
        for index in 0..layout.source_bits() {
            if index / 65536 >= 3 || index % 65536 >= LIVE_BITS {
                assert!(!source.bit(index));
            }
        }
    }

    #[test]
    fn exact_norm_boundary_and_coefficient_limits() {
        let public = FalconAlgebraicStatement {
            public_keys: vec![[0; N]], targets: vec![[0; N]],
        };
        let mut s2 = [0; N];
        s2[..7].copy_from_slice(&[8382, 85, 9, 3, 1, 1, 1]);
        let witness = FalconAlgebraicWitness { s1: vec![[0; N]], s2: vec![s2] };
        let data = WitnessData::new(&public, witness.clone()).unwrap();
        assert_eq!(data.slacks, vec![0]);
        let combined = WitnessData::from_s2(&public, witness.s2.clone()).unwrap();
        assert_eq!(combined.witness, witness);
        assert_eq!(combined.slacks, vec![0]);
        let mut excessive = witness;
        excessive.s2[0][7] = 1;
        assert!(matches!(WitnessData::from_s2(&public, excessive.s2.clone()),
            Err(FalconError::NormTooLarge { actual, bound }) if actual == bound + 1));
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
        public.targets[0][0] = 7000;
        let mut s1 = [0; N]; s1[0] = 7000;
        check_algebraic_statement(&public,
            &FalconAlgebraicWitness { s1: vec![s1], s2: vec![[0; N]] }).unwrap();
        public.targets[0][0] = Q as u16;
        assert!(public.validate(1).is_err());
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
