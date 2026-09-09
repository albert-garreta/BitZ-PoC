use super::{BABY_P, Corpus, Timing, TraceCapture, Workload, captured};
#[cfg(test)]
#[allow(unused_imports)] // `cargo bench` sets cfg(test) without running the #[test] user.
use super::Operands;
use limber::{
    imod_r1cs_modp::{IntModR1CSShapeModp, IntModR1CSWitnessModp},
    imod_spartan_modp::{
        IntModSpartanModpProverKey, IntModSpartanModpSNARK, IntModSpartanModpVerifierKey,
    },
    provider::{T256DynPrimeEngine, pcs::integer_modpcs::IntEvalParams},
    traits::mod_engine::{ModEngine, SumcheckEngine, SumcheckField},
};
use num_bigint::BigUint;
use serde_json::{Value, json};
use std::sync::Arc;
type E = T256DynPrimeEngine;

#[derive(Clone, Copy)]
enum Column {
    Witness(usize),
    One,
}
type Lc = Vec<(Column, u64)>;
struct Row {
    a: Lc,
    b: Lc,
    c: Lc,
    modulus: u64,
}
enum Assignment {
    Input(usize, usize),
    Output(usize),
    Bit(usize, usize),
    And(usize, usize),
}
/// The bit-decomposed program, used by BabyBear only.
///
/// BabyBear needs `a < p`, which is strictly stronger than the `a < 2^31` an
/// IntEval limb bound can give, so its operands are still decomposed into bit
/// columns and recomposed, with the canonical-form check on top. The integer
/// workloads use [`Wrapping`] instead.
struct BitProgram {
    assignments: Vec<Assignment>,
    rows: Vec<Row>,
    num_vars: usize,
    num_cons: usize,
}
impl BitProgram {
    fn compile(corpus: &Corpus) -> Self {
        let mut p = Self {
            assignments: vec![],
            rows: vec![],
            num_vars: 0,
            num_cons: 0,
        };
        assert_eq!(
            corpus.workload,
            Workload::BabyBear,
            "the bit-decomposed program is BabyBear-only"
        );
        for gate in 0..corpus.len() {
            let a = p.alloc(Assignment::Input(gate, 0));
            let b = p.alloc(Assignment::Input(gate, 1));
            let c = p.alloc(Assignment::Output(gate));
            p.rows.push(Row {
                a: term(a),
                b: term(b),
                c: term(c),
                modulus: BABY_P,
            });
            p.range(a, 31, true);
            p.range(b, 31, true);
            p.range(c, 31, true);
        }
        p.num_vars = p.assignments.len().next_power_of_two();
        p.num_cons = p.rows.len().next_power_of_two();
        p
    }
    fn alloc(&mut self, value: Assignment) -> usize {
        let i = self.assignments.len();
        self.assignments.push(value);
        i
    }
    fn range(&mut self, column: usize, bits: usize, canonical_babybear: bool) {
        let indices: Vec<_> = (0..bits)
            .map(|bit| {
                let i = self.alloc(Assignment::Bit(column, bit));
                self.rows.push(Row {
                    a: term(i),
                    b: term(i),
                    c: term(i),
                    modulus: 0,
                });
                i
            })
            .collect();
        self.rows.push(Row {
            a: indices
                .iter()
                .enumerate()
                .map(|(bit, &i)| (Column::Witness(i), 1u64 << bit))
                .collect(),
            b: one(),
            c: term(column),
            modulus: 0,
        });
        if canonical_babybear {
            // p-1 = 15 * 2^27. If the four high bits are all one, the low
            // 27 bits must be zero. All other 31-bit values are below p.
            let mut high = indices[27];
            for &bit in &indices[28..31] {
                let next = self.alloc(Assignment::And(high, bit));
                self.rows.push(Row {
                    a: term(high),
                    b: term(bit),
                    c: term(next),
                    modulus: 0,
                });
                high = next;
            }
            self.rows.push(Row {
                a: term(high),
                b: indices[..27]
                    .iter()
                    .enumerate()
                    .map(|(bit, &i)| (Column::Witness(i), 1u64 << bit))
                    .collect(),
                c: vec![],
                modulus: 0,
            });
        }
    }
    fn shape(&self) -> IntModR1CSShapeModp<E> {
        let entries = |select: fn(&Row) -> &Lc| {
            self.rows
                .iter()
                .enumerate()
                .flat_map(|(row, r)| {
                    select(r).iter().map(move |&(col, k)| {
                        (
                            row,
                            match col {
                                Column::Witness(i) => i,
                                Column::One => self.num_vars,
                            },
                            BigUint::from(k),
                        )
                    })
                })
                .collect()
        };
        let mut moduli: Vec<_> = self.rows.iter().map(|r| BigUint::from(r.modulus)).collect();
        moduli.resize(self.num_cons, BigUint::from(0u32));
        IntModR1CSShapeModp::new(
            self.num_cons,
            self.num_vars,
            0,
            entries(|r| &r.a),
            entries(|r| &r.b),
            entries(|r| &r.c),
            moduli,
        )
        .expect("Limber multiplication shape")
    }
    fn values(&self, corpus: &Corpus) -> Vec<u64> {
        let mut values = Vec::with_capacity(self.num_vars);
        for a in &self.assignments {
            let value = match *a {
                Assignment::Input(i, operand) => {
                    let (a, b) = corpus.inputs()[i];
                    if operand == 0 { a } else { b }
                }
                Assignment::Output(i) => {
                    let (a, b) = corpus.inputs()[i];
                    u64::try_from(corpus.workload.output(a, b)).expect("Limber outputs fit u64")
                }
                Assignment::Bit(i, bit) => (values[i] >> bit) & 1,
                Assignment::And(a, b) => values[a] * values[b],
            };
            values.push(value);
        }
        values.resize(self.num_vars, 0);
        values
    }
    fn quotients(&self, values: &[u64]) -> Result<Vec<BigUint>, String> {
        let eval = |lc: &Lc| {
            lc.iter()
                .map(|&(col, k)| {
                    k as u128
                        * match col {
                            Column::One => 1,
                            Column::Witness(i) => values[i] as u128,
                        }
                })
                .sum::<u128>()
        };
        let mut q = Vec::with_capacity(self.num_cons);
        for r in &self.rows {
            let residual = (eval(&r.a) * eval(&r.b))
                .checked_sub(eval(&r.c))
                .ok_or("negative residual")?;
            if r.modulus == 0 {
                if residual != 0 {
                    return Err("invalid exact row".into());
                }
                q.push(BigUint::from(0u32));
            } else {
                if residual % r.modulus as u128 != 0 {
                    return Err("invalid modular row".into());
                }
                q.push(BigUint::from(residual / r.modulus as u128));
            }
        }
        q.resize(self.num_cons, BigUint::from(0u32));
        Ok(q)
    }
}
fn term(i: usize) -> Lc {
    vec![(Column::Witness(i), 1)]
}
fn one() -> Lc {
    vec![(Column::One, 1)]
}

