//! Their `poly::eq` over [`Fq`]: the little-endian equality table and the
//! multiplicative `eq_eval`. Entry `j` of the table is `eq(b, r)` for
//! `b_i = (j >> i) & 1`, so variable `i` is index bit `i`.

use super::super::fq::Fq;
#[cfg(feature = "parallel")]
use rayon::prelude::*;

/// Below this many entries a doubling level runs sequentially.
const PARALLEL_HALF: usize = 1 << 13;

/// `eq(b, r)` over every `b ∈ {0,1}^n`, little-endian (their `eq_table`).
pub fn eq_table(r: &[Fq]) -> Vec<Fq> {
    let n = 1usize << r.len();
    let mut table = vec![Fq::ZERO; n];
    table[0] = Fq::ONE;
    for (i, &r_i) in r.iter().enumerate() {
        let half = 1usize << i;
        let (zero_children, one_children) = table[..2 * half].split_at_mut(half);
        let update = |(zero_child, one_child): (&mut Fq, &mut Fq)| {
            let parent = *zero_child;
            let one_value = parent * r_i;
            *zero_child = parent - one_value;
            *one_child = one_value;
        };
        #[cfg(feature = "parallel")]
        if half >= PARALLEL_HALF {
            zero_children
                .par_iter_mut()
                .zip(one_children.par_iter_mut())
                .for_each(update);
            continue;
        }
        zero_children.iter_mut().zip(one_children).for_each(update);
    }
    table
}

/// `∏_i [x_i y_i + (1 − x_i)(1 − y_i)]`, their `eq_eval` (as
/// `(1 − x_i) + y_i (2 x_i − 1)` per coordinate).
pub fn eq_eval(left: &[Fq], right: &[Fq]) -> Fq {
    assert_eq!(left.len(), right.len(), "equality points must have the same length");
    let one = Fq::ONE;
    let coordinate = |l: Fq, r: Fq| (one - l) + r * (l + l - one);
    let mut coordinates = left.iter().copied().zip(right.iter().copied());
    let Some((l, r)) = coordinates.next() else {
        return one;
    };
    coordinates.fold(coordinate(l, r), |value, (l, r)| value * coordinate(l, r))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_is_little_endian_and_matches_eq_eval() {
        let r = [Fq::new(3), Fq::new(5), Fq::new(7)];
        let table = eq_table(&r);
        assert_eq!(table.len(), 8);
        for (j, &entry) in table.iter().enumerate() {
            let b: Vec<Fq> = (0..3).map(|i| Fq::from((j >> i) & 1 == 1)).collect();
            assert_eq!(entry, eq_eval(&b, &r), "entry {j}");
        }
        assert_eq!(eq_table(&[]), vec![Fq::ONE]);
        assert_eq!(eq_eval(&[], &[]), Fq::ONE);
    }

    #[test]
    fn large_tables_agree_with_the_sequential_definition() {
        let r: Vec<Fq> = (0..15).map(|i| Fq::new(1_000_003 * (i as u128 + 1))).collect();
        let table = eq_table(&r);
        for j in [0usize, 1, 12345, (1 << 15) - 1] {
            let b: Vec<Fq> = (0..15).map(|i| Fq::from((j >> i) & 1 == 1)).collect();
            assert_eq!(table[j], eq_eval(&b, &r));
        }
    }
}
