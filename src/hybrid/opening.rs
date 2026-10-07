//! Round 0 (the out-of-domain sample), one ring switch and one Ligerito
//! continuation, authenticating disjoint slices from each source commitment.
//!
//! The shared opener runs in the Johnson (list-decoding) regime, at rate
//! 1/2 by default (rate 1/8 selectable through the prepared Ligerito
//! selection). The paper's theorem covers that regime only with Round 0: right
//! after the statement, before any other challenge, the prover sends
//! `y = MLE[V](ζ⃗)` of the virtual packed witness `V` at a transcript-drawn
//! `ζ⃗ = (ζ, ζ², ζ⁴, …)`, which pins it to one element of the level-0
//! list before the first forest or PIOP challenge. The claim is `K`-linear
//! in `V`, so it rides the final opening: one extra batching draw `η_ood`
//! adds `η_ood·eq(·, ζ⃗)` to the Ligerito basis and `η_ood·y` to its
//! target, and the verifier folds that term succinctly. Every primitive is
//! the audited one of [`crate::ligerito_flock`] (Round 0 of the paper's
//! core IOP); nothing here re-derives a bound.
pub(crate) mod grinding;
mod streamed;

use super::{CompositionProfile, Error, Gf};
use crate::{
    ligerito::{
        RingSwitchProof, phi_byte_tables, phi_from_words, residual_b_evals,
        ring_switch_prove_marginal, ring_switch_prove_with, ring_switch_verify, sv_fold_eq_tiled,
    },
    ligerito_flock::{
        OodProverClaim, OodRound, OodRoundParams, OodVerifierClaim, ZincChallenger, add_ood_basis,
        ood_residual_evals, prove_ood_round_packed, verify_ood_round,
    },
    piop::spartan::profile::{IopSecurityProfile, MAX_DERIVED_GRINDING_BITS},
    transcript::traits::Transcript,
};
use flock_core::{
    challenger::Challenger,
    field::Gf128 as F,
    merkle::{self, Hash},
    pcs::{
        commit::{PcsParams, ProverData},
        ligerito::{self, LigeritoProof, RecursiveProof},
    },
};
#[cfg(feature = "parallel")]
use rayon::prelude::*;

/// Default Reed–Solomon inverse-rate exponent of the shared opener
/// (rate 1/2). The effective rate is the prepared Ligerito selection's
/// level-0 rate — [`Geometry::params`] takes it explicitly — and the commit
/// rate MUST equal that level-0 configuration rate: the opener queries the
/// committed codewords.
pub(crate) const LOG_INV_RATE: usize = 1;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Geometry<const N: usize = 2> {
    pub logs: [usize; N],
    pub physical_logs: [usize; N],
    pub position_log: usize,
    pub lane_logs: [usize; N],
    pub virtual_lane_log: usize,
    pub offsets: [usize; N],
}

