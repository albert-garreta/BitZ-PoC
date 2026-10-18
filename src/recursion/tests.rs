//! End-to-end: real `sha256-fs` proofs through the verifier circuit.

use std::sync::Arc;

use circuit::witgen::CheckingWitgen;
use field::Gf128 as Gf;
use flock_core::pcs::ligerito::LigeritoProfile;

use crate::bitz::{BitZParams, BitZProver, LinearClaim, Pcs, Shape, WINDOW, build_prover};
use crate::ligerito_flock::LigeritoSelection;
use crate::ligerito_flock::grinding_plan::GrindingPlan;
use crate::transcript::sha::{TraceOp, start_trace, take_trace};

use super::arith::Zq;
use super::bitz_pcs::{ClaimIn, verify_direct};
use super::bitz_transcript::BitzTranscript;
use super::sha::first_divergence;
use super::val::PublicStream;

const Q: u128 = (1u128 << 100) - 15;

fn xorshift(state: &mut u64) -> u64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state
}

/// A direct claim on random bits, proved natively; returns everything the
/// circuit needs and the native transcript trace of the VERIFIER.
fn direct_fixture(
    log_rows: usize,
    log_cols: usize,
) -> (BitZParams, Pcs, [u8; 32], LinearClaim, crate::bitz::Proof, Vec<TraceOp>) {
    let shape = Shape::new(log_rows, log_cols).unwrap();
    let resolved = LigeritoSelection::CustomJohnson { log_inv_rate: 1, initial_k: 3 }
        .resolve(shape.log_bits() - 7, 100)
        .unwrap();
    let config = resolved.security().clone();
    let plan = Arc::new(GrindingPlan::resolve(&config, 100).unwrap());
    let pcs = Pcs::with_security_and_work(&shape, &config, LigeritoProfile::Fast, plan).unwrap();
    let params = BitZParams::new(shape, Q, crate::pcs::smallest_generator().into()).unwrap();
    let mut state = 0x5eed_1234_abcd_0001u64;
    let rows: Vec<Vec<u64>> = (0..shape.columns())
        .map(|_| (0..shape.rows() / 64).map(|_| xorshift(&mut state)).collect())
        .collect();
    let row_weights: Vec<u128> = (0..shape.rows())
        .map(|_| ((u128::from(xorshift(&mut state)) << 64) | u128::from(xorshift(&mut state))) % Q)
        .collect();
    let column_weights: Vec<u128> = (0..shape.columns())
        .map(|_| ((u128::from(xorshift(&mut state)) << 64) | u128::from(xorshift(&mut state))) % Q)
        .collect();
    // target = Σ_{i,j} w_i v_j f_ij mod Q.
    let mut target = num_bigint::BigUint::from(0u8);
    for (j, row) in rows.iter().enumerate() {
        for i in 0..shape.rows() {
            if (row[i / 64] >> (i % 64)) & 1 == 1 {
                target += num_bigint::BigUint::from(row_weights[i]) * num_bigint::BigUint::from(column_weights[j]);
            }
        }
    }
    let target = (target % num_bigint::BigUint::from(Q))
        .to_u64_digits()
        .iter()
        .enumerate()
        .fold(0u128, |v, (i, d)| v | (u128::from(*d) << (64 * i)));
    let (root, hint) = pcs.commit(&shape, rows).unwrap();
    let claim = LinearClaim::new(&params, row_weights, column_weights, target).unwrap();
    let mut prover = build_prover(b"recursion-direct", b"fixture");
    BitZProver::new(params, WINDOW)
        .prove(&claim, &pcs, &hint, &mut prover, None)
        .unwrap();
    let proof = prover.finish();
    start_trace();
    let verifier = crate::bitz::BitZVerifier::new(params, WINDOW);
    let state = crate::bitz::build_verifier(b"recursion-direct", b"fixture", &proof);
    verifier.verify(&claim, &pcs, root, state, None).unwrap();
    let trace: Vec<TraceOp> = take_trace().into_iter().map(|e| e.op).collect();
    (params, pcs, root.0, claim, proof, trace)
}

fn run_direct<CS: circuit::Circuit>(
    cs: &mut CS,
    public: super::val::Public,
    params: &BitZParams,
    pcs: &Pcs,
    root: [u8; 32],
    claim: &LinearClaim,
    proof: Option<&crate::bitz::Proof>,
    log: bool,
) -> (BitzTranscript<CS::Bool>, super::bitz_pcs::OpeningCounts) {
    let claim_in = ClaimIn {
        row_weights: claim.row_weights().to_vec(),
        column_weights: claim.column_weights().to_vec(),
        target: Zq::P(claim.target()),
    };
    let mut t = BitzTranscript::new(cs, public, b"recursion-direct", b"fixture", proof, log);
    let counts = verify_direct(cs, params, pcs, root, &claim_in, &mut t, None);
    (t, counts)
}

#[test]
fn direct_opening_circuit_accepts_a_real_proof() {
    let (params, pcs, root, claim, proof, trace) = direct_fixture(12, 8);
    let public = PublicStream::discover();
    let mut w = CheckingWitgen::new();
    let (t, counts) = run_direct(&mut w, public.clone(), &params, &pcs, root, &claim, Some(&proof), true);
    let log = t.duplex.log.clone().unwrap();
    if let Some((i, a, b)) = first_divergence(&trace, &log) {
        panic!("transcript diverges at op {i}: native {a:?} circuit {b:?}");
    }
    assert_eq!(public.borrow().failed, None);
    assert_eq!(w.first_failure, None, "honest proof satisfies the circuit");
    let st = public.borrow();
    eprintln!(
        "direct 2^{}: public {} values / {} bytes; transcript sha {:?}; ligerito {:?}; rows {}",
        params.shape().log_bits(),
        st.values.len(),
        st.bytes(),
        t.counts(),
        counts.ligerito,
        w.rows,
    );
}

