//! Compile-time opening modes for combined Spartan–F2Z proofs.

use core::marker::PhantomData;

use crate::{
    ligerito_flock::{IntEvalRsLigModQProof, IntEvalRsLigVirtProof},
    poly::mle::DenseMultilinearExtension,
};

use super::sumcheck::R1csProductMles;

mod private {
    pub trait Sealed {}
}

/// Selects the F2Z proof type paired with a Spartan proof.
pub trait OpeningMode: private::Sealed {
    /// Opening proof valid for this mode.
    type Proof;
}

/// Spartan constrains and opens the committed assignment `f` directly.
#[derive(Clone, Copy, Debug)]
pub enum Direct {}

/// Spartan constrains synthesized `h`; F2Z opens it through `h = M f`.
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
pub struct SpartanF2zProof<S, M: OpeningMode> {
    spartan: S,
    f2z: M::Proof,
    _mode: PhantomData<fn() -> M>,
}

impl<S, M: OpeningMode> SpartanF2zProof<S, M> {
    /// Pairs proof components belonging to the same compile-time mode.
    pub const fn new(spartan: S, f2z: M::Proof) -> Self {
        Self {
            spartan,
            f2z,
            _mode: PhantomData,
        }
    }

    /// Spartan proof component.
    pub const fn spartan(&self) -> &S {
        &self.spartan
    }

    /// Direct or virtualized F2Z opening component.
    pub const fn f2z(&self) -> &M::Proof {
        &self.f2z
    }

    /// Borrows both proof components.
    pub const fn components(&self) -> (&S, &M::Proof) {
        (&self.spartan, &self.f2z)
    }

    /// Moves out both proof components.
    pub fn into_components(self) -> (S, M::Proof) {
        (self.spartan, self.f2z)
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
