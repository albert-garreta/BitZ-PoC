//! A recorded circuit with SHA-256 compression blocks split in two relations
//! on one commitment (the BitZ paper's §2.4.1: commit once to the
//! concatenated witnesses, run each relation until its claim on the
//! committed bits, prove the claims together).
//!
//! * The compressions: one batch of the independent-compressions relation
//!   ([`super::sha256`]: the Ch-only CM relation and its product sumcheck),
//!   committed in its `Id ⊗ M` block layout, `2^k` compressions — the
//!   circuit's, then dummy compressions of zero inputs that nothing reads.
//! * The glue: the rest of the circuit as a FlockRec relation
//!   ([`super::flockrec`]) on the circuit with every compression block
//!   replaced by a stub ([`Glue`]). A stub's hints are its compression's
//!   768 input bits and 256 output bits — the batch's own cells of that
//!   instance, shared ([`Stacked`]) — and 256 copies of the outputs; its
//!   cells bind the inputs to their wiring (`input ⊕ wiring = 0`) and the
//!   copies to the outputs, each with the row `cell = 0`; its outputs are the
//!   copies, so whatever reads a compression reads a copy. The copies, the
//!   glue's constant and the tail's committed bits fill each instance
//!   block's top rows, which the compression relation leaves unread.
//!
//! One Round 0 follows both statements; each relation runs its PIOP to its
//! claim on its derived grid; [`crate::bitz::shared`] opens the two claims
//! with one sumcheck, ring switch and Ligerito opening.

use std::sync::Arc;

use circuit::compact::{BlockInstance, BlockTemplate, CBit, Coeff, CompactRows, LAZY, StructuredCircuit, Terms};
use circuit::witgen::PackedWitness;
use flock_core::pcs::commit::Commitment;
#[cfg(feature = "parallel")]
use rayon::prelude::*;

use crate::bitz::{BlockStatement, LinearClaim, Root, VirtualStatement, build_prover, build_verifier};
use crate::ligerito::LOG_PACKING;
use crate::ligerito_flock::{FlockCommitHint, LigeritoSelection, bind_prover_ood, bind_verifier_ood, commit_rs_ligerito_rows};
use crate::pcs::IntegerMatrixLayout;
use crate::transcript::traits::Transcript;

use super::absorb_spartan_message;
use super::flockrec::{FlockRecPiop, FlockRecRelation, FlockRecWitness, Stacked};
use super::profile::IopSecurityProfile;
use super::protocol::ProtocolError;
use super::protocol::bitz_opener::{BitZOpeningProof, fork_tag, opening_params, opening_shape, pcs_from_config};
use super::protocol::linear::LinearPiopProof;
use super::sha256::{
    PreparedSha256CompressionBatch, SHA256_MIN_LOG_COMPRESSIONS, Sha256CompressionInput,
    Sha256CompressionWitnessBatch, generate_sha256_compression_witnesses,
    prepare_sha256_compression_batch_block_at, prepare_sha256_compression_batch_block_with_profile,
    sha256_public_f_column, shared,
};

const PROTOCOL: &[u8] = b"bitz/split/sha256-batch+flockrec/v1";
const SESSION: &[u8] = b"bitz/split/shared-opening/v1";
const GLUE_SOURCE: &[u8] = b"bitz/split/glue-source/v1";

/// A compression block's input slots (the block's 512 bits, then the
/// state's 256) and outputs.
const INPUTS: usize = 768;
const OUTPUTS: usize = 256;
/// A stub's hints (inputs, outputs, copies) and cells (input bindings,
/// output bindings).
const STUB_HINTS: usize = INPUTS + 2 * OUTPUTS;
const STUB_CELLS: usize = INPUTS + OUTPUTS;

/// The compression relation's local committed row of a stub's input or
/// output hint `h < INPUTS + OUTPUTS`: the slot order (block word `w` bit
/// `b` at `32w + b`, then the state's) is the relation's source order from
/// column 1, and output word `w` bit `b` is its column `6881 + 33w + b`.
fn shared_row(h: usize) -> usize {
    if h < INPUTS {
        1 + h
    } else {
        let k = h - INPUTS;
        sha256_public_f_column(24 + k / 32, k % 32)
    }
}

