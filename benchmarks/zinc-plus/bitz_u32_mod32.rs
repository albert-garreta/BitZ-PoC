#![allow(clippy::arithmetic_side_effects)]

use crypto_primitives::{
    ConstIntRing, ConstIntSemiring, FixedSemiring, PrimeField, crypto_bigint_int::Int,
    crypto_bigint_uint::Uint,
};
use std::{fmt::Debug, hint::black_box, marker::PhantomData, ops::Neg, time::Instant};
use zinc_poly::{
    ConstCoeffBitWidth, Polynomial,
    mle::DenseMultilinearExtension,
    univariate::{
        binary::{BinaryPoly, BinaryPolyInnerProduct},
        dense::{DensePolyInnerProduct, DensePolynomial},
    },
};
use zinc_primality::{MillerRabin, PrimalityTest};
use zinc_protocol::{Proof, ZincPlusPiop, ZincTypes};
use zinc_transcript::traits::{ConstTranscribable, Transcribable};
use zinc_uair::{
    ConstraintBuilder, LookupColumnSpec, LookupTableType, PublicColumnLayout, TotalColumnLayout,
    TraceRow, Uair, UairSignature, UairTrace,
    ideal::ImpossibleIdeal,
    ideal_collector::IdealOrZero,
};
use zinc_utils::{
    field::runtime_monty::Fp,
    from_ref::FromRef,
    inner_product::{InnerProduct, MBSInnerProduct, ScalarProduct},
    mul_by_scalar::MulByScalar,
    named::Named,
};
use zip_plus::{
    code::{
        LinearCode,
        iprs::{IprsCode, IprsCodeNarrow, PnttConfigF65537},
    },
    pcs::structs::{ZipPlus, ZipTypes},
    utils::eprint_proof_size,
};

const PERFORM_CHECKS: bool = if cfg!(feature = "unchecked") {
    zinc_utils::UNCHECKED
} else {
    zinc_utils::CHECKED
};

/// Inverse rate, following the repo convention: default 4, `iprs-rate-1-8`
/// switches to 8.
const REP: usize = if cfg!(feature = "iprs-rate-1-16") {
    16
} else if cfg!(feature = "iprs-rate-1-8") {
    8
} else {
    4
};

/// Target security level (100 by default; `sec-114` / `sec-128` features).
const SECURITY_BITS: usize = zinc_protocol::SECURITY_BITS;

/// Openings for `SECURITY_BITS` at rate `1/REP` (150 / 100 / 75 at 100 bits
/// for rates 1/4, 1/8, 1/16; see `zip_plus::pcs::structs::num_column_openings`).
const NUM_COL_OPENINGS_FOR_REP: usize =
    zip_plus::pcs::structs::num_column_openings(REP, SECURITY_BITS);

//
// Generic Zip/Zinc type scaffolding — copied from `protocol/benches/e2e.rs`
// on this branch (same bounds), so the concrete instantiation below is the
// only new part.
//

#[allow(clippy::type_complexity)]
#[derive(Debug, Clone, Copy)]
pub struct GenericBenchZipTypes<
    Eval,
    Cw,
    Fmod,
    PrimeTest,
    Chal,
    Pt,
    CombR,
    Comb,
    EvalDotChal,
    CombDotChal,
    ArrCombRDotChal,
>(
    PhantomData<(
        Eval,
        Cw,
        Fmod,
        PrimeTest,
        Chal,
        Pt,
        CombR,
        Comb,
        EvalDotChal,
        CombDotChal,
        ArrCombRDotChal,
    )>,
);

impl<Eval, Cw, Fmod, PrimeTest, Chal, Pt, CombR, Comb, EvalDotChal, CombDotChal, ArrCombRDotChal>
    ZipTypes
    for GenericBenchZipTypes<
        Eval,
        Cw,
        Fmod,
        PrimeTest,
        Chal,
        Pt,
        CombR,
        Comb,
        EvalDotChal,
        CombDotChal,
        ArrCombRDotChal,
    >
