//! [`crate::piop::spartan::ecdsa_sha256::verify_sha256_ecdsa`] in the
//! circuit (the dense virtual opening): a SHA-256+P-256 proof as the base of
//! the recursion chain.
//!
//! Public, native through the proxy and the relation's hooks: the statement
//! binding, the prime and every challenge, the batched matrix evaluation
//! (the opening's scale) and the dense transposition. In the circuit: Round
//! 0's value, the grinding nonces, the outer (cubic) and inner sumchecks'
//! committed round polynomials, the outer terminal `eq·(a·b − c)` with its
//! one modular product, the opening target and the BitZ opening.

use circuit::Circuit;
use field::Gf128 as Gf;
use flock_core::pcs::commit::Commitment;

use crate::bitz::{LinearClaim, VirtualStatement};
use crate::piop::spartan::SpartanField;
use crate::piop::spartan::ecdsa_sha256::hooks;
use crate::piop::spartan::ecdsa_sha256::{PreparedSha256Ecdsa, Sha256EcdsaProof, Sha256EcdsaStatement};

use super::arith::{Zq, mul_mod_q};
use super::bitz_pcs::{OpeningCounts, verify_virtual, zq_bytes};
use super::bitz_transcript::BitzTranscript;
use super::sha::ShaCounts;
use super::spartan::{ModLc, SpartanTranscript, canonical, modulus, ood_round, sumcheck_verify, witness_residue};
use super::val::Public;

type F = crate::piop::spartan::u32_mul_relation::SpartanBitzField;

#[derive(Clone, Copy, Debug, Default)]
pub struct EcdsaCounts {
    pub spartan: ShaCounts,
    pub opening_transcript: ShaCounts,
    pub opening: OpeningCounts,
}

