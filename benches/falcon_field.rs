//! Public Falcon field power-generation comparisons.
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
    power_basis(alpha, &starts);
    power_pair(alpha, &starts);
}

fn power_basis(alpha: E, starts: &[E]) {
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
                "{{\"case\":\"power_basis\",\"degree\":{n},\"batch\":1024,\"trial\":{trial},\"fixed_ms\":{fixed_ms},\"power_ms\":{power_ms},\"setup_us\":{setup_us}}}"
            );
        }
    }
}

fn power_pair(alpha: E, starts: &[E]) {
    for n in [512, 1024] {
        let mut individual = vec![PowerCoordinates::ZERO; starts.len() * n];
        let mut paired = individual.clone();
        for trial in 0..12 {
            let mut times = [0.0; 2];
            // Alternate order; both measurements include basis preparation,
            // start conversion, feedback preparation, and all output stores.
            for mode in [trial % 2, 1 - trial % 2] {
                let timer = Instant::now();
                let basis = PowerBasis::try_new(black_box(alpha)).unwrap();
                if mode == 0 {
                    for (&seed, output) in starts.iter().zip(individual.chunks_mut(n)) {
                        basis.fill_powers(basis.encode(seed), output);
                    }
                    black_box(&individual);
                } else {
                    for (seeds, output) in starts.chunks_exact(2).zip(paired.chunks_mut(2 * n)) {
                        let (a, b) = output.split_at_mut(n);
                        basis.fill_powers_pair(
                            [basis.encode(seeds[0]), basis.encode(seeds[1])],
                            [a, b],
                        );
                    }
                    black_box(&paired);
                }
                times[mode] = timer.elapsed().as_secs_f64() * 1000.0;
            }
            assert_eq!(individual, paired);
            println!(
                "{{\"case\":\"power_pair\",\"degree\":{n},\"batch\":1024,\"trial\":\"{}\",\"iteration\":{trial},\"individual_ms\":{},\"paired_ms\":{}}}",
                if trial < 2 { "warmup" } else { "sample" },
                times[0],
                times[1]
            );
        }
    }
}
