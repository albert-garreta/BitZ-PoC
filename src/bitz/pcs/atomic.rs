//! Enforces Flock's atomic work plan on its existing transcript transport.
//! Native callbacks and vector framing are forwarded unchanged when no extra
//! work is required. Extra guards live in narg, never in a host nonce vector.

use super::*;
use crate::ligerito_flock::grinding_plan::{ChallengeBlock, GrindingPlan};

pub(super) trait Transport: Challenger {
    fn frame(&mut self, digest: &[u8; 32], index: usize, block: &ChallengeBlock);
    fn guard(&mut self, bits: u32) -> bool;
    fn failed(&self) -> bool;
}

macro_rules! atomic_frame {
    () => {
        fn frame(&mut self, digest: &[u8; 32], index: usize, block: &ChallengeBlock) {
            self.transcript
                .public_message(b"bitz/bitz/flock-atomic/v1");
            self.transcript.public_message(digest);
            self.transcript.public_message(&(index as u64));
            self.transcript.public_message(&(block.label.len() as u64));
            self.transcript.public_message(block.label.as_bytes());
            self.transcript
                .public_message(&[u8::from(block.native_bits.is_some())]);
            self.transcript
                .public_message(&block.native_bits.unwrap_or(0));
            self.transcript.public_message(&block.bits);
        }
    };
}

impl Transport for ProverChallenger<'_> {
    atomic_frame!();
    fn guard(&mut self, bits: u32) -> bool {
        self.grind_pow(bits);
        true
    }
    fn failed(&self) -> bool {
        self.failed()
    }
}

impl Transport for VerifierChallenger<'_, '_> {
    atomic_frame!();
    fn guard(&mut self, bits: u32) -> bool {
        self.checked_pow(None, bits)
    }
    fn failed(&self) -> bool {
        self.failed()
    }
}

impl VerifierChallenger<'_, '_> {
    pub(super) fn checked_pow(&mut self, expected: Option<u64>, bits: u32) -> bool {
        self.transcript.public_message(POW_TAG);
        self.transcript.public_message(&bits);
        let seed = super::super::codec::gf_to_bytes(self.transcript.verifier_message::<Gf>());
        let encoded = self.read::<[u8; 8]>().map(u64::from_le_bytes);
        let valid = encoded.is_some_and(|nonce| {
            expected.is_none_or(|expected| expected == nonce) && pow_valid(&seed, nonce, bits)
        });
        self.failed |= !valid;
        !self.failed
    }
}

struct Cursor<'a> {
    plan: &'a GrindingPlan,
    next: usize,
    active: bool,
    valid: bool,
    digest: [u8; 32],
}

impl<'a> Cursor<'a> {
    fn new(plan: &'a GrindingPlan) -> Self {
        let mut digest = [0; 32];
        if plan
            .blocks
            .iter()
            .any(|block| block.bits > block.native_bits.unwrap_or(0))
        {
            let mut hash = blake3::Hasher::new();
            hash.update(b"bitz/flock-work-plan/v1");
            for block in &plan.blocks {
                hash.update(&(block.label.len() as u64).to_le_bytes());
                hash.update(block.label.as_bytes());
                hash.update(&[u8::from(block.native_bits.is_some())]);
                hash.update(&block.native_bits.unwrap_or(0).to_le_bytes());
                hash.update(&block.bits.to_le_bytes());
            }
            digest = *hash.finalize().as_bytes();
        }
        Self {
            plan,
            next: 0,
            active: false,
            valid: true,
            digest,
        }
    }

    fn begin(&mut self, native: Option<u32>) -> Option<(usize, &'a ChallengeBlock)> {
        let Some(block) = self.plan.blocks.get(self.next) else {
            self.valid = false;
            return None;
        };
        self.valid &= block.native_bits == native;
        let index = self.next;
        self.next += 1;
        self.active = true;
        Some((index, block))
    }

    fn complete(&self) -> bool {
        self.valid && self.next == self.plan.blocks.len()
    }
}

pub(super) struct AtomicChallenger<'a, C> {
    inner: C,
    cursor: Option<Cursor<'a>>,
}

