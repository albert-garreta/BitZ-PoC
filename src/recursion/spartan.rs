//! The Spartan-level transcript ([`crate::transcript::Sha256Transcript`]) in
//! the circuit, and the mod-`q` pieces of the PIOP verifiers.
//!
//! [`Proxy`] implements the native [`Transcript`] trait over the in-circuit
//! duplex, every absorbed byte PUBLIC: native code that only touches public
//! data (statement frames, configuration bindings, prime sampling, challenge
//! draws and their re-absorption) runs unmodified against it, so its bytes
//! are exactly the native verifier's. The few absorbs of proof data — field
//! elements of round polynomials, grinding nonces, the Round-0 value, an
//! opening target — are mirrored by the `absorb_*` helpers below.

use circuit::Circuit;
use field::Gf128 as Gf;

use crate::piop::spartan::SpartanField;
use crate::transcript::traits::{ConstTranscribable, Transcript};

use super::arith::{ModTerm, Zq, assert_le, assert_mod_q, mulmod, witness_int};
use super::bytes::{Byte, bytes_of_bits, gf_bytes, public_bytes, witness_u64};
use super::sha::{Duplex, ShaCounts, assert_pow};
use super::val::{G, Public};

type F = crate::piop::spartan::u32_mul_relation::SpartanBitzField;
pub type Config = <F as SpartanField>::Config;

const SPARTAN_TRANSCRIPT_FRAME_DOMAIN: &[u8] = b"bitz/spartan/transcript-frame/v2";
const FIELD_ELEMENTS_TAG: &[u8] = b"field-elements";
const GRINDING_NONCE_DOMAIN: &[u8] = b"bitz/spartan/fiat-shamir-grinding/nonce/v1";
const GRINDING_TRANSCRIPT_DOMAIN: &[u8] = b"bitz/spartan/fiat-shamir-grinding/v1";

/// The native transcript over the in-circuit duplex (public data only).
pub struct Proxy<'a, CS: Circuit> {
    pub cs: &'a mut CS,
    pub duplex: &'a mut Duplex<CS::Bool>,
}

impl<CS: Circuit> Transcript for Proxy<'_, CS> {
    fn get_challenge<T: ConstTranscribable>(&mut self) -> T {
        let mut buf = self.duplex.squeeze(self.cs, T::NUM_BYTES);
        assert!(T::NUM_BITS <= T::NUM_BYTES * 8);
        for (i, byte) in buf.iter_mut().enumerate() {
            let remaining = T::NUM_BITS.saturating_sub(i * 8).min(8);
            *byte &= ((1u16 << remaining) - 1) as u8;
        }
        T::read_transcription_bytes_exact(&buf)
    }

    fn fill_sampling_bytes(&mut self, output: &mut [u8]) {
        let bytes = self.duplex.squeeze(self.cs, output.len());
        output.copy_from_slice(&bytes);
    }

    fn absorb_inner(&mut self, v: &[u8]) {
        self.duplex.absorb_public(self.cs, v);
    }
}

/// The Spartan-level transcript gadget.
pub struct SpartanTranscript<B: circuit::BoolWitness> {
    pub duplex: Duplex<B>,
    pub pow: ShaCounts,
}

impl<B: circuit::BoolWitness> SpartanTranscript<B> {
    pub fn new(public: Public) -> Self {
        Self {
            duplex: Duplex::new(public),
            pow: ShaCounts::default(),
        }
    }

    pub fn public(&self) -> &Public {
        self.duplex.public()
    }

    /// The native transcript over this one, for public operations.
    pub fn proxy<'a, CS: Circuit<Bool = B>>(&'a mut self, cs: &'a mut CS) -> Proxy<'a, CS> {
        Proxy { cs, duplex: &mut self.duplex }
    }

    /// [`Transcript::absorb_slice`] of bytes the caller built.
    pub fn absorb_slice<CS: Circuit<Bool = B>>(&mut self, cs: &mut CS, bytes: &[Byte<B>]) {
        self.duplex.absorb_public(cs, &[0x6]);
        self.duplex.absorb_public(cs, &(bytes.len() as u64).to_le_bytes());
        self.duplex.absorb(cs, bytes);
        self.duplex.absorb_public(cs, &[0x7]);
    }

    /// `absorb_spartan_message(tag, payload)`.
    pub fn absorb_message<CS: Circuit<Bool = B>>(&mut self, cs: &mut CS, tag: &[u8], payload: &[Byte<B>]) {
        let mut frame: Vec<Byte<B>> = public_bytes(SPARTAN_TRANSCRIPT_FRAME_DOMAIN);
        frame.extend(public_bytes(&(tag.len() as u64).to_le_bytes()));
        frame.extend(public_bytes(tag));
        frame.extend(public_bytes(&(payload.len() as u64).to_le_bytes()));
        frame.extend_from_slice(payload);
        self.absorb_slice(cs, &frame);
    }

