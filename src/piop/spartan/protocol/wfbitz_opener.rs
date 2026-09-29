//! Wfbitz openings for the shared relation protocol.
//!
//! The Spartan prefix binds the statement, prime, PIOP, and terminal claim.
//! The opening forks that transcript and proves the bitified row/column
//! functional through `crate::wfbitz`. Direct and virtual relations share
//! checked PCS construction and the concrete opening proof below.
//!
//! Johnson ladders bind Round 0 before the prime draw; its claim is batched
//! into the final binary opening. Native challenges use the profile's explicit
//! grinding schedule, while Flock retains its own challenge stream and work.

use flock_core::merkle::HashKind;
use flock_core::pcs::ligerito::{LigeritoProfile, LigeritoSecurityConfig};

use crate::{
    ligerito_flock::{
        FlockCommitHint, LigeritoSelection, OodRound, ResolvedLigerito, ood_round_bits,
    },
    pcs::IntegerMatrixLayout,
    piop::spartan::profile::IopSecurityProfile,
    transcript::traits::Transcript,
    wfbitz::{
        BitZParams, BitZProver, BitZVerifier, LinearClaim, Root, Shape, WINDOW,
        pcs::Pcs as BitzPcs,
        transcript::{Proof as BitzTranscriptProof, build_prover, build_verifier},
    },
};
use flock_core::pcs::commit::Commitment;

use super::{
    ClaimFrame, Opener, PreparedRelation, PreparedRelationPrefix, Proof, ProtocolError,
    ReductionProof, RelationSpec, bind_claim_frame, bind_prover_statement, bind_verifier_statement,
    bitify, bitz_generator, check_proof_kernel, grind_and_absorb_in_domain, instantiate_profile,
    packed_variables, prove_piop, sample_mod_q, step50_accepts_lift, step50_integer_lift,
    step50_reduce, validate_bit_rows, validate_commitment, verify_and_absorb_in_domain,
    verify_piop,
};

/// The session tag of the forked BitZ transcript.
const SESSION: &[u8] = b"bitz/wfbitz-opener/v1";

/// Which Ligerito ladder the BitZ opener commits and opens under.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WfbitzLigerito {
    /// BitZ as shipped: flock's embedded `fast` ladder for the size (a
    /// 100-bit Johnson ladder, rate 1/2, `k = 4`, query and fold grinding).
    Fast,
    /// One of the crate's resolved selections at the profile's target
    /// (`udr:<r>:<k>` validated unique decoding, `udrg:<r>:<k>` the same
    /// with fold grinding; a Johnson `custom:<r>:<k>` ladder, which
    /// [`WfbitzOpener::prepare`] pairs with the crate's Round 0).
    Selected(LigeritoSelection),
}

impl WfbitzLigerito {
    /// `fast`, or any selection [`LigeritoSelection::parse`] accepts.
    pub fn parse(request: &str, target: usize) -> Result<Self, String> {
        match request.trim() {
            "fast" | "bitz" => Ok(Self::Fast),
            other => LigeritoSelection::parse(other, target).map(Self::Selected),
        }
    }

    pub fn name(&self) -> String {
        match self {
            Self::Fast => "fast".to_string(),
            Self::Selected(selection) => selection.name(),
        }
    }
}

/// The BitZ opener of one relation: the shape, the parameters gate and the
/// PCS under the selected ladder.
#[derive(Clone, Debug)]
pub struct WfbitzOpener {
    layout: IntegerMatrixLayout,
    shape: Shape,
    pcs: BitzPcs,
    ligerito: WfbitzLigerito,
    security: LigeritoSecurityConfig,
    /// The ladder's identity, bound with the statement.
    digest: [u8; 32],
    /// A resolved selection as the crate's own opener carries it (`None`
    /// for BitZ's `fast` ladder): its policy digest is bound after the
    /// statement, before Round 0.
    resolved: Option<ResolvedLigerito>,
    /// `log₂` of the packed message (`m − 7`): the Round-0 point's arity.
    packed_vars: usize,
}

impl WfbitzOpener {
    /// For a relation's committed layout (word width one only: BitZ commits
    /// bits) and a ladder at `target_bits`.
    pub fn new(
        layout: IntegerMatrixLayout,
        ligerito: WfbitzLigerito,
        target_bits: usize,
    ) -> Result<Self, ProtocolError> {
        if layout.word_bits != 1 {
            return Err(ProtocolError::LigeritoConfig(
                "the BitZ opener commits bits (word width one)".into(),
            ));
        }
        let shape = Shape::new(layout.row_vars, layout.col_vars)
            .map_err(|error| ProtocolError::LigeritoConfig(format!("BitZ shape: {error:?}")))?;
        let (pcs, security, digest, resolved) = match ligerito {
            WfbitzLigerito::Fast => {
                let pcs = BitzPcs::new(&shape, HashKind::Blake3).map_err(|error| {
                    ProtocolError::LigeritoConfig(format!("BitZ pcs: {error:?}"))
                })?;
                let source = flock_core::pcs::ligerito::embedded_security_config(
                    shape.log_bits(),
                    LigeritoProfile::Fast,
                )
                .ok_or_else(|| ProtocolError::LigeritoConfig("no embedded fast ladder".into()))?;
                let mut security = LigeritoSecurityConfig::from_toml_str(source)
                    .map_err(ProtocolError::LigeritoConfig)?;
                security.hash = "blake3".to_owned();
                let digest = *blake3::hash(source.as_bytes()).as_bytes();
                (pcs, security, digest, None)
            }
            WfbitzLigerito::Selected(selection) => {
                let resolved = selection
                    .resolve(packed_variables(&layout)?, target_bits)
                    .map_err(ProtocolError::LigeritoConfig)?;
                let pcs =
                    BitzPcs::with_security(&shape, resolved.security(), LigeritoProfile::Fast)
                        .map_err(|error| {
                            ProtocolError::LigeritoConfig(format!("BitZ pcs: {error:?}"))
                        })?;
                let (security, digest) = (resolved.security().clone(), resolved.digest());
                (pcs, security, digest, Some(resolved))
            }
        };
        let policy = crate::wfbitz::grinding::Policy::new(
            Some(u32::try_from(target_bits).map_err(|_| ProtocolError::UnsupportedProfile)?),
            0,
            0,
        )
        .map_err(|_| ProtocolError::UnsupportedProfile)?;
        Ok(Self {
            layout,
            shape,
            pcs: pcs.with_native_policy(policy),
            ligerito,
            security,
            digest,
            resolved,
            packed_vars: packed_variables(&layout)?,
        })
    }

    /// The prefix and the opener of a relation together: the profile's
    /// security parameters adopt the ladder's Round-0 accounting (the
    /// out-of-domain sample and its grinding for a Johnson ladder; nothing
    /// in unique decoding), so [`prove`] runs Round 0 exactly as the
    /// crate's own opener does ([`super::PreparedRelation::with_ligerito`]),
    /// and drop the forest and ring-switch grinding the scheme does not run
    /// (a profile those bare terms cannot reach, `λ ≥ 127`, is refused;
    /// design-only schedules excepted, as at instantiation).
    pub fn prepare<P: IopSecurityProfile, S: RelationSpec>(
        spec: S,
        ligerito: WfbitzLigerito,
        target_bits: usize,
    ) -> Result<(PreparedRelationPrefix<S>, Self), ProtocolError> {
        spec.validate_geometry()?;
        let mut opener = Self::new(spec.committed_layout(), ligerito, target_bits)?;
        let mut security = instantiate_profile::<P, S>(&spec)?;
        security.adopt_ood_round(opener.ood_bits())?;
        security.adopt_native_opening(super::relation_native_geometry(&spec))?;
        opener.pcs = opener.pcs.with_native_policy(security.native_policy()?);
        let prefix = PreparedRelationPrefix::with_security(spec, security)?;
        Ok((prefix, opener))
    }

    /// The theorem's Round-0 collision bound at the ladder's level 0
    /// ([`ood_round_bits`]); `None` in unique decoding (no round).
    pub fn ood_bits(&self) -> Option<f64> {
        ood_round_bits(&self.security, self.packed_vars)
    }

    pub fn layout(&self) -> &IntegerMatrixLayout {
        &self.layout
    }

