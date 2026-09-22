//! A Binius prefix channel on the composition's single Fiat–Shamir transcript.
use crate::transcript::{Blake3Transcript, impl_plain_binius_ip_channel, traits::Transcript};
use binius_compute::Allocator;
use binius_field::PackedField;
use binius_iop::channel::{Error as IopError, IOPVerifierChannel, OracleSpec};
use binius_iop_prover::channel::IOPProverChannel;
use binius_ip::channel::Error as IpError;
use binius_math::{FieldSlice, FieldVec};
use binius_verifier::config::B128;

pub(super) struct ProverChannel<'a> {
    pub transcript: &'a mut Blake3Transcript,
    pub messages: Vec<u128>,
    pub spec: Vec<OracleSpec>,
}

impl_plain_binius_ip_channel!(prover for ProverChannel<'_>);

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
            .absorb_slice(b"hybrid/sha-precommitted-witness/v2");
    }
    fn prove_oracle_relation(&mut self, _: (), _: FieldVec<P, A>, _: B128) {
        panic!("the SHA prefix must stop before ring switching");
    }
    fn finalize_oracle(&mut self, _: (), _: FieldVec<P, A>) {
        panic!("the composition owns the precommitted SHA witness");
    }
}

pub(super) struct VerifierChannel<'a> {
    pub transcript: &'a mut Blake3Transcript,
    pub messages: &'a [u128],
    pub spec: Vec<OracleSpec>,
}

impl_plain_binius_ip_channel!(verifier for VerifierChannel<'_>);

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
            .absorb_slice(b"hybrid/sha-precommitted-witness/v2");
        Ok(())
    }
    fn verify_oracle_relation(
        &mut self,
        _: (),
        _: Box<dyn Fn(&[B128]) -> B128>,
        _: B128,
    ) -> Result<(), IopError> {
        Err(IpError::InvalidAssert.into())
    }
}
