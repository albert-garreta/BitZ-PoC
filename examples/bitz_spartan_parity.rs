//! Stage A parity harness: re-proves a `dump_spartan` dump (the oracle's
//! `tooling/cli/examples/dump_spartan.rs`) through `f2z::bitz::spartan` on
//! the same transcript prefix, and diffs the out-of-band proof bytes
//! (`spartan.bin`), the terminal claim, the next squeezed challenge, and —
//! for the e2e kind — the `opening_claim` on `h`. Both proofs are verified.
//!
//! `bitz_spartan_parity <dump-dir>`; `BITZ_REPEAT=k` proves `k` times.
//!
//! The session and instance are byte strings on their side (`b"..."`), so
//! they enter the domain separator raw; a `&str` would be length-prefixed.
use std::collections::HashMap;
use std::time::Instant;

use circuit::Circuit;
use circuit::constraints::ConstraintGenerator;
use circuit::matrix_transpose::MTransposeGenerator;
use circuit::sha256::{COMPRESSION_INPUT_BITS, INITIAL_STATE, Word, compress, compression_circuit};
use circuit::witgen::{PackedWitness, Witgen};
use f2z::bitz::fq::{Fq, Q};
use f2z::bitz::map::map_digest;
use f2z::bitz::spartan::{
    PreparedConstraintMatrices, ScaledMleEvaluationClaim, SpartanPiopProof, eq_table,
    prove_spartan_piop, verify_spartan_proof,
};
use f2z::bitz::transcript::PublicTranscript;
use f2z::bitz::{BitZParams, LinearClaim, Pcs, Shape, build_prover, build_verifier};
use f2z::poly::univariate::binary_gf128::BinaryFieldGF128 as Gf;
use flock_core::merkle::HashKind;
use spongefish::Encoding;

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len() / 2)
        .map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).expect("hex"))
        .collect()
}

fn describe(a: &[u8], b: &[u8]) -> String {
    if a == b {
        return "IDENTICAL".to_string();
    }
    let at = a.iter().zip(b).position(|(x, y)| x != y).unwrap_or(a.len().min(b.len()));
    format!("first mismatch at byte {at} (lengths {} vs {})", a.len(), b.len())
}

/// The e2e statement, their `Sha256Statement` on our vendored trait.
struct Statement {
    chain: bool,
    initial_state: [u32; 8],
    blocks: Vec<[u32; 16]>,
    digest: [u32; 8],
}

impl Statement {
    /// From `public_bytes()`: u64 block count, the initial state, the blocks,
    /// the digest, all u32 LE.
    fn from_public(chain: bool, public: &[u8]) -> Self {
        let count = u64::from_le_bytes(public[..8].try_into().unwrap()) as usize;
        let words: Vec<u32> = public[8..]
            .chunks_exact(4)
            .map(|c| u32::from_le_bytes(c.try_into().unwrap()))
            .collect();
        assert_eq!(words.len(), 8 + 16 * count + 8);
        Self {
            chain,
            initial_state: words[..8].try_into().unwrap(),
            blocks: (0..count)
                .map(|b| words[8 + 16 * b..8 + 16 * (b + 1)].try_into().unwrap())
                .collect(),
            digest: words[8 + 16 * count..].try_into().unwrap(),
        }
    }

    fn input(&self) -> Vec<bool> {
        self.blocks
            .iter()
            .flatten()
            .flat_map(|word| (0..32).map(move |bit| word >> bit & 1 != 0))
            .collect()
    }

    fn synthesize<CS: Circuit>(&self, cs: &mut CS, inputs: &[CS::Bool]) {
        if self.chain {
            assert_eq!(self.initial_state, INITIAL_STATE);
        } else {
            assert_eq!(self.blocks.len(), 1);
        }
        for (bit, expected) in inputs.iter().zip(self.input()) {
            constrain_bit(cs, bit.clone(), expected);
        }
        let mut state = self.initial_state.map(|word| Word::constant(u64::from(word)));
        for bits in inputs.chunks_exact(512) {
            let block = std::array::from_fn(|word| {
                Word::new(std::array::from_fn(|bit| bits[word * 32 + bit].clone()))
            });
            state = compress(cs, block, state).map(|value| value.word);
        }
        for (word, expected) in state.iter().zip(self.digest) {
            for bit in 0..32 {
                constrain_bit(cs, word.bit(bit), expected >> bit & 1 != 0);
            }
        }
    }
}