    pub fn shape(&self) -> &Shape {
        &self.shape
    }

    pub fn ligerito(&self) -> WfbitzLigerito {
        self.ligerito
    }

    /// The ladder the opener runs.
    pub fn security(&self) -> &LigeritoSecurityConfig {
        &self.security
    }

    pub fn pcs(&self) -> &BitzPcs {
        &self.pcs
    }

    /// The opener configuration the statement binding covers: a resolved
    /// selection as [`super::PreparedRelation::with_ligerito`] carries it
    /// (its policy digest bound after the statement, before Round 0; its
    /// configurations are the ones the scheme runs), and BitZ's `fast`
    /// ladder as its prover and verifier configurations (no policy digest
    /// of the crate's kind).
    pub fn opener(&self) -> Opener {
        match &self.resolved {
            Some(resolved) => Opener::Resolved(resolved.clone()),
            None => Opener::Custom {
                prover: Some(self.pcs.prover_config().clone()),
                verifier: Some(self.pcs.verifier_config().clone()),
            },
        }
    }

    /// Commits the per-column bit rows (the layout every relation's
    /// `f2z_bit_rows` produces) under the ladder's level 0.
    pub fn commit(&self, rows: Vec<Vec<u64>>) -> Result<FlockCommitHint, ProtocolError> {
        validate_bit_rows(&self.layout, &rows)?;
        let (_, hint) = self
            .pcs
            .commit(&self.shape, rows)
            .map_err(|error| ProtocolError::LigeritoConfig(format!("BitZ commit: {error:?}")))?;
        validate_commitment(&self.layout, &hint.commitment, self.pcs.prover_config())?;
        Ok(hint)
    }

    /// The parameters under the runtime prime (their gate: the fold bound).
    fn params(&self, q: u128) -> Result<BitZParams, ProtocolError> {
        BitZParams::new(self.shape, q, bitz_generator())
            .map_err(|error| ProtocolError::LigeritoConfig(format!("BitZ parameters: {error:?}")))
    }

    /// Round 0 as it runs under `ood` (the executed parameters): the
    /// collision bound plus that grinding, the profile's `step0:ood-draw`
    /// term; `None` when no round runs.
    fn round_0_bits(&self, ood: Option<crate::ligerito_flock::OodRoundParams>) -> Option<f64> {
        Some(self.ood_bits()? + f64::from(ood?.grinding_bits))
    }

    /// The round-by-round accounting of the opening: the ladder's weakest
    /// level (its paper-predicted proximity-gap and query terms with their
    /// grinding), Round 0 under the parameters the proof runs (`ood`: a
    /// relation's `prefix.security().ood`, which [`Self::prepare`] adopts at
    /// the profile's λ) and the `GF(2^128)` floor of every other draw.
    pub fn opening_bits(
        &self,
        ood: Option<crate::ligerito_flock::OodRoundParams>,
    ) -> (f64, &'static str) {
        // Round 0 runs exactly for a Johnson ladder; any other pairing is
        // inconsistent (as a Ligerito report is, `validate_report`), and
        // nothing is credited.
        if self.ood_bits().is_some() != ood.is_some() {
            return (0.0, "round 0 inconsistent with the ladder");
        }
        let mut weakest = (f64::INFINITY, "ligerito");
        for level in &self.security.levels {
            let (pg, query) = level.paper_predicted_bits();
            // The level's weakest fold round (a unique-decoding level's last
            // round keeps `k − 1` bits less than `fold_grinding_bits`).
            let pg = pg + crate::ligerito_flock::weakest_fold_round_grinding(level) as f64;
            let query = query + level.grinding_bits as f64;
            if pg < weakest.0 {
                weakest = (pg, "ligerito proximity gap");
            }
            if query < weakest.0 {
                weakest = (query, "ligerito queries");
            }
        }
        // Round 0 (a Johnson ladder): the collision bound plus its grinding.
        if let Some(bits) = self.round_0_bits(ood) {
            if bits < weakest.0 {
                weakest = (bits, "round 0 (ood collision)");
            }
        }
        // Every other draw is a GF(2^128) challenge: the crate's floor.
        let floor = 126.4;
        if floor < weakest.0 {
            weakest = (floor, "GF(2^128) floor");
        }
        weakest
    }

    /// The ladder's identity.
    pub fn digest(&self) -> [u8; 32] {
        self.digest
    }
}

/// The BitZ opening: the forked transcript's narg string and hints, and
/// the crate's Round-0 messages when the ladder runs the round.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WfbitzOpeningProof {
    pub narg: Vec<u8>,
    pub hints: Vec<u8>,
    pub ood: Option<OodRound>,
}

impl WfbitzOpeningProof {
    /// Both streams, each with a `u64` length, then the Round-0 messages:
    /// a tag byte (0 = no round, 1 = round), `y` (16 bytes) and the
    /// grinding nonce (a presence byte, then 8 bytes).
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(16 + self.narg.len() + self.hints.len() + 26);
        for stream in [&self.narg, &self.hints] {
            bytes.extend_from_slice(&(stream.len() as u64).to_le_bytes());
            bytes.extend_from_slice(stream);
        }
        match &self.ood {
            None => bytes.push(0),
            Some(round) => {
                bytes.push(1);
                bytes.extend_from_slice(&round.y.to_bytes());
                match round.nonce {
                    None => bytes.push(0),
                    Some(nonce) => {
                        bytes.push(1);
                        bytes.extend_from_slice(&nonce.to_le_bytes());
                    }
                }
            }
        }
        bytes
    }

    pub fn ood(&self) -> Option<&OodRound> {
        self.ood.as_ref()
    }
}

impl WfbitzOpeningProof {
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        let mut at = 0usize;
        let mut read = |len: Option<usize>| -> Option<Vec<u8>> {
            let len = match len {
                Some(len) => len,
                None => {
                    let end = at.checked_add(8)?;
                    let word = bytes.get(at..end)?.try_into().ok()?;
                    let len = usize::try_from(u64::from_le_bytes(word)).ok()?;
                    at = end;
                    len
                }
            };
            // A declared length past the end is a truncation, not an overflow.
            let end = at.checked_add(len)?;
            let stream = bytes.get(at..end)?.to_vec();
            at = end;
            Some(stream)
        };
        let narg = read(None)?;
        let hints = read(None)?;
        let ood = match read(Some(1))?[0] {
            0 => None,
            1 => {
                let y = field::Gf128::from_bytes(read(Some(16))?.try_into().ok()?);
                let nonce = match read(Some(1))?[0] {
                    0 => None,
                    1 => Some(u64::from_le_bytes(read(Some(8))?.try_into().ok()?)),
                    _ => return None,
                };
                Some(OodRound { y, nonce })
            }
            _ => return None,
        };
        (at == bytes.len()).then_some(Self { narg, hints, ood })
    }
}

/// The fork's instance tag: 32 bytes squeezed from the outer transcript
/// after the terminal boundary.
pub(crate) fn fork_tag<T: Transcript>(transcript: &mut T) -> [u8; 32] {
    let low: u128 = transcript.get_challenge();
    let high: u128 = transcript.get_challenge();
    let mut tag = [0u8; 32];
    tag[..16].copy_from_slice(&low.to_le_bytes());
    tag[16..].copy_from_slice(&high.to_le_bytes());
    tag
}

/// The modulus of the runtime prime context as a word.
pub(crate) fn modulus_u128(prime: &field::FpCtx<2>) -> u128 {
    let words = prime.modulus().as_words();
    u128::from(words[0]) | (u128::from(words[1]) << 64)
}

/// The bitified functional as BitZ's claim: dense row weights, the column
/// table, the adjusted target.
fn linear_claim(
    params: &BitZParams,
    opening: &bitify::BitifiedClaim,
    table: &bitify::BlockTable,
    arith: &field::FpCtx<2>,
) -> Result<LinearClaim, ProtocolError> {
    let rows = bitify::dense_row_weights(opening, table, arith)?;
    let columns = bitify::column_weights(opening, arith)?;
    LinearClaim::new(params, rows, columns, opening.claimed)
        .map_err(|error| ProtocolError::LigeritoConfig(format!("BitZ claim: {error:?}")))
}

