//! Compact matrix reduction: SHA's repeated rows never become a global matrix,
//! and the P-256 matrices are reduced through their table of distinct
//! coefficients and combined one assignment cell at a time.

use crypto_primitives::{FromWithConfig, PrimeField};
use num_bigint::BigInt;
use num_traits::ToPrimitive;
#[cfg(feature = "parallel")]
use rayon::prelude::*;

use super::{
    Result, error,
    relation::{OuterMode, PreparedSha256Ecdsa, SHA_H, Sha256EcdsaStatement},
    witness::Sha256EcdsaWitness,
};
use crate::{
    piop::spartan::{
        f2z::SpartanF2zField as F,
        matrix::eq_table,
        raw_monty::{Raw, RawMontyCtx},
        sumcheck::{OuterSumcheckProof, R1csProductMles},
    },
    poly::mle::DenseMultilinearExtension,
};

pub(super) type Config = <F as PrimeField>::Config;

/// Work items per parallel block over the P-256 assignment tail.
#[cfg(feature = "parallel")]
const TAIL_BLOCK: usize = 1 << 12;

pub(super) struct Projection {
    ctx: RawMontyCtx,
    /// Montgomery residues of `LocalRelation::coefficients` modulo `q`.
    residues: Vec<Raw>,
}

pub(super) fn reduce(value: &BigInt, q: u128, cfg: &Config) -> F {
    let modulus = BigInt::from(q);
    let value = ((value % &modulus) + &modulus) % modulus;
    F::from_with_cfg(value.to_u128().unwrap(), cfg)
}

impl Projection {
    pub fn new(p: &PreparedSha256Ecdsa, q: u128, cfg: &Config) -> Self {
        let _scope = crate::utils::prof::scope("ecdsa:matrix_projection");
        let ctx = RawMontyCtx::new(cfg);
        let residues = p
            .local
            .coefficients
            .iter()
            .map(|coefficient| ctx.raw(&reduce(coefficient, q, cfg)))
            .collect();
        Self { ctx, residues }
    }

    pub fn outer_products(
        &self,
        p: &PreparedSha256Ecdsa,
        w: &Sha256EcdsaWitness,
        q: u128,
        cfg: &Config,
    ) -> R1csProductMles<F> {
        let vars = outer_vars(p);
        let zero = F::zero_with_cfg(cfg);
        let mut tables = std::array::from_fn::<_, 3, _>(|_| vec![zero.clone(); 1 << vars]);
        let products = [&w.products.a_mw, &w.products.b_mw, &w.products.c_mw];
        for (table, products) in tables.iter_mut().zip(products) {
            let mut set = |dst: usize, src: usize| {
                let bytes: Vec<_> = products[src]
                    .words()
                    .iter()
                    .flat_map(|w| w.to_le_bytes())
                    .collect();
                table[dst] = reduce(&BigInt::from_signed_bytes_le(&bytes), q, cfg);
            };
            match p.mode {
                OuterMode::Split => {
                    for (i, &r) in p.local.nonlinear.iter().enumerate() {
                        set(i, r);
                    }
                }
                OuterMode::AllRows => {
                    for r in 0..p.local.rows() {
                        set(256 * p.compressions() + r, r);
                    }
                }
            }
        }
        // Honest SHA row products are identically zero. These are prover
        // claims, not a verifier assumption: the shared inner check binds the
        // zero C claims to the committed SHA assignment. Reuse them instead of
        // charging the comparison mode for a second SHA matrix multiplication.
        let [a, b, c] = tables.map(|evaluations| DenseMultilinearExtension {
            evaluations,
            num_vars: vars,
        });
        R1csProductMles {
            az: a,
            bz: b,
            cz: c,
        }
    }