/// The circuit of `verify_sha256_ecdsa` (dense opening) for one proof
/// (`None` while recording).
pub fn verify_sha256_ecdsa<CS: Circuit>(
    cs: &mut CS,
    prepared: &PreparedSha256Ecdsa,
    statement: &Sha256EcdsaStatement,
    commitment: &Commitment,
    proof: Option<&Sha256EcdsaProof>,
    public: Public,
    log: Option<&mut (Vec<crate::transcript::sha::TraceOp>, Vec<crate::transcript::sha::TraceOp>)>,
) -> EcdsaCounts {
    assert!(prepared.bitz.is_none(), "the circuit verifies the dense opening");
    let (grinding, security) = hooks::grinding(prepared).expect("public security profile");
    let mut t = SpartanTranscript::new(public.clone());
    if log.is_some() {
        t.duplex = t.duplex.clone().with_log();
    }
    hooks::bind_statement(&mut t.proxy(cs), prepared, statement, commitment, &security).expect("public statement");
    let ood = grinding.ood.map(|params| {
        let round = proof.and_then(|p| p.opening.ood);
        let (y, nonce) = round.map_or((Gf::ZERO, Some(0)), |r| (r.y, r.nonce));
        ood_round(cs, &mut t, hooks::packed_vars(prepared), params.grinding_bits, nonce, y)
    });
    if grinding.initial > 0 {
        t.grinding_boundary(cs, hooks::INITIAL_DOMAIN, 0, grinding.initial, proof.map_or(0, |p| p.initial_nonce));
    }
    let bits = prepared.modulus_bits();
    let q = crate::prime_sampling::sample_prime_in_interval(&mut t.proxy(cs), 1u128 << (bits - 1), (1u128 << bits) - 1)
        .expect("public prime");
    crate::piop::spartan::absorb_spartan_message(&mut t.proxy(cs), b"q", &q.to_le_bytes());
    let cfg = F::make_cfg(&field::Uint::from(q)).expect("sampled prime");
    let cfg = &cfg;
    let tau: Vec<F> = (0..hooks::outer_vars(prepared)).map(|_| F::from_with_cfg(t.squeeze_field(cs, cfg), cfg)).collect();
    let tau_c: Vec<u128> = tau.iter().map(|x| canonical(x, cfg)).collect();
    assert_eq!(modulus(cfg), q);

    // Outer sumcheck (cubic), its terminal identity, the A/B/C evaluations.
    let outer_rounds: Vec<[u128; 4]> = match proof {
        Some(p) => p.outer.sumcheck.round_polynomials.iter().map(|r| std::array::from_fn(|i| canonical(&r[i], cfg))).collect(),
        None => vec![[0; 4]; tau.len()],
    };
    let outer_nonces: Vec<u64> = match proof {
        Some(p) => p.outer_nonces.clone(),
        None => vec![0; if grinding.outer > 0 { tau.len() } else { 0 }],
    };
    let (outer_point, outer_final) = sumcheck_verify(
        cs,
        &mut t,
        &outer_rounds,
        ModLc::constant(0),
        cfg,
        Some((hooks::OUTER_DOMAIN, grinding.outer, &outer_nonces)),
    );
    let mut eq = 1u128;
    for (&t_i, &r_i) in tau_c.iter().zip(&outer_point) {
        // eq(t, r) = t·r + (1 − t)(1 − r).
        let tr = super::arith::mulmod(t_i, r_i, q);
        let one_t = (1 + q - t_i) % q;
        let one_r = (1 + q - r_i) % q;
        eq = super::arith::mulmod(eq, (tr + super::arith::mulmod(one_t, one_r, q)) % q, q);
    }
    let evals: [u128; 3] = proof.map_or([0; 3], |p| {
        [canonical(&p.outer.az_mle_claim, cfg), canonical(&p.outer.bz_mle_claim, cfg), canonical(&p.outer.cz_mle_claim, cfg)]
    });
    let [ax, bx, cx] = evals.map(|v| witness_residue(cs, v, q));
    let ab = mul_mod_q(cs, &ax, &bx, q, &public);
    // final ≡ eq·(a·b − c).
    let mut check = outer_final.clone();
    check.add((q - eq) % q, &ab, q);
    check.add(eq, &cx, q);
    check.assert_zero(cs, q, &public, "outer terminal identity");
    t.absorb_field_elements(cs, &[ax.clone(), bx.clone(), cx.clone()]);
    if grinding.batch > 0 {
        t.grinding_boundary(cs, hooks::BATCH_DOMAIN, 0, grinding.batch, proof.map_or(0, |p| p.batch_nonce));
    }
    let (rho, linear_point, linear_weight) = hooks::inner_batch(&mut t.proxy(cs), prepared, cfg).expect("public batching");
    let outer_point_f: Vec<F> = outer_point.iter().map(|&c| F::from_with_cfg(c, cfg)).collect();
    let rho_c = canonical(&rho, cfg);
    let (inner_claim, public_sum) =
        hooks::inner_claim(prepared, statement, outer_point_f, rho, linear_point, linear_weight, cfg).expect("public inner claim");
    // claimed = a + ρ b + ρ² c + (public part).
    let mut claimed = ModLc::constant(canonical(&public_sum, cfg));
    claimed.add(1, &ax, q);
    claimed.add(rho_c, &bx, q);
    claimed.add(super::arith::mulmod(rho_c, rho_c, q), &cx, q);

    // Inner sumcheck (quadratic).
    let n_inner = hooks::inner_vars(prepared);
    let inner_rounds: Vec<[u128; 3]> = match proof {
        Some(p) => p.inner.round_polynomials.iter().map(|r| std::array::from_fn(|i| canonical(&r[i], cfg))).collect(),
        None => vec![[0; 3]; n_inner],
    };
    let inner_nonces: Vec<u64> = match proof {
        Some(p) => p.inner_nonces.clone(),
        None => vec![0; if grinding.inner > 0 { n_inner } else { 0 }],
    };
    let (inner_point, inner_final) = sumcheck_verify(
        cs,
        &mut t,
        &inner_rounds,
        claimed,
        cfg,
        Some((crate::sumcheck::inner::packed::Sha256InnerGrinding::DOMAIN_BYTES, grinding.inner, &inner_nonces)),
    );
    let inner_point_f: Vec<F> = inner_point.iter().map(|&c| F::from_with_cfg(c, cfg)).collect();
    let scale = hooks::matrix_scale(prepared, &inner_claim, &inner_point_f, cfg).expect("public matrix evaluation");
    let target = inner_final.settle(cs, q, &public);
    // bind_opening: the public frame, then the committed value.
    hooks::opening_frame(&mut t.proxy(cs), &inner_point_f, &scale, cfg);
    t.absorb_message(cs, b"field", &zq_bytes(&target));

    let (params, committed, pcs, rows, cols) =
        hooks::dense_opening(prepared, q, &security, &inner_point_f, &scale, cfg).expect("public dense opening");
    let native_claim = LinearClaim::new(&params, rows, cols, 0).expect("public claim weights");
    let statement_v = VirtualStatement::new(params, committed, &prepared.map, &native_claim).expect("public statement");
    let tag = crate::piop::spartan::protocol::bitz_opener::fork_tag(&mut t.proxy(cs));
    let spartan = t.duplex.counts;
    let mut bt = BitzTranscript::new(cs, public, hooks::DENSE_SESSION, &tag, proof.map(|p| &p.opening.transcript), log.is_some());
    let ood_claim = ood.as_ref().map(|(p, y)| (p.as_slice(), y.clone()));
    let opening = verify_virtual(cs, &statement_v, &pcs, commitment.root, &target, &mut bt, ood_claim);
    if let Some(log) = log {
        log.0 = t.duplex.log.clone().unwrap_or_default();
        log.1 = bt.duplex.log.clone().unwrap_or_default();
    }
    let _ = Zq::<CS::Bool>::P(0);
    EcdsaCounts { spartan, opening_transcript: bt.counts(), opening }
}

