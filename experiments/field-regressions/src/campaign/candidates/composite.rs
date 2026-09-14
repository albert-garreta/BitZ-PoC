use crate::{Case, Rng, arithmetic, case_requested, measure};
use f2z::{poly::univariate::binary_gf128::BinaryFieldGF128 as Gf, utils::wide_mul::WideMulAcc};
use flock_core::{field::F128, ntt::AdditiveNttF128};
use rayon::prelude::*;
use std::hint::black_box;

// Write each equality weight once per coordinate, reuse its product for both
// children, and allocate the final table once. Index bit k belongs to point[k].
fn eq_into(point: &[Gf], out: &mut [Gf]) {
    assert_eq!(out.len(), 1usize << point.len());
    out[0] = Gf::one();
    let mut n = 1;
    for &z in point {
        let (lo, hi) = out[..2 * n].split_at_mut(n);
        for (a, b) in lo.iter_mut().zip(hi) {
            *b = *a * z;
            *a += *b;
        }
        n *= 2;
    }
}
fn eq(point: &[Gf]) -> Vec<Gf> {
    let mut out = vec![Gf::zero(); 1 << point.len()];
    eq_into(point, &mut out);
    out
}

#[inline(never)]
fn ood_reuse(p: &[F128], point: &[Gf], tail: &mut [Gf], head: &mut [Gf]) -> Gf {
    assert_eq!(p.len(), 1usize << point.len());
    let lo = point.len().min(12);
    let block = 1 << lo;
    assert_eq!(tail.len(), block);
    assert_eq!(head.len(), p.len() / block);
    eq_into(&point[..lo], tail);
    eq_into(&point[lo..], head);
    // Parallel reduction removes the intermediate inner Vec. Reduction ordering
    // is exact over the binary field. Scratch covers only public shape.
    let wide = p
        .par_chunks_exact(block)
        .zip(head.par_iter())
        .map(|(p, &h)| {
            let mut acc = Gf::wide_zero(&Gf::zero());
            for (&m, t) in p.iter().zip(tail.iter()) {
                Gf::wide_add_assign(&mut acc, &Gf::mul_wide(&Gf::from_words([m.lo, m.hi]), t));
            }
            Gf::mul_wide(&Gf::from_wide(acc), &h)
        })
        .reduce(
            || Gf::wide_zero(&Gf::zero()),
            |mut a, b| {
                Gf::wide_add_assign(&mut a, &b);
                a
            },
        );
    Gf::from_wide(wide)
}
fn ood(p: &[F128], point: &[Gf]) -> Gf {
    let lo = point.len().min(12);
    let mut tail = vec![Gf::zero(); 1 << lo];
    let mut head = vec![Gf::zero(); 1 << (point.len() - lo)];
    ood_reuse(p, point, &mut tail, &mut head)
}

// Indexed jobs retain the production block decomposition. Each job replaces
// its head weight with its weighted dot, so the final sum is a short serial
// XOR and no wide Rayon reduction tree or intermediate output Vec is needed.
fn ood_indexed_reuse(p: &[F128], point: &[Gf], tail: &mut [Gf], head: &mut [Gf]) -> Gf {
    assert_eq!(p.len(), 1usize << point.len());
    let lo = point.len().min(12);
    let block = 1 << lo;
    assert_eq!(tail.len(), block);
    assert_eq!(head.len(), p.len() / block);
    eq_into(&point[..lo], tail);
    eq_into(&point[lo..], head);
    head.par_iter_mut().enumerate().for_each(|(hi, h)| {
        let mut acc = Gf::wide_zero(&Gf::zero());
        for (j, t) in tail.iter().enumerate() {
            let m = p[hi * block + j];
            Gf::wide_add_assign(&mut acc, &Gf::mul_wide(&Gf::from_words([m.lo, m.hi]), t));
        }
        *h = Gf::from_wide(acc) * *h;
    });
    head.iter().fold(Gf::zero(), |a, &b| a + b)
}
fn ood_indexed(p: &[F128], point: &[Gf]) -> Gf {
    let lo = point.len().min(12);
    let mut tail = vec![Gf::zero(); 1 << lo];
    let mut head = vec![Gf::zero(); 1 << (point.len() - lo)];
    ood_indexed_reuse(p, point, &mut tail, &mut head)
}
fn ood_collect(p: &[F128], point: &[Gf]) -> Gf {
    assert_eq!(p.len(), 1usize << point.len());
    let lo = point.len().min(12);
    let block = 1 << lo;
    let tail = eq(&point[..lo]);
    let head = eq(&point[lo..]);
    let inner: Vec<Gf> = (0..head.len())
        .into_par_iter()
        .map(|hi| {
            let mut acc = Gf::wide_zero(&Gf::zero());
            for (j, t) in tail.iter().enumerate() {
                let m = p[hi * block + j];
                Gf::wide_add_assign(&mut acc, &Gf::mul_wide(&Gf::from_words([m.lo, m.hi]), t));
            }
            Gf::from_wide(acc)
        })
        .collect();
    inner
        .iter()
        .zip(&head)
        .fold(Gf::zero(), |a, (&i, &h)| a + i * h)
}