/// The hint stream with its Ligerito proof replaced (`[u32 len][bincode]`).
fn with_ligerito(proof: &crate::bitz::Proof, edit: impl FnOnce(&mut flock_core::pcs::ligerito::LigeritoProof)) -> crate::bitz::Proof {
    use bincode::Options;
    let len = u32::from_le_bytes(proof.hints[..4].try_into().unwrap()) as usize;
    let options = crate::bitz::pcs::proof_options();
    let mut lig: flock_core::pcs::ligerito::LigeritoProof = options.deserialize(&proof.hints[4..4 + len]).unwrap();
    edit(&mut lig);
    let bytes = crate::bitz::pcs::proof_options().serialize(&lig).unwrap();
    let mut hints = (bytes.len() as u32).to_le_bytes().to_vec();
    hints.extend_from_slice(&bytes);
    hints.extend_from_slice(&proof.hints[4 + len..]);
    crate::bitz::Proof { narg_string: proof.narg_string.clone(), hints }
}

/// Tampered proof data breaks the circuit when the prover keeps the honest
/// public stream: narg bytes (folds, GKR and sumcheck messages, claims, the
/// Ligerito echo) and the opened rows and siblings of every level. The
/// hint blob's copies of narg values (roots, nonces, messages) are not read
/// by the circuit — the narg echo is the one witness of each.
#[test]
fn direct_opening_circuit_rejects_tampered_proofs() {
    let (params, pcs, root, claim, proof, _) = direct_fixture(12, 8);
    let public = PublicStream::discover();
    let mut w = CheckingWitgen::new();
    run_direct(&mut w, public.clone(), &params, &pcs, root, &claim, Some(&proof), false);
    assert_eq!(w.first_failure, None);
    let honest = public.borrow().clone();
    let narg = proof.narg_string.len();
    let mut cases: Vec<(String, crate::bitz::Proof)> = Vec::new();
    for k in 0..16 {
        let at = (k * (narg - 1)) / 15;
        let mut bad = proof.clone();
        bad.narg_string[at] ^= 0x10;
        cases.push((format!("narg byte {at}/{narg}"), bad));
    }
    let one = Gf::ONE;
    cases.push(("L0 row".into(), with_ligerito(&proof, |l| l.initial_proof.opened_rows[3][1] += one)));
    cases.push(("L0 sibling".into(), with_ligerito(&proof, |l| l.initial_proof.merkle_proof[5][7] ^= 1)));
    cases.push(("last row".into(), with_ligerito(&proof, |l| l.final_proof.opened_rows[0][0] += one)));
    cases.push(("last sibling".into(), with_ligerito(&proof, |l| {
        let n = l.final_proof.merkle_proof.len();
        l.final_proof.merkle_proof[n - 1][0] ^= 1
    })));
    if !proof_has_no_recursive_levels(&proof) {
        cases.push(("mid row".into(), with_ligerito(&proof, |l| l.recursive_proofs[0].opened_rows[2][3] += one)));
        cases.push(("mid sibling".into(), with_ligerito(&proof, |l| l.recursive_proofs[0].merkle_proof[1][31] ^= 1)));
    }
    for (what, bad) in cases {
        // The prover re-derives every public value from the tampered proof
        // (the only stream that can be consistent with it): rejected. When
        // that stream equals the honest one the tamper hit committed data
        // only, and the honest stream is rejected too.
        let stream = PublicStream::discover();
        let mut w = CheckingWitgen::new();
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            run_direct(&mut w, stream.clone(), &params, &pcs, root, &claim, Some(&bad), false);
        }));
        let rejected = outcome.is_err() || w.first_failure.is_some() || stream.borrow().failed.is_some();
        assert!(rejected, "tampered {what} accepted");
        let derived = stream.borrow().values.clone();
        if derived.len() >= honest.values.len() && derived[..honest.values.len()] == honest.values[..] {
            let replay = PublicStream::replay(honest.values.clone(), honest.widths.clone());
            let mut w = CheckingWitgen::new();
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                run_direct(&mut w, replay.clone(), &params, &pcs, root, &claim, Some(&bad), false);
            }));
            assert!(
                outcome.is_err() || w.first_failure.is_some() || replay.borrow().failed.is_some(),
                "tampered {what} accepted against the honest stream"
            );
        }
    }
}

fn proof_has_no_recursive_levels(proof: &crate::bitz::Proof) -> bool {
    let mut empty = true;
    with_ligerito(proof, |l| empty = l.recursive_proofs.is_empty());
    empty
}

#[test]
fn direct_opening_circuit_records_one_template_and_counts() {
    use circuit::compact::CompactRecorder;
    use circuit::stats::PhaseStats;
    let (params, pcs, root, claim, proof, _) = direct_fixture(12, 8);
    let public = PublicStream::discover();
    let mut w = CheckingWitgen::new();
    run_direct(&mut w, public.clone(), &params, &pcs, root, &claim, Some(&proof), false);
    let stream = public.borrow().clone();
    // The outer verifier's view: replay the public stream, no proof.
    let replay = PublicStream::replay(stream.values.clone(), stream.widths.clone());
    let mut rec = CompactRecorder::new();
    run_direct(&mut rec, replay.clone(), &params, &pcs, root, &claim, None, false);
    assert!(replay.borrow().exhausted());
    let st = rec.finish_structured();
    assert_eq!(st.templates.len(), 1, "one SHA-256 compression template");
    let tpl = &st.templates[0];
    eprintln!(
        "structured: committed {} cells {} rows {}; template hints {} cells {} rows {} m_nnz {}; instances {}; tail hints {} cells {} rows {} m_nnz {}",
        st.committed, st.cells, st.rows, tpl.hints, tpl.cells(), tpl.rows(), tpl.m_nnz(), st.instances.len(),
        st.tail_hints.len(), st.tail_cells.len(), st.tail_rows.len(), st.tail_m_nnz(),
    );
    // (The recording is checked against the witness on small circuits in
    // `sha::tests`; flattening this one costs gigabytes.)
    drop(w);
    let mut stats = PhaseStats::new();
    let replay = PublicStream::replay(stream.values.clone(), stream.widths.clone());
    run_direct(&mut stats, replay, &params, &pcs, root, &claim, None, false);
    eprintln!("totals {:?}", stats.total);
}

/// Everything a recording of the direct-opening verifier is a function of.
fn direct_source_digest(params: &BitZParams, pcs: &Pcs, root: [u8; 32], claim: &LinearClaim, stream: &PublicStream) -> [u8; 32] {
    use spongefish::Encoding;
    let mut h = blake3::Hasher::new();
    h.update(b"bitz/recursion/direct-opening/v1");
    h.update(params.encode().as_ref());
    h.update(pcs.encode().as_ref());
    h.update(&root);
    h.update(claim.encode().as_ref());
    for (v, w) in stream.values.iter().zip(&stream.widths) {
        h.update(&v.to_le_bytes());
        h.update(&[*w]);
    }
    *h.finalize().as_bytes()
}