impl<'a, C: Transport> AtomicChallenger<'a, C> {
    pub(super) fn new(inner: C, plan: Option<&'a GrindingPlan>) -> Self {
        Self {
            inner,
            cursor: plan.map(Cursor::new),
        }
    }

    fn observed(&mut self) {
        if let Some(cursor) = &mut self.cursor {
            cursor.active = false;
        }
    }

    fn begin(&mut self, native: Option<u32>) -> u32 {
        let Some(cursor) = &mut self.cursor else {
            return native.unwrap_or(0);
        };
        let Some((index, block)) = cursor.begin(native) else {
            return native.unwrap_or(0);
        };
        if block.bits > block.native_bits.unwrap_or(0) {
            self.inner.frame(&cursor.digest, index, block);
        }
        block.bits
    }

    fn before_sample(&mut self) -> bool {
        if !self.valid() {
            return false;
        }
        if self.cursor.as_ref().is_none_or(|cursor| cursor.active) {
            return true;
        }
        let bits = self.begin(None);
        if bits > 0 && !self.inner.guard(bits) {
            self.cursor.as_mut().expect("atomic cursor").valid = false;
        }
        self.valid()
    }

    fn valid(&self) -> bool {
        !self.inner.failed() && self.cursor.as_ref().is_none_or(|cursor| cursor.valid)
    }

    // Closed Flock transcripts need no artificial prover suffix. The verifier's
    // final alpha vector/beta remain in its already-open query block.
    pub(super) fn finish(self) -> bool {
        !self.inner.failed() && self.cursor.as_ref().is_none_or(Cursor::complete)
    }
}

