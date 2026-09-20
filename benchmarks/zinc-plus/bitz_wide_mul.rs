//! Zinc+ rows of the u64 and u128 multiplication tables.
//!
//! A bench for zinc-plus (copied into `protocol/benches/`, see the README
//! beside this file) proving this repository's u64 / u128 corpora — the exact
//! operand pairs `examples/mul_corpus_export.rs` writes — as one integer
//! constraint `x·y = z` per multiplication over `4L` int columns of 16-bit
//! limbs (`L = 4` for u64, `8` for u128: `x` and `y` in `L` limbs each, the
//! full product `z` in `2L`), every column range-checked by a
//! `Word { width: 16 }` GKR-LogUp lookup. The proved limbs are recombined
//! and digested exactly as the BitZ harness digests its assignment
//! (`bitz/u64-mul-compare/integer-witness/v1`,
//! `bitz/u128-mul-compare/integer-witness/v1`), and the run refuses to report
//! unless that digest equals the corpus manifest's.
//!
//! Soundness of the wide relation is Zinc+'s random-prime fingerprinting:
//! the residue `x·y - z` is below `2^(64L+1)` bits, so a nonzero residue is
//! divisible by the transcript-drawn 128-bit prime with probability about
//! `(64L+1) / 2^121`; the deterministic `|residue| < q` argument of the u32
//! bench does not apply and is not needed. The binding term is the LogUp
//! one, `≈ (4L · 2^exponent + 2^16) / 2^prime_bits`, reported per run.
//!
//! Environment: `CORPUS` (the `.json` manifest from `mul_corpus_export`,
//! `.bin` beside it), `REPS` (default 5), `ROWLEN` (default min(2^exponent,
//! 8192)); threads come from rayon's `RAYON_NUM_THREADS` under `--features
//! parallel`. The generic Zip/Zinc scaffolding is the u32 bench's, verbatim.
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

/// Proof-of-work bits this revision grinds above 100 bits (0 at the default
/// target); openings are sized for the remaining bits, as the repo's own
/// benches do.
const GRINDING_BITS: usize = zinc_protocol::GRINDING_BITS;

/// Openings for `SECURITY_BITS - GRINDING_BITS` at rate `1/REP` (150 / 100 / 75
/// at 100 bits for rates 1/4, 1/8, 1/16; see `zip_plus::pcs::structs::num_column_openings`).
const NUM_COL_OPENINGS_FOR_REP: usize =
    zip_plus::pcs::structs::num_column_openings(REP, SECURITY_BITS - GRINDING_BITS);

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
// Concrete instantiation: 16-bit limb columns of the full product.
//

const DEGREE_PLUS_ONE: usize = 32;

/// Width of the transcript-drawn projecting prime, in 64-bit limbs (128 bits
/// at the 100-bit target; 192 above it, as the repo's own benches choose).
const FIELD_LIMBS: usize = if SECURITY_BITS - GRINDING_BITS > 100 || SECURITY_BITS > 117 { 3 } else { 2 };

zinc_utils::define_modulus!(WideMulBenchSlot, FIELD_LIMBS);
type F = Fp<WideMulBenchSlot, FIELD_LIMBS>;

const LIMB_BITS: u32 = 16;
/// Combination-ring limbs: 255 signed bits cover the 16-bit cells, 128-bit
/// challenges and at most 32 columns with room to spare (the MultiSwap bench
/// validates the same width over 512 columns).
const LIMB_M: usize = 4;

type LimbInt = i64;
type LimbCw = i128;

type WideZincTypes = GenericBenchZincTypes<
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
    /* int code    = */ PlainIprs,
>;

/// `x·y = z` over `L`-limb operands: columns `x_0..x_{L-1} | y_0..y_{L-1} | z_0..z_{2L-1}`.
#[derive(Clone, Debug)]
pub struct WideMulUair<const L: usize>;

impl<const L: usize> WideMulUair<L> {
    const COLS: usize = 4 * L;
}

impl<const L: usize> Uair for WideMulUair<L> {
    type Ideal = ImpossibleIdeal;
    /// Degree-0 scalar: the radix 2^16 alone (no carry weight is needed
    /// because the whole product is recombined limb by limb).
    type Scalar = DensePolynomial<LimbInt, 1>;

