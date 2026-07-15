use crate::code::{LinearCode, zip_types::ZipTypes, zip_utils::shuffle_seeded};
use crypto_primitives::PrimeField;
use num_traits::CheckedAdd;
use std::{fmt::Debug, marker::PhantomData, ops::AddAssign};
use crate::poly::ConstCoeffBitWidth;
use crate::utils::{add, from_ref::FromRef, mul};

pub trait RaaConfig: Copy + Send + Sync {
    /// Whether to permute the codeword in place, instead of copying it using a
    /// precomputed permutation.
    const PERMUTE_IN_PLACE: bool;
    /// Whether to check for overflows during encoding
    // TODO: Unify with `CHECK_FOR_OVERFLOW` in `zinc_poly`
    const CHECK_FOR_OVERFLOWS: bool;
}

/// Implementation of a repeat-accumulate-accumulate (RAA) codes over the binary
/// field, as defined by the Blaze paper (https://eprint.iacr.org/2024/1609)
#[derive(Clone)]
pub struct RaaCode<Zt: ZipTypes, Config: RaaConfig, const REP: usize> {
    pub(crate) row_len: usize,
    /// Randomness seed for the first permutation
    pub(crate) perm_1_seed: u64,

    /// Randomness seed for the second permutation
    pub(crate) perm_2_seed: u64,

    /// First permutation
    pub(crate) perm_1: Vec<usize>,

    /// Second permutation
    pub(crate) perm_2: Vec<usize>,

    phantom: PhantomData<(Zt, Config)>,
}

impl<Zt: ZipTypes, Config: RaaConfig, const REP: usize> RaaCode<Zt, Config, REP> {
    pub fn new(row_len: usize) -> Self {
        assert!(
            REP.is_power_of_two(),
            "Repetition factor must be a power of two"
        );
        assert!(
            row_len.is_power_of_two(),
            "Row length must be a power of two"
        );

        // Width of each entry in codeword vector, in bits.
        // For RAA it's initial_bits + 2*log2(codeword_len),
        // where codeword_len = row_len * REP and the factor of 2
        // comes from the two accumulation steps.
        let codeword_width_bits = {
            let initial_bits =
                u32::try_from(Zt::Eval::COEFF_BIT_WIDTH).expect("Size of EvalR type is too large");

            let row_len_log = row_len.ilog2();
            let rep_factor_log = REP.ilog2();
            add!(
                initial_bits,
                add!(mul!(row_len_log, 2), mul!(rep_factor_log, 2))
            )
        };
        let codeword_type_bits =
            u32::try_from(Zt::Cw::COEFF_BIT_WIDTH).expect("Size of CwR type is too large");
        assert!(
            codeword_type_bits >= codeword_width_bits,
            "Cannot fit {codeword_width_bits}-bit wide codeword entries in {} bits entries",
            codeword_type_bits
        );

        // We don't need a secure/unpredictable randomness here, so use fixed seeds
        const PERM_1_SEED: u64 = 1;
        const PERM_2_SEED: u64 = 2;

        let codeword_len = mul!(row_len, REP);

        let mut perm_1: Vec<usize> = (0..codeword_len).collect();
        shuffle_seeded(&mut perm_1, PERM_1_SEED);
        let mut perm_2: Vec<usize> = (0..codeword_len).collect();
        shuffle_seeded(&mut perm_2, PERM_2_SEED);

        Self {
            row_len,
            perm_1_seed: PERM_1_SEED,
            perm_2_seed: PERM_2_SEED,
            perm_1,
            perm_2,
            phantom: PhantomData,
        }
    }

    /// Do the actual encoding, as per RAA spec
    fn encode_inner<In, Out>(&self, row: &[In]) -> Vec<Out>
    where
        Out: CheckedAdd + for<'a> AddAssign<&'a Out> + FromRef<In> + Clone,
    {
        debug_assert_eq!(
            row.len(),
            self.row_len,
            "Row length must match the code's row length"
        );

        let mut result: Vec<Out> = repeat(row, REP);
        if Config::PERMUTE_IN_PLACE {
            shuffle_seeded(&mut result, self.perm_1_seed);
        } else {
            result = clone_shuffled(&result, &self.perm_1);
        }
        if Config::CHECK_FOR_OVERFLOWS {
            accumulate(&mut result);
        } else {
            accumulate_unchecked(&mut result);
        }
        if Config::PERMUTE_IN_PLACE {
            shuffle_seeded(&mut result, self.perm_2_seed);
        } else {
            result = clone_shuffled(&result, &self.perm_2);
        }
        if Config::CHECK_FOR_OVERFLOWS {
            accumulate(&mut result);
        } else {
            accumulate_unchecked(&mut result);
        }
        debug_assert_eq!(result.len(), self.codeword_len());
        result
    }
}

