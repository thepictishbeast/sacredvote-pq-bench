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

_(none yet — bench has not been run on real hardware.)_

The scaffold is complete and the host script + program compile against
recent SP1 (5.x). Run on a commodity laptop with `sp1up` installed and
append the first row.