where
    Eval: ConstCoeffBitWidth + Default + Named + Clone + Debug + Send + Sync,
    Cw: FixedSemiring + ConstCoeffBitWidth + ConstTranscribable + FromRef<Eval> + Named + Copy,
    Fmod: ConstIntSemiring + ConstTranscribable + Named,
    PrimeTest: PrimalityTest<Fmod> + Send + Sync,
    Chal: ConstIntRing + ConstTranscribable + Named,
    Pt: ConstIntRing,
    CombR: ConstIntRing
        + Neg<Output = CombR>
        + ConstTranscribable
        + FromRef<CombR>
        + for<'a> MulByScalar<&'a Chal>,
    Comb: FixedSemiring + Polynomial<CombR> + FromRef<Eval> + FromRef<Cw> + Named,
    EvalDotChal: InnerProduct<Eval, Chal, CombR> + Clone + Debug + Send + Sync,
    CombDotChal: InnerProduct<Comb, Chal, CombR> + Clone + Debug + Send + Sync,
    ArrCombRDotChal: InnerProduct<[CombR], Chal, CombR> + Clone + Debug + Send + Sync,
{
    const NUM_COLUMN_OPENINGS: usize = NUM_COL_OPENINGS_FOR_REP;
    type Eval = Eval;
    type Cw = Cw;
    type Fmod = Fmod;
    type PrimeTest = PrimeTest;
    type Chal = Chal;
    type Pt = Pt;
    type CombR = CombR;
    type Comb = Comb;
    type EvalDotChal = EvalDotChal;
    type CombDotChal = CombDotChal;
    type ArrCombRDotChal = ArrCombRDotChal;
}

/// Marker selecting the int lane's code: the plain IPRS code, or the
/// narrow-stage variant for 16-bit cells.
trait IntLane<Zt: ZipTypes> {
    type Code: LinearCode<Zt> + BenchCode;
}
#[derive(Clone, Debug)]
struct PlainIprs;
impl<Zt: ZipTypes> IntLane<Zt> for PlainIprs
where
    IprsCode<Zt, PnttConfigF65537, REP, PERFORM_CHECKS>: LinearCode<Zt>,
{
    type Code = IprsCode<Zt, PnttConfigF65537, REP, PERFORM_CHECKS>;
}
#[derive(Clone, Debug)]
struct NarrowIprs;
impl<Zt: ZipTypes> IntLane<Zt> for NarrowIprs
where
    IprsCodeNarrow<Zt, PnttConfigF65537, REP, PERFORM_CHECKS>: LinearCode<Zt>,
{
    type Code = IprsCodeNarrow<Zt, PnttConfigF65537, REP, PERFORM_CHECKS>;
}

#[derive(Clone, Debug)]
struct GenericBenchZincTypes<
    Int,
    CwR,
    Chal,
    Pt,
    BinaryCombR,
    CombR,
    IntCombR,
    Fmod,
    PrimeTest,
    const D: usize,
    IntCode = PlainIprs,
>(
    PhantomData<(
        Int,
        CwR,
        Chal,
        Pt,
        BinaryCombR,
        CombR,
        IntCombR,
        Fmod,
        PrimeTest,
        IntCode,
    )>,
);

impl<Int, CwR, Chal, Pt, BinaryCombR, CombR, IntCombR, Fmod, PrimeTest, const D: usize, IntCode>
    ZincTypes<D>
    for GenericBenchZincTypes<
        Int,
        CwR,
        Chal,
        Pt,
        BinaryCombR,
        CombR,
        IntCombR,
        Fmod,
        PrimeTest,
        D,
        IntCode,
    >