/// One recursion step: BitZ (SHA-256 variant, structured linear relation)
/// proves that the circuit verifying a 2^n-bit BitZ opening is satisfied.
/// `BITZ_REC_SHAPE=t:s` (default 12:8), `BITZ_REC_LIG` (outer Ligerito,
/// default custom:3:4), `BITZ_REC_T` (outer opening rows).
#[test]
#[ignore = "heavy: proves a 2^24-bit verifier circuit"]
fn recursion_step_proves_the_direct_opening_verifier() {
    use circuit::compact::CompactRecorder;
    use std::time::Instant;
    let shape = std::env::var("BITZ_REC_SHAPE").unwrap_or_else(|_| "12:8".into());
    let (t, s) = shape.split_once(':').map(|(a, b)| (a.parse().unwrap(), b.parse().unwrap())).unwrap();
    let started = Instant::now();
    let (params, pcs, root, claim, proof, _) = direct_fixture(t, s);
    let inner_bytes = proof.narg_string.len() + proof.hints.len();
    println!("inner: 2^{} bits, proof {} B ({:.2} s incl. native verify)", t + s, inner_bytes, started.elapsed().as_secs_f64());

    let started = Instant::now();
    let public = PublicStream::discover();
    let mut w = CheckingWitgen::new();
    let (tr, counts) = run_direct(&mut w, public.clone(), &params, &pcs, root, &claim, Some(&proof), false);
    assert_eq!(w.first_failure, None);
    assert_eq!(public.borrow().failed, None);
    let stream = public.borrow().clone();
    println!("witness: {:.2} s; transcript {:?}, ligerito {:?}", started.elapsed().as_secs_f64(), tr.counts(), counts.ligerito);

    let started = Instant::now();
    let replay = PublicStream::replay(stream.values.clone(), stream.widths.clone());
    let mut rec = CompactRecorder::new();
    run_direct(&mut rec, replay, &params, &pcs, root, &claim, None, false);
    let st = rec.finish_structured();
    println!(
        "record: {:.2} s; committed {} cells {} rows {}; {} blocks; tail cells {} m_nnz {} groups {}",
        started.elapsed().as_secs_f64(), st.committed, st.cells, st.rows, st.instances.len(),
        st.tail_cells.len(), st.tail_m_nnz(), st.groups.len(),
    );

    let selection = LigeritoSelection::parse(
        &std::env::var("BITZ_REC_LIG").unwrap_or_else(|_| "custom:3:4".into()),
        100,
    )
    .unwrap();
    let opening_t: Option<usize> = std::env::var("BITZ_REC_T").ok().map(|v| v.parse().unwrap());
    let started = Instant::now();
    let digest = direct_source_digest(&params, &pcs, root, &claim, &stream);
    let rel = crate::piop::spartan::flockrec::FlockRecRelation::new(Arc::new(st), digest, selection, opening_t)
        .expect("relation");
    println!("relation: {:.2} s; committed {:?}, derived {:?}", started.elapsed().as_secs_f64(), rel.committed_layout(), rel.derived_layout());
    let (g, h) = w.into_witnesses();
    let wit = rel.witness(&g, &h).expect("witness layout");
    let started = Instant::now();
    let hint = rel.commit(&wit);
    let commit_s = started.elapsed().as_secs_f64();
    let started = Instant::now();
    let mut tp = crate::transcript::Sha256Transcript::new();
    let outer = rel.prove(&mut tp, &wit, &hint).expect("outer prove");
    let prove_s = started.elapsed().as_secs_f64();
    let started = Instant::now();
    let mut tv = crate::transcript::Sha256Transcript::new();
    rel.verify(&mut tv, &hint.commitment, &outer).expect("outer verify");
    let verify_s = started.elapsed().as_secs_f64();
    let opening = outer.bitz.to_bytes().len();
    let commitment = bincode::serialize(&hint.commitment).unwrap().len();
    println!(
        "OUTER: commit {commit_s:.2} s, prove {prove_s:.2} s, verify {verify_s:.2} s; proof {} B (opening {opening}, PIOP {}, commitment {commitment}) + public stream {} B; inner proof was {inner_bytes} B",
        opening + outer.piop_bytes() + commitment,
        outer.piop_bytes(),
        stream.bytes(),
    );
}

/// Level 1 (the direct opening's verifier proved by FlockRec) as a reusable
/// fixture: the relation, its commitment and proof, and the inner proof size.
fn level_one(
    selection: &str,
    opening_t: Option<usize>,
) -> (crate::piop::spartan::flockrec::FlockRecRelation, flock_core::pcs::commit::Commitment, crate::piop::spartan::flockrec::FlockRecProof, usize, usize) {
    use circuit::compact::CompactRecorder;
    let (params, pcs, root, claim, proof, _) = direct_fixture(12, 8);
    let inner_bytes = proof.narg_string.len() + proof.hints.len();
    let public = PublicStream::discover();
    let mut w = CheckingWitgen::new();
    run_direct(&mut w, public.clone(), &params, &pcs, root, &claim, Some(&proof), false);
    assert_eq!(w.first_failure, None);
    let stream = public.borrow().clone();
    let replay = PublicStream::replay(stream.values.clone(), stream.widths.clone());
    let mut rec = CompactRecorder::new();
    run_direct(&mut rec, replay, &params, &pcs, root, &claim, None, false);
    let st = rec.finish_structured();
    let digest = direct_source_digest(&params, &pcs, root, &claim, &stream);
    let rel = crate::piop::spartan::flockrec::FlockRecRelation::new(
        Arc::new(st),
        digest,
        LigeritoSelection::parse(selection, 100).unwrap(),
        opening_t,
    )
    .unwrap();
    let (g, h) = w.into_witnesses();
    let wit = rel.witness(&g, &h).unwrap();
    let hint = rel.commit(&wit);
    let mut tp = crate::transcript::Sha256Transcript::new();
    let outer = rel.prove(&mut tp, &wit, &hint).unwrap();
    (rel, hint.commitment, outer, inner_bytes, stream.bytes())
}

