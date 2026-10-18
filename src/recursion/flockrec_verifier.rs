//! [`FlockRecRelation::verify`] in the circuit: the outer relation's own
//! verifier, so a proof that a verifier circuit is satisfied can itself be
//! verified by the next circuit — the recursion step.
//!
//! The Spartan level runs over [`super::spartan::SpartanTranscript`]: the
//! statement frames, the configuration binding, the prime, the batching
//! challenges and the matrix evaluation are public (native code through the
//! proxy); the circuit keeps Round 0's value, the grinding nonces, the inner
//! sumcheck's committed round polynomials, the settled opening target and
//! the virtual BitZ opening.

use circuit::Circuit;
use field::Gf128 as Gf;
use flock_core::pcs::commit::Commitment;

use crate::bitz::{LinearClaim, VirtualStatement};
use crate::piop::spartan::flockrec::{
    DOMAIN_INITIAL, DOMAIN_PRIME, DOMAIN_TERMINAL, FlockRecProof, FlockRecRelation, OPENING_TARGET_TAG,
    challenge_point, claim_digest, opening_claim,
};
use crate::piop::spartan::protocol::bitz_opener::{
    VIRTUAL_SESSION, fork_tag, opening_params, opening_shape, pcs_from_config,
};
use crate::piop::spartan::{SpartanField, absorb_spartan_message};

use super::bitz_pcs::{OpeningCounts, verify_virtual, zq_bytes};
use super::bitz_transcript::BitzTranscript;
use super::sha::ShaCounts;
use super::spartan::{ModLc, SpartanTranscript, canonical, modulus, ood_round, sumcheck_verify};
use super::val::Public;

type F = crate::piop::spartan::u32_mul_relation::SpartanBitzField;

/// In-circuit costs of one FlockRec verification.
#[derive(Clone, Copy, Debug, Default)]
pub struct FlockRecCounts {
    pub spartan: ShaCounts,
    pub spartan_pow: ShaCounts,
    pub opening_transcript: ShaCounts,
    pub opening: OpeningCounts,
}