where
    IntCode: IntLane<
            GenericBenchZipTypes<
                Int,
                CwR,
                Fmod,
                PrimeTest,
                Chal,
                Pt,
                IntCombR,
                IntCombR,
                ScalarProduct,
                ScalarProduct,
                MBSInnerProduct,
            >,
        > + Clone
        + Debug
        + Send
        + Sync
        + 'static,
    Int: ConstIntSemiring
        + for<'a> MulByScalar<&'a i64, CwR>
        + Named
        + ConstCoeffBitWidth
        + ConstTranscribable
        + Default
        + Clone
        + Send
        + Sync
        + 'static,
    CwR: FixedSemiring
        + for<'a> MulByScalar<&'a i64>
        + ConstCoeffBitWidth
        + ConstTranscribable
        + Named
        + FromRef<Int>
        + FromRef<CwR>
        + Copy,
    Chal: ConstIntRing + ConstTranscribable + Named,
    Pt: ConstIntRing,
    BinaryCombR: ConstIntRing
        + Polynomial<BinaryCombR>
        + Neg<Output = BinaryCombR>
        + for<'a> MulByScalar<&'a i64>
        + for<'a> MulByScalar<&'a Chal>
        + ConstTranscribable
        + Named
        + FromRef<i64>
        + FromRef<Int>
        + FromRef<CwR>
        + FromRef<Chal>
        + FromRef<BinaryCombR>,
    CombR: ConstIntRing
        + Polynomial<CombR>
        + Neg<Output = CombR>
        + for<'a> MulByScalar<&'a i64>
        + for<'a> MulByScalar<&'a Chal>
        + ConstTranscribable
        + Named
        + FromRef<i64>
        + FromRef<Int>
        + FromRef<CwR>
        + FromRef<Chal>
        + FromRef<CombR>,
    IntCombR: ConstIntRing
        + Polynomial<IntCombR>
        + Neg<Output = IntCombR>
        + for<'a> MulByScalar<&'a i64>
        + for<'a> MulByScalar<&'a Chal>
        + ConstTranscribable
        + Named
        + FromRef<i64>
        + FromRef<Int>
        + FromRef<CwR>
        + FromRef<Chal>
        + FromRef<IntCombR>,
    Fmod: ConstIntSemiring + ConstTranscribable + Named,
    PrimeTest: PrimalityTest<Fmod> + Debug + Send + Sync,
{
    type Int = Int;
    type Chal = Chal;
    type Pt = Pt;
    type Fmod = Fmod;
    type PrimeTest = PrimeTest;

    type BinaryZt = GenericBenchZipTypes<
        BinaryPoly<D>,
        DensePolynomial<i64, D>,
        Fmod,
        PrimeTest,
        Chal,
        Pt,
        BinaryCombR,
        DensePolynomial<BinaryCombR, D>,
        BinaryPolyInnerProduct<Chal, D>,
        DensePolyInnerProduct<BinaryCombR, Chal, BinaryCombR, MBSInnerProduct, D>,
        MBSInnerProduct,
    >;
    type ArbitraryZt = GenericBenchZipTypes<
        DensePolynomial<Int, D>,
        DensePolynomial<CwR, D>,
        Fmod,
        PrimeTest,
        Chal,
        Pt,
        CombR,
        DensePolynomial<CombR, D>,
        DensePolyInnerProduct<Int, Chal, CombR, MBSInnerProduct, D>,
        DensePolyInnerProduct<CombR, Chal, CombR, MBSInnerProduct, D>,
        MBSInnerProduct,
    >;
    type IntZt = GenericBenchZipTypes<
        Int,
        CwR,
        Fmod,
        PrimeTest,
        Chal,
        Pt,
        IntCombR,
        IntCombR,
        ScalarProduct,
        ScalarProduct,
        MBSInnerProduct,
    >;

    type BinaryLc = IprsCode<Self::BinaryZt, PnttConfigF65537, REP, PERFORM_CHECKS>;
    type ArbitraryLc = IprsCode<Self::ArbitraryZt, PnttConfigF65537, REP, PERFORM_CHECKS>;
    type IntLc = <IntCode as IntLane<Self::IntZt>>::Code;
}

/// How the harness builds a lane's linear code for a given row length.
trait BenchCode: Sized {
    fn bench_new(row_len: usize) -> Self;
}

impl<Zt: ZipTypes> BenchCode for IprsCode<Zt, PnttConfigF65537, REP, PERFORM_CHECKS> {
    fn bench_new(row_len: usize) -> Self {
        IprsCode::new_with_optimal_depth(row_len).expect("IPRS params")
    }
}

/// The limb lane's code: the base layer and the first radix-8 stage run
/// over `i64`. With 16-bit cells and the depth-3 code at row length 8192
/// (`base_len = 16`) the base layer stays below `2^35` and the first stage
/// below `2^53`, so one narrow stage is exact (CHECKED-validated at
/// nvars = 13); the second stage would reach `2^71`.
impl<Zt: ZipTypes> BenchCode for IprsCodeNarrow<Zt, PnttConfigF65537, REP, PERFORM_CHECKS> {
    fn bench_new(row_len: usize) -> Self {
        IprsCodeNarrow::new(IprsCode::new_with_optimal_depth(row_len).expect("IPRS params"), 1)
            .expect("narrow IPRS params")
    }
}