    fn signature() -> UairSignature {
        let total = TotalColumnLayout::new(0, 0, Self::COLS);
        let lookup_specs: Vec<LookupColumnSpec> = (0..Self::COLS)
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
        // value = Σ limb_i · 2^(16 i), Horner from the top limb down.
        let value = |base: usize, limbs: usize| -> B::Expr {
            let mut acc = up.int[base + limbs - 1].clone();
            for i in (0..limbs - 1).rev() {
                acc = mbs(&acc, &radix).expect("radix mul") + &up.int[base + i];
            }
            acc
        };
        let x = value(0, L);
        let y = value(L, L);
        let z = value(2 * L, 2 * L);
        b.assert_zero(x * &y - &z);
    }
}

//
// The corpus: the operand pairs the BitZ harness proves, from its exported
// manifest, and the assignment digest it records for them.
//

struct Corpus {
    workload: String,
    log_n: usize,
    seed: u64,
    corpus_digest: String,
    pairs: Vec<(u128, u128)>,
}

fn json_field<'a>(json: &'a str, key: &str) -> &'a str {
    let needle = format!("\"{key}\":");
    let start = json.find(&needle).unwrap_or_else(|| panic!("corpus manifest lacks {key}")) + needle.len();
    let rest = json[start..].trim_start();
    if let Some(quoted) = rest.strip_prefix('"') {
        &quoted[..quoted.find('"').expect("closing quote")]
    } else {
        let end = rest.find(|c: char| c == ',' || c == '}' || c.is_whitespace()).unwrap_or(rest.len());
        &rest[..end]
    }
}

fn read_corpus(manifest: &std::path::Path) -> Corpus {
    let json = std::fs::read_to_string(manifest).expect("read corpus manifest");
    assert_eq!(json_field(&json, "schema"), "bitz/mul-corpus/v1", "corpus manifest schema");
    let workload = json_field(&json, "workload").to_string();
    let operand_bytes: usize = json_field(&json, "operand_bytes").parse().expect("operand_bytes");
    assert!(
        (workload == "u64" && operand_bytes == 8) || (workload == "u128" && operand_bytes == 16),
        "this bench proves u64 or u128 corpora, got {workload}"
    );
    let bin = manifest.with_file_name(json_field(&json, "file"));
    let bytes = std::fs::read(&bin).expect("read corpus operands");
    assert_eq!(blake3::hash(&bytes).to_hex().to_string(), json_field(&json, "file_blake3"), "corpus file hash");
    let pairs: Vec<(u128, u128)> = bytes
        .chunks_exact(2 * operand_bytes)
        .map(|pair| {
            let (x, y) = pair.split_at(operand_bytes);
            let word = |b: &[u8]| {
                let mut buf = [0u8; 16];
                buf[..b.len()].copy_from_slice(b);
                u128::from_le_bytes(buf)
            };
            (word(x), word(y))
        })
        .collect();
    let log_n: usize = json_field(&json, "log_n").parse().expect("log_n");
    assert_eq!(pairs.len(), 1usize << log_n, "corpus length");
    Corpus {
        workload,
        log_n,
        seed: json_field(&json, "seed").parse().expect("seed"),
        corpus_digest: json_field(&json, "corpus_digest").to_string(),
        pairs,
    }
}

/// The limb columns of the statement for the corpus pairs.
fn build_columns<const L: usize>(pairs: &[(u128, u128)]) -> Vec<Vec<LimbInt>> {
    let cols_len = 4 * L;
    let mut cols: Vec<Vec<LimbInt>> = (0..cols_len).map(|_| Vec::with_capacity(pairs.len())).collect();
    for &(x, y) in pairs {
        let (lo, hi) = full_product(x, y);
        let limb = |v: u128, i: usize| ((v >> (LIMB_BITS as usize * i)) & 0xFFFF) as i64;
        for i in 0..L {
            cols[i].push(limb(x, i));
            cols[L + i].push(limb(y, i));
        }
        for i in 0..2 * L {
            // z limb i sits in `lo` for i < 8 (128 bits) and in `hi` above.
            let v = if 16 * i < 128 { limb(lo, i) } else { limb(hi, i - 8) };
            cols[2 * L + i].push(v);
        }
    }
    cols
}

