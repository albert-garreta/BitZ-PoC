//! The BitZ polynomial commitment scheme as the opener of the crate's
//! relations: the same Spartan PIOP, prime draw, bitification and
//! statement binding as [`super::prove`], with the bitified functional
//! discharged by `worldfnd/BitZ`'s scheme ([`crate::bitz`]: integer column
//! folds in the exponent, a batched grand-product GKR, the dense reduction
//! sumcheck, ring switching and flock's Ligerito) instead of the crate's
//! chunked exponent-fold forest.
//!
//! The interface matches without translation. The bitified claim is a
//! rank-one functional over the committed bit tensor — dense row weights
//! (one factor per variable block times the slot weights times the
//! equality table of the high gate coordinates) times the equality table
//! of the low gate coordinates — and BitZ's `LinearClaim` is exactly that:
//! row weights, column weights, a target modulo `q`. The commitment is the
//! same flock commitment over the same per-column bit rows. BitZ's fold
//! bound (`(q − 1)(2^t + 1) < 2^127`) coincides with the direct-opening
//! prime cap the profile already enforces at word width one
//! (`q_bits ≤ 126 − t`), so the profile's primes need no change.
//!
//! BitZ runs its own transcript (spongefish with a hint channel). It is
//! forked from the outer transcript after the terminal grinding boundary:
//! the fork's instance tag is a 32-byte squeeze of the outer state (which
//! has the statement binding, the prime, the PIOP and the bridge digest
//! behind it), so every BitZ challenge depends on everything before it,
//! and the fork's narg string and hints are the opening proof. Nothing is
//! drawn from the outer transcript after the fork.
//!
//! Soundness of the opening: BitZ draws only `GF(2^128)` challenges (the
//! fold batching point, the GKR rounds, the reduction sumcheck, the ring
//! switch) — every term sits at the field floor — plus flock's Ligerito at
//! the selected ladder; there is no mod-`q` draw and nothing to grind
//! outside Ligerito. Ligerito's grinding is the ladder's ([`WfbitzLigerito`]):
//! BitZ's embedded `fast` ladder is a 100-bit Johnson ladder with query
//! and fold grinding as shipped; the crate's validated unique-decoding
//! ladders carry fold grinding to the profile's target without an early
//! Round 0 (BitZ has none; a Johnson ladder without Round 0 is what BitZ
//! ships and is reported as such).

use flock_core::merkle::HashKind;
use flock_core::pcs::ligerito::{LigeritoProfile, LigeritoSecurityConfig};

use crate::{
    wfbitz::{
        BitZParams, BitZProver, BitZVerifier, LinearClaim, Root, Shape, WINDOW,
        pcs::Pcs as BitzPcs,
        transcript::{Proof as BitzTranscriptProof, build_prover, build_verifier},
    },
    ligerito_flock::{FlockCommitHint, LigeritoSelection, OodRound},
    pcs::IntegerMatrixLayout,
    transcript::traits::Transcript,
};
use flock_core::pcs::commit::Commitment;