    /// `absorb_field_elements`: canonical 16-byte residues.
    pub fn absorb_field_elements<CS: Circuit<Bool = B>>(&mut self, cs: &mut CS, values: &[Zq<B>]) {
        let mut payload: Vec<Byte<B>> = public_bytes(&(values.len() as u64).to_le_bytes());
        for v in values {
            payload.extend(public_bytes(&16u64.to_le_bytes()));
            payload.extend(super::bitz_pcs::zq_bytes(v));
        }
        self.absorb_message(cs, FIELD_ELEMENTS_TAG, &payload);
    }

    /// `absorb_gf_slice(tag, values)` (the Round-0 value uses tag `0x50`).
    pub fn absorb_gf_slice<CS: Circuit<Bool = B>>(&mut self, cs: &mut CS, tag: u8, values: &[G<B>]) {
        let mut bytes: Vec<Byte<B>> = vec![Byte::P(tag)];
        for v in values {
            bytes.extend(gf_bytes(v));
        }
        self.absorb_slice(cs, &bytes);
    }

    /// `verify_and_absorb_in_domain`: the seed (public), the committed nonce
    /// against it, the nonce absorbed.
    pub fn grinding_boundary<CS: Circuit<Bool = B>>(
        &mut self,
        cs: &mut CS,
        domain: &[u8],
        index: u64,
        bits: u32,
        nonce: u64,
    ) {
        assert!(bits > 0, "a zero-difficulty boundary has no transcript operations");
        let seed = crate::piop::spartan::grinding::derive_grinding_seed_in_domain(&mut self.proxy(cs), domain, index, bits)
            .expect("public grinding difficulty");
        let nonce = witness_u64(cs, nonce);
        assert_pow(cs, &public_bytes(seed.as_bytes()), &nonce, bits, &mut self.pow);
        self.absorb_slice(cs, &public_bytes(GRINDING_NONCE_DOMAIN));
        self.absorb_slice(cs, &bytes_of_bits(&nonce));
    }

    /// [`Self::grinding_boundary`] with the seed left unpublished: it only
    /// feeds the in-circuit proof-of-work hash, so the public stream carries
    /// no bytes for it.
    pub fn grinding_boundary_private<CS: Circuit<Bool = B>>(
        &mut self,
        cs: &mut CS,
        domain: &[u8],
        index: u64,
        bits: u32,
        nonce: u64,
    ) {
        assert!(bits > 0 && bits <= 256 && !domain.is_empty(), "a public grinding difficulty");
        {
            let mut p = self.proxy(cs);
            p.absorb_slice(GRINDING_TRANSCRIPT_DOMAIN);
            p.absorb_slice(domain);
            p.absorb_slice(&index.to_le_bytes());
            p.absorb_slice(&bits.to_le_bytes());
        }
        let seed = self.duplex.squeeze_bytes(cs, 32);
        let nonce = witness_u64(cs, nonce);
        assert_pow(cs, &seed, &nonce, bits, &mut self.pow);
        self.absorb_slice(cs, &public_bytes(GRINDING_NONCE_DOMAIN));
        self.absorb_slice(cs, &bytes_of_bits(&nonce));
    }

    /// `squeeze_field`: a public challenge (sampled and re-absorbed).
    pub fn squeeze_field<CS: Circuit<Bool = B>>(&mut self, cs: &mut CS, cfg: &Config) -> u128 {
        let x = crate::piop::spartan::squeeze_field::<F, _>(&mut self.proxy(cs), cfg)
            .expect("public field sampling");
        canonical(&x, cfg)
    }
}

/// The canonical integer of a residue.
pub fn canonical(x: &F, cfg: &Config) -> u128 {
    use field::RingOps;
    let words = cfg.to_integer(x);
    let w: &field::Uint<2> = &words;
    u128::from(w.as_words()[0]) | (u128::from(w.as_words()[1]) << 64)
}

/// A committed canonical residue (`x < q` asserted).
pub fn witness_residue<CS: Circuit>(cs: &mut CS, value: u128, q: u128) -> Zq<CS::Bool> {
    let width = 128 - q.leading_zeros() as usize;
    let bits = witness_int(cs, value, width);
    assert_le(cs, &bits, q - 1);
    Zq::V(bits)
}

/// A symbolic residue `constant + Σ c_i x_i (mod q)`.
#[derive(Clone)]
pub struct ModLc<B: circuit::BoolWitness> {
    pub constant: u128,
    pub terms: Vec<(u128, Zq<B>)>,
}