/// Exact 256-bit product as (low, high) 128-bit halves.
fn full_product(x: u128, y: u128) -> (u128, u128) {
    let (x0, x1) = (x & u64::MAX as u128, x >> 64);
    let (y0, y1) = (y & u64::MAX as u128, y >> 64);
    let ll = x0 * y0;
    let lh = x0 * y1;
    let hl = x1 * y0;
    let hh = x1 * y1;
    let mid = (ll >> 64) + (lh & u64::MAX as u128) + (hl & u64::MAX as u128);
    let lo = (ll & u64::MAX as u128) | (mid << 64);
    let hi = hh + (lh >> 64) + (hl >> 64) + (mid >> 64);
    (lo, hi)
}

/// Recombine the proved limbs and digest them as the BitZ harness digests
/// its assignment: capacity `max(n, 256).next_power_of_two()` per block,
/// blocks `e0 | x | y | z_lo | z_hi` (u64, u64 entries) or `e0 | x | y | z`
/// (u128, 32-byte entries), prefixed by the assignment length.
fn witness_digest<const L: usize>(cols: &[Vec<LimbInt>]) -> String {
    let rows = cols[0].len();
    let value = |base: usize, limbs: usize, row: usize| -> u128 {
        let mut acc = 0u128;
        for i in 0..limbs {
            let limb = cols[base + i][row];
            assert!((0..1 << LIMB_BITS).contains(&limb), "16-bit limbs");
            acc |= (limb as u128) << (LIMB_BITS as usize * i);
        }
        acc
    };
    let capacity = rows.max(256).next_power_of_two();
    let mut hasher = blake3::Hasher::new();
    if L == 4 {
        hasher.update(b"bitz/u64-mul-compare/integer-witness/v1");
        hasher.update(&((5 * capacity) as u64).to_le_bytes());
        let mut block = |f: &dyn Fn(usize) -> u64| {
            for row in 0..capacity {
                let v = if row < rows { f(row) } else { 0 };
                hasher.update(&v.to_le_bytes());
            }
        };
        block(&|row| u64::from(row == 0));
        block(&|row| value(0, L, row) as u64);
        block(&|row| value(L, L, row) as u64);
        for row in 0..rows {
            let (x, y) = (value(0, L, row), value(L, L, row));
            assert_eq!(x * y, value(2 * L, 2 * L, row), "x·y = z over the integers");
        }
        block(&|row| value(2 * L, L, row) as u64);
        block(&|row| value(3 * L, L, row) as u64);
    } else {
        hasher.update(b"bitz/u128-mul-compare/integer-witness/v1");
        hasher.update(&((4 * capacity) as u64).to_le_bytes());
        let mut block = |f: &dyn Fn(usize) -> (u128, u128)| {
            for row in 0..capacity {
                let (lo, hi) = if row < rows { f(row) } else { (0, 0) };
                hasher.update(&lo.to_le_bytes());
                hasher.update(&hi.to_le_bytes());
            }
        };
        block(&|row| (u128::from(row == 0), 0));
        block(&|row| (value(0, L, row), 0));
        block(&|row| (value(L, L, row), 0));
        for row in 0..rows {
            let (x, y) = (value(0, L, row), value(L, L, row));
            let (lo, hi) = (value(2 * L, 8, row), value(2 * L + 8, 8, row));
            assert_eq!(full_product(x, y), (lo, hi), "x·y = z over the integers");
        }
        block(&|row| (value(2 * L, 8, row), value(2 * L + 8, 8, row)));
    }
    hasher.finalize().to_hex().to_string()
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
/// size)/q: 4L·2^exponent cells over the 2^16-entry word table against the
/// projecting prime.
fn logup_bits(cols: usize, exponent: usize) -> f64 {
    let terms = (cols as f64) * (2f64).powi(exponent as i32) + 65536.0;
    (64 * FIELD_LIMBS) as f64 - terms.log2()
}

fn peak_rss_bytes() -> u64 {
    let mut usage: libc::rusage = unsafe { std::mem::zeroed() };
    unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut usage) };
    let raw = usage.ru_maxrss as u64;
    if cfg!(target_os = "macos") { raw } else { raw * 1024 }
}