/// The wrapping-multiplication program of Limber's own
/// `examples/int_mult.rs`: one modular row `x * y = z_lo (mod 2^w)` per gate,
/// whose quotient is the high half of the exact `2w`-bit product.
///
/// Every committed value — both operands, the low half and the quotient — is
/// then below `2^w`, so the IntEval limb range check that the Mod-PCS already
/// runs *is* the operand range check and the program needs no bit columns of
/// its own. The earlier program spent `2w` bit columns and two recomposition
/// rows per gate to bound operands that a `log_t_f = 2w` key could not pin,
/// which cost 67 constraints per 32-bit multiplication instead of one.
///
/// Variables are laid out `[x_0..x_{g-1}, y_0..y_{g-1}, z_0..z_{g-1}]`; unlike
/// `int_mult` the gates are independent (it chains `a_{i+1} = c_i`, which needs
/// only one fresh operand per gate), because the corpus is independent operand
/// pairs and the other backends prove independent gates.
struct Wrapping {
    /// Operand width `w`; the row modulus is `2^w`.
    bits: u32,
    gates: usize,
    num_vars: usize,
    num_cons: usize,
}
impl Wrapping {
    fn compile(corpus: &Corpus) -> Self {
        let bits = match corpus.workload {
            Workload::U32 => 32,
            Workload::U64 => 64,
            Workload::U128 => 128,
            Workload::BabyBear => unreachable!("BabyBear uses the bit-decomposed program"),
        };
        let gates = corpus.len();
        Self {
            bits,
            gates,
            num_vars: (3 * gates).next_power_of_two(),
            num_cons: gates.next_power_of_two(),
        }
    }
    fn modulus(&self) -> BigUint {
        BigUint::from(1u32) << self.bits
    }
    fn shape(&self) -> IntModR1CSShapeModp<E> {
        let one = BigUint::from(1u32);
        let column = |base: usize| -> Vec<(usize, usize, BigUint)> {
            (0..self.gates)
                .map(|i| (i, base + i, one.clone()))
                .collect()
        };
        IntModR1CSShapeModp::new(
            self.num_cons,
            self.num_vars,
            0,
            column(0),
            column(self.gates),
            column(2 * self.gates),
            // Padding rows keep the modulus: their empty combinations make
            // `0 * 0 = 0 (mod 2^w)` with quotient zero.
            vec![self.modulus(); self.num_cons],
        )
        .expect("Limber wrapping-multiplication shape")
    }
    fn mask(&self) -> u128 {
        if self.bits == 128 {
            u128::MAX
        } else {
            (1u128 << self.bits) - 1
        }
    }
    fn operands(&self, corpus: &Corpus, gate: usize) -> (u128, u128) {
        if corpus.workload == Workload::U128 {
            let (x, y) = corpus.wide_inputs()[gate];
            (x, y)
        } else {
            let (x, y) = corpus.inputs()[gate];
            (u128::from(x), u128::from(y))
        }
    }
    fn values(&self, corpus: &Corpus) -> Vec<u128> {
        let mut values = vec![0u128; self.num_vars];
        for gate in 0..self.gates {
            let (x, y) = self.operands(corpus, gate);
            values[gate] = x;
            values[self.gates + gate] = y;
            values[2 * self.gates + gate] = self.split(x, y).0;
        }
        values
    }
    /// The exact product of two in-range operands as `(low w bits, high w)`.
    fn split(&self, x: u128, y: u128) -> (u128, u128) {
        if self.bits == 128 {
            f2z::piop::spartan::mul_u128_full(x, y)
        } else {
            // Both operands are below `2^64` here, so the product fits `u128`.
            let product = x * y;
            (product & self.mask(), product >> self.bits)
        }
    }
    /// The native rows `[x, y, z_lo, z_hi]` recovered from the committed
    /// values alone, re-deriving each product so that a wrong output or an
    /// out-of-range operand is rejected here rather than inside the prover.
    fn native_rows(&self, values: &[u128]) -> Result<Vec<[u128; 4]>, String> {
        let mut rows = Vec::with_capacity(self.gates);
        for gate in 0..self.gates {
            let (x, y) = (values[gate], values[self.gates + gate]);
            if self.bits < 128 && (x >> self.bits != 0 || y >> self.bits != 0) {
                return Err(format!(
                    "operand of gate {gate} is not a {}-bit value",
                    self.bits
                ));
            }
            let (lo, hi) = self.split(x, y);
            if values[2 * self.gates + gate] != lo {
                return Err(format!("gate {gate} has the wrong low half"));
            }
            rows.push([x, y, lo, hi]);
        }
        Ok(rows)
    }
    fn quotients(&self, values: &[u128]) -> Result<Vec<BigUint>, String> {
        let mut q = vec![BigUint::from(0u32); self.num_cons];
        for (gate, row) in self.native_rows(values)?.iter().enumerate() {
            q[gate] = BigUint::from(row[3]);
        }
        Ok(q)
    }
}