/// Level 2: the circuit verifying a FlockRec proof accepts the level-1
/// proof, its transcripts are the native verifier's op for op, and BitZ
/// proves it again — the recursion closes. `BITZ_REC_LIG`, `BITZ_REC_T`.
#[test]
#[ignore = "heavy: two recursion levels"]
fn recursion_closes_on_the_outer_relation() {
    use circuit::compact::CompactRecorder;
    use std::time::Instant;
    let selection = std::env::var("BITZ_REC_LIG").unwrap_or_else(|_| "custom:3:4".into());
    let opening_t: Option<usize> = Some(std::env::var("BITZ_REC_T").ok().map_or(19, |v| v.parse().unwrap()));
    let started = Instant::now();
    let (rel1, com1, pi1, inner_bytes, stream1) = level_one(&selection, opening_t);
    let pi1_bytes = pi1.bitz.to_bytes().len() + pi1.piop_bytes() + bincode::serialize(&com1).unwrap().len();
    println!("level 1: {:.2} s; inner {inner_bytes} B -> pi1 {pi1_bytes} B + public {stream1} B", started.elapsed().as_secs_f64());

    // The native verifier's transcripts, for the op-by-op comparison.
    start_trace();
    let mut tv = crate::transcript::Sha256Transcript::new();
    rel1.verify(&mut tv, &com1, &pi1).expect("native level-1 verify");
    let trace = take_trace();
    let native_spartan: Vec<TraceOp> = trace.iter().filter(|e| e.duplex == 0).map(|e| e.op.clone()).collect();
    let native_bitz: Vec<TraceOp> = trace.iter().filter(|e| e.duplex == 1).map(|e| e.op.clone()).collect();

    let started = Instant::now();
    let public = PublicStream::discover();
    let mut w = CheckingWitgen::new();
    let mut logs = (Vec::new(), Vec::new());
    let counts = super::flockrec_verifier::verify_flockrec(&mut w, &rel1, &com1, Some(&pi1), public.clone(), Some(&mut logs));
    if let Some((i, a, b)) = first_divergence(&native_spartan, &logs.0) {
        panic!("spartan transcript diverges at op {i}: native {a:?} circuit {b:?}");
    }
    if let Some((i, a, b)) = first_divergence(&native_bitz, &logs.1) {
        panic!("opening transcript diverges at op {i}: native {a:?} circuit {b:?}");
    }
    assert_eq!(public.borrow().failed, None);
    assert_eq!(w.first_failure, None, "the level-1 proof satisfies the level-2 circuit");
    let stream = public.borrow().clone();
    println!("level-2 witness: {:.2} s; {counts:?}", started.elapsed().as_secs_f64());

    let started = Instant::now();
    let replay = PublicStream::replay(stream.values.clone(), stream.widths.clone());
    let mut rec = CompactRecorder::new();
    super::flockrec_verifier::verify_flockrec(&mut rec, &rel1, &com1, None, replay.clone(), None);
    assert!(replay.borrow().exhausted());
    let st = rec.finish_structured();
    println!(
        "level-2 circuit: {:.2} s; committed {} cells {} rows {}; {} blocks; tail cells {} groups {}; public {} B",
        started.elapsed().as_secs_f64(), st.committed, st.cells, st.rows, st.instances.len(), st.tail_cells.len(), st.groups.len(), stream.bytes(),
    );
    let mut h = blake3::Hasher::new();
    h.update(b"bitz/recursion/flockrec-level/v1");
    h.update(&bincode::serialize(&com1).unwrap());
    for (v, wd) in stream.values.iter().zip(&stream.widths) {
        h.update(&v.to_le_bytes());
        h.update(&[*wd]);
    }
    let rel2 = crate::piop::spartan::flockrec::FlockRecRelation::new(
        Arc::new(st),
        *h.finalize().as_bytes(),
        LigeritoSelection::parse(&selection, 100).unwrap(),
        opening_t,
    )
    .unwrap();
    let (g, hh) = w.into_witnesses();
    let wit = rel2.witness(&g, &hh).unwrap();
    let started = Instant::now();
    let hint = rel2.commit(&wit);
    let mut tp = crate::transcript::Sha256Transcript::new();
    let pi2 = rel2.prove(&mut tp, &wit, &hint).expect("level-2 prove");
    let prove_s = started.elapsed().as_secs_f64();
    let mut tv = crate::transcript::Sha256Transcript::new();
    rel2.verify(&mut tv, &hint.commitment, &pi2).expect("level-2 verify");
    let pi2_bytes = pi2.bitz.to_bytes().len() + pi2.piop_bytes() + bincode::serialize(&hint.commitment).unwrap().len();
    println!("level 2: prove {prove_s:.2} s; pi2 {pi2_bytes} B + public {} B", stream.bytes());
}

/// The level-2 circuit rejects tampered level-1 proofs: a Spartan round
/// coefficient, opening narg bytes, an opened row and a sibling.
#[test]
#[ignore = "heavy: one recursion level plus tampered replays"]
fn level_two_circuit_rejects_tampered_outer_proofs() {
    let (rel1, com1, pi1, _, _) = level_one("custom:6:4", Some(21));
    let accepts = |p: &crate::piop::spartan::flockrec::FlockRecProof| -> bool {
        let public = PublicStream::discover();
        let mut w = CheckingWitgen::new();
        let ok = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            super::flockrec_verifier::verify_flockrec(&mut w, &rel1, &com1, Some(p), public.clone(), None);
        }))
        .is_ok();
        ok && w.first_failure.is_none() && public.borrow().failed.is_none()
    };
    assert!(accepts(&pi1), "the honest level-1 proof is accepted");
    let mut cases: Vec<(String, crate::piop::spartan::flockrec::FlockRecProof)> = Vec::new();
    {
        // A different canonical coefficient (another round's).
        let mut bad = pi1.clone();
        bad.inner.round_polynomials[3][1] = bad.inner.round_polynomials[4][1];
        cases.push(("inner round coefficient".into(), bad));
    }
    let narg = pi1.bitz.transcript.narg_string.len();
    for k in 0..6 {
        let at = (k * (narg - 1)) / 5;
        let mut bad = pi1.clone();
        bad.bitz.transcript.narg_string[at] ^= 0x04;
        cases.push((format!("opening narg byte {at}/{narg}"), bad));
    }
    let one = Gf::ONE;
    let mut bad = pi1.clone();
    bad.bitz.transcript = with_ligerito(&pi1.bitz.transcript, |l| l.initial_proof.opened_rows[1][2] += one);
    cases.push(("opened row".into(), bad));
    let mut bad = pi1.clone();
    bad.bitz.transcript = with_ligerito(&pi1.bitz.transcript, |l| l.initial_proof.merkle_proof[2][3] ^= 1);
    cases.push(("sibling".into(), bad));
    for (what, bad) in cases {
        assert!(!accepts(&bad), "tampered {what} accepted by the level-2 circuit");
    }
}

