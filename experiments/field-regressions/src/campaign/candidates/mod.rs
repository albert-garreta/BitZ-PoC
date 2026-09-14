//! Isolated candidate kernels. Production callers and dispatch are unchanged.
use crate::Case;
use std::{fmt::Debug, hint::black_box};
mod binary;
mod composite;
mod integer;
mod prime;
mod words;

pub(crate) use composite::tiled_ntt;

/// An owned output per variant, checked before timing and observed after each pass.
fn pass<'a, I: ?Sized, O: Clone + Default + Debug + PartialEq + 'a>(
    name: &'static str,
    bytes: usize,
    input: &'a I,
    expected: &[O],
    kernel: impl Fn(&I, &mut [O]) + 'a,
) -> Case<'a> {
    let mut output = vec![O::default(); expected.len()];
    kernel(input, &mut output);
    assert_eq!(output, expected, "{name}");
    Case::new(name, bytes, move || {
        kernel(black_box(input), black_box(&mut output));
        black_box(&output);
    })
}

pub(super) fn run(samples: usize, rng: &mut crate::Rng) {
    integer::run(samples, rng);
    words::run(samples, rng);
    prime::run(samples, rng);
    binary::run(samples, rng);
    composite::run(samples, rng);
}
