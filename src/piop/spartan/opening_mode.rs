//! Compile-time opening modes for combined Spartan–BitZ proofs.

use core::marker::PhantomData;

use crate::{
    ligerito_flock::{IntEvalRsLigModQProof, IntEvalRsLigVirtProof},
    poly::mle::DenseMultilinearExtension,
};

use super::sumcheck::R1csProductMles;

mod private {
    pub trait Sealed {}
}

/// Selects the BitZ proof type paired with a Spartan proof.
pub trait OpeningMode: private::Sealed {
    /// Opening proof valid for this mode.
    type Proof;
}

/// Spartan constrains and opens the committed assignment `f` directly.
#[derive(Clone, Copy, Debug)]
pub enum Direct {}

/// Spartan constrains synthesized `h`; BitZ opens it through `h = M f`.
#[derive(Clone, Copy, Debug)]
pub enum Virtualized {}

impl private::Sealed for Direct {}
impl private::Sealed for Virtualized {}

impl OpeningMode for Direct {
    type Proof = IntEvalRsLigModQProof;
}

impl OpeningMode for Virtualized {
    type Proof = IntEvalRsLigVirtProof;
}

/// A Spartan proof and an opening proof whose mode agrees at compile time.
#[derive(Clone)]
pub struct SpartanBitzProof<S, M: OpeningMode> {
    spartan: S,
    bitz: M::Proof,
    _mode: PhantomData<fn() -> M>,
}

impl<S, M: OpeningMode> SpartanBitzProof<S, M> {
    /// Pairs proof components belonging to the same compile-time mode.
    pub const fn new(spartan: S, bitz: M::Proof) -> Self {
        Self {
            spartan,
            bitz,
            _mode: PhantomData,
        }
    }

    /// Spartan proof component.
    pub const fn spartan(&self) -> &S {
        &self.spartan
    }

    /// Direct or virtualized BitZ opening component.
    pub const fn bitz(&self) -> &M::Proof {
        &self.bitz
    }

    /// Borrows both proof components.
    pub const fn components(&self) -> (&S, &M::Proof) {
        (&self.spartan, &self.bitz)
    }

    /// Moves out both proof components.
    pub fn into_components(self) -> (S, M::Proof) {
        (self.spartan, self.bitz)
    }
}

/// An assignment and its already-evaluated `A`, `B`, and `C` products.
#[derive(Clone, Debug)]
pub struct EvaluatedSpartanAssignment<F> {
    assignment: DenseMultilinearExtension<F>,
    products: R1csProductMles<F>,
}

impl<F> EvaluatedSpartanAssignment<F> {
    /// Creates one internally consistent Spartan witness bundle.
    pub const fn new(
        assignment: DenseMultilinearExtension<F>,
        products: R1csProductMles<F>,
    ) -> Self {
        Self {
            assignment,
            products,
        }
    }

    /// Assignment MLE consumed by Spartan.
    pub const fn assignment(&self) -> &DenseMultilinearExtension<F> {
        &self.assignment
    }

    /// Evaluated `A`, `B`, and `C` product MLEs.
    pub const fn products(&self) -> &R1csProductMles<F> {
        &self.products
    }

    /// Moves out the assignment and products together.
    pub fn into_parts(self) -> (DenseMultilinearExtension<F>, R1csProductMles<F>) {
        (self.assignment, self.products)
    }
}
