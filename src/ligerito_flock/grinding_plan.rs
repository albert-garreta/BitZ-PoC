//! Transcript-independent work budgets for the pinned Flock challenge blocks.
use flock_core::pcs::ligerito;

/// Consecutive challenge draws covered by one grinding requirement.
#[derive(Clone, Debug)]
pub(crate) struct ChallengeBlock {
    pub label: String,
    pub native_bits: Option<u32>,
    pub raw_error: f64,
    pub bits: u32,
}

#[derive(Clone, Debug)]
pub(crate) struct GrindingPlan {
    pub blocks: Vec<ChallengeBlock>,
    pub(super) source: [u8; 32],
}

impl GrindingPlan {
    pub(crate) fn matches(&self, config: &ligerito::LigeritoSecurityConfig) -> bool {
        bincode::serialize(config)
            .is_ok_and(|bytes| self.source == *blake3::hash(&bytes).as_bytes())
    }

    #[cfg(test)]
    pub(crate) fn scripted(blocks: Vec<ChallengeBlock>) -> Self {
        Self {
            blocks,
            source: [0; 32],
        }
    }

    pub fn resolve(config: &ligerito::LigeritoSecurityConfig, target: u32) -> Result<Self, String> {
        config.validate()?;
        let source =
            *blake3::hash(&bincode::serialize(config).map_err(|e| e.to_string())?).as_bytes();
        let mut blocks: Vec<ChallengeBlock> = Vec::new();
        let gf_error = 2f64.powi(-128);
        for (level, params) in config.levels.iter().enumerate() {
            if params.fold_grinding_bits > 32 || params.grinding_bits > 32 {
                return Err("native Flock work exceeds the 32-bit grinding cap".into());
            }
            let (pg, query) = params.paper_predicted_bits();
            if !pg.is_finite() || !query.is_finite() {
                return Err("nonfinite Flock soundness bound".into());
            }
            let folds = if level == 0 {
                config.initial_k
            } else {
                params.k_recursive
            };
            for round in 0..folds {
                let native = (params.fold_grinding_bits as u32).saturating_sub(round as u32);
                // Johnson: the row union shrinks by one bit per round (the taper's
                // justification). Unique decoding: every round carries the same term.
                let raw = match params.regime {
                    ligerito::SoundnessRegime::JohnsonOod => pg + round as f64,
                    ligerito::SoundnessRegime::Udr => pg,
                };
                let error = 2f64.powf(-raw) + 2. * gf_error;
                if level > 0 && round == 0 && native == 0 {
                    // Introduce beta and the unground first fold have no
                    // intervening observation. Their errors share one budget.
                    let last = blocks.last_mut().ok_or("missing introduce block")?;
                    last.raw_error += error;
                    last.label.push_str("+fold0");
                } else {
                    blocks.push(ChallengeBlock {
                        label: format!("fold/{level}/{round}"),
                        native_bits: (native > 0).then_some(native),
                        raw_error: error,
                        bits: 0,
                    });
                }
            }
            // The next root is observed after these folds, before this level's
            // queries. Each OOD evaluation/introduction is an observation. Its
            // beta and the following sample's coordinates have no observation
            // between them and therefore share one uninterrupted block.
            if let Some(next) = config.levels.get(level + 1) {
                if next.ood_samples > 0 {
                    let mut single = next.clone();
                    single.ood_samples = 1;
                    let ood_bits = single
                        .paper_predicted_ood_bits()
                        .ok_or("OOD without a Johnson bound")?;
                    if !ood_bits.is_finite() {
                        return Err("nonfinite Flock OOD bound".into());
                    }
                    let collision = 2f64.powf(-ood_bits);
                    for sample in 0..next.ood_samples {
                        if sample == 0 {
                            blocks.push(ChallengeBlock {
                                label: format!("ood/{}/{sample}", level + 1),
                                native_bits: None,
                                raw_error: collision,
                                bits: 0,
                            });
                        } else {
                            let block = blocks.last_mut().ok_or("missing OOD beta block")?;
                            block.raw_error += collision;
                            block
                                .label
                                .push_str(&format!("+ood/{}/{sample}", level + 1));
                        }
                        blocks.push(ChallengeBlock {
                            label: format!("ood-beta/{}/{sample}", level + 1),
                            native_bits: None,
                            raw_error: gf_error,
                            bits: 0,
                        });
                    }
                }
            }
            let alpha_vars = params
                .queries
                .checked_next_power_of_two()
                .ok_or("Flock query count overflows")?
                .ilog2();
            blocks.push(ChallengeBlock {
                label: format!("queries/{level}"),
                native_bits: Some(params.grinding_bits as u32),
                // Count alpha even at the final level where the prover omits
                // the verifier's final alpha and beta draws. It does not split the block.
                raw_error: 2f64.powf(-query)
                    + (f64::from(alpha_vars)
                        + if level + 1 == config.levels.len() {
                            1.
                        } else {
                            0.
                        })
                        * gf_error,
                bits: 0,
            });
            if level + 1 < config.levels.len() {
                blocks.push(ChallengeBlock {
                    label: format!("introduce/{level}"),
                    native_bits: None,
                    raw_error: gf_error,
                    bits: 0,
                });
            }
        }
        for block in &mut blocks {
            if !block.raw_error.is_finite() || block.raw_error <= 0. {
                return Err(format!("{} has an invalid error bound", block.label));
            }
            let work = (f64::from(target) + block.raw_error.log2()).ceil().max(0.);
            if !work.is_finite() || work > 32. {
                return Err(format!("{} exceeds the 32-bit grinding cap", block.label));
            }
            block.bits = work as u32;
            block.bits = block.bits.max(block.native_bits.unwrap_or(0));
            if block.bits > 32 {
                return Err(format!("{} exceeds the 32-bit grinding cap", block.label));
            }
        }
        Ok(Self { blocks, source })
    }
}

