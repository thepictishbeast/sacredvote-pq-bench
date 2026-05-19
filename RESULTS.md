# Results

Append a new row under `## Measurements` for every bench invocation.
Do not edit prior rows. The append-only history is the audit trail
that lets us see when a regression landed (or when a precompile bought
us a 10× speedup).

## Format

```
### YYYY-MM-DD — short description (host)

- **Host:** make/model, CPU, cores/threads, RAM, OS
- **SP1 toolchain:** version (output of `sp1up --version`)
- **Workspace versions:** ml-kem X.Y.Z, ml-dsa A.B.C, sp1-sdk W.X.Y
- **Runs:** N
- **Cycles:** C
- **p50 / p95 / min / max:** floats in seconds
- **Notes:** what changed since last run, what to look for next
- **Raw JSON:** paste of the bench output
```

## Decision rule (copied from README.md for convenience)

| p95 prove time | Implication |
|---|---|
| ≤ 30s | Voter-laptop proving is viable. Privacy property holds end-to-end. |
| 30–90s | Marginal. Need UX research to confirm voters tolerate this wait. |
| 90s–5min | Voter-laptop proving is dead. Move to delegated proving (G5). |
| > 5min | Architecture rethink. |

## Measurements

### 2026-05-19 — first VPS execute-only run (host=vps, prove deferred)

- **Host:** sacredvote prod VPS, Intel Xeon Skylake (IBRS), 2 cores / 4
  threads @ 2.594 GHz, 7.7 GiB RAM + 8 GiB swap, Debian 12 (bookworm).
- **SP1 toolchain:** `cargo-prove sp1 (2a51f3d 2025-12-15)` with
  succinct rustc v5 (toolchain `JG95iJbUxS`, "rustc 1.91.1-dev").
- **Workspace versions:** sp1-zkvm 5.2, sp1-sdk 5.2.4, ml-kem 0.2.3,
  ml-dsa 0.0.4, aes-gcm 0.10.3, hkdf 0.12.4, sha2 0.10.
- **Runs:** 1 (execute-only, no prove() — see Notes).
- **Cycles:** 7,926,135 (~7.9 M).
- **Execute wall-clock:** 1.526 s.
- **ELF size:** 210,204 bytes.
- **p50 / p95 / min / max:** N/A — no STARK proof produced this run.
- **Notes:** Execute-only. The bitrot-recovery work happened today
  — see commits in this push. Full `prove()` is deliberately deferred
  off the prod VPS: per `~/.claude/projects/-/memory/project_pq_bench_bitrot.md`,
  prove() at this cycle count would compete with sacredvote for RAM
  even under a 4 GiB `systemd-run` cap, and the v6 reviewer (G4) target
  was a commodity laptop anyway. Next step is either (a) run prove on
  a laptop with the SP1 toolchain installed, or (b) explicit user-OK
  for a containment-budgeted prove() run on the VPS. The
  `sp1_sdk_version` field in the JSON below reads "0.1.0" — that's
  `env!("CARGO_PKG_VERSION")` of the bench crate, not the actual SP1
  SDK version; the real SP1 SDK is 5.2.4 (see Workspace versions
  above). Field is misleading; tracked but not refactored in this row.
- **Raw JSON:**

```json
{
  "runs": 1,
  "cycles": 7926135,
  "prove_seconds_p50": 0.0,
  "prove_seconds_p95": 0.0,
  "prove_seconds_min": 0.0,
  "prove_seconds_max": 0.0,
  "prove_seconds_each": [],
  "elf_bytes": 210204,
  "sp1_sdk_version": "0.1.0",
  "notes": "execute-only: no prove() run, no STARK proof produced"
}
```

The scaffold compiles cleanly and runs end-to-end. Run on a commodity
laptop with `sp1up` installed and append the first prove() row.
