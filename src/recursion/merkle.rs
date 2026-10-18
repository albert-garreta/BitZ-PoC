//! Flock's SHA-256 Merkle trees in the circuit: leaf digests of opened rows,
//! pair hashes, and the octopus multi-proof of
//! `flock_core::merkle::verify_merkle_multi_proof`. Query positions are
//! public, so every node's child order is known when the circuit is built.

use circuit::Circuit;
use circuit::sha256::Word;

use super::bytes::{Byte, bytes_of_state, gf_bytes, public_bytes, word_of_bytes, witness_bytes};
use super::sha::{ShaCounts, sha256};
use super::val::G;

/// A SHA-256 digest as eight big-endian words.
pub type Digest<B> = [Word<B, 32>; 8];

/// A committed digest (proof data).
pub fn witness_digest<CS: Circuit>(cs: &mut CS, bytes: [u8; 32]) -> Digest<CS::Bool> {
    let b = witness_bytes(cs, &bytes);
    std::array::from_fn(|w| word_of_bytes(&b[4 * w..4 * w + 4]))
}

/// A public digest.
pub fn public_digest<B: circuit::BoolWitness>(bytes: [u8; 32]) -> Digest<B> {
    let b = public_bytes::<B>(&bytes);
    std::array::from_fn(|w| word_of_bytes(&b[4 * w..4 * w + 4]))
}

/// The digest's 32 bytes in stream order.
pub fn digest_bytes<B: circuit::BoolWitness>(d: &Digest<B>) -> Vec<Byte<B>> {
    bytes_of_state(d)
}

/// The Merkle hash of `bytes` under [`crate::ligerito_flock::MERKLE_HASH`]:
/// standard padded SHA-256, or the raw compression chain from the IV
/// (`HashKind::Sha256Compress`, the last block zero-padded, no length).
fn merkle_hash<CS: Circuit>(cs: &mut CS, bytes: &[Byte<CS::Bool>], counts: &mut ShaCounts) -> Digest<CS::Bool> {
    use flock_core::merkle::HashKind;
    match crate::ligerito_flock::MERKLE_HASH {
        HashKind::Sha256 => sha256(cs, bytes, counts),
        HashKind::Sha256Compress => {
            let mut cv: super::sha::Cv<CS::Bool> = super::sha::Cv::P(crate::transcript::sha::IV);
            let mut blocks: Vec<Vec<Byte<CS::Bool>>> = bytes.chunks(64).map(|c| c.to_vec()).collect();
            if blocks.is_empty() {
                blocks.push(Vec::new());
            }
            for mut block in blocks {
                block.resize(64, Byte::P(0));
                cv = super::sha::compress(cs, &cv, &block, counts);
            }
            cv.into_words()
        }
        HashKind::Blake3 => unreachable!("the verifier circuit hashes with SHA-256"),
    }
}

/// The leaf digest of a row of 16-byte little-endian elements.
pub fn leaf_hash<CS: Circuit>(cs: &mut CS, row: &[G<CS::Bool>], counts: &mut ShaCounts) -> Digest<CS::Bool> {
    let bytes: Vec<Byte<CS::Bool>> = row.iter().flat_map(gf_bytes).collect();
    merkle_hash(cs, &bytes, counts)
}

/// The node digest of `left ‖ right`.
pub fn pair_hash<CS: Circuit>(
    cs: &mut CS,
    left: &Digest<CS::Bool>,
    right: &Digest<CS::Bool>,
    counts: &mut ShaCounts,
) -> Digest<CS::Bool> {
    let mut bytes = digest_bytes(left);
    bytes.extend(digest_bytes(right));
    merkle_hash(cs, &bytes, counts)
}

/// The number of sibling hashes the multi-proof of `positions` (sorted,
/// unique) carries in a tree of `num_leaves` leaves.
pub fn multi_proof_len(num_leaves: usize, positions: &[usize]) -> usize {
    if positions.is_empty() || num_leaves == 1 {
        return 0;
    }
    let mut active = positions.to_vec();
    let mut count = 0;
    let mut level_len = num_leaves;
    while level_len > 1 {
        let mut next = Vec::with_capacity(active.len());
        let mut i = 0;
        while i < active.len() {
            let p = active[i];
            if i + 1 < active.len() && active[i + 1] == (p ^ 1) {
                i += 2;
            } else {
                count += 1;
                i += 1;
            }
            next.push(p >> 1);
        }
        active = next;
        level_len >>= 1;
    }
    count
}