#[inline(never)]
fn dot3(a: &[F128], b: &[Gf]) -> Gf {
    assert_eq!(a.len(), b.len());
    #[cfg(all(target_arch = "aarch64", target_feature = "aes"))]
    unsafe {
        use core::arch::aarch64::*;
        let mut lo = vdupq_n_u64(0);
        let mut hi = vdupq_n_u64(0);
        let mut mid = vdupq_n_u64(0);
        for (a, b) in a.iter().zip(b) {
            // Both loads cover complete two-word arrays; neither requires
            // alignment or an assumption about the field wrapper's layout.
            let words = [a.lo, a.hi];
            let a = vld1q_u64(words.as_ptr());
            let b = vld1q_u64(b.words().as_ptr());
            lo = veorq_u64(
                lo,
                vreinterpretq_u64_p128(vmull_p64(vgetq_lane_u64::<0>(a), vgetq_lane_u64::<0>(b))),
            );
            hi = veorq_u64(
                hi,
                vreinterpretq_u64_p128(vmull_high_p64(
                    vreinterpretq_p64_u64(a),
                    vreinterpretq_p64_u64(b),
                )),
            );
            let ax = veorq_u64(a, vextq_u64::<1>(a, a));
            let bx = veorq_u64(b, vextq_u64::<1>(b, b));
            mid = veorq_u64(
                mid,
                vreinterpretq_u64_p128(vmull_p64(vgetq_lane_u64::<0>(ax), vgetq_lane_u64::<0>(bx))),
            );
        }
        let cross = veorq_u64(veorq_u64(mid, lo), hi);
        lo = veorq_u64(lo, vextq_u64::<1>(vdupq_n_u64(0), cross));
        hi = veorq_u64(hi, vextq_u64::<1>(cross, vdupq_n_u64(0)));
        Gf::reduce_wide([
            vgetq_lane_u64::<0>(lo),
            vgetq_lane_u64::<1>(lo),
            vgetq_lane_u64::<0>(hi),
            vgetq_lane_u64::<1>(hi),
        ])
    }
    #[cfg(not(all(target_arch = "aarch64", target_feature = "aes")))]
    {
        let mut acc = Gf::wide_zero(&Gf::zero());
        for (a, b) in a.iter().zip(b) {
            Gf::wide_add_assign(&mut acc, &Gf::mul_wide(&Gf::from_words([a.lo, a.hi]), b));
        }
        Gf::from_wide(acc)
    }
}
fn ood_vector(p: &[F128], point: &[Gf]) -> Gf {
    assert_eq!(p.len(), 1usize << point.len());
    let lo = point.len().min(12);
    let block = 1 << lo;
    let tail = eq(&point[..lo]);
    let head = eq(&point[lo..]);
    let inner: Vec<_> = p.par_chunks_exact(block).map(|p| dot3(p, &tail)).collect();
    inner
        .iter()
        .zip(&head)
        .fold(Gf::zero(), |a, (&i, &h)| a + i * h)
}
fn eq_prepared(point: &[Gf]) -> Vec<Gf> {
    let mut out = vec![Gf::zero(); 1 << point.len()];
    out[0] = Gf::one();
    let mut n = 1;
    for z in point {
        let w = z.words();
        let prep = arithmetic::Prepared::new(F128::new(w[0], w[1]));
        let (lo, hi) = out[..2 * n].split_at_mut(n);
        for (a, b) in lo.iter_mut().zip(hi) {
            let w = a.words();
            let p = prep.mul(F128::new(w[0], w[1]));
            *b = Gf::from_words([p.lo, p.hi]);
            *a += *b;
        }
        n *= 2;
    }
    out
}
fn ood_prepared(p: &[F128], point: &[Gf]) -> Gf {
    assert_eq!(p.len(), 1usize << point.len());
    let lo = point.len().min(12);
    let block = 1 << lo;
    let tail = eq_prepared(&point[..lo]);
    let head = eq_prepared(&point[lo..]);
    let inner: Vec<_> = (0..head.len())
        .into_par_iter()
        .map(|hi| {
            let mut acc = Gf::wide_zero(&Gf::zero());
            for (j, t) in tail.iter().enumerate() {
                let m = p[hi * block + j];
                Gf::wide_add_assign(&mut acc, &Gf::mul_wide(&Gf::from_words([m.lo, m.hi]), t));
            }
            Gf::from_wide(acc)
        })
        .collect();
    inner
        .iter()
        .zip(&head)
        .fold(Gf::zero(), |a, (&i, &h)| a + i * h)
}
fn ood_cases(samples: usize, rng: &mut Rng) {
    for log in [0, 1, 3, 10, 16, 18] {
        let size = format!("log{log}");
        if log >= 10
            && !["opt_ood", "opt_ood_reuse"]
                .iter()
                .any(|f| case_requested(f, &size))
        {
            continue;
        }
        let p = rng.values(1 << log);
        let point: Vec<_> = rng
            .values(log)
            .iter()
            .map(|v| Gf::from_words([v.lo, v.hi]))
            .collect();
        let expected = crate::production_ood::evaluate(&p, &point);
        assert_eq!(ood(&p, &point), expected);
        assert_eq!(ood_indexed(&p, &point), expected);
        assert_eq!(ood_collect(&p, &point), expected);
        assert_eq!(ood_vector(&p, &point), expected);
        assert_eq!(ood_prepared(&p, &point), expected);
        let weights = eq(&point);
        let oracle = p.iter().zip(&weights).fold(Gf::zero(), |acc, (p, w)| {
            acc + Gf::from_words([p.lo, p.hi]) * *w
        });
        assert_eq!(oracle, expected);
        if log < 10 {
            continue;
        }
        let mut cases = vec![
            Case::new("production_blocked", p.len() * 16, || {
                black_box(crate::production_ood::evaluate(
                    black_box(&p),
                    black_box(&point),
                ));
            }),
            Case::new("reuse_products", p.len() * 16, || {
                black_box(ood(black_box(&p), black_box(&point)));
            }),
            Case::new("indexed_products", p.len() * 16, || {
                black_box(ood_indexed(black_box(&p), black_box(&point)));
            }),
            Case::new("collect_products", p.len() * 16, || {
                black_box(ood_collect(black_box(&p), black_box(&point)));
            }),
            Case::new("vector_products", p.len() * 16, || {
                black_box(ood_vector(black_box(&p), black_box(&point)));
            }),
            Case::new("prepared_products", p.len() * 16, || {
                black_box(ood_prepared(black_box(&p), black_box(&point)));
            }),
        ];
        measure("opt_ood", &size, &mut cases, samples, rng);
        let lo = log.min(12);
        let mut tail = vec![Gf::zero(); 1 << lo];
        let mut head = vec![Gf::zero(); 1 << (log - lo)];
        let mut indexed_tail = tail.clone();
        let mut indexed_head = head.clone();
        let mut cases = vec![
            Case::new("allocate", p.len() * 16, || {
                black_box(ood(black_box(&p), black_box(&point)));
            }),
            Case::new("scratch", p.len() * 16, || {
                black_box(ood_reuse(
                    black_box(&p),
                    black_box(&point),
                    black_box(&mut tail),
                    black_box(&mut head),
                ));
            }),
            Case::new("indexed_scratch", p.len() * 16, || {
                black_box(ood_indexed_reuse(
                    black_box(&p),
                    black_box(&point),
                    black_box(&mut indexed_tail),
                    black_box(&mut indexed_head),
                ));
            }),
        ];
        measure("opt_ood_reuse", &size, &mut cases, samples, rng);
    }
}

