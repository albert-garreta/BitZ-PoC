//! Their `tooling/cli/src/end_to_end.rs`: a deterministic circuit with
//! public inputs, prepared once (the R1CS matrices lowered to `Fq` with their
//! digest, the materialised `M^T` with its digest, the two shapes, the
//! parameters, the PCS), then witness → commit → prove → verify. The proof
//! is `{ root, Spartan proof (out of band), opening transcript }`.
//!
//! Transcript: session `bitz/circuit-e2e/v1`, instance the statement's
//! domain; `bind` absorbs the public bytes' length and bytes, the root, the
//! parameters, the PCS and the map digest; then the Spartan PIOP, then the
//! virtual opening.

use circuit::Circuit;
use circuit::constraints::ConstraintGenerator;
use circuit::matrix_transpose::{MTransposeGenerator, MaterializedMTranspose};
use circuit::witgen::Witgen;
use flock_core::merkle::HashKind;

use super::fq::{Fq, Q};
use super::map::map_digest;
use super::params::{BitZParams, LinearClaim, Root, Shape, MIN_LOG_BITS};
use super::pcs::{CommitError, Pcs};
use super::spartan::{
    MatrixError, PreparedConstraintMatrices, Products, ScaledMleEvaluationClaim, SpartanError,
    SpartanPiopProof, eq_table, prove_spartan_piop, verify_spartan_proof,
};
use super::transcript::{Proof as TranscriptProof, PublicTranscript, build_prover, build_verifier};
use super::virt::{VirtualStatement, VirtualStatementError, VirtualTable};
use super::{BitZProver, BitZVerifier, ProveError, VerifyError, WINDOW};
use crate::ligerito_flock::FlockCommitHint;
use crate::poly::univariate::binary_gf128::BinaryFieldGF128 as Gf;

/// Their session label.
pub const SESSION: &[u8] = b"bitz/circuit-e2e/v1";

