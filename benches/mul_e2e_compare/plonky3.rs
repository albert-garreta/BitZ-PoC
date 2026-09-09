//! Independent u32 wrapping multiplication through Plonky3's univariate STARK and FRI.
use super::{Corpus, Timing, TraceCapture, Workload, captured};
use p3_air::{Air, AirBuilder, BaseAir, WindowAccess, symbolic::AirLayout};
use p3_challenger::DuplexChallenger;
use p3_commit::ExtensionMmcs;
use p3_dft::Radix2DitParallel;
use p3_field::{Field, PrimeCharacteristicRing, PrimeField64, extension::BinomialExtensionField};
use p3_fri::{FriParameters, TwoAdicFriPcs};
use p3_goldilocks::{Goldilocks, Poseidon2Goldilocks, default_goldilocks_poseidon2_8};
use p3_matrix::dense::RowMajorMatrix;
use p3_merkle_tree::MerkleTreeMmcs;
use p3_symmetric::{PaddingFreeSponge, TruncatedPermutation};
use p3_uni_stark::{
    Proof, ProvenSecurity, StarkConfig, StarkSecurityParams, get_log_num_quotient_chunks, prove,
    verify,
};
use serde_json::{Value, json};
use std::sync::Arc;

const LIMB_BASE: u64 = 1 << 16;
const VALUE_COLUMNS: usize = 8;
const TRACE_WIDTH: usize = VALUE_COLUMNS + 7 * 16 + 17;
const EXTENSION_DEGREE: usize = 5;
const TARGET_BITS: usize = 100;
const LOG_BLOWUP: usize = 3;
const NUM_QUERIES: usize = 100;

type Val = Goldilocks;
type Challenge = BinomialExtensionField<Val, EXTENSION_DEGREE>;
type Perm = Poseidon2Goldilocks<8>;
type Packed = <Val as Field>::Packing;
type Hash = PaddingFreeSponge<Perm, 8, 4, 4>;
type Compress = TruncatedPermutation<Perm, 2, 4, 8>;
type ValMmcs = MerkleTreeMmcs<Packed, Packed, Hash, Compress, 2, 4>;
type ChallengeMmcs = ExtensionMmcs<Val, Challenge, ValMmcs>;
type Challenger = DuplexChallenger<Val, Perm, 8, 4>;
type Pcs = TwoAdicFriPcs<Val, Radix2DitParallel<Val>, ValMmcs, ChallengeMmcs>;
type Config = StarkConfig<Pcs, Challenge, Challenger>;