fn constrain_bit<CS: Circuit>(cs: &mut CS, bit: CS::Bool, expected: bool) {
    let value = cs.f2z::<1>(bit);
    let expected = CS::Z::<1>::from(CS::Coefficient::<1>::from(u64::from(expected)));
    cs.assert_r1c::<1>(
        CS::Z::<1>::from(CS::Coefficient::<1>::from(1u64)),
        value,
        expected,
    );
}

/// The two circuits the dump can carry, driven through any backend.
enum Fixture {
    Abc(Box<[bool; COMPRESSION_INPUT_BITS]>),
    E2e(Statement),
}

impl Fixture {
    fn inputs(&self) -> Vec<bool> {
        match self {
            Self::Abc(bits) => bits.to_vec(),
            Self::E2e(statement) => statement.input(),
        }
    }

    fn synthesize<CS: Circuit>(&self, cs: &mut CS, inputs: &[CS::Bool]) {
        match self {
            Self::Abc(_) => {
                let inputs: &[CS::Bool; COMPRESSION_INPUT_BITS] = inputs.try_into().unwrap();
                let _ = compression_circuit(cs, inputs);
            }
            Self::E2e(statement) => statement.synthesize(cs, inputs),
        }
    }
}

fn packed_bits(bits: &PackedWitness) -> Vec<u8> {
    let mut out = Vec::with_capacity(bits.bit_len().div_ceil(8));
    for (i, word) in bits.words().iter().enumerate() {
        let remaining = bits.bit_len() - 64 * i;
        out.extend_from_slice(&word.to_le_bytes()[..remaining.min(64).div_ceil(8)]);
    }
    out
}

fn fq_bytes(values: &[Fq]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_bytes()).collect()
}

/// `end_to_end.rs::opening_claim` on our types.
fn opening_claim(params: &BitZParams, terminal: &ScaledMleEvaluationClaim) -> LinearClaim {
    let shape = params.shape();
    assert!(terminal.point.len() <= shape.log_bits());
    let mut point = terminal.point.clone();
    point.resize(shape.log_bits(), Fq::ZERO);
    let rows = eq_table(&point[..shape.log_rows()])
        .into_iter()
        .map(|weight| (terminal.scale * weight).lift())
        .collect();
    let columns = eq_table(&point[shape.log_rows()..]).into_iter().map(Fq::lift).collect();
    LinearClaim::new(params, rows, columns, terminal.value.lift()).expect("claim dimensions")
}

fn claim_bytes(claim: &LinearClaim) -> Vec<u8> {
    let mut bytes = Vec::new();
    for weights in [claim.row_weights(), claim.column_weights()] {
        bytes.extend_from_slice(&(weights.len() as u64).to_le_bytes());
        for &w in weights {
            bytes.extend_from_slice(&w.to_le_bytes());
        }
    }
    bytes.extend_from_slice(&claim.target().to_le_bytes());
    bytes
}

