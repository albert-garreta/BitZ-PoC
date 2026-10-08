use std::{hint::black_box, time::Instant};
#[path = "../../src/piop/spartan/falcon_polynomial.rs"]
mod candidate;
#[path = "ac2_polynomial.rs"]
mod baseline;
fn main() {
    for n in [512, 1024] {
        let mut seed = 0x4641_4c43_4f4e_u64;
        let mut next = || { seed ^= seed << 13; seed ^= seed >> 7; seed ^= seed << 17; seed };
        let cases: Vec<(Vec<u16>, Vec<i16>)> = (0..1024).map(|_| {
            let h = (0..n).map(|_| (next() % 12289) as u16).collect();
            let s2 = (0..n).map(|_| (next() % 32768) as i16 - 16384).collect();
            (h, s2)
        }).collect();
        let mut scratch = candidate::PolynomialWorkspace::karatsuba(n);
        let mut ntt = candidate::PolynomialWorkspace::ntt(n);
        for sample in 0..10 {
            let order = if sample % 2 == 0 { [0, 1, 2] } else { [2, 1, 0] };
            for mode in order {
                let started = Instant::now();
                for (h, s2) in &cases {
                    match mode {
                        0 => {
                            let left: Vec<_> = h.iter().copied().map(i64::from).collect();
                            let right: Vec<_> = s2.iter().copied().map(i64::from).collect();
                            black_box(baseline::integer_polynomial_product(black_box(&left), black_box(&right)));
                        }
                        1 => { black_box(scratch.product(black_box(h), black_box(s2))); }
                        _ => { black_box(ntt.product(black_box(h), black_box(s2))); }
                    }
                }
                if sample > 0 {
                    println!("{{\"degree\":{n},\"batch\":1024,\"sample\":{sample},\"mode\":\"{}\",\"elapsed_ms\":{:.6}}}", ["baseline", "scratch", "ntt"][mode], started.elapsed().as_secs_f64() * 1000.0);
                }
            }
        }
    }
}
