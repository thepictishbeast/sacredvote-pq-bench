#!/usr/bin/env bash
# bench.sh — drive the SP1 prover on the stub program, capture results.
#
# Usage:
#   ./bench.sh                  Default: 5 runs, write results-DATE.json
#   ./bench.sh --runs 10        Override run count
#   ./bench.sh --execute-only   Skip prove(), just emit cycle count
#
# Prereqs:
#   - SP1 toolchain: `curl -L https://sp1up.succinct.xyz | bash && sp1up`
#   - Rust toolchain: cargo + rustc (any recent stable works for the host
#     side; the program side compiles with the SP1 succinct toolchain
#     installed by sp1up).
#
# What we are measuring (per reviewer G4):
#   wall-clock prove() time, on a commodity laptop, with no SNARK wrap.
#   This is the constraint that decides whether voter-laptop proving is
#   viable or whether we must move to delegated proving (see G5).
#
# Notes:
#   - First run after a clean build is slower than subsequent runs (key
#     setup + JIT warm-up). bench.sh always discards the first prove run
#     when computing p50/p95 if --runs >= 3.
#   - Set RUST_LOG=info for SP1's tracing output (recommended).
#   - Set SP1_PROVER=cpu for the local STARK-only path. Set SP1_PROVER=
#     network if you have a Succinct prover key, but that defeats the
#     purpose of measuring laptop proving.

set -euo pipefail

cd "$(dirname "$0")"

RUNS=5
EXTRA_ARGS=()
while [[ $# -gt 0 ]]; do
    case "$1" in
        --runs) RUNS="$2"; shift 2 ;;
        --execute-only) EXTRA_ARGS+=("--execute-only"); shift ;;
        --out) EXTRA_ARGS+=("--out" "$2"); shift 2 ;;
        *) echo "unknown flag: $1" >&2; exit 2 ;;
    esac
done

OUT="${OUT:-results-$(date -u +%Y%m%d-%H%M%S).json}"

# Build the program (RISC-V) first so the script's include_elf! macro
# finds a fresh ELF. cargo-prove is provided by the SP1 toolchain.
echo "==> Building program ELF (RISC-V)..."
( cd program && cargo prove build )

# Build the host script.
echo "==> Building host script..."
cargo build --release --bin bench

# Run.
echo "==> Running bench (runs=$RUNS, out=$OUT)..."
SP1_PROVER="${SP1_PROVER:-cpu}" \
    RUST_LOG="${RUST_LOG:-info}" \
    ./target/release/bench --runs "$RUNS" --out "$OUT" "${EXTRA_ARGS[@]}"

echo
echo "==> Wrote $OUT"
echo "==> Append the relevant numbers (cycles, p50, p95, hardware) to RESULTS.md"