fn check_native_policy<S: RelationSpec>(
    prefix: &PreparedRelationPrefix<S>,
    opener: &WfbitzOpener,
) -> Result<(), ProtocolError> {
    if opener.pcs.native_policy() != prefix.security.native_policy()? {
        return Err(ProtocolError::UnsupportedProfile);
    }
    Ok(())
}

/// Proves the relation, the bitified claim discharged through BitZ.
pub fn prove<T: Transcript + Send, S: RelationSpec>(
    transcript: &mut T,
    prefix: &PreparedRelationPrefix<S>,
    opener: &WfbitzOpener,
    witness: &S::Witness,
    hint: &FlockCommitHint,
) -> Result<Proof, ProtocolError> {
    if prefix.params() != *opener.layout() {
        return Err(ProtocolError::RelationWitnessLayoutMismatch);
    }
    check_native_policy(prefix, opener)?;
    prove_direct(
        transcript,
        prefix,
        &opener.opener(),
        opener.pcs(),
        &opener.digest(),
        witness,
        hint,
    )
}

pub(super) fn prove_direct<T: Transcript + Send, S: RelationSpec>(
    transcript: &mut T,
    prefix: &PreparedRelationPrefix<S>,
    configuration: &Opener,
    pcs: &BitzPcs,
    digest: &[u8; 32],
    witness: &S::Witness,
    hint: &FlockCommitHint,
) -> Result<Proof, ProtocolError> {
    let spec = prefix.layout();
    // The direct discharge only: the claim of a relation with a virtual map
    // is about the derived grid, not the committed one.
    if spec.map().is_some() {
        return Err(ProtocolError::UnsupportedDischarge);
    }
    spec.check_witness(witness)?;
    validate_bit_rows(&prefix.params(), hint.rows())?;
    validate_commitment(&prefix.params(), &hint.commitment, pcs.prover_config())?;
    // The statement, a resolved ladder's policy digest and Round 0, as the
    // runner binds them; then the session and the ladder's identity (the
    // `fast` ladder's only digest: it has no policy digest).
    let (binding, ood) = bind_prover_statement(transcript, prefix, configuration, hint)?;
    transcript.absorb_slice(SESSION);
    transcript.absorb_slice(digest);
    let proved = prove_piop(transcript, prefix, witness, &binding)?;
    let prime = &proved.prime;

    let _step5 = tracing::info_span!("step5:open_prove").entered();
    let params = opening_params(&prefix.params(), modulus_u128(prime))?;
    let claim = {
        let _scope = tracing::info_span!("step5:bitz-claim").entered();
        linear_claim(&params, &proved.opening, &proved.table, prime)?
    };
    let ood = ood.opening_claim(transcript, hint);
    let tag = fork_tag(transcript);
    let mut state = build_prover(SESSION, &tag);
    state.public_message(&proved.bridge_digest);
    BitZProver::new(params, WINDOW)
        .prove(
            &claim,
            pcs,
            hint,
            &mut state,
            ood.as_ref().map(|claim| (claim.point.as_slice(), claim.y)),
        )
        .map_err(|error| ProtocolError::LigeritoConfig(format!("BitZ prove: {error:?}")))?;
    let BitzTranscriptProof { narg_string, hints } = state.finish();
    Ok(Proof::from_parts(
        proved.messages,
        None,
        WfbitzOpeningProof {
            narg: narg_string,
            hints,
            ood: ood.map(|claim| claim.round),
        },
    ))
}

/// Verifies a BitZ-discharged proof.
pub fn verify<T: Transcript + Send, S: RelationSpec>(
    transcript: &mut T,
    prefix: &PreparedRelationPrefix<S>,
    opener: &WfbitzOpener,
    commitment: &Commitment,
    proof: &Proof,
) -> Result<(), ProtocolError> {
    if prefix.params() != *opener.layout() {
        return Err(ProtocolError::RelationWitnessLayoutMismatch);
    }
    check_native_policy(prefix, opener)?;
    verify_direct(
        transcript,
        prefix,
        &opener.opener(),
        opener.pcs(),
        &opener.digest(),
        commitment,
        proof,
    )
}

pub(super) fn verify_direct<T: Transcript + Send, S: RelationSpec>(
    transcript: &mut T,
    prefix: &PreparedRelationPrefix<S>,
    configuration: &Opener,
    pcs: &BitzPcs,
    digest: &[u8; 32],
    commitment: &Commitment,
    proof: &Proof,
) -> Result<(), ProtocolError> {
    if prefix.layout().map().is_some() {
        return Err(ProtocolError::UnsupportedDischarge);
    }
    check_proof_kernel(prefix.layout().kernel(), &proof.prefix.spartan)?;
    let binding_config = configuration.binding_config()?;
    validate_commitment(&prefix.params(), commitment, &binding_config)?;
    let (binding, ood) = bind_verifier_statement(
        transcript,
        prefix,
        configuration,
        commitment,
        proof.bitz().ood(),
    )?;
    transcript.absorb_slice(SESSION);
    transcript.absorb_slice(digest);
    let verified = verify_piop(transcript, prefix, &binding, proof.prefix())?;
    let prime = &verified.prime;

    let _step5 = tracing::info_span!("step5:open_verify").entered();
    let params = opening_params(&prefix.params(), modulus_u128(prime))?;
    let claim = linear_claim(&params, &verified.opening, &verified.table, prime)?;
    let opening = proof.bitz();
    let ood = ood
        .opening_claim(
            transcript,
            packed_variables(&prefix.params())?,
            opening.ood(),
        )
        .map_err(ProtocolError::Bitz)?;
    let tag = fork_tag(transcript);
    let bitz_proof = BitzTranscriptProof {
        narg_string: opening.narg.clone(),
        hints: opening.hints.clone(),
    };
    let mut state = build_verifier(SESSION, &tag, &bitz_proof);
    state.public_message(&verified.bridge_digest);
    BitZVerifier::new(params, WINDOW)
        .verify(
            &claim,
            pcs,
            Root(commitment.root),
            state,
            ood.as_ref().map(|claim| (claim.point.as_slice(), claim.y)),
        )
        .map_err(|error| ProtocolError::LigeritoConfig(format!("BitZ verify: {error:?}")))
}

/// The session tag of the standalone opening's fork.
const STANDALONE_SESSION: &[u8] = b"bitz/wfbitz-opener/standalone/v1";

impl WfbitzOpener {
    /// Round 0 at the ladder's own target (`None` for a unique-decoding
    /// ladder), as the crate's standalone opener runs it.
    fn standalone_ood(&self) -> Option<crate::ligerito_flock::OodRoundParams> {
        crate::ligerito_flock::ood_round_params(
            &self.security,
            self.packed_vars,
            self.security.target_security_bits as u32,
        )
    }
}

/// The statement of a standalone claim, bound the way the crate's own
/// standalone opener (the `bitz` CLI) binds it: the frame (commitment, the
/// ladder's verifier configuration, the shape, the generator, the prime
/// width, the Round-0 parameters), then the ladder's digest.
fn bind_standalone_statement<T: Transcript>(
    transcript: &mut T,
    opener: &WfbitzOpener,
    commitment: &Commitment,
    q_bits: usize,
    ood: Option<crate::ligerito_flock::OodRoundParams>,
) {
    crate::ligerito_flock::absorb_standalone_mod_q_statement(
        transcript,
        commitment,
        &opener.layout,
        bitz_generator(),
        q_bits,
        ood,
        opener.pcs.verifier_config(),
    );
    transcript.absorb_slice(STANDALONE_SESSION);
    transcript.absorb_slice(&opener.digest);
}

/// The claim of a standalone opening once its statement and Round 0 are
/// bound: the transcript-sampled prime and point
/// ([`crate::ligerito_flock::sample_standalone_instance`], the CLI's raw
/// claim) as BitZ's factored claim with value `claimed`.
fn standalone_claim<T: Transcript>(
    transcript: &mut T,
    opener: &WfbitzOpener,
    q_bits: usize,
    claimed: u128,
) -> Result<(BitZParams, LinearClaim), ProtocolError> {
    let instance =
        crate::ligerito_flock::sample_standalone_instance(transcript, &opener.layout, q_bits);
    if claimed >= instance.q {
        return Err(ProtocolError::LigeritoConfig(
            "BitZ claim: value not reduced".into(),
        ));
    }
    crate::ligerito_flock::absorb_standalone_mod_q_claim(transcript, instance.q, claimed);
    let params = opener.params(instance.q)?;
    let claim = LinearClaim::new(
        &params,
        instance.row_weights_q,
        instance.col_weights_q,
        claimed,
    )
    .map_err(|error| ProtocolError::LigeritoConfig(format!("BitZ claim: {error:?}")))?;
    Ok((params, claim))
}