/// The SHA-256+P-256 relation the recursion's base proofs take: 2^k
/// compressions, the dense opening with `BITZ_REC_ECDSA_T` claim rows
/// (default [`hooks::DENSE_CLAIM_LOG_ROWS`]) under `BITZ_REC_BASE_LIG`
/// (default the rate-1/2 Johnson ladder).
fn ecdsa_prepared(log_compressions: u8) -> crate::piop::spartan::ecdsa_sha256::PreparedSha256Ecdsa {
    use crate::piop::spartan::ecdsa_sha256::*;
    let rows = std::env::var("BITZ_REC_ECDSA_T").ok().map_or(hooks::DENSE_CLAIM_LOG_ROWS, |v| v.parse().unwrap());
    let selection = std::env::var("BITZ_REC_BASE_LIG")
        .ok()
        .map_or(LigeritoSelection::JOHNSON, |v| LigeritoSelection::parse(&v, 100).unwrap());
    let mut prepared = prepare_sha256_ecdsa(log_compressions as usize, 100, OuterMode::Split)
        .unwrap()
        .with_ligerito(selection)
        .unwrap();
    hooks::force_dense(&mut prepared, rows).unwrap();
    prepared
}

/// A real SHA-256+P-256 proof (2^k compressions, dense opening, SHA-256
/// variant) through the base-layer circuit.
fn ecdsa_fixture(
    log_compressions: u8,
) -> (
    crate::piop::spartan::ecdsa_sha256::PreparedSha256Ecdsa,
    crate::piop::spartan::ecdsa_sha256::Sha256EcdsaStatement,
    flock_core::pcs::commit::Commitment,
    crate::piop::spartan::ecdsa_sha256::Sha256EcdsaProof,
) {
    use crate::piop::spartan::ecdsa_sha256::*;
    let (statement, message) = crate::piop::spartan::ecdsa_sha256::tests::fixture_at(log_compressions);
    let prepared = ecdsa_prepared(log_compressions);
    let witness = generate_sha256_ecdsa_witness(&prepared, &statement, &message).unwrap();
    let hint = commit_sha256_ecdsa(&prepared, &witness).unwrap();
    let mut tp = crate::transcript::Sha256Transcript::new();
    let proof = prove_sha256_ecdsa(&mut tp, &prepared, &statement, &witness, &hint, 4).unwrap();
    (prepared, statement, hint.commitment, proof)
}

#[test]
fn sha256_ecdsa_circuit_accepts_a_real_proof() {
    use crate::piop::spartan::ecdsa_sha256::verify_sha256_ecdsa;
    let (prepared, statement, commitment, proof) = ecdsa_fixture(3);
    start_trace();
    let mut tv = crate::transcript::Sha256Transcript::new();
    verify_sha256_ecdsa(&mut tv, &prepared, &statement, &commitment, &proof).expect("native verify");
    let trace = take_trace();
    let native_spartan: Vec<TraceOp> = trace.iter().filter(|e| e.duplex == 0).map(|e| e.op.clone()).collect();
    let native_bitz: Vec<TraceOp> = trace.iter().filter(|e| e.duplex == 1).map(|e| e.op.clone()).collect();

    let public = PublicStream::discover();
    let mut w = CheckingWitgen::new();
    let mut logs = (Vec::new(), Vec::new());
    let counts = super::ecdsa_verifier::verify_sha256_ecdsa(&mut w, &prepared, &statement, &commitment, Some(&proof), public.clone(), Some(&mut logs));
    if let Some((i, a, b)) = first_divergence(&native_spartan, &logs.0) {
        panic!("spartan transcript diverges at op {i}: native {a:?} circuit {b:?}");
    }
    if let Some((i, a, b)) = first_divergence(&native_bitz, &logs.1) {
        panic!("opening transcript diverges at op {i}: native {a:?} circuit {b:?}");
    }
    assert_eq!(public.borrow().failed, None);
    assert_eq!(w.first_failure, None, "the SHA+ECDSA proof satisfies the circuit");
    eprintln!("sha256+ecdsa 2^3: public {} B; {counts:?}; committed {} cells {}", public.borrow().bytes(), w.witness().bit_len(), w.integer_witness().bit_len());
}

