//! Host-side prover for the SP1 PQ-bench stub.
//!
//! Builds the program ELF (via `sp1-build` if needed), mints fresh
//! cryptographic fixtures (ML-KEM-768 keypair + ciphertext, ML-DSA-65
//! keypair + signature on a fixed message), runs SP1 in STARK-only mode,
//! and prints wall-clock proving time + total cycle count.
//!
//! **STARK-only is non-negotiable.** The reviewer's G7 directive is to
//! avoid the SP1 SNARK terminal wrap entirely (see
//! `~/.claude/notes/sacredvote-whitepaper-v6-reviewer-2026-05-01.md` G7).
//! We use `prove()` not `prove_compressed()` and we definitely do not
//! call `prove_groth16()` / `prove_plonk()`.
//!
//! Usage:
//!     bench --runs 5                  # 5 measurement runs, print p50/p95
//!     bench --runs 1 --execute-only   # skip prove(), just measure cycles
//!
//! Fixture minting uses `rand::thread_rng()`; the proving run is
//! deterministic given the fixtures, so the only source of variance in
//! reported timings should be the host's wall-clock noise.

use std::time::Instant;

use anyhow::{Context, Result};
use clap::Parser;
use ml_dsa::{signature::Signer, MlDsa65, KeyGen, KeyPair};
use ml_kem::{kem::Encapsulate, KemCore, MlKem768};
use rand::rngs::OsRng;
use serde::Serialize;
use sp1_sdk::{include_elf, ProverClient, SP1Stdin};

/// Path that `sp1-build` writes the program ELF to. The macro resolves at
/// compile time relative to this script crate's manifest.
pub const PROGRAM_ELF: &[u8] = include_elf!("sacredvote-pq-bench-program");

#[derive(Parser, Debug)]
#[command(version, about = "SP1-STARK-only PQ-bench harness", long_about = None)]
struct Args {
    /// Number of proving runs. Reported numbers are p50 + p95 + min/max.
    #[arg(long, default_value_t = 3)]
    runs: u32,

    /// Skip prove(); only execute the program and report cycle count.
    /// Useful for fast iteration on the program logic before committing
    /// to a multi-minute proving run.
    #[arg(long)]
    execute_only: bool,

    /// JSON output path. Defaults to stdout.
    #[arg(long)]
    out: Option<String>,
}

#[derive(Serialize)]
struct StubInputs {
    hybrid_shared_secret: [u8; 32],
    transcript_hash: [u8; 32],
    ml_kem_ciphertext: Vec<u8>,
    ml_kem_decap_key: Vec<u8>,
    ml_dsa_msg: Vec<u8>,
    ml_dsa_sig: Vec<u8>,
    ml_dsa_verifying_key: Vec<u8>,
}

#[derive(Serialize, Default)]
struct RunReport {
    runs: u32,
    cycles: u64,
    prove_seconds_p50: f64,
    prove_seconds_p95: f64,
    prove_seconds_min: f64,
    prove_seconds_max: f64,
    prove_seconds_each: Vec<f64>,
    elf_bytes: usize,
    sp1_sdk_version: String,
    notes: &'static str,
}

fn mint_fixtures() -> Result<StubInputs> {
    let mut rng = OsRng;

    // ML-KEM-768 keypair + encapsulation.
    let (dk, ek) = MlKem768::generate(&mut rng);
    let (ct, ss_alice) = ek.encapsulate(&mut rng).expect("ml-kem encapsulate");

    // ML-DSA-65 keypair + signature on a fixed message.
    let kp: KeyPair<MlDsa65> = MlDsa65::key_gen(&mut rng);
    let msg = b"sacredvote-pq-bench/handshake-finished".to_vec();
    let sig = kp.signing_key().sign(&msg);

    // The hybrid shared secret in the real verifier is the post-combiner
    // output of (X25519 secret || ML-KEM-768 secret). For the bench we
    // just XOR a fresh 32-byte vector with the ML-KEM secret to keep
    // the shape but skip the X25519 half (its cost is negligible vs ML-KEM).
    let mut hybrid = [0u8; 32];
    let ss_bytes: [u8; 32] = ss_alice.into();
    hybrid.copy_from_slice(&ss_bytes);

    let transcript_hash = sha256_of(b"sacredvote-pq-bench/server-finished");

    Ok(StubInputs {
        hybrid_shared_secret: hybrid,
        transcript_hash,
        ml_kem_ciphertext: ct.as_bytes().to_vec(),
        ml_kem_decap_key: dk.as_bytes().to_vec(),
        ml_dsa_msg: msg,
        ml_dsa_sig: sig.encode().to_vec(),
        ml_dsa_verifying_key: kp.verifying_key().encode().to_vec(),
    })
}