/// The honest value of the standalone claim on the committed bits of
/// `hint`: the statement, Round 0 and the prime and point draws replayed as
/// [`prove_standalone`] runs them, then the bits folded against the drawn
/// weights. Callers compute it once, outside any timer (the claim is the
/// statement's; the CLI does the same).
pub fn standalone_evaluation(
    opener: &WfbitzOpener,
    hint: &FlockCommitHint,
) -> Result<u128, ProtocolError> {
    validate_bit_rows(&opener.layout, hint.rows())?;
    let q_bits = crate::ligerito_flock::standalone_q_bits(&opener.layout);
    let ood = opener.standalone_ood();
    let mut transcript = crate::transcript::Blake3Transcript::new();
    bind_standalone_statement(&mut transcript, opener, &hint.commitment, q_bits, ood);
    let _ = crate::ligerito_flock::bind_prover_ood(&mut transcript, hint, ood);
    let instance =
        crate::ligerito_flock::sample_standalone_instance(&mut transcript, &opener.layout, q_bits);
    let params = opener.params(instance.q)?;
    let unresolved =
        LinearClaim::new(&params, instance.row_weights_q, instance.col_weights_q, 0)
            .map_err(|error| ProtocolError::LigeritoConfig(format!("BitZ claim: {error:?}")))?;
    let folds =
        crate::wfbitz::fold::fold_columns(&opener.shape, hint.rows(), unresolved.row_exponents());
    Ok(crate::wfbitz::fold::reconstruct(
        &unresolved,
        &folds,
        instance.q,
    ))
}

/// Proves a standalone claim `⟨eq(·, r₁) ⊗ eq(·, r₂), f⟩ = claimed` on the
/// committed bits of `hint` — the `bitz` CLI's raw claim (statement frame,
/// Round 0 for a Johnson ladder, transcript-sampled prime and point, claim
/// frame) opened by BitZ's scheme on a transcript forked from that state.
pub fn prove_standalone(
    opener: &WfbitzOpener,
    hint: &FlockCommitHint,
    claimed: u128,
) -> Result<WfbitzOpeningProof, ProtocolError> {
    validate_bit_rows(&opener.layout, hint.rows())?;
    let q_bits = crate::ligerito_flock::standalone_q_bits(&opener.layout);
    let ood_params = opener.standalone_ood();
    let mut transcript = crate::transcript::Blake3Transcript::new();
    bind_standalone_statement(
        &mut transcript,
        opener,
        &hint.commitment,
        q_bits,
        ood_params,
    );
    let ood = crate::ligerito_flock::bind_prover_ood(&mut transcript, hint, ood_params)
        .opening_claim(&mut transcript, hint);
    let (params, claim) = standalone_claim(&mut transcript, opener, q_bits, claimed)?;
    let tag = fork_tag(&mut transcript);
    let mut state = build_prover(STANDALONE_SESSION, &tag);
    BitZProver::new(params, WINDOW)
        .prove(
            &claim,
            opener.pcs(),
            hint,
            &mut state,
            ood.as_ref().map(|claim| (claim.point.as_slice(), claim.y)),
        )
        .map_err(|error| ProtocolError::LigeritoConfig(format!("BitZ prove: {error:?}")))?;
    let BitzTranscriptProof { narg_string, hints } = state.finish();
    Ok(WfbitzOpeningProof {
        narg: narg_string,
        hints,
        ood: ood.map(|claim| claim.round),
    })
}

