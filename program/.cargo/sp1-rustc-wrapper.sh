#!/usr/bin/env bash
# Wrapper around SP1's `succinct` rustc that filters out flags the
# succinct fork doesn't yet support but the host's modern cargo
# (1.81+) emits unconditionally.
#
# Filtered flags:
#   --remap-path-scope=...   stabilized rustc 1.81 / 2024; succinct
#                            v5.x and v6.x rustc forks reject it.
#
# Why a wrapper not a cargo config: the cargo profile knob
# `trim-paths` (which would suppress the flag at the source) is still
# unstable; enabling it requires nightly cargo, which we don't have
# on the prod VPS. A 3-line shim is simpler than a toolchain swap.

set -e

# First positional arg is rustc itself when invoked via RUSTC_WRAPPER;
# when invoked via target.<triple>.linker / .rustc cargo passes the
# rustc path implicitly via the script binding. We accept both.
if [[ "$1" == *rustc* ]]; then
  RUSTC_BIN="$1"
  shift
else
  RUSTC_BIN="${RUSTC_BIN:-$HOME/.rustup/toolchains/succinct/bin/rustc}"
fi

args=()
for arg in "$@"; do
  case "$arg" in
    --remap-path-scope=*) ;;  # drop
    *) args+=("$arg") ;;
  esac
done

exec "$RUSTC_BIN" "${args[@]}"
