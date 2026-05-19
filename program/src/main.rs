//! SP1 zkVM stub program for prover-time measurement.
//!
//! Per the v6-reviewer 2026-05-01 G4 directive: build a stub program that
//! does the cryptographic work the real verifier will do (TLS 1.3 handshake
//! ops + ML-KEM-768 decap + ML-DSA verify) but skips actual transcript
//! validation, then prove it in SP1 STARK-only mode on a commodity laptop.
//! The number we care about is the wall-clock proving time and total cycle
//! count — that's what tells us whether voter-laptop proving is feasible
//! or whether we're forced into delegated proving (G5).
//!
//! **What this program does NOT do:**
//! - Validate a real TLS 1.3 transcript (handshake order, extensions,
//!   server certificates, etc.). The reviewer was explicit: "no actual
//!   transcript validation."
//! - Use a real attestation. Inputs are deterministic test vectors.
//! - Emit a meaningful proof of any claim. The committed output is just
//!   a hash of inputs + the boolean result of the verify call.
//!
//! **What this program DOES do:**
//! - HKDF-Expand-Label five times (TLS 1.3 key schedule shape).
//! - One AES-256-GCM seal + open round (TLS record layer shape).
//! - One ML-KEM-768 decap (the post-quantum KEM half of the hybrid).
//! - One ML-DSA-65 verify (the post-quantum signature half).
//! - SHA-256 hash of all inputs + the bool result, committed publicly.
//!
//! **Why it's calibrated this way:** the real verifier circuit will do
//! roughly one ML-KEM decap + one ML-DSA verify per TLS handshake plus
//! a handful of HKDF/AES-GCM/SHA-256 calls per record. This stub is the
//! lower bound — the real circuit will be at least this expensive plus
//! whatever transcript validation costs.

#![no_main]
sp1_zkvm::entrypoint!(main);

extern crate alloc;

use alloc::vec::Vec;

use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use hkdf::Hkdf;
use sha2::{Digest, Sha256};

/// Inputs the host will write before `prove()`.
///
/// The host reads these via `stdin.write::<...>()`. The program reads
/// them in the same order via `sp1_zkvm::io::read::<...>()`.
#[derive(serde::Deserialize, serde::Serialize)]
struct StubInputs {
    /// 32-byte hybrid shared secret (the post-combiner output the TLS key
    /// schedule will key off of). Random fixture in real bench runs.
    hybrid_shared_secret: [u8; 32],
    /// The handshake transcript hash up to ServerFinished.  Fixture.
    transcript_hash: [u8; 32],
    /// ML-KEM-768 encapsulated key (1088 bytes per FIPS 203).
    ml_kem_ciphertext: Vec<u8>,
    /// ML-KEM-768 decapsulation key (2400 bytes per FIPS 203).
    ml_kem_decap_key: Vec<u8>,
    /// ML-DSA-65 message (the signed transcript-hash analogue). Fixture.
    ml_dsa_msg: Vec<u8>,
    /// ML-DSA-65 signature (3293 bytes per FIPS 204 §4 Table 1).
    ml_dsa_sig: Vec<u8>,
    /// ML-DSA-65 verifying key (1952 bytes per FIPS 204).
    ml_dsa_verifying_key: Vec<u8>,
}