/// The base of the chain: a SHA-256+P-256 proof, its verifier circuit
/// proved by FlockRec (then the fixed-point levels above it behave as in
/// `recursion_closes_on_the_outer_relation`). `BITZ_REC_ECDSA_LOG` (default
/// 3), `BITZ_REC_LIG`, `BITZ_REC_T`.
#[test]
#[ignore = "heavy: SHA+ECDSA proof, its verifier circuit, one FlockRec proof"]
fn recursion_compresses_a_sha256_ecdsa_proof() {
    use circuit::compact::CompactRecorder;
    use std::time::Instant;
    let log: u8 = std::env::var("BITZ_REC_ECDSA_LOG").ok().map_or(3, |v| v.parse().unwrap());
    let selection = std::env::var("BITZ_REC_LIG").unwrap_or_else(|_| "custom:6:4".into());
    let opening_t: Option<usize> = Some(std::env::var("BITZ_REC_T").ok().map_or(19, |v| v.parse().unwrap()));
    let started = Instant::now();
    let (prepared, statement, commitment, proof) = ecdsa_fixture(log);
    let base_bytes = proof.to_bytes().len();
    println!("base: SHA-256 2^{log} + P-256, proof {base_bytes} B ({:.2} s)", started.elapsed().as_secs_f64());
    let started = Instant::now();
    let public = PublicStream::discover();
    let mut w = CheckingWitgen::new();
    let counts = super::ecdsa_verifier::verify_sha256_ecdsa(&mut w, &prepared, &statement, &commitment, Some(&proof), public.clone(), None);
    assert_eq!(w.first_failure, None);
    assert_eq!(public.borrow().failed, None);
    let stream = public.borrow().clone();
    println!("witness: {:.2} s; {counts:?}", started.elapsed().as_secs_f64());
    let replay = PublicStream::replay(stream.values.clone(), stream.widths.clone());
    let mut rec = CompactRecorder::new();
    super::ecdsa_verifier::verify_sha256_ecdsa(&mut rec, &prepared, &statement, &commitment, None, replay, None);
    let st = rec.finish_structured();
    println!("circuit: committed {} cells {} rows {}; {} blocks; public {} B", st.committed, st.cells, st.rows, st.instances.len(), stream.bytes());
    let mut h = blake3::Hasher::new();
    h.update(b"bitz/recursion/sha256-ecdsa/v1");
    h.update(&statement.qx);
    h.update(&statement.qy);
    h.update(&statement.r);
    h.update(&statement.s);
    h.update(&[statement.log_compressions]);
    h.update(&bincode::serialize(&commitment).unwrap());
    for (v, wd) in stream.values.iter().zip(&stream.widths) {
        h.update(&v.to_le_bytes());
        h.update(&[*wd]);
    }
    let rel = crate::piop::spartan::flockrec::FlockRecRelation::new(
        Arc::new(st),
        *h.finalize().as_bytes(),
        LigeritoSelection::parse(&selection, 100).unwrap(),
        opening_t,
    )
    .unwrap();
    let (g, hh) = w.into_witnesses();
    let wit = rel.witness(&g, &hh).unwrap();
    let started = Instant::now();
    let hint = rel.commit(&wit);
    let mut tp = crate::transcript::Sha256Transcript::new();
    let pi = rel.prove(&mut tp, &wit, &hint).expect("FlockRec prove");
    let prove_s = started.elapsed().as_secs_f64();
    let mut tv = crate::transcript::Sha256Transcript::new();
    rel.verify(&mut tv, &hint.commitment, &pi).expect("FlockRec verify");
    let bytes = pi.bitz.to_bytes().len() + pi.piop_bytes() + bincode::serialize(&hint.commitment).unwrap().len();
    println!("OUTER: prove {prove_s:.2} s; proof {bytes} B + public {} B (base proof {base_bytes} B)", stream.bytes());
}

/// The FlockRec relation of a base proof's verifier circuit, rebuilt from
/// public data only (what the final verifier runs): the circuit recorded on
/// the public stream, its digest over the statement, the base commitment and
/// the stream.
fn ecdsa_outer_relation(
    prepared: &crate::piop::spartan::ecdsa_sha256::PreparedSha256Ecdsa,
    statement: &crate::piop::spartan::ecdsa_sha256::Sha256EcdsaStatement,
    commitment: &flock_core::pcs::commit::Commitment,
    stream: &super::val::PublicStream,
    selection: &str,
    opening_t: Option<usize>,
) -> (crate::piop::spartan::flockrec::FlockRecRelation, f64, f64, [usize; 8], [u8; 32]) {
    use circuit::compact::CompactRecorder;
    use std::time::Instant;
    let started = Instant::now();
    let replay = PublicStream::replay(stream.values.clone(), stream.widths.clone());
    let mut rec = CompactRecorder::new();
    super::ecdsa_verifier::verify_sha256_ecdsa(&mut rec, prepared, statement, commitment, None, replay, None);
    let st = rec.finish_structured();
    let record_s = started.elapsed().as_secs_f64();
    let circuit_digest = st.digest();
    let (rel, relation_s, stats) = outer_relation_from(st, statement, commitment, stream, selection, opening_t);
    (rel, record_s, relation_s, stats, circuit_digest)
}

/// [`ecdsa_outer_relation`] from a recording: the digest of its sources and
/// the FlockRec relation. Returns the relation, its build time and the
/// circuit's shape `[committed, cells, rows, blocks, n, inst_vars, f_bits, h_bits]`.
fn outer_relation_from(
    st: circuit::compact::StructuredCircuit,
    statement: &crate::piop::spartan::ecdsa_sha256::Sha256EcdsaStatement,
    commitment: &flock_core::pcs::commit::Commitment,
    stream: &super::val::PublicStream,
    selection: &str,
    opening_t: Option<usize>,
) -> (crate::piop::spartan::flockrec::FlockRecRelation, f64, [usize; 8]) {
    let stats = [st.committed, st.cells, st.rows, st.instances.len()];
    let started = std::time::Instant::now();
    let mut h = blake3::Hasher::new();
    h.update(b"bitz/recursion/sha256-ecdsa/v1");
    h.update(&statement.qx);
    h.update(&statement.qy);
    h.update(&statement.r);
    h.update(&statement.s);
    h.update(&[statement.log_compressions]);
    h.update(&bincode::serialize(commitment).unwrap());
    for (v, wd) in stream.values.iter().zip(&stream.widths) {
        h.update(&v.to_le_bytes());
        h.update(&[*wd]);
    }
    let rel = crate::piop::spartan::flockrec::FlockRecRelation::new(
        Arc::new(st),
        *h.finalize().as_bytes(),
        LigeritoSelection::parse(selection, 100).unwrap(),
        opening_t,
    )
    .unwrap();
    let bits = |l: &crate::pcs::IntegerMatrixLayout| l.row_vars + l.col_vars;
    let stats = [stats[0], stats[1], stats[2], stats[3], rel.n, rel.inst_vars, bits(&rel.f_layout), bits(&rel.h_layout)];
    (rel, started.elapsed().as_secs_f64(), stats)
}

/// Digests pinning a recursion step's artifacts (prover optimizations must
/// leave them unchanged): the circuit witness `(w, M·w)` and the outer proof.
fn witness_digest(g: &circuit::witgen::PackedWitness, h: &circuit::witgen::PackedWitness) -> [u8; 32] {
    let mut d = blake3::Hasher::new();
    for w in [g, h] {
        d.update(&(w.bit_len() as u64).to_le_bytes());
        for x in w.words() {
            d.update(&x.to_le_bytes());
        }
    }
    *d.finalize().as_bytes()
}

