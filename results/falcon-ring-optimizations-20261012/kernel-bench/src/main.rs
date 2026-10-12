use field::{
    FpLinearAcc, IntegerEmbedding, PreparedLinearCombination, Reduce, RingOps, Uint,
    create_prime_field,
};
use std::{hint::black_box, time::Instant};
fn collapse() {
    let mut prime = (1u128 << 126) - 1;
    while !field::is_probable_prime_public(&Uint::<2>::from(prime)) {
        prime -= 2;
    }
    let field = black_box(create_prime_field(Uint::<2>::from(prime)));
    let xi = field.from_integer(&Uint::from(123456789012345678901234567890u128));
    let mut powers = [field.one(); 11];
    for k in 1..11 {
        powers[k] = field.mul(&powers[k - 1], &xi);
    }
    let inputs: Vec<[u16; 11]> = (0..2 * 1024 * 1024)
        .map(|i| std::array::from_fn(|k| ((i * 127 + k * 931) % 12289) as u16))
        .collect();
    let mut expected = vec![field.zero(); inputs.len()];
    let mut actual = expected.clone();
    for trial in 0..12 {
        let mut times = [0.0; 2];
        for mode in [trial % 2, 1 - trial % 2] {
            let start = Instant::now();
            if mode == 0 {
                for (v, out) in inputs.iter().zip(&mut expected) {
                    let mut acc = FpLinearAcc::<2, 1>::default();
                    for (power, &c) in powers.iter().zip(v) {
                        acc.accumulate(power, &Uint::from_words([u64::from(c)]));
                    }
                    *out = field.reduce(acc);
                }
                black_box(&expected);
            } else {
                let prepared = <_ as PreparedLinearCombination<u64>>::prepare_linear_combination(
                    &field,
                    black_box(powers),
                );
                for (v, out) in inputs.iter().zip(&mut actual) {
                    *out = <field::FpCtx<2> as PreparedLinearCombination<u64>>::linear_combination(
                        &prepared,
                        |k| u64::from(v[k]),
                    );
                }
                black_box(&actual);
            }
            times[mode] = start.elapsed().as_secs_f64() * 1000.0;
        }
        assert_eq!(actual, expected);
        println!(
            "{{\"trial\":{trial},\"barrett_ms\":{},\"prepared_ms\":{}}}",
            times[0], times[1]
        );
    }
}

use field::q12289::{PowerBasis, PowerCoordinates, Q12289Extension};
fn interleaved<const B: usize>(
    basis: &PowerBasis<11>,
    n: usize,
    starts: &[[PowerCoordinates<11>; 2]],
    subtract: &[PowerCoordinates<11>],
    polynomials: &[Vec<u16>],
    a: &mut [PowerCoordinates<11>],
    b: &mut [PowerCoordinates<11>],
) {
    for (group, (a, b)) in a.chunks_mut(B * n).zip(b.chunks_mut(B * n)).enumerate() {
        let mut ac = a.chunks_mut(n);
        let mut bc = b.chunks_mut(n);
        basis.fill_powers_adjoint_pairs::<B>(
            std::array::from_fn(|i| starts[group * B + i]),
            std::array::from_fn(|i| subtract[group * B + i]),
            std::array::from_fn(|i| polynomials[group * B + i].as_slice()),
            std::array::from_fn(|_| [ac.next().unwrap(), bc.next().unwrap()]),
        );
    }
}
fn adjoint() {
    type E = Q12289Extension<11>;
    let basis =
        PowerBasis::try_new(E::new(std::array::from_fn(|i| (i * 931 + 219) as u16))).unwrap();
    let starts: Vec<_> = (0..1024)
        .map(|i| {
            std::array::from_fn(|j| {
                basis.encode(E::new(std::array::from_fn(|k| {
                    ((i * 127 + j * 251 + k * 599) % 12289) as u16
                })))
            })
        })
        .collect();
    let subtract: Vec<_> = starts.iter().map(|s| s[1]).collect();
    for n in [512, 1024] {
        let polynomials: Vec<Vec<u16>> = (0..1024)
            .map(|i| {
                (0..n)
                    .map(|j| ((i * 137 + j * 599) % 12289) as u16)
                    .collect()
            })
            .collect();
        let mut outputs: [[Vec<PowerCoordinates<11>>; 2]; 3] = std::array::from_fn(|_| {
            std::array::from_fn(|_| vec![PowerCoordinates::ZERO; 1024 * n])
        });
        for trial in 0..12 {
            let mut times = [0.0; 3];
            for mode in [trial % 3, (trial + 1) % 3, (trial + 2) % 3] {
                let [a, b] = &mut outputs[mode];
                let start = Instant::now();
                match mode {
                    0 => {
                        for (i, (a, b)) in a.chunks_mut(n).zip(b.chunks_mut(n)).enumerate() {
                            basis.fill_powers_adjoint_pair(
                                starts[i],
                                subtract[i],
                                &polynomials[i],
                                [a, b],
                            );
                        }
                    }
                    1 => interleaved::<2>(&basis, n, &starts, &subtract, &polynomials, a, b),
                    _ => interleaved::<4>(&basis, n, &starts, &subtract, &polynomials, a, b),
                }
                black_box((a, b));
                times[mode] = start.elapsed().as_secs_f64() * 1000.0;
            }
            assert_eq!(outputs[0], outputs[1]);
            assert_eq!(outputs[0], outputs[2]);
            println!(
                "{{\"degree\":{n},\"trial\":{trial},\"single_ms\":{},\"two_ms\":{},\"four_ms\":{}}}",
                times[0], times[1], times[2]
            );
        }
    }
}

#[path = "../../../../src/piop/spartan/falcon_polynomial.rs"]
mod polynomial;
fn ntt() {
    for n in [512, 1024] {
        let h: Vec<Vec<u16>> = (0..1024)
            .map(|i| {
                (0..n)
                    .map(|j| ((i * 137 + j * 599) % 12289) as u16)
                    .collect()
            })
            .collect();
        let s: Vec<Vec<i16>> = (0..1024)
            .map(|i| {
                (0..n)
                    .map(|j| ((i * 157 + j * 197) % 32768) as i16 - 16384)
                    .collect()
            })
            .collect();
        let mut ordinary = polynomial::PolynomialWorkspace::new(n);
        let mut negacyclic = polynomial::NegacyclicWorkspace::new(n);
        for (h, s) in h.iter().zip(&s) {
            let a = ordinary.product(h, s);
            let b = negacyclic.product(h, s);
            for j in 0..n {
                assert_eq!(b[j], (a[j] - a[n + j]).rem_euclid(12289));
            }
        }
        for trial in 0..12 {
            let mut times = [0.0; 2];
            for mode in [trial % 2, 1 - trial % 2] {
                let start = Instant::now();
                if mode == 0 {
                    for (h, s) in h.iter().zip(&s) {
                        black_box(ordinary.product(black_box(h), black_box(s)));
                    }
                } else {
                    for (h, s) in h.iter().zip(&s) {
                        black_box(negacyclic.product(black_box(h), black_box(s)));
                    }
                }
                times[mode] = start.elapsed().as_secs_f64() * 1000.0;
            }
            println!(
                "{{\"degree\":{n},\"trial\":{trial},\"ordinary_ms\":{},\"negacyclic_ms\":{}}}",
                times[0], times[1]
            );
        }
    }
}
fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("adjoint") => adjoint(),
        Some("ntt") => ntt(),
        _ => collapse(),
    }
}
