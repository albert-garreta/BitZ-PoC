//! Flock work-plan transport for the shared opening's host transcript.
//!
//! Work budgets come from the current PCS's transcript-independent plan.
//! Falcon retains its atomic frames, auxiliary nonce vector, and final
//! verifier-only draws so its enclosing transcript remains synchronized.

use super::Error;
use crate::{
    ligerito_flock::{ZincChallenger, grinding_plan::GrindingPlan},
    transcript::traits::Transcript,
};
use flock_core::{challenger::Challenger, field::Gf128, pcs::ligerito::LigeritoSecurityConfig};

pub(crate) enum GrindingNonces<'a> {
    Prove(&'a mut Vec<u64>),
    Verify { values: &'a [u64], cursor: usize },
}

pub(crate) struct GrindingContext<'a> {
    pub plan: &'a GrindingPlan,
    pub nonces: GrindingNonces<'a>,
}

pub(super) struct GrindingChallenger<'a, 'b, T: Transcript + Send> {
    inner: ZincChallenger<'a, T>,
    security: &'a mut GrindingContext<'b>,
    next: usize,
    active: bool,
    valid: bool,
    final_verifier_draws: usize,
}

impl<'a, 'b, T: Transcript + Send> GrindingChallenger<'a, 'b, T> {
    pub(super) fn new(
        transcript: &'a mut T,
        security: &'a mut GrindingContext<'b>,
        config: &LigeritoSecurityConfig,
    ) -> Result<Self, Error> {
        if !security.plan.matches(config) {
            return Err(Error::Invalid("shared Ligerito grinding configuration"));
        }
        let final_verifier_draws = config
            .levels
            .last()
            .and_then(|level| level.queries.checked_next_power_of_two())
            .filter(|&queries| queries > 0)
            .ok_or(Error::Invalid("shared Ligerito query count"))?
            .ilog2() as usize
            + 1;
        Ok(Self {
            inner: ZincChallenger(transcript),
            security,
            next: 0,
            active: false,
            valid: true,
            final_verifier_draws,
        })
    }

    fn begin(&mut self, native: Option<u32>) -> Option<u32> {
        if !self.valid {
            return None;
        }
        let Some(block) = self.security.plan.blocks.get(self.next) else {
            self.valid = false;
            return None;
        };
        if block.native_bits != native {
            self.valid = false;
            return None;
        }
        self.inner.observe_label(b"bitz/flock/atomic/v1");
        self.inner.observe_bytes(&(self.next as u64).to_le_bytes());
        self.inner.observe_label(block.label.as_bytes());
        self.inner.observe_bytes(&block.bits.to_le_bytes());
        self.inner.observe_bytes(&[u8::from(native.is_some())]);
        self.next += 1;
        self.active = true;
        Some(block.bits)
    }

    fn before_sample(&mut self) -> bool {
        if !self.valid {
            return false;
        }
        if !self.active {
            let Some(bits) = self.begin(None) else {
                return false;
            };
            if bits > 0 {
                match &mut self.security.nonces {
                    GrindingNonces::Prove(nonces) => nonces.push(self.inner.grind_pow(bits)),
                    GrindingNonces::Verify { values, cursor } => {
                        let Some(&nonce) = values.get(*cursor) else {
                            self.valid = false;
                            return false;
                        };
                        *cursor += 1;
                        self.valid &= self.inner.verify_pow(nonce, bits);
                    }
                }
            }
        }
        self.valid
    }