/// `IntEvalParams` per-iteration variable count; `limber`'s `DEFAULT_K`.
const K: usize = 9;

/// The Mod-R1CS program of one corpus.
enum Program {
    /// The 32-, 64- and 128-bit integer workloads.
    Wrapping(Wrapping),
    /// BabyBear, which still needs explicit bit columns for canonicity.
    Bits(BitProgram),
}
impl Program {
    fn compile(corpus: &Corpus) -> Self {
        match corpus.workload {
            Workload::BabyBear => Self::Bits(BitProgram::compile(corpus)),
            Workload::U32 | Workload::U64 | Workload::U128 => {
                Self::Wrapping(Wrapping::compile(corpus))
            }
        }
    }
    fn num_vars(&self) -> usize {
        match self {
            Self::Wrapping(w) => w.num_vars,
            Self::Bits(b) => b.num_vars,
        }
    }
    fn num_cons(&self) -> usize {
        match self {
            Self::Wrapping(w) => w.num_cons,
            Self::Bits(b) => b.num_cons,
        }
    }
    /// Live (pre-padding) constraint and variable counts, for the run config.
    fn live(&self) -> (usize, usize) {
        match self {
            Self::Wrapping(w) => (w.gates, 3 * w.gates),
            Self::Bits(b) => (b.rows.len(), b.assignments.len()),
        }
    }
    fn shape(&self) -> IntModR1CSShapeModp<E> {
        match self {
            Self::Wrapping(w) => w.shape(),
            Self::Bits(b) => b.shape(),
        }
    }
    /// The IntEval bound each committed value is proven to respect.
    ///
    /// The wrapping program keeps every value below `2^w`, so `w` is the whole
    /// witness bound and the limb range check the Mod-PCS already runs doubles
    /// as the operand range check. Widths up to 64 bits take a single limb, as
    /// in `int_mult`; 128-bit values are split into 32-bit limbs, since a
    /// single-limb 128-bit bound would need roughly ninety CRT primes.
    fn params(&self, arity: usize) -> IntEvalParams {
        match self {
            Self::Wrapping(w) if w.bits <= 64 => {
                IntEvalParams::derive_no_limb_split(w.bits as usize, K, arity)
            }
            Self::Wrapping(w) => IntEvalParams::derive(w.bits as usize, 32, K, arity),
            // BabyBear commits the 62-bit product `a * b` before reduction.
            Self::Bits(_) => IntEvalParams::derive(64, 32, K, arity),
        }
        .expect("Limber IntEval parameters")
    }
    /// The committed witness and the row quotients.
    fn witness(&self, corpus: &Corpus) -> Result<(Vec<BigUint>, Vec<BigUint>), String> {
        match self {
            Self::Wrapping(w) => {
                let values = w.values(corpus);
                let q = w.quotients(&values)?;
                Ok((values.into_iter().map(BigUint::from).collect(), q))
            }
            Self::Bits(b) => {
                let values = b.values(corpus);
                let q = b.quotients(&values)?;
                Ok((values.into_iter().map(BigUint::from).collect(), q))
            }
        }
    }
}

