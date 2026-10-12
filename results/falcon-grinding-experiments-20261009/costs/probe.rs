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
    pin(31);
    let mut out = BufWriter::new(std::io::stdout().lock());
    writeln!(out, "kernel,threads,bits,sample,worker,cpu,nonce,attempts,elapsed_ns,valid").unwrap();
    for threads in [1,2,4,8,16] {
        let pool = rayon::ThreadPoolBuilder::new().num_threads(threads)
            .start_handler(|i| pin(WORKERS[i])).build().unwrap();
        // Pool installation is outside the timer, as it is in Falcon proving.
        let records = pool.install(|| {
            let mut records = Vec::new();
            for kernel in ["spartan", "pcs"] {
                for bits in 0..=24 {
                    if bits == 0 && kernel == "spartan" { continue; }
                    let count = if bits <= 18 { 64 } else { 32 };
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
                            rayon::current_thread_index().unwrap(), unsafe {libc::sched_getcpu()}, nonce+1));
                    }
                    eprintln!("calibrated {kernel}, threads={threads}, bits={bits}, samples={count}");
                }
            }
            records
        });
        for record in records { writeln!(out, "{record}").unwrap(); }
        out.flush().unwrap();
    }
}
