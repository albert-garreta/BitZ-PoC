//! End-to-end tests of the F₂-virtualization opening (paper
//! `s:to_f2_virtual` / `c:virtual_iop`): mod-q claims about the derived
//! vector `h = M·f` proven against the commitment to `f` alone.
//!
//! The battery checks (a) semantic agreement with the DIRECT path — the
//! same claim proven by committing `h` itself through
//! `prove_mle_eval_mod_q_ligerito` accepts the same `y`; (b) the virtual
//! roundtrip across a 1-chunk W=1 shape and a 2-chunk W=32 shape;
//! (c) rejection of a wrong claim, an out-of-range chunk fold, swapped
//! pre-sumchecks, a tampered batching message (both a c₀-visible flip
//! and a step-3-consistent one), a non-generator α, a substituted map,
//! and a wrong geometry.

use f2z::f2map::{PreparedVirtualMap, cell_count, cell_row_bits};
use f2z::ligerito::IntEvalRsError;
use f2z::ligerito::{LOG_PACKING, RsOpenError, packed_vars};
use f2z::ligerito_flock::{
    FlockRsError, IntEvalRsLigVirtProof, LigConfig, VirtualReductionProof, commit_rs_ligerito_rows,
    lig_configs, prove_mle_eval_mod_q_ligerito, prove_mle_eval_mod_q_ligerito_virtual,
    verify_mle_eval_mod_q_ligerito, verify_mle_eval_mod_q_ligerito_virtual,
};
use f2z::pcs::{IntegerMatrixLayout, smallest_generator};
use f2z::sparse_matrix::SparseMatrix;
use f2z::transcript::{Blake3Transcript, traits::Transcript};

const Q: u128 = (1u128 << 100) - 15;
const Q_BITS: usize = 100;

#[derive(Clone, Copy, PartialEq, Debug)]
struct Fq(u128);
impl From<u128> for Fq {
    fn from(v: u128) -> Self {
        Fq(v % Q)
    }
}
impl core::ops::Add for Fq {
    type Output = Fq;
    fn add(self, o: Fq) -> Fq {
        let s = self.0 + o.0; // both < Q < 2^100: no overflow
        Fq(if s >= Q { s - Q } else { s })
    }
}
impl core::ops::Mul for Fq {
    type Output = Fq;
    fn mul(self, o: Fq) -> Fq {
        let (mut a, mut b, mut acc) = (self.0, o.0, 0u128);
        while b != 0 {
            if b & 1 == 1 {
                let s = acc + a;
                acc = if s >= Q { s - Q } else { s };
            }
            let d = a << 1;
            a = if d >= Q { d - Q } else { d };
            b >>= 1;
        }
        Fq(acc)
    }
}

