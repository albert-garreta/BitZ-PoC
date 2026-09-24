use super::{
    Metrics, Run,
    config::{Case, Mode, Workload},
};
use anyhow::Result;
use bitz::{
    ligerito_flock::LigeritoSelection,
    piop::spartan::{
        BabyBearMulWitness, IopSecurityProfile, Lambda100, Lambda128,
        baby_bear_mul::{BabyBearMulLayout, sample_baby_bear_operand_with},
        mul::{MulLayout, MulWitness, MulWord},
        protocol::{self, PreparedRelation, RelationSpec},
    },
    transcript::Blake3Transcript,
};
use rand::{RngExt, SeedableRng, rngs::StdRng};
use serde_json::{Value, json};
use std::time::Instant;
#[path = "mod32.rs"]
pub(super) mod mod32;

pub fn selection(case: &Case) -> LigeritoSelection {
    let f = case.bitz.as_ref().expect("BitZ configuration");
    LigeritoSelection::parse(
        f.ligerito.as_deref().expect("proof selection"),
        f.profile.expect("profile"),
    )
    .expect("validated Ligerito selection")
}
pub fn packing<T: MulWord>(layout: &MulLayout<T>) -> Value {
    let shape = |p: bitz::pcs::IntegerMatrixLayout| json!({"t":p.row_vars,"s":p.col_vars,"physical_word_bits":p.word_bits});
    json!({"logical_word_bits":layout.word_bits(),"mode":if layout.uses_direct_opening(){"direct"}else{"virtual"},"committed":shape(layout.committed_layout()),"opening":shape(layout.bitz_params())})
}
pub fn shape_seed(case: &Case) -> u64 {
    if case.workload == Workload::U32Mod32 {
        case.seed
    } else {
        crate::common::mul_witness::shape_seed(case.seed, case.log_n)
    }
}
fn bounds_inputs(seed: u64, log_n: usize) -> Vec<(u128, u128)> {
    let mut hash = blake3::Hasher::new();
    hash.update(b"bitz/ligerito-bound-inputs/v1");
    hash.update(&seed.to_le_bytes());
    let mut stream = hash.finalize_xof();
    (0..1 << log_n)
        .map(|_| {
            let mut b = [0; 32];
            stream.fill(&mut b);
            (
                u128::from_le_bytes(b[..16].try_into().unwrap()),
                u128::from_le_bytes(b[16..].try_into().unwrap()),
            )
        })
        .collect()
}
pub fn run(run: &mut Run, compare: bool) -> Result<()> {
    let mut rng = StdRng::seed_from_u64(shape_seed(&run.job.case));
    let n = 1 << run.job.case.log_n;
    let bounds = (run.job.case.mode == Mode::Bounds)
        .then(|| bounds_inputs(run.job.case.seed, run.job.case.log_n));
    match run.job.case.workload {
        Workload::U32Full | Workload::U32Mod32 => {
            let data = if let Some(inputs) = &bounds {
                inputs.iter().map(|&(a, b)| (a as u32, b as u32)).collect()
            } else if run.job.case.workload == Workload::U32Mod32 {
                mod32::inputs(run.job.case.log_n, run.job.case.seed)
                    .into_iter()
                    .map(|(a, b)| (a as u32, b as u32))
                    .collect()
            } else {
                (0..n).map(|_| (rng.random(), rng.random())).collect()
            };
            integer(run, compare, data, crate::common::mul_witness::u32_digest)
        }
        Workload::U64 => integer(
            run,
            compare,
            bounds
                .as_ref()
                .map(|v| v.iter().map(|&(a, b)| (a as u64, b as u64)).collect())
                .unwrap_or_else(|| {
                    (0..n)
                        .map(|_| (rng.random::<u64>(), rng.random()))
                        .collect()
                }),
            crate::common::mul_witness::u64_digest,
        ),
        Workload::U128 => integer(
            run,
            compare,
            bounds.unwrap_or_else(|| {
                (0..n)
                    .map(|_| (rng.random::<u128>(), rng.random()))
                    .collect()
            }),
            crate::common::mul_witness::u128_digest,
        ),
        Workload::Field => unreachable!("validated proof workload"),
        Workload::BabyBear => {
            let inputs: Vec<_> = bounds
                .map(|v| {
                    v.into_iter()
                        .map(|(a, b)| ((a % 2013265921) as u32, (b % 2013265921) as u32))
                        .collect()
                })
                .unwrap_or_else(|| {
                    (0..n)
                        .map(|_| {
                            (
                                sample_baby_bear_operand_with(|| rng.random()),
                                sample_baby_bear_operand_with(|| rng.random()),
                            )
                        })
                        .collect()
                });
            let layout = BabyBearMulLayout::new(n)?;
            trials(
                run,
                compare,
                layout,
                || Ok(BabyBearMulWitness::from_inputs(&inputs)?),
                BabyBearMulWitness::bitz_bit_rows,
                |w| Ok(crate::common::mul_witness::baby_bear_digest(w)),
                json!({"word_bits":1}),
            )
        }
    }
}
fn integer<T: MulWord>(
    run: &mut Run,
    compare: bool,
    inputs: Vec<(T, T)>,
    digest: fn(&MulWitness<T>) -> String,
) -> Result<()>
where
    MulLayout<T>: RelationSpec<Witness = MulWitness<T>>,
{
    let c = run.job.case.bitz.as_ref().expect("BitZ config");
    let layout = MulLayout::<T>::new_with_word_bits(inputs.len(), c.w)?;
    let layout = if c.opener.as_deref() == Some("wfbitz") {
        layout.wfbitz_split(c.split)?
    } else {
        layout.with_split_shift(c.split)?
    };
    let packing = packing(&layout);
    let modular = run.job.case.workload == Workload::U32Mod32;
    trials(
        run,
        compare,
        layout,
        || Ok(MulWitness::from_fn_with_layout(layout, |i| inputs[i])?),
        MulWitness::bitz_bit_rows,
        |w| {
            // Auditing and digesting are deliberately outside witness generation.
            for (row, &(x, y)) in w.rows().zip(&inputs) {
                anyhow::ensure!(row.x == x && row.y == y, "changed witness input");
                let expected =
                    num_bigint::BigUint::from(x.as_u128()) * num_bigint::BigUint::from(y.as_u128());
                let actual = num_bigint::BigUint::from(row.lo.as_u128())
                    + (num_bigint::BigUint::from(row.hi.as_u128()) << T::BITS);
                anyhow::ensure!(actual == expected, "incorrect multiplication witness");
            }
            Ok(if modular {
                mod32::digest_rows(
                    w.rows().map(|r| {
                        [
                            r.x.as_u128() as u64,
                            r.y.as_u128() as u64,
                            r.lo.as_u128() as u64,
                        ]
                    }),
                    inputs.len(),
                )
            } else {
                digest(w)
            })
        },
        packing,
    )
}
fn trials<S, W>(
    run: &mut Run,
    compare: bool,
    layout: S,
    witness: impl Fn() -> Result<W>,
    pack: impl Fn(&W) -> Vec<Vec<u64>>,
    audit: impl Fn(&W) -> Result<String>,
    packing: Value,
) -> Result<()>
where
    S: RelationSpec<Witness = W>,
    W: Sync,
{
    if run.job.case.mode == Mode::Witness {
        let mut expected = None;
        for i in 0..run.trials() {
            run.begin_memory();
            let start = Instant::now();
            let w = witness()?;
            let ms = start.elapsed().as_secs_f64() * 1000.;
            let digest = audit(&w)?;
            run.end_memory();
            if let Some(expected) = &expected {
                anyhow::ensure!(*expected == digest, "witness changed across trials");
            } else {
                expected = Some(digest);
            }
            run.sample(i, Metrics::from([("witness_ms".into(), ms)]));
        }
        run.effective =
            json!({"packing":packing,"corpus_digest":expected,"boundary":"witness-generation"});
        return Ok(());
    }
    let config = run.job.case.bitz.as_ref().expect("config");
    let profile128 = config.profile == Some(128);
    match config.opener.as_deref() {
        None => {
            let setup = Instant::now();
            let prepared = if profile128 {
                PreparedRelation::new_with_profile_and_ligerito::<Lambda128>(
                    layout,
                    selection(&run.job.case),
                )?
            } else {
                PreparedRelation::new_with_profile_and_ligerito::<Lambda100>(
                    layout,
                    selection(&run.job.case),
                )?
            };
            let setup_ms = setup.elapsed().as_secs_f64() * 1000.;
            let lig = prepared.ligerito_configuration();
            let security = json!({
                "profile":if prepared.security().lambda==128 {Lambda128::NAME}else{Lambda100::NAME},
                "lambda":prepared.security().lambda,"achieved_bits":prepared.security().accounting.achieved_bits(),
                "ligerito":lig.report(&lig.selection().name(),prepared.security().ood)});
            let bounds = run.job.case.mode == Mode::Bounds;
            trial_loop(
                run,
                compare,
                setup_ms,
                security,
                packing,
                witness,
                pack,
                audit,
                |rows| Ok(protocol::commit(&prepared, rows)?),
                |w, hint, transcript| Ok(protocol::prove(transcript, &prepared, w, hint)?),
                |hint, proof| {
                    protocol::verify(&mut Blake3Transcript::new(), &prepared, &hint.commitment, proof)?;
                    Ok(())
                },
                |proof| proof.size_bytes(prepared.security()),
                |proof| {
                    if bounds {
                        let encoded = proof.bitz().to_bytes();
                        let roundtrip = match proof.bitz() {
                            protocol::Opening::Direct(_) => {
                                bitz::ligerito_flock::IntEvalRsLigModQProof::from_bytes(&encoded)?.to_bytes()
                            }
                            protocol::Opening::Virtual(_) => {
                                bitz::ligerito_flock::IntEvalRsLigVirtProof::from_bytes(&encoded)?.to_bytes()
                            }
                        };
                        anyhow::ensure!(encoded == roundtrip, "opening codec roundtrip");
                    }
                    Ok(())
                },
                |proof, root, transcript| {
                    crate::common::proof_fingerprint::nonlinear_fingerprint(proof, root, transcript)
                },
            )
        }
        #[cfg(feature = "bitz-parity")]
        Some("wfbitz") => {
            use bitz::piop::spartan::protocol::wfbitz_opener::{self, WfbitzLigerito, WfbitzOpener};
            let target = config.profile.expect("profile");
            let ladder = WfbitzLigerito::parse(
                config.ligerito.as_deref().expect("proof selection"),
                target,
            )
            .map_err(anyhow::Error::msg)?;
            let setup = Instant::now();
            let (prefix, opener) = if profile128 {
                WfbitzOpener::prepare::<Lambda128, _>(layout, ladder, target)?
            } else {
                WfbitzOpener::prepare::<Lambda100, _>(layout, ladder, target)?
            };
            let setup_ms = setup.elapsed().as_secs_f64() * 1000.;
            let security_params = prefix.security();
            let (opening_bits, opening_binding) = opener.opening_bits();
            let ladder_config = opener.security();
            let security = json!({
                "profile":if security_params.lambda==128 {Lambda128::NAME}else{Lambda100::NAME},
                "lambda":security_params.lambda,
                "achieved_bits":security_params.accounting.achieved_bits().min(opening_bits),
                "piop_achieved_bits":security_params.accounting.achieved_bits(),
                "opener":"wfbitz",
                "ligerito":{
                    "requested_profile":opener.ligerito().name(),
                    "resolved_profile":format!("wfbitz-{}", opener.ligerito().name()),
                    "regime":if ladder_config.levels.first().is_some_and(|l| l.eta.is_some()) {"johnson"} else {"udr"},
                    "target_bits":ladder_config.target_security_bits,
                    "configuration_fingerprint":opener.digest().iter().map(|b| format!("{b:02x}")).collect::<String>(),
                    "outer_ood":prefix.security().ood.is_some(),
                    "ood_grinding_bits":prefix.security().ood.map(|params| params.grinding_bits),
                    "opening_bits":opening_bits,
                    "opening_binding":opening_binding,
                    "configuration":ladder_config,
                }});
            trial_loop(
                run,
                compare,
                setup_ms,
                security,
                packing,
                witness,
                pack,
                audit,
                |rows| Ok(opener.commit(rows)?),
                |w, hint, transcript| Ok(wfbitz_opener::prove(transcript, &prefix, &opener, w, hint)?),
                |hint, proof| {
                    wfbitz_opener::verify(&mut Blake3Transcript::new(), &prefix, &opener, &hint.commitment, proof)?;
                    Ok(())
                },
                |proof| proof.size_bytes(security_params),
                |_proof| Ok(()),
                |proof, root, transcript| {
                    crate::common::proof_fingerprint::nonlinear_fingerprint(proof, root, transcript)
                },
            )
        }
        Some(other) => anyhow::bail!("unknown BitZ opener {other} (or a build without its feature)"),
    }
}