impl<B: circuit::BoolWitness> ModLc<B> {
    pub fn constant(c: u128) -> Self {
        Self { constant: c, terms: Vec::new() }
    }

    pub fn of(x: &Zq<B>) -> Self {
        Self { constant: 0, terms: vec![(1, x.clone())] }
    }

    pub fn add(&mut self, c: u128, x: &Zq<B>, q: u128) -> &mut Self {
        match x {
            Zq::P(v) => self.constant = (self.constant + mulmod(c % q, *v % q, q)) % q,
            Zq::V(_) => self.terms.push((c % q, x.clone())),
        }
        self
    }

    pub fn add_lc(&mut self, c: u128, other: &Self, q: u128) -> &mut Self {
        self.constant = (self.constant + mulmod(c % q, other.constant, q)) % q;
        for (k, x) in &other.terms {
            self.terms.push((mulmod(c % q, *k, q), x.clone()));
        }
        self
    }

    /// Asserts `self ≡ 0 (mod q)`.
    pub fn assert_zero<CS: Circuit<Bool = B>>(&self, cs: &mut CS, q: u128, public: &Public, what: &str) {
        let terms: Vec<ModTerm<'_, B>> = self.terms.iter().map(|(c, x)| ModTerm { coeff: *c, value: x }).collect();
        assert_mod_q(cs, &terms, self.constant, q, public, what);
    }

    /// The canonical residue as a committed value (public when public).
    pub fn settle<CS: Circuit<Bool = B>>(&self, cs: &mut CS, q: u128, public: &Public) -> Zq<B> {
        if self.terms.is_empty() {
            return Zq::P(self.constant);
        }
        let value = cs_value(cs, self, q);
        let r = witness_residue(cs, value.unwrap_or(0), q);
        let mut check = self.clone();
        check.add(q - 1, &r, q);
        check.assert_zero(cs, q, public, "settled residue");
        r
    }
}

/// The value of a symbolic residue (witness backends).
fn cs_value<CS: Circuit>(cs: &CS, lc: &ModLc<CS::Bool>, q: u128) -> Option<u128> {
    let mut acc = lc.constant;
    for (c, x) in &lc.terms {
        let v = match x {
            Zq::P(v) => *v,
            Zq::V(bits) => super::val::try_value(cs, bits)?,
        };
        acc = (acc + mulmod(*c, v % q, q)) % q;
    }
    Some(acc)
}

/// The Spartan sumcheck verifier (`SumcheckProof::verify_with_round_boundary`)
/// over committed round polynomials: per round the coefficients are absorbed,
/// `p(0) + p(1) ≡ claim`, the grinding boundary, the public challenge, and
/// the next claim `p(r)` kept symbolic. Returns the point and the final claim.
#[allow(clippy::too_many_arguments)]
pub fn sumcheck_verify<CS: Circuit, const C: usize>(
    cs: &mut CS,
    t: &mut SpartanTranscript<CS::Bool>,
    rounds: &[[u128; C]],
    initial: ModLc<CS::Bool>,
    cfg: &Config,
    grinding: Option<(&[u8], u32, &[u64])>,
) -> (Vec<u128>, ModLc<CS::Bool>) {
    sumcheck_rounds(cs, t, rounds, initial, cfg, grinding, false)
}

/// [`sumcheck_verify`] with unpublished grinding seeds
/// ([`SpartanTranscript::grinding_boundary_private`]).
pub fn sumcheck_verify_private<CS: Circuit, const C: usize>(
    cs: &mut CS,
    t: &mut SpartanTranscript<CS::Bool>,
    rounds: &[[u128; C]],
    initial: ModLc<CS::Bool>,
    cfg: &Config,
    grinding: Option<(&[u8], u32, &[u64])>,
) -> (Vec<u128>, ModLc<CS::Bool>) {
    sumcheck_rounds(cs, t, rounds, initial, cfg, grinding, true)
}