fn flockrec_proof_digest(pi: &crate::piop::spartan::flockrec::FlockRecProof) -> [u8; 32] {
    let mut d = blake3::Hasher::new();
    d.update(&pi.initial_nonce.to_le_bytes());
    d.update(&pi.terminal_nonce.to_le_bytes());
    d.update(format!("{:?}", pi.inner.round_polynomials).as_bytes());
    for n in &pi.inner_nonces {
        d.update(&n.to_le_bytes());
    }
    d.update(&pi.bitz.to_bytes());
    *d.finalize().as_bytes()
}

fn hex(d: &[u8; 32]) -> String {
    d.iter().map(|b| format!("{b:02x}")).collect()
}

fn median(mut xs: Vec<f64>) -> f64 {
    xs.sort_by(f64::total_cmp);
    xs[xs.len() / 2]
}

/// Prove times and sizes of 2^n SHA-256 compressions + one P-256 signature
/// before and after one recursion step. Base: witness, commitment and
/// proof of the SHA-256-variant relation ([`ecdsa_prepared`]). Recursion:
/// the verifier circuit's witness ([`circuit::witgen::Witgen`]), its
/// structure recorded on the public stream, the FlockRec relation, its
/// witness, commitment and proof. After: the final verifier rebuilds the
/// relation from the public stream and checks the FlockRec proof. Medians
/// over `BITZ_REC_REPS` (default 3) runs; one `TIMING {json}` line.
/// `BITZ_REC_ECDSA_LOG` (n), `BITZ_REC_LIG`, `BITZ_REC_T` (outer opening
/// rows, default 19: with the split block layout the derived grid is 2^26,
/// and 2^19 rows keep the outer proof at the T=21 unsplit size; 20 is 1 KB
/// smaller and its GKR 40 % slower), plus [`ecdsa_prepared`]'s knobs.
#[test]
#[ignore = "heavy: timed SHA+ECDSA proofs and one recursion step"]
fn sha256_ecdsa_recursion_timings() {
    use crate::piop::spartan::ecdsa_sha256::*;
    use std::time::Instant;
    let log: u8 = std::env::var("BITZ_REC_ECDSA_LOG").ok().map_or(3, |v| v.parse().unwrap());
    let reps: usize = std::env::var("BITZ_REC_REPS").ok().map_or(3, |v| v.parse().unwrap());
    let selection = std::env::var("BITZ_REC_LIG").unwrap_or_else(|_| "custom:6:4".into());
    let opening_t: Option<usize> = Some(std::env::var("BITZ_REC_T").ok().map_or(19, |v| v.parse().unwrap()));
    let threads = rayon::current_num_threads();
    let (statement, message) = crate::piop::spartan::ecdsa_sha256::tests::fixture_at(log);
    let started = Instant::now();
    let prepared = ecdsa_prepared(log);
    let setup_s = started.elapsed().as_secs_f64();
    let (rows, cols) = (prepared.h_layout.row_vars, prepared.h_layout.col_vars);

    // Before: the base proof.
    let mut base = Vec::new();
    let mut last = None;
    for _ in 0..reps {
        let t0 = Instant::now();
        let witness = generate_sha256_ecdsa_witness(&prepared, &statement, &message).unwrap();
        let t1 = Instant::now();
        let hint = commit_sha256_ecdsa(&prepared, &witness).unwrap();
        let t2 = Instant::now();
        let mut tp = crate::transcript::Sha256Transcript::new();
        let proof = prove_sha256_ecdsa(&mut tp, &prepared, &statement, &witness, &hint, 4).unwrap();
        let t3 = Instant::now();
        let mut tv = crate::transcript::Sha256Transcript::new();
        verify_sha256_ecdsa(&mut tv, &prepared, &statement, &hint.commitment, &proof).expect("native verify");
        let t4 = Instant::now();
        base.push([t1 - t0, t2 - t1, t3 - t2, t3 - t0, t4 - t3].map(|d| d.as_secs_f64()));
        drop(witness);
        last = Some((hint.commitment, proof));
    }
    let (commitment, proof) = last.unwrap();
    let proof_bytes = proof.to_bytes().len();
    let commitment_bytes = bincode::serialize(&commitment).unwrap().len();

    // The honest base proof satisfies its verifier circuit (untimed check).
    let public = PublicStream::discover();
    let mut check = CheckingWitgen::new();
    let counts = super::ecdsa_verifier::verify_sha256_ecdsa(&mut check, &prepared, &statement, &commitment, Some(&proof), public.clone(), None);
    assert_eq!(check.first_failure, None, "the base proof satisfies its verifier circuit");
    assert_eq!(public.borrow().failed, None);
    let stream = public.borrow().clone();
    drop(check);
    let mut by_label = stream.bytes_by_label();
    by_label.sort_by_key(|&(_, _, b)| std::cmp::Reverse(b));

    // After: one recursion step, then its verifier.
    let mut rec = Vec::new();
    let mut ver = Vec::new();
    let mut digests: Vec<[String; 4]> = Vec::new();
    let mut outer = None;
    for _ in 0..reps {
        let t0 = Instant::now();
        // The witness pass and the recording, concurrently; the commitment
        // from the witness pass while the recording finishes.
        let lig = LigeritoSelection::parse(&selection, 100).unwrap();
        let passes = super::ecdsa_verifier::witness_and_record(&prepared, &statement, &commitment, &proof, |g, blocks| {
            let early = crate::piop::spartan::flockrec::FlockRecRelation::early_committed(g, blocks, lig).expect("early committed rows");
            let hint = early.commit();
            (early, hint)
        })
        .expect("the base proof verifies");
        let t1 = Instant::now();
        assert_eq!(passes.stream.values, stream.values, "deterministic public stream");
        let circuit_d = passes.circuit.digest();
        let untimed_circuit = t1.elapsed();
        let (rel, relation_s, _) = outer_relation_from(passes.circuit, &statement, &commitment, &passes.stream, &selection, opening_t);
        let t2 = Instant::now();
        let (g, hh) = (passes.g, passes.h);
        let (early, hint) = passes.early;
        assert!(rel.matches_early(&early), "the relation lays out the committed bits as the witness pass did");
        let wit = rel.witness_with_committed(early.rows, &hh).unwrap();
        let outer_witness_s = t2.elapsed().as_secs_f64();
        let u0 = Instant::now();
        let witness_d = witness_digest(&g, &hh);
        drop((g, hh));
        let untimed = u0.elapsed() + untimed_circuit;
        let t3 = Instant::now();
        let t4 = Instant::now();
        let mut tp = crate::transcript::Sha256Transcript::new();
        let pi = rel.prove(&mut tp, &wit, &hint).expect("FlockRec prove");
        let t5 = Instant::now();
        let relation_d = circuit::linear_map::binary::VirtualMap::digest(rel.map());
        drop((wit, rel));
        rec.push([
            passes.witness_s,
            passes.record_s,
            relation_s,
            outer_witness_s,
            passes.early_s + (t4 - t3).as_secs_f64(),
            (t5 - t4).as_secs_f64(),
            (t5 - t0 - untimed).as_secs_f64(),
            (t1 - t0).as_secs_f64(),
        ]);
        digests.push([relation_d, circuit_d, witness_d, flockrec_proof_digest(&pi)].map(|d| hex(&d)));
        // The final verifier: public stream, statement, base commitment, pi.
        let v0 = Instant::now();
        let (vrel, _, _, stats, _) = ecdsa_outer_relation(&prepared, &statement, &commitment, &stream, &selection, opening_t);
        let mut tv = crate::transcript::Sha256Transcript::new();
        vrel.verify(&mut tv, &hint.commitment, &pi).expect("FlockRec verify");
        ver.push(v0.elapsed().as_secs_f64());
        outer = Some((stats, hint.commitment, pi));
    }
    assert!(digests.windows(2).all(|w| w[0] == w[1]), "a recursion step is deterministic");
    let (st, outer_commitment, pi) = outer.unwrap();
    let pi_bytes = pi.bitz.to_bytes().len() + pi.piop_bytes() + bincode::serialize(&outer_commitment).unwrap().len();
    let col = |rows: &Vec<[f64; 5]>, i: usize| median(rows.iter().map(|r| r[i]).collect());
    let colr = |i: usize| median(rec.iter().map(|r| r[i]).collect());
    let json = serde_json::json!({
        "n": log,
        "threads": threads,
        "reps": reps,
        "claim_grid": [rows, cols],
        "modulus_bits": prepared.modulus_bits(),
        "base_ligerito": std::env::var("BITZ_REC_BASE_LIG").unwrap_or_else(|_| "custom:1:4".into()),
        "outer_ligerito": selection,
        "outer_t": opening_t,
        "setup_s": setup_s,
        "base_witness_s": col(&base, 0),
        "base_commit_s": col(&base, 1),
        "base_protocol_s": col(&base, 2),
        "base_prove_s": col(&base, 3),
        "base_verify_s": col(&base, 4),
        "base_proof_bytes": proof_bytes,
        "base_commitment_bytes": commitment_bytes,
        "rec_witness_s": colr(0),
        "rec_record_s": colr(1),
        "rec_circuit_s": colr(7),
        "rec_relation_s": colr(2),
        "rec_outer_witness_s": colr(3),
        "rec_commit_s": colr(4),
        "rec_protocol_s": colr(5),
        "rec_prove_s": colr(6),
        "final_verify_s": median(ver.clone()),
        "pi_bytes": pi_bytes,
        "public_bytes": stream.bytes(),
        "public_top": by_label.iter().take(6).map(|(l, n, b)| format!("{l}:{n}:{b}")).collect::<Vec<_>>(),
        "circuit": {"committed": st[0], "cells": st[1], "rows": st[2], "blocks": st[3], "outer_n": st[4], "outer_inst_vars": st[5], "outer_f_bits": st[6], "outer_h_bits": st[7]},
        "counts": format!("{counts:?}"),
        "digests": {"relation": digests[0][0], "circuit": digests[0][1], "witness": digests[0][2], "pi": digests[0][3]},
        "base_runs": base,
        "rec_runs": rec,
        "verify_runs": ver,
    });
    println!("TIMING {json}");
}