pub(super) struct Context {
    corpus: Arc<Corpus>,
    program: Program,
    shape: IntModR1CSShapeModp<E>,
    params: IntEvalParams,
    pk: IntModSpartanModpProverKey<E>,
    vk: IntModSpartanModpVerifierKey<E>,
}
impl Context {
    pub(super) fn setup(corpus: Arc<Corpus>) -> Self {
        let program = Program::compile(&corpus);
        let shape = program.shape();
        let arity = program.num_cons().max(program.num_vars()).ilog2() as usize;
        let params = program.params(arity);
        let (pk, vk) = IntModSpartanModpSNARK::<E>::setup_with_params(shape.clone(), params.clone())
            .expect("Limber setup");
        Self {
            corpus,
            program,
            shape,
            params,
            pk,
            vk,
        }
    }
    pub(super) fn config(&self) -> Value {
        let (constraints, variables) = self.program.live();
        let encoding = match self.program {
            Program::Wrapping(ref w) => format!(
                "one modular row x*y = z_lo (mod 2^{}) per gate, quotient = high half",
                w.bits
            ),
            Program::Bits(_) => "bit-decomposed operands with the canonical-form check".into(),
        };
        json!({"piop":"Integer-Mod Spartan","pcs":"Limber IntEval/Hyrax","encoding":encoding,
               "log_t_f":self.params.log_t_f,"log_t":self.params.log_t,"k":self.params.k,
               "log_p":self.params.log_p,"s":self.params.s,"numlimb":self.params.numlimb,
               "constraints":constraints,"padded_constraints":self.program.num_cons(),
               "variables":variables,"padded_variables":self.program.num_vars(),
               "security_policy":"native Limber parameters; not asserted equal to F2Z or WHIR"})
    }
    pub(super) fn run(&self, capture: &TraceCapture) -> Timing {
        capture.begin();
        let start = capture.now_ns();
        let (w, q) = self
            .program
            .witness(&self.corpus)
            .expect("Limber witness satisfies exact and modular constraints");
        let wend = capture.now_ns();
        let (witness, instance) =
            IntModR1CSWitnessModp::<E>::new(&self.shape, self.pk.ck(), w, q, vec![])
                .expect("Limber witness commitment");
        let proof = IntModSpartanModpSNARK::<E>::prove(&self.pk, &instance, &witness)
            .expect("Limber full proof");
        let ready = capture.now_ns();
        let vstart = capture.now_ns();
        proof
            .verify(&self.vk, &instance)
            .expect("Limber full verification");
        let end = capture.now_ns();
        let raw = capture.finish();
        // The pinned Limber driver has no whole-proof serializer. Its
        // unsegmented PIOP sends three coefficients per outer round, two per
        // inner round, five outer evaluations and eval_w. Round counts are
        // fixed by the public padded shape; no length prefixes or sampled
        // modulus need be sent. Count these payload bytes separately from the
        // actually serialized commitments and batch opening, including both W/Q.
        let scalar_bytes = <E as SumcheckEngine>::Scalar::zero(&E::bootstrap_params())
            .to_le_bytes()
            .len();
        let piop_bytes = scalar_bytes
            * (3 * self.program.num_cons().ilog2() as usize
                + 2 * (self.program.num_vars().ilog2() as usize + 1)
                + 6);
        let proof_bytes = instance
            .commitment_bytes()
            .expect("serialize Limber commitments")
            .len()
            + piop_bytes
            + proof
                .eval_arg_bytes()
                .expect("serialize Limber opening")
                .len();
        let commit = captured(&raw, "imod_modp_wq_commit", wend, ready);
        let piop = captured(&raw, "imod_modp_piop", commit.end_ns, ready);
        let opening = captured(&raw, "imod_modp_wq_open", commit.end_ns, ready);
        let mut t = Timing::new(start, wend, ready, vstart, end, proof_bytes);
        t.add("commit", "commit", commit.start_ns, commit.end_ns);
        t.add(
            "prime_projection",
            "preparation",
            commit.end_ns,
            piop.start_ns,
        );
        t.add("piop", "constraint-proof", piop.start_ns, piop.end_ns);
        t.add("opening", "opening-proof", opening.start_ns, opening.end_ns);
        std::hint::black_box((proof, witness, instance));
        t
    }
}

