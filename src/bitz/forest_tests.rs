//! The packed forest must emit the dense GKR's exact transcript.

use super::{Forest, Gf};
use crate::bitz::gkr::{GrandProductCircuit, gpgkr_prove, gpgkr_verify};
use crate::bitz::transcript::{Proof, build_kernel_prover, build_kernel_verifier};

#[derive(Clone, Copy, Debug)]
enum Bits {
    Mixed,
    Zero,
    One,
}

fn next_word(state: &mut u64) -> u64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state
}

fn next_field(state: &mut u64) -> Gf {
    Gf::from_polynomial_words([next_word(state), next_word(state)])
}

/// A deliberately simple, little-endian MLE oracle: adjacent entries differ
/// in the first coordinate, independent of the forest's MSB-first passes.
fn evaluate(mut values: Vec<Gf>, point: &[Gf]) -> Gf {
    assert_eq!(values.len(), 1usize << point.len());
    for &r in point {
        let half = values.len() / 2;
        for i in 0..half {
            let lo = values[2 * i];
            let hi = values[2 * i + 1];
            values[i] = lo + r * (hi - lo);
        }
        values.truncate(half);
    }
    values[0]
}

fn compare_with_dense(t: usize, s: usize, bits: Bits) -> Proof {
    let rows = 1usize << t;
    let cols = 1usize << s;
    let mut state = 0x9E37_79B9_7F4A_7C15_u64 ^ ((t as u64) << 32) ^ s as u64;
    let images: Vec<Gf> = (0..rows)
        .map(|row| {
            if row % 7 == 0 {
                Gf::one()
            } else {
                next_field(&mut state)
            }
        })
        .collect();
    let packed: Vec<Vec<u64>> = (0..cols.div_ceil(64))
        .map(|_| {
            (0..rows)
                .map(|_| match bits {
                    Bits::Mixed => next_word(&mut state),
                    Bits::Zero => 0,
                    Bits::One => u64::MAX,
                })
                .collect()
        })
        .collect();
    // Unused high lanes in a partial 64-column group deliberately remain
    // nonzero; their weights must exclude them from every round sum.
    let mut leaves = Vec::with_capacity(rows * cols);
    for (row, &image) in images.iter().enumerate() {
        for column in 0..cols {
            let bit = (packed[column / 64][row] >> (column % 64)) & 1;
            leaves.push(if bit == 0 { Gf::one() } else { image });
        }
    }
    let zeta: Vec<Gf> = (0..s)
        .map(|coordinate| match coordinate % 3 {
            0 => Gf::zero(),
            1 => Gf::one(),
            _ => next_field(&mut state),
        })
        .collect();
    let instance = format!("t={t};s={s};bits={bits:?}");
    let session = "forest-dense-parity/v1";

    let (roots, witnesses) = GrandProductCircuit::new(leaves.clone()).batched_eval(cols);
    let root_claim = evaluate(roots, &zeta);
    let mut dense_transcript = build_kernel_prover(session, instance.as_str());
    let dense_terminal = gpgkr_prove(&mut dense_transcript, &zeta, witnesses);
    let dense_proof = dense_transcript.finish();

    let mut packed_transcript = build_kernel_prover(session, instance.as_str());
    let terminal = Forest::new(t, s, &packed, &images).prove(&mut packed_transcript, &zeta);
    let proof = packed_transcript.finish();
    assert_eq!(terminal, dense_terminal, "terminal: {instance}");
    assert_eq!(
        terminal.1,
        evaluate(leaves, &terminal.0),
        "leaf MLE: {instance}"
    );
    assert!(
        proof.narg_string == dense_proof.narg_string,
        "GKR transcript differs: {instance}; first differing byte {:?}",
        proof
            .narg_string
            .iter()
            .zip(&dense_proof.narg_string)
            .position(|(actual, expected)| actual != expected)
            .or_else(
                || (proof.narg_string.len() != dense_proof.narg_string.len())
                    .then_some(proof.narg_string.len().min(dense_proof.narg_string.len()))
            )
    );
    assert_eq!(proof.hints, dense_proof.hints, "hints: {instance}");

    let mut verifier = build_kernel_verifier(session, instance.as_str(), &proof);
    assert_eq!(
        gpgkr_verify(&mut verifier, root_claim, &zeta, t as u32),
        Some(terminal),
        "verification: {instance}"
    );
    verifier.check_eof().expect("all GKR messages consumed");

    let mut tampered = proof.clone();
    tampered.narg_string[0] ^= 1;
    let mut verifier = build_kernel_verifier(session, instance.as_str(), &tampered);
    assert!(
        gpgkr_verify(&mut verifier, root_claim, &zeta, t as u32).is_none(),
        "changed round message accepted: {instance}"
    );
    proof
}

#[test]
fn packed_forest_matches_dense_across_lookup_paths() {
    for (t, s) in [
        (4, 6), // Tiny materialized fallback.
        (5, 6),
        (6, 9),  // Below the 1024-column prescaling threshold.
        (6, 10), // At the threshold.
        (7, 12), // Wide rows and several column groups.
        (10, 10), // Multiple scaling chunks per side of a position table.
        (7, 0),  // One live lane, no column variables.
        (7, 3),  // A partial 64-column group.
    ] {
        compare_with_dense(t, s, Bits::Mixed);
    }
}

