use super::{Fp, FpCtx};
use crate::{CtMask, CtOrd, CtSelect, IntegerEmbedding, Uint, Z};

/// Signed projection with a prepared radix and public-width sign corrections.
/// Horner steps stay canonical; Montgomery output performs one final conversion.
#[derive(Clone, Debug)]
pub struct PreparedSignedProjection<const L: usize> {
    field: FpCtx<L>,
    radix: Fp<L>,
    width_powers: Vec<Uint<L>>,
    native_words_fit: bool,
}

impl<const L: usize> PreparedSignedProjection<L> {
    pub fn new(field: FpCtx<L>, max_words: usize) -> Self {
        let radix = field.from_integer(&(1u128 << 64));
        let mut width_powers =
            Vec::with_capacity(max_words.checked_add(1).expect("projection width overflow"));
        width_powers.push(Uint::ONE);
        for index in 0..max_words {
            width_powers.push(field.params().mul(&width_powers[index], &radix.words));
        }
        let native_words_fit = Uint::from_u64(u64::MAX).ct_lt(field.modulus()).declassify();
        Self {
            field,
            radix,
            width_powers,
            native_words_fit,
        }
    }

    /// Declared maximum input width, fixed before reading private values.
    pub fn max_words(&self) -> usize {
        self.width_powers.len() - 1
    }

    /// Explicit canonical output for the next canonical-input operation.
    pub fn project_canonical(&self, words: &[u64]) -> Uint<L> {
        assert!(
            words.len() <= self.max_words(),
            "integer exceeds prepared width"
        );
        self.canonical_kernel(words)
    }

    pub fn project(&self, words: &[u64]) -> Fp<L> {
        Fp::new(
            self.field
                .params()
                .from_canonical(&self.project_canonical(words)),
        )
    }

    /// Reuses output and validates the public shape once before the row loop.
    pub fn project_into<const N: usize>(&self, input: &[Z<N>], out: &mut [Fp<L>]) {
        assert_eq!(input.len(), out.len());
        assert!(N <= self.max_words(), "integer exceeds prepared width");
        for (value, slot) in input.iter().zip(out) {
            *slot = Fp::new(
                self.field
                    .params()
                    .from_canonical(&self.canonical_kernel(value.as_words())),
            );
        }
    }

    fn canonical_kernel(&self, words: &[u64]) -> Uint<L> {
        let params = self.field.params();
        let mut plain = Uint::ZERO;
        for &word in words.iter().rev() {
            let native = Uint::from_u64(word);
            let native = if self.native_words_fit {
                native
            } else {
                params.remainder(&native)
            };
            plain = params.add(&params.mul(&plain, &self.radix.words), &native);
        }
        let corrected = params.sub(&plain, &self.width_powers[words.len()]);
        let negative = CtMask::from_lsb(words.last().copied().unwrap_or(0) >> 63);
        Uint::ct_select(&plain, &corrected, negative)
    }
}