/// A trusted, deterministic circuit and its public inputs. Implementations
/// must emit identical operations for symbolic and concrete backends and
/// constrain every public input/output.
pub trait CircuitStatement {
    fn domain(&self) -> &'static [u8];
    fn public_bytes(&self) -> Vec<u8>;
    fn input_bits(&self) -> usize;
    fn synthesize<C: Circuit>(&self, cs: &mut C, inputs: &[C::Bool]) -> Result<(), Error>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    Input(&'static str),
    Configuration(&'static str),
    Matrix(MatrixError),
    Unsatisfied,
    Spartan(SpartanError),
    Commit(CommitError),
    VirtualStatement(VirtualStatementError),
    Prove(ProveError),
    Verify(VerifyError),
}

impl From<MatrixError> for Error {
    fn from(error: MatrixError) -> Self {
        Self::Matrix(error)
    }
}

impl From<VirtualStatementError> for Error {
    fn from(error: VirtualStatementError) -> Self {
        Self::VirtualStatement(error)
    }
}

/// Their generator: the smallest generator of `GF(2^128)^×` in this
/// representation, `X`.
pub fn generator() -> Gf {
    crate::pcs::smallest_generator()
}

/// Their `shape_for` on the oracle branch: padded to a power of two, no
/// smaller than the protocol's minimum, split by the reference rule
/// `t = ⌈3n/5⌉` (no narrower than the packing width, at least one column
/// bit).
pub fn shape_for(bits: usize) -> Result<Shape, Error> {
    let padded = bits
        .checked_next_power_of_two()
        .ok_or(Error::Configuration("witness too large"))?;
    let log_bits = (padded.ilog2() as usize).max(MIN_LOG_BITS);
    let log_rows = (3 * log_bits).div_ceil(5).max(7).min(log_bits - 1);
    Shape::new(log_rows, log_bits - log_rows)
        .map_err(|_| Error::Configuration("witness shape outside supported range"))
}

/// Their `opening_claim`: the terminal Spartan claim as a `LinearClaim` on
/// `h` — the point zero-extended to the claim shape, `rows = scale ·
/// eq(point[..t])`, `columns = eq(point[t..])`, target the claimed value.
pub fn opening_claim(
    params: &BitZParams,
    terminal: &ScaledMleEvaluationClaim,
) -> Result<LinearClaim, Error> {
    let shape = params.shape();
    if terminal.point.len() > shape.log_bits() {
        return Err(Error::Configuration("Spartan point exceeds virtual shape"));
    }
    let mut point = terminal.point.clone();
    point.resize(shape.log_bits(), Fq::ZERO);
    let rows = eq_table(&point[..shape.log_rows()])
        .into_iter()
        .map(|weight| (terminal.scale * weight).lift())
        .collect();
    let columns = eq_table(&point[shape.log_rows()..])
        .into_iter()
        .map(Fq::lift)
        .collect();
    LinearClaim::new(params, rows, columns, terminal.value.lift())
        .map_err(|_| Error::Configuration("invalid terminal claim dimensions"))
}

/// Everything derived from the statement alone.
#[derive(Debug)]
pub struct Prepared<S> {
    statement: S,
    matrices: PreparedConstraintMatrices,
    map: MaterializedMTranspose,
    map_digest: [u8; 32],
    params: BitZParams,
    committed_shape: Shape,
    pcs: Pcs,
}

/// The witness for one input: `f` as one row of `2^m` bits (the flat packed
/// vector the commitment is over), `h` laid out at the claim shape, the
/// products and the assignment.
#[derive(Debug)]
pub struct Witness {
    committed_row: Vec<u64>,
    virtual_bits: VirtualTable,
    products: Products,
    assignment: Vec<Fq>,
}

impl Witness {
    pub fn committed_row(&self) -> &[u64] {
        &self.committed_row
    }

    pub fn virtual_bits(&self) -> &VirtualTable {
        &self.virtual_bits
    }

    pub fn products(&self) -> &Products {
        &self.products
    }

    pub fn assignment(&self) -> &[Fq] {
        &self.assignment
    }
}

/// Their `Proof`, plus the terminal claim the prover derived (the verifier
/// recomputes it; the dumps carry it).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Proof {
    pub root: Root,
    pub spartan: SpartanPiopProof,
    pub terminal: ScaledMleEvaluationClaim,
    pub opening: TranscriptProof,
}

impl<S: CircuitStatement> Prepared<S> {
    /// Their `Prepared::new`.
    pub fn new(statement: S) -> Result<Self, Error> {
        let mut constraints = ConstraintGenerator::new(statement.input_bits());
        let inputs: Vec<_> = (0..statement.input_bits())
            .map(|i| constraints.input(i))
            .collect();
        statement.synthesize(&mut constraints, &inputs)?;
        let matrices = PreparedConstraintMatrices::from_vendored(&constraints.into_matrices())?;
        let mut generator = MTransposeGenerator::new(statement.input_bits());
        let inputs = generator.take_inputs();
        statement.synthesize(&mut generator, &inputs)?;
        let map = generator.finish();
        if map.row_count() != matrices.column_count() {
            return Err(Error::Configuration("map and assignment dimensions differ"));
        }
        let claim_shape = shape_for(map.row_count())?;
        let committed_shape = shape_for(map.column_count() - 1)?;
        let params = BitZParams::new(claim_shape, Q, self::generator())
            .map_err(|_| Error::Configuration("inadmissible BitZ parameters"))?;
        let pcs = Pcs::new(&committed_shape, HashKind::Blake3)
            .map_err(|_| Error::Configuration("unsupported PCS shape"))?;
        let map_digest = map_digest(&map);
        Ok(Self {
            statement,
            matrices,
            map,
            map_digest,
            params,
            committed_shape,
            pcs,
        })
    }

    pub fn statement(&self) -> &S {
        &self.statement
    }

    pub fn matrices(&self) -> &PreparedConstraintMatrices {
        &self.matrices
    }

    pub fn map(&self) -> &MaterializedMTranspose {
        &self.map
    }

    pub fn map_digest(&self) -> &[u8; 32] {
        &self.map_digest
    }

    pub fn params(&self) -> &BitZParams {
        &self.params
    }

    pub fn committed_shape(&self) -> &Shape {
        &self.committed_shape
    }

    pub fn pcs(&self) -> &Pcs {
        &self.pcs
    }

    /// The single-row shape the commitment to `f` is made under (the flat
    /// packed vector; the root is the same as under the committed split).
    pub fn commit_shape(&self) -> Shape {
        Shape::new(self.committed_shape.log_bits(), 0).expect("a valid bit count")
    }