/// Where the bytes of a FlockRec proof go (for the size levers).
pub(super) fn flockrec_breakdown(pi: &crate::piop::spartan::flockrec::FlockRecProof) -> String {
    use bincode::Options;
    let t = &pi.bitz.transcript;
    let len = u32::from_le_bytes(t.hints[..4].try_into().unwrap()) as usize;
    let lig: flock_core::pcs::ligerito::LigeritoProof =
        crate::bitz::pcs::proof_options().deserialize(&t.hints[4..4 + len]).unwrap();
    let rows = |r: &Vec<Vec<Gf>>| r.iter().map(|x| 16 * x.len()).sum::<usize>();
    let row_bytes = rows(&lig.initial_proof.opened_rows)
        + lig.recursive_proofs.iter().map(|p| rows(&p.opened_rows)).sum::<usize>()
        + rows(&lig.final_proof.opened_rows);
    let merkle = 32
        * (lig.initial_proof.merkle_proof.len()
            + lig.recursive_proofs.iter().map(|p| p.merkle_proof.len()).sum::<usize>()
            + lig.final_proof.merkle_proof.len());
    format!(
        "narg {} B, hints {} B (rows {row_bytes}, merkle {merkle}, yr {}, sumcheck msgs {}, other {}), piop {} B",
        t.narg_string.len(),
        t.hints.len(),
        16 * lig.final_proof.yr.len(),
        32 * lig.sumcheck_transcript.len(),
        t.hints.len() - row_bytes - merkle - 16 * lig.final_proof.yr.len() - 32 * lig.sumcheck_transcript.len(),
        pi.piop_bytes(),
    )
}

#[test]
#[ignore = "heavy: one recursion level"]
fn flockrec_proof_breakdown() {
    let selection = std::env::var("BITZ_REC_LIG").unwrap_or_else(|_| "custom:6:4".into());
    let opening_t: Option<usize> = Some(std::env::var("BITZ_REC_T").ok().map_or(19, |v| v.parse().unwrap()));
    let (_rel1, _com1, pi1, _, _) = level_one(&selection, opening_t);
    println!("BREAKDOWN {selection} T={opening_t:?}: {}", flockrec_breakdown(&pi1));
}