/// The circuit of [`FlockRecRelation::verify`] for one proof (`None` while
/// recording: the shape needs only the relation and the public stream).
pub fn verify_flockrec<CS: Circuit>(
    cs: &mut CS,
    rel: &FlockRecRelation,
    commitment: &Commitment,
    proof: Option<&FlockRecProof>,
    public: Public,
    log: Option<&mut (Vec<crate::transcript::sha::TraceOp>, Vec<crate::transcript::sha::TraceOp>)>,
) -> FlockRecCounts {
    let mut t = SpartanTranscript::new(public.clone());
    if log.is_some() {
        t.duplex = t.duplex.clone().with_log();
    }
    for (tag, bytes) in rel.statement_frames(commitment).expect("public statement frames") {
        absorb_spartan_message(&mut t.proxy(cs), tag, &bytes);
    }
    rel.ligerito.bind(&mut t.proxy(cs));
    let packed_vars = rel.f_layout.row_vars + rel.f_layout.col_vars - crate::ligerito::LOG_PACKING;
    let security = &rel.security;
    let ood = security.ood.map(|params| {
        let round = proof.and_then(|p| p.bitz.ood);
        let (y, nonce) = round.map_or((Gf::ZERO, Some(0)), |r| (r.y, r.nonce));
        ood_round(cs, &mut t, packed_vars, params.grinding_bits, nonce, y)
    });
    if security.initial_grinding_bits > 0 {
        let nonce = proof.map_or(0, |p| p.initial_nonce);
        t.grinding_boundary(cs, DOMAIN_INITIAL, 0, security.initial_grinding_bits, nonce);
    }
    let prime = crate::piop::spartan::protocol::sample_mod_q(
        &mut t.proxy(cs),
        DOMAIN_PRIME,
        security.projection_min,
        security.projection_max,
    )
    .expect("public prime");
    let cfg = &prime;
    let q = modulus(cfg);
    absorb_spartan_message(&mut t.proxy(cs), b"runtime-field-modulus", &F::canonical_modulus_encoding(cfg));
    let xi = challenge_point(&mut t.proxy(cs), b"flockrec/local-rows", rel.rt, cfg).expect("public challenge");
    let eta = challenge_point(&mut t.proxy(cs), b"flockrec/instances", rel.inst_vars, cfg).expect("public challenge");
    if security.terminal_grinding_bits > 0 {
        let nonce = proof.map_or(0, |p| p.terminal_nonce);
        t.grinding_boundary(cs, DOMAIN_TERMINAL, 0, security.terminal_grinding_bits, nonce);
    }
    let omega = challenge_point(&mut t.proxy(cs), b"flockrec/constant-one", 1, cfg).expect("public challenge")[0];
    let batching = rel.batching(&xi, &eta, omega, cfg).expect("public batching");

    let n_vars = rel.h_layout.row_vars + rel.h_layout.col_vars;
    let rounds: Vec<[u128; 3]> = match proof {
        Some(p) => p
            .inner
            .round_polynomials
            .iter()
            .map(|r| std::array::from_fn(|i| canonical(&r[i], cfg)))
            .collect(),
        None => vec![[0; 3]; n_vars],
    };
    assert_eq!(rounds.len(), n_vars);
    let bits = security.piop_round_grinding_bits;
    let nonces: Vec<u64> = match proof {
        Some(p) => p.inner_nonces.clone(),
        None => vec![0; if bits > 0 { n_vars } else { 0 }],
    };
    let spartan_before = t.duplex.counts;
    let (point, final_lc) = sumcheck_verify(
        cs,
        &mut t,
        &rounds,
        ModLc::constant(canonical(&omega, cfg)),
        cfg,
        Some((crate::sumcheck::inner::packed::Sha256InnerGrinding::DOMAIN_BYTES, bits, &nonces)),
    );
    let _ = spartan_before;
    let point_f: Vec<F> = point.iter().map(|&c| F::from_with_cfg(c, cfg)).collect();
    let v_at = rel.evaluate(&batching, &point_f, cfg).expect("public matrix evaluation");
    let target = final_lc.settle(cs, q, &public);
    let zero = F::zero_with_cfg(cfg);
    let claim = opening_claim(rel, &point_f, &v_at, &zero, cfg).expect("public opening weights");
    // bind_opening_claim: the public digest, then the committed target.
    let digest = claim_digest(rel, &xi, &eta, &omega, &point_f, &v_at, &claim, cfg);
    absorb_spartan_message(&mut t.proxy(cs), b"flockrec/opening-claim", &digest);
    t.absorb_message(cs, OPENING_TARGET_TAG, &zq_bytes(&target));

    let params = opening_params(&rel.h_layout, q).expect("public opening parameters");
    let committed = opening_shape(&rel.f_layout).expect("public committed shape");
    let native_claim = LinearClaim::new(&params, claim.0, claim.1, 0).expect("public opening claim");
    let statement = VirtualStatement::new(params, committed, &rel.map, &native_claim).expect("public statement");
    let pcs = pcs_from_config(&committed, rel.ligerito.verifier())
        .expect("public opener")
        .with_native_policy(security.native_policy().expect("public native policy"));
    let tag = fork_tag(&mut t.proxy(cs));
    let spartan = t.duplex.counts;
    let mut bt = BitzTranscript::new(cs, public, VIRTUAL_SESSION, &tag, proof.map(|p| &p.bitz.transcript), log.is_some());
    let ood_claim = ood.as_ref().map(|(p, y)| (p.as_slice(), y.clone()));
    let opening = verify_virtual(cs, &statement, &pcs, commitment.root, &target, &mut bt, ood_claim);
    if let Some(log) = log {
        log.0 = t.duplex.log.clone().unwrap_or_default();
        log.1 = bt.duplex.log.clone().unwrap_or_default();
    }
    FlockRecCounts {
        spartan,
        spartan_pow: t.pow,
        opening_transcript: bt.counts(),
        opening,
    }
}
