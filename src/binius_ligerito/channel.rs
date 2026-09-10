//! Binius64's IOP channel on the composition's single BLAKE3 transcript.
//!
//! Every oracle the PIOP commits — the packed witness, and the IntMul
//! reduction's logup* pushforward when the circuit multiplies — is committed
//! by [`BinaryPcs`] (rate 1/8, BLAKE3 Merkle tree), its root bound into the
//! transcript and Round 0 taken immediately, before the next challenge. Oracle
//! linear relations are queued, not opened: the adapter discharges each
//! oracle's relations with one Ligerito opening after the PIOP prefix ends,
//! exactly as Binius64's own BaseFold channel defers its openings to
//! `finish()`.
use super::{Oracle, b128_to_f128};
use crate::{
    binary_pcs::{BinaryPcs, Round0Prover, Round0Verifier},
    ligerito_flock::OodRound,
    poly::univariate::binary_gf128::BinaryFieldGF128 as Gf,
    transcript::{Blake3Transcript, traits::Transcript},
};
use binius_compute::Allocator;
use binius_field::{Field, PackedField, util::FieldFn};
use binius_iop::channel::{
    Error as IopError, IOPVerifierChannel, OracleLinearRelation, OracleSpec,
};
use binius_iop_prover::channel::IOPProverChannel;
use binius_ip::channel::{Error as IpError, IPVerifierChannel};
use binius_ip_prover::channel::IPProverChannel;
use binius_math::{FieldSlice, FieldVec};
use binius_verifier::config::B128;
use flock_core::{field::F128, merkle::Hash, pcs::commit::ProverData};
use std::time::{Duration, Instant};

const ORACLE_DOMAIN: &[u8] = b"f2z/binius64-ligerito/oracle/v1";

/// Bind an oracle's root, in commitment order, before Round 0 draws `ζ`.
fn absorb_root(t: &mut Blake3Transcript, index: usize, root: &Hash) {
    t.absorb_slice(ORACLE_DOMAIN);
    t.absorb_slice(&(index as u64).to_le_bytes());
    t.absorb_slice(root);
}

fn observe(t: &mut Blake3Transcript, v: B128) {
    t.absorb_slice(&u128::from(v.val()).to_le_bytes());
}

fn challenge(t: &mut Blake3Transcript) -> B128 {
    let x: Gf = t.get_field_challenge(&());
    B128::new(u128::from(x.words()[0]) | (u128::from(x.words()[1]) << 64))
}

/// One committed oracle on the prover side.
pub(super) struct ProverOracle {
    pub packed: Vec<F128>,
    pub data: ProverData,
    pub root: Hash,
    pub round0: Round0Prover,
}

/// One queued relation `⟨basis, oracle⟩ = claim`.
pub(super) struct ProverRelation {
    pub oracle: Oracle,
    pub basis: Vec<F128>,
    pub claim: B128,
}

pub(super) struct ProverChannel<'a> {
    pub transcript: &'a mut Blake3Transcript,
    pub messages: Vec<u128>,
    /// Oracles still to be committed, in order.
    pub specs: Vec<OracleSpec>,
    /// One solved opener per expected oracle, in commitment order.
    pub pcs: &'a [BinaryPcs],
    pub oracles: Vec<ProverOracle>,
    pub relations: Vec<ProverRelation>,
    /// The witness commitment (oracle 0): encoding + Merkle tree.
    pub commit_time: Duration,
    /// Commitments the PIOP takes mid-protocol (later oracles).
    pub extra_commit_time: Duration,
    /// Every oracle's Round 0.
    pub round0_time: Duration,
}

impl IPProverChannel<B128> for ProverChannel<'_> {
    fn send_one(&mut self, elem: B128) {
        self.messages.push(elem.val().into());
        observe(self.transcript, elem);
    }
    fn observe_one(&mut self, elem: B128) {
        observe(self.transcript, elem);
    }
    fn sample(&mut self) -> B128 {
        challenge(self.transcript)
    }
}

