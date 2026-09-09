use super::{PreparedSha256Ecdsa, Result, error, reduction::outer_vars};
use crate::{
    ligerito_flock::{atomic::AtomicPlan, custom_udr_grind_config_bits},
    piop::spartan::profile::log2_prime_count_lower_bound,
};

/// One complete Fiat–Shamir draw block, summing all bad events before grinding.
#[derive(Clone, Debug)]
pub struct ChallengeBudget {
    pub label: String,
    pub raw_error: f64,
    pub grinding_bits: u32,
    pub multiplicity_bound: usize,
}

/// Round-by-round economic accounting; grinding does not improve the separately
/// reported statistical union bound. BLAKE3 collision resistance is capped at 128.
#[derive(Clone, Debug)]
pub struct Sha256EcdsaSecurity {
    pub target: u32,
    pub blocks: Vec<ChallengeBudget>,
    pub(crate) initial: u32,
    pub(crate) batch: u32,
    pub(crate) outer: u32,
    pub(crate) inner: u32,
    pub(crate) forest: u32,
    pub(crate) flock: AtomicPlan,
}

impl Sha256EcdsaSecurity {
    pub fn economic_bits(&self) -> f64 {
        self.blocks
            .iter()
            .map(|b| -b.raw_error.log2() + f64::from(b.grinding_bits))
            .fold(128., f64::min)
    }
    pub fn statistical_bits(&self) -> f64 {
        -self
            .blocks
            .iter()
            .map(|b| b.raw_error * b.multiplicity_bound as f64)
            .sum::<f64>()
            .log2()
    }
    pub(crate) fn derive(p: &PreparedSha256Ecdsa) -> Result<Self> {
        let mut blocks = Vec::new();
        let mut add = |label: &str, error: f64, count: usize| -> Result<u32> {
            let bits = (f64::from(p.lambda) + error.log2()).ceil().max(0.) as u32;
            if bits > 32 {
                return Err(super::error(format!("{label} exceeds 32-bit grinding cap")));
            }
            blocks.push(ChallengeBudget {
                label: label.into(),
                raw_error: error,
                grinding_bits: bits,
                multiplicity_bound: count,
            });
            Ok(bits)
        };
        let q_inv = 2f64.powi(-112);
        let divisors = p.local.defect_bits / 112;
        let bad_prime = f64::from(divisors) * 2f64.powf(-log2_prime_count_lower_bound(113));
        // The maximum all-row arity keeps the two benchmark modes on one schedule.
        let max_outer = (256 * p.compressions() + p.local.a.len())
            .next_power_of_two()
            .ilog2() as usize;
        let initial = add("prime+tau", bad_prime + max_outer as f64 * q_inv, 1)?;
        let batch = add("rho+sigma+gamma", (p.linear_vars() + 3) as f64 * q_inv, 1)?;
        let outer = add("outer-round", 3. * q_inv, outer_vars(p))?;
        let inner = add("inner-round", 2. * q_inv, p.p_h.t + p.p_h.s)?;
        // Each host forest/bridge draw has degree at most the assignment arity
        // plus seven ring coordinates. 4096 bounds the number of draws for the
        // supported <=31-variable, single-chunk shapes (deliberately conservative).
        let forest = add(
            "forest-and-bridge",
            (p.p_h.t + p.p_h.s + 7) as f64 * 2f64.powi(-128),
            4096,
        )?;
        let config = custom_udr_grind_config_bits(p.p_f.t + p.p_f.s, 1, 4, Some(p.lambda as usize));
        config.validate().map_err(error)?;
        let flock = AtomicPlan::udr(&config, p.lambda).map_err(error)?;
        for b in &flock.blocks {
            blocks.push(ChallengeBudget {
                label: format!("flock/{}", b.label),
                raw_error: b.raw_error,
                grinding_bits: b.bits,
                multiplicity_bound: 1,
            });
        }
        Ok(Self {
            target: p.lambda,
            blocks,
            initial,
            batch,
            outer,
            inner,
            forest,
            flock,
        })
    }
}

impl PreparedSha256Ecdsa {
    pub fn security(&self) -> Result<Sha256EcdsaSecurity> {
        Sha256EcdsaSecurity::derive(self)
    }
}
