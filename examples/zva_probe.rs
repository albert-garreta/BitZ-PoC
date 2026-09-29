//! Does `dc zva` skip the read-for-ownership of writes into lines that are
//! not in cache? The probe for the forest's fresh-output passes (the JIT
//! fold, the level-4 rebuild, the upper levels), which write a 256 MiB
//! arena they do not read.
//!
//! `zva_probe [--mib M] [--threads T,T,...] [--passes P]` pre-faults an
//! `M` MiB buffer (default 256, larger than every cache) and times, per
//! thread count, `P` passes (median) of each writer over it, every thread
//! a contiguous share:
//! - `store`: four 16-byte stores per 64-byte block;
//! - `zva+store`: `dc zva` on the block (size from `DCZID_EL0`), then the
//!   same four stores;
//! - `stnp`: two non-temporal pair stores per block;
//! - `read` and `rmw` (load, XOR, store in place) as the references a
//!   write pass without and with its own read would sit between.
//!
//! aarch64 only. On the M5 (2026-09-28, idle-gated, 256 MiB, median of
//! 15 passes; `DCZID_EL0` block 64 B), GB/s at 1 / 2 / 4 / 10 threads:
//!
//! | writer    |    1 |    2 |     4 |    10 |
//! |-----------|------|------|-------|-------|
//! | store     | 72.5 | 87.2 |  87.7 | 108.9 |
//! | zva+store | 10.9 | 21.5 |  41.4 |  74.5 |
//! | stnp      | 83.4 | 58.2 | 108.5 | 142.6 |
//! | read      | 70.8 | 83.3 |  99.5 | 135.2 |
//! | rmw       | 57.5 | 68.1 |  65.2 |  71.3 |
//!
//! Plain stores into lines not in cache already run at 0.8x the read
//! bandwidth (no full read-for-ownership), and `dc zva` makes them slower
//! at every thread count (0.68x at 10 threads), so the forest's fresh-write
//! passes keep plain stores. Non-temporal pair stores are 1.31x at 10
//! threads, but need whole contiguous 64-byte blocks per store pair.

#[cfg(target_arch = "aarch64")]
fn main() {
    probe::main();
}

#[cfg(not(target_arch = "aarch64"))]
fn main() {
    eprintln!("zva_probe: aarch64 only");
}

#[cfg(target_arch = "aarch64")]
mod probe {
    use core::arch::aarch64::{uint64x2_t, vdupq_n_u64, veorq_u64, vld1q_u64, vst1q_u64};
    use std::alloc::{Layout, alloc, dealloc};
    use std::sync::Barrier;
    use std::time::{Duration, Instant};

    /// `(log2 of the block size in bytes, zeroing prohibited)`.
    fn dczid() -> (u32, bool) {
        let v: u64;
        // SAFETY: DCZID_EL0 is readable at EL0 on every AArch64 core.
        unsafe { core::arch::asm!("mrs {0}, dczid_el0", out(reg) v) };
        ((v & 0xF) as u32 + 2, v & 0x10 != 0)
    }

    #[derive(Clone, Copy, Debug)]
    enum Writer {
        Store,
        ZvaStore,
        Stnp,
        Read,
        Rmw,
    }

    /// One pass of `w` over `[p, p + bytes)` (64-byte aligned, a multiple of
    /// 64 bytes); returns a value that depends on what was read.
    ///
    /// # Safety
    /// The range must be valid for reads and writes and 64-byte aligned.
    #[inline(never)]
    unsafe fn pass(w: Writer, p: *mut u8, bytes: usize, seed: u64) -> u64 {
        // SAFETY: the caller's contract; NEON is baseline on aarch64.
        unsafe {
            let mut v: uint64x2_t = vdupq_n_u64(seed);
            let inc = vdupq_n_u64(0x9E37_79B9_7F4A_7C15);
            let mut acc = vdupq_n_u64(0);
            let mut off = 0usize;
            while off < bytes {
                let b = p.add(off);
                let q = b.cast::<u64>();
                match w {
                    Writer::Store => {
                        vst1q_u64(q, v);
                        vst1q_u64(q.add(2), v);
                        vst1q_u64(q.add(4), v);
                        vst1q_u64(q.add(6), v);
                    }
                    Writer::ZvaStore => {
                        core::arch::asm!("dc zva, {0}", in(reg) b, options(nostack, preserves_flags));
                        vst1q_u64(q, v);
                        vst1q_u64(q.add(2), v);
                        vst1q_u64(q.add(4), v);
                        vst1q_u64(q.add(6), v);
                    }
                    Writer::Stnp => {
                        core::arch::asm!(
                            "stnp {a:q}, {b:q}, [{p}]",
                            "stnp {a:q}, {b:q}, [{p}, #32]",
                            p = in(reg) b,
                            a = in(vreg) v,
                            b = in(vreg) v,
                            options(nostack, preserves_flags),
                        );
                    }
                    Writer::Read => {
                        acc = veorq_u64(acc, vld1q_u64(q));
                        acc = veorq_u64(acc, vld1q_u64(q.add(2)));
                        acc = veorq_u64(acc, vld1q_u64(q.add(4)));
                        acc = veorq_u64(acc, vld1q_u64(q.add(6)));
                    }
                    Writer::Rmw => {
                        for k in 0..4 {
                            let x = vld1q_u64(q.add(2 * k));
                            vst1q_u64(q.add(2 * k), veorq_u64(x, v));
                        }
                    }
                }
                v = veorq_u64(v, inc);
                off += 64;
            }
            let mut out = [0u64; 2];
            vst1q_u64(out.as_mut_ptr(), acc);
            out[0] ^ out[1]
        }
    }

