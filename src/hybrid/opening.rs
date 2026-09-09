//! Round 0 (the out-of-domain sample), one ring switch and one Ligerito
//! continuation, with two-root authentication.
//!
//! The shared opener runs in the Johnson (list-decoding) regime at rate
//! 1/8. The paper's theorem covers that regime only with Round 0: right
//! after the statement, before any other challenge, the prover sends
//! `y = MLE[V](ζ⃗)` of the virtual packed witness `V` at a transcript-drawn
//! `ζ⃗ = (ζ, ζ², ζ⁴, …)`, which pins it to one element of the level-0
//! list before the first forest or PIOP challenge. The claim is `K`-linear
//! in `V`, so it rides the final opening: one extra batching draw `η_ood`
//! adds `η_ood·eq(·, ζ⃗)` to the Ligerito basis and `η_ood·y` to its
//! target, and the verifier folds that term succinctly. Every primitive is
//! the audited one of [`crate::ligerito_flock`] (Round 0 of the paper's
//! core IOP); nothing here re-derives a bound.
use super::{CompositionProfile, Error, Gf, security::LIGERITO_COMPONENT_BITS};
use crate::{
    ligerito::{RingSwitchProof, residual_b_evals, ring_switch_prove, ring_switch_verify},
    ligerito_flock::{
        OodProverClaim, OodRound, OodRoundParams, OodVerifierClaim, ZincChallenger, add_ood_basis,
        custom_johnson_config_bits, f128_to_gf, gf_to_f128, ood_residual_evals, ood_round_bits,
        ood_round_params, prove_ood_round_packed, verify_ood_round,
    },
    piop::spartan::profile::{IopSecurityProfile, MAX_DERIVED_GRINDING_BITS},
    transcript::{Blake3Transcript, traits::Transcript},
};
use flock_core::{
    field::F128 as F,
    merkle::{self, Hash},
    pcs::{
        commit::{PcsParams, ProverData},
        ligerito::{self, LigeritoProof, LigeritoSecurityConfig, RecursiveProof},
    },
};

/// Reed–Solomon inverse-rate exponent shared by both initial commitments
/// and the opener's level 0 (rate 1/8). The commit rate MUST equal the
/// level-0 configuration rate: the opener queries the committed codewords.
pub(super) const LOG_INV_RATE: usize = 3;

#[derive(Clone, Debug)]
pub(super) struct Geometry {
    pub logs: [usize; 2],
    pub position_log: usize,
    pub lane_logs: [usize; 2],
    pub virtual_lane_log: usize,
}