//
// Concrete instantiation: 2048-bit integer witness cells.
//

const DEGREE_PLUS_ONE: usize = 32;

/// Width of the transcript-drawn projecting prime, in 64-bit limbs. Every
/// Fiat–Shamir soundness term is `O(size / prime)`: the fingerprinting
/// (random-prime) step spends `≈ (bits of the largest constraint residue,
/// ~2^13 here) / prime`, and the range-check LogUp spends
/// `≈ (#looked-up cells + table size, ~2^22) / prime`, so 128 bits leaves
/// > 100 bits at the default target and 192 bits is used above it.
const FIELD_LIMBS: usize = if SECURITY_BITS > 100 { 3 } else { 2 };

zinc_utils::define_modulus!(LimberBenchSlot, FIELD_LIMBS);
type F = Fp<LimberBenchSlot, FIELD_LIMBS>;

//
// The statement: N independent u32 multiplications mod 2^32, on the BitZ
// native-mul corpus (`benches/mul_e2e_compare/mod32.rs`, domain
// "native-mul/mod32/inputs/v1"), so every scheme in that table proves the
// same operands.
//
// Per row: x·y = z + 2^32·w over the integers, with x, y, z, w each given
// as two little-endian 16-bit limbs in their own int column (8 columns, one
// multiplication per row). Every limb column carries a `Word { width: 16 }`
// lookup, so each cell is a proven integer in [0, 2^16) and each value a
// proven integer in [0, 2^32). Then |x·y − z − 2^32·w| < 2^65 < q for the
// 128-bit projecting prime, so the equation holds over Z and z = x·y mod
// 2^32 exactly. Range-checking w is what makes this sound: without it,
// w = (x·y − z)·2^-32 mod q satisfies the constraint for any z.
//

const LIMB_BITS: u32 = 16;
const LIMBS_PER_VALUE: usize = 2; // 32-bit values in two 16-bit limbs
const COLS: usize = 4 * LIMBS_PER_VALUE; // x | y | z | w
/// Combination-ring limbs, as in the 16-bit-limb MultiSwap statement: the
/// same 16-bit cells and 128-bit challenges over 8 columns instead of 512,
/// so every bound there covers this statement too.
const LIMB_M: usize = 6;

type LimbInt = i64;
type LimbCw = i128;

type U32ZincTypes = GenericBenchZincTypes<
    /* Int         = */ LimbInt,
    /* CwR         = */ LimbCw,
    /* Chal        = */ i128,
    /* Pt          = */ i128,
    /* BinaryCombR = */ Int<5>,
    /* CombR       = */ Int<LIMB_M>,
    /* IntCombR    = */ Int<LIMB_M>,
    /* Fmod        = */ Uint<FIELD_LIMBS>,
    MillerRabin,
    DEGREE_PLUS_ONE,
    // The narrow int code keeps the commit-time encode on i64/i128, but the
    // verifier's `encode_wide` of an alpha-combined row (128-bit alphas over
    // 16-bit cells) overflows those lanes as soon as the matrix has more than
    // one row, so the multi-row geometry this statement needs runs the plain
    // code over the wide combination ring.
    /* int code    = */ PlainIprs,
>;

#[derive(Clone, Debug)]
pub struct U32Mod32Uair;

impl Uair for U32Mod32Uair {
    type Ideal = ImpossibleIdeal;
    /// Degree-0 scalars: the radix 2^16 and the carry weight 2^32.
    type Scalar = DensePolynomial<LimbInt, 1>;

    fn signature() -> UairSignature {
        let total = TotalColumnLayout::new(0, 0, COLS);
        let lookup_specs: Vec<LookupColumnSpec> = (0..COLS)
            .map(|i| LookupColumnSpec {
                column_index: i,
                table_type: LookupTableType::Word { width: LIMB_BITS as usize, chunk_width: None },
            })
            .collect();
        UairSignature::new(total, PublicColumnLayout::default(), vec![], lookup_specs, vec![])
    }

