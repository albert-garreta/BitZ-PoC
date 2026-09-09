//! Compact matrix reduction: SHA's repeated rows never become a global matrix.

use crypto_primitives::{FromWithConfig, PrimeField};
use num_bigint::BigInt;
use num_traits::ToPrimitive;

use super::{
    Result, error,
    relation::{IntegerRow, OuterMode, PreparedSha256Ecdsa, SHA_H, Sha256EcdsaStatement},
    witness::Sha256EcdsaWitness,
};
use crate::{
    piop::spartan::{
        f2z::SpartanF2zField as F,
        matrix::eq_table,
        sumcheck::{OuterSumcheckProof, R1csProductMles},
    },
    poly::mle::DenseMultilinearExtension,
};

pub(super) type Config = <F as PrimeField>::Config;
type Row = Vec<(usize, F)>;

pub(super) struct Projection {
    sha_c: Vec<Row>,
    a: Vec<Row>,
    b: Vec<Row>,
    c: Vec<Row>,
}

pub(super) fn reduce(value: &BigInt, q: u128, cfg: &Config) -> F {
    let modulus = BigInt::from(q);
    let value = ((value % &modulus) + &modulus) % modulus;
    F::from_with_cfg(value.to_u128().unwrap(), cfg)
}

impl Projection {
    pub fn new(p: &PreparedSha256Ecdsa, q: u128, cfg: &Config) -> Self {
        let project = |rows: &[IntegerRow]| {
            rows.iter()
                .map(|row| {
                    row.iter()
                        .filter_map(|(c, v)| {
                            let v = reduce(v, q, cfg);
                            (!F::is_zero(&v)).then_some((*c, v))
                        })
                        .collect()
                })
                .collect()
        };
        Self {
            sha_c: project(&p.local.sha_c),
            a: project(&p.local.a),
            b: project(&p.local.b),
            c: project(&p.local.c),
        }
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
                    for r in 0..p.local.a.len() {
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
        let zero = F::zero_with_cfg(cfg);
        let mut rho2 = rho.clone();
        rho2 *= rho;
        let mut target = outer.az_mle_claim.clone();
        let mut term = outer.bz_mle_claim.clone();
        term *= rho;
        target += &term;
        term = outer.cz_mle_claim.clone();
        term *= &rho2;
        target += &term;
        let mut tail = vec![zero.clone(); p.local.p_map.rows()];
        let sigma_weights = Equality::new(sigma, cfg)?;
        let row_weights = Equality::new(rx, cfg)?;
        let add_row = |dst: &mut [F], row: &Row, weight: &F| {
            for (j, value) in row {
                let mut term = value.clone();
                term *= weight;
                dst[*j] += &term;
            }
        };
        let add_outer = |dst: &mut [F], r: usize, weight: F| {
            add_row(dst, &self.a[r], &weight);
            let mut wb = weight.clone();
            wb *= rho;
            add_row(dst, &self.b[r], &wb);
            let mut wc = weight;
            wc *= &rho2;
            add_row(dst, &self.c[r], &wc);
        };
        match p.mode {
            OuterMode::Split => {
                for (i, &r) in p.local.nonlinear.iter().enumerate() {
                    add_outer(&mut tail, r, row_weights.at(i));
                }
                for (i, &r) in p.local.linear.iter().enumerate() {
                    let mut weight = sigma_weights.at(256 * p.compressions() + i);
                    weight *= gamma;
                    add_row(&mut tail, &self.c[r], &weight);
                }
            }
            OuterMode::AllRows => {
                for r in 0..self.a.len() {
                    add_outer(&mut tail, r, row_weights.at(256 * p.compressions() + r));
                }
            }
        }
        let public_start = 256 * p.compressions() + p.local.linear.len();
        let mut constant = sigma_weights.at(public_start);
        constant *= gamma;
        target += &constant;
        for bit in 0..1024 {
            let mut weight = sigma_weights.at(public_start + 1 + bit);
            weight *= gamma;
            tail[p.local.public_h[bit]] += &weight;
            if statement.bit(bit) {
                target += &weight;
            }
        }
        let (point, multiplier) = match p.mode {
            OuterMode::Split => (sigma, gamma),
            OuterMode::AllRows => (rx, &rho2),
        };
        let instances = eq_table(&point[..p.log_n], cfg).map_err(error)?;
        let locals = eq_table(&point[p.log_n..], cfg).map_err(error)?;
        let mut sha = vec![zero; SHA_H];
        for (r, row) in self.sha_c.iter().enumerate() {
            let mut weight = locals[r].clone();
            weight *= multiplier;
            add_row(&mut sha, row, &weight);
        }
        Ok(Coefficients {
            instances,
            sha,
            tail,
            constant,
            target,
            h_offset: p.map.h_offset,
        })
    }
}

pub(super) fn outer_vars(p: &PreparedSha256Ecdsa) -> usize {
    let len = match p.mode {
        OuterMode::Split => p.local.nonlinear.len(),
        OuterMode::AllRows => 256 * p.compressions() + p.local.a.len(),
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
    instances: Vec<F>,
    sha: Vec<F>,
    tail: Vec<F>,
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
        for (i, coefficient) in self.tail.iter().enumerate() {
            let mut v = eq.at(self.h_offset + i);
            v *= coefficient;
            value += &v;
        }
        Ok(value)
    }
}