/// The root the multi-proof reconstructs from the leaf digests at the sorted
/// unique `positions` (`siblings` in proof order).
pub fn multi_proof_root<CS: Circuit>(
    cs: &mut CS,
    num_leaves: usize,
    positions: &[usize],
    leaves: Vec<Digest<CS::Bool>>,
    siblings: &[Digest<CS::Bool>],
    counts: &mut ShaCounts,
) -> Digest<CS::Bool> {
    assert!(num_leaves.is_power_of_two());
    assert_eq!(positions.len(), leaves.len());
    assert!(positions.windows(2).all(|w| w[0] < w[1]));
    let mut active: Vec<(usize, Digest<CS::Bool>)> = positions.iter().copied().zip(leaves).collect();
    let mut next_sibling = siblings.iter();
    let mut level_len = num_leaves;
    while level_len > 1 {
        let mut next = Vec::with_capacity(active.len());
        let mut i = 0;
        while i < active.len() {
            let p = active[i].0;
            let parent = if i + 1 < active.len() && active[i + 1].0 == (p ^ 1) {
                let h = pair_hash(cs, &active[i].1, &active[i + 1].1, counts);
                i += 2;
                h
            } else {
                let sib = next_sibling.next().expect("multi-proof length");
                let h = if p & 1 == 0 {
                    pair_hash(cs, &active[i].1, sib, counts)
                } else {
                    pair_hash(cs, sib, &active[i].1, counts)
                };
                i += 1;
                h
            };
            next.push((p >> 1, parent));
        }
        active = next;
        level_len >>= 1;
    }
    assert!(next_sibling.next().is_none(), "multi-proof length");
    assert_eq!(active.len(), 1);
    active.pop().unwrap().1
}

/// Asserts two digests are equal.
pub fn assert_digest_eq<CS: Circuit>(cs: &mut CS, a: &Digest<CS::Bool>, b: &Digest<CS::Bool>) {
    let diff: Vec<CS::Bool> = (0..256)
        .map(|i| cs.xor(a[i / 32].bit(i % 32), b[i / 32].bit(i % 32)))
        .collect();
    circuit::gf128::assert_zero(cs, &diff);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recursion::val::{gf_of, witness};
    use circuit::witgen::CheckingWitgen;
    use flock_core::merkle::{merkle_multi_proof, merkle_tree};
    use field::Gf128 as Gf;

    #[test]
    fn multi_proof_matches_flock() {
        let lanes = 16;
        let num_leaves = 64;
        let rows: Vec<Vec<Gf>> = (0..num_leaves)
            .map(|i| (0..lanes).map(|j| gf_of((i * 1000 + j) as u128 * 0x9e37_79b9_7f4a_7c15)).collect())
            .collect();
        let data: Vec<u8> = rows.iter().flatten().flat_map(|g| g.to_bytes()).collect();
        let tree = merkle_tree(&data, num_leaves, crate::ligerito_flock::MERKLE_HASH);
        let root = tree[tree.len() - 1];
        for positions in [vec![0usize], vec![3, 4, 5, 40, 63], (0..64).step_by(3).collect::<Vec<_>>()] {
            let proof = merkle_multi_proof(&tree, num_leaves, &positions);
            assert_eq!(proof.len(), multi_proof_len(num_leaves, &positions));
            for tamper in [false, true] {
                let mut w = CheckingWitgen::new();
                let mut counts = ShaCounts::default();
                let leaves: Vec<_> = positions
                    .iter()
                    .map(|&p| {
                        let mut row = rows[p].clone();
                        if tamper && p == positions[0] {
                            row[1] += Gf::ONE;
                        }
                        let g: Vec<_> = row.iter().map(|&v| witness(&mut w, v)).collect();
                        leaf_hash(&mut w, &g, &mut counts)
                    })
                    .collect();
                let sibs: Vec<_> = proof.iter().map(|&h| witness_digest(&mut w, h)).collect();
                let got = multi_proof_root(&mut w, num_leaves, &positions, leaves, &sibs, &mut counts);
                let expected = public_digest(root);
                assert_digest_eq(&mut w, &got, &expected);
                assert_eq!(w.first_failure.is_none(), !tamper, "{positions:?} tamper {tamper}");
            }
        }
    }
}
