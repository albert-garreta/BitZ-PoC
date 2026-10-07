//! Compare canonical weight generation using general multiplication and a
//! prepared power basis. Run in release mode; this is a public-data kernel.
use field::q12289::{PowerBasis, PowerCoordinates, Q12289Extension};
use std::{hint::black_box, time::Instant};

type E = Q12289Extension<11>;

fn main() {
    let alpha = black_box(E::new(std::array::from_fn(|i| (i * 931 + 219) as u16)));
    let starts: Vec<_> = (0..2048)
        .map(|i| {
            E::new(std::array::from_fn(|k| {
                ((i * 127 + k * 599) % 12289) as u16
            }))
        })
        .collect();
    for n in [512, 1024] {
        let mut fixed = vec![E::ZERO; starts.len() * n];
        let mut power = vec![PowerCoordinates::ZERO; starts.len() * n];
        for trial in 0..6 {
            let start = Instant::now();
            for (&seed, output) in starts.iter().zip(fixed.chunks_mut(n)) {
                let mut value = seed;
                for slot in output {
                    *slot = value;
                    value = value.mul(alpha);
                }
            }
            black_box(&fixed);
            let fixed_ms = start.elapsed().as_secs_f64() * 1000.0;
            let start = Instant::now();
            let basis = PowerBasis::try_new(black_box(alpha)).unwrap();
            let setup_us = start.elapsed().as_secs_f64() * 1e6;
            for (&seed, output) in starts.iter().zip(power.chunks_mut(n)) {
                basis.fill_powers(basis.encode(seed), output);
            }
            black_box(&power);
            let power_ms = start.elapsed().as_secs_f64() * 1000.0;
            // Equality is checked outside the measured section.
            for j in [0, n / 2, n - 1, power.len() - 1] {
                assert_eq!(basis.decode(power[j]), fixed[j]);
            }
            println!(
                "{{\"degree\":{n},\"batch\":1024,\"trial\":{trial},\"fixed_ms\":{fixed_ms},\"power_ms\":{power_ms},\"setup_us\":{setup_us}}}"
            );
        }
    }
}