/// The stub template (see the module docs). Local ids: hints `0..1280`
/// (inputs, outputs, copies), slots `1280 + s`; row `l` is `cell l = 0`.
fn stub_template() -> BlockTemplate {
    let mut m_offsets = Vec::with_capacity(STUB_CELLS + 1);
    let mut m_terms = Vec::with_capacity(2 * STUB_CELLS);
    m_offsets.push(0);
    for s in 0..INPUTS {
        m_terms.extend([s as u32, (STUB_HINTS + s) as u32]);
        m_offsets.push(m_terms.len() as u32);
    }
    for k in 0..OUTPUTS {
        m_terms.extend([(INPUTS + k) as u32, (INPUTS + OUTPUTS + k) as u32]);
        m_offsets.push(m_terms.len() as u32);
    }
    let empty = || CompactRows { offsets: vec![0; STUB_CELLS + 1], columns: Vec::new(), coeffs: Vec::new() };
    let c = CompactRows {
        offsets: (0..=STUB_CELLS as u64).collect(),
        columns: (1..=STUB_CELLS as u32).collect(),
        coeffs: vec![Coeff::Pow2 { neg: false, shift: 0 }; STUB_CELLS],
    };
    BlockTemplate {
        kind: "sha256-stub",
        slots: INPUTS,
        hints: STUB_HINTS,
        m_offsets,
        m_terms,
        m_const: vec![false; STUB_CELLS],
        a: empty(),
        b: empty(),
        c,
        row_limbs: vec![1; STUB_CELLS],
        outputs: (0..OUTPUTS)
            .map(|k| CBit { constant: false, terms: Terms::from_slice(&[(INPUTS + OUTPUTS + k) as u32]) })
            .collect(),
    }
}

/// The glue circuit of a recording, and what its witness needs.
pub struct Glue {
    pub circuit: StructuredCircuit,
    /// Compression output `k` is the template's local hint `outputs[k]`.
    outputs: Vec<u32>,
}

impl Glue {
    /// The recording's compression blocks (one template, 768 input slots, 256
    /// outputs each one committed bit) as stubs: committed bits renumbered
    /// (stubs' hints `i·1280..`, then the tail's in order; a compression's
    /// output hint becomes its stub's copy, and no other block hint may be
    /// read from outside its block), cells and rows likewise (stubs', then
    /// the tail's).
    pub fn new(st: &StructuredCircuit) -> Result<Self, String> {
        let [template] = st.templates.as_slice() else {
            return Err("glue: exactly one block template".into());
        };
        if template.slots != INPUTS || template.outputs.len() != OUTPUTS {
            return Err(format!("glue: a {}-slot, {}-output block is not a compression", template.slots, template.outputs.len()));
        }
        let mut output_of = vec![u32::MAX; template.hints];
        let mut outputs = Vec::with_capacity(OUTPUTS);
        for (k, out) in template.outputs.iter().enumerate() {
            match out.terms.as_slice() {
                [h] if !out.constant && (*h as usize) < template.hints && output_of[*h as usize] == u32::MAX => {
                    output_of[*h as usize] = k as u32;
                    outputs.push(*h);
                }
                _ => return Err(format!("glue: compression output {k} is not one committed bit")),
            }
        }
        let n = st.instances.len();
        let hints = template.hints as u32;
        let tail_base = u32::try_from(n * STUB_HINTS + st.tail_hints.len())
            .map(|end| end - st.tail_hints.len() as u32)
            .map_err(|_| "glue: too many committed bits".to_string())?;
        let renumber = |g: u32| -> Result<u32, String> {
            if g >= LAZY {
                return Ok(g);
            }
            if let Ok(k) = st.tail_hints.binary_search(&g) {
                return Ok(tail_base + k as u32);
            }
            let i = st.instances.partition_point(|inst| inst.hint_base + hints <= g);
            match st.instances.get(i) {
                Some(inst) if inst.hint_base <= g => match output_of[(g - inst.hint_base) as usize] {
                    u32::MAX => Err(format!("glue: block {i}'s hint {} is read from outside", g - inst.hint_base)),
                    k => Ok((i * STUB_HINTS + INPUTS + OUTPUTS) as u32 + k),
                },
                _ => Err(format!("glue: committed bit {g} is neither a block's nor the tail's")),
            }
        };
        let renumber_bit = |b: &CBit| -> Result<CBit, String> {
            let mut terms: Vec<u32> = b.terms.iter().map(|&t| renumber(t)).collect::<Result<_, _>>()?;
            terms.sort_unstable();
            Ok(CBit { constant: b.constant, terms: Terms::from_vec(terms) })
        };
        let instance = |(i, inst): (usize, &BlockInstance)| -> Result<BlockInstance, String> {
            if inst.template != 0 || inst.inputs.len() != INPUTS {
                return Err(format!("glue: instance {i} shape"));
            }
            Ok(BlockInstance {
                template: 0,
                hint_base: (i * STUB_HINTS) as u32,
                cell_base: (i * STUB_CELLS) as u32,
                row_base: (i * STUB_CELLS) as u32,
                inputs: inst.inputs.iter().map(renumber_bit).collect::<Result<_, _>>()?,
            })
        };
        #[cfg(feature = "parallel")]
        let instances: Vec<BlockInstance> = st.instances.par_iter().enumerate().map(instance).collect::<Result<_, _>>()?;
        #[cfg(not(feature = "parallel"))]
        let instances: Vec<BlockInstance> = st.instances.iter().enumerate().map(instance).collect::<Result<_, _>>()?;
        // The tail: committed bits renumbered, cells after the stubs' (its
        // rows read tail cells only), lazy ids and groups' order unchanged.
        let cell_base = n * STUB_CELLS;
        let mut tail_m_terms = Vec::with_capacity(st.tail_m_terms.len());
        for k in 0..st.tail_cells.len() {
            let (s, e) = (st.tail_m_offsets[k] as usize, st.tail_m_offsets[k + 1] as usize);
            let at = tail_m_terms.len();
            for &t in &st.tail_m_terms[s..e] {
                tail_m_terms.push(renumber(t)?);
            }
            tail_m_terms[at..].sort_unstable();
        }
        let cell_column = |column: u32| -> Result<u32, String> {
            if column == 0 {
                return Ok(0);
            }
            st.tail_cells
                .binary_search(&(column - 1))
                .map(|k| (cell_base + k + 1) as u32)
                .map_err(|_| format!("glue: a tail row reads cell {} outside the tail", column - 1))
        };
        let rows = |r: &CompactRows| -> Result<CompactRows, String> {
            Ok(CompactRows {
                offsets: r.offsets.clone(),
                columns: r.columns.iter().map(|&c| cell_column(c)).collect::<Result<_, _>>()?,
                coeffs: r.coeffs.clone(),
            })
        };
        let mut groups = Vec::with_capacity(st.groups.len());
        for group in &st.groups {
            let mut terms = Vec::with_capacity(group.terms.len());
            for (columns, inputs) in &group.terms {
                terms.push((*columns, inputs.iter().map(renumber_bit).collect::<Result<Vec<_>, _>>()?));
            }
            groups.push(circuit::compact::LinearGroup { terms, constant: group.constant });
        }
        let circuit = StructuredCircuit {
            committed: tail_base as usize + st.tail_hints.len(),
            cells: cell_base + st.tail_cells.len(),
            rows: n * STUB_CELLS + st.tail_rows.len(),
            templates: vec![Arc::new(stub_template())],
            instances,
            tail_hints: (0..st.tail_hints.len() as u32).map(|k| tail_base + k).collect(),
            tail_cells: (0..st.tail_cells.len()).map(|k| (cell_base + k) as u32).collect(),
            tail_m_offsets: st.tail_m_offsets.clone(),
            tail_m_terms,
            tail_m_const: st.tail_m_const.clone(),
            tail_rows: (0..st.tail_rows.len()).map(|k| (n * STUB_CELLS + k) as u32).collect(),
            tail_a: rows(&st.tail_a)?,
            tail_b: rows(&st.tail_b)?,
            tail_c: rows(&st.tail_c)?,
            tail_row_limbs: st.tail_row_limbs.clone(),
            groups,
        };
        Ok(Self { circuit, outputs })
    }

