//! One grinding boundary per uninterrupted block of transcript challenges.
//!
//! Vector coordinates share a nonce. Any observation ends the current block.
//! Each caller supplies its own protocol domain and accounts for the *total*
//! failure probability of all draws within a block when choosing difficulty.

use core::marker::PhantomData;

use crate::piop::spartan::grinding::{
    GrindingDomain, GrindingError, GrindingRound, grind_and_absorb, verify_and_absorb,
};
use crate::transcript::traits::{ConstTranscribable, Transcript};

pub(crate) struct ProverBlockGrindingTranscript<'a, T, D> {
    inner: &'a mut T,
    bits: u32,
    active: bool,
    blocks: usize,
    nonces: Vec<u64>,
    domain: PhantomData<D>,
}

impl<'a, T: Transcript, D: GrindingDomain> ProverBlockGrindingTranscript<'a, T, D> {
    pub(crate) fn new(inner: &'a mut T, bits: u32) -> Self {
        assert!(bits <= 256 && !D::DOMAIN.is_empty());
        Self {
            inner,
            bits,
            active: false,
            blocks: 0,
            nonces: Vec::new(),
            domain: PhantomData,
        }
    }

    fn begin_block(&mut self) {
        if !self.active {
            if self.bits != 0 {
                self.nonces.push(
                    grind_and_absorb::<D, _>(
                        self.inner,
                        GrindingRound::new(self.blocks as u64),
                        self.bits,
                    )
                    .expect("validated block grinding difficulty"),
                );
            }
            self.blocks += 1;
            self.active = true;
        }
    }

    pub(crate) fn block_count(&self) -> usize {
        self.blocks
    }

    pub(crate) fn finish(self) -> Vec<u64> {
        self.nonces
    }
}

impl<T: Transcript, D: GrindingDomain> Transcript for ProverBlockGrindingTranscript<'_, T, D> {
    fn fill_sampling_bytes(&mut self, output: &mut [u8]) {
        self.inner.fill_sampling_bytes(output);
    }

    fn begin_sampling(&mut self) {
        self.begin_block();
        self.inner.begin_sampling();
    }

    fn get_challenge<C: ConstTranscribable>(&mut self) -> C {
        self.begin_block();
        self.inner.get_challenge()
    }

    fn absorb_inner(&mut self, value: &[u8]) {
        self.active = false;
        self.inner.absorb_inner(value);
    }
}

pub(crate) struct VerifierBlockGrindingTranscript<'a, 'n, T, D> {
    inner: &'a mut T,
    bits: u32,
    active: bool,
    blocks: usize,
    nonces: &'n [u64],
    consumed: usize,
    failure: Option<GrindingError>,
    domain: PhantomData<D>,
}

impl<'a, 'n, T: Transcript, D: GrindingDomain> VerifierBlockGrindingTranscript<'a, 'n, T, D> {
    pub(crate) fn new(inner: &'a mut T, bits: u32, nonces: &'n [u64]) -> Self {
        assert!(bits <= 256 && !D::DOMAIN.is_empty());
        Self {
            inner,
            bits,
            active: false,
            blocks: 0,
            nonces,
            consumed: 0,
            failure: None,
            domain: PhantomData,
        }
    }

    fn begin_block(&mut self) {
        if !self.active {
            if self.bits != 0 {
                let nonce = self.nonces.get(self.consumed).copied().unwrap_or(0);
                if self.consumed >= self.nonces.len() {
                    self.failure.get_or_insert(GrindingError::InvalidNonce {
                        nonce,
                        bits: self.bits,
                    });
                }
                self.consumed += 1;
                if let Err(error) = verify_and_absorb::<D, _>(
                    self.inner,
                    GrindingRound::new(self.blocks as u64),
                    self.bits,
                    nonce,
                ) {
                    self.failure.get_or_insert(error);
                }
            }
            self.blocks += 1;
            self.active = true;
        }
    }

    pub(crate) fn block_count(&self) -> usize {
        self.blocks
    }

    #[must_use = "all grinding nonces must be verified before accepting a proof"]
    pub(crate) fn finish(self) -> Result<(), GrindingError> {
        if let Some(error) = self.failure {
            return Err(error);
        }
        if self.consumed != self.nonces.len() {
            return Err(GrindingError::InvalidNonce {
                nonce: self.nonces.get(self.consumed).copied().unwrap_or(0),
                bits: self.bits,
            });
        }
        Ok(())
    }
}

impl<T: Transcript, D: GrindingDomain> Transcript
    for VerifierBlockGrindingTranscript<'_, '_, T, D>
{
    fn fill_sampling_bytes(&mut self, output: &mut [u8]) {
        self.inner.fill_sampling_bytes(output);
    }

    fn begin_sampling(&mut self) {
        self.begin_block();
        self.inner.begin_sampling();
    }

    fn get_challenge<C: ConstTranscribable>(&mut self) -> C {
        self.begin_block();
        self.inner.get_challenge()
    }

    fn absorb_inner(&mut self, value: &[u8]) {
        self.active = false;
        self.inner.absorb_inner(value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transcript::Blake3Transcript;

    enum TestDomain {}
    impl GrindingDomain for TestDomain {
        const DOMAIN: &'static [u8] = b"falcon-hybrid/block-test/v1";
    }

    fn draw<T: Transcript>(t: &mut T) -> Vec<u128> {
        t.absorb_slice(b"first block");
        let mut out = (0..7).map(|_| t.get_challenge()).collect::<Vec<u128>>();
        t.absorb_slice(b"second block");
        out.push(t.get_challenge());
        out
    }

    #[test]
    fn vectors_share_nonce_and_replay_continuation() {
        let mut pt = Blake3Transcript::new();
        let mut p = ProverBlockGrindingTranscript::<_, TestDomain>::new(&mut pt, 3);
        let expected = draw(&mut p);
        assert_eq!(p.block_count(), 2);
        let nonces = p.finish();
        assert_eq!(nonces.len(), 2);
        let mut vt = Blake3Transcript::new();
        let mut v = VerifierBlockGrindingTranscript::<_, TestDomain>::new(&mut vt, 3, &nonces);
        assert_eq!(draw(&mut v), expected);
        assert_eq!(v.block_count(), 2);
        v.finish().unwrap();
        assert_eq!(pt.get_challenge::<u128>(), vt.get_challenge::<u128>());

        for malformed in [nonces[..1].to_vec(), [nonces.as_slice(), &[0]].concat()] {
            let mut t = Blake3Transcript::new();
            let mut v =
                VerifierBlockGrindingTranscript::<_, TestDomain>::new(&mut t, 3, &malformed);
            draw(&mut v);
            assert!(v.finish().is_err());
        }
    }

    #[test]
    fn zero_difficulty_is_transparent_and_rejects_extra_nonces() {
        let mut plain = Blake3Transcript::new();
        let expected = draw(&mut plain);
        let mut pt = Blake3Transcript::new();
        let mut p = ProverBlockGrindingTranscript::<_, TestDomain>::new(&mut pt, 0);
        assert_eq!(draw(&mut p), expected);
        assert!(p.finish().is_empty());
        assert_eq!(plain.get_challenge::<u128>(), pt.get_challenge::<u128>());
        let mut vt = Blake3Transcript::new();
        let mut v = VerifierBlockGrindingTranscript::<_, TestDomain>::new(&mut vt, 0, &[0]);
        draw(&mut v);
        assert!(v.finish().is_err());
    }
}
