//! Grinding guards for challenge blocks in the pinned Flock schedule (Johnson+OOD or UDR).
//!
//! A native PoW starts a block. Otherwise the first draw after an observation
//! starts one. Consecutive draws (including query retries and alpha) share it.
use super::*;

use super::grinding_plan::{ChallengeBlock, GrindingPlan};

pub(crate) enum GrindingNonces<'a> {
    Prove(&'a mut Vec<u64>),
    Verify { values: &'a [u64], cursor: usize },
}

pub(crate) struct GrindingContext<'a> {
    pub plan: &'a GrindingPlan,
    pub nonces: GrindingNonces<'a>,
}

pub(crate) struct GrindingChallenger<'a, 'b, T: Transcript + Send> {
    inner: ZincChallenger<'a, T>,
    security: &'a mut GrindingContext<'b>,
    next: usize,
    active: bool,
    valid: bool,
}

impl<'a, 'b, T: Transcript + Send> GrindingChallenger<'a, 'b, T> {
    pub fn new(transcript: &'a mut T, security: &'a mut GrindingContext<'b>) -> Self {
        Self {
            inner: ZincChallenger(transcript),
            security,
            next: 0,
            active: false,
            valid: true,
        }
    }
    fn begin(&mut self, native: Option<u32>) -> u32 {
        let Some(block) = self.security.plan.blocks.get(self.next) else {
            self.valid = false;
            return 0;
        };
        self.valid &= block.native_bits == native;
        // This protocol domain separator stays fixed across Rust type renames.
        self.inner.observe_label(b"bitz/flock/atomic/v1");
        self.inner.observe_bytes(&(self.next as u64).to_le_bytes());
        self.inner.observe_label(block.label.as_bytes());
        self.inner.observe_bytes(&block.bits.to_le_bytes());
        self.inner.observe_bytes(&[u8::from(native.is_some())]);
        self.next += 1;
        self.active = true;
        block.bits
    }
    pub fn finish(mut self) -> bool {
        // The pinned prover omits alpha and beta for the final verifier-only check.
        // Consume that suffix so callers can safely continue the transcript.
        if matches!(self.security.nonces, GrindingNonces::Prove(_)) {
            for _ in 0..self.security.plan.final_verifier_draws {
                self.inner.sample_f128();
            }
        }
        self.valid
            && self.next == self.security.plan.blocks.len()
            && match &self.security.nonces {
                GrindingNonces::Prove(_) => true,
                GrindingNonces::Verify { values, cursor } => values.len() == *cursor,
            }
    }
}

impl<T: Transcript + Send> Challenger for GrindingChallenger<'_, '_, T> {
    fn observe_label(&mut self, label: &[u8]) {
        self.active = false;
        self.inner.observe_label(label);
    }
    fn observe_f128(&mut self, value: Gf128) {
        self.active = false;
        self.inner.observe_f128(value);
    }
    fn observe_f128_slice(&mut self, values: &[Gf128]) {
        self.active = false;
        self.inner.observe_f128_slice(values);
    }
    fn observe_bytes(&mut self, bytes: &[u8]) {
        self.active = false;
        self.inner.observe_bytes(bytes);
    }
    fn sample_f128(&mut self) -> Gf128 {
        if !self.active {
            let bits = self.begin(None);
            if bits > 0 {
                match &mut self.security.nonces {
                    GrindingNonces::Prove(nonces) => nonces.push(self.inner.grind_pow(bits)),
                    GrindingNonces::Verify { values, cursor } => {
                        let nonce = values.get(*cursor).copied().unwrap_or(0);
                        self.valid &= *cursor < values.len() && self.inner.verify_pow(nonce, bits);
                        *cursor += 1;
                    }
                }
            }
        }
        self.inner.sample_f128()
    }
    fn grind_pow(&mut self, bits: u32) -> u64 {
        let bits = self.begin(Some(bits));
        self.inner.grind_pow(bits)
    }
    fn verify_pow(&mut self, nonce: u64, bits: u32) -> bool {
        let bits = self.begin(Some(bits));
        let valid = self.inner.verify_pow(nonce, bits);
        self.valid &= valid;
        valid && self.valid
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transcript::Blake3Transcript;

    #[test]
    fn vectors_and_native_pow_share_blocks_and_replay_exactly() {
        let plan = GrindingPlan::scripted(vec![
                ChallengeBlock {
                    label: "host".into(),
                    native_bits: None,
                    raw_error: 0.01,
                    bits: 3,
                },
                ChallengeBlock {
                    label: "queries".into(),
                    native_bits: Some(4),
                    raw_error: 0.01,
                    bits: 6,
                },
            ]);
        let mut pt = Blake3Transcript::new();
        let mut vt = Blake3Transcript::new();
        let mut nonces = Vec::new();
        let mut security = GrindingContext {
            plan: &plan,
            nonces: GrindingNonces::Prove(&mut nonces),
        };
        let mut p = GrindingChallenger::new(&mut pt, &mut security);
        let host: Vec<_> = (0..7).map(|_| p.sample_f128()).collect();
        // No observation between the host vector and native grind. The native
        // grind still starts a new block, upgrading its existing nonce to 6 bits.
        let native = p.grind_pow(4);
        let queries: Vec<_> = (0..40).map(|_| p.sample_f128()).collect();
        assert!(p.finish());
        assert_eq!(nonces.len(), 1, "no per-coordinate or per-query host nonce");
        let mut security = GrindingContext {
            plan: &plan,
            nonces: GrindingNonces::Verify {
                values: &nonces,
                cursor: 0,
            },
        };
        let mut v = GrindingChallenger::new(&mut vt, &mut security);
        for x in host {
            assert_eq!(v.sample_f128(), x);
        }
        assert!(v.verify_pow(native, 4));
        for x in queries {
            assert_eq!(v.sample_f128(), x);
        }
        assert!(v.finish());
        assert_eq!(pt.get_challenge::<u128>(), vt.get_challenge::<u128>());
    }

    /// A unique-decoding level charges every fold round the same
    /// proximity-gap term (no row union shrinks it), and flock's taper
    /// leaves each round its own `fold_grinding_bits − j` bits: MultiSwap's
    /// `udrg:3:4:114` at 2^25 committed bits.
    #[test]
    fn udr_fold_rounds_share_one_term() {
        let config = crate::ligerito_flock::custom_udr_grind_config_bits(25, 3, 4, Some(114));
        let plan = GrindingPlan::resolve(&config, 114).unwrap();
        let level = &config.levels[0];
        let (pg, _) = level.paper_predicted_bits();
        for round in 0..config.initial_k {
            let block = plan
                .blocks
                .iter()
                .find(|block| block.label == format!("fold/0/{round}"))
                .expect("fold block");
            assert_eq!(block.raw_error, 2f64.powf(-pg) + 2. * 2f64.powi(-128), "round {round}");
            let native = level.fold_grinding_bits as u32 - round as u32;
            assert_eq!((block.native_bits, block.bits), (Some(native), native), "round {round}");
            assert!(pg + f64::from(block.bits) >= 114., "round {round}");
        }
    }
}
