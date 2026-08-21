//! Proof generation module.
//!
//! Orchestrates the end-to-end proof pipeline: commit to the model and inputs,
//! run inference (natively or inside the RISC Zero zkVM), and package results
//! for on-chain verification.
//!
//! - [`generate_receipt`]: Phase 1 STARK receipt from the zkVM guest (this issue).
//! - [`generate_proof`]: public-input bundle with a placeholder Groth16 proof
//!   until STARK→Groth16 compression lands in issue #11.
//! - [`generate_proof_with_groth16`]: Placeholder for STARK→Groth16 compression
//!   with backend selection (not yet implemented - see TODO comments).

use zkml_common::commitment::{commit_i64, commitment_hash, Commitment};
use zkml_common::fixed_point::FixedPoint;
use zkml_common::models::Model;
use zkml_common::proof::{Groth16Proof, PublicInputs, VerificationBundle};

/// Prover backend selection for Groth16 proof generation.
///
/// # Not Yet Implemented
///
/// Both backends are currently placeholders that return `Err`. The actual
/// STARK→Groth16 compression implementation is deferred pending investigation
/// of the risc0-groth16 3.0.5 and bonsai-sdk 1.1 APIs.
///
/// - `Local`: Uses Docker-based local prover (requires `risc0-groth16` component).
/// - `Bonsai`: Uses RISC Zero's remote proving service (requires API credentials).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProverBackend {
    /// Local Docker-based Groth16 prover.
    Local,
    /// Remote Bonsai proving service.
    Bonsai,
}

/// Flatten model parameters for commitments (shared with the guest).
pub use zkml_common::commitment::model_elements;

/// Commit to a model's parameters (the on-chain `initialize` value).
pub fn model_commitment(model: &Model) -> Commitment {
    commitment_hash(&model_elements(model))
}

/// Commit to a set of input features (a proof public input).
pub fn input_commitment(inputs: &[FixedPoint]) -> Commitment {
    let elements: Vec<i64> = inputs.iter().map(|x| x.value).collect();
    commitment_hash(&elements)
}

/// Generate a verification bundle for `model` evaluated on `inputs`.
///
/// The Groth16 proof bytes are a Phase 1 placeholder; the public inputs and
/// commitments are fully computed so the on-chain interface can be exercised
/// end to end. STARK→Groth16 compression is tracked in issue #11.
pub fn generate_proof(model: &Model, inputs: &[FixedPoint]) -> Result<VerificationBundle, String> {
    // Fallible inference: a feature-count mismatch or an overflow must surface
    // as `Err` on this public API rather than panicking inside the caller.
    let output = crate::inference::try_run_inference(model, inputs).map_err(|e| e.to_string())?;

    let public_inputs = PublicInputs {
        model_hash: model_commitment(model),
        input_hash: input_commitment(inputs),
        output: output.value.to_le_bytes().to_vec(),
    };

    // TODO(#11): replace with a real RISC Zero receipt lowered to Groth16.
    let proof = Groth16Proof { data: Vec::new() };

    Ok(VerificationBundle {
        proof,
        public_inputs,
    })
}