#[cfg(test)]
mod falcon_survey {
    use super::*;
    use crate::ligerito_flock::LigeritoSelection;

    /// Diagnostic dump of the PCS challenge blocks at the Falcon geometries.
    #[test]
    #[ignore = "diagnostic survey; run explicitly with --nocapture"]
    fn falcon_pcs_grinding_survey() {
        let selections = [
            ("udrg:1:4", LigeritoSelection::MATCHED_UDR),
            ("custom:1:4", LigeritoSelection::JOHNSON),
            ("udrg:2:4", LigeritoSelection::parse("udrg:2:4", 128).unwrap()),
            ("udrg:1:5", LigeritoSelection::parse("udrg:1:5", 128).unwrap()),
            ("udrg:1:6", LigeritoSelection::parse("udrg:1:6", 128).unwrap()),
        ];
        for (name, packed_log) in [("alg512", 18), ("alg1024", 19), ("full512", 23), ("full1024", 24)] {
            for target in [100usize, 128] {
                for (sel_name, selection) in selections {
                    let Ok(resolved) = selection.resolve(packed_log, target) else {
                        println!("SKIP {name} {target} {sel_name}");
                        continue;
                    };
                    let first = GrindingPlan::resolve(resolved.security(), target as u32).unwrap();
                    let pcs_target =
                        target + 2 + first.blocks.len().next_power_of_two().ilog2() as usize;
                    let plan = GrindingPlan::resolve(resolved.security(), pcs_target as u32).unwrap();
                    let work: f64 = plan.blocks.iter().map(|b| 2f64.powi(b.bits as i32)).sum();
                    let err: f64 = plan.blocks.iter().map(|b| b.raw_error * 2f64.powi(-(b.bits as i32))).sum();
                    let queries: Vec<usize> = resolved.security().levels.iter().map(|l| l.queries).collect();
                    println!(
                        "PLAN {name} target={target} sel={sel_name} pcs_target={pcs_target} blocks={} work_log2={:.2} err_log2={:.2} queries={queries:?}",
                        plan.blocks.len(),
                        work.log2(),
                        err.log2()
                    );
                    if sel_name == "udrg:1:4" {
                        for b in &plan.blocks {
                            println!(
                                "BLOCK {name} {target} {} raw_log2={:.3} bits={} native={:?}",
                                b.label,
                                b.raw_error.log2(),
                                b.bits,
                                b.native_bits
                            );
                        }
                    }
                }
            }
        }
    }
}