#[derive(Clone, Copy)]
struct MulAir;
impl<F> BaseAir<F> for MulAir {
    fn width(&self) -> usize {
        TRACE_WIDTH
    }
    fn max_constraint_degree(&self) -> Option<usize> {
        Some(2)
    }
    fn main_next_row_columns(&self) -> Vec<usize> {
        vec![]
    }
}
impl<AB: AirBuilder> Air<AB> for MulAir {
    fn eval(&self, builder: &mut AB) {
        let main = builder.main();
        let row = main.current_slice();
        // [a0, a1, b0, b1, z0, z1, c0, c1], followed by their bits.
        // c1 is 17 bits; the other seven values are 16 bits. Both carry
        // equations stay below 2^33, so a field equality is an integer equality.
        // A single a*b = z + 2^32*q equation would admit Goldilocks aliases.
        for column in 0..VALUE_COLUMNS {
            let start = VALUE_COLUMNS + 16 * column;
            let mut value = AB::Expr::ZERO;
            for bit in 0..value_bits(column) {
                let v = row[start + bit];
                builder.assert_bool(v);
                value += v.into() * AB::Expr::from_u64(1 << bit);
            }
            builder.assert_eq(row[column], value);
        }
        let base = AB::Expr::from_u64(LIMB_BASE);
        builder.assert_eq(
            row[0].into() * row[2].into(),
            row[4].into() + base.clone() * row[6].into(),
        );
        builder.assert_eq(
            row[0].into() * row[3].into() + row[1].into() * row[2].into() + row[6].into(),
            row[5].into() + base * row[7].into(),
        );
    }
}
const fn value_bits(column: usize) -> usize {
    if column == 7 { 17 } else { 16 }
}
fn set_value(row: &mut [Val], column: usize, value: u64) {
    row[column] = Val::from_u64(value);
    for bit in 0..value_bits(column) {
        row[VALUE_COLUMNS + 16 * column + bit] = Val::from_u64((value >> bit) & 1);
    }
}
fn generate(corpus: &Corpus) -> RowMajorMatrix<Val> {
    assert_eq!(
        corpus.workload,
        Workload::U32,
        "Plonky3-FRI supports u32 only"
    );
    let mut values = Val::zero_vec(TRACE_WIDTH * corpus.len());
    for (row, &(a, b)) in values.chunks_exact_mut(TRACE_WIDTH).zip(corpus.inputs()) {
        assert!(a <= u32::MAX as u64 && b <= u32::MAX as u64);
        let (a0, a1, b0, b1) = (a % LIMB_BASE, a / LIMB_BASE, b % LIMB_BASE, b / LIMB_BASE);
        let low = a0 * b0;
        let c0 = low / LIMB_BASE;
        let middle = a0 * b1 + a1 * b0 + c0;
        let columns = [
            a0,
            a1,
            b0,
            b1,
            low % LIMB_BASE,
            middle % LIMB_BASE,
            c0,
            middle / LIMB_BASE,
        ];
        for (column, value) in columns.into_iter().enumerate() {
            set_value(row, column, value);
        }
    }
    RowMajorMatrix::new(values, TRACE_WIDTH)
}
fn configuration(trace_len: usize) -> (Config, StarkSecurityParams) {
    assert!(trace_len.is_power_of_two());
    let perm = default_goldilocks_poseidon2_8();
    let mmcs = ValMmcs::new(Hash::new(perm.clone()), Compress::new(perm.clone()), 0);
    let fri = FriParameters {
        log_blowup: LOG_BLOWUP,
        log_final_poly_len: 0,
        max_log_arity: 1,
        num_queries: NUM_QUERIES,
        commit_proof_of_work_bits: 0,
        query_proof_of_work_bits: 0,
        mmcs: ChallengeMmcs::new(mmcs.clone()),
    };
    let layout = AirLayout::from_air::<Val>(&MulAir);
    // Floor log2(p^5) and half of log2(p^4) conservatively. The report
    // includes AIR composition, DEEP-ALI, FRI and batched-opening terms.
    let mut security = StarkSecurityParams::from_air::<Val, Challenge, _>(
        fri.security_regime(),
        &MulAir,
        layout,
        319,
        127,
        1,
    );
    let quotient_chunks = 1 << get_log_num_quotient_chunks::<Val, _>(&MulAir, layout, trace_len, 0);
    security.num_batched_functions = TRACE_WIDTH + EXTENSION_DEGREE * quotient_chunks;
    require_security(ProvenSecurity::compute(&security, trace_len));
    let pcs = Pcs::new(Radix2DitParallel::default(), mmcs, fri);
    (Config::new(pcs, Challenger::new(perm)), security)
}
fn require_security(security: ProvenSecurity) {
    assert!(
        security.security_bits() >= TARGET_BITS,
        "Plonky3-FRI proven round-by-round security is below {TARGET_BITS} bits: {security:?}"
    );
}

pub(super) struct Context {
    corpus: Arc<Corpus>,
    config: Config,
    security: StarkSecurityParams,
}
impl Context {
    pub(super) fn setup(corpus: Arc<Corpus>) -> Self {
        assert_eq!(
            corpus.workload,
            Workload::U32,
            "Plonky3-FRI supports u32 only"
        );
        let (config, security) = configuration(corpus.len());
        Self {
            corpus,
            config,
            security,
        }
    }
    pub(super) fn config(&self) -> Value {
        let security = ProvenSecurity::compute(&self.security, self.corpus.len());
        json!({
            "piop":"Plonky3 univariate STARK AIR quotient", "pcs":"FRI",
            "base_field":"Goldilocks", "extension_degree":EXTENSION_DEGREE,
            "target_bits":TARGET_BITS, "security_scope":"proven round-by-round",
            "proven_bits":security.security_bits(), "unique_decoding_bits":security.unique_decoding_bits,
            "list_decoding_bits":security.list_decoding_bits, "hash":"Poseidon2Goldilocks-width8",
            "log_inv_rate":LOG_BLOWUP, "num_queries":NUM_QUERIES, "max_log_arity":1,
            "log_final_poly_len":0, "commit_pow_bits":0, "query_pow_bits":0,
            "trace_width":TRACE_WIDTH, "num_constraints":self.security.num_constraints,
            "max_constraint_degree":self.security.air_max_constraint_degree,
            "num_batched_functions":self.security.num_batched_functions,
            "revision":super::common::locked_git_revision("p3-fri"),
        })
    }
    pub(super) fn run(&self, capture: &TraceCapture) -> Timing {
        capture.begin();
        let start = capture.now_ns();
        let trace = generate(&self.corpus);
        let wend = capture.now_ns();
        let proof: Proof<Config> = prove(&self.config, &MulAir, trace, &[]);
        let ready = capture.now_ns();
        let bytes = postcard::to_allocvec(&proof).expect("encode Plonky3-FRI proof");
        assert_eq!(proof.degree_bits, self.corpus.len().ilog2() as usize);
        require_security(proof.proven_security(&self.security));
        let vstart = capture.now_ns();
        verify(&self.config, &MulAir, &proof, &[]).expect("Plonky3-FRI full proof verifies");
        let end = capture.now_ns();
        let raw = capture.finish();
        let commit = captured(&raw, "commit to trace data", wend, ready);
        let piop = captured(&raw, "AIR quotient PIOP", commit.end_ns, ready);
        let opening = captured(&raw, "open", piop.end_ns, ready);
        let mut timing = Timing::new(start, wend, ready, vstart, end, bytes.len());
        timing.add("commit", "commit", commit.start_ns, commit.end_ns);
        timing.add("piop", "constraint-proof", piop.start_ns, piop.end_ns);
        timing.add("opening", "opening-proof", opening.start_ns, opening.end_ns);
        std::hint::black_box(proof);
        timing
    }
}