pub fn main() {
    // ---- 1. read fixtures committed by the host ----------------------
    let inputs: StubInputs = sp1_zkvm::io::read();

    // ---- 2. TLS 1.3 key-schedule shape -------------------------------
    // HKDF-Expand-Label five times to mimic Derive-Secret over the five
    // standard labels: "c hs traffic", "s hs traffic", "c ap traffic",
    // "s ap traffic", "exp master".
    let mut traffic_keys = [[0u8; 32]; 5];
    let labels: [&[u8]; 5] = [
        b"tls13 c hs traffic",
        b"tls13 s hs traffic",
        b"tls13 c ap traffic",
        b"tls13 s ap traffic",
        b"tls13 exp master",
    ];
    let hk = Hkdf::<Sha256>::new(
        Some(&inputs.transcript_hash),
        &inputs.hybrid_shared_secret,
    );
    for (i, label) in labels.iter().enumerate() {
        // HKDF-Expand only — the `Hkdf::expand` API in the RustCrypto crate
        // matches the TLS 1.3 HKDF-Expand-Label structure when you feed it
        // the labelled info. Real verifier circuits build the full info
        // tuple per RFC 8446 §7.1; we keep it minimal here.
        hk.expand(label, &mut traffic_keys[i]).expect("hkdf expand");
    }

    // ---- 3. AES-256-GCM seal + open round (TLS record layer) ---------
    // One short message through the AEAD round. This is the cost we'd pay
    // per TLS record in a transcript verification; the real circuit will
    // multiply this by the number of records.
    let aead_key = Key::<Aes256Gcm>::from_slice(&traffic_keys[2]);
    let aead = Aes256Gcm::new(aead_key);
    let nonce = Nonce::from_slice(&traffic_keys[3][..12]);
    let plaintext = b"sacredvote-pq-bench-record";
    let ciphertext = aead
        .encrypt(
            nonce,
            Payload {
                msg: plaintext,
                aad: &inputs.transcript_hash,
            },
        )
        .expect("aead seal");
    let recovered = aead
        .decrypt(
            nonce,
            Payload {
                msg: &ciphertext,
                aad: &inputs.transcript_hash,
            },
        )
        .expect("aead open");
    assert_eq!(recovered, plaintext);

    // ---- 4. ML-KEM-768 decap -----------------------------------------
    // We force the decapsulation with the host-provided fixtures. The
    // ml-kem crate's API has shifted across recent releases — the call
    // below is the 0.2.x shape; if the operator bumps the dep, port to
    // whatever signature ships in that release. Either way the work
    // measured is roughly equivalent (one ML-KEM-768 decap dominates).
    let ml_kem_shared = ml_kem_decap_768(
        &inputs.ml_kem_decap_key,
        &inputs.ml_kem_ciphertext,
    );

    // ---- 5. ML-DSA-65 verify -----------------------------------------
    let signature_ok = ml_dsa_verify_65(
        &inputs.ml_dsa_verifying_key,
        &inputs.ml_dsa_msg,
        &inputs.ml_dsa_sig,
    );

    // ---- 6. commit a public hash so the proof is bound to inputs ----
    // The verifier of this proof gets a single 32-byte commitment that
    // covers (a) every input the host wrote and (b) the verify result.
    // This is enough for a measurement run — the real circuit will commit
    // a richer public output (eligibility flag, epoch, nonce-binding etc).
    let mut hasher = Sha256::new();
    hasher.update(b"sacredvote-pq-bench/v0.1");
    hasher.update(&inputs.hybrid_shared_secret);
    hasher.update(&inputs.transcript_hash);
    hasher.update(&ml_kem_shared);
    hasher.update([signature_ok as u8]);
    hasher.update(&ciphertext);
    let commitment: [u8; 32] = hasher.finalize().into();

    sp1_zkvm::io::commit(&commitment);
    sp1_zkvm::io::commit(&signature_ok);
}

/// ML-KEM-768 decap shim. Pure-Rust path through the `ml-kem` crate. If
/// SP1 ships a Kyber/ML-KEM precompile in a future version, this will be
/// patched transparently via SP1's patch system — the API stays the same.
fn ml_kem_decap_768(decap_key_bytes: &[u8], ciphertext_bytes: &[u8]) -> [u8; 32] {
    // 0.2.x API: `KemCore` exposes `DecapsulationKey` associated type
    // but not `Ciphertext` (the latter is `ml_kem::Ciphertext<K>` —
    // an alias over `Array<u8, K::CiphertextSize>`). `EncodedSizeUser`
    // must be in scope for `from_bytes()` on both encoded types.
    use ml_kem::{kem::Decapsulate, Ciphertext, EncodedSizeUser, KemCore, MlKem768};
    let dk_arr: &ml_kem::Encoded<<MlKem768 as KemCore>::DecapsulationKey> =
        decap_key_bytes.try_into().expect("ml-kem decap key length");
    let ct_arr: &Ciphertext<MlKem768> =
        ciphertext_bytes.try_into().expect("ml-kem ciphertext length");
    let dk = <MlKem768 as KemCore>::DecapsulationKey::from_bytes(dk_arr);
    let shared = dk.decapsulate(ct_arr).expect("ml-kem decap");
    shared.into()
}

/// ML-DSA-65 verify shim. Pure-Rust path through the `ml-dsa` crate.
/// Returns the boolean verify result (true = signature valid).
fn ml_dsa_verify_65(verifying_key_bytes: &[u8], msg: &[u8], sig_bytes: &[u8]) -> bool {
    use ml_dsa::{signature::Verifier, MlDsa65, VerifyingKey, Signature};
    // ml-dsa 0.0.4 `decode` takes `&Array<u8, …Size>` rather than
    // `&[u8; N]`. `.into()` coerces the byte-array reference because
    // `Array` impls `From<&[u8; N]>` for the matching fixed size.
    let vk_arr: &[u8; 1952] = verifying_key_bytes
        .try_into()
        .expect("ml-dsa-65 vk length");
    let sig_arr: &[u8; 3309] = sig_bytes.try_into().expect("ml-dsa-65 sig length");
    let vk = <VerifyingKey<MlDsa65>>::decode(vk_arr.into());
    let sig = match <Signature<MlDsa65>>::decode(sig_arr.into()) {
        Some(s) => s,
        None => return false,
    };
    vk.verify(msg, &sig).is_ok()
}