impl<P: PackedField<Scalar = B128>, A: Allocator> IOPProverChannel<P, A> for ProverChannel<'_> {
    type Oracle = Oracle;
    fn remaining_oracle_specs(&self) -> &[OracleSpec] {
        &self.specs
    }
    fn send_oracle(&mut self, buffer: FieldSlice<P>) -> Oracle {
        let index = self.oracles.len();
        let expected = OracleSpec::new(buffer.log_len());
        assert!(
            self.specs.first() == Some(&expected),
            "oracle {index} has shape {expected:?}, expected {:?}",
            self.specs.first()
        );
        self.specs.remove(0);
        let pcs = &self.pcs[index];
        let started = Instant::now();
        let packed: Vec<F128> = buffer.iter_scalars().map(b128_to_f128).collect();
        let (commitment, data) = pcs
            .commit(&packed)
            .expect("the oracle length matches its specification");
        absorb_root(self.transcript, index, &commitment.root);
        let committed = Instant::now();
        let round0 = pcs.prove_round0(self.transcript, &packed);
        let pinned = Instant::now();
        if index == 0 {
            self.commit_time += committed - started;
        } else {
            self.extra_commit_time += committed - started;
        }
        self.round0_time += pinned - committed;
        self.oracles.push(ProverOracle {
            packed,
            data,
            root: commitment.root,
            round0,
        });
        index
    }
    fn prove_oracle_relations(
        &mut self,
        relations: impl IntoIterator<Item = (Oracle, FieldVec<P, A>, FieldVec<P, A>, B128)>,
    ) {
        for (oracle, message, transparent, claim) in relations {
            let packed = &self.oracles[oracle].packed;
            assert_eq!(
                message.to_ref().log_len(),
                packed.len().trailing_zeros() as usize,
                "relation message must be the committed oracle"
            );
            let basis: Vec<F128> = transparent.iter_scalars().map(b128_to_f128).collect();
            assert_eq!(basis.len(), packed.len(), "transparent basis covers the oracle");
            self.relations.push(ProverRelation {
                oracle,
                basis,
                claim,
            });
        }
    }
}

/// One received oracle on the verifier side.
pub(super) struct VerifierOracle {
    pub root: Hash,
    pub round0: Round0Verifier,
}

pub(super) struct VerifierChannel<'a> {
    pub transcript: &'a mut Blake3Transcript,
    pub messages: &'a [u128],
    pub specs: Vec<OracleSpec>,
    pub pcs: &'a [BinaryPcs],
    /// The proof's oracle roots and Round-0 messages, in commitment order.
    pub roots: &'a [Hash],
    pub rounds0: &'a [OodRound],
    pub oracles: Vec<VerifierOracle>,
    pub relations: Vec<OracleLinearRelation<Oracle, B128>>,
}

impl IPVerifierChannel<B128> for VerifierChannel<'_> {
    type Elem = B128;
    fn recv_one(&mut self) -> Result<B128, IpError> {
        let (&head, tail) = self.messages.split_first().ok_or(IpError::ProofEmpty)?;
        self.messages = tail;
        let elem = B128::new(head);
        observe(self.transcript, elem);
        Ok(elem)
    }
    fn sample(&mut self) -> B128 {
        challenge(self.transcript)
    }
    fn observe_one(&mut self, elem: B128) -> B128 {
        observe(self.transcript, elem);
        elem
    }
    fn assert_zero(&mut self, elem: B128) -> Result<(), IpError> {
        if elem == B128::ZERO {
            Ok(())
        } else {
            Err(IpError::InvalidAssert)
        }
    }
    fn compute_public_value(&mut self, inputs: &[B128], f: impl FieldFn<B128>) -> B128 {
        f.call_native(inputs)
    }
}

impl IOPVerifierChannel<B128> for VerifierChannel<'_> {
    type Oracle = Oracle;
    fn remaining_oracle_specs(&self) -> &[OracleSpec] {
        &self.specs
    }
    fn recv_oracle(&mut self, log_msg_len: usize, dependent: bool) -> Result<Oracle, IopError> {
        let index = self.oracles.len();
        if !dependent
            || self.specs.first() != Some(&OracleSpec::new(log_msg_len))
            || index >= self.pcs.len()
        {
            return Err(IpError::InvalidAssert.into());
        }
        let (Some(root), Some(round)) = (self.roots.get(index), self.rounds0.get(index)) else {
            return Err(IopError::ProofEmpty);
        };
        self.specs.remove(0);
        absorb_root(self.transcript, index, root);
        let round0 = self.pcs[index]
            .verify_round0(self.transcript, round)
            .map_err(|_| IopError::from(IpError::InvalidAssert))?;
        self.oracles.push(VerifierOracle {
            root: *root,
            round0,
        });
        Ok(index)
    }
    fn verify_oracle_relations(
        &mut self,
        relations: impl IntoIterator<Item = OracleLinearRelation<Oracle, B128>>,
    ) -> Result<(), IopError> {
        self.relations.extend(relations);
        Ok(())
    }
}