impl Geometry {
    pub fn new(logs: [usize; 2]) -> Result<Self, Error> {
        if logs.iter().any(|&n| !(9..=27).contains(&n)) {
            return Err(Error::Invalid("packed witness logarithm outside 9..=27"));
        }
        // Cap the virtual leaf width so strongly unbalanced workloads still
        // fit the proof codec's 64 MiB budget. Moving coordinates from lanes
        // to positions preserves the same two-root construction.
        let position_log = (logs[0].min(logs[1]) - 4)
            .max(9)
            .max(logs[0].max(logs[1]).saturating_sub(11));
        if position_log > logs[0].min(logs[1]) {
            return Err(Error::Invalid(
                "source sizes require excessive virtual padding",
            ));
        }
        let lane_logs = [logs[0] - position_log, logs[1] - position_log];
        let virtual_lane_log = lane_logs[0].max(lane_logs[1]) + 1;
        Ok(Self {
            logs,
            position_log,
            lane_logs,
            virtual_lane_log,
        })
    }
    pub fn packed_log(&self) -> usize {
        self.position_log + self.virtual_lane_log
    }
    pub fn bit_log(&self) -> usize {
        self.packed_log() + 7
    }
    pub fn lanes(&self) -> usize {
        1 << self.virtual_lane_log
    }
    pub fn offset(&self, branch: usize) -> usize {
        branch << (self.virtual_lane_log - 1)
    }
    pub fn embed(&self, branch: usize, original: usize) -> usize {
        let k = self.lane_logs[branch];
        ((original >> k) << self.virtual_lane_log)
            + self.offset(branch)
            + (original & ((1 << k) - 1))
    }
    pub fn project_point(&self, branch: usize, point: &[F]) -> (Vec<F>, F) {
        let k = self.lane_logs[branch];
        let mut original = point[..7 + k].to_vec();
        original.extend_from_slice(&point[7 + self.virtual_lane_log..]);
        let mut padding = F::ONE;
        for j in k..self.virtual_lane_log {
            padding *= if branch == 1 && j == self.virtual_lane_log - 1 {
                point[7 + j]
            } else {
                F::ONE + point[7 + j]
            };
        }
        (original, padding)
    }
    /// Commitment parameters of one branch. `profile` is inert here: the
    /// hybrid path never consults flock's embedded profiles, its opener
    /// configuration is [`Geometry::security`]; `commit` reads only `m`,
    /// `log_inv_rate`, `log_batch_size` and `merkle_hash`.
    pub fn params(&self, branch: usize) -> PcsParams {
        PcsParams {
            m: self.logs[branch] + 7,
            log_inv_rate: LOG_INV_RATE,
            log_batch_size: self.lane_logs[branch],
            profile: ligerito::LigeritoProfile::Secure,
            merkle_hash: merkle::HashKind::Blake3,
        }
    }
    /// The shared opener: a Johnson-regime Ligerito configuration for the
    /// VIRTUAL geometry (`initial_k = virtual_lane_log` is forced by the
    /// two-root construction) at rate 1/8 and the composition's component
    /// target, solved and validated by flock's own machinery.
    pub fn security(&self) -> LigeritoSecurityConfig {
        let mut config = custom_johnson_config_bits(
            self.bit_log(),
            LOG_INV_RATE,
            self.virtual_lane_log,
            Some(LIGERITO_COMPONENT_BITS),
        );
        config.hash = "blake3".into();
        config
    }
    /// Round-0 parameters, derived exactly as
    /// [`crate::piop::spartan::profile::IopSecurityParams::adopt_ood_round`]
    /// derives them for the standalone relations: the theorem's collision
    /// bound at the opener's level-0 Johnson parameters
    /// ([`ood_round_bits`]) topped up to the composition profile's λ by
    /// proof of work, under the same economic cap. Returns the bound (in
    /// bits, before grinding) and the parameters.
    pub fn ood(&self) -> Result<(f64, OodRoundParams), Error> {
        let config = self.security();
        let packed_vars = self.packed_log();
        let bits = ood_round_bits(&config, packed_vars).ok_or_else(|| {
            Error::Config("the shared opener must run in the Johnson regime with Round 0".into())
        })?;
        let params = ood_round_params(&config, packed_vars, CompositionProfile::LAMBDA)
            .expect("Johnson regime: Round 0 parameters exist");
        if params.grinding_bits > MAX_DERIVED_GRINDING_BITS {
            return Err(Error::Config(format!(
                "step0:ood-draw needs {} grinding bits, above the {}-bit cap",
                params.grinding_bits, MAX_DERIVED_GRINDING_BITS
            )));
        }
        Ok((bits, params))
    }
    pub fn virtual_packed(&self, sources: [&[F]; 2]) -> Vec<F> {
        let mut out = vec![F::ZERO; 1 << self.packed_log()];
        for branch in 0..2 {
            for (index, &word) in sources[branch].iter().enumerate() {
                out[self.embed(branch, index)] = word;
            }
        }
        out
    }
}

#[derive(Clone, Debug)]
pub(super) struct Proof {
    /// Round 0: `y = MLE[V](ζ⃗)` and the proof-of-work nonce before the
    /// `ζ` draw.
    pub ood: OodRound,
    pub ring: RingSwitchProof,
    pub ligerito: LigeritoProof,
    pub paths: [Vec<Hash>; 2],
}

/// Round 0 on the prover side, on the virtual packed witness `packed`.
/// Must run right after the statement, before any other challenge.
pub(super) fn prove_ood(
    t: &mut Blake3Transcript,
    params: OodRoundParams,
    packed: &[F],
) -> OodProverClaim {
    prove_ood_round_packed(t, packed, params)
}

/// Round 0 on the verifier side: the same frame and draw, the nonce
/// checked against `params`, the prover's `y` absorbed.
pub(super) fn verify_ood(
    t: &mut Blake3Transcript,
    geometry: &Geometry,
    params: OodRoundParams,
    round: &OodRound,
) -> Result<OodVerifierClaim, Error> {
    verify_ood_round(t, geometry.packed_log(), params, round)
        .map_err(|_| Error::Invalid("Round 0 (out-of-domain sample)"))
}