    /// The glue's witness and the batch's inputs from the recording's
    /// witness (`g`: committed bits in recording order; `h`: `M·g`, entry 0
    /// the constant): `g'` holds each stub's inputs (its wiring's values),
    /// outputs and copies (the compression's committed outputs), then the
    /// tail's committed bits; `h'` the constant, every binding cell zero,
    /// the tail cells. Each compression's input in the batch's form.
    pub fn witness(
        &self,
        st: &StructuredCircuit,
        g: &PackedWitness,
        h: &PackedWitness,
    ) -> Result<(PackedWitness, PackedWitness, Vec<Sha256CompressionInput>), String> {
        glue_witness(&self.outputs, st, g, h)
    }
}

/// [`Glue::witness`] from the compressions' output hints alone.
fn glue_witness(
    outputs: &[u32],
    st: &StructuredCircuit,
    g: &PackedWitness,
    h: &PackedWitness,
) -> Result<(PackedWitness, PackedWitness, Vec<Sha256CompressionInput>), String> {
    let n = st.instances.len();
    if g.bit_len() != st.committed || h.bit_len() != st.cells + 1 || outputs.len() != OUTPUTS {
        return Err("glue witness: shape".into());
    }
    const WORDS: usize = STUB_HINTS / 64;
    let stub = |inst: &BlockInstance| -> ([u64; WORDS], Sha256CompressionInput) {
        let mut words = [0u64; WORDS];
        let mut set = |k: usize| words[k / 64] |= 1u64 << (k % 64);
        let mut input = ([0u32; 8], [0u32; 16]);
        for (s, bit) in inst.inputs.iter().enumerate() {
            if bit.terms.iter().fold(bit.constant, |acc, &t| acc ^ g.bit(t as usize)) {
                set(s);
                if s < 512 {
                    input.1[s / 32] |= 1 << (s % 32);
                } else {
                    input.0[(s - 512) / 32] |= 1 << (s % 32);
                }
            }
        }
        for (k, &local) in outputs.iter().enumerate() {
            if g.bit(inst.hint_base as usize + local as usize) {
                set(INPUTS + k);
                set(INPUTS + OUTPUTS + k);
            }
        }
        (words, input)
    };
    #[cfg(feature = "parallel")]
    let stubs: Vec<_> = st.instances.par_iter().map(stub).collect();
    #[cfg(not(feature = "parallel"))]
    let stubs: Vec<_> = st.instances.iter().map(stub).collect();
    let tail = st.tail_hints.len();
    let mut g_words = Vec::with_capacity((n * STUB_HINTS + tail).div_ceil(64));
    let mut inputs = Vec::with_capacity(n);
    for (words, input) in stubs {
        g_words.extend_from_slice(&words);
        inputs.push(input);
    }
    g_words.resize((n * STUB_HINTS + tail).div_ceil(64), 0);
    for (k, &gi) in st.tail_hints.iter().enumerate() {
        if g.bit(gi as usize) {
            let at = n * STUB_HINTS + k;
            g_words[at / 64] |= 1u64 << (at % 64);
        }
    }
    let cells = n * STUB_CELLS + st.tail_cells.len();
    let mut h_words = vec![0u64; (cells + 1).div_ceil(64)];
    h_words[0] = 1;
    for (k, &c) in st.tail_cells.iter().enumerate() {
        if h.bit(1 + c as usize) {
            let at = 1 + n * STUB_CELLS + k;
            h_words[at / 64] |= 1u64 << (at % 64);
        }
    }
    Ok((PackedWitness::from_words(g_words, n * STUB_HINTS + tail), PackedWitness::from_words(h_words, cells + 1), inputs))
}