#[inline(always)]
fn butterfly(values: &mut [F128], t: F128) {
    let (a, b) = values.split_at_mut(values.len() / 2);
    if t == F128::ZERO {
        for (a, b) in a.iter_mut().zip(b) {
            *b += *a;
        }
    } else {
        let p = arithmetic::Prepared::new(t);
        for (a, b) in a.iter_mut().zip(b) {
            *a += p.mul(*b);
            *b += *a;
        }
    }
}

// Cache-local depth-first schedule, with public block/length decisions. It
// does not allocate a layer plan or enter a parallel iterator at every layer.
fn subtree(ntt: &AdditiveNttF128, data: &mut [F128], lanes: usize, layer: usize, block: usize) {
    if data.len() == lanes {
        return;
    }
    butterfly(data, ntt.twiddle(layer, block));
    let len = data.len();
    let (a, b) = data.split_at_mut(len / 2);
    if len >= 1 << 15 && rayon::current_num_threads() > 1 {
        rayon::join(
            || subtree(ntt, a, lanes, layer + 1, block * 2),
            || subtree(ntt, b, lanes, layer + 1, block * 2 + 1),
        );
    } else {
        subtree(ntt, a, lanes, layer + 1, block * 2);
        subtree(ntt, b, lanes, layer + 1, block * 2 + 1);
    }
}