pub(super) fn prove(
    t: &mut Blake3Transcript,
    geometry: &Geometry,
    statement: &Hash,
    packed: Vec<F>,
    ood: &OodProverClaim,
    data: [&ProverData; 2],
    point: &[Gf],
) -> Result<Proof, Error> {
    let words: Vec<_> = packed.iter().copied().map(f128_to_gf).collect();
    let (ring, basis, mut target) = ring_switch_prove(t, &words, &point[7..]);
    drop(words);
    // Batch the Round-0 claim into the same opening: one draw adds
    // `η_ood·eq(·, ζ⃗)` to the basis and `η_ood·y` to the target.
    let eta_ood: Gf = t.get_field_challenge(&());
    let mut basis: Vec<F> = basis.into_iter().map(gf_to_f128).collect();
    add_ood_basis(&mut basis, &packed, &ood.point, eta_ood, None);
    target += eta_ood * ood.y;
    let (pc, _) = geometry
        .security()
        .to_prover_verifier_configs()
        .map_err(Error::Config)?;
    let mut paths = [Vec::new(), Vec::new()];
    let proof = ligerito::recursive_prover_with_basis_initial(
        &pc,
        packed,
        basis,
        gf_to_f128(target),
        *statement,
        |positions, lanes, queries| {
            let mut rows = vec![vec![F::ZERO; lanes]; queries.len()];
            for branch in 0..2 {
                let width = 1 << geometry.lane_logs[branch];
                let start = geometry.offset(branch);
                for (row, &q) in rows.iter_mut().zip(queries) {
                    row[start..start + width]
                        .copy_from_slice(&data[branch].codeword[q * width..(q + 1) * width]);
                }
                paths[branch] =
                    merkle::merkle_multi_proof(&data[branch].merkle_tree, positions, queries);
            }
            RecursiveProof {
                opened_rows: rows,
                merkle_proof: Vec::new(),
            }
        },
        &mut ZincChallenger(t),
    );
    Ok(Proof {
        ood: ood.round,
        ring,
        ligerito: proof,
        paths,
    })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn verify(
    t: &mut Blake3Transcript,
    geometry: &Geometry,
    statement: &Hash,
    roots: &[Hash; 2],
    point: &[Gf],
    value: F,
    ood: &OodVerifierClaim,
    proof: &Proof,
) -> Result<(), Error> {
    let (eq_r2, mut target) = ring_switch_verify(t, &proof.ring, f128_to_gf(value), &point[..7])
        .map_err(|_| Error::Invalid("ring switch"))?;
    let eta_ood: Gf = t.get_field_challenge(&());
    target += eta_ood * ood.y;
    let (_, vc) = geometry
        .security()
        .to_prover_verifier_configs()
        .map_err(Error::Config)?;
    // Deeper levels take explicit out-of-domain samples and grind their
    // fold challenges (tapered one bit per round, as in flock's prover);
    // level 0's binding is Round 0. flock's verifier consumes both
    // vectors exactly and rejects leftovers; the counts are fixed here as
    // well so a malformed proof fails on shape, not deep inside.
    let expected_ood_values: usize = vc.ood_samples.iter().skip(1).sum();
    let level_ks = std::iter::once(vc.initial_k).chain(vc.recursive_ks.iter().copied());
    let expected_fold_nonces: usize = vc
        .fold_grinding_bits
        .iter()
        .zip(level_ks)
        .map(|(&bits, k)| (0..k).filter(|&j| bits.saturating_sub(j) > 0).count())
        .sum();
    if proof.ligerito.recursive_roots.len() != vc.recursive_steps
        || proof.ligerito.recursive_proofs.len() + 1 != vc.recursive_steps
        || proof.ligerito.grinding_nonces.len() != vc.recursive_steps + 1
        || proof.ligerito.ood_values.len() != expected_ood_values
        || proof.ligerito.fold_grinding_nonces.len() != expected_fold_nonces
    {
        return Err(Error::Invalid("Ligerito proof shape"));
    }
    let valid = ligerito::recursive_verifier_with_basis_initial(
        &vc,
        &proof.ligerito,
        geometry.packed_log(),
        gf_to_f128(target),
        statement,
        |prefix, log_y| {
            let prefix_gf: Vec<_> = prefix.iter().copied().map(f128_to_gf).collect();
            let mut out = residual_b_evals(&prefix_gf, log_y, &point[7..], &eq_r2);
            let add = ood_residual_evals(prefix, log_y, &ood.point, eta_ood);
            for (slot, term) in out.iter_mut().zip(add) {
                *slot += term;
            }
            out.into_iter().map(gf_to_f128).collect()
        },
        |positions, lanes, queries, opening| {
            if !opening.merkle_proof.is_empty() || lanes != geometry.lanes() {
                return false;
            }
            let mut hashes = [
                Vec::with_capacity(queries.len()),
                Vec::with_capacity(queries.len()),
            ];
            for row in &opening.opened_rows {
                // Authentication of both real slices AND every public zero lane.
                for branch in 0..2 {
                    let start = geometry.offset(branch);
                    let end = start + (1 << geometry.lane_logs[branch]);
                    let half_end = start + lanes / 2;
                    if row[end..half_end].iter().any(|&x| x != F::ZERO) {
                        return false;
                    }
                    let mut bytes = Vec::with_capacity((end - start) * 16);
                    for word in &row[start..end] {
                        bytes.extend_from_slice(&word.lo.to_le_bytes());
                        bytes.extend_from_slice(&word.hi.to_le_bytes());
                    }
                    hashes[branch].push(merkle::hash_leaf(&bytes, vc.merkle_hash));
                }
            }
            (0..2).all(|branch| {
                merkle::verify_merkle_multi_proof(
                    &roots[branch],
                    positions,
                    queries,
                    &hashes[branch],
                    &proof.paths[branch],
                    vc.merkle_hash,
                )
            })
        },
        &mut ZincChallenger(t),
    );
    if valid {
        Ok(())
    } else {
        Err(Error::Invalid("two-root Ligerito opening"))
    }
}