    fn constrain_general<B, FromR, MulByScalar, IFromR>(
        b: &mut B,
        up: TraceRow<B::Expr>,
        _down: TraceRow<B::Expr>,
        _from_ref: FromR,
        mbs: MulByScalar,
        _ideal_from_ref: IFromR,
    ) where
        B: ConstraintBuilder,
        FromR: Fn(&Self::Scalar) -> B::Expr,
        MulByScalar: Fn(&B::Expr, &Self::Scalar) -> Option<B::Expr>,
        IFromR: Fn(&Self::Ideal) -> B::Ideal,
    {
        let radix = DensePolynomial::<LimbInt, 1>::new([1i64 << LIMB_BITS]);
        let carry = DensePolynomial::<LimbInt, 1>::new([1i64 << (2 * LIMB_BITS)]);
        // value = limb_0 + 2^16 · limb_1, Horner from the top limb down.
        let value = |base: usize| -> B::Expr {
            let mut acc = up.int[base + LIMBS_PER_VALUE - 1].clone();
            for i in (0..LIMBS_PER_VALUE - 1).rev() {
                acc = mbs(&acc, &radix).expect("radix mul") + &up.int[base + i];
            }
            acc
        };
        let x = value(0);
        let y = value(LIMBS_PER_VALUE);
        let z = value(2 * LIMBS_PER_VALUE);
        let w = value(3 * LIMBS_PER_VALUE);
        let carry_term = mbs(&w, &carry).expect("2^32 mul");

        b.assert_zero(x * &y - &z - &carry_term);
    }
}

//
// The corpus, byte-for-byte the one every other scheme in the table proves.
//

const INPUT_DOMAIN: &[u8] = b"native-mul/mod32/inputs/v1";
const ROW_DOMAIN: &[u8] = b"native-mul/mod32/rows/v1";

fn corpus_inputs(exponent: usize, seed: u64) -> Vec<(u32, u32)> {
    let mut hasher = blake3::Hasher::new();
    hasher.update(INPUT_DOMAIN);
    hasher.update(&seed.to_le_bytes());
    hasher.update(&u32::try_from(exponent).expect("exponent fits u32").to_le_bytes());
    let mut stream = hasher.finalize_xof();
    (0..1usize << exponent)
        .map(|_| {
            let mut bytes = [0u8; 8];
            stream.fill(&mut bytes);
            (
                u32::from_le_bytes(bytes[..4].try_into().expect("4 bytes")),
                u32::from_le_bytes(bytes[4..].try_into().expect("4 bytes")),
            )
        })
        .collect()
}

/// The table's row digest: blake3 over (x, y, x·y mod 2^32) as the BitZ
/// bench computes it (`mod32::digest_rows`).
fn corpus_digest(inputs: &[(u32, u32)]) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(ROW_DOMAIN);
    hasher.update(&(inputs.len() as u64).to_le_bytes());
    for &(x, y) in inputs {
        for value in [x, y, x.wrapping_mul(y)] {
            hasher.update(&value.to_le_bytes());
        }
    }
    hasher.finalize().to_hex().to_string()
}

/// The same digest recomputed from the proved limbs, after checking that
/// every limb is a 16-bit integer and that x·y = z + 2^32·w holds over the
/// integers: what the trace actually commits to, not what was intended.
fn witness_digest(cols: &[Vec<LimbInt>]) -> String {
    let rows = cols[0].len();
    let value = |base: usize, row: usize| -> u64 {
        let lo = u64::try_from(cols[base][row]).expect("non-negative limb");
        let hi = u64::try_from(cols[base + 1][row]).expect("non-negative limb");
        assert!(lo < 1 << LIMB_BITS && hi < 1 << LIMB_BITS, "16-bit limbs");
        lo | (hi << LIMB_BITS)
    };
    let mut hasher = blake3::Hasher::new();
    hasher.update(ROW_DOMAIN);
    hasher.update(&(rows as u64).to_le_bytes());
    for row in 0..rows {
        let (x, y) = (value(0, row), value(LIMBS_PER_VALUE, row));
        let (z, w) = (value(2 * LIMBS_PER_VALUE, row), value(3 * LIMBS_PER_VALUE, row));
        assert_eq!(x * y, z + (w << (2 * LIMB_BITS)), "x·y = z + 2^32·w");
        for value in [x, y, z] {
            hasher.update(&u32::try_from(value).expect("32-bit value").to_le_bytes());
        }
    }
    hasher.finalize().to_hex().to_string()
}

