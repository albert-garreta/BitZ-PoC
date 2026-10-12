use bitz::ligerito_flock::ZincChallenger;
use bitz::piop::spartan::grinding::{find_grinding_nonce, GrindingSeed};
use bitz::poly::univariate::binary_gf128::Gf128;
use bitz::transcript::{Blake3Transcript, traits::Transcript};
use flock_core::challenger::Challenger;
use std::io::{BufWriter, Write};
use std::time::Instant;

const WORKERS: [usize; 16] = [6,7,4,5,2,3,0,1,14,15,12,13,10,11,8,9];

fn pin(cpu: usize) {
    unsafe {
        let mut set: libc::cpu_set_t = std::mem::zeroed();
        libc::CPU_ZERO(&mut set);
        libc::CPU_SET(cpu, &mut set);
        assert_eq!(libc::sched_setaffinity(0, std::mem::size_of_val(&set), &set), 0);
    }
}

fn seed(bits: u32, sample: i32) -> [u8;32] {
    *blake3::hash(format!("bitz/grinding-cost-calibration/20261009/{bits}/{sample}").as_bytes()).as_bytes()
}

fn valid(prefix: &[u8], nonce: u64, bits: u32) -> bool {
    let mut h = blake3::Hasher::new();
    h.update(prefix);
    h.update(&nonce.to_le_bytes());
    let mut remaining = bits;
    for byte in h.finalize().as_bytes() {
        if remaining == 0 { return true }
        let z = byte.leading_zeros();
        if z >= remaining { return true }
        if z < 8 { return false }
        remaining -= 8;
    }
    remaining == 0
}

fn main() {
    let threads: usize = std::env::args().nth(1).expect("thread count").parse().unwrap();
    assert!([1,2,4,8,16].contains(&threads));
    let sample_scale: i32 = std::env::args().nth(2).map_or(1, |s|s.parse().unwrap());
    assert!((1..=16).contains(&sample_scale));
    let mut out = BufWriter::new(std::io::stdout().lock());
    writeln!(out, "kernel,threads,bits,sample,worker,cpu,nonce,attempts,elapsed_ns,valid").unwrap();
    {
        let (ready, receiver) = std::sync::mpsc::channel();
        rayon::ThreadPoolBuilder::new().num_threads(threads)
            .start_handler(move |i| {pin(WORKERS[i]); ready.send(i).unwrap();})
            .build_global().unwrap();
        for _ in 0..threads { receiver.recv().unwrap(); }
        pin(6);
        assert!(rayon::current_thread_index().is_none());
        // Falcon benchmark proving executes on the pinned external main thread.
        // Warm clocks/pool before any measured cells, outside every timer.
        let warm = Instant::now();
        let mut precondition = 0;
        while warm.elapsed() < std::time::Duration::from_millis(250) {
            let input = GrindingSeed::from_bytes(seed(20, -1000-precondition));
            std::hint::black_box(find_grinding_nonce(&input, 20).unwrap());
            precondition += 1;
        }
        eprintln!("preconditioning searches={precondition}, elapsed_ns={}", warm.elapsed().as_nanos());
        let records = {
            let mut records = Vec::new();
            for kernel in ["spartan", "pcs"] {
                for bits in 0..=24 {
                    if bits == 0 && kernel == "spartan" { continue; }
                    let count = sample_scale * if bits <= 18 { 128 } else { 64 };
                    for sample in -3..count {
                        let input = seed(bits, sample);
                        let (nonce, ns, ok) = if kernel == "spartan" {
                            let s = GrindingSeed::from_bytes(input);
                            let start = Instant::now();
                            let n = std::hint::black_box(find_grinding_nonce(&s, bits).unwrap());
                            let ns = start.elapsed().as_nanos();
                            (n, ns, valid(&input, n, bits))
                        } else {
                            let mut t = Blake3Transcript::new();
                            t.absorb_slice(&input);
                            let mut check = t.clone();
                            let g = check.get_field_challenge::<Gf128>(&());
                            let w = g.as_words();
                            let mut prefix = [0u8;16];
                            prefix[..8].copy_from_slice(&w[0].to_le_bytes());
                            prefix[8..].copy_from_slice(&w[1].to_le_bytes());
                            let start = Instant::now();
                            let n = std::hint::black_box(ZincChallenger(&mut t).grind_pow(bits));
                            let ns = start.elapsed().as_nanos();
                            assert!(ZincChallenger(&mut {
                                let mut verifier = Blake3Transcript::new();
                                verifier.absorb_slice(&input);
                                verifier
                            }).verify_pow(n, bits));
                            (n, ns, valid(&prefix, n, bits))
                        };
                        assert!(ok);
                        records.push(format!("{kernel},{threads},{bits},{sample},{},{},{nonce},{},{ns},{ok}",
                            rayon::current_thread_index().map_or(-1, |i| i as i32), unsafe {libc::sched_getcpu()}, nonce+1));
                    }
                    eprintln!("calibrated {kernel}, threads={threads}, bits={bits}, samples={count}");
                }
            }
            records
        };
        assert!(rayon::current_thread_index().is_none());
        for record in records { writeln!(out, "{record}").unwrap(); }
        out.flush().unwrap();
    }
}