/// The relation pair over one commitment (both roles build it from the
/// recording).
pub struct SplitRelation {
    glue: FlockRecRelation,
    outputs: Vec<u32>,
    sha: PreparedSha256CompressionBatch,
    layout: IntegerMatrixLayout,
    compressions: usize,
}

/// The prover's witness: the batch's (its derived rows) and the glue's (its
/// derived rows; its committed rows are the shared grid's).
pub struct SplitWitness {
    pub sha: Sha256CompressionWitnessBatch,
    pub glue: FlockRecWitness,
}

/// A proof: the two PIOPs and the shared opening.
#[derive(Clone)]
pub struct SplitProof {
    pub sha: LinearPiopProof,
    pub glue: FlockRecPiop,
    pub bitz: BitZOpeningProof,
}

impl SplitProof {
    /// Bytes outside the opening.
    pub fn piop_bytes(&self) -> usize {
        self.sha.bytes() + self.glue.bytes()
    }
}

/// The witness pass's costs, apart (seconds).
#[derive(Clone, Copy, Debug, Default)]
pub struct SplitWitnessTimes {
    /// The glue's witness and the compressions' inputs.
    pub glue_s: f64,
    /// The compression relation's witness (every compression's trace).
    pub sha_s: f64,
    /// The shared grid's rows.
    pub rows_s: f64,
}

impl SplitRelation {
    /// `source_digest` must commit to everything the recording is a
    /// function of; `glue_rows`: the glue opening's derived rows (at least
    /// `10 + r` for `2^r` compressions per committed column).
    pub fn new<P: IopSecurityProfile>(
        st: &StructuredCircuit,
        source_digest: [u8; 32],
        selection: LigeritoSelection,
        glue_rows: usize,
    ) -> Result<Self, ProtocolError> {
        Self::new_at::<P>(st, source_digest, selection, glue_rows, None)
    }

    /// [`SplitRelation::new`] with the batch's product-tensor rows `sha_rows`
    /// (`14 + r` for `2^r` compressions per committed column; `None`: the
    /// batch's own split).
    pub fn new_at<P: IopSecurityProfile>(
        st: &StructuredCircuit,
        source_digest: [u8; 32],
        selection: LigeritoSelection,
        glue_rows: usize,
        sha_rows: Option<usize>,
    ) -> Result<Self, ProtocolError> {
        let geometry = |msg: String| ProtocolError::LigeritoConfig(format!("split: {msg}"));
        let glue = Glue::new(st).map_err(geometry)?;
        let compressions = st.instances.len();
        let k = ((compressions.max(2) - 1).ilog2() as usize + 1).max(SHA256_MIN_LOG_COMPRESSIONS);
        let sha = match sha_rows {
            Some(rows) => prepare_sha256_compression_batch_block_at::<P>(k, rows),
            None => prepare_sha256_compression_batch_block_with_profile::<P>(k),
        };
        let sha = sha
            .map_err(|e| geometry(format!("SHA batch: {e:?}")))?
            .with_private_io()
            .with_ligerito(selection)
            .map_err(|e| geometry(format!("SHA batch opener: {e:?}")))?;
        let (map, _) = shared::block_claim_geometry(&sha)?;
        let layout = *sha.block_committed_params().ok_or(ProtocolError::InvalidGeometry)?;
        let a = map.log_source_block();
        let (r, s) = (layout.row_vars - a, layout.col_vars);
        // The box: the smallest top band of each block clear of the
        // compression's columns that holds the copies, the constant and
        // the tail.
        let live = map.local().cols();
        let tail = glue.circuit.tail_hints.len();
        let stacked = (8..=a)
            .map(|box_log| Stacked {
                a,
                r,
                s,
                pos: (0..STUB_HINTS)
                    .map(|h| if h < INPUTS + OUTPUTS { shared_row(h) } else { (1 << a) - (1 << box_log) + h - INPUTS - OUTPUTS } as u32)
                    .collect(),
                box_log,
                used: OUTPUTS,
            })
            .take_while(|l| l.box_start() >= live)
            .find(|l| l.capacity(compressions) > tail)
            .ok_or_else(|| geometry(format!("{tail} tail bits do not fit the blocks' top rows")))?;
        let mut hasher = blake3::Hasher::new();
        hasher.update(GLUE_SOURCE);
        hasher.update(&source_digest);
        hasher.update(&(k as u64).to_le_bytes());
        let glue_digest = *hasher.finalize().as_bytes();
        let outputs = glue.outputs.clone();
        let rel = FlockRecRelation::new_stacked::<P>(Arc::new(glue.circuit), glue_digest, selection, glue_rows, stacked)?;
        if *rel.committed_layout() != layout || rel.ligerito().digest() != sha.ligerito_configuration()?.digest() {
            return Err(geometry("the glue's grid or opener is not the batch's".into()));
        }
        Ok(Self { glue: rel, outputs, sha, layout, compressions })
    }