#[inline(always)]
fn improved_pairs(a: &mut [F128], b: &mut [F128], t: F128) {
    if t == F128::ZERO {
        for (a, b) in a.iter_mut().zip(b) {
            *b += *a;
        }
    } else if t.hi == 0 {
        for (a, b) in a.iter_mut().zip(b) {
            *a += super::binary::half(*b, t.lo);
            *b += *a;
        }
    } else {
        let p = arithmetic::Prepared::new(t);
        for (a, b) in a.iter_mut().zip(b) {
            *a += p.mul(*b);
            *b += *a;
        }
    }
}
fn serial_tree(ntt: &AdditiveNttF128, data: &mut [F128], lanes: usize, layer: usize, block: usize) {
    if data.len() == lanes {
        return;
    }
    let (a, b) = data.split_at_mut(data.len() / 2);
    improved_pairs(a, b, ntt.twiddle(layer, block));
    serial_tree(ntt, a, lanes, layer + 1, block * 2);
    serial_tree(ntt, b, lanes, layer + 1, block * 2 + 1);
}
fn half_depth(ntt: &AdditiveNttF128, data: &mut [F128], lanes: usize, layer: usize, block: usize) {
    if data.len() == lanes {
        return;
    }
    let len = data.len();
    let (a, b) = data.split_at_mut(len / 2);
    improved_pairs(a, b, ntt.twiddle(layer, block));
    if len >= 1 << 15 && rayon::current_num_threads() > 1 {
        rayon::join(
            || half_depth(ntt, a, lanes, layer + 1, block * 2),
            || half_depth(ntt, b, lanes, layer + 1, block * 2 + 1),
        );
    } else {
        serial_tree(ntt, a, lanes, layer + 1, block * 2);
        serial_tree(ntt, b, lanes, layer + 1, block * 2 + 1);
    }
}
fn tiled_ntt(ntt: &AdditiveNttF128, data: &mut [F128], lanes: usize) {
    assert!(lanes.is_power_of_two());
    assert!(data.len().is_power_of_two() && data.len() >= lanes);
    let log = (data.len() / lanes).ilog2() as usize;
    assert!(log <= ntt.log_domain_size());
    let threads = rayon::current_num_threads();
    if threads == 1 || data.len() < 1 << 15 {
        serial_tree(ntt, data, lanes, 0, 0);
        return;
    }
    // Enter the worker pool once for the complete transform. Re-entering it
    // from the caller at each prefix block creates extra injector jobs/pages.
    rayon::join(|| tiled_ntt_worker(ntt, data, lanes, log, threads), || {});
}
fn tiled_ntt_worker(
    ntt: &AdditiveNttF128,
    data: &mut [F128],
    lanes: usize,
    log: usize,
    threads: usize,
) {
    // A fixed breadth prefix exposes parallel rows even in the first layer.
    // Below it, each worker executes an entire subtree without nested jobs.
    let top = (threads.next_power_of_two().ilog2() as usize).min(log.saturating_sub(4));
    for layer in 0..top {
        let len = data.len() >> layer;
        for (block, chunk) in data.chunks_exact_mut(len).enumerate() {
            let (a, b) = chunk.split_at_mut(len / 2);
            let t = ntt.twiddle(layer, block);
            a.par_chunks_mut(2048)
                .zip(b.par_chunks_mut(2048))
                .for_each(|(a, b)| improved_pairs(a, b, t));
        }
    }
    let len = data.len() >> top;
    data.par_chunks_mut(len)
        .enumerate()
        .for_each(|(block, data)| serial_tree(ntt, data, lanes, top, block));
}
fn ntt_cases(samples: usize, rng: &mut Rng) {
    for (log, lanes) in [
        (8, 1),
        (8, 32),
        (12, 8),
        (15, 8),
        (15, 32),
        (16, 32),
        (18, 32),
    ] {
        let size = format!("log{log}_lanes{lanes}");
        if !case_requested("opt_ntt", &size) {
            continue;
        }
        let basis: Vec<_> = (0..log)
            .map(|i| arithmetic::from_u128(1u128 << i))
            .collect();
        let ntt = AdditiveNttF128::new(&basis);
        let input = rng.values((1 << log) * lanes);
        let mut old = input.clone();
        let mut candidate = input.clone();
        let mut tiled = input.clone();
        let mut half_depth_data = input.clone();
        ntt.forward_transform_interleaved(&mut old, lanes);
        subtree(&ntt, &mut candidate, lanes, 0, 0);
        assert_eq!(old, candidate);
        tiled_ntt(&ntt, &mut tiled, lanes);
        assert_eq!(old, tiled);
        half_depth(&ntt, &mut half_depth_data, lanes, 0, 0);
        assert_eq!(old, half_depth_data);
        let mut cases = vec![
            Case::new("production", input.len() * 48, || {
                old.copy_from_slice(black_box(&input));
                ntt.forward_transform_interleaved(black_box(&mut old), lanes);
                black_box(&old);
            }),
            Case::new("depth_first", input.len() * 48, || {
                candidate.copy_from_slice(black_box(&input));
                subtree(black_box(&ntt), black_box(&mut candidate), lanes, 0, 0);
                black_box(&candidate);
            }),
            Case::new("tiled_half", input.len() * 48, || {
                tiled.copy_from_slice(black_box(&input));
                tiled_ntt(black_box(&ntt), black_box(&mut tiled), lanes);
                black_box(&tiled);
            }),
            Case::new("half_depth", input.len() * 48, || {
                half_depth_data.copy_from_slice(black_box(&input));
                half_depth(
                    black_box(&ntt),
                    black_box(&mut half_depth_data),
                    lanes,
                    0,
                    0,
                );
                black_box(&half_depth_data);
            }),
        ];
        measure("opt_ntt", &size, &mut cases, samples, rng);
    }
}