/// Generate a verification bundle with Groth16 proof via STARK→SNARK compression.
///
/// # Not Yet Implemented
///
/// This function is a placeholder for the full STARK→Groth16 compression pipeline.
/// Both backends currently return `Err` with a "not yet implemented" message.
///
/// TODO: Implement actual STARK→Groth16 compression using risc0-groth16 3.0.5 API.
/// The compression pipeline should:
/// 1. Run inference inside the zkVM guest to produce a STARK receipt
/// 2. Convert the STARK receipt to a Groth16 SNARK using the specified backend
/// 3. Serialize the Groth16 proof according to the BN254 wire format
/// 4. Package the proof with public inputs into a VerificationBundle
///
/// # Arguments
///
/// * `model` - The quantized model to evaluate
/// * `inputs` - Input features for inference
/// * `backend` - Prover backend to use for Groth16 compression
///
/// # Feature Flags
///
/// Requires the `groth16` feature. For `ProverBackend::Bonsai`, also requires `bonsai` feature.
///
/// # Errors
///
/// Returns an error if:
/// - Inference fails (feature mismatch, overflow)
/// - zkVM proof generation fails
/// - STARK→Groth16 compression fails
/// - Proof serialization fails
#[cfg(all(feature = "groth16", feature = "zkvm"))]
pub fn generate_proof_with_groth16(
    model: &Model,
    inputs: &[FixedPoint],
    backend: ProverBackend,
) -> Result<VerificationBundle, String> {
    let (receipt, journal) = generate_receipt(model, inputs)?;

    let groth16_proof = match backend {
        ProverBackend::Local => compress_to_groth16_local(&receipt)?,
        #[cfg(feature = "bonsai")]
        ProverBackend::Bonsai => compress_to_groth16_bonsai(&receipt)?,
        #[cfg(not(feature = "bonsai"))]
        ProverBackend::Bonsai => {
            return Err("Bonsai backend requires 'bonsai' feature to be enabled".to_string())
        }
    };

    let public_inputs = PublicInputs {
        model_hash: journal.model_hash,
        input_hash: journal.input_hash,
        output: journal.output.to_le_bytes().to_vec(),
    };

    Ok(VerificationBundle {
        proof: groth16_proof,
        public_inputs,
    })
}

/// Compress a STARK receipt to Groth16 using the local Docker prover.
#[cfg(all(feature = "groth16", feature = "zkvm"))]
fn compress_to_groth16_local(_receipt: &risc0_zkvm::Receipt) -> Result<Groth16Proof, String> {
    // TODO: Implement actual STARK to Groth16 compression using risc0-groth16 3.0.5 API.
    // The shrink_wrap function requires the seal bytes from the receipt, and the
    // resulting Groth16Seal needs to be serialized according to the BN254 wire format.
    // This is deferred pending investigation of the exact risc0-groth16 3.0.5 API surface.
    Err("STARK to Groth16 compression not yet implemented for local backend".to_string())
}

/// Compress a STARK receipt to Groth16 using the Bonsai remote proving service.
#[cfg(all(feature = "groth16", feature = "zkvm", feature = "bonsai"))]
fn compress_to_groth16_bonsai(_receipt: &risc0_zkvm::Receipt) -> Result<Groth16Proof, String> {
    // TODO: Implement actual STARK to Groth16 compression using Bonsai SDK 1.1 API.
    // This requires investigating the exact Bonsai SNARK creation flow and
    // how to serialize the resulting Groth16Seal according to the BN254 wire format.
    // This is deferred pending investigation of the exact Bonsai API surface.
    Err("STARK to Groth16 compression not yet implemented for Bonsai backend".to_string())
}

/// Verification key export is deferred to a follow-up issue.
///
/// The verification key is required by the Soroban contract's `initialize` function
/// to verify Groth16 proofs. This requires implementing VK extraction against
/// risc0-groth16 3.0.5, which needs further research into the specific API surface.
///
/// TODO: Open a follow-up issue to implement verification key export once the
/// risc0-groth16 3.0.5 VK extraction API is documented and understood.

/// Journal fields committed by the zkVM guest (public inputs).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct InferenceJournal {
    /// Commitment to model parameters.
    pub model_hash: Commitment,
    /// Commitment to input features.
    pub input_hash: Commitment,
    /// Raw Q-format integer output (`FixedPoint::value`).
    pub output: i64,
}

#[cfg(feature = "zkvm")]
mod zkvm_prove {
    use super::*;
    use risc0_zkvm::{default_prover, ExecutorEnv, Receipt};
    use zkml_methods::{ZKML_GUEST_ELF, ZKML_GUEST_ID};

