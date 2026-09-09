//! One ring switch and Ligerito continuation, with two-root authentication.
use super::{Error, Gf};
use crate::{
    ligerito::{RingSwitchProof, residual_b_evals, ring_switch_prove, ring_switch_verify},
    ligerito_flock::{ZincChallenger, custom_udr_grind_config_bits, f128_to_gf, gf_to_f128},
    transcript::Blake3Transcript,
};
use flock_core::{
    field::F128 as F,
    merkle::{self, Hash},
    pcs::{
        commit::{PcsParams, ProverData},
        ligerito::{self, LigeritoProof, LigeritoSecurityConfig, RecursiveProof},
    },
};

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
    pub fn params(&self, branch: usize) -> PcsParams {
        PcsParams {
            m: self.logs[branch] + 7,
            log_inv_rate: 1,
            log_batch_size: self.lane_logs[branch],
            profile: ligerito::LigeritoProfile::Secure,
            merkle_hash: merkle::HashKind::Blake3,
        }
    }
    pub fn security(&self) -> LigeritoSecurityConfig {
        let mut config =
            custom_udr_grind_config_bits(self.bit_log(), 1, self.virtual_lane_log, Some(112));
        config.hash = "blake3".into();
        config
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
    pub ring: RingSwitchProof,
    pub ligerito: LigeritoProof,
    pub paths: [Vec<Hash>; 2],
}

pub(super) fn prove(
    t: &mut Blake3Transcript,
    geometry: &Geometry,
    statement: &Hash,
    sources: [&[F]; 2],
    data: [&ProverData; 2],
    point: &[Gf],
) -> Result<Proof, Error> {
    let packed = geometry.virtual_packed(sources);
    let words: Vec<_> = packed.iter().copied().map(f128_to_gf).collect();
    let (ring, basis, target) = ring_switch_prove(t, &words, &point[7..]);
    drop(words);
    let (pc, _) = geometry
        .security()
        .to_prover_verifier_configs()
        .map_err(Error::Config)?;
    let mut paths = [Vec::new(), Vec::new()];
    let proof = ligerito::recursive_prover_with_basis_initial(
        &pc,
        packed,
        basis.into_iter().map(gf_to_f128).collect(),
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
        ring,
        ligerito: proof,
        paths,
    })
}

pub(super) fn verify(
    t: &mut Blake3Transcript,
    geometry: &Geometry,
    statement: &Hash,
    roots: &[Hash; 2],
    point: &[Gf],
    value: F,
    proof: &Proof,
) -> Result<(), Error> {
    let (eq_r2, target) = ring_switch_verify(t, &proof.ring, f128_to_gf(value), &point[..7])
        .map_err(|_| Error::Invalid("ring switch"))?;
    let (_, vc) = geometry
        .security()
        .to_prover_verifier_configs()
        .map_err(Error::Config)?;
    if proof.ligerito.recursive_roots.len() != vc.recursive_steps
        || proof.ligerito.recursive_proofs.len() + 1 != vc.recursive_steps
        || proof.ligerito.grinding_nonces.len() != vc.recursive_steps + 1
        || !proof.ligerito.ood_values.is_empty()
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
            let prefix: Vec<_> = prefix.iter().copied().map(f128_to_gf).collect();
            residual_b_evals(&prefix, log_y, &point[7..], &eq_r2)
                .into_iter()
                .map(gf_to_f128)
                .collect()
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
