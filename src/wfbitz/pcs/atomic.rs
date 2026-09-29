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
            self.transcript.public_message(b"bitz/wfbitz/flock-atomic/v1");
            self.transcript.public_message(digest);
            self.transcript.public_message(&(index as u64));
            self.transcript.public_message(&(block.label.len() as u64));
            self.transcript.public_message(block.label.as_bytes());
            self.transcript.public_message(&[u8::from(block.native_bits.is_some())]);
            self.transcript.public_message(&block.native_bits.unwrap_or(0));
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
    fn failed(&self) -> bool { self.failed() }
}

impl Transport for VerifierChallenger<'_, '_> {
    atomic_frame!();
    fn guard(&mut self, bits: u32) -> bool { self.checked_pow(None, bits) }
    fn failed(&self) -> bool { self.failed() }
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
        if plan.blocks.iter().any(|block| block.bits > block.native_bits.unwrap_or(0)) {
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
        Self { plan, next: 0, active: false, valid: true, digest }
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

    fn complete(&self) -> bool { self.valid && self.next == self.plan.blocks.len() }
}

pub(super) struct AtomicChallenger<'a, C> {
    inner: C,
    cursor: Option<Cursor<'a>>,
}

impl<'a, C: Transport> AtomicChallenger<'a, C> {
    pub(super) fn new(inner: C, plan: Option<&'a GrindingPlan>) -> Self {
        Self { inner, cursor: plan.map(Cursor::new) }
    }

    fn observed(&mut self) {
        if let Some(cursor) = &mut self.cursor { cursor.active = false; }
    }

    fn begin(&mut self, native: Option<u32>) -> u32 {
        let Some(cursor) = &mut self.cursor else { return native.unwrap_or(0); };
        let Some((index, block)) = cursor.begin(native) else { return native.unwrap_or(0); };
        if block.bits > block.native_bits.unwrap_or(0) {
            self.inner.frame(&cursor.digest, index, block);
        }
        block.bits
    }

    fn before_sample(&mut self) {
        if self.cursor.as_ref().is_some_and(|cursor| !cursor.active) {
            let bits = self.begin(None);
            if bits > 0 && !self.inner.guard(bits) {
                self.cursor.as_mut().expect("atomic cursor").valid = false;
            }
        }
        // Flock's sampling trait is infallible. Keep its ordinary random stream
        // after failure so distinct-query retries terminate; finish rejects the
        // entire replay. Never substitute constant challenges here.
    }

    // Closed Flock transcripts need no artificial prover suffix. The verifier's
    // final alpha vector/beta remain in its already-open query block.
    pub(super) fn finish(self) -> bool {
        !self.inner.failed() && self.cursor.as_ref().is_none_or(Cursor::complete)
    }
}

impl<C: Transport> Challenger for AtomicChallenger<'_, C> {
    fn observe_label(&mut self, label: &[u8]) {
        self.observed(); self.inner.observe_label(label);
    }
    fn observe_f128(&mut self, value: FlockF128) {
        self.observed(); self.inner.observe_f128(value);
    }
    fn observe_f128_slice(&mut self, values: &[FlockF128]) {
        self.observed(); self.inner.observe_f128_slice(values);
    }
    fn observe_bytes(&mut self, bytes: &[u8]) {
        self.observed(); self.inner.observe_bytes(bytes);
    }
    fn sample_f128(&mut self) -> FlockF128 {
        self.before_sample(); self.inner.sample_f128()
    }
    fn sample_f128_vec(&mut self, n: usize) -> Vec<FlockF128> {
        if n > 0 { self.before_sample(); }
        self.inner.sample_f128_vec(n)
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
        } else { valid }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wfbitz::{Proof, build_prover, build_verifier};

    fn plan(extra: bool) -> GrindingPlan {
        GrindingPlan::scripted([("host", None, if extra { 3 } else { 0 }),
                ("queries", Some(0), if extra { 4 } else { 0 }),
                ("fold", Some(2), if extra { 5 } else { 2 })]
                .into_iter().map(|(label, native_bits, bits)| ChallengeBlock {
                    label: label.into(), native_bits, raw_error: 0.01, bits,
                }).collect())
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
        let mut state = build_verifier(b"atomic-test", b"fixture", proof);
        let raw = VerifierChallenger::new_ligerito(&mut state, Gf::one());
        let mut c = AtomicChallenger::new(raw, Some(plan));
        prefix(&mut c);
        assert!(c.sample_f128_vec(0).is_empty());
        let mut actual = c.sample_f128_vec(7);
        actual.push(c.sample_f128());
        let first = c.verify_pow(nonces[0], 0);
        actual.extend(c.sample_f128_vec(10));
        c.observe_bytes(b"next polynomial");
        let second = c.verify_pow(nonces[1], 2);
        actual.push(c.sample_f128());
        // Verifier-only suffix stays in the already-open final work block.
        c.sample_f128_vec(3);
        c.sample_f128();
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
            let mut changed = proof.clone(); changed.narg_string.truncate(cut);
            assert!(!replay(&changed, &plan, &values, nonces));
        }
        let mut changed = proof.clone(); changed.narg_string[0] ^= 1;
        assert!(!replay(&changed, &plan, &values, nonces));
        assert!(!replay(&proof, &plan, &values, [nonces[0] ^ 1, nonces[1]]));
        let mut wrong = plan.clone(); wrong.blocks[1].native_bits = Some(1);
        assert!(!replay(&proof, &wrong, &values, nonces));
        let mut extra = plan.clone(); extra.blocks.push(plan.blocks[2].clone());
        assert!(!replay(&proof, &extra, &values, nonces));
    }
}