    pub(super) fn finish(mut self) -> bool {
        // Flock omits this verifier-only suffix on the prover. These draws
        // stay in the last query block, without another atomic guard.
        if self.valid && matches!(self.security.nonces, GrindingNonces::Prove(_)) {
            for _ in 0..self.final_verifier_draws {
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
        self.try_sample_f128().expect("valid prover work schedule")
    }

    fn try_sample_f128(&mut self) -> Option<Gf128> {
        self.before_sample().then(|| self.inner.sample_f128())
    }

    fn try_sample_f128_vec(&mut self, n: usize) -> Option<Vec<Gf128>> {
        let valid = if n == 0 {
            self.valid
        } else {
            self.before_sample()
        };
        valid.then(|| self.inner.sample_f128_vec(n))
    }

    fn grind_pow(&mut self, bits: u32) -> u64 {
        let bits = self.begin(Some(bits)).expect("valid prover work schedule");
        self.inner.grind_pow(bits)
    }

    fn verify_pow(&mut self, nonce: u64, bits: u32) -> bool {
        let Some(bits) = self.begin(Some(bits)) else {
            return false;
        };
        self.valid &= self.inner.verify_pow(nonce, bits);
        self.valid
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ligerito_flock::{LigeritoSelection, grinding_plan::ChallengeBlock},
        transcript::Blake3Transcript,
    };

    fn plan() -> GrindingPlan {
        GrindingPlan::scripted(vec![
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
        ])
    }

    fn scripted<'a, 'b>(
        transcript: &'a mut Blake3Transcript,
        security: &'a mut GrindingContext<'b>,
    ) -> GrindingChallenger<'a, 'b, Blake3Transcript> {
        GrindingChallenger {
            inner: ZincChallenger(transcript),
            security,
            next: 0,
            active: false,
            valid: true,
            final_verifier_draws: 0,
        }
    }

    #[test]
    fn vectors_and_native_pow_share_blocks_and_replay_exactly() {
        let plan = plan();
        let mut pt = Blake3Transcript::new();
        let mut vt = Blake3Transcript::new();
        let mut nonces = Vec::new();
        let mut security = GrindingContext {
            plan: &plan,
            nonces: GrindingNonces::Prove(&mut nonces),
        };
        let mut p = scripted(&mut pt, &mut security);
        p.final_verifier_draws = 3;
        let host = p.sample_f128_vec(7);
        let native = p.grind_pow(4);
        let queries = p.sample_f128_vec(40);
        assert!(p.finish());
        assert_eq!(nonces.len(), 1);
        let mut security = GrindingContext {
            plan: &plan,
            nonces: GrindingNonces::Verify {
                values: &nonces,
                cursor: 0,
            },
        };
        let mut v = scripted(&mut vt, &mut security);
        assert_eq!(v.try_sample_f128_vec(7), Some(host));
        assert!(v.verify_pow(native, 4));
        assert_eq!(v.try_sample_f128_vec(40), Some(queries));
        assert!(v.try_sample_f128_vec(3).is_some());
        assert!(v.finish());
        assert_eq!(pt.get_challenge::<u128>(), vt.get_challenge::<u128>());
    }

    #[test]
    fn rejects_missing_extra_nonces_and_wrong_native_schedule() {
        let plan = plan();
        let mut pt = Blake3Transcript::new();
        let mut nonces = Vec::new();
        let mut security = GrindingContext {
            plan: &plan,
            nonces: GrindingNonces::Prove(&mut nonces),
        };
        let mut p = scripted(&mut pt, &mut security);
        let host = p.sample_f128();
        let native = p.grind_pow(4);
        assert!(p.finish());

        let mut vt = Blake3Transcript::new();
        let mut missing = GrindingContext {
            plan: &plan,
            nonces: GrindingNonces::Verify {
                values: &[],
                cursor: 0,
            },
        };
        let mut v = scripted(&mut vt, &mut missing);
        assert!(v.try_sample_f128().is_none());
        assert!(v.try_sample_f128_vec(7).is_none());
        assert!(!v.finish());

        nonces.push(0);
        for wrong_native in [false, true] {
            let mut vt = Blake3Transcript::new();
            let mut security = GrindingContext {
                plan: &plan,
                nonces: GrindingNonces::Verify {
                    values: &nonces,
                    cursor: 0,
                },
            };
            let mut v = scripted(&mut vt, &mut security);
            assert_eq!(v.try_sample_f128(), Some(host));
            assert_eq!(
                v.verify_pow(native, if wrong_native { 3 } else { 4 }),
                !wrong_native
            );
            assert!(!v.finish());
        }
    }

    #[test]
    fn rejects_a_plan_from_another_ligerito_configuration() {
        let resolved = LigeritoSelection::MATCHED_UDR.resolve(13, 100).unwrap();
        let plan = GrindingPlan::resolve(resolved.security(), 100).unwrap();
        let mut changed = resolved.security().clone();
        changed.levels[0].queries += 1;
        let mut transcript = Blake3Transcript::new();
        let mut nonces = Vec::new();
        let mut security = GrindingContext {
            plan: &plan,
            nonces: GrindingNonces::Prove(&mut nonces),
        };
        assert!(GrindingChallenger::new(&mut transcript, &mut security, &changed).is_err());
        assert!(
            GrindingChallenger::new(&mut transcript, &mut security, resolved.security()).is_ok()
        );
    }
}
