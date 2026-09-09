//! A Binius prefix channel on the composition's single Fiat–Shamir transcript.
use super::Gf;
use crate::transcript::{Blake3Transcript, traits::Transcript};
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

pub(super) struct ProverChannel<'a> {
    pub transcript: &'a mut Blake3Transcript,
    pub messages: Vec<u128>,
    pub spec: Vec<OracleSpec>,
}

fn observe(t: &mut Blake3Transcript, v: B128) {
    t.absorb_slice(&u128::from(v.val()).to_le_bytes());
}

fn challenge(t: &mut Blake3Transcript) -> B128 {
    let x: Gf = t.get_field_challenge(&());
    B128::new(u128::from(x.words()[0]) | (u128::from(x.words()[1]) << 64))
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
    type Oracle = ();
    fn remaining_oracle_specs(&self) -> &[OracleSpec] {
        &self.spec
    }
    fn send_oracle(&mut self, buffer: FieldSlice<P>) {
        assert_eq!(self.spec, [OracleSpec::new(buffer.log_len())]);
        self.spec.clear();
        // The composition already committed this witness and bound its root.
        self.transcript
            .absorb_slice(b"hybrid/sha-precommitted-witness/v1");
    }
    fn prove_oracle_relations(
        &mut self,
        _: impl IntoIterator<Item = ((), FieldVec<P, A>, FieldVec<P, A>, B128)>,
    ) {
        panic!("the SHA prefix must stop before ring switching");
    }
}

pub(super) struct VerifierChannel<'a> {
    pub transcript: &'a mut Blake3Transcript,
    pub messages: &'a [u128],
    pub spec: Vec<OracleSpec>,
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
    type Oracle = ();
    fn remaining_oracle_specs(&self) -> &[OracleSpec] {
        &self.spec
    }
    fn recv_oracle(&mut self, log_msg_len: usize, dependent: bool) -> Result<(), IopError> {
        if !dependent || self.spec != [OracleSpec::new(log_msg_len)] {
            return Err(IpError::InvalidAssert.into());
        }
        self.spec.clear();
        self.transcript
            .absorb_slice(b"hybrid/sha-precommitted-witness/v1");
        Ok(())
    }
    fn verify_oracle_relations(
        &mut self,
        _: impl IntoIterator<Item = OracleLinearRelation<(), B128>>,
    ) -> Result<(), IopError> {
        Err(IpError::InvalidAssert.into())
    }
}