    pub fn glue(&self) -> &FlockRecRelation {
        &self.glue
    }

    pub fn sha(&self) -> &PreparedSha256CompressionBatch {
        &self.sha
    }

    /// The shared committed grid.
    pub fn committed_layout(&self) -> &IntegerMatrixLayout {
        &self.layout
    }

    /// The circuit's compressions (the batch's real instances).
    pub fn compressions(&self) -> usize {
        self.compressions
    }

    /// The prover's rows from the recording and its witness (`g`, `h` as
    /// [`FlockRecRelation::witness`] takes them), with the costs apart.
    pub fn witness(
        &self,
        st: &StructuredCircuit,
        g: &PackedWitness,
        h: &PackedWitness,
    ) -> Result<(SplitWitness, SplitWitnessTimes), ProtocolError> {
        let geometry = |msg: String| ProtocolError::LigeritoConfig(format!("split witness: {msg}"));
        let started = std::time::Instant::now();
        let (g2, h2, mut inputs) = glue_witness(&self.outputs, st, g, h).map_err(geometry)?;
        let glue_s = started.elapsed().as_secs_f64();
        let started = std::time::Instant::now();
        inputs.resize(self.sha.instances(), ([0; 8], [0; 16]));
        let sha = generate_sha256_compression_witnesses(&self.sha, &inputs).map_err(|e| geometry(format!("{e:?}")))?;
        let sha_s = started.elapsed().as_secs_f64();
        // The compressions' outputs are the circuit's committed outputs.
        for (i, inst) in st.instances.iter().enumerate() {
            let output = sha.outputs()[i];
            for (k, &local) in self.outputs.iter().enumerate() {
                if g.bit(inst.hint_base as usize + local as usize) != ((output[k / 32] >> (k % 32)) & 1 == 1) {
                    return Err(geometry(format!("compression {i}: output bit {k} is not the circuit's")));
                }
            }
        }
        let started = std::time::Instant::now();
        let rows = shared::block_rows(&self.sha, &sha)?;
        let glue = self.glue.witness_stacked(&g2, &h2, rows)?;
        let rows_s = started.elapsed().as_secs_f64();
        Ok((SplitWitness { sha, glue }, SplitWitnessTimes { glue_s, sha_s, rows_s }))
    }

    /// The commitment to the shared grid.
    pub fn commit(&self, witness: &SplitWitness) -> FlockCommitHint {
        commit_rs_ligerito_rows(&self.layout, witness.glue.f_rows.clone(), self.glue.ligerito().prover())
    }

    fn pcs(&self, prover: bool) -> Result<crate::bitz::Pcs, ProtocolError> {
        let committed = opening_shape(&self.layout)?;
        let lig = self.glue.ligerito();
        let config: &dyn crate::ligerito_flock::LigeritoStatementConfig = if prover { lig.prover() } else { lig.verifier() };
        Ok(pcs_from_config(&committed, config)?
            .with_native_policy(self.glue.security().native_policy()?)
            .with_extension(lig.extension()))
    }

