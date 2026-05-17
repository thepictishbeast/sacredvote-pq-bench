> ## In Development — Unverified
>
> This is an active engineering tree, not a release. APIs, file
> layouts, and behavior may change between commits. Tests pass
> locally; CI may or may not be green at any given moment.
>
> Source is published publicly for transparency and audit.

# sacredvote-pq-bench

> SP1 STARK-only prover-time measurement for the Sacred.Vote post-quantum
> verifier circuit.

This repo answers exactly one question: **how long does it take for a
commodity laptop to STARK-prove a stub program that performs the cryptographic
operations the real Sacred.Vote PQ verifier circuit will perform?**

It is the experiment the v6 whitepaper external reviewer (2026-05-01)
flagged as the highest-information-density next experiment in the
cryptographic critical path. See [`METHODOLOGY.md`](METHODOLOGY.md) for
why this is load-bearing and [`RESULTS.md`](RESULTS.md) for measured
numbers.

## What it does

A two-crate workspace:

| Crate | Target | Purpose |
|---|---|---|
| `program/` | `riscv32im-succinct-zkvm-elf` | Stub TLS-1.3 handshake + ML-KEM-768 decap + ML-DSA-65 verify, run inside the SP1 zkVM. |
| `script/`  | host                          | Mints fresh fixtures, runs the prover, prints p50/p95/min/max wall-clock + cycle count. |

The program does **not** validate a real TLS transcript. It does the
*shape* of the work the real verifier will do: HKDF-Expand the way TLS 1.3
key schedule does, run one AES-256-GCM seal/open, decap a real
ML-KEM-768 ciphertext, verify a real ML-DSA-65 signature, and commit a
SHA-256 hash of the inputs. The proving cost we measure here is the
*lower bound* on what the real circuit will cost — the production
verifier will be at least this expensive, plus the cost of validating
the actual transcript bytes.

## Why STARK-only (no SNARK wrap)

The standard zkVM pattern is "fast STARK inner + cheap Groth16 outer."
That collapses post-quantum soundness because Groth16 relies on
discrete-log-style assumptions that break under quantum attack. We are
on a chain we control (Sacred.Vote / Chain A / Chain B), so we can
absorb the proof-size and verification-gas cost of staying STARK-only.

The `script/` runner uses `prove(...).core()`, which is SP1's
STARK-only path. It deliberately does **not** call
`prove_compressed()`, `prove_groth16()`, or `prove_plonk()`.

This is reviewer directive G7. See
`~/.claude/notes/sacredvote-whitepaper-v6-reviewer-2026-05-01.md`.

## Running

```bash
# 1. Install the SP1 toolchain (one-time):
curl -L https://sp1up.succinct.xyz | bash
sp1up                              # downloads & installs the succinct toolchain

# 2. Run the bench:
./bench.sh --runs 5
```

Bench output is JSON:

```json
{
  "runs": 5,
  "cycles": 32178400,
  "prove_seconds_p50": 38.412,
  "prove_seconds_p95": 41.107,
  "prove_seconds_min": 36.998,
  "prove_seconds_max": 41.107,
  "prove_seconds_each": [38.412, 36.998, 39.101, 41.107, 38.220],
  "elf_bytes": 524288,
  "sp1_sdk_version": "0.1.0",
  "notes": "STARK-only (core), no SNARK wrap. Reviewer G7 compliant."
}
```

## Decision rule

The number that matters is **`prove_seconds_p95` on a typical voter
laptop** (commodity CPU, no GPU, 16 GB RAM is the assumed floor).

Decision rule the v6 architecture commits to:

| p95 prove time | Implication |
|---|---|
| ≤ 30s | Voter-laptop proving is viable. Privacy property holds end-to-end. |
| 30–90s | Marginal. Need UX research to confirm voters tolerate this wait. |
| 90s–5min | Voter-laptop proving is dead. Move to delegated proving (G5). Wealth-neutrality breaks unless mitigations land. |
| > 5min | Architecture rethink. Possibly drop the in-circuit ML-DSA verify in favor of attestation-by-staked-prover. |

This bench is the input to that decision. Rerun whenever:
- SP1 ships a precompile that affects ML-KEM or ML-DSA (likely huge
  speedup — ML-KEM precompiles dropped median proving time by 8–20x in
  earlier RustCrypto crates).
- The `ml-kem` / `ml-dsa` crates change their core lattice arithmetic.
- We move from ML-KEM-768 to ML-KEM-1024 (G9 decision pending).
- The chain bumps to ML-DSA-87 from ML-DSA-65.

## Status

Scaffold is complete; it has not yet been measured. Measurement requires
running on a commodity laptop with the SP1 toolchain installed —
deliberately *not* on the production VPS, because the VPS hardware
profile is irrelevant to the voter-UX question. Tim's machine or any dev
laptop is the right host.

When numbers land, append a row to [`RESULTS.md`](RESULTS.md) with
hardware, SP1 version, crate versions, and the JSON.