    /// D = A(r_x,.) + rho B(r_x,.) + rho² C(r_x,.) + gamma Lᵀeq(sigma,.).
    /// The explicit public equations include h[0]=1, so this is affine, not homogeneous.
    #[allow(clippy::too_many_arguments)]
    pub fn combine(
        &self,
        p: &PreparedSha256Ecdsa,
        statement: &Sha256EcdsaStatement,
        outer: &OuterSumcheckProof<F>,
        rx: &[F],
        rho: &F,
        sigma: &[F],
        gamma: &F,
        cfg: &Config,
    ) -> Result<Coefficients> {
        let _scope = crate::utils::prof::scope("ecdsa:coefficient_combine");
        let ctx = self.ctx;
        let mut rho2 = rho.clone();
        rho2 *= rho;
        let mut target = outer.az_mle_claim.clone();
        let mut term = outer.bz_mle_claim.clone();
        term *= rho;
        target += &term;
        term = outer.cz_mle_claim.clone();
        term *= &rho2;
        target += &term;
        let sigma_weights = Equality::new(sigma, cfg)?;
        let row_weights = Equality::new(rx, cfg)?;
        let (rho_raw, rho2_raw) = (ctx.raw(rho), ctx.raw(&rho2));
        // One weight per (row, matrix) slot of the column-major tail index:
        // the outer row weight for A, times rho for B, times rho² for C; the
        // linear rows' C weight is their gamma-scaled sigma weight.
        let mut weights = vec![0 as Raw; 3 * p.local.rows()];
        let outer_weights = |slots: &mut [Raw], weight: Raw| {
            slots[0] = weight;
            slots[1] = ctx.mul(weight, rho_raw);
            slots[2] = ctx.mul(weight, rho2_raw);
        };
        match p.mode {
            OuterMode::Split => {
                for (i, &r) in p.local.nonlinear.iter().enumerate() {
                    outer_weights(&mut weights[3 * r..3 * r + 3], ctx.raw(&row_weights.at(i)));
                }
                for (i, &r) in p.local.linear.iter().enumerate() {
                    let mut weight = sigma_weights.at(256 * p.compressions() + i);
                    weight *= gamma;
                    weights[3 * r + 2] = ctx.raw(&weight);
                }
            }
            OuterMode::AllRows => {
                for r in 0..p.local.rows() {
                    let weight = ctx.raw(&row_weights.at(256 * p.compressions() + r));
                    outer_weights(&mut weights[3 * r..3 * r + 3], weight);
                }
            }
        }
        let columns = &p.local.tail;
        let residues = &self.residues;
        let gather = |j: usize| -> Raw {
            columns.column(j).fold(0 as Raw, |sum, (slot, k)| {
                ctx.add(sum, ctx.mul(weights[slot], residues[k]))
            })
        };
        #[cfg(feature = "parallel")]
        let mut tail_raw: Vec<Raw> = (0..columns.columns())
            .into_par_iter()
            .with_min_len(TAIL_BLOCK)
            .map(gather)
            .collect();
        #[cfg(not(feature = "parallel"))]
        let mut tail_raw: Vec<Raw> = (0..columns.columns()).map(gather).collect();
        let public_start = 256 * p.compressions() + p.local.linear.len();
        let mut constant = sigma_weights.at(public_start);
        constant *= gamma;
        target += &constant;
        for bit in 0..1024 {
            let mut weight = sigma_weights.at(public_start + 1 + bit);
            weight *= gamma;
            let cell = p.local.public_h[bit];
            tail_raw[cell] = ctx.add(tail_raw[cell], ctx.raw(&weight));
            if statement.bit(bit) {
                target += &weight;
            }
        }
        #[cfg(feature = "parallel")]
        let tail: Vec<F> = tail_raw
            .par_iter()
            .with_min_len(TAIL_BLOCK)
            .map(|&value| ctx.field(value))
            .collect();
        #[cfg(not(feature = "parallel"))]
        let tail: Vec<F> = tail_raw.iter().map(|&value| ctx.field(value)).collect();
        let (point, multiplier) = match p.mode {
            OuterMode::Split => (sigma, gamma),
            OuterMode::AllRows => (rx, &rho2),
        };
        let instances = eq_table(&point[..p.log_n], cfg).map_err(error)?;
        let locals = eq_table(&point[p.log_n..], cfg).map_err(error)?;
        let multiplier = ctx.raw(multiplier);
        let mut sha_raw = vec![0 as Raw; SHA_H];
        for r in 0..p.local.sha_c.rows() {
            let weight = ctx.mul(ctx.raw(&locals[r]), multiplier);
            for (j, k) in p.local.sha_c.row(r) {
                sha_raw[j] = ctx.add(sha_raw[j], ctx.mul(weight, residues[k]));
            }
        }
        let sha = sha_raw.iter().map(|&value| ctx.field(value)).collect();
        Ok(Coefficients {
            ctx,
            instances,
            sha,
            tail,
            tail_raw,
            constant,
            target,
            h_offset: p.map.h_offset,
        })
    }
}