    /// Run inference inside the RISC Zero zkVM and return a verified receipt.
    ///
    /// Steps:
    /// 1. Build an executor environment with the model and inputs.
    /// 2. Prove guest execution (`RISC0_DEV_MODE=1` for fast local/CI runs).
    /// 3. Verify the receipt against the guest image ID.
    /// 4. Decode the journal and cross-check against native inference.
    ///
    /// STARK→Groth16 compression is **not** performed here (issue #11).
    pub fn generate_receipt(
        model: &Model,
        inputs: &[FixedPoint],
    ) -> Result<(Receipt, InferenceJournal), String> {
        let inputs_owned: Vec<FixedPoint> = inputs.to_vec();
        let env = ExecutorEnv::builder()
            .write(model)
            .map_err(|e| format!("failed to write model to guest env: {e}"))?
            .write(&inputs_owned)
            .map_err(|e| format!("failed to write inputs to guest env: {e}"))?
            .build()
            .map_err(|e| format!("failed to build guest env: {e}"))?;

        let prover = default_prover();
        let prove_info = prover
            .prove(env, ZKML_GUEST_ELF)
            .map_err(|e| format!("zkVM prove failed: {e}"))?;
        let receipt = prove_info.receipt;

        receipt
            .verify(ZKML_GUEST_ID)
            .map_err(|e| format!("receipt verification failed: {e}"))?;

        let journal = decode_journal(&receipt)?;
        cross_check_native(model, inputs, &journal)?;

        Ok((receipt, journal))
    }

    /// Decode the guest journal into [`InferenceJournal`].
    pub fn decode_journal(receipt: &Receipt) -> Result<InferenceJournal, String> {
        receipt
            .journal
            .decode()
            .map_err(|e| format!("journal decode failed: {e}"))
    }

    fn cross_check_native(
        model: &Model,
        inputs: &[FixedPoint],
        journal: &InferenceJournal,
    ) -> Result<(), String> {
        let native_out = crate::inference::run_inference(model, inputs);
        if journal.output != native_out.value {
            return Err(format!(
                "journal output {} != native inference {}",
                journal.output, native_out.value
            ));
        }
        let expected_model = model_commitment(model);
        if journal.model_hash != expected_model {
            return Err("journal model_hash does not match native model_commitment".into());
        }
        let expected_input = input_commitment(inputs);
        if journal.input_hash != expected_input {
            return Err("journal input_hash does not match native input_commitment".into());
        }
        Ok(())
    }
}

#[cfg(feature = "zkvm")]
pub use zkvm_prove::{decode_journal, generate_receipt};

#[cfg(test)]
mod tests {
    use super::*;
    use zkml_common::models::LogisticRegression;

    fn fp(x: f64) -> FixedPoint {
        FixedPoint::quantize(x)
    }

    #[test]
    fn bundle_is_populated() {
        let model = Model::LogisticRegression(LogisticRegression {
            weights: vec![fp(0.5), fp(-0.25)],
            bias: fp(0.1),
        });
        let inputs = vec![fp(1.0), fp(2.0)];
        let bundle = generate_proof(&model, &inputs).unwrap();
        assert_ne!(bundle.public_inputs.model_hash, [0u8; 32]);
        assert_eq!(bundle.public_inputs.output.len(), 8);
    }
}

/// Serialize a verification bundle to a JSON string.
pub fn bundle_to_json(bundle: &VerificationBundle) -> Result<String, String> {
    serde_json::to_string(bundle).map_err(|e| e.to_string())
}

/// Deserialize a verification bundle from a JSON string.
pub fn bundle_from_json(s: &str) -> Result<VerificationBundle, String> {
    serde_json::from_str(s).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests_json {
    use super::*;
    use zkml_common::models::LogisticRegression;

    #[test]
    fn bundle_json_round_trips() {
        let model = Model::LogisticRegression(LogisticRegression {
            weights: vec![FixedPoint::quantize(0.5)],
            bias: FixedPoint::quantize(0.0),
        });
        let bundle = generate_proof(&model, &[FixedPoint::quantize(1.0)]).unwrap();
        let json = bundle_to_json(&bundle).unwrap();
        let restored = bundle_from_json(&json).unwrap();
        assert_eq!(
            restored.public_inputs.model_hash,
            bundle.public_inputs.model_hash
        );
    }
}

/// A deterministic identifier for a bundle, derived from its public inputs.
///
/// Useful for de-duplicating or indexing submitted proofs off-chain.
pub fn bundle_id(bundle: &VerificationBundle) -> Commitment {
    let pi = &bundle.public_inputs;
    let elements: Vec<i64> = pi
        .model_hash
        .iter()
        .chain(pi.input_hash.iter())
        .chain(pi.output.iter())
        .map(|b| *b as i64)
        .collect();
    commit_i64(&elements)
}