/// The timed trials of one prepared relation, the opener abstracted: each
/// trial commits the packed rows, proves, verifies, and records the
/// paper's metrics (and the traced phases on latency runs).
#[allow(clippy::too_many_arguments)]
fn trial_loop<W, P>(
    run: &mut Run,
    compare: bool,
    setup_ms: f64,
    security: Value,
    packing: Value,
    witness: impl Fn() -> Result<W>,
    pack: impl Fn(&W) -> Vec<Vec<u64>>,
    audit: impl Fn(&W) -> Result<String>,
    commit: impl Fn(Vec<Vec<u64>>) -> Result<bitz::ligerito_flock::FlockCommitHint>,
    prove: impl Fn(&W, &bitz::ligerito_flock::FlockCommitHint, &mut Blake3Transcript) -> Result<P>,
    verify: impl Fn(&bitz::ligerito_flock::FlockCommitHint, &P) -> Result<()>,
    size: impl Fn(&P) -> usize,
    roundtrip: impl Fn(&P) -> Result<()>,
    fingerprint: impl Fn(&P, &[u8], &Blake3Transcript) -> crate::common::proof_fingerprint::ProofFingerprint,
) -> Result<()>
where
    W: Sync,
{
    let generate_each = compare || run.job.case.mode == Mode::Bounds;
    let baseline = witness()?;
    let expected = audit(&baseline)?;
    let baseline = (!generate_each).then_some(baseline);
    run.effective = json!({"packing":packing,"corpus_digest":expected,"boundary":if generate_each {"witness-to-proof"}else{"standalone-proving"},"security": security});
    for i in 0..run.trials() {
        run.begin_memory();
        let recording = run
            .latency()
            .then(|| bitz::observability::Recording::start(Vec::new()))
            .transpose()?;
        let start = Instant::now();
        let generated = if generate_each {
            Some(witness()?)
        } else {
            None
        };
        let witness_ms = if generate_each {
            start.elapsed().as_secs_f64() * 1000.
        } else {
            0.
        };
        let w = generated
            .as_ref()
            .or(baseline.as_ref())
            .expect("trial witness");
        let proving = tracing::info_span!("mul:proving").entered();
        let online = Instant::now();
        let hint = commit(pack(w))?;
        let commit_ms = online.elapsed().as_secs_f64() * 1000.;
        let mut transcript = Blake3Transcript::new();
        let proof = prove(w, &hint, &mut transcript)?;
        let online_ms = online.elapsed().as_secs_f64() * 1000.;
        drop(proving);
        let total_ms = start.elapsed().as_secs_f64() * 1000.;
        let verification = tracing::info_span!("mul:verification").entered();
        let verify_start = Instant::now();
        verify(&hint, &proof)?;
        let verify_ms = verify_start.elapsed().as_secs_f64() * 1000.;
        drop(verification);
        let verified_ms = start.elapsed().as_secs_f64() * 1000.;
        run.end_memory();
        let mut metrics = Metrics::from([
            ("setup_ms".into(), setup_ms),
            ("commit_ms".into(), commit_ms),
            ("online_prover_ms".into(), online_ms),
            ("verify_ms".into(), verify_ms),
            (
                "proof_bytes".into(),
                (hint.commitment.root.len() + size(&proof)) as f64,
            ),
        ]);
        if generate_each {
            metrics.insert("verified_trial_ms".into(), verified_ms);
            metrics.insert("witness_ms".into(), witness_ms);
            metrics.insert("witness_to_proof_ms".into(), total_ms);
        }
        if let Some(recording) = recording {
            let intervals = recording.intervals()?;
            let phases = super::phase_milliseconds(&intervals, "mul:proving")?;
            let phase = |label: &str| {
                phases
                    .iter()
                    .find(|(name, _)| name == label)
                    .map(|(_, ms)| *ms)
            };
            if let Some(piop) = phase("step3:piop_prove") {
                metrics.insert("piop_ms".into(), piop);
            }
            if let (Some(bitify), Some(open)) =
                (phase("step4:bitify_prove"), phase("step5:open_prove"))
            {
                metrics.insert("opening_ms".into(), bitify + open);
                metrics.insert("pcs_ms".into(), commit_ms + bitify + open);
            }
            for (prefix, scope) in [("prove", "mul:proving"), ("verify", "mul:verification")] {
                for (phase, ms) in super::phase_milliseconds(&intervals, scope)? {
                    metrics.insert(format!("{prefix}/{phase}_ms"), ms);
                }
            }
        }
        anyhow::ensure!(audit(w)? == expected, "trial witness differs from corpus");
        roundtrip(&proof)?;
        let fingerprint = (run.latency() && run.job.proof_fingerprints)
            .then(|| fingerprint(&proof, &hint.commitment.root, &transcript));
        std::hint::black_box(proof);
        run.sample(i, metrics);
        run.samples.last_mut().expect("recorded sample").fingerprint = fingerprint;
    }
    Ok(())
}
