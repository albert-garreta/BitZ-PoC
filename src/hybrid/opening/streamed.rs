//! Deferred initial PCS tables. First messages are streamed over small tiles;
//! only the tables after two unchanged fold challenges are materialized.
use super::*;
use flock_core::field::Gf128Product;

pub(super) fn prepare<'a, const N: usize>(
    geometry: &'a Geometry<N>,
    sources: [&'a [F]; N],
    point: &'a [Gf],
    eq_r2: &'a [Gf],
    padding: (&'a [Gf], Gf),
    ood: Option<(&'a [Gf], Gf)>,
) -> ligerito::DeferredInitial<'a> {
    let (first_msg, lookahead) = initial_messages(geometry, sources, point, eq_r2, padding, ood);
    ligerito::DeferredInitial {
        log_n: geometry.packed_log(),
        first_msg,
        lookahead,
        fold: Box::new(move |a, b| {
            folded_tables(geometry, sources, point, eq_r2, padding, ood, [a, b])
        }),
    }
}

fn merge(mut a: [Gf128Product; 8], b: [Gf128Product; 8]) -> [Gf128Product; 8] {
    for (a, b) in a.iter_mut().zip(b) {
        *a ^= b;
    }
    a
}

fn initial_messages<const N: usize>(
    geometry: &Geometry<N>,
    sources: [&[F]; N],
    point: &[Gf],
    eq_r2: &[Gf],
    padding: (&[Gf], Gf),
    ood: Option<(&[Gf], Gf)>,
) -> (ligerito::SumcheckMessage, ligerito::FoldLookahead) {
    let _scope = tracing::info_span!("op:initial_messages").entered();
    let tiles = InitialBasisTiles::new(geometry, point, eq_r2, padding, ood);
    let block = tiles.block_len();
    // Several tiles per task amortize allocation; each worker needs 128 KiB
    // of reusable scratch rather than two full-domain vectors.
    const TILES: usize = 16;
    let partials: Vec<_> = crate::utils::cfg_into_iter!(0..tiles.tile_count().div_ceil(TILES))
        .map(|task| {
            let mut words = vec![F::ZERO; block];
            let mut basis = vec![F::ZERO; block];
            let mut acc = [Gf128Product::zero(); 8];
            for hi in task * TILES..((task + 1) * TILES).min(tiles.tile_count()) {
                geometry.write_words(sources, hi * block / geometry.lanes(), &mut words);
                tiles.write(hi, &mut basis);
                acc = merge(acc, ligerito::lookahead_accumulate(&words, &basis));
            }
            acc
        })
        .collect();
    let acc = partials.into_iter().fold([Gf128Product::zero(); 8], merge);
    ligerito::lookahead_finish(acc)
}

/// If B(u,y) = Phi(eq(h_low,u) * eq(h_high,y)), then after two folds
/// B'(y) = Psi(eq(h_high,y)), where
/// Psi(x) = sum_u eq(folds,u) * Phi(eq(h_low,u) * x).
/// Psi remains F2-linear; compile its 128 basis images once. This constructs
/// the quarter-sized table with one Phi-style lookup per output, not four.
fn folded_phi(point: &[Gf], eq_r2: &[Gf], folds: &[Gf; 2]) -> Vec<Gf> {
    let old_phi = phi_byte_tables(eq_r2, F::ONE);
    let low = super::super::sumcheck::eq_table(&point[..2]);
    let weights = super::super::sumcheck::eq_table(folds);
    let mut images = [F::ZERO; 128];
    for (bit, image) in images.iter_mut().enumerate() {
        let unit = F::from_polynomial_words(if bit < 64 {
            [1u64 << bit, 0]
        } else {
            [0, 1u64 << (bit - 64)]
        });
        for u in 0..4 {
            let product = low[u] * unit;
            *image += weights[u] * phi_from_words(*product.as_words(), &old_phi);
        }
    }
    phi_byte_tables(&images, F::ONE)
}

fn folded_tables<const N: usize>(
    geometry: &Geometry<N>,
    sources: [&[F]; N],
    point: &[Gf],
    eq_r2: &[Gf],
    padding: (&[Gf], Gf),
    ood: Option<(&[Gf], Gf)>,
    folds: [Gf; 2],
) -> ligerito::DeferredFold {
    let _scope = tracing::info_span!("op:deferred_first_fold").entered();
    let low = (geometry.packed_log() - 2).min(10);
    let block = 1 << low;
    let ring = EqualityTiles::new(&point[2..], low, F::ONE);
    let phi = folded_phi(point, eq_r2, &folds);
    let weights = super::super::sumcheck::eq_table(&folds);
    let padding_low = super::super::sumcheck::eq_table(&padding.0[..2]);
    let padding_sums =
        crate::ligerito::subset_sums_4(std::array::from_fn(|u| weights[u] * padding_low[u]));
    let padding_eq = EqualityTiles::new(&padding.0[2..], low, padding.1);
    let ood = ood.map(|(point, scale)| {
        let eq_low = super::super::sumcheck::eq_table(&point[..2]);
        let contraction = weights
            .iter()
            .zip(eq_low)
            .fold(F::ZERO, |sum, (&w, e)| sum + w * e);
        EqualityTiles::new(&point[2..], low, scale * contraction)
    });
    let live = geometry.live_groups();
    let size = 1 << (geometry.packed_log() - 2);
    // All slots are overwritten below, including unoccupied witness lanes.
    let mut f = flock_core::scratch::take_f128(size);
    let mut basis = flock_core::scratch::take_f128(size);
    let partials: Vec<_> = crate::utils::cfg_chunks_mut!(f, block)
        .zip(crate::utils::cfg_chunks_mut!(basis, block))
        .enumerate()
        .map(|(hi, (f, basis))| {
            let first_group = hi * block / 4;
            let mut words = vec![F::ZERO; 4 * f.len()];
            geometry.write_words(sources, first_group, &mut words);
            ligerito::fold_two_into(&words, f, folds[0], folds[1]);
            for (lo, coefficient) in basis.iter_mut().enumerate() {
                let weight = ring.head[hi] * ring.tail[lo];
                let mut value = phi_from_words(*weight.as_words(), &phi);
                let group = first_group + lo / 4;
                let lane = (lo % 4) * 4;
                // Missing witness lanes still have nonzero padding basis
                // coefficients. Contract that exact support mask, even when
                // the corresponding folded witness happens to be zero.
                let mut mask = 0;
                for u in 0..4 {
                    mask |= usize::from(group >= live[lane + u]) << u;
                }
                if mask != 0 {
                    value += padding_eq.head[hi] * padding_eq.tail[lo] * padding_sums[mask];
                }
                if let Some(ood) = &ood {
                    value += ood.head[hi] * ood.tail[lo];
                }
                *coefficient = value;
            }
            ligerito::lookahead_accumulate(f, basis)
        })
        .collect();
    let acc = partials.into_iter().fold([Gf128Product::zero(); 8], merge);
    let (message, lookahead) = ligerito::lookahead_finish(acc);
    ligerito::DeferredFold {
        f,
        basis,
        message,
        lookahead,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ligerito::bind_low;
    use rand::{RngExt, SeedableRng, rngs::StdRng};

    fn check<const N: usize>(logs: [usize; N]) {
        let geometry = Geometry::new(logs).unwrap();
        let mut rng = StdRng::seed_from_u64(0x4445464552524544);
        let mut sample = || F {
            lo: rng.random(),
            hi: rng.random(),
        };
        // Nonzero physical padding is intentional. The deferred representation
        // must fold the actual committed polynomial, not its logical truncation.
        let sources: [Vec<_>; N] = std::array::from_fn(|branch| {
            (0..1 << geometry.physical_logs[branch])
                .map(|_| sample())
                .collect()
        });
        let sources = sources.each_ref().map(Vec::as_slice);
        let point: Vec<_> = (0..geometry.packed_log()).map(|_| sample()).collect();
        let padding: Vec<_> = (0..geometry.packed_log()).map(|_| sample()).collect();
        let ood_point: Vec<_> = (0..geometry.packed_log()).map(|_| sample()).collect();
        let padding_scale = sample();
        let ood_scale = sample();
        let eq_r2 =
            super::super::super::sumcheck::eq_table(&std::array::from_fn::<_, 7, _>(|_| sample()));
        for ood in [None, Some((ood_point.as_slice(), ood_scale))] {
            let dense =
                geometry.initial_tables(sources, &point, &eq_r2, (&padding, padding_scale), ood);
            let (first, _) = initial_messages(
                &geometry,
                sources,
                &point,
                &eq_r2,
                (&padding, padding_scale),
                ood,
            );
            assert_eq!(first, dense.first_message);
            for folds in [
                [F::ZERO, F::ZERO],
                [F::ONE, F::ZERO],
                [F::ZERO, F::ONE],
                [F::ONE, F::ONE],
                [sample(), sample()],
            ] {
                let mut f = dense.packed.clone();
                let mut b = dense.basis.clone();
                for r in folds {
                    bind_low(&mut f, r);
                    bind_low(&mut b, r);
                }
                let folded = folded_tables(
                    &geometry,
                    sources,
                    &point,
                    &eq_r2,
                    (&padding, padding_scale),
                    ood,
                    folds,
                );
                assert_eq!(folded.f.len(), 1 << (geometry.packed_log() - 2));
                assert_eq!(folded.f, f);
                assert_eq!(folded.basis, b);
                // Inactive witness lanes can have nonzero folded basis terms;
                // equality of both full tables above protects that obligation.
                let (mut reference, msg) = ligerito::SumcheckProver::new(f, b, F::ZERO);
                assert_eq!(folded.message, msg);
                let (mut actual, _) = ligerito::SumcheckProver::new_with_first_msg(
                    folded.f,
                    folded.basis,
                    F::ZERO,
                    folded.message,
                );
                let r = sample();
                assert_eq!(actual.fold_skip(&folded.lookahead, r), reference.fold(r));
                actual.drain_pending_fold();
                assert_eq!(actual.f(), reference.f());
            }
        }
    }

    #[test]
    fn deferred_two_round_tables_match_dense_for_padding_ood_and_boolean_challenges() {
        check([9, 9]);
        check([9, 13]);
        check([9, 11, 10]);
        check([9, 14, 10]);
    }
}