pub(super) fn audit(corpus: &Corpus) -> super::WitnessAudit {
    let started = std::time::Instant::now();
    let trace = generate(corpus);
    let generation_ms = started.elapsed().as_secs_f64() * 1e3;
    let rows = trace
        .values
        .chunks_exact(TRACE_WIDTH)
        .map(|row| {
            let pair = |column: usize| {
                row[column].as_canonical_u64() + LIMB_BASE * row[column + 1].as_canonical_u64()
            };
            [pair(0), pair(2), pair(4), 0]
        })
        .collect();
    super::WitnessAudit::check(
        corpus,
        rows,
        generation_ms,
        "Plonky3-FRI low-32-bit limb assignment",
        false,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn boundary_corpus() -> Corpus {
        let values = [0, 1, LIMB_BASE - 1, LIMB_BASE, u32::MAX as u64];
        let mut inputs: Vec<_> = values
            .into_iter()
            .flat_map(|a| values.map(|b| (a, b)))
            .collect();
        inputs.resize(inputs.len().next_power_of_two(), (0, 0));
        Corpus::from_inputs(Workload::U32, inputs)
    }
    fn assert_invalid(trace: RowMajorMatrix<Val>) {
        assert!(
            std::panic::catch_unwind(|| p3_air::check_constraints(&MulAir, &trace, &[])).is_err()
        );
    }
    #[test]
    fn wrapping_air_checks_boundaries_and_both_carries() {
        let corpus = boundary_corpus();
        let trace = generate(&corpus);
        p3_air::check_constraints(&MulAir, &trace, &[]);
        for (row, &(a, b)) in trace.values.chunks_exact(TRACE_WIDTH).zip(corpus.inputs()) {
            let z = row[4].as_canonical_u64() + LIMB_BASE * row[5].as_canonical_u64();
            assert_eq!(z, (a * b) & u32::MAX as u64);
        }
        for column in [4, 5, 6, 7] {
            let mut wrong = trace.clone();
            set_value(&mut wrong.values[..TRACE_WIDTH], column, 1);
            assert_invalid(wrong);
        }
        let mut wrong = trace.clone();
        set_value(&mut wrong.values[..TRACE_WIDTH], 0, LIMB_BASE);
        assert_invalid(wrong);
        let mut wrong = trace;
        wrong.values[VALUE_COLUMNS] = Val::TWO;
        assert_invalid(wrong);
    }
    #[test]
    fn rejects_goldilocks_alias_of_zero_product() {
        // The naive four-u32 equation accepts (0,0,1,2^32-1): its RHS is p.
        let modulus = (1u128 << 64) - (1u128 << 32) + 1;
        assert_eq!(1 + (1u128 << 32) * u128::from(u32::MAX), modulus);
        let corpus = Corpus::from_inputs(Workload::U32, vec![(0, 0); 16]);
        let mut trace = generate(&corpus);
        set_value(&mut trace.values[..TRACE_WIDTH], 4, 1);
        assert_invalid(trace);
    }
    #[test]
    fn actual_air_and_every_supported_shape_reach_security_target() {
        for exponent in 4..=25 {
            let (_, params) = configuration(1 << exponent);
            assert_eq!(params.num_constraints, 139);
            assert_eq!(params.air_max_constraint_degree, 2);
            assert_eq!(params.max_combo, 1);
            assert_eq!(params.num_batched_functions, 142);
            require_security(ProvenSecurity::compute(&params, 1 << exponent));
        }
    }
    #[test]
    fn fri_proof_roundtrip_and_opening_tamper_rejection() {
        let corpus = boundary_corpus();
        let (config, security) = configuration(corpus.len());
        let mut proof = prove(&config, &MulAir, generate(&corpus), &[]);
        verify(&config, &MulAir, &proof, &[]).unwrap();
        require_security(proof.proven_security(&security));
        let bytes = postcard::to_allocvec(&proof).unwrap();
        assert!(!bytes.is_empty());
        let decoded: Proof<Config> = postcard::from_bytes(&bytes).unwrap();
        verify(&config, &MulAir, &decoded, &[]).unwrap();
        proof.opened_values.trace_local[4] += Challenge::ONE;
        assert!(verify(&config, &MulAir, &proof, &[]).is_err());
    }
}
