//! Build script for the SP1 PQ-bench host.
//!
//! Invokes `sp1_build::build_program("../program")` so the program ELF
//! is (re)built with the SP1 succinct toolchain and the
//! `SP1_ELF_sacredvote-pq-bench-program` env var is set at compile time.
//! Without this, the `include_elf!()` macro in `script/src/main.rs`
//! resolves to `env!("SP1_ELF_...")` and fails with "environment
//! variable not defined at compile time."
//!
//! Set `SP1_SKIP_PROGRAM_BUILD=true` in the environment to skip the
//! program rebuild — useful when iterating on the host side after a
//! known-good program ELF has already been produced via `cargo prove
//! build` from `program/`. The env var pointer is still emitted.

fn main() {
    sp1_build::build_program("../program");
}
