//! Union-bound accounting for the supported composition shapes.
use super::{Error, opening::Geometry};
use crate::piop::spartan::{
    f2z::PreparedU32MulRelation,
    profile::{IopSecurityProfile, PrimePolicy},
};

/// Slack for the complete composition: the standalone 100-bit per-round
/// preset cannot be reused as a 100-bit whole-protocol guarantee.
pub struct CompositionProfile;
impl IopSecurityProfile for CompositionProfile {
    const NAME: &'static str = "hybrid100-components108";
    const LAMBDA: u32 = 108;
    const PRIME_POLICY: PrimePolicy = PrimePolicy::SingleDerived;
    const LIGERITO_TARGET_BITS: usize = 112;
    const FOREST_ROUND_GRINDING_BITS: u32 = 0;
    const RING_SWITCH_GRINDING_BITS: u32 = 0;
}

#[derive(Clone, Debug)]
pub struct SecurityTerm {
    pub name: &'static str,
    pub error_bound: f64,
}

/// Algebraic/IOP error accounting, using the pinned implementations' soundness
/// analyses and grinding model. Fiat–Shamir uses BLAKE3, and both Merkle trees
/// use its 256-bit output (128-bit generic collision resistance). This is not
/// a claim of an unconditional 100-bit Fiat–Shamir security theorem.
#[derive(Clone, Debug)]
pub struct SecurityReport {
    pub target_bits: u32,
    pub algebraic_bits: f64,
    pub terms: Vec<SecurityTerm>,
}

pub(super) fn account(
    mul: &PreparedU32MulRelation,
    sha: &binius_verifier::IOPVerifier,
    geometry: &Geometry,
) -> Result<SecurityReport, Error> {
    let mut terms = Vec::new();
    let mut add = |name, error_bound| terms.push(SecurityTerm { name, error_bound });
    let gate_log = mul.layout().gate_vars();
    for term in &mul.security().accounting.terms {
        // These are precisely the integer-prefix stages retained here.
        let count = match term.name {
            "step2:projection-draw" | "step3:tau-draw" | "step4:terminal-draw" => 1,
            "step3:piop-round" => 2 * gate_log + 8,
            _ if term.name.starts_with("step2:") => 1,
            _ => continue,
        };
        add(term.name, count as f64 * 2f64.powf(-term.bits));
    }
    let p = mul.params();
    let depth = p.t + p.word_bits.trailing_zeros() as usize;
    let k_inv = 2f64.powi(-128);
    // Two sumchecks per GKR layer, with degrees at most three, plus the
    // closing child randomization. Overcount all rounds by depth+s+4.
    add(
        "multiplication GKR",
        (4 * depth * (depth + p.s + 4)) as f64 * k_inv,
    );
    let cs = sha.constraint_system();
    let sha_dims = sha.log_witness_words()
        + cs.log_and_constraints().unwrap_or(0)
        + cs.log_zero_constraints().unwrap_or(0)
        + 64;
    // AND's univariate degree is <=126 (64-bit word domain), with three
    // skipped Boolean coordinates. The subsequent zerocheck/shift rounds
    // have degree <=3. 4096 per coordinate bounds the initial identity
    // tests, skipped rounds, all operand batches and their sumchecks.
    // No IMUL/BMUL auxiliary protocols are allowed in this SHA circuit.
    add("SHA PIOP", (4096 * sha_dims) as f64 * k_inv);
    add(
        "joint sumcheck and batching",
        (2 * geometry.bit_log() + 1) as f64 * k_inv,
    );
    add("ring switching", 128.0 * k_inv);
    let config = geometry.security();
    config.validate().map_err(Error::Config)?;
    for level in &config.levels {
        let (pg, query) = level.paper_predicted_bits();
        add(
            "Ligerito proximity folds",
            level.k_recursive as f64 * 2f64.powf(-pg - level.fold_grinding_bits as f64),
        );
        add(
            "Ligerito queries",
            2f64.powf(-query - level.grinding_bits as f64),
        );
    }
    add(
        "Ligerito field rounds",
        (16 * geometry.packed_log() + 128) as f64 * k_inv,
    );
    let algebraic_bits = -terms
        .iter()
        .map(|term| term.error_bound)
        .sum::<f64>()
        .log2();
    if algebraic_bits < 100.0 {
        return Err(Error::Invalid("composition does not reach 100 bits"));
    }
    Ok(SecurityReport {
        target_bits: 100,
        algebraic_bits,
        terms,
    })
}