    pub fn prove<T: Transcript + Send>(
        &self,
        transcript: &mut T,
        witness: &SplitWitness,
        hint: &FlockCommitHint,
    ) -> Result<SplitProof, ProtocolError> {
        let pc = self.glue.ligerito().prover();
        shared::check_prover(&self.sha, &witness.sha, hint, pc)?;
        absorb_spartan_message(transcript, b"protocol", PROTOCOL);
        let binding = shared::bind_statement(transcript, &self.sha, &hint.commitment, pc)?;
        self.glue.bind_statement(transcript, &hint.commitment)?;
        let ood = bind_prover_ood(transcript, hint, self.glue.security().ood);
        let started = std::time::Instant::now();
        let (sha_piop, sha_claim) = shared::prove_piop(transcript, &self.sha, &witness.sha, binding)?;
        trace("sha piop", started);
        let started = std::time::Instant::now();
        let (glue_piop, glue_claim) = self.glue.prove_piop(transcript, &witness.glue)?;
        trace("glue piop", started);

        let started = std::time::Instant::now();
        let ood = ood.opening_claim(transcript, hint);
        let mut state = build_prover(SESSION, &fork_tag(transcript));
        let (map, opened) = shared::block_claim_geometry(&self.sha)?;
        let committed = opening_shape(&self.layout)?;
        let sha_params = opening_params(opened, sha_claim.prime.modulus_u128())?;
        let sha_native = LinearClaim::new(&sha_params, sha_claim.rows, sha_claim.cols, sha_claim.claimed).map_err(claim_error)?;
        let block = BlockStatement::new(sha_params, committed, map, &sha_native).map_err(|e| statement_error(format!("{e:?}")))?;
        let glue_params = opening_params(self.glue.derived_layout(), glue_claim.prime.modulus_u128())?;
        let (rows, cols, target) = glue_claim.claim;
        let glue_native = LinearClaim::new(&glue_params, rows, cols, target).map_err(claim_error)?;
        let glue = VirtualStatement::new(glue_params, committed, self.glue.map(), &glue_native)
            .map_err(|e| statement_error(format!("{e:?}")))?;
        crate::bitz::shared::prove_shared(
            &block,
            shared::derived_rows(&witness.sha)?,
            &glue,
            &witness.glue.h_rows,
            &self.pcs(true)?,
            hint,
            &mut state,
            ood.as_ref().map(|claim| claim.value.as_claim()),
        )
        .map_err(|e| ProtocolError::LigeritoConfig(format!("split shared opening: {e:?}")))?;
        trace("shared opening", started);
        Ok(SplitProof {
            sha: sha_piop,
            glue: glue_piop,
            bitz: BitZOpeningProof { transcript: state.finish(), ood: ood.map(|claim| claim.round) },
        })
    }

    pub fn verify<T: Transcript + Send>(
        &self,
        transcript: &mut T,
        commitment: &Commitment,
        proof: &SplitProof,
    ) -> Result<(), ProtocolError> {
        let vc = self.glue.ligerito().verifier();
        shared::check_verifier(&self.sha, commitment, &proof.sha, vc)?;
        absorb_spartan_message(transcript, b"protocol", PROTOCOL);
        let binding = shared::bind_statement(transcript, &self.sha, commitment, vc)?;
        self.glue.bind_statement(transcript, commitment)?;
        let packed_vars = self.layout.row_vars + self.layout.col_vars - LOG_PACKING;
        let ood = bind_verifier_ood(transcript, packed_vars, self.glue.security().ood, proof.bitz.ood.as_ref())
            .map_err(ProtocolError::Bitz)?;
        let sha_claim = shared::verify_piop(transcript, &self.sha, &proof.sha, binding)?;
        let glue_claim = self.glue.verify_piop(transcript, &proof.glue)?;

        let ood = ood.opening_claim(transcript, packed_vars, proof.bitz.ood.as_ref()).map_err(ProtocolError::Bitz)?;
        let state = build_verifier(SESSION, &fork_tag(transcript), &proof.bitz.transcript);
        let (map, opened) = shared::block_claim_geometry(&self.sha)?;
        let committed = opening_shape(&self.layout)?;
        let sha_params = opening_params(opened, sha_claim.prime.modulus_u128())?;
        let sha_native = LinearClaim::new(&sha_params, sha_claim.rows, sha_claim.cols, sha_claim.claimed).map_err(claim_error)?;
        let block = BlockStatement::new(sha_params, committed, map, &sha_native).map_err(|e| statement_error(format!("{e:?}")))?;
        let glue_params = opening_params(self.glue.derived_layout(), glue_claim.prime.modulus_u128())?;
        let (rows, cols, target) = glue_claim.claim;
        let glue_native = LinearClaim::new(&glue_params, rows, cols, target).map_err(claim_error)?;
        let glue = VirtualStatement::new(glue_params, committed, self.glue.map(), &glue_native)
            .map_err(|e| statement_error(format!("{e:?}")))?;
        crate::bitz::shared::verify_shared(
            &block,
            &glue,
            &self.pcs(false)?,
            Root(commitment.root),
            state,
            ood.as_ref().map(|claim| claim.value.as_claim()),
        )
        .map_err(|e| ProtocolError::LigeritoConfig(format!("split shared opening: {e:?}")))
    }
}

fn claim_error(e: crate::bitz::params::ClaimError) -> ProtocolError {
    ProtocolError::LigeritoConfig(format!("split claim: {e:?}"))
}

fn statement_error(e: String) -> ProtocolError {
    ProtocolError::LigeritoConfig(format!("split statement: {e}"))
}