pub(super) fn outer_vars(p: &PreparedSha256Ecdsa) -> usize {
    let len = match p.mode {
        OuterMode::Split => p.local.nonlinear.len(),
        OuterMode::AllRows => 256 * p.compressions() + p.local.rows(),
    };
    len.next_power_of_two().ilog2() as usize
}

/// Only O(sqrt(domain)) equality weights, also used for the unaligned P-256 tail.
pub(super) struct Equality {
    low: Vec<F>,
    high: Vec<F>,
    split: usize,
}
impl Equality {
    pub fn new(point: &[F], cfg: &Config) -> Result<Self> {
        let split = point.len() / 2;
        Ok(Self {
            low: eq_table(&point[..split], cfg).map_err(error)?,
            high: eq_table(&point[split..], cfg).map_err(error)?,
            split,
        })
    }
    pub fn at(&self, i: usize) -> F {
        let mut v = self.low[i & (self.low.len() - 1)].clone();
        v *= &self.high[i >> self.split];
        v
    }
}

pub(super) struct Coefficients {
    ctx: RawMontyCtx,
    instances: Vec<F>,
    sha: Vec<F>,
    tail: Vec<F>,
    /// The same tail as raw residues, for the terminal evaluation.
    tail_raw: Vec<Raw>,
    constant: F,
    pub target: F,
    h_offset: usize,
}
impl Coefficients {
    pub fn inner_source(
        &self,
        cfg: &Config,
    ) -> Result<super::super::sha256::inner_sumcheck::CompositeCoefficients<'_>> {
        super::super::sha256::inner_sumcheck::CompositeCoefficients::new(
            &self.sha,
            &self.instances,
            &self.tail,
            self.constant.clone(),
            cfg,
        )
        .map_err(error)
    }
    pub fn evaluate(&self, point: &[F], cfg: &Config) -> Result<F> {
        let _scope = crate::utils::prof::scope("ecdsa:coefficient_evaluate");
        let log_n = self.instances.len().ilog2() as usize;
        let inst_eq = eq_table(&point[..log_n], cfg).map_err(error)?;
        let local_eq = eq_table(&point[log_n..], cfg).map_err(error)?;
        let dot = |a: &[F], b: &[F]| {
            a.iter()
                .zip(b)
                .fold(F::zero_with_cfg(cfg), |mut s, (a, b)| {
                    let mut v = a.clone();
                    v *= b;
                    s += &v;
                    s
                })
        };
        let mut value = dot(&self.instances, &inst_eq);
        value *= &dot(&self.sha, &local_eq);
        let eq = Equality::new(point, cfg)?;
        let mut constant = eq.at(0);
        constant *= &self.constant;
        value += &constant;
        // The tail against eq(point, h_offset + i), one O(1) weight per cell.
        let ctx = self.ctx;
        let (low, high) = (ctx.raw_vec(&eq.low), ctx.raw_vec(&eq.high));
        let (mask, split, h_offset) = (low.len() - 1, eq.split, self.h_offset);
        let term = |i: usize, coefficient: &Raw| -> Raw {
            let index = h_offset + i;
            ctx.mul(ctx.mul(low[index & mask], high[index >> split]), *coefficient)
        };
        #[cfg(feature = "parallel")]
        let tail = self
            .tail_raw
            .par_iter()
            .enumerate()
            .with_min_len(TAIL_BLOCK)
            .fold(|| 0 as Raw, |sum, (i, coefficient)| ctx.add(sum, term(i, coefficient)))
            .reduce(|| 0 as Raw, |a, b| ctx.add(a, b));
        #[cfg(not(feature = "parallel"))]
        let tail = self
            .tail_raw
            .iter()
            .enumerate()
            .fold(0 as Raw, |sum, (i, coefficient)| ctx.add(sum, term(i, coefficient)));
        value += &ctx.field(tail);
        Ok(value)
    }
}