impl<Zt: ZipTypes, Config: RaaConfig, const REP: usize> LinearCode<Zt>
    for RaaCode<Zt, Config, REP>
{
    const REPETITION_FACTOR: usize = REP;

    fn row_len(&self) -> usize {
        self.row_len
    }

    #[allow(clippy::arithmetic_side_effects)]
    fn codeword_len(&self) -> usize {
        self.row_len * REP
    }

    fn params_string(&self) -> String {
        format!("row_len={}, rate=1/{REP}", self.row_len())
    }

    fn encode(&self, row: &[Zt::Eval]) -> Vec<Zt::Cw> {
        self.encode_inner(row)
    }

    fn encode_wide(&self, row: &[Zt::CombR]) -> Vec<Zt::CombR> {
        self.encode_inner(row)
    }

    fn encode_f<F>(&self, row: &[F]) -> Vec<F>
    where
        F: PrimeField + FromRef<F>,
    {
        self.encode_inner(row)
    }
}

impl<Zt: ZipTypes, Config: RaaConfig, const REP: usize> Debug for RaaCode<Zt, Config, REP> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RaaCode")
            .field("row_len", &self.row_len)
            .field("perm_1_seed", &self.perm_1_seed)
            .field("perm_2_seed", &self.perm_2_seed)
            .finish()
    }
}

impl<Zt: ZipTypes, Config: RaaConfig, const REP: usize> PartialEq for RaaCode<Zt, Config, REP> {
    fn eq(&self, other: &Self) -> bool {
        self.row_len == other.row_len
            && self.perm_1_seed == other.perm_1_seed
            && self.perm_2_seed == other.perm_2_seed
    }
}

impl<Zt: ZipTypes, Config: RaaConfig, const REP: usize> Eq for RaaCode<Zt, Config, REP> {}

/// Repeat the given slice N times, e.g `[1,2,3] => [1,2,3,1,2,3]`
#[allow(clippy::arithmetic_side_effects)]
pub(crate) fn repeat<In, Out: FromRef<In> + Clone>(
    input: &[In],
    repetition_factor: usize,
) -> Vec<Out> {
    input
        .iter()
        .map(Out::from_ref)
        .cycle()
        .take(input.len() * repetition_factor)
        .collect()
}

/// Perform an operation equivalent to multiplying the slice in-place by the
/// accumulation matrix from the RAA code - a lower triangular matrix of the
/// appropriate size, i.e. a matrix looking like this:
///
/// ```text
/// 1 0 0 0
/// 1 1 0 0
/// 1 1 1 0
/// 1 1 1 1
/// ```
#[allow(clippy::arithmetic_side_effects)] // Clippy is too dumb to realize `i - 1` is safe here
pub(crate) fn accumulate<I>(input: &mut [I])
where
    I: CheckedAdd + Clone,
{
    if let Some(first) = input.first().cloned() {
        let mut acc = first;
        for curr in input.iter_mut().skip(1) {
            acc = add!(*curr, acc, "Accumulation overflow");
            *curr = acc.clone();
        }
    }
}

#[allow(clippy::arithmetic_side_effects)]
pub(crate) fn accumulate_unchecked<I>(input: &mut [I])
where
    I: for<'a> AddAssign<&'a I> + Clone,
{
    if let Some(first) = input.first().cloned() {
        let mut acc = first;
        for i in 1..input.len() {
            // Avoid bound checking
            unsafe {
                acc += input.get_unchecked(i);
                *input.get_unchecked_mut(i) = acc.clone();
            };
        }
    }
}

/// Clone the data using a precomputed permutation.
pub(crate) fn clone_shuffled<T>(data: &[T], perm: &[usize]) -> Vec<T>
where
    T: Clone,
{
    perm.iter().map(|&i| data[i].clone()).collect()
}