fn trace(label: &str, started: std::time::Instant) {
    if std::env::var_os("BITZ_TRACE").is_some() {
        eprintln!("split: {label:<24} {:>9.1?}", started.elapsed());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::piop::spartan::profile::{Lambda100, Lambda128};
    use circuit::compact::CompactRecorder;
    use circuit::linear_map::binary::VirtualMap;
    use circuit::sha256::{self, Word};
    use circuit::witgen::CheckingWitgen;
    use circuit::{BoolRepresentation, Circuit, HintResult, PackedBits};
    use field::Gf128 as Gf;

    /// A chain of `n` compression blocks over committed message words, with
    /// tail cells and rows reading block outputs (each block's low output
    /// word lifted and checked against a committed copy), as FlockRec's test
    /// circuit.
    fn chain<CS: Circuit>(cs: &mut CS, n: usize) {
        let mut state = sha256::initial_state::<CS::Bool>();
        for k in 0..n {
            let words = cs.hint::<1, 512, 8, _>(move |_| HintResult::Ok(PackedBits::from_fn(|i| (i * 7 + i / 3 + k) % 5 < 2)));
            let block: [Word<CS::Bool, 32>; 16] = std::array::from_fn(|w| Word::new(std::array::from_fn(|b| words.bit(32 * w + b))));
            state = sha256::compress_block(cs, &block, &state);
            let copy = {
                let bits: Vec<CS::Bool> = (0..32).map(|b| state[0].bit(b)).collect();
                let bits2 = bits.clone();
                let packed = cs.hint::<1, 32, 1, _>(move |ctx| HintResult::Ok(PackedBits::from_fn(|b| ctx.eval_bool(&bits2[b]))));
                (0..32).map(|b| packed.bit(b)).collect::<Vec<_>>()
            };
            let mut diff = CS::Z::<1>::from(CS::Coefficient::<1>::from(0));
            let mut power = CS::Coefficient::<1>::from(1);
            for b in 0..32 {
                let x = cs.bitz::<1>(state[0].bit(b));
                let y = cs.bitz::<1>(copy[b].clone());
                diff += (x - y) * power.clone();
                power += power.clone();
            }
            let zero = || CS::Z::<1>::from(CS::Coefficient::<1>::from(0));
            cs.assert_r1c::<1>(zero(), zero(), diff);
        }
    }

    fn recorded(n: usize) -> (StructuredCircuit, PackedWitness, PackedWitness) {
        let mut w = CheckingWitgen::new();
        chain(&mut w, n);
        assert_eq!(w.first_failure, None);
        let mut rec = CompactRecorder::new();
        chain(&mut rec, n);
        let (g, h) = w.into_witnesses();
        (rec.finish_structured(), g, h)
    }

    /// The stub circuit holds on the glue witness: `h' = M'·g'` cell by
    /// cell (every binding cell zero) and every row over the integers.
    #[test]
    fn stub_circuit_holds_on_the_glue_witness() {
        let (st, g, h) = recorded(37);
        let glue = Glue::new(&st).unwrap();
        assert_eq!(glue.circuit.instances.len(), 37);
        let (g2, h2, inputs) = glue.witness(&st, &g, &h).unwrap();
        glue.circuit.flatten().check_witness(&g2, &h2).expect("the stub circuit holds");
        // The first compression reads the initial state.
        assert_eq!(inputs[0].0, [0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19]);
        // A wrong output copy breaks its binding cell.
        let at = STUB_HINTS * 3 + INPUTS + OUTPUTS + 5;
        let mut words = g2.words().to_vec();
        words[at / 64] ^= 1 << (at % 64);
        let bad = PackedWitness::from_words(words, g2.bit_len());
        assert!(glue.circuit.flatten().check_witness(&bad, &h2).is_err());
    }

    fn split<P: IopSecurityProfile>(n: usize, lig: &str, glue_rows: usize, sha_rows: Option<usize>) -> (SplitRelation, SplitWitness) {
        let (st, g, h) = recorded(n);
        let selection = LigeritoSelection::parse(lig, P::LIGERITO_TARGET_BITS).unwrap();
        let rel = SplitRelation::new_at::<P>(&st, [9; 32], selection, glue_rows, sha_rows).unwrap();
        let (wit, _) = rel.witness(&st, &g, &h).unwrap();
        (rel, wit)
    }

    /// The stacked map on the shared grid: it derives the glue's rows from
    /// the shared rows (the batch's cells included), and its fast
    /// transposition, structured weights and lazy extension are the generic
    /// column-by-column transposition's, for one and four compressions per
    /// committed column.
    #[test]
    fn stacked_map_matches_the_generic_transpose() {
        for sha_rows in [None, Some(16)] {
            let (rel, wit) = split::<Lambda100>(100, "custom:3:4", 14, sha_rows);
            let glue = rel.glue();
            let map = glue.map();
            assert_eq!(glue.derive_rows(&wit.glue.f_rows), wit.glue.h_rows, "{sha_rows:?}: h = M·f on the shared grid");
            let mut state = 0x2468_ace0_1357_9bdfu64;
            let mut gf = || {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                Gf::new(state, state.rotate_left(29) ^ 0x5555)
            };
            let derived = glue.derived_layout();
            let row_w: Vec<Gf> = (0..derived.rows()).map(|_| gf()).collect();
            let col_w: Vec<Gf> = (0..derived.cols()).map(|_| gf()).collect();
            let cells = map.cols();
            let fast = map.transpose_weights(&row_w, &col_w, derived.row_vars, cells).unwrap();
            // A sample of every kind of committed cell: the batch's rows of
            // the first, middle and last stubs (inputs, outputs), the boxes,
            // a dummy block, and random cells.
            let committed = rel.committed_layout();
            let a = 13;
            let mut sample: Vec<usize> = Vec::new();
            for i in [0usize, 1, 50, 99, 100, 127] {
                let base = i << a;
                sample.extend((0..24).map(|l| base + l));
                sample.extend((6870..6900).map(|l| base + l));
                sample.extend((7600..7700).map(|l| base + l));
                sample.extend((8100..8192).map(|l| base + l));
            }
            sample.extend((0..400).map(|_| gf().lo as usize % cells));
            for j in sample {
                let mut slow = Gf::ZERO;
                for d in map.column_rows(j).unwrap() {
                    if d < row_w.len() * col_w.len() {
                        slow += row_w[d & (row_w.len() - 1)] * col_w[d >> derived.row_vars];
                    }
                }
                assert_eq!(fast[j], slow, "{sha_rows:?}: transposed weight {j}");
            }
            let sw = map.structured_weights(&row_w, &col_w, derived.row_vars).expect("structured weights");
            assert_eq!((sw.t, sw.s), (committed.row_vars, committed.col_vars));
            assert!(sw.term2.is_empty(), "the second term is the batch's");
            assert!(sw.dense() == fast, "{sha_rows:?}: structured weights");
            let point: Vec<Gf> = (0..cells.ilog2()).map(|_| gf()).collect();
            let eq = crate::bitz::gkr::eq_table(&point);
            let dense = fast.iter().zip(&eq).fold(Gf::ZERO, |acc, (w, e)| acc + *w * *e);
            assert_eq!(map.transposed_mle(&row_w, &col_w, derived.row_vars, &point), Some(dense), "{sha_rows:?}: lazy extension");
        }
    }

    fn prove_and_tamper(rel: &SplitRelation, wit: &SplitWitness) {
        let hint = rel.commit(wit);
        let mut tp = crate::transcript::Sha256Transcript::new();
        let pi = rel.prove(&mut tp, wit, &hint).unwrap();
        let mut tv = crate::transcript::Sha256Transcript::new();
        rel.verify(&mut tv, &hint.commitment, &pi).expect("the split proof verifies");
        let rejects = |bad: &SplitProof, what: &str| {
            let mut tv = crate::transcript::Sha256Transcript::new();
            assert!(rel.verify(&mut tv, &hint.commitment, bad).is_err(), "tampered {what}");
        };
        for hints in [false, true] {
            let mut bad = pi.clone();
            let bytes = if hints { &mut bad.bitz.transcript.hints } else { &mut bad.bitz.transcript.narg_string };
            let at = bytes.len() / 2;
            bytes[at] ^= 0x10;
            rejects(&bad, if hints { "opening hint" } else { "opening narg" });
        }
        let mut bad = pi.clone();
        bad.glue.inner.round_polynomials[3][1] = bad.glue.inner.round_polynomials[3][0];
        rejects(&bad, "glue inner sumcheck");
        let mut bad = pi.clone();
        if let Some(product) = bad.sha.product.as_mut() {
            product.terminal[0] = product.terminal[1];
        }
        rejects(&bad, "batch product sumcheck");
        if let Some(round) = pi.bitz.ood.clone() {
            let mut bad = pi.clone();
            let mut round = round;
            match round.y_ext.as_mut() {
                Some(y) => *y += Gf::ONE,
                None => round.y += Gf::ONE,
            }
            bad.bitz.ood = Some(round);
            rejects(&bad, "Round-0 value");
        }
    }

    /// The pair proves and verifies at 100 bits (Johnson over GF(2^128)) and
    /// 128 bits (Johnson over GF(2^256)), four compressions per column; a
    /// tampered opening, PIOP message or Round-0 value is rejected.
    #[test]
    fn split_proves_and_verifies() {
        let (rel, wit) = split::<Lambda100>(100, "custom:3:4", 14, Some(16));
        prove_and_tamper(&rel, &wit);
        let (rel, wit) = split::<Lambda128>(100, "ext:3:4", 14, Some(16));
        assert!(rel.glue().ligerito().extension());
        prove_and_tamper(&rel, &wit);
    }
}