// All 16 output lanes are initialized exactly once, including padding. The
// returned Vec survives OOD evaluation for the subsequent opening consumer.
fn pack(g: &crate::production_packing::Geometry, sources: [&[F128]; 2]) -> Vec<F128> {
    assert_eq!(g.lanes(), 16);
    let n = 1 << g.position_log;
    for b in 0..2 {
        assert_eq!(sources[b].len(), n << g.lane_logs[b]);
        assert!(g.lane_logs[b] <= 3);
    }
    let mut out = Vec::with_capacity(n * 16);
    for pos in 0..n {
        let mut group = [F128::ZERO; 16];
        for b in 0..2 {
            let k = g.lane_logs[b];
            let words = &sources[b][pos << k..(pos + 1) << k];
            group[b * 8..b * 8 + words.len()].copy_from_slice(words);
        }
        out.extend_from_slice(&group);
    }
    out
}

fn pack_tiled(g: &crate::production_packing::Geometry, sources: [&[F128]; 2]) -> Vec<F128> {
    assert_eq!(g.lanes(), 16);
    let n = 1usize << g.position_log;
    for b in 0..2 {
        assert!(g.lane_logs[b] <= 3);
        assert_eq!(sources[b].len(), n << g.lane_logs[b]);
    }
    let len = n.checked_mul(16).expect("packing length");
    let mut out = Vec::<F128>::with_capacity(len);
    let slots = &mut out.spare_capacity_mut()[..len];
    let write = |(pos, group): (usize, &mut [std::mem::MaybeUninit<F128>])| {
        for (b, lane) in group.chunks_exact_mut(8).enumerate() {
            let width = 1 << g.lane_logs[b];
            let (values, padding) = lane.split_at_mut(width);
            for (slot, &value) in values
                .iter_mut()
                .zip(&sources[b][pos * width..(pos + 1) * width])
            {
                slot.write(value);
            }
            for slot in padding {
                slot.write(F128::ZERO);
            }
        }
    };
    if n >= 4096 && rayon::current_num_threads() > 1 {
        slots
            .par_chunks_mut(16)
            .with_min_len(64)
            .enumerate()
            .for_each(write);
    } else {
        slots.chunks_mut(16).enumerate().for_each(write);
    }
    // Every group has 16 slots, each written exactly once above. If a worker
    // panics, len remains zero; F128 is Copy and requires no destructor.
    unsafe {
        out.set_len(len);
    }
    out
}
fn packing_cases(samples: usize, rng: &mut Rng) {
    for logs in [[9, 9], [16, 14], [18, 18]] {
        let size = format!("logs{}_{}", logs[0], logs[1]);
        if !["opt_pack", "opt_packed_ood"]
            .iter()
            .any(|f| case_requested(f, &size))
        {
            continue;
        }
        let position_log = logs[0].max(logs[1]) - 3;
        let g = crate::production_packing::Geometry {
            position_log,
            lane_logs: logs.map(|l| l.max(position_log) - position_log),
            virtual_lane_log: 4,
        };
        let a = rng.values(1 << (position_log + g.lane_logs[0]));
        let b = rng.values(1 << (position_log + g.lane_logs[1]));
        let sources = [a.as_slice(), b.as_slice()];
        let expected = g.virtual_packed(sources);
        assert_eq!(pack(&g, sources), expected);
        assert_eq!(pack_tiled(&g, sources), expected);
        let point: Vec<_> = rng
            .values(g.packed_log())
            .iter()
            .map(|v| Gf::from_words([v.lo, v.hi]))
            .collect();
        assert_eq!(
            ood(&expected, &point),
            crate::production_ood::evaluate(&expected, &point)
        );
        let mut cases = vec![
            Case::new("production", expected.len() * 16, || {
                black_box(g.virtual_packed(black_box(sources)));
            }),
            Case::new("single_write", expected.len() * 16, || {
                black_box(pack(black_box(&g), black_box(sources)));
            }),
            Case::new("tiled_write", expected.len() * 16, || {
                black_box(pack_tiled(black_box(&g), black_box(sources)));
            }),
        ];
        measure("opt_pack", &size, &mut cases, samples, rng);
        let mut cases = vec![
            Case::new("production", expected.len() * 32, || {
                let packed = g.virtual_packed(black_box(sources));
                let y = crate::production_ood::evaluate(&packed, black_box(&point));
                black_box((packed, y));
            }),
            Case::new("single_write_ood", expected.len() * 32, || {
                let packed = pack(black_box(&g), black_box(sources));
                let y = ood(&packed, black_box(&point));
                black_box((packed, y));
            }),
            Case::new("tiled_indexed_ood", expected.len() * 32, || {
                let packed = pack_tiled(black_box(&g), black_box(sources));
                let y = ood_indexed(&packed, black_box(&point));
                black_box((packed, y));
            }),
        ];
        measure("opt_packed_ood", &size, &mut cases, samples, rng);
    }
}

pub(super) fn run(samples: usize, rng: &mut Rng) {
    ood_cases(samples, rng);
    ntt_cases(samples, rng);
    packing_cases(samples, rng);
}