/// Verifies a [`prove_standalone`] proof of `claimed` against `commitment`.
pub fn verify_standalone(
    opener: &WfbitzOpener,
    commitment: &Commitment,
    claimed: u128,
    proof: &WfbitzOpeningProof,
) -> Result<(), ProtocolError> {
    validate_commitment(&opener.layout, commitment, opener.pcs.prover_config())?;
    let q_bits = crate::ligerito_flock::standalone_q_bits(&opener.layout);
    let ood_params = opener.standalone_ood();
    let mut transcript = crate::transcript::Blake3Transcript::new();
    bind_standalone_statement(&mut transcript, opener, commitment, q_bits, ood_params);
    let ood = crate::ligerito_flock::bind_verifier_ood(
        &mut transcript,
        opener.packed_vars,
        ood_params,
        proof.ood.as_ref(),
    )
    .map_err(ProtocolError::Bitz)?
    .opening_claim(&mut transcript, opener.packed_vars, proof.ood.as_ref())
    .map_err(ProtocolError::Bitz)?;
    let (params, claim) = standalone_claim(&mut transcript, opener, q_bits, claimed)?;
    let tag = fork_tag(&mut transcript);
    let bitz_proof = BitzTranscriptProof {
        narg_string: proof.narg.clone(),
        hints: proof.hints.clone(),
    };
    BitZVerifier::new(params, WINDOW)
        .verify(
            &claim,
            opener.pcs(),
            Root(commitment.root),
            build_verifier(STANDALONE_SESSION, &tag, &bitz_proof),
            ood.as_ref().map(|claim| (claim.point.as_slice(), claim.y)),
        )
        .map_err(|error| ProtocolError::LigeritoConfig(format!("BitZ verify: {error:?}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        piop::spartan::{
            Lambda100,
            mul::{MulLayout, MulWitness},
        },
        transcript::Blake3Transcript,
    };

    fn u32_witness(multiplications: usize) -> MulWitness<u32> {
        let inputs: Vec<(u32, u32)> = (0..multiplications as u64)
            .map(|i| {
                let a = (i.wrapping_mul(0x9e37_79b9) ^ 0x5bd1_e995) as u32;
                let b = (i.wrapping_mul(0x85eb_ca6b) ^ 0xc2b2_ae35) as u32;
                (a, b)
            })
            .collect();
        MulWitness::<u32>::from_inputs(&inputs).unwrap()
    }

    fn u64_witness(multiplications: usize) -> MulWitness<u64> {
        let mut state = 0x243f_6a88_85a3_08d3_u64;
        MulWitness::<u64>::from_fn(multiplications, |_| {
            let mut next = || {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                state
            };
            (next(), next())
        })
        .unwrap()
    }

    /// The u32 relation proves and verifies through the BitZ opener under
    /// both ladders; a flipped opening byte and a foreign proof are
    /// rejected.
    #[test]
    fn u32_products_prove_through_bitz_under_both_ladders() {
        let multiplications = 1 << 15;
        let witness = u32_witness(multiplications);
        let layout = MulLayout::<u32>::new(multiplications).unwrap();
        for ladder in [
            WfbitzLigerito::Fast,
            WfbitzLigerito::Selected(LigeritoSelection::MATCHED_UDR),
        ] {
            let (prefix, opener) =
                WfbitzOpener::prepare::<Lambda100, _>(layout, ladder, 100).unwrap();
            // Round 0 runs for the Johnson ladder only.
            assert_eq!(
                prefix.security().ood.is_some(),
                matches!(ladder, WfbitzLigerito::Fast),
                "{ladder:?}"
            );
            let hint = opener.commit(witness.bitz_bit_rows()).unwrap();
            let proof = prove(
                &mut Blake3Transcript::new(),
                &prefix,
                &opener,
                &witness,
                &hint,
            )
            .unwrap();
            verify(
                &mut Blake3Transcript::new(),
                &prefix,
                &opener,
                &hint.commitment,
                &proof,
            )
            .unwrap();
            assert_eq!(proof.bitz().ood.is_some(), prefix.security().ood.is_some());
            let bytes = proof.bitz().to_bytes();
            assert_eq!(
                WfbitzOpeningProof::from_bytes(&bytes).as_ref(),
                Some(proof.bitz())
            );
            let (bits, term) = opener.opening_bits(prefix.security().ood);
            assert!(bits >= 100.0, "{ladder:?}: {bits} bits ({term})");
            if proof.bitz().ood.is_some() {
                // A wrong out-of-domain value is caught by the batched opening.
                let mut tampered = proof.clone();
                let round = tampered.bitz_mut().ood.as_mut().unwrap();
                round.y = round.y + field::Gf128::ONE;
                assert!(
                    verify(
                        &mut Blake3Transcript::new(),
                        &prefix,
                        &opener,
                        &hint.commitment,
                        &tampered
                    )
                    .is_err()
                );
            }

            let mut tampered = proof.clone();
            tampered.bitz_mut().narg[7] ^= 1;
            assert!(
                verify(
                    &mut Blake3Transcript::new(),
                    &prefix,
                    &opener,
                    &hint.commitment,
                    &tampered
                )
                .is_err()
            );
            let mut tampered = proof.clone();
            let last = tampered.bitz().hints.len() - 1;
            tampered.bitz_mut().hints[last] ^= 1;
            assert!(
                verify(
                    &mut Blake3Transcript::new(),
                    &prefix,
                    &opener,
                    &hint.commitment,
                    &tampered
                )
                .is_err()
            );
        }
    }

    /// The standalone claim (the CLI's raw claim through BitZ's scheme):
    /// Round 0 runs for a Johnson ladder and is absent for a unique-decoding
    /// one; the honest value verifies, and a different value, a changed
    /// Round-0 value, a stray Round-0 record and a flipped byte do not.
    #[test]
    fn standalone_claim_binds_its_value_and_round_0() {
        let shape = Shape::reference(22).unwrap();
        let mut state = 0x2545_f491_4f6c_dd1du64;
        let mut next = move || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        let rows: Vec<Vec<u64>> = (0..shape.columns())
            .map(|_| (0..shape.rows() / 64).map(|_| next()).collect())
            .collect();
        for ladder in ["custom:1:4", "udr:1:4"] {
            let opener = WfbitzOpener::new(
                shape.layout(),
                WfbitzLigerito::parse(ladder, 100).unwrap(),
                100,
            )
            .unwrap();
            let hint = opener.commit(rows.clone()).unwrap();
            let claimed = standalone_evaluation(&opener, &hint).unwrap();
            let proof = prove_standalone(&opener, &hint, claimed).unwrap();
            assert_eq!(proof.ood.is_some(), opener.ood_bits().is_some(), "{ladder}");
            verify_standalone(&opener, &hint.commitment, claimed, &proof).unwrap();
            let bytes = proof.to_bytes();
            assert_eq!(
                WfbitzOpeningProof::from_bytes(&bytes).as_ref(),
                Some(&proof)
            );
            assert!(
                verify_standalone(&opener, &hint.commitment, claimed ^ 1, &proof).is_err(),
                "{ladder}: a different value"
            );
            let mut tampered = proof.clone();
            match tampered.ood.as_mut() {
                Some(round) => round.y = round.y + field::Gf128::ONE,
                None => {
                    tampered.ood = Some(OodRound {
                        y: field::Gf128::ONE,
                        nonce: None,
                    })
                }
            }
            assert!(
                verify_standalone(&opener, &hint.commitment, claimed, &tampered).is_err(),
                "{ladder}: Round 0"
            );
            let mut tampered = proof.clone();
            tampered.narg[9] ^= 1;
            assert!(
                verify_standalone(&opener, &hint.commitment, claimed, &tampered).is_err(),
                "{ladder}: a flipped byte"
            );
        }
    }

    /// A proof under one ladder does not verify under the other (the
    /// statement binding covers the ladder).
    #[test]
    fn ladders_are_bound() {
        let multiplications = 1 << 15;
        let witness = u64_witness(multiplications);
        let layout = MulLayout::<u64>::new(multiplications).unwrap();
        let (prefix, fast) =
            WfbitzOpener::prepare::<Lambda100, _>(layout, WfbitzLigerito::Fast, 100).unwrap();
        let (udr_prefix, udr) = WfbitzOpener::prepare::<Lambda100, _>(
            layout,
            WfbitzLigerito::Selected(LigeritoSelection::MATCHED_UDR),
            100,
        )
        .unwrap();
        let hint = fast.commit(witness.bitz_bit_rows()).unwrap();
        let proof = prove(
            &mut Blake3Transcript::new(),
            &prefix,
            &fast,
            &witness,
            &hint,
        )
        .unwrap();
        verify(
            &mut Blake3Transcript::new(),
            &prefix,
            &fast,
            &hint.commitment,
            &proof,
        )
        .unwrap();
        assert!(
            verify(
                &mut Blake3Transcript::new(),
                &udr_prefix,
                &udr,
                &hint.commitment,
                &proof
            )
            .is_err()
        );
    }

    /// Under a resolved ladder the statement phase is the crate's own
    /// runner's ([`PreparedRelation::with_ligerito`]): the configuration
    /// the statement binds is the one the scheme runs, the commitment is
    /// the runner's, and the statement, the policy digest and Round 0 are
    /// absorbed in the runner's order, so the Round-0 record of a proof is
    /// the forest proof's.
    #[test]
    fn statement_phase_matches_the_runner() {
        use crate::piop::spartan::{mul::MulWord, protocol as runner};

        fn check<T: MulWord>(input: impl FnMut(usize) -> (T, T))
        where
            MulLayout<T>: RelationSpec<Witness = MulWitness<T>>,
        {
            let layout = MulLayout::<T>::new(1 << 15)
                .unwrap()
                .with_split_shift(0)
                .unwrap();
            let witness = MulWitness::from_fn_with_layout(layout, input).unwrap();
            let selection = LigeritoSelection::JOHNSON;
            let forest =
                PreparedRelation::new_with_profile_and_ligerito::<Lambda100>(layout, selection)
                    .unwrap();
            let (prefix, opener) = WfbitzOpener::prepare::<Lambda100, _>(
                layout,
                WfbitzLigerito::Selected(selection),
                100,
            )
            .unwrap();
            let configuration = opener.opener();
            let resolved = configuration.resolved().expect("a resolved ladder");
            assert_eq!(resolved.digest(), forest.ligerito_configuration().digest());
            assert_eq!(resolved.digest(), opener.digest());
            // What the statement binds and the commitment is checked against
            // is what BitZ commits and opens under.
            let bound = configuration.binding_config().unwrap();
            assert_eq!(
                format!("{bound:?}"),
                format!("{:?}", opener.pcs().prover_config())
            );
            assert_eq!(
                format!("{:?}", configuration.verifier().unwrap()),
                format!("{:?}", opener.pcs().verifier_config())
            );
            assert_eq!(prefix.security().ood, forest.security().ood);
            assert!(prefix.security().ood.is_some());

            let rows = witness.bitz_bit_rows();
            let forest_hint = runner::commit(&forest, rows.clone()).unwrap();
            let hint = opener.commit(rows).unwrap();
            assert_eq!(hint.commitment.root, forest_hint.commitment.root);
            let forest_proof = runner::prove(
                &mut Blake3Transcript::new(),
                &forest,
                &witness,
                &forest_hint,
            )
            .unwrap();
            let proof = prove(
                &mut Blake3Transcript::new(),
                &prefix,
                &opener,
                &witness,
                &hint,
            )
            .unwrap();
            verify(
                &mut Blake3Transcript::new(),
                &prefix,
                &opener,
                &hint.commitment,
                &proof,
            )
            .unwrap();
            assert!(proof.bitz().ood().is_some());
            assert_eq!(proof.bitz().ood(), forest_proof.bitz().ood());
        }

        check::<u32>(|i| {
            let i = i as u32;
            (
                i.wrapping_mul(0x9e37_79b9) ^ 0x5bd1_e995,
                i.wrapping_mul(0x85eb_ca6b) ^ 0xc2b2_ae35,
            )
        });
        check::<u64>(|i| {
            let i = i as u64;
            (
                i.wrapping_mul(0x9e37_79b9_7f4a_7c15) ^ 0x243f_6a88_85a3_08d3,
                !i.rotate_left(17),
            )
        });
    }

    /// The commitment is checked against the opener's configuration up
    /// front on both sides, as the crate's own runner does: a commitment
    /// that declares another rate, profile or size is refused as such, not
    /// later by a transcript mismatch.
    #[test]
    fn commitment_configuration_is_checked_up_front() {
        let multiplications = 1 << 15;
        let witness = u32_witness(multiplications);
        let layout = MulLayout::<u32>::new(multiplications).unwrap();
        // Rate 1/8, the ladder of the paper's u32 rows.
        let ladder = WfbitzLigerito::parse("custom:3:4", 100).unwrap();
        let (prefix, opener) = WfbitzOpener::prepare::<Lambda100, _>(layout, ladder, 100).unwrap();
        let mut hint = opener.commit(witness.bitz_bit_rows()).unwrap();
        let proof = prove(
            &mut Blake3Transcript::new(),
            &prefix,
            &opener,
            &witness,
            &hint,
        )
        .unwrap();
        verify(
            &mut Blake3Transcript::new(),
            &prefix,
            &opener,
            &hint.commitment,
            &proof,
        )
        .unwrap();
        let changes: [fn(&mut flock_core::pcs::commit::PcsParams); 3] = [
            |params| params.log_inv_rate += 1,
            |params| params.profile = LigeritoProfile::Slim,
            |params| params.m += 1,
        ];
        for change in changes {
            let mut foreign = hint.commitment.clone();
            change(&mut foreign.params);
            assert!(matches!(
                verify(
                    &mut Blake3Transcript::new(),
                    &prefix,
                    &opener,
                    &foreign,
                    &proof
                ),
                Err(ProtocolError::CommitmentConfigMismatch)
            ));
        }
        hint.commitment.params.log_inv_rate += 1;
        assert!(matches!(
            prove(
                &mut Blake3Transcript::new(),
                &prefix,
                &opener,
                &witness,
                &hint
            ),
            Err(ProtocolError::CommitmentConfigMismatch)
        ));
    }

    /// A prefix that credits forest or ring-switch grinding (λ = 128 built
    /// without [`WfbitzOpener::prepare`]) is refused by the direct discharge
    /// before any transcript work, as the reduced discharge refuses it.
    #[test]
    fn a_prefix_crediting_opener_grinding_is_not_discharged() {
        let multiplications = 1 << 15;
        let witness = u32_witness(multiplications);
        let layout = MulLayout::<u32>::new(multiplications).unwrap();
        let (prefix, opener) =
            WfbitzOpener::prepare::<Lambda100, _>(layout, WfbitzLigerito::Fast, 100).unwrap();
        let hint = opener.commit(witness.bitz_bit_rows()).unwrap();
        let proof = prove(
            &mut Blake3Transcript::new(),
            &prefix,
            &opener,
            &witness,
            &hint,
        )
        .unwrap();
        let credited =
            PreparedRelationPrefix::new::<crate::piop::spartan::Lambda128>(layout).unwrap();
        let security = credited.security();
        assert!(
            security.forest_round_grinding_bits != 0 || security.ring_switch_grinding_bits != 0
        );
        let ladder = WfbitzLigerito::Selected(LigeritoSelection::ValidatedUdr);
        let udr = WfbitzOpener::new(RelationSpec::committed_layout(&layout), ladder, 128).unwrap();
        let mut transcript = Blake3Transcript::new();
        let fresh = transcript.state_digest();
        assert!(matches!(
            prove(&mut transcript, &credited, &udr, &witness, &hint),
            Err(ProtocolError::UnsupportedProfile)
        ));
        assert!(matches!(
            verify(&mut transcript, &credited, &udr, &hint.commitment, &proof),
            Err(ProtocolError::UnsupportedProfile)
        ));
        assert_eq!(transcript.state_digest(), fresh);
    }

    /// A stated length past the end of the input is a truncation, not an
    /// overflow. (Under overflow checks, as in a debug build, the unchecked
    /// `at + len` panicked; in release it wrapped to an empty range, so only
    /// a debug run tells the two apart.)
    #[test]
    fn opening_codec_rejects_an_oversized_length() {
        for len in [u64::MAX, u64::MAX - 7, 1 << 40] {
            let mut bytes = len.to_le_bytes().to_vec();
            bytes.extend_from_slice(&[0; 16]);
            assert_eq!(WfbitzOpeningProof::from_bytes(&bytes), None, "{len}");
            let mut bytes = 0u64.to_le_bytes().to_vec();
            bytes.extend_from_slice(&len.to_le_bytes());
            assert_eq!(WfbitzOpeningProof::from_bytes(&bytes), None, "{len}");
        }
    }

    /// Round 0 is reported with the grinding the proof runs (the profile's
    /// λ, as `prepare` adopts it), not at the ladder's own target.
    #[test]
    fn round_0_is_reported_as_it_runs() {
        let layout = MulLayout::<u32>::new(1 << 15).unwrap();
        // A Johnson ladder resolved above the profile's λ = 100.
        let ladder = WfbitzLigerito::parse("custom:1:4", 110).unwrap();
        let (prefix, opener) = WfbitzOpener::prepare::<Lambda100, _>(layout, ladder, 110).unwrap();
        let bits = opener.ood_bits().unwrap();
        let executed = prefix.security().ood.unwrap();
        assert_eq!(
            executed.grinding_bits,
            (100.0 - bits).max(0.0).ceil() as u32
        );
        let booked = prefix
            .security()
            .accounting
            .terms
            .iter()
            .find(|term| term.name == "step0:ood-draw")
            .unwrap();
        assert_eq!(opener.round_0_bits(Some(executed)), Some(booked.bits));
        assert_eq!(opener.round_0_bits(None), None);
        // The ladder's own target would grind more than runs.
        let at_target =
            crate::ligerito_flock::ood_round_params(opener.security(), opener.packed_vars, 110)
                .unwrap();
        assert!(at_target.grinding_bits > executed.grinding_bits, "{bits}");
        let (reported, term) = opener.opening_bits(Some(executed));
        assert!(reported <= booked.bits, "{reported} ({term})");
        assert!(
            reported < bits + f64::from(at_target.grinding_bits),
            "{reported} ({term})"
        );
        // A Johnson ladder without its Round 0 is credited nothing.
        assert_eq!(opener.opening_bits(None).0, 0.0);
    }

    /// Native terms come from the executed schedule; the independent Flock
    /// field floor remains visible even when controllable terms reach 128 bits.
    #[test]
    fn prepare_accounts_for_the_executed_native_schedule() {
        let layout = MulLayout::<u32>::new(1 << 15).unwrap();
        let ladder = WfbitzLigerito::Selected(LigeritoSelection::ValidatedUdr);
        let (low, _) = WfbitzOpener::prepare::<Lambda100, _>(layout, ladder, 100).unwrap();
        assert_eq!(low.security().native_grinding_nonce_count(), 0);
        assert!(low.security().accounting.controllable_bits() >= 100.0);
        let (high, opener) =
            WfbitzOpener::prepare::<crate::piop::spartan::Lambda128, _>(layout, ladder, 128)
                .unwrap();
        assert!(high.security().native_grinding_nonce_count() > 0);
        assert!(high.security().accounting.controllable_bits() >= 128.0);
        assert!((high.security().accounting.achieved_bits() - (128.0 - 3f64.log2())).abs() < 1e-9);
        let witness = u32_witness(1 << 15);
        let hint = opener.commit(witness.bitz_bit_rows()).unwrap();
        let proof = prove(
            &mut Blake3Transcript::new(),
            &high,
            &opener,
            &witness,
            &hint,
        )
        .unwrap();
        verify(
            &mut Blake3Transcript::new(),
            &high,
            &opener,
            &hint.commitment,
            &proof,
        )
        .unwrap();
    }
}

/// The session tag of the reduced (two-prime, virtual) discharge's fork.
const REDUCED_SESSION: &[u8] = b"bitz/wfbitz-opener/reduced/v1";

/// The scheme over the relation's opener: a resolved ladder's security
/// config, or explicit configurations (MultiSwap's UDR ladder).
fn reduced_pcs(opener: &Opener, committed: &Shape) -> Result<BitzPcs, ProtocolError> {
    let pcs = match opener {
        Opener::Resolved(resolved) => {
            BitzPcs::with_security(committed, resolved.security(), LigeritoProfile::Fast)
        }
        Opener::Custom {
            prover: Some(prover),
            verifier: Some(verifier),
        } => BitzPcs::from_configs(committed, prover.clone(), verifier.clone()),
        Opener::Custom { .. } => return Err(ProtocolError::OpenerConfigUnavailable),
    };
    pcs.map_err(|error| ProtocolError::LigeritoConfig(format!("BitZ pcs: {error:?}")))
}

/// The derived grid `h = M·f` the claim is about, as per-column bit rows of
/// the opening layout: the relation's own derived rows when it materialises
/// them, the committed rows under an identity map between equal grids (the
/// statement's direct case), and otherwise the map applied to the committed
/// rows (every source cell XORed into the derived cells it feeds; `O(nnz)`).
fn derived_bit_rows<S: RelationSpec>(
    spec: &S,
    witness: &S::Witness,
    committed: &IntegerMatrixLayout,
    opening: &IntegerMatrixLayout,
    rows: &[Vec<u64>],
) -> Vec<Vec<u64>> {
    if let Some(derived) = spec.derived_rows(witness) {
        return derived;
    }
    let map = match spec.map() {
        Some(map) if !(map.is_identity() && opening == committed) => map,
        _ => return rows.to_vec(),
    };
    use circuit::linear_map::binary::VirtualMap;
    let words = opening.rows() / 64;
    let mut derived = vec![vec![0u64; words]; opening.cols()];
    let t_f = committed.row_vars;
    let t_h = opening.row_vars;
    for (source_column, row) in rows.iter().enumerate() {
        for (word_index, &word) in row.iter().enumerate() {
            let mut bits = word;
            while bits != 0 {
                let b = word_index * 64 + bits.trailing_zeros() as usize;
                bits &= bits - 1;
                let source = (source_column << t_f) | b;
                if let Some(targets) = map.column_rows(source) {
                    for d in targets {
                        let (c, r) = (d >> t_h, d & ((1 << t_h) - 1));
                        derived[c][r / 64] ^= 1u64 << (r % 64);
                    }
                }
            }
        }
    }
    derived
}

/// Proves a two-prime relation (MultiSwap's Strategy 2) with the reduced
/// claim discharged through the scheme's virtual opening
/// ([`crate::wfbitz::virt`]): the protocol prefix, the integer lift and the
/// second prime are the runner's (`protocol::prove_reduced`), then the fold
/// and GKR run over the derived grid on a forked transcript and the
/// transposed claim is opened on the commitment under the relation's own
/// ladder.
pub fn prove_reduced<T: Transcript + Send, S: RelationSpec>(
    transcript: &mut T,
    prepared: &PreparedRelation<S>,
    witness: &S::Witness,
    hint: &FlockCommitHint,
) -> Result<Proof, ProtocolError> {
    let prefix = &prepared.prefix;
    let opener = &prepared.opener;
    let spec = &prefix.spec;
    spec.check_witness(witness)?;
    let p = prepared.params();
    let pc = opener.prover()?;
    validate_bit_rows(&p, hint.rows())?;
    validate_commitment(&p, &hint.commitment, pc)?;
    let security = &prefix.security;
    let reduction = security
        .reduction
        .ok_or(ProtocolError::UnsupportedProfile)?;
    let domains = spec.domains();
    let map = spec.map().ok_or(ProtocolError::UnsupportedDischarge)?;
    let (binding, ood) = bind_prover_statement(transcript, prefix, opener, hint)?;
    let proved = prove_piop(transcript, prefix, witness, &binding)?;
    let prime = &proved.prime;
    let row_weights = bitify::dense_row_weights(&proved.opening, &proved.table, prime)?;
    let col_weights: Vec<u128> = bitify::column_weights(&proved.opening, prime)?;
    let step5_0_scope = tracing::info_span!("step5_0:reduce_prove").entered();
    let mu_prime = step50_integer_lift(hint.rows(), &row_weights, &col_weights);
    if !step50_accepts_lift(
        &mu_prime,
        proved.opening.claimed,
        prime.modulus_u128(),
        p.cells(),
    ) {
        return Err(ProtocolError::InvalidIntegerLift);
    }
    bind_claim_frame(
        transcript,
        spec,
        ClaimFrame {
            field: prime,
            binding: &binding,
            matrices_digest: &proved.matrices_digest,
            terminal_claim: &proved.terminal_claim,
            opening: &proved.opening,
            row_weights: &row_weights,
            col_weights: &col_weights,
            mu_prime: Some(&mu_prime),
        },
    )?;
    let nonce = grind_and_absorb_in_domain(
        transcript,
        domains.reduction_grinding,
        0,
        reduction.grinding_bits,
    )?;
    let reduced = sample_mod_q(
        transcript,
        domains.reduction_prime,
        reduction.min,
        reduction.max,
    )?;
    let (rows_reduced, cols_reduced, claimed_reduced) = step50_reduce(
        &row_weights,
        &col_weights,
        &mu_prime,
        reduced.modulus_u128(),
    );
    drop(step5_0_scope);

    let _step5 = tracing::info_span!("step5:open_prove").entered();
    let opening_layout = spec.opening_layout();
    let derived_shape = Shape::new(opening_layout.row_vars, opening_layout.col_vars)
        .map_err(|error| ProtocolError::LigeritoConfig(format!("BitZ derived shape: {error:?}")))?;
    let committed = Shape::new(p.row_vars, p.col_vars)
        .map_err(|error| ProtocolError::LigeritoConfig(format!("BitZ shape: {error:?}")))?;
    let params = BitZParams::new(
        derived_shape,
        reduced.modulus_u128(),
        bitz_generator().into(),
    )
    .map_err(|error| ProtocolError::LigeritoConfig(format!("BitZ params: {error:?}")))?;
    let claim = LinearClaim::new(&params, rows_reduced, cols_reduced, claimed_reduced)
        .map_err(|error| ProtocolError::LigeritoConfig(format!("BitZ claim: {error:?}")))?;
    let statement = crate::wfbitz::VirtualStatement::new(params, committed, map, &claim)
        .map_err(|error| ProtocolError::LigeritoConfig(format!("BitZ statement: {error:?}")))?;
    let pcs = reduced_pcs(opener, &committed)?.with_native_policy(security.native_policy()?);
    let derived_rows = derived_bit_rows(spec, witness, &p, &opening_layout, hint.rows());
    let ood = ood.opening_claim(transcript, hint);
    let tag = fork_tag(transcript);
    let mut state = build_prover(REDUCED_SESSION, &tag);
    state.public_message(&proved.bridge_digest);
    BitZProver::new(params, WINDOW)
        .prove_virtual(
            &statement,
            &pcs,
            hint,
            &derived_rows,
            &mut state,
            ood.as_ref().map(|claim| (claim.point.as_slice(), claim.y)),
        )
        .map_err(|error| ProtocolError::LigeritoConfig(format!("BitZ prove: {error:?}")))?;
    let BitzTranscriptProof { narg_string, hints } = state.finish();
    Ok(Proof::from_parts(
        proved.messages,
        Some(ReductionProof { mu_prime, nonce }),
        WfbitzOpeningProof {
            narg: narg_string,
            hints,
            ood: ood.map(|claim| claim.round),
        },
    ))
}

/// Verifies a [`prove_reduced`] proof.
pub fn verify_reduced<T: Transcript + Send, S: RelationSpec>(
    transcript: &mut T,
    prepared: &PreparedRelation<S>,
    commitment: &Commitment,
    proof: &Proof,
) -> Result<(), ProtocolError> {
    let prefix = &prepared.prefix;
    let opener = &prepared.opener;
    let spec = &prefix.spec;
    let p = prepared.params();
    let binding_config = opener.binding_config()?;
    validate_commitment(&p, commitment, &binding_config)?;
    let security = &prefix.security;
    let reduction = security
        .reduction
        .ok_or(ProtocolError::UnsupportedProfile)?;
    let domains = spec.domains();
    let map = spec.map().ok_or(ProtocolError::UnsupportedDischarge)?;
    let lift = proof
        .reduction
        .as_ref()
        .ok_or(ProtocolError::InvalidIntegerLift)?;
    check_proof_kernel(spec.kernel(), &proof.prefix.spartan)?;
    let (binding, ood) = bind_verifier_statement(
        transcript,
        prefix,
        opener,
        commitment,
        proof.bitz.ood.as_ref(),
    )?;
    let verified = verify_piop(transcript, prefix, &binding, &proof.prefix)?;
    let prime = &verified.prime;
    let row_weights = bitify::dense_row_weights(&verified.opening, &verified.table, prime)?;
    let col_weights: Vec<u128> = bitify::column_weights(&verified.opening, prime)?;
    if !step50_accepts_lift(
        &lift.mu_prime,
        verified.opening.claimed,
        prime.modulus_u128(),
        p.cells(),
    ) {
        return Err(ProtocolError::InvalidIntegerLift);
    }
    bind_claim_frame(
        transcript,
        spec,
        ClaimFrame {
            field: prime,
            binding: &binding,
            matrices_digest: &verified.matrices_digest,
            terminal_claim: &verified.terminal_claim,
            opening: &verified.opening,
            row_weights: &row_weights,
            col_weights: &col_weights,
            mu_prime: Some(&lift.mu_prime),
        },
    )?;
    verify_and_absorb_in_domain(
        transcript,
        domains.reduction_grinding,
        0,
        reduction.grinding_bits,
        lift.nonce,
    )?;
    let reduced = sample_mod_q(
        transcript,
        domains.reduction_prime,
        reduction.min,
        reduction.max,
    )?;
    let (rows_reduced, cols_reduced, claimed_reduced) = step50_reduce(
        &row_weights,
        &col_weights,
        &lift.mu_prime,
        reduced.modulus_u128(),
    );
    let _step5 = tracing::info_span!("step5:open_verify").entered();
    let opening_layout = spec.opening_layout();
    let derived_shape = Shape::new(opening_layout.row_vars, opening_layout.col_vars)
        .map_err(|error| ProtocolError::LigeritoConfig(format!("BitZ derived shape: {error:?}")))?;
    let committed = Shape::new(p.row_vars, p.col_vars)
        .map_err(|error| ProtocolError::LigeritoConfig(format!("BitZ shape: {error:?}")))?;
    let params = BitZParams::new(
        derived_shape,
        reduced.modulus_u128(),
        bitz_generator().into(),
    )
    .map_err(|error| ProtocolError::LigeritoConfig(format!("BitZ params: {error:?}")))?;
    let claim = LinearClaim::new(&params, rows_reduced, cols_reduced, claimed_reduced)
        .map_err(|error| ProtocolError::LigeritoConfig(format!("BitZ claim: {error:?}")))?;
    let statement = crate::wfbitz::VirtualStatement::new(params, committed, map, &claim)
        .map_err(|error| ProtocolError::LigeritoConfig(format!("BitZ statement: {error:?}")))?;
    let pcs = reduced_pcs(opener, &committed)?.with_native_policy(security.native_policy()?);
    let opening = proof.bitz();
    let ood = ood
        .opening_claim(transcript, packed_variables(&p)?, opening.ood.as_ref())
        .map_err(ProtocolError::Bitz)?;
    let tag = fork_tag(transcript);
    let bitz_proof = BitzTranscriptProof {
        narg_string: opening.narg.clone(),
        hints: opening.hints.clone(),
    };
    let mut state = build_verifier(REDUCED_SESSION, &tag, &bitz_proof);
    state.public_message(&verified.bridge_digest);
    BitZVerifier::new(params, WINDOW)
        .verify_virtual(
            &statement,
            &pcs,
            Root(commitment.root),
            state,
            ood.as_ref().map(|claim| (claim.point.as_slice(), claim.y)),
        )
        .map_err(|error| ProtocolError::LigeritoConfig(format!("BitZ verify: {error:?}")))
}

/// PCS configuration shared by one-sided explicit contexts and resolved policies.
pub(crate) fn pcs_from_config(
    shape: &Shape,
    config: &dyn crate::ligerito_flock::LigeritoStatementConfig,
) -> Result<BitzPcs, ProtocolError> {
    use flock_core::pcs::ligerito::{ProverConfig, VerifierConfig};
    macro_rules! copy_config {
        ($ty:ident) => {
            $ty {
                recursive_steps: config.recursive_steps(),
                initial_log_msg_cols: config.initial_log_msg_cols(),
                initial_log_num_interleaved: config.initial_log_num_interleaved(),
                initial_k: config.initial_k(),
                log_inv_rates: config.log_inv_rates().to_vec(),
                recursive_log_msg_cols: config.recursive_log_msg_cols().to_vec(),
                recursive_ks: config.recursive_ks().to_vec(),
                queries: config.queries().to_vec(),
                grinding_bits: config.grinding_bits().to_vec(),
                fold_grinding_bits: config.fold_grinding_bits().to_vec(),
                ood_samples: config.ood_samples().to_vec(),
                merkle_hash: config.merkle_hash(),
            }
        };
    }
    BitzPcs::from_configs(
        shape,
        copy_config!(ProverConfig),
        copy_config!(VerifierConfig),
    )
    .map_err(|error| ProtocolError::LigeritoConfig(format!("BitZ pcs: {error:?}")))
}

pub(crate) fn opening_shape(layout: &IntegerMatrixLayout) -> Result<Shape, ProtocolError> {
    if layout.word_bits != 1 {
        return Err(ProtocolError::InvalidBitzParameters);
    }
    Shape::new(layout.row_vars, layout.col_vars)
        .map_err(|error| ProtocolError::LigeritoConfig(format!("BitZ shape: {error:?}")))
}

pub(crate) fn opening_params(
    layout: &IntegerMatrixLayout,
    modulus: u128,
) -> Result<BitZParams, ProtocolError> {
    BitZParams::new(opening_shape(layout)?, modulus, bitz_generator().into())
        .map_err(|error| ProtocolError::LigeritoConfig(format!("BitZ params: {error:?}")))
}

// CM and linear SHA bind their relation and terminal claim before this fork.
const VIRTUAL_SESSION: &[u8] = b"bitz/wfbitz-opener/virtual/v1";

pub(crate) fn prove_virtual_opening<
    T: Transcript + Send,
    M: circuit::linear_map::binary::VirtualMap,
>(
    transcript: &mut T,
    statement: &crate::wfbitz::VirtualStatement<'_, M>,
    pcs: &BitzPcs,
    hint: &FlockCommitHint,
    derived_rows: &[Vec<u64>],
    ood: crate::ligerito_flock::ProverOod,
) -> Result<WfbitzOpeningProof, ProtocolError> {
    let ood = ood.opening_claim(transcript, hint);
    let mut state = build_prover(VIRTUAL_SESSION, &fork_tag(transcript));
    BitZProver::new(*statement.claim_params(), WINDOW)
        .prove_virtual(
            statement,
            pcs,
            hint,
            derived_rows,
            &mut state,
            ood.as_ref().map(|claim| (claim.point.as_slice(), claim.y)),
        )
        .map_err(|error| ProtocolError::LigeritoConfig(format!("BitZ prove: {error:?}")))?;
    let BitzTranscriptProof { narg_string, hints } = state.finish();
    Ok(WfbitzOpeningProof {
        narg: narg_string,
        hints,
        ood: ood.map(|claim| claim.round),
    })
}

pub(crate) fn verify_virtual_opening<
    T: Transcript + Send,
    M: circuit::linear_map::binary::VirtualMap,
>(
    transcript: &mut T,
    statement: &crate::wfbitz::VirtualStatement<'_, M>,
    pcs: &BitzPcs,
    commitment: &Commitment,
    proof: &WfbitzOpeningProof,
    ood: crate::ligerito_flock::VerifierOod,
) -> Result<(), ProtocolError> {
    let packed_vars = statement
        .committed_shape()
        .log_bits()
        .checked_sub(crate::ligerito::LOG_PACKING)
        .ok_or(ProtocolError::InvalidGeometry)?;
    let ood = ood
        .opening_claim(transcript, packed_vars, proof.ood.as_ref())
        .map_err(ProtocolError::Bitz)?;
    let streams = BitzTranscriptProof {
        narg_string: proof.narg.clone(),
        hints: proof.hints.clone(),
    };
    let state = build_verifier(VIRTUAL_SESSION, &fork_tag(transcript), &streams);
    BitZVerifier::new(*statement.claim_params(), WINDOW)
        .verify_virtual(
            statement,
            pcs,
            Root(commitment.root),
            state,
            ood.as_ref().map(|claim| (claim.point.as_slice(), claim.y)),
        )
        .map_err(|error| ProtocolError::LigeritoConfig(format!("BitZ verify: {error:?}")))
}