fn build_columns(inputs: &[(u32, u32)]) -> Vec<Vec<LimbInt>> {
    let mut cols: Vec<Vec<LimbInt>> = (0..COLS).map(|_| Vec::with_capacity(inputs.len())).collect();
    for &(x, y) in inputs {
        let product = u64::from(x) * u64::from(y);
        let values = [u64::from(x), u64::from(y), product & 0xFFFF_FFFF, product >> 32];
        for (k, v) in values.into_iter().enumerate() {
            for i in 0..LIMBS_PER_VALUE {
                let limb = (v >> (LIMB_BITS * u32::try_from(i).expect("limb index"))) & 0xFFFF;
                cols[k * LIMBS_PER_VALUE + i].push(i64::try_from(limb).expect("16-bit limb"));
            }
        }
    }
    cols
}

fn trace_from_columns(cols: Vec<Vec<LimbInt>>) -> UairTrace<'static, LimbInt, LimbInt, DEGREE_PLUS_ONE> {
    UairTrace {
        int: cols
            .into_iter()
            .map(|v| v.into_iter().collect::<DenseMultilinearExtension<_>>())
            .collect::<Vec<_>>()
            .into(),
        ..Default::default()
    }
}

fn median(mut xs: Vec<f64>) -> f64 {
    xs.sort_by(f64::total_cmp);
    xs[xs.len() / 2]
}

/// The LogUp range-check error is bounded by (#looked-up cells + table
/// size)/q: 8·2^L cells over the 2^16-entry word table against the
/// 128-bit projecting prime.
fn logup_bits(exponent: usize) -> f64 {
    let terms = (COLS as f64) * (2f64).powi(exponent as i32) + 65536.0;
    (64 * FIELD_LIMBS) as f64 - terms.log2()
}