use super::{
    OpeningProof, Opener, PreparedRelationPrefix, Proof, ProtocolError, RelationSpec,
    bind_prover_statement, bind_verifier_statement, bitify, bitz_generator, packed_variables,
    prove_piop, validate_bit_rows, verify_piop,
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
    /// (`udr:<r>:<k>` validated unique decoding with fold grinding; a
    /// Johnson `custom:<r>:<k>` ladder runs but without the crate's early
    /// Round 0).
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
        let (pcs, security, digest) = match ligerito {
            WfbitzLigerito::Fast => {
                let pcs = BitzPcs::new(&shape, HashKind::Blake3)
                    .map_err(|error| ProtocolError::LigeritoConfig(format!("BitZ pcs: {error:?}")))?;
                let source = flock_core::pcs::ligerito::embedded_security_config(
                    shape.log_bits(),
                    LigeritoProfile::Fast,
                )
                .ok_or_else(|| ProtocolError::LigeritoConfig("no embedded fast ladder".into()))?;
                let mut security = LigeritoSecurityConfig::from_toml_str(source)
                    .map_err(ProtocolError::LigeritoConfig)?;
                security.hash = "blake3".to_owned();
                let digest = *blake3::hash(source.as_bytes()).as_bytes();
                (pcs, security, digest)
            }
            WfbitzLigerito::Selected(selection) => {
                let resolved = selection
                    .resolve(packed_variables(&layout)?, target_bits)
                    .map_err(ProtocolError::LigeritoConfig)?;
                let pcs = BitzPcs::with_security(&shape, resolved.security(), LigeritoProfile::Fast)
                    .map_err(|error| ProtocolError::LigeritoConfig(format!("BitZ pcs: {error:?}")))?;
                (pcs, resolved.security().clone(), resolved.digest())
            }
        };
        Ok(Self {
            layout,
            shape,
            pcs,
            ligerito,
            security,
            digest,
        })
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

    /// The opener configuration the statement binding covers: the ladder's
    /// prover and verifier configurations (no policy digest of the crate's
    /// kind, no Round 0).
    pub fn opener(&self) -> Opener {
        Opener::Custom {
            prover: Some(self.pcs.prover_config().clone()),
            verifier: Some(self.pcs.verifier_config().clone()),
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
        Ok(hint)
    }

    /// The parameters under the runtime prime (their gate: the fold bound).
    fn params(&self, q: u128) -> Result<BitZParams, ProtocolError> {
        BitZParams::new(self.shape, q, bitz_generator())
            .map_err(|error| ProtocolError::LigeritoConfig(format!("BitZ parameters: {error:?}")))
    }

    /// The round-by-round accounting of the opening: the ladder's weakest
    /// level (its paper-predicted proximity-gap and query terms with their
    /// grinding) and the `GF(2^128)` floor of every other draw.
    pub fn opening_bits(&self) -> (f64, &'static str) {
        let mut weakest = (f64::INFINITY, "ligerito");
        for level in &self.security.levels {
            let (pg, query) = level.paper_predicted_bits();
            let pg = pg + level.fold_grinding_bits as f64;
            let query = query + level.grinding_bits as f64;
            if pg < weakest.0 {
                weakest = (pg, "ligerito proximity gap");
            }
            if query < weakest.0 {
                weakest = (query, "ligerito queries");
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

/// The BitZ opening: the forked transcript's narg string and hints.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WfbitzOpeningProof {
    pub narg: Vec<u8>,
    pub hints: Vec<u8>,
}

impl OpeningProof for WfbitzOpeningProof {
    /// Both streams, each with a `u64` length.
    fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(16 + self.narg.len() + self.hints.len());
        for stream in [&self.narg, &self.hints] {
            bytes.extend_from_slice(&(stream.len() as u64).to_le_bytes());
            bytes.extend_from_slice(stream);
        }
        bytes
    }

    fn grinding_nonces(&self) -> &[u64] {
        &[]
    }

    fn ood(&self) -> Option<&OodRound> {
        None
    }
}

impl WfbitzOpeningProof {
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        let mut at = 0;
        let mut read = || -> Option<Vec<u8>> {
            let len = u64::from_le_bytes(bytes.get(at..at + 8)?.try_into().ok()?) as usize;
            at += 8;
            let stream = bytes.get(at..at + len)?.to_vec();
            at += len;
            Some(stream)
        };
        let narg = read()?;
        let hints = read()?;
        (at == bytes.len()).then_some(Self { narg, hints })
    }
}

/// A proof discharged through the BitZ opener.
pub type WfbitzProof = Proof<WfbitzOpeningProof>;

/// The fork's instance tag: 32 bytes squeezed from the outer transcript
/// after the terminal boundary.
fn fork_tag<T: Transcript>(transcript: &mut T) -> [u8; 32] {
    let low: u128 = transcript.get_challenge();
    let high: u128 = transcript.get_challenge();
    let mut tag = [0u8; 32];
    tag[..16].copy_from_slice(&low.to_le_bytes());
    tag[16..].copy_from_slice(&high.to_le_bytes());
    tag
}

/// The modulus of the runtime prime context as a word.
fn modulus_u128(prime: &field::FpCtx<2>) -> u128 {
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

/// Proves the relation, the bitified claim discharged through BitZ.
pub fn prove<T: Transcript + Send, S: RelationSpec>(
    transcript: &mut T,
    prefix: &PreparedRelationPrefix<S>,
    opener: &WfbitzOpener,
    witness: &S::Witness,
    hint: &FlockCommitHint,
) -> Result<WfbitzProof, ProtocolError> {
    let spec = prefix.layout();
    spec.check_witness(witness)?;
    if prefix.params() != *opener.layout() {
        return Err(ProtocolError::RelationWitnessLayoutMismatch);
    }
    validate_bit_rows(opener.layout(), hint.rows())?;
    let configuration = opener.opener();
    let (binding, _ood) = bind_prover_statement(transcript, prefix, &configuration, hint)?;
    transcript.absorb_slice(SESSION);
    transcript.absorb_slice(&opener.digest());
    let proved = prove_piop(transcript, prefix, witness, &binding)?;
    let prime = &proved.prime;

    let _step5 = tracing::info_span!("step5:open_prove").entered();
    let params = opener.params(modulus_u128(prime))?;
    let claim = {
        let _scope = tracing::info_span!("step5:bitz-claim").entered();
        linear_claim(&params, &proved.opening, &proved.table, prime)?
    };
    let tag = fork_tag(transcript);
    let mut state = build_prover(SESSION, &tag);
    state.public_message(&proved.bridge_digest);
    BitZProver::new(params, WINDOW)
        .prove(&claim, opener.pcs(), hint, &mut state)
        .map_err(|error| ProtocolError::LigeritoConfig(format!("BitZ prove: {error:?}")))?;
    let BitzTranscriptProof { narg_string, hints } = state.finish();
    Ok(Proof::from_parts(
        proved.messages,
        None,
        WfbitzOpeningProof {
            narg: narg_string,
            hints,
        },
    ))
}

/// Verifies a BitZ-discharged proof.
pub fn verify<T: Transcript + Send, S: RelationSpec>(
    transcript: &mut T,
    prefix: &PreparedRelationPrefix<S>,
    opener: &WfbitzOpener,
    commitment: &Commitment,
    proof: &WfbitzProof,
) -> Result<(), ProtocolError> {
    if prefix.params() != *opener.layout() {
        return Err(ProtocolError::RelationWitnessLayoutMismatch);
    }
    let configuration = opener.opener();
    let (binding, _ood) =
        bind_verifier_statement(transcript, prefix, &configuration, commitment, None)?;
    transcript.absorb_slice(SESSION);
    transcript.absorb_slice(&opener.digest());
    let verified = verify_piop(transcript, prefix, &binding, proof.prefix())?;
    let prime = &verified.prime;

    let _step5 = tracing::info_span!("step5:open_verify").entered();
    let params = opener.params(modulus_u128(prime))?;
    let claim = linear_claim(&params, &verified.opening, &verified.table, prime)?;
    let tag = fork_tag(transcript);
    let opening = proof.bitz();
    let bitz_proof = BitzTranscriptProof {
        narg_string: opening.narg.clone(),
        hints: opening.hints.clone(),
    };
    let mut state = build_verifier(SESSION, &tag, &bitz_proof);
    state.public_message(&verified.bridge_digest);
    BitZVerifier::new(params, WINDOW)
        .verify(&claim, opener.pcs(), Root(commitment.root), state)
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
        let prefix =
            PreparedRelationPrefix::new::<Lambda100>(MulLayout::<u32>::new(multiplications).unwrap()).unwrap();
        for ladder in [WfbitzLigerito::Fast, WfbitzLigerito::Selected(LigeritoSelection::MATCHED_UDR)] {
            let opener = WfbitzOpener::new(prefix.params(), ladder, 100).unwrap();
            let hint = opener.commit(witness.bitz_bit_rows()).unwrap();
            let proof = prove(
                &mut Blake3Transcript::new(),
                &prefix,
                &opener,
                &witness,
                &hint,
            )
            .unwrap();
            verify(&mut Blake3Transcript::new(), &prefix, &opener, &hint.commitment, &proof).unwrap();
            let bytes = proof.bitz().to_bytes();
            assert_eq!(WfbitzOpeningProof::from_bytes(&bytes).as_ref(), Some(proof.bitz()));
            let (bits, term) = opener.opening_bits();
            assert!(bits >= 100.0, "{ladder:?}: {bits} bits ({term})");

            let mut tampered = proof.clone();
            tampered.bitz_mut().narg[7] ^= 1;
            assert!(verify(&mut Blake3Transcript::new(), &prefix, &opener, &hint.commitment, &tampered).is_err());
            let mut tampered = proof.clone();
            let last = tampered.bitz().hints.len() - 1;
            tampered.bitz_mut().hints[last] ^= 1;
            assert!(verify(&mut Blake3Transcript::new(), &prefix, &opener, &hint.commitment, &tampered).is_err());
        }
    }

    /// A proof under one ladder does not verify under the other (the
    /// statement binding covers the ladder).
    #[test]
    fn ladders_are_bound() {
        let multiplications = 1 << 15;
        let witness = u64_witness(multiplications);
        let prefix =
            PreparedRelationPrefix::new::<Lambda100>(MulLayout::<u64>::new(multiplications).unwrap()).unwrap();
        let fast = WfbitzOpener::new(prefix.params(), WfbitzLigerito::Fast, 100).unwrap();
        let udr = WfbitzOpener::new(
            prefix.params(),
            WfbitzLigerito::Selected(LigeritoSelection::MATCHED_UDR),
            100,
        )
        .unwrap();
        let hint = fast.commit(witness.bitz_bit_rows()).unwrap();
        let proof = prove(&mut Blake3Transcript::new(), &prefix, &fast, &witness, &hint).unwrap();
        verify(&mut Blake3Transcript::new(), &prefix, &fast, &hint.commitment, &proof).unwrap();
        assert!(verify(&mut Blake3Transcript::new(), &prefix, &udr, &hint.commitment, &proof).is_err());
    }
}
