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

fn initial_tiles_per_task_override() -> Option<usize> {
    #[cfg(feature = "bench-internals")]
    return match std::env::var("BITZ_PCS_INITIAL_TILES_PER_TASK") {
        Err(std::env::VarError::NotPresent) => None,
        Ok(value) => match value.as_str() {
            "16" => Some(16),
            "4" => Some(4),
            "2" => Some(2),
            _ => panic!("BITZ_PCS_INITIAL_TILES_PER_TASK must be 16, 4, or 2"),
        },
        Err(std::env::VarError::NotUnicode(_)) => {
            panic!("BITZ_PCS_INITIAL_TILES_PER_TASK must be valid Unicode")
        }
    };
    #[cfg(not(feature = "bench-internals"))]
    None
}

fn default_initial_tiles_per_task(tile_count: usize, workers: usize) -> usize {
    // Qualified for Falcon-512/1024 batch 1024 at these worker counts; retain
    // the previous grouping for serial execution and unqualified workloads.
    if matches!(tile_count, 32 | 64) && matches!(workers, 8 | 12 | 16) {
        4
    } else {
        16
    }
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
    initial_messages_with_tiles_per_task(
        geometry,
        sources,
        point,
        eq_r2,
        padding,
        ood,
        initial_tiles_per_task_override(),
    )
}

fn initial_messages_with_tiles_per_task<const N: usize>(
    geometry: &Geometry<N>,
    sources: [&[F]; N],
    point: &[Gf],
    eq_r2: &[Gf],
    padding: (&[Gf], Gf),
    ood: Option<(&[Gf], Gf)>,
    tiles_per_task: Option<usize>,
) -> (ligerito::SumcheckMessage, ligerito::FoldLookahead) {
    let tiles = InitialBasisTiles::new(geometry, point, eq_r2, padding, ood);
    let tiles_per_task = tiles_per_task.unwrap_or_else(|| {
        #[cfg(feature = "parallel")]
        let workers = rayon::current_num_threads();
        #[cfg(not(feature = "parallel"))]
        let workers = 1;
        default_initial_tiles_per_task(tiles.tile_count(), workers)
    });
    assert!(
        tiles_per_task > 0,
        "initial task must contain at least one tile"
    );
    let block = tiles.block_len();
    // Grouping changes scheduling only: each task keeps at most 128 KiB of
    // scratch, visits complete tiles, and returns the same XOR accumulators.
    let partials: Vec<_> =
        crate::utils::cfg_into_iter!(0..tiles.tile_count().div_ceil(tiles_per_task))
            .map(|task| {
                let mut words = vec![F::ZERO; block];
                let mut basis = vec![F::ZERO; block];
                let mut acc = [Gf128Product::zero(); 8];
                let first = task * tiles_per_task;
                let end = first + tiles_per_task.min(tiles.tile_count() - first);
                for hi in first..end {
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

    fn check<const N: usize>(geometry: Geometry<N>) {
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
            // Include a non-power-of-two grouping to exercise a partial final
            // task after full tasks, as well as every benchmark strategy.
            for tiles_per_task in [16, 4, 2, 3] {
                let (first, lookahead) = initial_messages_with_tiles_per_task(
                    &geometry,
                    sources,
                    &point,
                    &eq_r2,
                    (&padding, padding_scale),
                    ood,
                    Some(tiles_per_task),
                );
                assert_eq!(first, dense.first_message);
                for r in [F::ZERO, F::ONE, F { lo: 7, hi: 11 }] {
                    let (mut reference, reference_first) = ligerito::SumcheckProver::new(
                        dense.packed.clone(),
                        dense.basis.clone(),
                        F::ZERO,
                    );
                    assert_eq!(first, reference_first);
                    let (mut actual, _) = ligerito::SumcheckProver::new_with_first_msg(
                        dense.packed.clone(),
                        dense.basis.clone(),
                        F::ZERO,
                        first,
                    );
                    assert_eq!(actual.fold_skip(&lookahead, r), reference.fold(r));
                    actual.drain_pending_fold();
                    assert_eq!(actual.f(), reference.f());
                    // A following ordinary round checks the folded basis too.
                    let next = F { lo: 13, hi: 17 };
                    assert_eq!(actual.fold(next), reference.fold(next));
                }
            }
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
    fn initial_task_default_is_limited_to_qualified_shapes_and_workers() {
        for tiles in [32, 64] {
            for workers in [8, 12, 16] {
                assert_eq!(default_initial_tiles_per_task(tiles, workers), 4);
            }
            for workers in [1, 2, 4, 7, 9, 15, 17, 32] {
                assert_eq!(default_initial_tiles_per_task(tiles, workers), 16);
            }
        }
        for tiles in [1, 16, 31, 33, 63, 65, 128] {
            for workers in [1, 8, 12, 16] {
                assert_eq!(default_initial_tiles_per_task(tiles, workers), 16);
            }
        }
    }

    #[test]
    fn deferred_two_round_tables_match_dense_for_padding_ood_and_boolean_challenges() {
        check(Geometry::new([9, 9]).unwrap());
        check(Geometry::new([9, 13]).unwrap());
        check(Geometry::new([9, 11, 10]).unwrap());
        check(Geometry::new([9, 14, 10]).unwrap());
    }

    #[test]
    fn initial_message_task_sizes_match_dense_across_worker_counts_and_allocations() {
        for workers in [1, 4] {
            rayon::ThreadPoolBuilder::new()
                .num_threads(workers)
                .build()
                .unwrap()
                .install(|| {
                    // Falcon-512 batch 1024: 32 tiles, giving 2/8/16 tasks for
                    // the three benchmark strategies and a tail for size 3.
                    check(Geometry::new([16, 15, 13]).unwrap());
                    // Logical supports are smaller than committed allocations;
                    // preserve nonzero physical padding and inactive lanes.
                    check(Geometry::with_allocation_logs([9, 9, 9], [14, 13, 12]).unwrap());
                });
        }
    }
}