fn main() {
    let exponent: usize = std::env::var("EXPONENT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(15);
    let seed: u64 = std::env::var("SEED").ok().and_then(|s| s.parse().ok()).unwrap_or(0);
    let reps: usize = std::env::var("REPS").ok().and_then(|s| s.parse().ok()).unwrap_or(5);
    let poly_size = 1usize << exponent;
    // Two caps on the row length. The IPRS NTT over F65537 needs
    // row_len · REP < 65537 (2^14 at rate 1/4), and the narrow int lane --
    // whose base layer and first radix-8 stage run over i64 -- is exact for
    // 16-bit cells only up to the depth-3 code at row length 8192: at 2^14
    // the encoder overflows the narrow stage (measured, CHECKED). So rows
    // are 8192 wide and the matrix has 2^(L-13) of them.
    let max_row_len = 8192.min((65536 / REP).next_power_of_two() / 2);
    let row_len: usize = std::env::var("ROWLEN")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| poly_size.min(max_row_len));
    assert!(row_len * REP < 65537, "IPRS NTT cap: row_len · REP < 65537");
    assert!(poly_size % row_len == 0, "row_len must divide the column length");

    let threads = if cfg!(feature = "parallel") {
        std::env::var("RAYON_NUM_THREADS").ok().and_then(|s| s.parse().ok()).unwrap_or(0)
    } else {
        1
    };

    type Zt = U32ZincTypes;
    type U = U32Mod32Uair;
    macro_rules! piop {
        () => {
            ZincPlusPiop::<Zt, U, F, DEGREE_PLUS_ONE>
        };
    }

    let inputs = corpus_inputs(exponent, seed);
    let input_digest = corpus_digest(&inputs);

    let t0 = Instant::now();
    let cols = build_columns(&inputs);
    let trace = trace_from_columns(cols.clone());
    let witness_ms = t0.elapsed().as_secs_f64() * 1e3;
    let witness_digest = witness_digest(&cols);
    assert_eq!(witness_digest, input_digest, "proved rows differ from the corpus");
    drop(cols);

    // `setup` takes the whole polynomial size; the matrix row length comes
    // from the linear code, so a code built for `row_len < poly_size` gives
    // the rectangular geometry the NTT cap forces above 2^14.
    let t0 = Instant::now();
    let pp = (
        ZipPlus::<<Zt as ZincTypes<DEGREE_PLUS_ONE>>::BinaryZt, _>::setup(
            poly_size,
            IprsCode::new_with_optimal_depth(row_len).unwrap(),
        ),
        ZipPlus::<<Zt as ZincTypes<DEGREE_PLUS_ONE>>::ArbitraryZt, _>::setup(
            poly_size,
            IprsCode::new_with_optimal_depth(row_len).unwrap(),
        ),
        ZipPlus::<<Zt as ZincTypes<DEGREE_PLUS_ONE>>::IntZt, _>::setup(
            poly_size,
            <<Zt as ZincTypes<DEGREE_PLUS_ONE>>::IntLc as BenchCode>::bench_new(row_len),
        ),
    );
    let setup_ms = t0.elapsed().as_secs_f64() * 1e3;

    // Warm-up proof, then the timed repetitions.
    let proof: Proof<F> = <piop!()>::prove::<false, PERFORM_CHECKS>(
        &pp,
        &trace,
        exponent,
        zinc_protocol::project_scalar_fn,
    )
    .expect("prove failed");

    let mut prove_ms = Vec::with_capacity(reps);
    for _ in 0..reps {
        let t0 = Instant::now();
        let p: Proof<F> = <piop!()>::prove::<false, PERFORM_CHECKS>(
            &pp,
            &trace,
            exponent,
            zinc_protocol::project_scalar_fn,
        )
        .expect("prove failed");
        prove_ms.push(t0.elapsed().as_secs_f64() * 1e3);
        black_box(p);
    }

    let sig = U::signature();
    let public_trace = trace.public(&sig);
    let proj_ideal = |_: &IdealOrZero<<U as Uair>::Ideal>,
                      _: &<F as PrimeField>::Config|
     -> ImpossibleIdeal { unreachable!("only assert_zero constraints") };
    let verify_once = |p: Proof<F>| {
        <piop!()>::verify::<_, PERFORM_CHECKS>(
            &pp,
            p,
            &public_trace,
            exponent,
            zinc_protocol::project_scalar_fn,
            proj_ideal,
        )
    };

    verify_once(proof.clone()).expect("the warm-up proof must verify");
    let mut verify_ms = Vec::with_capacity(reps);
    for _ in 0..reps {
        let p = proof.clone();
        let t0 = Instant::now();
        verify_once(p).expect("verify failed");
        verify_ms.push(t0.elapsed().as_secs_f64() * 1e3);
    }

    // One line per timed repetition, so the table's per-sample checks see
    // real measurements rather than a median repeated.
    let proof_bytes = proof.get_num_bytes();
    for (rep, (prove, verify)) in prove_ms.iter().zip(verify_ms.iter()).enumerate() {
        println!(
            r#"ZINC_TRIAL {{"rep":{},"prove_ms":{:.4},"verify_ms":{:.4},"proof_bytes":{}}}"#,
            rep, prove, verify, proof_bytes,
        );
    }

    println!(
        concat!(
            r#"ZINC_RESULT {{"schema":"zinc-plus/u32-mod32/v1","exponent":{},"seed":{},"reps":{},"#,
            r#""threads":{},"parallel":{},"checks":{},"row_len":{},"num_rows":{},"columns":{},"#,
            r#""limb_bits":{},"inverse_rate":{},"column_openings":{},"security_bits":{},"#,
            r#""prime_bits":{},"grinding_bits":{},"logup_bits":{:.2},"#,
            r#""setup_ms":{:.4},"witness_ms":{:.4},"prove_ms":{:.4},"verify_ms":{:.4},"proof_bytes":{},"#,
            r#""input_digest":"{}","witness_digest":"{}","proof_verified":true}}"#
        ),
        exponent,
        seed,
        reps,
        threads,
        cfg!(feature = "parallel"),
        PERFORM_CHECKS,
        row_len,
        poly_size / row_len,
        COLS,
        LIMB_BITS,
        REP,
        NUM_COL_OPENINGS_FOR_REP,
        SECURITY_BITS,
        64 * FIELD_LIMBS,
        0, // this revision has no proof-of-work grinding
        logup_bits(exponent),
        setup_ms,
        witness_ms,
        median(prove_ms),
        median(verify_ms),
        proof_bytes,
        input_digest,
        witness_digest,
    );

    eprint_proof_size(
        format!("u32-mod32/rate1-{REP}/sec{SECURITY_BITS}/nvars={exponent}/row_len={row_len}"),
        &proof,
    );
}