#[test]
fn packed_forest_matches_dense_for_constant_bits() {
    for bits in [Bits::Zero, Bits::One] {
        compare_with_dense(7, 10, bits);
    }
}

#[test]
fn partial_forest_matches_dense_and_binds_interleaved_images() {
    // A tiny materialized case, the JIT path, and the wide prescaled path.
    for (row_bits, column_bits) in [(4, 3), (6, 6), (6, 10)] {
        let rows = 1usize << row_bits;
        let columns = 1usize << column_bits;
        let mut state = 0x47D2_9AE3_C168_05B9;
        let original: Vec<Vec<u64>> = (0..columns.div_ceil(64))
            .map(|_| (0..rows).map(|_| next_word(&mut state)).collect())
            .collect();
        let packed: Vec<Vec<u64>> = original
            .iter()
            .map(|group| group.iter().flat_map(|&bits| [bits, bits]).collect())
            .collect();
        let images: Vec<Gf> = (0..2 * rows).map(|_| next_field(&mut state)).collect();
        let mut leaves = Vec::with_capacity(2 * rows * columns);
        for (row, &image) in images.iter().enumerate() {
            for column in 0..columns {
                let bit = (original[column / 64][row / 2] >> (column % 64)) & 1;
                leaves.push(if bit == 0 { Gf::one() } else { image });
            }
        }
        let zeta: Vec<Gf> = (0..column_bits + 1)
            .map(|i| match i % 3 {
                0 => Gf::zero(),
                1 => Gf::one(),
                _ => next_field(&mut state),
            })
            .collect();
        let (roots, witnesses) =
            GrandProductCircuit::new(leaves.clone()).batched_eval(2 * columns);
        // Roots are columns first, then the unmultiplied low row bit (limb).
        for limb in 0..2 {
            for column in 0..columns {
                let expected = (0..rows).fold(Gf::one(), |acc, row| {
                    if (original[column / 64][row] >> (column % 64)) & 1 == 0 {
                        acc
                    } else {
                        acc * images[2 * row + limb]
                    }
                });
                assert_eq!(roots[limb * columns + column], expected);
            }
        }
        let root_claim = evaluate(roots, &zeta);
        let session = "forest-partial-parity/v1";
        let instance = format!("rows={row_bits};columns={column_bits}");
        let mut dense = build_prover(session, instance.as_str());
        let dense_terminal = gpgkr_prove(&mut dense, &zeta, witnesses);
        let dense_proof = dense.finish();
        let mut prover = build_prover(session, instance.as_str());
        let terminal = Forest::new(row_bits + 1, column_bits, &packed, &images)
            .prove_depth(&mut prover, &zeta, row_bits);
        let proof = prover.finish();
        assert_eq!(terminal, dense_terminal);
        assert_eq!(proof, dense_proof);
        assert_eq!(terminal.1, evaluate(leaves, &terminal.0));

        // Authenticate the common source bits from [column | limb | row].
        // This oracle evaluates equality weights directly, independently of
        // the forest's table builders and variable-binding order.
        let eq_at = |point: &[Gf], index: usize| {
            point.iter().enumerate().fold(Gf::one(), |acc, (bit, &r)| {
                acc * if index >> bit & 1 == 0 { Gf::one() + r } else { r }
            })
        };
        let column_point = &terminal.0[..column_bits];
        let limb_point = terminal.0[column_bits];
        let row_point = &terminal.0[column_bits + 1..];
        let mut target = Gf::zero();
        for row in 0..rows {
            let image_weight = (Gf::one() + limb_point) * (images[2 * row] + Gf::one())
                + limb_point * (images[2 * row + 1] + Gf::one());
            let low = eq_at(row_point, row) * image_weight;
            for column in 0..columns {
                if (original[column / 64][row] >> (column % 64)) & 1 != 0 {
                    target += low * eq_at(column_point, column);
                }
            }
        }
        assert_eq!(target, terminal.1 + Gf::one());

        let mut verifier = build_verifier(session, instance.as_str(), &proof);
        assert_eq!(
            gpgkr_verify(&mut verifier, root_claim, &zeta, row_bits as u32),
            Some(terminal)
        );
        verifier.check_eof().unwrap();

        let mut changed = proof.clone();
        changed.narg_string[0] ^= 1;
        let mut verifier = build_verifier(session, instance.as_str(), &changed);
        assert!(gpgkr_verify(&mut verifier, root_claim, &zeta, row_bits as u32).is_none());
    }
}

#[cfg(feature = "parallel")]
#[test]
fn packed_forest_transcript_is_independent_of_thread_count() {
    let prove = |threads| {
        rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .expect("test thread pool")
            .install(|| compare_with_dense(7, 10, Bits::Mixed))
    };
    assert_eq!(prove(1), prove(4));
}