    /// Their `Prepared::witness`.
    pub fn witness(&self, inputs: &[bool]) -> Result<Witness, Error> {
        if inputs.len() != self.statement.input_bits() {
            return Err(Error::Input("wrong witness input length"));
        }
        let mut witgen = Witgen::with_inputs(inputs);
        self.statement.synthesize(&mut witgen, inputs)?;
        let (f, h) = witgen.into_witnesses();
        if f.bit_len() + 1 != self.map.column_count() || h.bit_len() != self.map.row_count() {
            return Err(Error::Input("circuit replay changed witness dimensions"));
        }
        let products = self.matrices.products(&h)?;
        if !products.satisfied() {
            return Err(Error::Unsatisfied);
        }
        let assignment = self.matrices.assignment(&h)?;
        let mut committed_row = f.words().to_vec();
        committed_row.resize((1 << self.committed_shape.log_bits()) / 64, 0);
        let virtual_bits = VirtualTable::new(self.params.shape(), h.words(), h.bit_len());
        Ok(Witness {
            committed_row,
            virtual_bits,
            products,
            assignment,
        })
    }

    /// Their `Prepared::commit`: the flat `f` under the PCS.
    pub fn commit(&self, witness: &Witness) -> Result<(Root, FlockCommitHint), Error> {
        self.pcs
            .commit(&self.commit_shape(), vec![witness.committed_row.clone()])
            .map_err(Error::Commit)
    }

    /// Their `Prepared::prove`.
    pub fn prove(&self, witness: &Witness, hint: &FlockCommitHint) -> Result<Proof, Error> {
        let root = Root(*hint.root());
        let mut transcript = build_prover(SESSION, self.statement.domain());
        self.bind(&mut transcript, &root);
        let (spartan, terminal) = prove_spartan_piop(
            &mut transcript,
            &self.matrices,
            &witness.products,
            &witness.assignment,
        )
        .map_err(Error::Spartan)?;
        let claim = opening_claim(&self.params, &terminal)?;
        let statement = VirtualStatement::new(
            self.params,
            self.committed_shape,
            &self.map,
            self.map_digest,
            &claim,
        )?;
        BitZProver::new(self.params, WINDOW)
            .prove_virtual(&statement, &self.pcs, hint, &witness.virtual_bits, &mut transcript)
            .map_err(Error::Prove)?;
        Ok(Proof {
            root,
            spartan,
            terminal,
            opening: transcript.finish(),
        })
    }

    /// Their `Prepared::verify`.
    pub fn verify(&self, proof: &Proof) -> Result<(), Error> {
        let mut transcript = build_verifier(SESSION, self.statement.domain(), &proof.opening);
        self.bind(&mut transcript, &proof.root);
        let terminal = verify_spartan_proof(&mut transcript, &self.matrices, &proof.spartan)
            .map_err(Error::Spartan)?;
        let claim = opening_claim(&self.params, &terminal)?;
        let statement = VirtualStatement::new(
            self.params,
            self.committed_shape,
            &self.map,
            self.map_digest,
            &claim,
        )?;
        BitZVerifier::new(self.params, WINDOW)
            .verify_virtual(&statement, &self.pcs, proof.root, transcript)
            .map_err(Error::Verify)
    }

    /// Their `bind`.
    fn bind<T: PublicTranscript>(&self, transcript: &mut T, root: &Root) {
        let public = self.statement.public_bytes();
        transcript.public_message(&(public.len() as u64));
        transcript.public_message(public.as_slice());
        transcript.public_message(&root.0);
        transcript.public_message(&self.params);
        transcript.public_message(&self.pcs);
        transcript.public_message(&self.map_digest);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shapes_follow_the_reference_split() {
        assert_eq!(shape_for(1).unwrap(), Shape::new(14, 8).unwrap());
        assert_eq!(shape_for(20_457).unwrap(), Shape::new(14, 8).unwrap());
        assert_eq!(shape_for(1 << 22).unwrap(), Shape::new(14, 8).unwrap());
        assert_eq!(shape_for((1 << 22) + 1).unwrap(), Shape::new(14, 9).unwrap());
        assert_eq!(shape_for(12_000_000).unwrap(), Shape::new(15, 9).unwrap());
    }

    #[test]
    fn the_generator_is_x() {
        assert_eq!(generator(), Gf::from_words([2, 0]));
    }
}