impl<const N: usize> Geometry<N> {
    pub fn new(logs: [usize; N]) -> Result<Self, Error> {
        if N == 0 || logs.iter().any(|&n| !(9..=27).contains(&n)) {
            return Err(Error::Invalid("packed witness logarithm outside 9..=27"));
        }
        let mut position_log = *logs.iter().max().expect("nonempty geometry") - 3;
        // Additional sources can require a taller common row domain. Keep the
        // established sixteen virtual lanes, shrinking each branch's lane slice
        // until all sources fit. Two-source layouts retain their exact geometry.
        if N > 16 {
            return Err(Error::Invalid("too many shared opening sources"));
        }
        while N != 2
            && logs
                .iter()
                .map(|&l| 1usize << l.saturating_sub(position_log))
                .sum::<usize>()
                > 16
        {
            position_log += 1;
        }
        let physical_logs = logs.map(|l| l.max(position_log));
        let lane_logs = physical_logs.map(|l| l - position_log);
        let virtual_lane_log = 4;
        let mut offsets = [0; N];
        if N == 2 {
            // Preserve the established two-root layout and transcript.
            offsets[1] = 8;
        } else {
            // Largest first keeps every power-of-two slice aligned. Falcon's
            // A/K16/K4 widths depend on the prepared circuit capacities.
            let mut order: Vec<_> = (0..N).collect();
            order.sort_by_key(|&i| std::cmp::Reverse(lane_logs[i]));
            let mut next = 0;
            for branch in order {
                offsets[branch] = next;
                next += 1 << lane_logs[branch];
            }
            if next > 16 {
                return Err(Error::Invalid("too many shared opening lanes"));
            }
        }
        Ok(Self {
            logs,
            physical_logs,
            position_log,
            lane_logs,
            virtual_lane_log,
            offsets,
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
        self.offsets[branch]
    }
    #[cfg(test)]
    pub fn embed(&self, branch: usize, original: usize) -> usize {
        let k = self.lane_logs[branch];
        ((original >> k) << self.virtual_lane_log)
            + self.offset(branch)
            + (original & ((1 << k) - 1))
    }
    pub fn project_point(&self, branch: usize, point: &[F]) -> (Vec<F>, F) {
        let k = self.lane_logs[branch];
        let mut original = point[..7 + k].to_vec();
        let high = self.logs[branch] - k;
        original.extend_from_slice(&point[11..11 + high]);
        let mut padding = F::ONE;
        for j in k..self.virtual_lane_log {
            padding *= if self.offset(branch) >> j & 1 == 1 {
                point[7 + j]
            } else {
                F::ONE + point[7 + j]
            };
        }
        for &r in &point[11 + high..] {
            padding *= F::ONE + r;
        }
        (original, padding)
    }
    /// Commitment parameters of one branch, committing at `log_inv_rate` —
    /// the prepared Ligerito selection's level-0 rate. `profile` is inert
    /// here: the hybrid path never consults flock's embedded profiles, its
    /// opener configuration is the prepared Ligerito resolver; `commit` reads
    /// only `m`, `log_inv_rate`, `log_batch_size` and `merkle_hash`.
    pub fn params(&self, branch: usize, log_inv_rate: usize) -> PcsParams {
        PcsParams {
            m: self.physical_logs[branch] + 7,
            log_inv_rate,
            log_batch_size: self.lane_logs[branch],
            profile: ligerito::LigeritoProfile::Secure,
            merkle_hash: merkle::HashKind::Blake3,
        }
    }
    /// H_r = eq(r,.) minus its restrictions to the disjoint logical supports.
    /// In characteristic two subtraction is addition. The padding mask needs
    /// N+1 equality bases and no dense mask.
    fn padding_bases(&self, r: Vec<Gf>, eta: Gf) -> Vec<(Vec<Gf>, Gf)> {
        let mut bases = vec![(r.clone(), eta)];
        for branch in 0..N {
            let k = self.lane_logs[branch];
            let high = self.logs[branch] - k;
            let mut clamped = r.clone();
            let mut scale = eta;
            for coordinate in (k..4).chain(4 + high..self.packed_log()) {
                let bit = coordinate < 4 && self.offset(branch) >> coordinate & 1 == 1;
                scale *= if bit {
                    r[coordinate]
                } else {
                    Gf::one() + r[coordinate]
                };
                clamped[coordinate] = if bit { Gf::one() } else { Gf::zero() };
            }
            bases.push((clamped, scale));
        }
        bases
    }

    /// Add the sum of `padding_bases(point, eta)` without traversing the
    /// virtual domain once per branch. At Boolean indices the restricted
    /// bases cancel the full equality basis exactly on the disjoint logical
    /// supports, leaving `eta * eq(point, index)` only on padding.
    fn add_padding_basis(&self, basis: &mut [F], point: &[Gf], eta: Gf) {
        use super::sumcheck::eq_table;

        assert_eq!(point.len(), self.packed_log());
        assert_eq!(basis.len(), 1 << point.len());
        let lanes = self.lanes();
        let mut live_groups = vec![0usize; lanes];
        let mut min_group_log = self.position_log;
        for branch in 0..N {
            let group_log = self.logs[branch] - self.lane_logs[branch];
            min_group_log = min_group_log.min(group_log);
            let start = self.offset(branch);
            let width = 1 << self.lane_logs[branch];
            live_groups[start..start + width].fill(1 << group_log);
        }
        // Every support boundary is a multiple of the block size in groups,
        // so a lane is either live throughout a block or padded throughout.
        // Splitting the equality table also avoids another full-domain table.
        let block_log = self
            .packed_log()
            .min(12)
            .min(self.virtual_lane_log + min_group_log);
        let block = 1 << block_log;
        let tail = eq_table(&point[..block_log]);
        let mut head = eq_table(&point[block_log..]);
        for scale in &mut head {
            *scale *= eta;
        }
        crate::utils::cfg_chunks_mut!(basis, block)
            .enumerate()
            .for_each(|(hi, chunk)| {
                let first_group = hi * (block / lanes);
                let mut padded_lanes = [0usize; 16];
                let mut count = 0;
                for (lane, &live) in live_groups.iter().enumerate() {
                    if first_group >= live {
                        padded_lanes[count] = lane;
                        count += 1;
                    }
                }
                if count == 0 {
                    return;
                }
                let scale = head[hi];
                for (group, values) in chunk.chunks_exact_mut(lanes).enumerate() {
                    for &lane in &padded_lanes[..count] {
                        values[lane] += scale * tail[group * lanes + lane];
                    }
                }
            });
    }

    pub fn virtual_packed(&self, sources: [&[F]; N]) -> Vec<F> {
        let lanes = self.lanes();
        let mut out = vec![F::ZERO; 1 << self.packed_log()];
        // One lane group per position: `embed` places word `(g << k) | l`
        // of branch `b` at lane `offset(b) + l` of group `g`.
        crate::utils::cfg_chunks_mut!(out, lanes)
            .enumerate()
            .for_each(|(g, group)| {
                for branch in 0..N {
                    let k = self.lane_logs[branch];
                    let start = self.offset(branch);
                    let words = &sources[branch][g << k..(g + 1) << k];
                    group[start..start + words.len()].copy_from_slice(words);
                }
            });
        out
    }

    /// The virtual equality weight restricted to a physical branch is a
    /// scalar times an equality weight on that branch. Include its *physical*
    /// padding: the separately sampled padding claim authenticates that it is
    /// zero, so discarding these coefficients here would change the protocol.
    fn ring_marginal(&self, sources: [&[F]; N], point: &[Gf]) -> Vec<Gf> {
        assert_eq!(point.len(), self.packed_log());
        let mut marginal = vec![Gf::zero(); 128];
        for branch in 0..N {
            assert_eq!(sources[branch].len(), 1 << self.physical_logs[branch]);
            let k = self.lane_logs[branch];
            let mut branch_point = point[..k].to_vec();
            branch_point.extend_from_slice(&point[self.virtual_lane_log..]);
            let mut scale = Gf::one();
            for (coordinate, &r) in point.iter().enumerate().take(4).skip(k) {
                scale *= if self.offset(branch) >> coordinate & 1 == 1 {
                    r
                } else {
                    Gf::one() + r
                };
            }
            let partial = sv_fold_eq_tiled(sources[branch], &branch_point);
            for (value, term) in marginal.iter_mut().zip(partial) {
                *value += scale * term;
            }
        }
        marginal
    }

    fn live_groups(&self) -> [usize; 16] {
        let mut live = [0; 16];
        for branch in 0..N {
            let k = self.lane_logs[branch];
            live[self.offset(branch)..self.offset(branch) + (1 << k)]
                .fill(1 << (self.logs[branch] - k));
        }
        live
    }

    /// Copy every physically committed word, including source padding. The
    /// caller zero-initializes unused lanes when allocating the tile; reuse
    /// leaves those lanes untouched.
    fn write_words(&self, sources: [&[F]; N], first_group: usize, words: &mut [F]) {
        for (local, group) in words.chunks_exact_mut(self.lanes()).enumerate() {
            let g = first_group + local;
            for branch in 0..N {
                let k = self.lane_logs[branch];
                let start = self.offset(branch);
                group[start..start + (1 << k)]
                    .copy_from_slice(&sources[branch][g << k..(g + 1) << k]);
            }
        }
    }

    /// Materialize the two tables needed by Ligerito exactly once. Equality
    /// factors fit in tiles; ring, OOD and padding coefficients are combined
    /// before being written. The same pass computes the first message and
    /// next-round lookahead, eliminating the full-size entry fold/read pass.
    fn initial_tables(
        &self,
        sources: [&[F]; N],
        point: &[Gf],
        eq_r2: &[Gf],
        padding: (&[Gf], Gf),
        ood: Option<(&[Gf], Gf)>,
    ) -> InitialTables {
        use flock_core::field::Gf128Product;

        let tiles = InitialBasisTiles::new(self, point, eq_r2, padding, ood);
        let block = tiles.block_len();
        let size = 1 << self.packed_log();
        let mut packed = vec![F::ZERO; size];
        let mut basis = vec![F::ZERO; size];
        let partials: Vec<_> = crate::utils::cfg_chunks_mut!(packed, block)
            .zip(crate::utils::cfg_chunks_mut!(basis, block))
            .enumerate()
            .map(|(hi, (words, coefficients))| {
                self.write_words(sources, hi * block / self.lanes(), words);
                tiles.write(hi, coefficients);
                ligerito::lookahead_accumulate(words, coefficients)
            })
            .collect();
        let mut acc = [Gf128Product::zero(); 8];
        for partial in partials {
            for (value, term) in acc.iter_mut().zip(partial) {
                *value ^= term;
            }
        }
        let (first_message, lookahead) = ligerito::lookahead_finish(acc);
        InitialTables {
            packed,
            basis,
            first_message,
            lookahead,
        }
    }
}

struct EqualityTiles {
    tail: Vec<Gf>,
    head: Vec<Gf>,
}

impl EqualityTiles {
    fn new(point: &[Gf], low: usize, scale: Gf) -> Self {
        let tail = super::sumcheck::eq_table(&point[..low]);
        let mut head = super::sumcheck::eq_table(&point[low..]);
        for value in &mut head {
            *value *= scale;
        }
        Self { tail, head }
    }
}

/// Initial basis coefficients shared by dense and streamed PCS tables.
/// Allocation and the deferred two-fold basis remain separate from this tile writer.
struct InitialBasisTiles {
    ring: EqualityTiles,
    padding: EqualityTiles,
    ood: Option<EqualityTiles>,
    phi: Vec<Gf>,
    live_groups: [usize; 16],
}

impl InitialBasisTiles {
    fn new<const N: usize>(
        geometry: &Geometry<N>,
        point: &[Gf],
        eq_r2: &[Gf],
        padding: (&[Gf], Gf),
        ood: Option<(&[Gf], Gf)>,
    ) -> Self {
        let low = geometry.packed_log().min(12);
        Self {
            ring: EqualityTiles::new(point, low, Gf::one()),
            padding: EqualityTiles::new(padding.0, low, padding.1),
            ood: ood.map(|(point, scale)| EqualityTiles::new(point, low, scale)),
            phi: phi_byte_tables(eq_r2, Gf::one()),
            live_groups: geometry.live_groups(),
        }
    }

    fn block_len(&self) -> usize {
        self.ring.tail.len()
    }

    fn tile_count(&self) -> usize {
        self.ring.head.len()
    }

    fn write(&self, hi: usize, coefficients: &mut [Gf]) {
        let lanes = self.live_groups.len();
        let first_group = hi * self.block_len() / lanes;
        for (lo, coefficient) in coefficients.iter_mut().enumerate() {
            let weight = self.ring.head[hi] * self.ring.tail[lo];
            let mut value = phi_from_words(*weight.as_words(), &self.phi);
            if first_group + lo / lanes >= self.live_groups[lo % lanes] {
                value += self.padding.head[hi] * self.padding.tail[lo];
            }
            if let Some(ood) = &self.ood {
                value += ood.head[hi] * ood.tail[lo];
            }
            *coefficient = value;
        }
    }
}

struct InitialTables {
    packed: Vec<F>,
    basis: Vec<F>,
    first_message: ligerito::SumcheckMessage,
    lookahead: ligerito::FoldLookahead,
}

#[cfg(test)]
mod geometry_tests {
    use super::*;
    use crate::hybrid::sumcheck::eq_table;

    fn sources<const N: usize>(geometry: &Geometry<N>) -> [Vec<F>; N] {
        std::array::from_fn(|branch| {
            (0..1 << geometry.physical_logs[branch])
                .map(|i| {
                    if i < 1 << geometry.logs[branch] {
                        F {
                            lo: (i as u64 + 1).wrapping_mul(0x9e3779b97f4a7c15),
                            hi: (i as u64 + branch as u64 + 7).wrapping_mul(0x85ebca6b27d4eb2f),
                        }
                    } else {
                        F::ZERO
                    }
                })
                .collect()
        })
    }

    fn bit_evaluation(words: &[F], point: &[F]) -> F {
        let weights: [F; 128] = eq_table(&point[..7]).try_into().unwrap();
        let low = super::super::sumcheck::byte_table(&weights);
        words
            .iter()
            .zip(eq_table(&point[7..]))
            .fold(F::ZERO, |sum, (&word, weight)| {
                sum + super::super::sumcheck::apply(&low, word) * weight
            })
    }

    #[test]
    fn joint_source_encoding_and_paths_match_dense_reference() {
        use flock_core::pcs::commit::commit;

        // Cover Falcon's ordinary and batch-one widths, plus physical padding.
        for logs in [[9, 12, 10], [10, 13, 12], [9, 13, 10]] {
            let geometry = Geometry::new(logs).unwrap();
            let sources = sources(&geometry);
            let borrowed = sources.each_ref().map(Vec::as_slice);
            let packed = geometry.virtual_packed(borrowed);
            for rate in [1, 2, 3] {
                let (root, data) = commit_sources(&geometry, borrowed, rate).unwrap();
                let params = PcsParams {
                    m: geometry.bit_log(),
                    log_inv_rate: rate,
                    log_batch_size: geometry.virtual_lane_log,
                    profile: ligerito::LigeritoProfile::Secure,
                    merkle_hash: merkle::HashKind::Blake3,
                };
                let (dense, reference) = commit(&packed, &params);
                assert_eq!(root, dense.root, "logs={logs:?}, rate={rate}");
                assert_eq!(data.merkle_tree, reference.merkle_tree);
                let positions = params.n_positions();
                let queries = [0, 1, positions / 2, positions - 1];
                let opening = data.open(positions, geometry.lanes(), &queries);
                for (&q, row) in queries.iter().zip(&opening.opened_rows) {
                    assert_eq!(row, &reference.codeword[q * 16..(q + 1) * 16]);
                }
                assert!(authenticate_joint(
                    &geometry,
                    &root,
                    positions,
                    16,
                    &queries,
                    &opening,
                    params.merkle_hash
                ));
                assert_eq!(
                    opening.merkle_proof,
                    merkle::merkle_multi_proof(&reference.merkle_tree, positions, &queries)
                );
            }
        }
    }

    #[test]
    fn compact_source_rows_reconstruct_canonical_openings() {
        for (logs, expected_lanes) in [([9, 12, 10], 11), ([10, 13, 12], 13), ([9, 13, 10], 10)] {
            let geometry = Geometry::new(logs).unwrap();
            let sources = sources(&geometry);
            let (root, data) =
                commit_sources(&geometry, sources.each_ref().map(Vec::as_slice), 1).unwrap();
            let positions = 1 << (geometry.position_log + 1);
            let queries = [0, 1, positions / 2, positions - 1];
            let original = data.open(positions, geometry.lanes(), &queries);
            let mut compact = original.clone();
            compact_initial_rows(&geometry, &mut compact).unwrap();
            let occupied = occupied_lanes(&geometry);
            assert_eq!(occupied.len(), expected_lanes);
            assert!(
                compact
                    .opened_rows
                    .iter()
                    .all(|row| row.len() == occupied.len())
            );
            let expanded = expand_initial_rows(&geometry, &compact, queries.len(), 1).unwrap();
            assert_eq!(expanded, original);
            let valid = |opening: &RecursiveProof| {
                expand_initial_rows(&geometry, opening, queries.len(), 1).is_ok_and(|full| {
                    authenticate_joint(
                        &geometry,
                        &root,
                        positions,
                        geometry.lanes(),
                        &queries,
                        &full,
                        merkle::HashKind::Blake3,
                    )
                })
            };
            assert!(valid(&compact));
            // The former full-width representation is not an accepted encoding.
            assert!(!valid(&original));
            let mut wrong = compact.clone();
            wrong.opened_rows[0].push(F::ZERO);
            assert!(!valid(&wrong));
            let mut wrong = compact.clone();
            wrong.opened_rows[0].pop();
            assert!(!valid(&wrong));
            let mut wrong = compact.clone();
            wrong.opened_rows.pop();
            assert!(!valid(&wrong));
            let mut wrong = compact.clone();
            wrong.opened_rows[0].swap(0, occupied.len() - 1);
            assert!(!valid(&wrong));
            for lane in 0..occupied.len() {
                let mut wrong = compact.clone();
                wrong.opened_rows[0][lane] += F::ONE;
                assert!(!valid(&wrong));
            }
            let mut wrong = compact.clone();
            wrong
                .merkle_proof
                .resize(queries.len() * (geometry.position_log + 1) + 1, [0; 32]);
            assert!(!valid(&wrong));
            let mut nonzero_padding = original;
            nonzero_padding.opened_rows[0][15] = F::ONE;
            assert!(compact_initial_rows(&geometry, &mut nonzero_padding).is_err());
        }
    }

    #[test]
    fn compact_source_rows_preserve_gapped_two_source_lanes() {
        let geometry = Geometry::new([9, 12]).unwrap();
        let mut row = vec![F::ZERO; 16];
        row[0] = F::ONE;
        for (index, value) in row[8..].iter_mut().enumerate() {
            *value = F {
                lo: index as u64 + 10,
                hi: 1,
            };
        }
        let expected = std::iter::once(row[0])
            .chain(row[8..].iter().copied())
            .collect::<Vec<_>>();
        let original = RecursiveProof {
            opened_rows: vec![row],
            merkle_proof: Vec::new(),
        };
        let mut compact = original.clone();
        compact_initial_rows(&geometry, &mut compact).unwrap();
        assert_eq!(compact.opened_rows[0], expected);
        assert_eq!(
            expand_initial_rows(&geometry, &compact, 1, 1).unwrap(),
            original
        );
        let mut invalid = original;
        invalid.opened_rows[0][1] = F::ONE;
        assert!(compact_initial_rows(&geometry, &mut invalid).is_err());
    }

    #[test]
    fn compact_joint_opening_preserves_roots_messages_and_transcript() {
        use crate::{ligerito_flock::LigeritoSelection, transcript::Blake3Transcript};

        for logs in [[9, 12, 10], [10, 13, 12], [9, 14, 10]] {
            let geometry = Geometry::new(logs).unwrap();
            let sources = sources(&geometry);
            let borrowed = sources.each_ref().map(Vec::as_slice);
            let resolved = LigeritoSelection::MATCHED_UDR
                .resolve(geometry.packed_log(), 100)
                .unwrap();
            let (root, data) =
                commit_sources(&geometry, borrowed, resolved.prover().log_inv_rates[0]).unwrap();
            let statement = *blake3::hash(&root).as_bytes();
            let point: Vec<_> = (0..geometry.bit_log())
                .map(|i| F {
                    lo: i as u64 + 37,
                    hi: 23,
                })
                .collect();
            let value = bit_evaluation(&geometry.virtual_packed(borrowed), &point);
            let start = || {
                let mut t = Blake3Transcript::new();
                t.absorb_slice(&statement);
                t
            };
            let mut full_t = start();
            let full: JointProof<ligerito::FinalProof> = prove_sources_initial_with_security(
                &mut full_t,
                &geometry,
                &statement,
                borrowed,
                None,
                &resolved,
                |positions, lanes, queries| data.open(positions, lanes, queries),
                &point,
                None,
            )
            .unwrap();
            let mut compact_t = start();
            let compact = prove_joint_sources_with_security(
                &mut compact_t,
                &geometry,
                &statement,
                borrowed,
                &resolved,
                &data,
                &point,
                None,
            )
            .unwrap();
            let full_next: F = full_t.get_field_challenge(&());
            let compact_next: F = compact_t.get_field_challenge(&());
            assert_eq!(full_next, compact_next);
            assert_eq!(full.ring.s_v, compact.ring.s_v);
            let original = &full.ligerito;
            let encoded = &compact.ligerito;
            assert_eq!(original.initial_root, encoded.initial_root);
            assert_eq!(original.recursive_roots, encoded.recursive_roots);
            assert_eq!(original.recursive_proofs, encoded.recursive_proofs);
            assert_eq!(original.sumcheck_transcript, encoded.sumcheck_transcript);
            assert_eq!(original.grinding_nonces, encoded.grinding_nonces);
            assert_eq!(original.fold_grinding_nonces, encoded.fold_grinding_nonces);
            assert_eq!(original.ood_values, encoded.ood_values);
            let vc = resolved.verifier();
            assert_eq!(
                original.initial_proof,
                expand_initial_rows(
                    &geometry,
                    &encoded.initial_proof,
                    vc.queries[0],
                    vc.log_inv_rates[0]
                )
                .unwrap(),
            );
            assert!(
                bincode::serialized_size(encoded).unwrap()
                    < bincode::serialized_size(original).unwrap()
            );
            let mut full_t = start();
            verify_initial_with_security(
                &mut full_t,
                &geometry,
                &statement,
                &point,
                value,
                None,
                &resolved,
                &full.ring,
                original,
                &original.initial_proof,
                |positions, lanes, queries, opening| {
                    authenticate_joint(
                        &geometry,
                        &root,
                        positions,
                        lanes,
                        queries,
                        opening,
                        vc.merkle_hash,
                    )
                },
                None,
            )
            .unwrap();
            let mut compact_t = start();
            verify_joint_with_security(
                &mut compact_t,
                &geometry,
                &statement,
                &root,
                &point,
                value,
                &resolved,
                &compact,
                None,
            )
            .unwrap();
            let full_next: F = full_t.get_field_challenge(&());
            let compact_next: F = compact_t.get_field_challenge(&());
            assert_eq!(full_next, compact_next);
        }
    }

    #[test]
    fn joint_projection_matches_binary_mle_at_non_boolean_points() {
        for logs in [[9, 12, 10], [10, 13, 12], [9, 13, 10]] {
            let geometry = Geometry::new(logs).unwrap();
            let sources = sources(&geometry);
            let packed = geometry.virtual_packed(sources.each_ref().map(Vec::as_slice));
            let point: Vec<_> = (0..geometry.bit_log())
                .map(|i| F {
                    lo: i as u64 + 37,
                    hi: 23,
                })
                .collect();
            let projected = (0..3).fold(F::ZERO, |sum, branch| {
                let (local, scale) = geometry.project_point(branch, &point);
                sum + scale * bit_evaluation(&sources[branch][..1 << logs[branch]], &local)
            });
            assert_eq!(bit_evaluation(&packed, &point), projected);
        }
    }

    #[test]
    fn joint_source_authentication_rejects_rows_paths_and_padding_tampering() {
        let geometry = Geometry::new([9, 12, 10]).unwrap();
        let sources = sources(&geometry);
        let (root, data) =
            commit_sources(&geometry, sources.each_ref().map(Vec::as_slice), 1).unwrap();
        let positions = 1 << (geometry.position_log + 1);
        let queries = [0, 1, 17, positions - 1];
        let opening = data.open(positions, 16, &queries);
        let valid = |proof: &RecursiveProof| {
            authenticate_joint(
                &geometry,
                &root,
                positions,
                16,
                &queries,
                proof,
                merkle::HashKind::Blake3,
            )
        };
        assert!(valid(&opening));
        for lane in [
            geometry.offset(0),
            geometry.offset(1),
            geometry.offset(2),
            15,
        ] {
            let mut wrong = opening.clone();
            wrong.opened_rows[0][lane] += F::ONE;
            assert!(!valid(&wrong));
        }
        let mut wrong = opening.clone();
        wrong.merkle_proof[0][0] ^= 1;
        assert!(!valid(&wrong));
        let mut wrong = opening.clone();
        wrong.merkle_proof.pop();
        assert!(!valid(&wrong));
        let mut wrong = opening.clone();
        wrong.merkle_proof.push([0; 32]);
        assert!(!valid(&wrong));
        let mut wrong = opening.clone();
        wrong.opened_rows.pop();
        assert!(!valid(&wrong));
        let mut wrong = opening.clone();
        wrong.opened_rows[0].pop();
        assert!(!valid(&wrong));
        let mut wrong_root = root;
        wrong_root[0] ^= 1;
        assert!(!authenticate_joint(
            &geometry,
            &wrong_root,
            positions,
            16,
            &queries,
            &opening,
            merkle::HashKind::Blake3
        ));
    }

    #[test]
    fn joint_opening_binds_root_and_rejects_nonzero_physical_padding() {
        use crate::{ligerito_flock::LigeritoSelection, transcript::Blake3Transcript};

        for bad_padding in [false, true] {
            let geometry = Geometry::new([9, 13, 10]).unwrap();
            let mut sources = sources(&geometry);
            if bad_padding {
                sources[0][1 << geometry.logs[0]] = F::ONE;
            }
            let borrowed = sources.each_ref().map(Vec::as_slice);
            let resolved = LigeritoSelection::MATCHED_UDR
                .resolve(geometry.packed_log(), 100)
                .unwrap();
            let (root, data) =
                commit_sources(&geometry, borrowed, resolved.prover().log_inv_rates[0]).unwrap();
            let statement = *blake3::hash(&root).as_bytes();
            let point: Vec<_> = (0..geometry.bit_log())
                .map(|i| F {
                    lo: i as u64 + 37,
                    hi: 23,
                })
                .collect();
            let value = bit_evaluation(&geometry.virtual_packed(borrowed), &point);
            let start = || {
                let mut t = Blake3Transcript::new();
                t.absorb_slice(&statement);
                t
            };
            let proof = prove_joint_sources_with_security(
                &mut start(),
                &geometry,
                &statement,
                borrowed,
                &resolved,
                &data,
                &point,
                None,
            )
            .unwrap();
            let result = verify_joint_with_security(
                &mut start(),
                &geometry,
                &statement,
                &root,
                &point,
                value,
                &resolved,
                &proof,
                None,
            );
            assert_eq!(result.is_ok(), !bad_padding);
            if !bad_padding {
                let mut wrong = proof.clone();
                wrong.ligerito.initial_root[0] ^= 1;
                assert!(
                    verify_joint_with_security(
                        &mut start(),
                        &geometry,
                        &statement,
                        &root,
                        &point,
                        value,
                        &resolved,
                        &wrong,
                        None,
                    )
                    .is_err()
                );
            }
        }
    }

    #[test]
    fn three_root_padding_basis_matches_logical_support_exactly() {
        for logs in [[9, 11, 9], [9, 11, 10], [9, 13, 10]] {
            let g = Geometry::new(logs).unwrap();
            let mut live = vec![false; 1 << g.packed_log()];
            for branch in 0..3 {
                for word in 0..1 << logs[branch] {
                    let index = g.embed(branch, word);
                    assert!(!live[index], "source supports overlap");
                    live[index] = true;
                }
            }
            let point: Vec<_> = (0..g.packed_log())
                .map(|i| F {
                    lo: 31 + i as u64,
                    hi: 17,
                })
                .collect();
            let scale = F { lo: 19, hi: 271 };
            let mut actual = vec![F::ZERO; live.len()];
            for (point, scale) in g.padding_bases(point.clone(), scale) {
                for (a, weight) in actual.iter_mut().zip(eq_table(&point)) {
                    *a += scale * weight;
                }
            }
            for ((actual, live), expected) in actual.into_iter().zip(live).zip(eq_table(&point)) {
                assert_eq!(actual, if live { F::ZERO } else { scale * expected });
            }
        }
        assert!(Geometry::new([13, 13, 13]).is_ok());
        assert!(Geometry::new([13; 17]).is_err());
        assert!(Geometry::<0>::new([]).is_err());
    }

    fn check_direct_padding<const N: usize>(logs: [usize; N]) {
        use rand::{RngExt, SeedableRng, rngs::StdRng};

        let geometry = Geometry::new(logs).unwrap();
        let mut rng = StdRng::seed_from_u64(0x50414444494e47);
        let mut sample = || F {
            lo: rng.random(),
            hi: rng.random(),
        };
        let size = 1 << geometry.packed_log();
        let packed: Vec<_> = (0..size).map(|_| sample()).collect();
        let initial: Vec<_> = (0..size).map(|_| sample()).collect();
        for case in 0..5 {
            let point: Vec<_> = (0..geometry.packed_log())
                .map(|i| match case {
                    1 => F::ZERO,
                    2 => F::ONE,
                    3 if i % 3 == 0 => F::ZERO,
                    3 if i % 3 == 1 => F::ONE,
                    _ => sample(),
                })
                .collect();
            let eta = if case == 4 { F::ZERO } else { sample() };
            let mut reference = initial.clone();
            for (basis_point, scale) in geometry.padding_bases(point.clone(), eta) {
                add_ood_basis(&mut reference, &packed, &basis_point, scale, None);
            }
            let mut direct = initial.clone();
            geometry.add_padding_basis(&mut direct, &point, eta);
            assert_eq!(direct, reference, "logs={logs:?}, case={case}");
        }
    }

    #[test]
    fn direct_padding_matches_separate_basis_updates() {
        // Include a completely occupied virtual domain, unused lanes,
        // unequal source sizes, and logical zeros inside physical slices.
        check_direct_padding([9, 9]);
        check_direct_padding([9]);
        check_direct_padding([9, 13]);
        check_direct_padding([9, 12, 10]);
        check_direct_padding([9, 13, 10]);
        check_direct_padding([10, 9, 9, 12]);
    }

    fn check_streamed_tables<const N: usize>(logs: [usize; N]) {
        use crate::transcript::Blake3Transcript;
        use rand::{RngExt, SeedableRng, rngs::StdRng};

        let geometry = Geometry::new(logs).unwrap();
        let mut rng = StdRng::seed_from_u64(0x53545245414d);
        let mut sample = || F {
            lo: rng.random(),
            hi: rng.random(),
        };
        // Deliberately nonzero physical padding tests that streaming never
        // discards it before the separate zero-padding claim authenticates it.
        let sources: [Vec<_>; N] = std::array::from_fn(|branch| {
            (0..1 << geometry.physical_logs[branch])
                .map(|_| sample())
                .collect()
        });
        let borrowed = sources.each_ref().map(Vec::as_slice);
        let packed = geometry.virtual_packed(borrowed);
        for case in 0..3 {
            let point: Vec<_> = (0..geometry.packed_log())
                .map(|i| match case {
                    1 if i % 3 == 0 => F::ZERO,
                    1 if i % 3 == 1 => F::ONE,
                    _ => sample(),
                })
                .collect();
            let mut dense_t = Blake3Transcript::new();
            let (dense_ring, mut dense_basis, dense_target) =
                ring_switch_prove_with(&mut dense_t, &packed, &point, |v| v);
            let mut streamed_t = Blake3Transcript::new();
            let (streamed_ring, eq_r2, streamed_target) = ring_switch_prove_marginal(
                &mut streamed_t,
                geometry.ring_marginal(borrowed, &point),
            );
            assert_eq!(streamed_ring.s_v, dense_ring.s_v);
            assert_eq!(streamed_target, dense_target);
            assert_eq!(
                streamed_t.get_field_challenge::<Gf>(&()),
                dense_t.get_field_challenge::<Gf>(&()),
            );
            let padding: Vec<_> = (0..geometry.packed_log()).map(|_| sample()).collect();
            let padding_scale = sample();
            let ood_point: Vec<_> = (0..geometry.packed_log()).map(|_| sample()).collect();
            let ood_scale = sample();
            let ood = (case == 2).then_some((ood_point.as_slice(), ood_scale));
            if let Some((point, scale)) = ood {
                add_ood_basis(&mut dense_basis, &packed, point, scale, None);
            }
            geometry.add_padding_basis(&mut dense_basis, &padding, padding_scale);
            let initial =
                geometry.initial_tables(borrowed, &point, &eq_r2, (&padding, padding_scale), ood);
            assert_eq!(initial.packed, packed);
            assert_eq!(initial.basis, dense_basis);
            let (mut reference, first) =
                ligerito::SumcheckProver::new(packed.clone(), dense_basis, dense_target);
            assert_eq!(initial.first_message, first);
            let (mut streamed, _) = ligerito::SumcheckProver::new_with_first_msg(
                initial.packed,
                initial.basis,
                streamed_target,
                initial.first_message,
            );
            let r = sample();
            assert_eq!(streamed.fold_skip(&initial.lookahead, r), reference.fold(r),);
            streamed.drain_pending_fold();
            assert_eq!(streamed.f(), reference.f());
        }
    }

    #[test]
    fn falcon_three_keccak_slabs_fit_and_preserve_openings() {
        let geometry = Geometry::new([9, 12, 12, 12]).unwrap();
        assert_eq!(geometry.position_log, 10);
        assert_eq!(geometry.lane_logs, [0, 2, 2, 2]);
        assert_eq!(geometry.virtual_lane_log, 4);
        check_streamed_tables([9, 12, 12, 12]);
        check_streamed_proof([9, 12, 12, 12], false);
    }

    #[test]
    fn streamed_ring_switch_and_initial_tables_match_dense_reference() {
        check_streamed_tables([9, 9]);
        check_streamed_tables([9, 13]);
        check_streamed_tables([9, 11, 10]);
        check_streamed_tables([9, 14, 10]);
    }

    fn check_streamed_proof<const N: usize>(logs: [usize; N], with_ood: bool) {
        use crate::{ligerito_flock::LigeritoSelection, transcript::Blake3Transcript};
        use flock_core::pcs::commit::commit;

        let geometry = Geometry::new(logs).unwrap();
        let selection = if with_ood {
            LigeritoSelection::JOHNSON
        } else {
            LigeritoSelection::MATCHED_UDR
        };
        let resolved = selection.resolve(geometry.packed_log(), 100).unwrap();
        let sources: [Vec<_>; N] = std::array::from_fn(|branch| {
            (0..1 << geometry.physical_logs[branch])
                .map(|i| {
                    if i < 1 << logs[branch] {
                        F {
                            lo: (i as u64).wrapping_mul(0x9e3779b97f4a7c15),
                            hi: ((i + branch) as u64).wrapping_mul(0x85ebca6b27d4eb2f),
                        }
                    } else {
                        F::ZERO
                    }
                })
                .collect()
        });
        let committed: [_; N] = std::array::from_fn(|branch| {
            commit(
                &sources[branch],
                &geometry.params(branch, resolved.prover().log_inv_rates[0]),
            )
        });
        let borrowed = sources.each_ref().map(Vec::as_slice);
        let packed = geometry.virtual_packed(borrowed);
        let statement = [0x53; 32];
        let mut dense_t = Blake3Transcript::new();
        let mut streamed_t = Blake3Transcript::new();
        let params = with_ood.then_some(OodRoundParams { grinding_bits: 0 });
        let dense_ood = prove_ood(&mut dense_t, params, &packed);
        let streamed_ood = prove_ood(&mut streamed_t, params, &packed);
        let point: Vec<_> = (0..geometry.bit_log())
            .map(|i| F {
                lo: i as u64 + 17,
                hi: 42,
            })
            .collect();
        let data = committed.each_ref().map(|(_, data)| data);
        let dense = prove(
            &mut dense_t,
            &geometry,
            &statement,
            packed,
            dense_ood.as_ref(),
            &resolved,
            data,
            &point,
        )
        .unwrap();
        let streamed = prove_sources_with_security(
            &mut streamed_t,
            &geometry,
            &statement,
            borrowed,
            streamed_ood.as_ref(),
            &resolved,
            data,
            &point,
            None,
        )
        .unwrap();
        assert_eq!(dense.ring.s_v, streamed.ring.s_v);
        assert_eq!(dense.paths, streamed.paths);
        assert_eq!(
            bincode::serialize(&dense.ligerito).unwrap(),
            bincode::serialize(&streamed.ligerito).unwrap(),
        );
        assert_eq!(
            streamed_t.get_field_challenge::<Gf>(&()),
            dense_t.get_field_challenge::<Gf>(&()),
        );
        let mut verifier_t = Blake3Transcript::new();
        let ood = verify_ood(&mut verifier_t, &geometry, params, streamed.ood.as_ref()).unwrap();
        let value = streamed
            .ring
            .s_v
            .iter()
            .zip(eq_table(&point[..7]))
            .fold(F::ZERO, |sum, (&marginal, weight)| sum + marginal * weight);
        verify(
            &mut verifier_t,
            &geometry,
            &statement,
            &committed.each_ref().map(|(commitment, _)| commitment.root),
            &point,
            value,
            ood.as_ref(),
            &resolved,
            &streamed,
        )
        .unwrap();
    }

    #[test]
    fn streamed_shared_opening_preserves_entire_proof_and_transcript() {
        // Keep the materialized fallback covered below the lookahead cutoff.
        check_streamed_proof([9, 12, 10], true);
        check_streamed_proof([9, 13], false);
        check_streamed_proof([9, 13, 10], true);
        // Domain large enough for the precomputed two-round lookahead path.
        check_streamed_proof([9, 14, 10], false);
    }

    #[test]
    fn deferred_initial_requires_two_supported_lookahead_rounds() {
        let resolved = crate::ligerito_flock::LigeritoSelection::MATCHED_UDR
            .resolve(15, 100)
            .unwrap();
        let mut config = resolved.prover().clone();
        assert!(!ligerito::supports_deferred_initial(&config, 13));
        config.initial_k = 1;
        assert!(!ligerito::supports_deferred_initial(&config, 15));
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Proof<const N: usize = 2> {
    /// Round 0: `y = MLE[V](ζ⃗)` and the proof-of-work nonce before the
    /// `ζ` draw.
    pub ood: Option<OodRound>,
    pub ring: RingSwitchProof,
    pub ligerito: LigeritoProof,
    pub paths: [Vec<Hash>; N],
}

/// One initial multiproof authenticates rows stored without structural-zero
/// lanes. The compact terminal stores the complete final pre-fold message.
/// The logical source count is independent of this proof shape.
#[derive(Clone, Debug)]
pub(crate) struct JointProof<Final = Vec<F>> {
    pub ring: RingSwitchProof,
    pub ligerito: LigeritoProof<Final>,
}

pub(crate) struct JointProverData<const N: usize> {
    geometry: Geometry<N>,
    log_inv_rate: usize,
    codewords: [Vec<F>; N],
    merkle_tree: Vec<Hash>,
}

impl<const N: usize> Drop for JointProverData<N> {
    fn drop(&mut self) {
        for words in &mut self.codewords {
            flock_core::scratch::give_f128(std::mem::take(words));
        }
    }
}

fn validate_geometry<const N: usize>(geometry: &Geometry<N>) -> Result<(), Error> {
    if *geometry != Geometry::new(geometry.logs)? {
        return Err(Error::Invalid("joint source geometry"));
    }
    Ok(())
}

/// The transport order is canonical lane order, independent of branch order.
/// Only entire unused lanes are omitted: padding inside an occupied source
/// does not imply zero RS evaluations.
fn occupied_lanes<const N: usize>(geometry: &Geometry<N>) -> Vec<usize> {
    (0..geometry.lanes())
        .filter(|&lane| {
            (0..N).any(|branch| {
                let start = geometry.offset(branch);
                (start..start + (1 << geometry.lane_logs[branch])).contains(&lane)
            })
        })
        .collect()
}

fn compact_initial_rows<const N: usize>(
    geometry: &Geometry<N>,
    opening: &mut RecursiveProof,
) -> Result<(), Error> {
    let occupied = occupied_lanes(geometry);
    let occupied_prefix = occupied.iter().copied().eq(0..occupied.len());
    for row in &mut opening.opened_rows {
        if row.len() != geometry.lanes() {
            return Err(Error::Invalid("noncanonical initial source row"));
        }
        if occupied_prefix {
            if row[occupied.len()..].iter().any(|&value| value != F::ZERO) {
                return Err(Error::Invalid("noncanonical initial source row"));
            }
        } else {
            if row
                .iter()
                .enumerate()
                .any(|(lane, &value)| !occupied.contains(&lane) && value != F::ZERO)
            {
                return Err(Error::Invalid("noncanonical initial source row"));
            }
            for (destination, &lane) in occupied.iter().enumerate() {
                row[destination] = row[lane];
            }
        }
        row.truncate(occupied.len());
    }
    Ok(())
}

fn expand_initial_rows<const N: usize>(
    geometry: &Geometry<N>,
    opening: &RecursiveProof,
    queries: usize,
    log_inv_rate: usize,
) -> Result<RecursiveProof, Error> {
    let occupied = occupied_lanes(geometry);
    let max_path_hashes = queries
        .checked_mul(geometry.position_log + log_inv_rate)
        .ok_or(Error::Invalid("initial source query shape"))?;
    if opening.opened_rows.len() != queries
        || opening.merkle_proof.len() > max_path_hashes
        || opening
            .opened_rows
            .iter()
            .any(|row| row.len() != occupied.len())
    {
        return Err(Error::Invalid("compact initial source opening shape"));
    }
    let opened_rows = opening
        .opened_rows
        .iter()
        .map(|row| {
            let mut full = vec![F::ZERO; geometry.lanes()];
            for (&lane, &value) in occupied.iter().zip(row) {
                full[lane] = value;
            }
            full
        })
        .collect();
    Ok(RecursiveProof {
        opened_rows,
        merkle_proof: opening.merkle_proof.clone(),
    })
}

/// RS-encode populated lanes and build one tree over full canonical virtual
/// rows. Zero lanes are synthesized in bounded leaf-hashing buffers.
pub(crate) fn commit_sources<const N: usize>(
    geometry: &Geometry<N>,
    sources: [&[F]; N],
    log_inv_rate: usize,
) -> Result<(Hash, JointProverData<N>), Error> {
    validate_geometry(geometry)?;
    if log_inv_rate == 0
        || geometry
            .packed_log()
            .checked_add(log_inv_rate)
            .is_none_or(|log| log >= usize::BITS as usize)
        || (0..N).any(|branch| sources[branch].len() != 1 << geometry.physical_logs[branch])
    {
        return Err(Error::Invalid("joint source encoding shape"));
    }
    let timing = std::env::var_os("FLOCK_COMMIT_TIMING").is_some();
    let encode_started = std::time::Instant::now();
    let encode_span = tracing::info_span!("shared_source:encode").entered();
    let codewords = std::array::from_fn(|branch| {
        flock_core::pcs::commit::encode(sources[branch], &geometry.params(branch, log_inv_rate))
    });
    drop(encode_span);
    if timing {
        eprintln!(
            "[joint-commit-timing] encoding: {:.2} ms",
            encode_started.elapsed().as_secs_f64() * 1e3
        );
    }
    let positions = 1 << (geometry.position_log + log_inv_rate);
    let merkle_started = std::time::Instant::now();
    let merkle_span = tracing::info_span!("shared_source:merkle").entered();
    let merkle_tree = merkle::merkle_tree_from_rows(
        positions,
        geometry.lanes() * 16,
        merkle::HashKind::Blake3,
        |first, bytes| {
            bytes.fill(0);
            for (row_index, row) in bytes.chunks_exact_mut(geometry.lanes() * 16).enumerate() {
                for branch in 0..N {
                    let width = 1 << geometry.lane_logs[branch];
                    let first_word = (first + row_index) * width;
                    for (lane, word) in codewords[branch][first_word..first_word + width]
                        .iter()
                        .enumerate()
                    {
                        let offset = (geometry.offset(branch) + lane) * 16;
                        row[offset..offset + 8].copy_from_slice(&word.lo.to_le_bytes());
                        row[offset + 8..offset + 16].copy_from_slice(&word.hi.to_le_bytes());
                    }
                }
            }
        },
    );
    drop(merkle_span);
    if timing {
        eprintln!(
            "[joint-commit-timing] merkle: {:.2} ms",
            merkle_started.elapsed().as_secs_f64() * 1e3
        );
    }
    let root = *merkle_tree.last().expect("nonempty joint source tree");
    Ok((
        root,
        JointProverData {
            geometry: geometry.clone(),
            log_inv_rate,
            codewords,
            merkle_tree,
        },
    ))
}

impl<const N: usize> JointProverData<N> {
    fn open(&self, positions: usize, lanes: usize, queries: &[usize]) -> RecursiveProof {
        assert_eq!(
            positions,
            1 << (self.geometry.position_log + self.log_inv_rate)
        );
        assert_eq!(lanes, self.geometry.lanes());
        let mut rows = vec![vec![F::ZERO; lanes]; queries.len()];
        for branch in 0..N {
            let width = 1 << self.geometry.lane_logs[branch];
            let start = self.geometry.offset(branch);
            for (row, &q) in rows.iter_mut().zip(queries) {
                row[start..start + width]
                    .copy_from_slice(&self.codewords[branch][q * width..(q + 1) * width]);
            }
        }
        RecursiveProof {
            opened_rows: rows,
            merkle_proof: merkle::merkle_multi_proof(&self.merkle_tree, positions, queries),
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn prove_joint_sources_with_security<const N: usize>(
    t: &mut (impl Transcript + Send),
    geometry: &Geometry<N>,
    statement: &Hash,
    sources: [&[F]; N],
    resolved: &crate::ligerito_flock::ResolvedLigerito,
    data: &JointProverData<N>,
    point: &[Gf],
    security: Option<&mut grinding::GrindingContext<'_>>,
) -> Result<JointProof, Error> {
    if *geometry != data.geometry
        || data.log_inv_rate != resolved.prover().log_inv_rates[0]
        || point.len() != geometry.bit_log()
        || (0..N).any(|branch| sources[branch].len() != 1 << geometry.physical_logs[branch])
    {
        return Err(Error::Invalid("joint source opening shape"));
    }
    let mut proof = prove_sources_initial_with_security(
        t,
        geometry,
        statement,
        sources,
        None,
        resolved,
        |positions, lanes, queries| data.open(positions, lanes, queries),
        point,
        security,
    )?;
    compact_initial_rows(geometry, &mut proof.ligerito.initial_proof)?;
    Ok(proof)
}

fn authenticate_joint<const N: usize>(
    geometry: &Geometry<N>,
    root: &Hash,
    positions: usize,
    lanes: usize,
    queries: &[usize],
    opening: &RecursiveProof,
    hash: merkle::HashKind,
) -> bool {
    if lanes != geometry.lanes() || opening.opened_rows.len() != queries.len() {
        return false;
    }
    let mut occupied = vec![false; lanes];
    for branch in 0..N {
        let start = geometry.offset(branch);
        occupied[start..start + (1 << geometry.lane_logs[branch])].fill(true);
    }
    let mut bytes = vec![0u8; lanes * 16];
    let mut hashes = Vec::with_capacity(queries.len());
    for row in &opening.opened_rows {
        if row.len() != lanes {
            return false;
        }
        for (lane, word) in row.iter().enumerate() {
            if !occupied[lane] && *word != F::ZERO {
                return false;
            }
            bytes[lane * 16..lane * 16 + 8].copy_from_slice(&word.lo.to_le_bytes());
            bytes[lane * 16 + 8..(lane + 1) * 16].copy_from_slice(&word.hi.to_le_bytes());
        }
        hashes.push(merkle::hash_leaf(&bytes, hash));
    }
    merkle::verify_merkle_multi_proof(
        root,
        positions,
        queries,
        &hashes,
        &opening.merkle_proof,
        hash,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn verify_joint_with_security<const N: usize>(
    t: &mut (impl Transcript + Send),
    geometry: &Geometry<N>,
    statement: &Hash,
    root: &Hash,
    point: &[Gf],
    value: F,
    resolved: &crate::ligerito_flock::ResolvedLigerito,
    proof: &JointProof,
    security: Option<&mut grinding::GrindingContext<'_>>,
) -> Result<(), Error> {
    validate_geometry(geometry)?;
    if point.len() != geometry.bit_log() {
        return Err(Error::Invalid("joint source opening point"));
    }
    let vc = resolved.verifier();
    let initial = expand_initial_rows(
        geometry,
        &proof.ligerito.initial_proof,
        vc.queries[0],
        vc.log_inv_rates[0],
    )?;
    verify_initial_with_security(
        t,
        geometry,
        statement,
        point,
        value,
        None,
        resolved,
        &proof.ring,
        &proof.ligerito,
        &initial,
        |positions, lanes, queries, opening| {
            positions == 1 << (geometry.position_log + vc.log_inv_rates[0])
                && authenticate_joint(
                    geometry,
                    root,
                    positions,
                    lanes,
                    queries,
                    opening,
                    vc.merkle_hash,
                )
        },
        security,
    )
}

/// Round 0 on the prover side, on the virtual packed witness `packed`.
/// Must run right after the statement, before any other challenge.
pub(crate) fn prove_ood(
    t: &mut (impl Transcript + Send),
    params: Option<OodRoundParams>,
    packed: &[F],
) -> Option<OodProverClaim> {
    params.map(|params| prove_ood_round_packed(t, packed, params))
}

/// Round 0 on the verifier side: the same frame and draw, the nonce
/// checked against `params`, the prover's `y` absorbed.
pub(crate) fn verify_ood<const N: usize>(
    t: &mut (impl Transcript + Send),
    geometry: &Geometry<N>,
    params: Option<OodRoundParams>,
    round: Option<&OodRound>,
) -> Result<Option<OodVerifierClaim>, Error> {
    match (params, round) {
        (None, None) => Ok(None),
        (Some(params), Some(round)) => verify_ood_round(t, geometry.packed_log(), params, round)
            .map(Some)
            .map_err(|_| Error::Invalid("Round 0 (out-of-domain sample)")),
        _ => Err(Error::Invalid(
            "Round-0 presence disagrees with Ligerito regime",
        )),
    }
}

pub(crate) fn ood_parameters(
    resolved: &crate::ligerito_flock::ResolvedLigerito,
) -> Result<Option<(f64, OodRoundParams)>, Error> {
    resolved
        .ood_bits()
        .map(|bits| {
            let grinding_bits = (CompositionProfile::LAMBDA as f64 - bits).ceil().max(0.) as u32;
            if grinding_bits > MAX_DERIVED_GRINDING_BITS {
                return Err(Error::Config(
                    "Round 0 exceeds the derived grinding cap".into(),
                ));
            }
            Ok((bits, OodRoundParams { grinding_bits }))
        })
        .transpose()
}

fn sample_padding<const N: usize>(
    t: &mut (impl Transcript + Send),
    geometry: &Geometry<N>,
) -> Vec<(Vec<Gf>, Gf)> {
    if N == 2 {
        t.absorb_slice(b"hybrid/zero-padding/three-equality-bases/v1");
    } else {
        t.absorb_slice(b"hybrid/zero-padding/disjoint-supports/v2");
        t.absorb_slice(&(N as u64).to_le_bytes());
        for branch in 0..N {
            t.absorb_slice(&(geometry.logs[branch] as u64).to_le_bytes());
            t.absorb_slice(&(geometry.offset(branch) as u64).to_le_bytes());
        }
    }
    let point = (0..geometry.packed_log())
        .map(|_| t.get_field_challenge(&()))
        .collect();
    let eta = t.get_field_challenge(&());
    geometry.padding_bases(point, eta)
}

pub(crate) fn prove<const N: usize>(
    t: &mut (impl Transcript + Send),
    geometry: &Geometry<N>,
    statement: &Hash,
    packed: Vec<F>,
    ood: Option<&OodProverClaim>,
    resolved: &crate::ligerito_flock::ResolvedLigerito,
    data: [&ProverData; N],
    point: &[Gf],
) -> Result<Proof<N>, Error> {
    prove_with_security(
        t, geometry, statement, packed, ood, resolved, data, point, None,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn prove_with_security<const N: usize>(
    t: &mut (impl Transcript + Send),
    geometry: &Geometry<N>,
    statement: &Hash,
    packed: Vec<F>,
    ood: Option<&OodProverClaim>,
    resolved: &crate::ligerito_flock::ResolvedLigerito,
    data: [&ProverData; N],
    point: &[Gf],
    security: Option<&mut grinding::GrindingContext<'_>>,
) -> Result<Proof<N>, Error> {
    let ring_scope = tracing::info_span!("op:ring_switch").entered();
    // flock's packed words are bit-compatible with `Gf`: the ring switch
    // reads them in place and writes the basis in flock's element type (no
    // 2^m-element conversion pass either way).
    let (ring, mut basis, mut target) =
        ring_switch_prove_with(t, &packed, &point[7..], |value| value);
    drop(ring_scope);
    let basis_scope = tracing::info_span!("op:extra_bases").entered();
    // Batch the Round-0 claim into the same opening: one draw adds
    // `η_ood·eq(·, ζ⃗)` to the basis and `η_ood·y` to the target.
    if let Some(ood) = ood {
        let eta_ood: Gf = t.get_field_challenge(&());
        add_ood_basis(&mut basis, &packed, &ood.point, eta_ood, None);
        target += eta_ood * ood.y;
    }
    // Sample after all ring-switch messages. The authenticated target of this
    // independent claim is zero; a nonzero padded message cannot be discarded.
    let padding = sample_padding(t, geometry);
    let (padding_point, padding_scale) = &padding[0];
    geometry.add_padding_basis(&mut basis, padding_point, *padding_scale);
    drop(basis_scope);
    continue_prove(
        t,
        geometry,
        statement,
        PreparedInitial::Dense {
            packed,
            basis,
            precomputed: None,
        },
        target,
        ring,
        ood,
        resolved,
        data,
        security,
    )
}

/// Branch-aware shared opener: preserve the dense prover's messages while
/// avoiding its full virtual witness/equality allocation during ring switch.
#[cfg(test)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn prove_sources_with_security<const N: usize>(
    t: &mut (impl Transcript + Send),
    geometry: &Geometry<N>,
    statement: &Hash,
    sources: [&[F]; N],
    ood: Option<&OodProverClaim>,
    resolved: &crate::ligerito_flock::ResolvedLigerito,
    data: [&ProverData; N],
    point: &[Gf],
    security: Option<&mut grinding::GrindingContext<'_>>,
) -> Result<Proof<N>, Error> {
    let mut paths = std::array::from_fn(|_| Vec::new());
    let proof = prove_sources_initial_with_security(
        t,
        geometry,
        statement,
        sources,
        ood,
        resolved,
        separate_initial(geometry, data, &mut paths),
        point,
        security,
    )?;
    Ok(Proof {
        ood: ood.map(|claim| claim.round),
        ring: proof.ring,
        ligerito: proof.ligerito,
        paths,
    })
}

#[allow(clippy::too_many_arguments)]
fn prove_sources_initial_with_security<const N: usize, Final: OpeningEncoding>(
    t: &mut (impl Transcript + Send),
    geometry: &Geometry<N>,
    statement: &Hash,
    sources: [&[F]; N],
    ood: Option<&OodProverClaim>,
    resolved: &crate::ligerito_flock::ResolvedLigerito,
    open_initial: impl FnOnce(usize, usize, &[usize]) -> RecursiveProof,
    point: &[Gf],
    security: Option<&mut grinding::GrindingContext<'_>>,
) -> Result<JointProof<Final>, Error> {
    let ring_scope = tracing::info_span!("op:ring_switch").entered();
    let marginal = geometry.ring_marginal(sources, &point[7..]);
    let (ring, eq_r2, mut target) = ring_switch_prove_marginal(t, marginal);
    drop(ring_scope);
    let basis_scope = tracing::info_span!("op:extra_bases").entered();
    let ood_basis = ood.map(|ood| {
        let eta: Gf = t.get_field_challenge(&());
        target += eta * ood.y;
        (ood.point.as_slice(), eta)
    });
    let padding = sample_padding(t, geometry);
    let (padding_point, padding_scale) = &padding[0];
    let initial = if ligerito::supports_deferred_initial(&resolved.prover(), geometry.packed_log())
    {
        PreparedInitial::Deferred(streamed::prepare(
            geometry,
            sources,
            &point[7..],
            &eq_r2,
            (padding_point, *padding_scale),
            ood_basis,
        ))
    } else {
        let tables = geometry.initial_tables(
            sources,
            &point[7..],
            &eq_r2,
            (padding_point, *padding_scale),
            ood_basis,
        );
        PreparedInitial::Dense {
            packed: tables.packed,
            basis: tables.basis,
            precomputed: Some((tables.first_message, tables.lookahead)),
        }
    };
    drop(basis_scope);
    continue_prove_with_initial(
        t,
        statement,
        initial,
        target,
        ring,
        resolved,
        open_initial,
        security,
    )
}

enum PreparedInitial<'a> {
    Dense {
        packed: Vec<F>,
        basis: Vec<F>,
        precomputed: Option<(ligerito::SumcheckMessage, ligerito::FoldLookahead)>,
    },
    Deferred(ligerito::DeferredInitial<'a>),
}

/// Static dispatch keeps the existing separate-source clients and the compact
/// joint opening on one transcript implementation. Falcon accepts only the
/// compact type; the proof cannot select an encoding.
#[allow(clippy::too_many_arguments)]
trait OpeningEncoding: Sized {
    fn prove(
        config: &ligerito::ProverConfig,
        initial: PreparedInitial<'_>,
        target: F,
        statement: Hash,
        open: impl FnOnce(usize, usize, &[usize]) -> RecursiveProof,
        challenger: &mut impl Challenger,
    ) -> LigeritoProof<Self>;

    fn verify(
        config: &ligerito::VerifierConfig,
        proof: &LigeritoProof<Self>,
        initial: &RecursiveProof,
        log_n: usize,
        target: F,
        statement: &Hash,
        eval: impl Fn(&[F], usize) -> Vec<F>,
        authenticate: impl FnOnce(usize, usize, &[usize], &RecursiveProof) -> bool,
        challenger: &mut impl Challenger,
    ) -> bool;
}

macro_rules! opening_encoding {
    ($final:ty, $dense:ident, $precomputed:ident, $deferred:ident, $verify:ident, $initial:ident $(=> $expanded:ident)?) => {
        impl OpeningEncoding for $final {
            fn prove(
                config: &ligerito::ProverConfig,
                initial: PreparedInitial<'_>,
                target: F,
                statement: Hash,
                open: impl FnOnce(usize, usize, &[usize]) -> RecursiveProof,
                challenger: &mut impl Challenger,
            ) -> LigeritoProof<Self> {
                match initial {
                    PreparedInitial::Deferred(initial) => ligerito::$deferred(
                        config, initial, target, statement, open, challenger,
                    ),
                    PreparedInitial::Dense {
                        packed,
                        basis,
                        precomputed: Some((message, lookahead)),
                    } => ligerito::$precomputed(
                        config, packed, basis, target, statement, open, message,
                        Some(lookahead), challenger,
                    ),
                    PreparedInitial::Dense { packed, basis, precomputed: None } => {
                        ligerito::$dense(config, packed, basis, target, statement, open, challenger)
                    }
                }
            }

            fn verify(
                config: &ligerito::VerifierConfig,
                proof: &LigeritoProof<Self>,
                $initial: &RecursiveProof,
                log_n: usize,
                target: F,
                statement: &Hash,
                eval: impl Fn(&[F], usize) -> Vec<F>,
                authenticate: impl FnOnce(usize, usize, &[usize], &RecursiveProof) -> bool,
                challenger: &mut impl Challenger,
            ) -> bool {
                let _ = $initial;
                ligerito::$verify(
                    config, proof, $( $expanded, )? log_n, target, statement,
                    eval, authenticate, challenger,
                )
            }
        }
    };
}

opening_encoding!(
    ligerito::FinalProof,
    recursive_prover_with_basis_initial,
    recursive_prover_with_basis_initial_precomputed_round0,
    recursive_prover_with_basis_initial_deferred,
    recursive_verifier_with_basis_initial,
    initial
);
opening_encoding!(
    Vec<F>,
    recursive_prover_with_basis_initial_compact,
    recursive_prover_with_basis_initial_precomputed_round0_compact,
    recursive_prover_with_basis_initial_deferred_compact,
    recursive_verifier_with_basis_initial_compact,
    initial => initial
);

#[allow(clippy::too_many_arguments)]
fn continue_prove<const N: usize>(
    t: &mut (impl Transcript + Send),
    geometry: &Geometry<N>,
    statement: &Hash,
    initial: PreparedInitial<'_>,
    target: F,
    ring: RingSwitchProof,
    ood: Option<&OodProverClaim>,
    resolved: &crate::ligerito_flock::ResolvedLigerito,
    data: [&ProverData; N],
    security: Option<&mut grinding::GrindingContext<'_>>,
) -> Result<Proof<N>, Error> {
    let mut paths = std::array::from_fn(|_| Vec::new());
    let proof = continue_prove_with_initial(
        t,
        statement,
        initial,
        target,
        ring,
        resolved,
        separate_initial(geometry, data, &mut paths),
        security,
    )?;
    Ok(Proof {
        ood: ood.map(|claim| claim.round),
        ring: proof.ring,
        ligerito: proof.ligerito,
        paths,
    })
}

fn separate_initial<'a, const N: usize>(
    geometry: &'a Geometry<N>,
    data: [&'a ProverData; N],
    paths: &'a mut [Vec<Hash>; N],
) -> impl FnOnce(usize, usize, &[usize]) -> RecursiveProof + 'a {
    move |positions, lanes, queries: &[usize]| {
        let mut rows = vec![vec![F::ZERO; lanes]; queries.len()];
        for branch in 0..N {
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
    }
}

#[allow(clippy::too_many_arguments)]
fn continue_prove_with_initial<Final: OpeningEncoding>(
    t: &mut (impl Transcript + Send),
    statement: &Hash,
    initial: PreparedInitial<'_>,
    target: F,
    ring: RingSwitchProof,
    resolved: &crate::ligerito_flock::ResolvedLigerito,
    open_initial: impl FnOnce(usize, usize, &[usize]) -> RecursiveProof,
    security: Option<&mut grinding::GrindingContext<'_>>,
) -> Result<JointProof<Final>, Error> {
    let _lig_scope = tracing::info_span!("op:ligerito").entered();
    let pc = resolved.prover();
    macro_rules! run {
        ($challenger:expr) => {{ Final::prove(&pc, initial, target, *statement, open_initial, $challenger) }};
    }
    let proof = if let Some(security) = security {
        let mut challenger = grinding::GrindingChallenger::new(t, security, resolved.security())?;
        let proof = run!(&mut challenger);
        if !challenger.finish() {
            return Err(Error::Invalid("shared Ligerito grinding schedule"));
        }
        proof
    } else {
        run!(&mut ZincChallenger(t))
    };
    Ok(JointProof {
        ring,
        ligerito: proof,
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn verify<const N: usize>(
    t: &mut (impl Transcript + Send),
    geometry: &Geometry<N>,
    statement: &Hash,
    roots: &[Hash; N],
    point: &[Gf],
    value: F,
    ood: Option<&OodVerifierClaim>,
    resolved: &crate::ligerito_flock::ResolvedLigerito,
    proof: &Proof<N>,
) -> Result<(), Error> {
    verify_with_security(
        t, geometry, statement, roots, point, value, ood, resolved, proof, None,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn verify_with_security<const N: usize>(
    t: &mut (impl Transcript + Send),
    geometry: &Geometry<N>,
    statement: &Hash,
    roots: &[Hash; N],
    point: &[Gf],
    value: F,
    ood: Option<&OodVerifierClaim>,
    resolved: &crate::ligerito_flock::ResolvedLigerito,
    proof: &Proof<N>,
    security: Option<&mut grinding::GrindingContext<'_>>,
) -> Result<(), Error> {
    let vc = resolved.verifier();
    let authenticate_initial = |positions, lanes, queries: &[usize], opening: &RecursiveProof| {
        if !opening.merkle_proof.is_empty() || lanes != geometry.lanes() {
            return false;
        }
        let mut hashes: [Vec<Hash>; N] = std::array::from_fn(|_| Vec::with_capacity(queries.len()));
        for row in &opening.opened_rows {
            // Authenticate every branch and reject all unused virtual lanes.
            if row.len() != lanes {
                return false;
            }
            for (lane, &word) in row.iter().enumerate() {
                let occupied = (0..N).any(|branch| {
                    let start = geometry.offset(branch);
                    (start..start + (1 << geometry.lane_logs[branch])).contains(&lane)
                });
                if !occupied && word != F::ZERO {
                    return false;
                }
            }
            for branch in 0..N {
                let start = geometry.offset(branch);
                let end = start + (1 << geometry.lane_logs[branch]);
                let mut bytes = Vec::with_capacity((end - start) * 16);
                for word in &row[start..end] {
                    bytes.extend_from_slice(&word.lo.to_le_bytes());
                    bytes.extend_from_slice(&word.hi.to_le_bytes());
                }
                hashes[branch].push(merkle::hash_leaf(&bytes, vc.merkle_hash));
            }
        }
        (0..N).all(|branch| {
            merkle::verify_merkle_multi_proof(
                &roots[branch],
                positions,
                queries,
                &hashes[branch],
                &proof.paths[branch],
                vc.merkle_hash,
            )
        })
    };
    verify_initial_with_security(
        t,
        geometry,
        statement,
        point,
        value,
        ood,
        resolved,
        &proof.ring,
        &proof.ligerito,
        &proof.ligerito.initial_proof,
        authenticate_initial,
        security,
    )
}

#[allow(clippy::too_many_arguments)]
fn verify_initial_with_security<const N: usize, Final: OpeningEncoding>(
    t: &mut (impl Transcript + Send),
    geometry: &Geometry<N>,
    statement: &Hash,
    point: &[Gf],
    value: F,
    ood: Option<&OodVerifierClaim>,
    resolved: &crate::ligerito_flock::ResolvedLigerito,
    ring: &RingSwitchProof,
    proof: &LigeritoProof<Final>,
    initial: &RecursiveProof,
    authenticate_initial: impl FnOnce(usize, usize, &[usize], &RecursiveProof) -> bool,
    security: Option<&mut grinding::GrindingContext<'_>>,
) -> Result<(), Error> {
    let (eq_r2, mut target) = ring_switch_verify(t, ring, value, &point[..7])
        .map_err(|_| Error::Invalid("ring switch"))?;
    let eta_ood: Option<Gf> = ood.map(|claim| {
        let eta: Gf = t.get_field_challenge(&());
        target += eta * claim.y;
        eta
    });
    let padding = sample_padding(t, geometry);
    let vc = resolved.verifier();
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
    if proof.recursive_roots.len() != vc.recursive_steps
        || proof.recursive_proofs.len() + 1 != vc.recursive_steps
        || proof.grinding_nonces.len() != vc.recursive_steps + 1
        || proof.ood_values.len() != expected_ood_values
        || proof.fold_grinding_nonces.len() != expected_fold_nonces
    {
        return Err(Error::Invalid("Ligerito proof shape"));
    }
    macro_rules! run {
        ($challenger:expr) => {
            Final::verify(
                &vc,
                proof,
                initial,
                geometry.packed_log(),
                target,
                statement,
                |prefix, log_y| {
                    let prefix_gf: Vec<_> = prefix.iter().copied().collect();
                    let mut out = residual_b_evals(&prefix_gf, log_y, &point[7..], &eq_r2);
                    if let (Some(ood), Some(eta)) = (ood, eta_ood) {
                        for (slot, term) in out
                            .iter_mut()
                            .zip(ood_residual_evals(prefix, log_y, &ood.point, eta))
                        {
                            *slot += term;
                        }
                    }
                    for (point, scale) in &padding {
                        for (slot, term) in out
                            .iter_mut()
                            .zip(ood_residual_evals(prefix, log_y, point, *scale))
                        {
                            *slot += term;
                        }
                    }
                    out.into_iter().collect()
                },
                authenticate_initial,
                $challenger,
            )
        };
    }
    let valid = if let Some(security) = security {
        let mut challenger = grinding::GrindingChallenger::new(t, security, resolved.security())?;
        let valid = run!(&mut challenger);
        valid && challenger.finish()
    } else {
        run!(&mut ZincChallenger(t))
    };
    if valid {
        Ok(())
    } else {
        Err(Error::Invalid("shared Ligerito opening"))
    }
}