fn splitmix(x: u64) -> u64 {
    let mut z = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Pseudo-random committed bits for `f`, in commit-row layout.
fn f_rows(f_layout: &IntegerMatrixLayout, seed: u64) -> Vec<Vec<u64>> {
    let t_wf = cell_row_bits(f_layout);
    (0..1usize << f_layout.col_vars)
        .map(|c| {
            (0..(1usize << t_wf) / 64)
                .map(|w| splitmix(seed ^ ((c as u64) << 32) ^ w as u64))
                .collect()
        })
        .collect()
}

fn bit_at(rows: &[Vec<u64>], t_w: usize, flat: usize) -> u64 {
    let (c, b) = (flat >> t_w, flat & ((1 << t_w) - 1));
    (rows[c][b >> 6] >> (b & 63)) & 1
}

/// A deterministic sparse map: derived cell `i` = XOR of up to three
/// pseudo-random source cells; every seventh row is empty.
fn prepared_from_rows(rows: usize, columns: usize, lists: Vec<Vec<usize>>) -> PreparedVirtualMap {
    let matrix = SparseMatrix::try_from_rows(
        columns,
        lists
            .into_iter()
            .map(|row| row.into_iter().map(|column| (column, true)).collect())
            .collect(),
    )
    .unwrap();
    assert_eq!(matrix.row_count(), rows);
    PreparedVirtualMap::new(matrix).unwrap()
}

fn test_map(n_h: usize, n_f: usize, seed: u64) -> PreparedVirtualMap {
    let lists: Vec<Vec<usize>> = (0..n_h)
        .map(|i| {
            if i % 7 == 6 {
                return Vec::new();
            }
            let mut l: Vec<usize> = (0..=(i % 3))
                .map(|k| (splitmix(seed ^ ((i as u64) << 8) ^ k as u64) % n_f as u64) as usize)
                .collect();
            l.sort_unstable();
            l.dedup();
            l
        })
        .collect();
    prepared_from_rows(n_h, n_f, lists)
}

/// Test-only forward multiplication. Production proving receives synthesized
/// `h` and never exposes an `M f` operation.
fn apply_map(
    map: &PreparedVirtualMap,
    h_layout: &IntegerMatrixLayout,
    f_layout: &IntegerMatrixLayout,
    f_rows: &[Vec<u64>],
) -> Vec<Vec<u64>> {
    let t_wh = cell_row_bits(h_layout);
    let t_wf = cell_row_bits(f_layout);
    let mut h_rows = vec![vec![0u64; (1usize << t_wh) / 64]; 1usize << h_layout.col_vars];
    for (source, column) in map.matrix().columns().enumerate() {
        if bit_at(f_rows, t_wf, source) == 0 {
            continue;
        }
        for &derived in column.row_indices() {
            let (c, b) = (derived >> t_wh, derived & ((1 << t_wh) - 1));
            h_rows[c][b >> 6] ^= 1u64 << (b & 63);
        }
    }
    h_rows
}

fn row_lists(map: &PreparedVirtualMap) -> Vec<Vec<usize>> {
    let mut rows = vec![Vec::new(); map.rows()];
    for (source, column) in map.matrix().columns().enumerate() {
        for &derived in column.row_indices() {
            rows[derived].push(source);
        }
    }
    rows
}

/// The claimed value `y = Σ_c w'_c · Σ_b rw[b] · WORD(b, c)` computed
/// naively from `h`'s bit cells (`W` word bits per row entry).
fn expected_y(h_layout: &IntegerMatrixLayout, h_rows: &[Vec<u64>], rw: &[u128], col_w: &[Fq]) -> Fq {
    let t_wh = cell_row_bits(h_layout);
    let log_w = h_layout.word_bits.trailing_zeros() as usize;
    let mut y = Fq::from(0u128);
    for c in 0..1usize << h_layout.col_vars {
        let mut acc = Fq::from(0u128);
        for b in 0..1usize << h_layout.row_vars {
            let mut word = 0u128;
            for j in 0..h_layout.word_bits {
                let flat = (c << t_wh) | (b << log_w) | j;
                word |= (bit_at(h_rows, t_wh, flat) as u128) << j;
            }
            acc = acc + Fq::from(rw[b]) * Fq::from(word);
        }
        y = y + col_w[c] * acc;
    }
    y
}

fn clone_proof(p: &IntEvalRsLigVirtProof) -> IntEvalRsLigVirtProof {
    IntEvalRsLigVirtProof {
        mfs: p.mfs.clone(),
        us: p.us.clone(),
        presums: p.presums.clone(),
        reduction: p.reduction.clone(),
        lig: p.lig.clone(),
        grinding_nonces: p.grinding_nonces.clone(),
        ood: p.ood,
    }
}

/// The AdjointBatch reduction's `h_i` vector, mutable (all `run_shape`
/// proofs use non-identity maps).
fn hs_mut(
    p: &mut IntEvalRsLigVirtProof,
) -> &mut [f2z::poly::univariate::binary_gf128::BinaryFieldGF128; 128] {
    match &mut p.reduction {
        VirtualReductionProof::AdjointBatch { hs } => hs,
        VirtualReductionProof::Eq { .. } => panic!("expected the adjoint-batch reduction"),
    }
}

// Proof and transcript pins were refreshed from pre-change commit 2733f5b.
fn assert_proof_pin(
    label: &str,
    bytes: &[u8],
    transcript: &mut Blake3Transcript,
    expected_hash: &str,
    expected_next: u128,
) {
    assert_eq!(
        blake3::hash(bytes).to_hex().to_string(),
        expected_hash,
        "{label} proof bytes changed"
    );
    assert_eq!(
        transcript.get_challenge::<u128>(),
        expected_next,
        "{label} transcript continuation changed"
    );
}

fn run_shape(
    h_layout: IntegerMatrixLayout,
    seed: u64,
    virtual_pin: (&str, u128),
    direct_pin: (&str, u128),
) {
    let f_layout = IntegerMatrixLayout {
        row_vars: 10,
        col_vars: 5,
        word_bits: 1,
    };
    let alpha = smallest_generator();
    let (pc_f, vc_f) = lig_configs(
        packed_vars(&f_layout),
        LigConfig::Adhoc {
            log_batch: 2,
            log_inv_rate: 2,
        },
    )
    .unwrap();

    let rows_f = f_rows(&f_layout, seed);
    let map = test_map(cell_count(&h_layout), cell_count(&f_layout), seed ^ 0xF00D);
    let hint_f = commit_rs_ligerito_rows(&f_layout, rows_f.clone(), &pc_f);

    // Independent test-only forward multiplication.
    let t_wf = cell_row_bits(&f_layout);
    let t_wh = cell_row_bits(&h_layout);
    let h_rows = apply_map(&map, &h_layout, &f_layout, &rows_f);
    for (i, sources) in row_lists(&map).iter().enumerate() {
        let expect = sources
            .iter()
            .fold(0u64, |a, &j| a ^ bit_at(&rows_f, t_wf, j));
        assert_eq!(bit_at(&h_rows, t_wh, i), expect, "derived cell {i}");
    }

    let rw_q: Vec<u128> = (0..h_layout.rows())
        .map(|b| (splitmix(seed ^ 0xBEEF ^ b as u64) as u128) << 40 | b as u128)
        .map(|x| x % Q)
        .collect();
    let col_w: Vec<Fq> = (0..h_layout.cols())
        .map(|c| Fq::from((splitmix(seed ^ c as u64) & 0xFF) as u128 + 1))
        .collect();
    let y = expected_y(&h_layout, &h_rows, &rw_q, &col_w);

    // Virtual proof against f's commitment.
    let mut pt = Blake3Transcript::new();
    let proof = prove_mle_eval_mod_q_ligerito_virtual(
        &mut pt, &hint_f, &h_rows, &h_layout, &f_layout, &map, &rw_q, Q_BITS, alpha, &pc_f,
    );
    let proof_bytes = proof.to_bytes();
    assert_proof_pin(
        "non-identity AdjointBatch",
        &proof_bytes,
        &mut pt,
        virtual_pin.0,
        virtual_pin.1,
    );
    let decoded = IntEvalRsLigVirtProof::from_bytes(&proof_bytes)
        .expect("AdjointBatch proof codec roundtrip");
    assert_eq!(
        decoded.to_bytes(),
        proof_bytes,
        "virtual codec is canonical"
    );
    let hs = match &proof.reduction {
        VirtualReductionProof::AdjointBatch { hs } => hs,
        VirtualReductionProof::Eq { .. } => panic!("expected AdjointBatch"),
    };
    let mut hs_bytes = Vec::with_capacity(128 * 16);
    for value in hs.iter() {
        for word in value.words() {
            hs_bytes.extend_from_slice(&word.to_le_bytes());
        }
    }
    let hs_start = proof_bytes
        .windows(hs_bytes.len())
        .position(|window| window == hs_bytes)
        .expect("fixed h_i block in virtual proof stream");
    assert_eq!(proof_bytes[hs_start - 1], 0, "AdjointBatch codec tag");
    let mut short_hs = proof_bytes.clone();
    short_hs.drain(hs_start + 127 * 16..hs_start + 128 * 16);
    assert!(
        IntEvalRsLigVirtProof::from_bytes(&short_hs).is_err(),
        "a 127-element h_i block must not decode"
    );
    let mut vt = Blake3Transcript::new();
    verify_mle_eval_mod_q_ligerito_virtual(
        &mut vt,
        &hint_f.commitment,
        &proof,
        &h_layout,
        &f_layout,
        &map,
        &rw_q,
        &col_w,
        alpha,
        y,
        Q_BITS,
        &vc_f,
    )
    .unwrap_or_else(|e| {
        panic!(
            "virtual roundtrip (t_h={}, W={}) failed: {e:?}",
            h_layout.row_vars, h_layout.word_bits
        )
    });

    // Semantic agreement: the DIRECT path (committing h itself) accepts the
    // same y under the same weights.
    let (pc_h, vc_h) = lig_configs(
        packed_vars(&h_layout),
        LigConfig::Adhoc {
            log_batch: 2,
            log_inv_rate: 2,
        },
    )
    .unwrap();
    let hint_h = commit_rs_ligerito_rows(&h_layout, h_rows.clone(), &pc_h);
    let mut pt = Blake3Transcript::new();
    let direct =
        prove_mle_eval_mod_q_ligerito(&mut pt, &hint_h, &h_layout, &rw_q, Q_BITS, alpha, &pc_h);
    assert_proof_pin(
        "direct Eq",
        &direct.to_bytes(),
        &mut pt,
        direct_pin.0,
        direct_pin.1,
    );
    let mut vt = Blake3Transcript::new();
    verify_mle_eval_mod_q_ligerito(
        &mut vt,
        &hint_h.commitment,
        &direct,
        &h_layout,
        &rw_q,
        &col_w,
        alpha,
        y,
        Q_BITS,
        &vc_h,
    )
    .expect("direct path agrees with the virtual claim");

    // Wrong claim.
    let mut vt = Blake3Transcript::new();
    assert_eq!(
        verify_mle_eval_mod_q_ligerito_virtual(
            &mut vt,
            &hint_f.commitment,
            &proof,
            &h_layout,
            &f_layout,
            &map,
            &rw_q,
            &col_w,
            alpha,
            y + Fq::from(1u128),
            Q_BITS,
            &vc_f,
        ),
        Err(FlockRsError::Common(IntEvalRsError::ReadOff)),
    );

    // Out-of-range chunk fold.
    let mut bad = clone_proof(&proof);
    bad.us[0][0] = u128::MAX - 1;
    let mut vt = Blake3Transcript::new();
    assert!(matches!(
        verify_mle_eval_mod_q_ligerito_virtual(
            &mut vt,
            &hint_f.commitment,
            &bad,
            &h_layout,
            &f_layout,
            &map,
            &rw_q,
            &col_w,
            alpha,
            y,
            Q_BITS,
            &vc_f,
        ),
        Err(FlockRsError::ChunkRange { .. })
    ));

    // Pre-sumchecks from a different statement (swapped between chunks)
    // must not verify.
    if proof.presums.len() > 1 {
        let mut bad = clone_proof(&proof);
        bad.presums.swap(0, 1);
        let mut vt = Blake3Transcript::new();
        assert!(
            verify_mle_eval_mod_q_ligerito_virtual(
                &mut vt,
                &hint_f.commitment,
                &bad,
                &h_layout,
                &f_layout,
                &map,
                &rw_q,
                &col_w,
                alpha,
                y,
                Q_BITS,
                &vc_f,
            )
            .is_err()
        );
    }

    // A batching-message value flip that touches c₀ fails the step-3
    // coefficient-projection check outright.
    let mut bad = clone_proof(&proof);
    {
        let hs = hs_mut(&mut bad);
        hs[5] = hs[5] + f2z::poly::univariate::binary_gf128::BinaryFieldGF128::one();
    }
    let mut vt = Blake3Transcript::new();
    assert_eq!(
        verify_mle_eval_mod_q_ligerito_virtual(
            &mut vt,
            &hint_f.commitment,
            &bad,
            &h_layout,
            &f_layout,
            &map,
            &rw_q,
            &col_w,
            alpha,
            y,
            Q_BITS,
            &vc_f,
        ),
        Err(FlockRsError::VirtualBatch),
    );

    // A step-3-CONSISTENT tamper (upper coordinates of one h_i, c₀
    // untouched) passes the projection check but must still be rejected:
    // the ρ-batched target no longer matches the committed basis claim,
    // and the h_i are transcript-bound before ρ is drawn.
    let mut bad = clone_proof(&proof);
    {
        let hs = hs_mut(&mut bad);
        hs[7] =
            hs[7] + f2z::poly::univariate::binary_gf128::BinaryFieldGF128::from_words([1 << 9, 0]);
    }
    let mut vt = Blake3Transcript::new();
    assert!(
        verify_mle_eval_mod_q_ligerito_virtual(
            &mut vt,
            &hint_f.commitment,
            &bad,
            &h_layout,
            &f_layout,
            &map,
            &rw_q,
            &col_w,
            alpha,
            y,
            Q_BITS,
            &vc_f,
        )
        .is_err()
    );

    // Non-generator α.
    let mut vt = Blake3Transcript::new();
    assert_eq!(
        verify_mle_eval_mod_q_ligerito_virtual(
            &mut vt,
            &hint_f.commitment,
            &proof,
            &h_layout,
            &f_layout,
            &map,
            &rw_q,
            &col_w,
            f2z::poly::univariate::binary_gf128::BinaryFieldGF128::one(),
            y,
            Q_BITS,
            &vc_f,
        ),
        Err(FlockRsError::Common(IntEvalRsError::ChallengeNotGenerator)),
    );

    // A substituted map (one extra source in the first nonempty row) must
    // not verify: the statement digest, the derived claims, and Ŵ all
    // change.
    let mut lists = row_lists(&map);
    let target = lists.iter().position(|l| !l.is_empty()).unwrap();
    let extra = (0..cell_count(&f_layout))
        .find(|j| !lists[target].contains(j))
        .unwrap();
    lists[target].push(extra);
    lists[target].sort_unstable();
    let map2 = prepared_from_rows(map.rows(), map.cols(), lists);
    let mut vt = Blake3Transcript::new();
    assert!(
        verify_mle_eval_mod_q_ligerito_virtual(
            &mut vt,
            &hint_f.commitment,
            &proof,
            &h_layout,
            &f_layout,
            &map2,
            &rw_q,
            &col_w,
            alpha,
            y,
            Q_BITS,
            &vc_f,
        )
        .is_err()
    );

    // Geometry mismatch is rejected up front.
    let p_wrong = IntegerMatrixLayout {
        row_vars: h_layout.row_vars,
        col_vars: h_layout.col_vars + 1,
        word_bits: h_layout.word_bits,
    };
    let mut vt = Blake3Transcript::new();
    assert_eq!(
        verify_mle_eval_mod_q_ligerito_virtual(
            &mut vt,
            &hint_f.commitment,
            &proof,
            &p_wrong,
            &f_layout,
            &map,
            &rw_q,
            &col_w,
            alpha,
            y,
            Q_BITS,
            &vc_f,
        ),
        Err(FlockRsError::RingSwitch(RsOpenError::Shape)),
    );
}

/// 1-chunk regime: W = 1 derived shape (c_w = 116 ≥ q_bits).
#[test]
fn virtual_open_roundtrips_one_chunk_w1() {
    run_shape(
        IntegerMatrixLayout {
            row_vars: 9,
            col_vars: 6,
            word_bits: 1,
        },
        0x5EED_0001,
        (
            "fe7ee1bdd13bde75ab2f9f968b940c90f76a1836a5ffa6e0c7ce59e6474a2f6e",
            204_570_546_892_575_606_146_720_026_402_923_708_369,
        ),
        (
            "5eb7f77af208641b6bb0ee40e95c72ced4943cc0218ace39adf22347addaf459",
            309_955_990_109_051_369_771_710_748_047_216_439_850,
        ),
    );
}

/// 2-chunk regime: W = 32 derived shape (c_w = 91 < q_bits = 100).
#[test]
fn virtual_open_roundtrips_two_chunks_w32() {
    run_shape(
        IntegerMatrixLayout {
            row_vars: 4,
            col_vars: 6,
            word_bits: 32,
        },
        0x5EED_0002,
        (
            "5c138c346455634825a0e0e4d6f05dcad3bd163c71122d3f6997e4dbafdea799",
            319_005_638_020_492_504_682_831_733_900_486_516_633,
        ),
        (
            "c14da7b200b19170910d4f687e749b69102314eab8ac2193bf686b25a547df24",
            139_231_682_465_937_171_433_285_275_040_497_622_785,
        ),
    );
}

/// A map with a single live derived cell still roundtrips (near-degenerate
/// forests: all-but-one tree constant).
#[test]
fn virtual_open_single_live_row() {
    let h_layout = IntegerMatrixLayout {
        row_vars: 9,
        col_vars: 6,
        word_bits: 1,
    };
    let f_layout = IntegerMatrixLayout {
        row_vars: 10,
        col_vars: 5,
        word_bits: 1,
    };
    let alpha = smallest_generator();
    let (pc_f, vc_f) = lig_configs(
        packed_vars(&f_layout),
        LigConfig::Adhoc {
            log_batch: 2,
            log_inv_rate: 2,
        },
    )
    .unwrap();
    let rows_f = f_rows(&f_layout, 0x51_4E);
    // One nonempty derived row XORing three sources.
    let mut lists = vec![Vec::new(); cell_count(&h_layout)];
    lists[137] = vec![3usize, 1000, 8000];
    let map = prepared_from_rows(cell_count(&h_layout), cell_count(&f_layout), lists);
    let hint_f = commit_rs_ligerito_rows(&f_layout, rows_f.clone(), &pc_f);
    let h_rows = apply_map(&map, &h_layout, &f_layout, &rows_f);

    let rw_q: Vec<u128> = (0..h_layout.rows())
        .map(|b| (b as u128 * 977 + 3) % Q)
        .collect();
    let col_w: Vec<Fq> = (0..h_layout.cols())
        .map(|c| Fq::from(c as u128 + 2))
        .collect();
    let y = expected_y(&h_layout, &h_rows, &rw_q, &col_w);

    let mut pt = Blake3Transcript::new();
    let proof = prove_mle_eval_mod_q_ligerito_virtual(
        &mut pt, &hint_f, &h_rows, &h_layout, &f_layout, &map, &rw_q, Q_BITS, alpha, &pc_f,
    );
    let mut vt = Blake3Transcript::new();
    verify_mle_eval_mod_q_ligerito_virtual(
        &mut vt,
        &hint_f.commitment,
        &proof,
        &h_layout,
        &f_layout,
        &map,
        &rw_q,
        &col_w,
        alpha,
        y,
        Q_BITS,
        &vc_f,
    )
    .expect("single-live-row virtual roundtrip");

    // The prover may supply any correctly shaped synthesized h, but the
    // virtual relation binds it to the committed f. A changed h and matching
    // changed read-off claim must therefore still be rejected.
    let mut wrong_h = h_rows;
    wrong_h[0][0] ^= 1;
    let wrong_y = expected_y(&h_layout, &wrong_h, &rw_q, &col_w);
    let mut pt = Blake3Transcript::new();
    let wrong = prove_mle_eval_mod_q_ligerito_virtual(
        &mut pt, &hint_f, &wrong_h, &h_layout, &f_layout, &map, &rw_q, Q_BITS, alpha, &pc_f,
    );
    let mut vt = Blake3Transcript::new();
    assert!(
        verify_mle_eval_mod_q_ligerito_virtual(
            &mut vt,
            &hint_f.commitment,
            &wrong,
            &h_layout,
            &f_layout,
            &map,
            &rw_q,
            &col_w,
            alpha,
            wrong_y,
            Q_BITS,
            &vc_f,
        )
        .is_err(),
        "a synthesized h inconsistent with committed f was accepted"
    );
}

/// Identity `M` with one shared row layout: the fast path routes to the
/// base eq ring switch (`Eq` — supplied `h` is unused, no `h_i` fold, no
/// `a′` build); with the switch off the general AdjointBatch reduction proves the
/// same statement; the verifier accepts either reduction there but rejects
/// Eq on a non-eligible statement.
#[test]
fn virtual_open_identity_fast_path() {
    use f2z::poly::univariate::binary_gf128::BinaryFieldGF128 as Gf;
    let p = IntegerMatrixLayout {
        row_vars: 10,
        col_vars: 5,
        word_bits: 1,
    };
    let alpha = smallest_generator();
    let (pc, vc) = lig_configs(
        packed_vars(&p),
        LigConfig::Adhoc {
            log_batch: 2,
            log_inv_rate: 2,
        },
    )
    .unwrap();
    let rows_f = f_rows(&p, 0x1D_FA57);
    let n = cell_count(&p);
    let map = PreparedVirtualMap::new(
        SparseMatrix::try_from_binary_csc(n, (0..=n).collect(), (0..n).collect()).unwrap(),
    )
    .unwrap();
    assert!(map.is_identity());
    let hint = commit_rs_ligerito_rows(&p, rows_f.clone(), &pc);

    let rw_q: Vec<u128> = (0..p.rows())
        .map(|b| (splitmix(0xF457 ^ b as u64) as u128) % Q)
        .collect();
    let col_w: Vec<Fq> = (0..p.cols()).map(|c| Fq::from(c as u128 + 3)).collect();
    // h = f: the expected value reads f's own cells.
    let y = expected_y(&p, &rows_f, &rw_q, &col_w);

    // Fast path (default ON): the proof carries Eq and verifies.
    let mut pt = Blake3Transcript::new();
    let proof = prove_mle_eval_mod_q_ligerito_virtual(
        &mut pt, &hint, &rows_f, &p, &p, &map, &rw_q, Q_BITS, alpha, &pc,
    );
    assert_proof_pin(
        "virtual identity Eq",
        &proof.to_bytes(),
        &mut pt,
        "730be73a4dcf40c40cb45cf6e180e326d6779b4d60487f5b7956b9176457d58c",
        218_938_428_708_468_322_662_924_008_685_302_992_257,
    );
    assert!(
        matches!(proof.reduction, VirtualReductionProof::Eq { .. }),
        "identity map with matching layout must take the fast path"
    );
    let mut vt = Blake3Transcript::new();
    verify_mle_eval_mod_q_ligerito_virtual(
        &mut vt,
        &hint.commitment,
        &proof,
        &p,
        &p,
        &map,
        &rw_q,
        &col_w,
        alpha,
        y,
        Q_BITS,
        &vc,
    )
    .expect("identity fast-path roundtrip");

    // Adversarial public geometry is rejected before any unchecked shape
    // helper or transcript absorption.
    let reject_shape_without_absorption =
        |h_layout: &IntegerMatrixLayout, f_layout: &IntegerMatrixLayout, q_bits: usize| {
            let mut actual = Blake3Transcript::new();
            let mut untouched = actual.clone();
            assert_eq!(
                verify_mle_eval_mod_q_ligerito_virtual(
                    &mut actual,
                    &hint.commitment,
                    &proof,
                    h_layout,
                    f_layout,
                    &map,
                    &rw_q,
                    &col_w,
                    alpha,
                    y,
                    q_bits,
                    &vc,
                ),
                Err(FlockRsError::RingSwitch(RsOpenError::Shape))
            );
            assert_eq!(
                actual.get_challenge::<u128>(),
                untouched.get_challenge::<u128>(),
                "shape rejection must not mutate the transcript"
            );
        };
    reject_shape_without_absorption(
        &IntegerMatrixLayout {
            row_vars: usize::MAX,
            col_vars: p.col_vars,
            word_bits: p.word_bits,
        },
        &p,
        Q_BITS,
    );
    reject_shape_without_absorption(
        &p,
        &IntegerMatrixLayout {
            row_vars: p.row_vars,
            col_vars: p.col_vars,
            word_bits: 0,
        },
        Q_BITS,
    );
    reject_shape_without_absorption(
        &IntegerMatrixLayout {
            row_vars: p.row_vars,
            col_vars: usize::MAX,
            word_bits: p.word_bits,
        },
        &p,
        Q_BITS,
    );
    reject_shape_without_absorption(
        &IntegerMatrixLayout {
            row_vars: LOG_PACKING - 1,
            col_vars: p.col_vars,
            word_bits: 1,
        },
        &p,
        Q_BITS,
    );
    reject_shape_without_absorption(
        &IntegerMatrixLayout {
            row_vars: p.row_vars + p.col_vars,
            col_vars: 0,
            word_bits: 1,
        },
        &p,
        Q_BITS,
    );
    reject_shape_without_absorption(&p, &p, 0);

    // Codec: Eq roundtrips canonically and the decoded proof
    // verifies.
    let bytes = proof.to_bytes();
    let decoded = IntEvalRsLigVirtProof::from_bytes(&bytes).expect("eq-tail decode");
    assert_eq!(
        decoded.to_bytes(),
        bytes,
        "codec is a bijection on its image"
    );
    let mut vt = Blake3Transcript::new();
    verify_mle_eval_mod_q_ligerito_virtual(
        &mut vt,
        &hint.commitment,
        &decoded,
        &p,
        &p,
        &map,
        &rw_q,
        &col_w,
        alpha,
        y,
        Q_BITS,
        &vc,
    )
    .expect("decoded fast-path proof verifies");

    // A tampered ring-switch message must not verify.
    let mut bad = clone_proof(&proof);
    match &mut bad.reduction {
        VirtualReductionProof::Eq { rings } => rings[0].s_v[3] = rings[0].s_v[3] + Gf::one(),
        VirtualReductionProof::AdjointBatch { .. } => unreachable!(),
    }
    let mut vt = Blake3Transcript::new();
    assert!(
        verify_mle_eval_mod_q_ligerito_virtual(
            &mut vt,
            &hint.commitment,
            &bad,
            &p,
            &p,
            &map,
            &rw_q,
            &col_w,
            alpha,
            y,
            Q_BITS,
            &vc,
        )
        .is_err()
    );

    // Eq is rejected outright on a non-eligible statement (a
    // non-identity map of the same shape) — the routing gate fires even
    // before any transcript divergence matters.
    let mut lists: Vec<Vec<usize>> = (0..n).map(|i| vec![i]).collect();
    lists[0] = vec![0, 1];
    let map2 = prepared_from_rows(n, n, lists);
    assert!(!map2.is_identity());
    let mut vt = Blake3Transcript::new();
    assert!(
        verify_mle_eval_mod_q_ligerito_virtual(
            &mut vt,
            &hint.commitment,
            &proof,
            &p,
            &p,
            &map2,
            &rw_q,
            &col_w,
            alpha,
            y,
            Q_BITS,
            &vc,
        )
        .is_err()
    );

    // Switch off: the general dual-basis reduction proves the identity map
    // too, and the (env-independent) verifier accepts it as well.
    unsafe { std::env::set_var("F2Z_VIRT_ID_FAST", "0") };
    let mut pt = Blake3Transcript::new();
    let general = prove_mle_eval_mod_q_ligerito_virtual(
        &mut pt, &hint, &rows_f, &p, &p, &map, &rw_q, Q_BITS, alpha, &pc,
    );
    assert_proof_pin(
        "virtual identity AdjointBatch",
        &general.to_bytes(),
        &mut pt,
        "657d18cf6dd2683e6a8d20427d0b10e9f201f340337d20e6e34ce31e335a3473",
        265_177_699_075_422_809_345_164_704_473_886_491_828,
    );
    unsafe { std::env::remove_var("F2Z_VIRT_ID_FAST") };
    assert!(
        matches!(
            general.reduction,
            VirtualReductionProof::AdjointBatch { .. }
        ),
        "F2Z_VIRT_ID_FAST=0 must fall back to AdjointBatch"
    );
    let mut vt = Blake3Transcript::new();
    verify_mle_eval_mod_q_ligerito_virtual(
        &mut vt,
        &hint.commitment,
        &general,
        &p,
        &p,
        &map,
        &rw_q,
        &col_w,
        alpha,
        y,
        Q_BITS,
        &vc,
    )
    .expect("AdjointBatch on an identity map verifies");
}