/// One proof's verifier circuit as the recursion prover needs it.
pub struct CircuitPasses<X> {
    /// The committed bits `w` and the cells `M·w` (entry 0 the constant).
    pub g: circuit::witgen::PackedWitness,
    pub h: circuit::witgen::PackedWitness,
    pub circuit: circuit::compact::StructuredCircuit,
    pub stream: super::val::PublicStream,
    /// What `after_witness` returned.
    pub early: X,
    /// Each pass's own wall time (they overlap), and `after_witness`'s.
    pub witness_s: f64,
    pub record_s: f64,
    pub early_s: f64,
}

/// The witness pass ([`circuit::witgen::Witgen`]) and the structured
/// recording of [`verify_sha256_ecdsa`] for `proof`, run concurrently: the
/// recorder, on its own thread, follows the public values as the witness
/// pass discovers them (the recording is a function of those alone).
/// `after_witness` runs on this thread once the witness is complete, while
/// the recording continues (the prover commits there). With one worker
/// thread (or `BITZ_REC_SERIAL=1`) the passes run one after the other on
/// this thread, the recording replaying the discovered stream.
pub fn witness_and_record<X>(
    prepared: &PreparedSha256Ecdsa,
    statement: &Sha256EcdsaStatement,
    commitment: &Commitment,
    proof: &Sha256EcdsaProof,
    after_witness: impl FnOnce(&circuit::witgen::PackedWitness, &[circuit::witgen::BlockSpan]) -> X,
) -> Result<CircuitPasses<X>, String> {
    struct Ecdsa<'a> {
        prepared: &'a PreparedSha256Ecdsa,
        statement: &'a Sha256EcdsaStatement,
        commitment: &'a Commitment,
        proof: &'a Sha256EcdsaProof,
    }
    impl VerifierCircuit for Ecdsa<'_> {
        fn run<CS: Circuit>(&self, cs: &mut CS, with_proof: bool, public: Public) {
            let proof = with_proof.then_some(self.proof);
            verify_sha256_ecdsa(cs, self.prepared, self.statement, self.commitment, proof, public, None);
        }
    }
    witness_and_record_circuit(&Ecdsa { prepared, statement, commitment, proof }, after_witness)
}

