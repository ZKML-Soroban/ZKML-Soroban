//! Groth16 proving on a remote proof market.
//!
//! The market itself sits behind [`Market`], so the request lifecycle and the
//! checks on what comes back are the same for the real Boundless client
//! (`boundless` feature) and for the mock the tests use. Nothing a market
//! returns is trusted: the seal must be a 260-byte Groth16 seal for this RISC
//! Zero version, the journal must equal native inference, and the assembled
//! bundle must pass [`verify_bundle`] before it is returned.

use std::time::{Duration, Instant};

use zkml_common::bundle::{BundleMeta, ProofSystemId, ProveTimings, VerificationBundleV2};
use zkml_common::commitment::Opening;
use zkml_common::fixed_point::FixedPoint;
use zkml_common::journal::JournalV1;
use zkml_common::models::Model;
use zkml_common::risc0::SEAL_LEN;

use crate::prover::{count_cycles, journal_for, verify_bundle, ProveError};

/// What a market is asked to prove: the guest identified by `image_id`, run on
/// the same private inputs the local prover writes (model, inputs, opening).
pub struct RemoteRequest<'a> {
    pub image_id: [u8; 32],
    pub model: &'a Model,
    pub inputs: &'a [FixedPoint],
    pub opening: &'a Opening,
    /// User cycles from a local execution, for pricing.
    pub cycles: u64,
}

/// A fulfilled request: the Groth16 seal and the journal it proves.
pub struct Fulfilled {
    pub seal: Vec<u8>,
    pub journal: Vec<u8>,
}

/// A proof market. Implementations block until done.
pub trait Market {
    /// Submit a request for a standalone Groth16 proof; return its id.
    fn submit(&self, request: &RemoteRequest<'_>) -> Result<String, ProveError>;
    /// Wait for the request to be fulfilled, at most `timeout`.
    fn wait(&self, request_id: &str, timeout: Duration) -> Result<Fulfilled, ProveError>;
}

/// Prove `model` on `inputs` through `market` and return a checked v2 bundle.
///
/// The guest runs locally first, without proving, for the cycle count and the
/// journal native inference predicts. The request id is printed as soon as it
/// exists, so an interrupted run can be looked up on the market.
pub fn prove_remote(
    model: &Model,
    inputs: &[FixedPoint],
    opening: &Opening,
    image_id: [u8; 32],
    market: &dyn Market,
    timeout: Duration,
) -> Result<VerificationBundleV2, ProveError> {
    let started = Instant::now();
    let cycles = count_cycles(model, inputs, opening)?;
    let expected = journal_for(model, inputs, opening)?;

    let request_id = market.submit(&RemoteRequest {
        image_id,
        model,
        inputs,
        opening,
        cycles: cycles.user_cycles,
    })?;
    eprintln!("submitted proof request {request_id}");
    let submitted = Instant::now();
    let fulfilled = market.wait(&request_id, timeout)?;

    if fulfilled.seal.len() != SEAL_LEN {
        return Err(ProveError::RemoteWrongProofType {
            request_id,
            detail: format!(
                "the seal is {} bytes, not the {SEAL_LEN} of a Groth16 seal",
                fulfilled.seal.len()
            ),
        });
    }
    let journal = JournalV1::decode(&fulfilled.journal)?;
    if journal != expected {
        return Err(ProveError::JournalMismatch(format!(
            "request {request_id} proved {journal:?}, native inference produced {expected:?}"
        )));
    }

    let bundle = VerificationBundleV2::new(
        ProofSystemId::Risc0Groth16,
        image_id,
        fulfilled.seal,
        fulfilled.journal,
        BundleMeta {
            prover_version: env!("CARGO_PKG_VERSION").to_string(),
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0),
            cycles: cycles.padded_cycles,
            timings: ProveTimings {
                prove_and_compress_ms: submitted.elapsed().as_millis() as u64,
                total_ms: started.elapsed().as_millis() as u64,
            },
        },
    )?;
    // The selector and the proof itself, exactly as the contract checks them.
    verify_bundle(&bundle).map_err(|e| ProveError::RemoteWrongProofType {
        request_id: request_id.clone(),
        detail: e.to_string(),
    })?;
    Ok(bundle)
}
