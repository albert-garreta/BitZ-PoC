use field::{IntegerEmbedding, PreparedLinearCombination, Reduce, RingOps, Uint, WideMul, Z};
use std::{hint::black_box, time::Instant};

fn main() {
    let mut modulus = (1u128 << 126) - 1;
    while !field::is_probable_prime_public(&Uint::<2>::from(modulus)) {
        modulus -= 2;
    }
    let f = black_box(field::create_prime_field(Uint::<2>::from(modulus)));
    let r = black_box(f.from_integer(&Uint::from(modulus / 3)));
    let one_minus_r = f.sub(&f.one(), &r);
    let signed =
        <_ as PreparedLinearCombination<Z<1>>>::prepare_linear_combination(&f, [one_minus_r, r]);
    let biased =
        <_ as PreparedLinearCombination<u64>>::prepare_linear_combination(&f, [one_minus_r, r]);
    let difference =
        <_ as PreparedLinearCombination<Z<1>>>::prepare_linear_combination(&f, [f.one(), r]);
    let bias = f.from_integer(&16384u64);
    let biased_acc = <_ as PreparedLinearCombination<u64>>::prepare_linear_combination(
        &f,
        [one_minus_r, r, f.neg(&bias)],
    );
    let mut rng = 0x72846cb238fu64;
    let input: Vec<[i64; 2]> = (0..1 << 20)
        .map(|_| {
            std::array::from_fn(|_| {
                rng ^= rng << 13;
                rng ^= rng >> 7;
                rng ^= rng << 17;
                ((rng as u16 & 0x7fff) as i64) - 16384
            })
        })
        .collect();
    let reference: Vec<_> = input
        .iter()
        .map(|&[a, b]| {
            f.add(
                &f.from_integer(&Z::<1>::from(a)),
                &f.reduce(f.mul_wide(&r, &Z::<1>::from(b - a))),
            )
        })
        .collect();
    let mut output = vec![f.zero(); input.len()];
    for trial in 0..15 {
        for offset in 0..5 {
            let mode = (trial + offset) % 5;
            let start = Instant::now();
            match mode {
                0 => {
                    for (&[a, b], out) in black_box(&input).iter().zip(&mut output) {
                        let magnitude = f.from_integer(&u128::from(a.unsigned_abs()));
                        let embedded = if a < 0 { f.neg(&magnitude) } else { magnitude };
                        *out = f.add(&embedded, &f.reduce(f.mul_wide(&r, &Z::<1>::from(b - a))));
                    }
                }
                1 => {
                    for (&pair, out) in black_box(&input).iter().zip(&mut output) {
                        *out = <field::FpCtx<2> as PreparedLinearCombination<Z<1>>>::linear_combination(
                        &signed, |j| Z::from(pair[j]),
                    );
                    }
                }
                2 => {
                    for (&pair, out) in black_box(&input).iter().zip(&mut output) {
                        *out = f.sub(&<field::FpCtx<2> as PreparedLinearCombination<u64>>::linear_combination(
                        &biased, |j| (pair[j] + 16384) as u64,
                    ), &bias);
                    }
                }
                3 => {
                    for (&[a, b], out) in black_box(&input).iter().zip(&mut output) {
                        *out = <field::FpCtx<2> as PreparedLinearCombination<Z<1>>>::linear_combination(
                        &difference, |j| Z::from([a,b-a][j]),
                    );
                    }
                }
                _ => {
                    for (&pair, out) in black_box(&input).iter().zip(&mut output) {
                        *out =
                            <field::FpCtx<2> as PreparedLinearCombination<u64>>::linear_combination(
                                &biased_acc,
                                |j| if j == 2 { 1 } else { (pair[j] + 16384) as u64 },
                            );
                    }
                }
            }
            black_box(&output);
            let ms = start.elapsed().as_secs_f64() * 1000.0;
            assert_eq!(output, reference);
            println!(
                "{{\"trial\":{trial},\"mode\":{mode},\"pairs\":{},\"ms\":{ms}}}",
                input.len()
            );
        }
    }
}
