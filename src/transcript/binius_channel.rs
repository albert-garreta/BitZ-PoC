//! The oracle-agnostic half of every `binius_ip`/`binius_iop` channel this
//! crate bridges onto a [`Blake3Transcript`]: send/observe/sample one field
//! element, and the bit/word primitives (`WordIP*Channel`). Every backend
//! that adapts this crate's transcript to Binius's channel traits needs
//! this piece verbatim, so it lives once here instead of once per backend.
//!
//! What's deliberately NOT here: `IOPProverChannel`/`IOPVerifierChannel`
//! (oracle commitment and relation handling). Those differ per backend —
//! one backend's oracle is precommitted elsewhere and its handlers just
//! refuse to be called; another's really commits through a PCS and queues
//! relations for a later opening — and each needs its own extra state
//! (`pcs`, `oracles`, `relations`, ...) that the other doesn't have. That
//! part stays hand-written in each backend's own `channel.rs`.

use crate::{
    poly::univariate::binary_gf128::Gf128,
    transcript::{Blake3Transcript, traits::Transcript},
};
use binius_core::word::Word;
use binius_verifier::config::B128;

pub(crate) fn observe(t: &mut Blake3Transcript, v: B128) {
    t.absorb_slice(&u128::from(v).to_le_bytes());
}

pub(crate) fn challenge(t: &mut Blake3Transcript) -> B128 {
    let x: Gf128 = t.get_field_challenge(&());
    B128::new(u128::from(x.as_words()[0]) | (u128::from(x.as_words()[1]) << 64))
}

pub(crate) fn observe_words(t: &mut Blake3Transcript, words: &[Word]) {
    for word in words {
        t.absorb_slice(&word.0.to_le_bytes());
    }
}

pub(crate) fn sample_bits(t: &mut Blake3Transcript, bits: usize) -> Word {
    assert!(bits <= Word::BITS);
    let value = u128::from(challenge(t)) as u64;
    Word(
        value
            & (u64::MAX
                .checked_shr((Word::BITS - bits) as u32)
                .unwrap_or(0)),
    )
}

// Generates the plain `IP*Channel`/`WordIP*Channel` impls above for one
// backend's `ProverChannel`/`VerifierChannel`, assuming only that `$ty` has
// `transcript: &mut Blake3Transcript` and `messages: Vec<u128>` (prover) or
// `&[u128]` (verifier) fields -- true of both backends today.
#[macro_export]
macro_rules! impl_plain_binius_ip_channel {
    (prover for $ty:ty) => {
        impl binius_ip_prover::channel::IPProverChannel<binius_verifier::config::B128> for $ty {
            fn send_one(&mut self, elem: binius_verifier::config::B128) {
                self.messages.push(u128::from(elem));
                $crate::transcript::binius_channel::observe(self.transcript, elem);
            }
            fn observe_one(&mut self, elem: binius_verifier::config::B128) {
                $crate::transcript::binius_channel::observe(self.transcript, elem);
            }
            fn sample(&mut self) -> binius_verifier::config::B128 {
                $crate::transcript::binius_channel::challenge(self.transcript)
            }
        }
        impl binius_ip_prover::channel::WordIPProverChannel<binius_verifier::config::B128>
            for $ty
        {
            type Word = binius_core::word::Word;
            fn observe_words(&mut self, words: &[binius_core::word::Word]) {
                $crate::transcript::binius_channel::observe_words(self.transcript, words);
            }
            fn sample_bits(&mut self, bits: usize) -> binius_core::word::Word {
                $crate::transcript::binius_channel::sample_bits(self.transcript, bits)
            }
        }
    };
    (verifier for $ty:ty) => {
        impl binius_ip::channel::IPVerifierChannel<binius_verifier::config::B128> for $ty {
            type Elem = binius_verifier::config::B128;
            fn recv_one(&mut self) -> Result<binius_verifier::config::B128, binius_ip::channel::Error> {
                let (&head, tail) = self
                    .messages
                    .split_first()
                    .ok_or(binius_ip::channel::Error::ProofEmpty)?;
                self.messages = tail;
                let elem = binius_verifier::config::B128::new(head);
                $crate::transcript::binius_channel::observe(self.transcript, elem);
                Ok(elem)
            }
            fn sample(&mut self) -> binius_verifier::config::B128 {
                $crate::transcript::binius_channel::challenge(self.transcript)
            }
            fn observe_one(
                &mut self,
                elem: binius_verifier::config::B128,
            ) -> binius_verifier::config::B128 {
                $crate::transcript::binius_channel::observe(self.transcript, elem);
                elem
            }
            fn assert_zero(
                &mut self,
                elem: binius_verifier::config::B128,
            ) -> Result<(), binius_ip::channel::Error> {
                if elem == <binius_verifier::config::B128 as binius_field::Field>::ZERO {
                    Ok(())
                } else {
                    Err(binius_ip::channel::Error::InvalidAssert)
                }
            }
        }
        impl binius_ip::channel::WordIPVerifierChannel<binius_verifier::config::B128> for $ty {
            type Word = binius_core::word::Word;
            fn observe_words(
                &mut self,
                words: &[binius_core::word::Word],
            ) -> Vec<binius_core::word::Word> {
                $crate::transcript::binius_channel::observe_words(self.transcript, words);
                words.to_vec()
            }
            fn sample_bits(&mut self, bits: usize) -> binius_core::word::Word {
                $crate::transcript::binius_channel::sample_bits(self.transcript, bits)
            }
            fn subset_sum(
                &mut self,
                elems: &[binius_verifier::config::B128],
                word: &binius_core::word::Word,
            ) -> binius_verifier::config::B128 {
                binius_ip::channel::subset_sum_word(elems, *word)
            }
            fn select(
                &mut self,
                elems: &[binius_verifier::config::B128],
                word: &binius_core::word::Word,
            ) -> binius_verifier::config::B128 {
                binius_ip::channel::select_word(elems, *word)
            }
            fn pack_words(
                &mut self,
                words: &[binius_core::word::Word],
            ) -> Vec<binius_verifier::config::B128> {
                binius_ip::channel::pack_words_concrete::<
                    binius_verifier::config::B128,
                    binius_verifier::config::B128,
                >(words)
            }
        }
    };
}