    struct Buf(*mut u8);
    // SAFETY: threads touch disjoint ranges of the buffer.
    unsafe impl Send for Buf {}
    unsafe impl Sync for Buf {}

    fn median(v: &mut [Duration]) -> Duration {
        v.sort();
        v[v.len() / 2]
    }

    pub(super) fn main() {
        let args: Vec<String> = std::env::args().collect();
        let mut mib = 256usize;
        let mut threads: Vec<usize> = vec![1, 10];
        let mut passes = 15usize;
        let mut i = 1;
        while i < args.len() {
            match args[i].as_str() {
                "--mib" => mib = args[i + 1].parse().expect("--mib"),
                "--threads" => threads = args[i + 1].split(',').map(|t| t.parse().expect("--threads")).collect(),
                "--passes" => passes = args[i + 1].parse().expect("--passes"),
                other => panic!("unknown argument {other}"),
            }
            i += 2;
        }
        let (log_block, prohibited) = dczid();
        println!("zva_probe mib={mib} passes={passes} dczid_block={} zva_prohibited={prohibited}", 1u32 << log_block);
        assert!(!prohibited && log_block == 6, "the probe writes 64-byte blocks");
        let bytes = mib << 20;
        let layout = Layout::from_size_align(bytes, 1 << 14).expect("layout");
        // SAFETY: non-zero size; freed below.
        let base = unsafe { alloc(layout) };
        assert!(!base.is_null());
        // Pre-fault every page so the passes time the stores, not the faults.
        // SAFETY: the whole allocation.
        unsafe { std::ptr::write_bytes(base, 1, bytes) };
        let buf = Buf(base);
        let mut sink = 0u64;
        for &t in &threads {
            let share = (bytes / t) & !63;
            for w in [Writer::Store, Writer::ZvaStore, Writer::Stnp, Writer::Read, Writer::Rmw] {
                let barrier = Barrier::new(t + 1);
                let mut times = Vec::with_capacity(passes);
                std::thread::scope(|scope| {
                    for k in 0..t {
                        let (barrier, buf) = (&barrier, &buf);
                        scope.spawn(move || {
                            let mut local = 0u64;
                            for round in 0..passes + 1 {
                                barrier.wait();
                                // SAFETY: thread `k` owns `[k·share, (k+1)·share)`.
                                local ^= unsafe { pass(w, buf.0.add(k * share), share, (round * 131 + k) as u64) };
                                barrier.wait();
                            }
                            std::hint::black_box(local);
                        });
                    }
                    for round in 0..passes + 1 {
                        barrier.wait();
                        let started = Instant::now();
                        barrier.wait();
                        // The first pass warms the threads up.
                        if round > 0 {
                            times.push(started.elapsed());
                        }
                    }
                });
                let m = median(&mut times);
                let gbs = (share * t) as f64 / m.as_secs_f64() / 1e9;
                println!("threads={t:>2} {:<10} median {:>7.3} ms  {gbs:>6.1} GB/s", format!("{w:?}"), m.as_secs_f64() * 1e3);
                // SAFETY: in bounds.
                sink ^= unsafe { *buf.0.add(bytes - 1) } as u64;
            }
        }
        std::hint::black_box(sink);
        // SAFETY: allocated above with this layout.
        unsafe { dealloc(base, layout) };
    }
}
