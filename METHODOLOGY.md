# Methodology

This document is the load-bearing rationale behind `sacredvote-pq-bench`.
If you change the bench (different crypto, different framework, different
runner), update this document at the same time.

## Origin

This bench exists because the external reviewer of the v6 whitepaper
(2026-05-01) flagged voter-laptop prover time as the constraint most
likely to break the entire UX assumption of Sacred.Vote's
wealth-neutral PoP architecture.

> SP1 Hypercube's ~10s for an Ethereum block on 16x RTX 5090 cluster
> does not translate to laptop-friendly proving for a TLS+PQ verifier.
> The actual question is whether a single laptop CPU (no GPU) can prove
> a TLS 1.3 + ML-KEM-768 + ML-DSA verifier in under N seconds, where N
> is voter UX tolerance.
>
> *Recommend: before circuit design, build a stub program (a TLS handshake
> simulator with ML-KEM-768 decap and ML-DSA verify, no actual transcript
> validation) and prove it in SP1 STARK-only mode on a commodity laptop.
> Measure. Decide UX tolerance against measured reality before
> architectural commitments depend on it.*

The reviewer's framing is correct: **measurement before architecture.**
The proof-of-personhood + privacy story collapses if voter laptops cannot
prove. Mitigations exist — delegated proving, MPC-based proving,
TEE-attested provers — but every one of them either costs money (breaks
wealth-neutrality) or trusts a third party (breaks the privacy
property). Knowing the prove time *now* lets us either commit to the
laptop-prover story with confidence or budget for a contingency before
the architecture commits to a path that doesn't work.

## What we measure

**Wall-clock prove() time + total RISC-V cycle count**, in SP1 with the
STARK-only terminal layer (`prove(...).core()` in the SDK), on commodity
laptop hardware.

We measure five values:

- `prove_seconds_p50` — median; what most voters see.
- `prove_seconds_p95` — 95th percentile; what slow voters see.
- `prove_seconds_min` / `_max` — variance bounds.
- `cycles` — total RISC-V instruction count. Linear-ish predictor of
  prove time across hardware.

We deliberately do **not** measure:

- Verifier time (irrelevant on Sacred.Vote chain — verifiers are stake-
  weighted nodes, not voter laptops).
- Proof size (relevant for chain bandwidth, but not the gating constraint).
- Setup time (one-time cost, amortized).

## What the program does

The program crate runs five categories of work, each chosen to match the
real verifier circuit's expected workload:

### TLS 1.3 key schedule (HKDF-Expand × 5)

The TLS 1.3 key schedule derives five distinct traffic secrets via
HKDF-Expand-Label (RFC 8446 §7.1). We approximate this by calling
`Hkdf::<Sha256>::expand` five times with the actual TLS-1.3 label
strings. Real verifier circuits build the full info tuple (label length,
"tls13 " prefix, label, transcript hash); we keep the call minimal
because the label-tuple bytes don't materially affect proving cost —
the cost is dominated by the SHA-256 inside HMAC inside HKDF.

### AES-256-GCM seal + open (one round)

One `Aes256Gcm::encrypt` followed by one `Aes256Gcm::decrypt` over a
short message with the transcript hash as AAD. This is the per-record
cost on the TLS record layer. The real verifier multiplies this by the
number of records in the transcript; we do exactly one round so the
measurement is a per-record cost we can extrapolate from.

### ML-KEM-768 decap

The post-quantum half of the TLS 1.3 hybrid key exchange (the X25519
half is negligible by comparison and we skip it). One decap per
handshake.

We run the decap with **fixtures minted by the host** — a fresh
ML-KEM-768 keypair and ciphertext for every bench invocation. This is
deliberate: it guards against the program being tuned for a specific
fixture's hot path. The decap arithmetic dominates either way.

### ML-DSA-65 verify

One ML-DSA-65 signature verification on a fixed-message fixture. ML-DSA
verify is heavier than X25519 verify by roughly 30-50× in pure-Rust on
commodity hardware; in-circuit the ratio is similar.

### SHA-256 commitment

Final `Sha256::digest` over all inputs and the verify result, committed
as the program's public output. Token gesture toward circuit binding
of inputs.

## What the program deliberately does NOT do

- **Validate a real TLS 1.3 transcript.** No checking that ClientHello
  came before ServerHello, that extensions are well-formed, that the
  certificate chain validates, that the handshake hash matches. These
  are circuit work that the real verifier will do but that we elide
  here per the reviewer's guidance ("no actual transcript validation").
- **Generate a real attestation.** The output is a SHA-256 hash, not
  a meaningful claim about identity, eligibility, or anything else.
- **Use a real TLS notary.** No notary keys; no co-signing path.
  This is a verifier-side benchmark, not a notary-side benchmark.

## What the bench does NOT settle

- **Notary-side proving costs** (TLSNotary v0.2 or Reclaim). Different
  bench. Different document. (Tracked as part of the wire-format
  one-pager.)
- **Eligibility gate cost** (the §2 PoP gate is separate from the §4
  privacy primitive being measured here).
- **Energy budget** (G6) — although the cycle count measured here is a
  proxy. A separate calculation in the whitepaper translates cycles +
  validator count + epoch length into joules-per-tx.
- **Whether ML-KEM-768 or ML-KEM-1024 is the right parameter set** (G9).
  The bench is currently parameterized for 768 because that's the IETF
  hybrid default; rerunning under 1024 is a one-line change in
  `program/src/main.rs` and a re-mint in `script/src/main.rs`.

## Reproducibility expectations

- The bench mints fresh fixtures every run, but the proving run is
  deterministic *given* fixtures. Variance in `prove_seconds_each`
  across runs reflects host-side noise (thermal throttling, scheduler
  jitter, page-cache state), not algorithmic non-determinism.
- All crates are pinned to specific versions in `Cargo.toml`. `Cargo.lock`
  is committed.
- SP1 toolchain version is recorded in the `RESULTS.md` row alongside
  the numbers. Bumping SP1 (especially crossing a precompile change)
  invalidates prior measurements.

## When this bench is wrong

- **If voters use phones, not laptops.** The bench measures laptop
  hardware. A separate phone-class bench will be needed if mobile
  voting becomes the primary surface (it isn't, per the v6 architecture
  and Sacred.Vote roadmap).
- **If we abandon TLS 1.3 in favor of QUIC or a custom transport.**
  Then the handshake-shape work in the program needs rewriting to match
  the actual transport.
- **If a precompile lands for ML-KEM or ML-DSA.** Then this bench
  becomes a *floor* — real prove times will be much faster. Re-run.