fn sumcheck_rounds<CS: Circuit, const C: usize>(
    cs: &mut CS,
    t: &mut SpartanTranscript<CS::Bool>,
    rounds: &[[u128; C]],
    initial: ModLc<CS::Bool>,
    cfg: &Config,
    grinding: Option<(&[u8], u32, &[u64])>,
    private_seeds: bool,
) -> (Vec<u128>, ModLc<CS::Bool>) {
    let q = modulus(cfg);
    let public = t.public().clone();
    let mut claim = initial;
    let mut point = Vec::with_capacity(rounds.len());
    for (round, coefficients) in rounds.iter().enumerate() {
        let coeffs: Vec<Zq<CS::Bool>> = coefficients.iter().map(|&c| witness_residue(cs, c, q)).collect();
        t.absorb_field_elements(cs, &coeffs);
        // c_0 + Σ_i c_i − claim ≡ 0.
        let mut check = ModLc::constant(0);
        check.add(2, &coeffs[0], q);
        for c in &coeffs[1..] {
            check.add(1, c, q);
        }
        check.add_lc(q - 1, &claim, q);
        check.assert_zero(cs, q, &public, "spartan sumcheck round");
        if let Some((domain, bits, nonces)) = grinding
            && bits > 0
        {
            if private_seeds {
                t.grinding_boundary_private(cs, domain, round as u64, bits, nonces[round]);
            } else {
                t.grinding_boundary(cs, domain, round as u64, bits, nonces[round]);
            }
        }
        let r = t.squeeze_field(cs, cfg);
        let mut next = ModLc::constant(0);
        let mut power = 1u128;
        for c in &coeffs {
            next.add(power, c, q);
            power = mulmod(power, r, q);
        }
        claim = next;
        point.push(r);
    }
    (point, claim)
}

/// The prime of a runtime field configuration.
pub fn modulus(cfg: &Config) -> u128 {
    let words = cfg.modulus().as_words();
    u128::from(words[0]) | (u128::from(words[1]) << 64)
}

/// [`crate::ligerito_flock::verify_ood_round`]: the header and the optional
/// grinding boundary, the public `ζ`, the committed `y` absorbed. Returns
/// the OOD point and `y`.
pub fn ood_round<CS: Circuit>(
    cs: &mut CS,
    t: &mut SpartanTranscript<CS::Bool>,
    packed_vars: usize,
    grinding_bits: u32,
    nonce: Option<u64>,
    y: Gf,
) -> (Vec<Gf>, G<CS::Bool>) {
    {
        let mut p = t.proxy(cs);
        p.absorb_slice(b"bitz/core/ood-round/v1");
        p.absorb_slice(&(packed_vars as u64).to_le_bytes());
        p.absorb_slice(&grinding_bits.to_le_bytes());
    }
    if grinding_bits > 0 {
        t.grinding_boundary(cs, b"bitz/core/ood-round-grinding/v1", 0, grinding_bits, nonce.expect("Round-0 nonce"));
    }
    let zeta: Gf = t.proxy(cs).get_field_challenge(&());
    let y = super::val::witness(cs, y);
    t.absorb_gf_slice(cs, 0x50, &[y.clone()]);
    (crate::ligerito_flock::ood_point(zeta, packed_vars), y)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recursion::val::PublicStream;
    use crate::transcript::Sha256Transcript;
    use crate::transcript::sha::{TraceOp, start_trace, take_trace};
    use circuit::witgen::CheckingWitgen;

    /// The proxy and the witness helpers reproduce the native transcript.
    #[test]
    fn spartan_transcript_matches_native() {
        let q = (1u128 << 100) - 15;
        let cfg = <F as SpartanField>::make_cfg(&field::Uint::from(q)).unwrap();
        let rounds: Vec<[u128; 3]> = vec![[5, 7, 11], [13, 17, 19]];
        start_trace();
        let mut native = Sha256Transcript::new();
        crate::piop::spartan::absorb_spartan_message(&mut native, b"tag", b"public payload");
        let mut fixed = Vec::new();
        for coeffs in &rounds {
            let values: Vec<F> = coeffs.iter().map(|&c| F::from_with_cfg(c, &cfg)).collect();
            crate::piop::spartan::absorb_field_elements(&mut native, &values, &cfg);
            fixed.push(canonical(&crate::piop::spartan::squeeze_field::<F, _>(&mut native, &cfg).unwrap(), &cfg));
        }
        crate::ligerito::absorb_ood_value(&mut native, Gf::new(3, 9));
        let expected: Vec<TraceOp> = take_trace().into_iter().map(|e| e.op).collect();

        let public = PublicStream::discover();
        let mut w = CheckingWitgen::new();
        let mut t = SpartanTranscript::new(public);
        t.duplex = t.duplex.clone().with_log();
        crate::piop::spartan::absorb_spartan_message(&mut t.proxy(&mut w), b"tag", b"public payload");
        let mut got = Vec::new();
        for coeffs in &rounds {
            let values: Vec<Zq<bool>> = coeffs.iter().map(|&c| witness_residue(&mut w, c, q)).collect();
            t.absorb_field_elements(&mut w, &values);
            got.push(t.squeeze_field(&mut w, &cfg));
        }
        let y = crate::recursion::val::witness(&mut w, Gf::new(3, 9));
        t.absorb_gf_slice(&mut w, 0x50, &[y]);
        assert_eq!(got, fixed);
        let log = t.duplex.log.clone().unwrap();
        assert_eq!(crate::recursion::sha::first_divergence(&expected, &log), None);
        assert_eq!(w.first_failure, None);
    }
}