impl<C: Transport> Challenger for AtomicChallenger<'_, C> {
    fn observe_label(&mut self, label: &[u8]) {
        self.observed();
        self.inner.observe_label(label);
    }
    fn observe_f128(&mut self, value: FlockF128) {
        self.observed();
        self.inner.observe_f128(value);
    }
    fn observe_f128_slice(&mut self, values: &[FlockF128]) {
        self.observed();
        self.inner.observe_f128_slice(values);
    }
    fn observe_bytes(&mut self, bytes: &[u8]) {
        self.observed();
        self.inner.observe_bytes(bytes);
    }
    fn sample_f128(&mut self) -> FlockF128 {
        self.try_sample_f128().expect("valid prover work schedule")
    }
    fn sample_f128_vec(&mut self, n: usize) -> Vec<FlockF128> {
        self.try_sample_f128_vec(n)
            .expect("valid prover work schedule")
    }
    fn try_sample_f128(&mut self) -> Option<FlockF128> {
        self.before_sample().then(|| self.inner.sample_f128())
    }
    fn try_sample_f128_vec(&mut self, n: usize) -> Option<Vec<FlockF128>> {
        let valid = if n == 0 {
            self.valid()
        } else {
            self.before_sample()
        };
        valid.then(|| self.inner.sample_f128_vec(n))
    }
    fn grind_pow(&mut self, bits: u32) -> u64 {
        let effective = self.begin(Some(bits));
        self.inner.grind_pow(effective)
    }
    fn verify_pow(&mut self, nonce: u64, bits: u32) -> bool {
        let effective = self.begin(Some(bits));
        let valid = self.inner.verify_pow(nonce, effective);
        if let Some(cursor) = &mut self.cursor {
            cursor.valid &= valid;
            cursor.valid
        } else {
            valid
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bitz::{Proof, build_prover, build_verifier};

    #[test]
    fn failed_guard_never_forwards_a_protected_draw() {
        #[derive(Default)]
        struct Counter {
            draws: usize,
            guards: usize,
        }
        impl Challenger for Counter {
            fn observe_f128(&mut self, _: Gf) {}
            fn sample_f128(&mut self) -> Gf {
                self.draws += 1;
                Gf::one()
            }
            fn sample_f128_vec(&mut self, n: usize) -> Vec<Gf> {
                self.draws += 1;
                vec![Gf::one(); n]
            }
        }
        impl Transport for Counter {
            fn frame(&mut self, _: &[u8; 32], _: usize, _: &ChallengeBlock) {}
            fn guard(&mut self, _: u32) -> bool {
                self.guards += 1;
                false
            }
            fn failed(&self) -> bool {
                false
            }
        }
        let plan = plan(true);
        for first_is_vector in [false, true] {
            let mut c = AtomicChallenger::new(Counter::default(), Some(&plan));
            if first_is_vector {
                assert!(c.try_sample_f128_vec(3).is_none());
            } else {
                assert!(c.try_sample_f128().is_none());
            }
            assert!(c.try_sample_f128().is_none());
            assert!(c.try_sample_f128_vec(3).is_none());
            assert!(c.try_sample_f128_vec(0).is_none());
            assert_eq!((c.inner.draws, c.inner.guards), (0, 1));
            assert!(!c.finish());
        }
    }

    #[test]
    fn real_johnson_opening_replays_additional_ood_work() {
        use crate::ligerito_flock::LigeritoSelection;
        use crate::bitz::{BitZParams, BitZProver, BitZVerifier, LinearClaim, WINDOW};
        let shape = Shape::new(7, 13).unwrap();
        // This is a validated target-100 ladder with stronger local work.
        // Production Johnson target-128 remains unsupported by its OOD bound.
        let resolved = LigeritoSelection::CustomJohnson {
            log_inv_rate: 1,
            initial_k: 3,
        }
        .resolve(13, 100)
        .unwrap();
        let mut config = resolved.security().clone();
        for (index, level) in config.levels.iter_mut().enumerate() {
            level.eta = Some(0.125);
            level.ood_samples = if index == 0 { 0 } else { 2 };
            level.queries = 1;
            while level.paper_predicted_bits().1 < 128.0 {
                level.queries += 1;
            }
            let (pg, query) = level.paper_predicted_bits();
            level.expected_eps_pg_bits = pg;
            level.expected_eps_query_bits = query;
            level.expected_eps_ood_bits = level.paper_predicted_ood_bits();
            level.fold_grinding_bits = (100.0 - pg).ceil().max(0.0) as usize;
        }
        config.validate().unwrap();
        let plan = Arc::new(GrindingPlan::resolve(&config, 128).unwrap());
        assert!(
            plan.blocks
                .iter()
                .any(|b| b.native_bits.is_none() && b.bits > 0)
        );
        let pcs = Pcs::with_security_and_work(&shape, &config, LigeritoProfile::Fast, plan)
            .unwrap()
            .with_native_policy(Policy::new(Some(128), 0, 0).unwrap());
        let params = BitZParams::new(shape, 17, crate::pcs::smallest_generator().into()).unwrap();
        let rows = vec![vec![1, 0]; shape.columns()];
        let (root, hint) = pcs.commit(&shape, rows).unwrap();
        let claim = LinearClaim::new(
            &params,
            vec![1; shape.rows()],
            vec![1; shape.columns()],
            (shape.columns() % 17) as u128,
        )
        .unwrap();
        let mut prover = build_prover(b"johnson-local-work", b"fixture");
        BitZProver::new(params, WINDOW)
            .prove(&claim, &pcs, &hint, &mut prover, None)
            .unwrap();
        let proof = prover.finish();
        BitZVerifier::new(params, WINDOW)
            .verify(
                &claim,
                &pcs,
                root,
                build_verifier(b"johnson-local-work", b"fixture", &proof),
                None,
            )
            .unwrap();
    }

    fn plan(extra: bool) -> GrindingPlan {
        GrindingPlan::scripted(
            [
                ("host", None, if extra { 3 } else { 0 }),
                ("queries", Some(0), if extra { 4 } else { 0 }),
                ("fold", Some(2), if extra { 5 } else { 2 }),
            ]
            .into_iter()
            .map(|(label, native_bits, bits)| ChallengeBlock {
                label: label.into(),
                native_bits,
                raw_error: 0.01,
                bits,
            })
            .collect(),
        )
    }

    fn prefix(c: &mut impl Challenger) {
        c.observe_label(LIGERITO_BASIS_LABEL);
        c.observe_f128(Gf::one());
    }

    fn samples(c: &mut impl Challenger) -> (Vec<Gf>, [u64; 2]) {
        prefix(c);
        assert!(c.sample_f128_vec(0).is_empty());
        let mut values = c.sample_f128_vec(7);
        values.push(c.sample_f128());
        let first = c.grind_pow(0);
        values.extend(c.sample_f128_vec(10));
        c.observe_bytes(b"next polynomial");
        let second = c.grind_pow(2);
        values.push(c.sample_f128());
        (values, [first, second])
    }

    fn replay(proof: &Proof, plan: &GrindingPlan, values: &[Gf], nonces: [u64; 2]) -> bool {
        macro_rules! draw {
            ($expr:expr) => {
                match $expr {
                    Some(value) => value,
                    None => return false,
                }
            };
        }
        let mut state = build_verifier(b"atomic-test", b"fixture", proof);
        let raw = VerifierChallenger::new_ligerito(&mut state, Gf::one());
        let mut c = AtomicChallenger::new(raw, Some(plan));
        prefix(&mut c);
        assert!(draw!(c.try_sample_f128_vec(0)).is_empty());
        let mut actual = draw!(c.try_sample_f128_vec(7));
        actual.push(draw!(c.try_sample_f128()));
        let first = c.verify_pow(nonces[0], 0);
        actual.extend(draw!(c.try_sample_f128_vec(10)));
        c.observe_bytes(b"next polynomial");
        let second = c.verify_pow(nonces[1], 2);
        actual.push(draw!(c.try_sample_f128()));
        // Verifier-only suffix stays in the already-open final work block.
        draw!(c.try_sample_f128_vec(3));
        draw!(c.try_sample_f128());
        let finished = c.finish();
        first && second && finished && actual == values && state.check_eof().is_ok()
    }

    #[test]
    fn no_extra_work_preserves_scalar_vector_and_zero_bit_nonce_bytes() {
        let plan = plan(false);
        let mut reference = build_prover(b"atomic-test", b"fixture");
        let mut raw = ProverChallenger::new_ligerito(&mut reference, Gf::one());
        let expected = samples(&mut raw);
        assert!(!raw.failed());
        let mut state = build_prover(b"atomic-test", b"fixture");
        let raw = ProverChallenger::new_ligerito(&mut state, Gf::one());
        let mut c = AtomicChallenger::new(raw, Some(&plan));
        let actual = samples(&mut c);
        assert!(c.finish());
        assert_eq!(actual, expected);
        let proof = state.finish();
        assert_eq!(proof, reference.finish());
        assert!(replay(&proof, &plan, &actual.0, actual.1));
    }

    #[test]
    fn additional_guards_and_upgraded_native_callbacks_are_checked() {
        let plan = plan(true);
        let mut state = build_prover(b"atomic-test", b"fixture");
        let raw = ProverChallenger::new_ligerito(&mut state, Gf::one());
        let mut c = AtomicChallenger::new(raw, Some(&plan));
        let (values, nonces) = samples(&mut c);
        assert!(c.finish());
        let proof = state.finish();
        assert!(replay(&proof, &plan, &values, nonces));
        for cut in [0, 7, proof.narg_string.len() - 1] {
            let mut changed = proof.clone();
            changed.narg_string.truncate(cut);
            assert!(!replay(&changed, &plan, &values, nonces));
        }
        let mut changed = proof.clone();
        changed.narg_string[0] ^= 1;
        assert!(!replay(&changed, &plan, &values, nonces));
        assert!(!replay(&proof, &plan, &values, [nonces[0] ^ 1, nonces[1]]));
        let mut wrong = plan.clone();
        wrong.blocks[1].native_bits = Some(1);
        assert!(!replay(&proof, &wrong, &values, nonces));
        let mut extra = plan.clone();
        extra.blocks.push(plan.blocks[2].clone());
        assert!(!replay(&proof, &extra, &values, nonces));
    }
}