fn sha256_of(input: &[u8]) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(input);
    h.finalize().into()
}

fn main() -> Result<()> {
    sp1_sdk::utils::setup_logger();
    let args = Args::parse();

    let inputs = mint_fixtures().context("minting test fixtures")?;
    let mut stdin = SP1Stdin::new();
    stdin.write(&inputs);

    let client = ProverClient::from_env();

    // ---- Always do at least one execute pass: cycle count is cheap and
    //      tells us whether the program even runs end-to-end before we
    //      commit to a multi-minute prove() run.
    let exec_start = Instant::now();
    let (_public_values, exec_report) = client
        .execute(PROGRAM_ELF, &stdin)
        .run()
        .context("sp1 execute")?;
    let exec_seconds = exec_start.elapsed().as_secs_f64();
    let cycles = exec_report.total_instruction_count();
    eprintln!(
        "execute: {} cycles in {:.3}s",
        cycles, exec_seconds
    );

    if args.execute_only {
        let report = RunReport {
            runs: 1,
            cycles,
            elf_bytes: PROGRAM_ELF.len(),
            sp1_sdk_version: env!("CARGO_PKG_VERSION").into(),
            notes: "execute-only: no prove() run, no STARK proof produced",
            ..Default::default()
        };
        return emit_report(&args, &report);
    }

    // ---- STARK-only proving runs ------------------------------------
    // We use `prove()` (NOT `prove_compressed()` / `prove_groth16()` /
    // `prove_plonk()`) so the terminal layer stays a STARK. This is
    // exactly the configuration the v6 architecture commits to.
    let pk = {
        let setup_start = Instant::now();
        let (pk, _vk) = client.setup(PROGRAM_ELF);
        eprintln!("setup: {:.3}s", setup_start.elapsed().as_secs_f64());
        pk
    };

    let mut prove_seconds_each = Vec::with_capacity(args.runs as usize);
    for run in 1..=args.runs {
        let start = Instant::now();
        let _proof = client
            .prove(&pk, &stdin)
            .core() // STARK-only — no SNARK wrap
            .run()
            .with_context(|| format!("sp1 prove (run {run})"))?;
        let elapsed = start.elapsed().as_secs_f64();
        eprintln!("prove run {run}/{}: {:.3}s", args.runs, elapsed);
        prove_seconds_each.push(elapsed);
    }

    let mut sorted = prove_seconds_each.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let p = |q: f64| {
        let idx = ((sorted.len() as f64 - 1.0) * q).round() as usize;
        sorted[idx]
    };

    let report = RunReport {
        runs: args.runs,
        cycles,
        prove_seconds_p50: p(0.50),
        prove_seconds_p95: p(0.95),
        prove_seconds_min: *sorted.first().unwrap(),
        prove_seconds_max: *sorted.last().unwrap(),
        prove_seconds_each,
        elf_bytes: PROGRAM_ELF.len(),
        sp1_sdk_version: env!("CARGO_PKG_VERSION").into(),
        notes: "STARK-only (core), no SNARK wrap. Reviewer G7 compliant.",
    };
    emit_report(&args, &report)
}

fn emit_report(args: &Args, report: &RunReport) -> Result<()> {
    let json = serde_json::to_string_pretty(report)?;
    match &args.out {
        Some(p) => std::fs::write(p, json).context("writing report"),
        None => {
            println!("{json}");
            Ok(())
        }
    }
}