fn main() {
    let dir = std::path::PathBuf::from(std::env::args().nth(1).expect("dump dir"));
    let meta: HashMap<String, String> = std::fs::read_to_string(dir.join("meta.txt"))
        .expect("meta.txt")
        .lines()
        .filter_map(|l| l.split_once('='))
        .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
        .collect();
    let read = |name: &str| std::fs::read(dir.join(name)).unwrap_or_else(|e| panic!("{name}: {e}"));
    let kind = meta["kind"].as_str();
    let session = meta["session"].clone();
    let instance = meta["instance"].clone();
    let inputs_bin = read("inputs.bin");
    let input_bits: usize = meta["input_bits"].parse().unwrap();
    let input_bools: Vec<bool> = (0..input_bits).map(|i| inputs_bin[i / 8] >> (i % 8) & 1 == 1).collect();

    let fixture = match kind {
        "abc" => Fixture::Abc(input_bools.clone().into_boxed_slice().try_into().unwrap()),
        "e2e" => {
            let chain = meta["circuit"] == "sha256-chain";
            let statement = Statement::from_public(chain, &read("public.bin"));
            assert_eq!(statement.input(), input_bools, "public.bin and inputs.bin disagree");
            Fixture::E2e(statement)
        }
        other => panic!("unknown dump kind {other}"),
    };
    let inputs = fixture.inputs();

    // Matrices, digest.
    let started = Instant::now();
    let mut generator = ConstraintGenerator::new(inputs.len());
    let symbolic: Vec<_> = (0..inputs.len()).map(|i| generator.input(i)).collect();
    fixture.synthesize(&mut generator, &symbolic);
    let integer = generator.into_matrices();
    let t_gen = started.elapsed();
    let started = Instant::now();
    let matrices = PreparedConstraintMatrices::from_vendored(&integer).expect("prepared");
    drop(integer);
    println!(
        "matrices: {} ({:.1} ms gen + {:.1} ms prepare); constraint digest {}",
        matrices.short_debug_info(),
        t_gen.as_secs_f64() * 1e3,
        started.elapsed().as_secs_f64() * 1e3,
        if hex(matrices.digest()) == meta["constraint_digest"] { "MATCH" } else { "MISMATCH" }
    );
    assert_eq!(matrices.num_row_vars().to_string(), meta["num_row_vars"]);
    assert_eq!(matrices.num_column_vars().to_string(), meta["num_column_vars"]);

    // Witness: f, h, the products, the assignment.
    let started = Instant::now();
    let mut witgen = Witgen::with_inputs(&inputs);
    fixture.synthesize(&mut witgen, &inputs);
    let (f, h) = witgen.into_witnesses();
    println!(
        "witness: f {} h {} ({:.1} ms); f.bin {}; h.bin {}",
        f.bit_len(),
        h.bit_len(),
        started.elapsed().as_secs_f64() * 1e3,
        describe(&packed_bits(&f), &read("f.bin")),
        describe(&packed_bits(&h), &read("h.bin"))
    );
    let products = matrices.products(&h).expect("products");
    assert!(products.satisfied(), "Az ∘ Bz = Cz");
    let mut ours_products = fq_bytes(&products.az);
    ours_products.extend(fq_bytes(&products.bz));
    ours_products.extend(fq_bytes(&products.cz));
    println!("products.bin {}", describe(&ours_products, &read("products.bin")));
    let assignment = matrices.assignment(&h).expect("assignment");

    // The transcript prefix.
    let e2e = match &fixture {
        Fixture::E2e(_) => {
            let claim_shape = Shape::new(meta["claim_t"].parse().unwrap(), meta["claim_s"].parse().unwrap()).unwrap();
            let committed_shape =
                Shape::new(meta["committed_t"].parse().unwrap(), meta["committed_s"].parse().unwrap()).unwrap();
            let generator = {
                let b = unhex(&meta["generator"]);
                Gf::from_words([
                    u64::from_le_bytes(b[..8].try_into().unwrap()),
                    u64::from_le_bytes(b[8..].try_into().unwrap()),
                ])
            };
            let q: u128 = meta["q"].parse().unwrap();
            assert_eq!(q, Q);
            let params = BitZParams::new(claim_shape, q, generator).expect("params");
            let pcs = Pcs::new(&committed_shape, HashKind::Blake3).expect("pcs");
            let root: [u8; 32] = unhex(&meta["root"]).try_into().unwrap();
            let started = Instant::now();
            let mut map_generator = MTransposeGenerator::new(inputs.len());
            let map_inputs = map_generator.take_inputs();
            fixture.synthesize(&mut map_generator, &map_inputs);
            let map = map_generator.finish();
            let digest = map_digest(&map);
            println!(
                "map: h_len {} f_len {} nnz {} ({:.1} ms); map digest {}",
                map.row_count(),
                map.column_count(),
                map.nonzero_count(),
                started.elapsed().as_secs_f64() * 1e3,
                if hex(&digest) == meta["map_digest"] { "MATCH" } else { "MISMATCH" }
            );
            Some((read("public.bin"), root, params, pcs, digest))
        }
        Fixture::Abc(_) => None,
    };
    let bind = |t: &mut dyn BindTarget| {
        if let Some((public, root, params, pcs, digest)) = &e2e {
            t.bind(public, root, params, pcs, digest);
        }
    };

    // Prove, compare.
    let repeats: usize = std::env::var("BITZ_REPEAT").ok().and_then(|r| r.parse().ok()).unwrap_or(1);
    let theirs = read("spartan.bin");
    let mut result = None;
    let mut times = Vec::new();
    for _ in 0..repeats {
        let mut prover = build_prover(session.as_bytes(), instance.as_bytes());
        bind(&mut prover);
        let started = Instant::now();
        let (proof, terminal) = prove_spartan_piop(&mut prover, &matrices, &products, &assignment).expect("prove");
        times.push(started.elapsed());
        let next: Gf = prover.verifier_message();
        let transcript_proof = prover.finish();
        assert!(transcript_proof.narg_string.is_empty());
        let bytes = proof.to_bytes(&terminal);
        if let Some((_, _, previous, _)) = &result {
            assert_eq!(previous, &bytes, "our prover is not deterministic");
        }
        result = Some((proof, terminal, bytes, next));
    }
    let (proof, terminal, bytes, next) = result.unwrap();
    times.sort();
    println!(
        "our prove: min {:.1?} median {:.1?} over {repeats}",
        times[0],
        times[times.len() / 2]
    );
    println!("spartan.bin: ours {} B theirs {} B {}", bytes.len(), theirs.len(), describe(&bytes, &theirs));
    std::fs::write(dir.join("ours.spartan.bin"), &bytes).unwrap();
    if bytes != theirs {
        // Per-coefficient diff, labelled.
        let (nr, nc) = (matrices.num_row_vars(), matrices.num_column_vars());
        let ours_v: Vec<u128> = bytes.chunks_exact(16).map(|c| u128::from_le_bytes(c.try_into().unwrap())).collect();
        let theirs_v: Vec<u128> = theirs.chunks_exact(16).map(|c| u128::from_le_bytes(c.try_into().unwrap())).collect();
        let label = |k: usize| -> String {
            if k < 4 * nr {
                format!("outer round {} c{}", k / 4, k % 4)
            } else if k < 4 * nr + 3 {
                format!("[Ah,Bh,Ch][{}]", k - 4 * nr)
            } else if k < 4 * nr + 3 + 3 * nc {
                let j = k - 4 * nr - 3;
                format!("inner round {} c{}", j / 3, j % 3)
            } else {
                let j = k - 4 * nr - 3 - 3 * nc;
                if j < nc { format!("point[{j}]") } else if j == nc { "scale".into() } else { "value".into() }
            }
        };
        let mut shown = 0;
        for (k, (o, t)) in ours_v.iter().zip(&theirs_v).enumerate() {
            if o != t && shown < 8 {
                println!("  differs: {} ours {:x} theirs {:x}", label(k), o, t);
                shown += 1;
            }
        }
    }
    let next_bytes = f2z::bitz::codec::gf_to_bytes(next);
    println!(
        "next challenge after the PIOP: {}",
        if hex(&next_bytes) == meta["next_challenge"] { "MATCH" } else { "MISMATCH" }
    );
    if let Some((_, _, params, _, _)) = &e2e {
        let claim = opening_claim(params, &terminal);
        println!("claim_h.bin {}", describe(&claim_bytes(&claim), &read("claim_h.bin")));
    }

    // Their challenge sequence (`tau ‖ r_x ‖ rho ‖ r_y`) and their bound table D,
    // against ours replayed on THEIR bytes through our verifier pieces.
    if dir.join("challenges.bin").exists() {
        let theirs_c = read("challenges.bin");
        let empty = f2z::bitz::Proof::default();
        let mut verifier = build_verifier(session.as_bytes(), instance.as_bytes(), &empty);
        bind(&mut verifier);
        verifier.public_message(matrices.digest());
        let tau: Vec<Fq> = (0..matrices.num_row_vars()).map(|_| verifier.squeeze_fq()).collect();
        let (their_proof, _) = SpartanPiopProof::from_bytes(&theirs, matrices.num_row_vars(), matrices.num_column_vars()).unwrap();
        let outer = their_proof.outer.verify(&mut verifier, Fq::ZERO, &tau);
        let mut ours_c = fq_bytes(&tau);
        let label = |k: usize| -> String {
            let nr = matrices.num_row_vars();
            if k < nr { format!("tau[{k}]") } else if k < 2 * nr { format!("r_x[{}]", k - nr) } else if k == 2 * nr { "rho".into() } else { format!("r_y[{}]", k - 2 * nr - 1) }
        };
        match outer {
            Ok(outer) => {
                let rho = verifier.squeeze_fq();
                ours_c.extend(fq_bytes(&outer.eval_points));
                ours_c.extend(fq_bytes(&[rho]));
                let inner_claim = outer.az_mle_claim + rho * outer.bz_mle_claim + rho * rho * outer.cz_mle_claim;
                if let Ok((r_y, _)) = their_proof.inner.verify(&mut verifier, inner_claim, matrices.num_column_vars()) {
                    ours_c.extend(fq_bytes(&r_y));
                }
                let d = matrices.bind_and_batch(&outer.eval_points, rho).unwrap();
                let ours_d = fq_bytes(&d);
                let theirs_d = read("d.bin");
                println!("D table (bind_and_batch at THEIR r_x, rho): {}", describe(&ours_d, &theirs_d));
                if ours_d != theirs_d {
                    let ov: Vec<u128> = ours_d.chunks_exact(16).map(|c| u128::from_le_bytes(c.try_into().unwrap())).collect();
                    let tv: Vec<u128> = theirs_d.chunks_exact(16).map(|c| u128::from_le_bytes(c.try_into().unwrap())).collect();
                    let n = ov.iter().zip(&tv).filter(|(o, t)| o != t).count();
                    let first = ov.iter().zip(&tv).position(|(o, t)| o != t).unwrap();
                    println!("  {n} of {} entries differ; first at column {first}: ours {:x} theirs {:x}", ov.len(), ov[first], tv[first]);
                }
            }
            Err(e) => println!("their outer proof under our challenges: {e:?}"),
        }
        let n = ours_c.len().min(theirs_c.len()) / 16;
        let first = (0..n).find(|&k| ours_c[16 * k..16 * k + 16] != theirs_c[16 * k..16 * k + 16]);
        match first {
            None => println!("challenges: all {n} compared IDENTICAL (theirs has {})", theirs_c.len() / 16),
            Some(k) => println!("challenges: first difference at {} ({} of {} compared)", label(k), k, n),
        }
    }

    // Verify theirs and ours.
    let (their_proof, their_terminal) =
        SpartanPiopProof::from_bytes(&theirs, matrices.num_row_vars(), matrices.num_column_vars()).expect("their spartan.bin decodes");
    for (label, proof, terminal) in [("THEIR", &their_proof, &their_terminal), ("OUR", &proof, &terminal)] {
        let empty = f2z::bitz::Proof::default();
        let mut verifier = build_verifier(session.as_bytes(), instance.as_bytes(), &empty);
        bind(&mut verifier);
        let started = Instant::now();
        let checked = verify_spartan_proof(&mut verifier, &matrices, proof);
        let elapsed = started.elapsed();
        let next_v: Gf = verifier.verifier_message();
        let eof = verifier.check_eof().is_ok();
        println!(
            "our verifier on {label} proof: {} ({elapsed:.1?}); terminal {}; next challenge {}; eof {eof}",
            match &checked {
                Ok(_) => "Ok".to_string(),
                Err(e) => format!("Err({e:?})"),
            },
            if checked.as_ref().ok() == Some(terminal) { "MATCH" } else { "MISMATCH" },
            if next_v == next { "MATCH" } else { "MISMATCH" }
        );
    }
    assert!(terminal.nonsuccinct_verify(&assignment), "terminal claim holds on the assignment");
}

/// `Prepared::bind`, object-safe over both transcript halves.
trait BindTarget {
    fn bind(&mut self, public: &[u8], root: &[u8; 32], params: &BitZParams, pcs: &Pcs, map_digest: &[u8; 32]);
}

impl<T: PublicTranscript> BindTarget for T {
    fn bind(&mut self, public: &[u8], root: &[u8; 32], params: &BitZParams, pcs: &Pcs, map_digest: &[u8; 32]) {
        self.public_message(&(public.len() as u64));
        self.public_message(public);
        self.public_message(root);
        self.public_message(params);
        self.public_message(pcs);
        self.public_message(map_digest);
        let _ = Encoding::encode(&0u64);
    }
}