#[cfg(test)]
#[allow(unused_imports)]
mod tests {
    use super::*;
    #[test]
    fn wrapping_rows_reject_bad_outputs_and_ranges() {
        for workload in [Workload::U32, Workload::U64, Workload::U128] {
            let corpus = Corpus::new(workload, 4, 7);
            let shape = Wrapping::compile(&corpus);
            let values = shape.values(&corpus);
            let rows = shape.native_rows(&values).expect("honest witness");
            assert_eq!(rows.len(), corpus.len());
            // A wrong low half is caught by re-deriving the product.
            let mut wrong = values.clone();
            wrong[2 * shape.gates] ^= 1;
            assert!(shape.native_rows(&wrong).is_err());
            // So is an operand wider than the row modulus.
            if shape.bits < 128 {
                let mut wide = values.clone();
                wide[0] = 1u128 << shape.bits;
                assert!(shape.native_rows(&wide).is_err());
            }
        }
    }
    #[test]
    fn babybear_rows_reject_bad_outputs_and_ranges() {
        let corpus = Corpus::new(Workload::BabyBear, 4, 7);
        let p = BitProgram::compile(&corpus);
        let mut values = p.values(&corpus);
        p.quotients(&values).unwrap();
        values[2] ^= 1;
        assert!(p.quotients(&values).is_err());
        // An out-of-range operand has no canonical witness, so the corpus is
        // assembled directly (its digest is unused here).
        let mut inputs = corpus.inputs().to_vec();
        inputs[0] = (BABY_P, 0);
        let corpus = Corpus {
            workload: Workload::BabyBear,
            operands: Operands::Narrow(inputs),
            digest: corpus.digest.clone(),
        };
        assert!(p.quotients(&p.values(&corpus)).is_err());
    }
}