fn run<const L: usize>(corpus: &Corpus, reps: usize) {
    let exponent = corpus.log_n;
    let poly_size = 1usize << exponent;
    // The IPRS NTT over F65537 needs row_len · REP < 65537; the plain int
    // lane over the wide combination ring has no further cap, but rows of
    // 8192 keep the geometry of the u32 row.
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

    type Zt = WideZincTypes;
    macro_rules! piop {
        () => {
            ZincPlusPiop::<Zt, WideMulUair<L>, F, DEGREE_PLUS_ONE>
        };
    }

    let t0 = Instant::now();
    let cols = build_columns::<L>(&corpus.pairs);
    let trace = trace_from_columns(cols.clone());
    let witness_ms = t0.elapsed().as_secs_f64() * 1e3;
    let witness_digest = witness_digest::<L>(&cols);
    assert_eq!(witness_digest, corpus.corpus_digest, "proved limbs differ from the corpus");
    drop(cols);

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

    let sig = WideMulUair::<L>::signature();
    let public_trace = trace.public(&sig);
    let proj_ideal = |_: &IdealOrZero<<WideMulUair<L> as Uair>::Ideal>,
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

    let proof_bytes = proof.get_num_bytes();
    for (rep, (prove, verify)) in prove_ms.iter().zip(verify_ms.iter()).enumerate() {
        println!(
            r#"ZINC_TRIAL {{"rep":{},"prove_ms":{:.4},"verify_ms":{:.4},"proof_bytes":{}}}"#,
            rep, prove, verify, proof_bytes,
        );
    }
    println!(
        concat!(
            r#"ZINC_RESULT {{"schema":"zinc-plus/wide-mul/v1","workload":"{}","exponent":{},"seed":{},"reps":{},"#,
            r#""threads":{},"parallel":{},"checks":{},"row_len":{},"num_rows":{},"columns":{},"#,
            r#""limb_bits":{},"inverse_rate":{},"column_openings":{},"security_bits":{},"#,
            r#""prime_bits":{},"grinding_bits":{},"logup_bits":{:.2},"#,
            r#""setup_ms":{:.4},"witness_ms":{:.4},"prove_ms":{:.4},"verify_ms":{:.4},"proof_bytes":{},"#,
            r#""peak_rss_bytes":{},"input_digest":"{}","witness_digest":"{}","proof_verified":true}}"#
        ),
        corpus.workload,
        exponent,
        corpus.seed,
        reps,
        threads,
        cfg!(feature = "parallel"),
        PERFORM_CHECKS,
        row_len,
        poly_size / row_len,
        4 * L,
        LIMB_BITS,
        REP,
        NUM_COL_OPENINGS_FOR_REP,
        SECURITY_BITS,
        64 * FIELD_LIMBS,
        GRINDING_BITS,
        logup_bits(4 * L, exponent),
        setup_ms,
        witness_ms,
        median(prove_ms),
        median(verify_ms),
        proof_bytes,
        peak_rss_bytes(),
        corpus.corpus_digest,
        witness_digest,
    );
    eprint_proof_size(
        format!("{}/rate1-{REP}/sec{SECURITY_BITS}/nvars={exponent}/row_len={row_len}", corpus.workload),
        &proof,
    );
}

fn main() {
    let manifest = std::path::PathBuf::from(std::env::var("CORPUS").expect("CORPUS=<corpus .json manifest>"));
    let reps: usize = std::env::var("REPS").ok().and_then(|s| s.parse().ok()).unwrap_or(5);
    let corpus = read_corpus(&manifest);
    match corpus.workload.as_str() {
        "u64" => run::<4>(&corpus, reps),
        "u128" => run::<8>(&corpus, reps),
        other => panic!("unsupported workload {other}"),
    }
}