/// A base proof's verifier circuit as [`witness_and_record_circuit`] runs
/// it: once with the proof (the witness pass), once without (the recording,
/// a function of the public stream alone).
pub trait VerifierCircuit: Sync {
    fn run<CS: Circuit>(&self, cs: &mut CS, with_proof: bool, public: Public);
}

/// [`witness_and_record`] for any [`VerifierCircuit`].
pub fn witness_and_record_circuit<V: VerifierCircuit, X>(
    circuit_v: &V,
    after_witness: impl FnOnce(&circuit::witgen::PackedWitness, &[circuit::witgen::BlockSpan]) -> X,
) -> Result<CircuitPasses<X>, String> {
    use super::val::{Feed, PublicStream};
    use std::time::Instant;
    /// Ends the feed however the witness pass ends, so a follower never
    /// waits forever.
    struct Closing(std::sync::Arc<Feed>);
    impl Drop for Closing {
        fn drop(&mut self) {
            self.0.close();
        }
    }
    #[cfg(feature = "parallel")]
    let threads = rayon::current_num_threads();
    #[cfg(not(feature = "parallel"))]
    let threads = 1;
    if threads <= 1 || std::env::var_os("BITZ_REC_SERIAL").is_some() {
        let started = Instant::now();
        let public = PublicStream::discover();
        let mut w = circuit::witgen::Witgen::new();
        circuit_v.run(&mut w, true, public.clone());
        let witness_s = started.elapsed().as_secs_f64();
        let stream = public.borrow().clone();
        if let Some(why) = &stream.failed {
            return Err(why.clone());
        }
        let blocks = w.blocks().to_vec();
        let (g, h) = w.into_witnesses();
        let started = Instant::now();
        let early = after_witness(&g, &blocks);
        let early_s = started.elapsed().as_secs_f64();
        let started = Instant::now();
        let mut rec = circuit::compact::CompactRecorder::new();
        let replay = PublicStream::replay(stream.values.clone(), stream.widths.clone());
        circuit_v.run(&mut rec, false, replay);
        let circuit = rec.finish_structured();
        let record_s = started.elapsed().as_secs_f64();
        return Ok(CircuitPasses { g, h, circuit, stream, early, witness_s, record_s, early_s });
    }
    let feed = Feed::new();
    std::thread::scope(|scope| {
        let follower = feed.clone();
        let recorder = std::thread::Builder::new()
            .name("bitz-recursion-record".into())
            .stack_size(64 << 20)
            .spawn_scoped(scope, move || {
                let started = Instant::now();
                let mut rec = circuit::compact::CompactRecorder::new();
                circuit_v.run(&mut rec, false, PublicStream::follow(follower));
                (rec.finish_structured(), started.elapsed().as_secs_f64())
            })
            .map_err(|e| e.to_string())?;
        let started = Instant::now();
        let closing = Closing(feed.clone());
        let public = PublicStream::discover_into(feed.clone());
        let mut w = circuit::witgen::Witgen::new();
        circuit_v.run(&mut w, true, public.clone());
        let witness_s = started.elapsed().as_secs_f64();
        drop(closing);
        let stream = public.borrow().clone();
        let blocks = w.blocks().to_vec();
        let (g, h) = w.into_witnesses();
        let started = Instant::now();
        let early = stream.failed.is_none().then(|| after_witness(&g, &blocks));
        let early_s = started.elapsed().as_secs_f64();
        // A false statement ends the feed early, and the follower with it.
        let recorded = recorder.join();
        if let Some(why) = &stream.failed {
            return Err(why.clone());
        }
        let (circuit, record_s) = recorded.map_err(|_| "the recording pass panicked".to_string())?;
        let early = early.expect("computed for a true statement");
        Ok(CircuitPasses { g, h, circuit, stream, early, witness_s, record_s, early_s })
    })
}