pub(super) fn audit(corpus: &Corpus) -> super::WitnessAudit {
    let program = Program::compile(corpus);
    let started = std::time::Instant::now();
    match &program {
        Program::Wrapping(shape) => {
            let values = shape.values(corpus);
            let rows = shape.native_rows(&values).expect("integer constraints");
            let generation_ms = started.elapsed().as_secs_f64() * 1e3;
            let representation = "Limber wrapping rows and quotients";
            if corpus.workload == Workload::U128 {
                return super::WitnessAudit::check_wide(corpus, rows, generation_ms, representation);
            }
            // The canonical `u32` row carries the whole `64`-bit product in one
            // entry; the `u64` row keeps the two halves the program commits.
            let narrow = rows
                .iter()
                .map(|&[x, y, lo, hi]| {
                    let cell = |v: u128| u64::try_from(v).expect("native value fits u64");
                    match corpus.workload {
                        Workload::U32 => [cell(x), cell(y), cell(lo | (hi << 32)), 0],
                        _ => [cell(x), cell(y), cell(lo), cell(hi)],
                    }
                })
                .collect();
            super::WitnessAudit::check(corpus, narrow, generation_ms, representation, false)
        }
        Program::Bits(bits) => {
            let values = bits.values(corpus);
            let q = bits.quotients(&values).expect("integer constraints");
            let generation_ms = started.elapsed().as_secs_f64() * 1e3;
            let mut rows = vec![[0u64; 4]; corpus.len()];
            for (index, assignment) in bits.assignments.iter().enumerate() {
                match *assignment {
                    Assignment::Input(gate, operand) => rows[gate][operand] = values[index],
                    Assignment::Output(gate) => rows[gate][2] = values[index],
                    _ => {}
                }
            }
            // Modular multiplication is the only nonzero-modulus row kind, in gate order.
            for (gate, (_, q)) in bits
                .rows
                .iter()
                .zip(q)
                .filter(|(row, _)| row.modulus != 0)
                .enumerate()
            {
                rows[gate][3] = u64::try_from(q).expect("native quotient fits canonical u64");
            }
            super::WitnessAudit::check(
                corpus,
                rows,
                generation_ms,
                "Limber integer assignment and quotient",
                false,
            )
        }
    }
}
