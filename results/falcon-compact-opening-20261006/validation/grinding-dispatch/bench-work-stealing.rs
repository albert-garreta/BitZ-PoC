use std::hint::black_box;
use std::io::{BufWriter, Write};
use std::time::Instant;
mod blake3x4;
use blake3x4 as grind;

fn affinity() -> Result<Vec<usize>, String> {
    let mut set: libc::cpu_set_t = unsafe { std::mem::zeroed() };
    if unsafe { libc::sched_getaffinity(0, std::mem::size_of_val(&set), &mut set) } != 0 {
        return Err(std::io::Error::last_os_error().to_string());
    }
    Ok((0..libc::CPU_SETSIZE as usize).filter(|&c| unsafe { libc::CPU_ISSET(c, &set) }).collect())
}
fn pin(cpu: usize) -> Result<Vec<usize>, String> {
    let mut set: libc::cpu_set_t = unsafe { std::mem::zeroed() };
    unsafe { libc::CPU_SET(cpu, &mut set); }
    if unsafe { libc::sched_setaffinity(0, std::mem::size_of_val(&set), &set) } != 0 {
        return Err(std::io::Error::last_os_error().to_string());
    }
    let actual = affinity()?;
    if actual != vec![cpu] { return Err(format!("requested {cpu}; actual {actual:?}")); }
    Ok(actual)
}
fn serial(prefix: &[u8], bits: u32) -> u64 {
    grind::first_pow_nonce(prefix, 0, u64::MAX, bits)
        .or_else(|| grind::pow_ok(prefix, u64::MAX, bits).then_some(u64::MAX)).unwrap()
}
fn parallel(prefix: &[u8], bits: u32) -> u64 {
    grind::smallest_pow_nonce(prefix, bits).unwrap()
}
fn stealing_with<const CHUNK_LOG: u32>(prefix: &[u8], bits: u32) -> Option<u64> {
    use rayon::prelude::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    assert!((1..64).contains(&CHUNK_LOG));
    let next = AtomicU64::new(0);
    let best = AtomicU64::new(u64::MAX);
    (0..rayon::current_num_threads()).into_par_iter().for_each(|_| {
        loop {
            let chunk = next.fetch_add(1, Ordering::Relaxed);
            if chunk >= 1 << (64 - CHUNK_LOG) { break; }
            let start = chunk << CHUNK_LOG;
            if start >= best.load(Ordering::Relaxed) { break; }
            if let Some(n) = grind::first_pow_nonce(prefix, start, start.saturating_add(1 << CHUNK_LOG), bits) {
                best.fetch_min(n, Ordering::Relaxed);
            }
        }
    });
    let found = best.load(Ordering::Relaxed);
    if found == u64::MAX && !grind::pow_ok(prefix, u64::MAX, bits) { return None; }
    Some(found)
}
fn stealing(prefix: &[u8], bits: u32) -> u64 {
    if bits <= 16 && prefix.len() % 4 == 0 && std::is_x86_feature_detected!("avx512f") {
        stealing_with::<8>(prefix, bits).unwrap()
    } else { stealing_with::<10>(prefix, bits).unwrap() }
}
fn prefix(len: usize, seed: usize) -> Vec<u8> {
    let mut h=blake3::Hasher::new();
    h.update(b"BitZ-grinding-dispatch-independent-20261006-v1");
    h.update(&(len as u64).to_le_bytes()); h.update(&(seed as u64).to_le_bytes());
    h.finalize().as_bytes()[..len].to_vec()
}
fn scalar_oracle(prefix: &[u8]) -> [u64; 9] {
    let mut found = [u64::MAX; 9];
    let mut buf = [0u8; 64]; buf[..prefix.len()].copy_from_slice(prefix);
    for nonce in 0..u64::MAX {
        buf[prefix.len()..prefix.len()+8].copy_from_slice(&nonce.to_le_bytes());
        let h = blake3::hash(&buf[..prefix.len()+8]);
        let zeros = grind::leading_zero_bits(h.as_bytes());
        for bits in 12..=zeros.min(20) {
            let slot=&mut found[(bits-12) as usize]; if *slot==u64::MAX { *slot=nonce; }
        }
        if found[8]!=u64::MAX { return found; }
    }
    panic!("reference exhausted nonce range")
}
fn main() {
    assert!(std::is_x86_feature_detected!("avx512f"), "AVX512 measurement required");
    let allowed=affinity().unwrap();
    assert!((0..16).all(|c|allowed.contains(&c)), "CPU0..15 must be allowed");
    rayon::ThreadPoolBuilder::new().num_threads(16).build_global().unwrap();
    let workers=rayon::broadcast(|cx| (cx.index(),pin(cx.index()).unwrap()));
    let main=pin(0).unwrap();
    assert_eq!(rayon::current_num_threads(),16);
    eprintln!("workers={workers:?} main={main:?} AVX512F=true");
    let outdir=std::env::args().nth(1).expect("output directory");
    let mut raw=BufWriter::new(std::fs::File::create(format!("{outdir}/raw.csv")).unwrap());
    writeln!(raw,"prefix_len,bits,seed,rep,position,method,elapsed_ns,nonce").unwrap();
    let mut cases=BufWriter::new(std::fs::File::create(format!("{outdir}/cases.csv")).unwrap());
    writeln!(cases,"prefix_len,bits,seed,prefix_hex,nonce").unwrap();
    for len in [32usize] {
        let prefixes:Vec<_>=(0..64).map(|seed|prefix(len,seed)).collect();
        let mut expected=Vec::new();
        for (seed,p) in prefixes.iter().enumerate() {
            expected.push(scalar_oracle(p));
            if seed%8==7 { eprintln!("scalar reference len={len} seeds={}",seed+1); }
        }
        for bits in 12..=20u32 {
            for seed in 0..64 {
                let p=&prefixes[seed]; let exp=expected[seed][(bits-12)as usize];
                assert_eq!(serial(p,bits),exp); assert_eq!(parallel(p,bits),exp); assert_eq!(stealing(p,bits),exp);
                let hex:String=p.iter().map(|b|format!("{b:02x}")).collect();
                writeln!(cases,"{len},{bits},{seed},{hex},{exp}").unwrap();
            }
            for rep in 0..4usize {
                for order_index in 0..64 {
                    let seed=(order_index*37+rep*19+bits as usize*7)%64;
                    let p=&prefixes[seed]; let exp=expected[seed][(bits-12)as usize];
                    let order = if (seed+rep)%2==0 { [false,true,true,false] } else { [true,false,false,true] };
                    for (position,is_parallel) in order.into_iter().enumerate() {
                        let start=Instant::now();
                        let nonce=if is_parallel { stealing(black_box(p),black_box(bits)) } else { parallel(black_box(p),black_box(bits)) };
                        black_box(nonce); let elapsed=start.elapsed().as_nanos();
                        assert_eq!(nonce,exp,"nonce mismatch len={len} bits={bits} seed={seed}");
                        writeln!(raw,"{len},{bits},{seed},{rep},{position},{},{elapsed},{nonce}",if is_parallel {"stealing"} else {"broadcast"}).unwrap();
                    }
                }
            }
            raw.flush().unwrap(); cases.flush().unwrap();
            eprintln!("measured len={len} bits={bits} cases=64 reps=4 ABBA/BAAB");
        }
    }
    eprintln!("all 576 cases match independent scalar oracle; measured 9216 calls");
}
