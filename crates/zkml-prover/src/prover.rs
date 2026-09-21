//! Proof generation.
//!
//! Orchestrates the pipeline: commit to the model and the inputs, run inference
//! inside the RISC Zero zkVM, compress the STARK receipt to Groth16 and package
//! everything as a [`VerificationBundleV2`].
//!
//! # What runs where
//!
//! | Step | Requirement |
//! | ---- | ----------- |
//! | Commitments and native inference | any platform |
//! | zkVM execution and STARK receipt (`zkvm` feature) | RISC Zero toolchain |
//! | Groth16 compression (`groth16` feature) | x86_64 Linux with Docker |
//! | Groth16 compression (`cuda` feature) | NVIDIA GPU on Linux, no Docker |
//!
//! Without CUDA the Groth16 step shells out to a Docker image that runs the
//! Circom witness generator, which is published for x86_64 only (risc0 issue
//! #1749). The platform is checked up front so the failure is a clear error
//! instead of a confusing one deep inside the prover.
//!
//! With the `cuda` feature, `risc0-groth16` uses its native prover: a Rust
//! witness calculator and a CUDA Groth16 prover. That path needs neither Docker
//! nor x86_64, so it is also how an ARM Linux machine with an NVIDIA GPU can
//! compress. `metal` accelerates proving on Apple Silicon but has no Groth16
//! path, so macOS still needs Docker for the wrap.

use zkml_common::bundle::VerificationBundleV2;
#[cfg(feature = "zkvm")]
use zkml_common::bundle::{BundleMeta, ProofSystemId, ProveTimings};
use zkml_common::commitment::{commit_i64, commitment_hash, Commitment};
use zkml_common::fixed_point::FixedPoint;
use zkml_common::journal::{JournalV1, ModelKind};
use zkml_common::models::Model;
use zkml_common::proof::{Groth16Proof, PublicInputs, VerificationBundle};

/// Flatten model parameters for commitments (shared with the guest).
pub use zkml_common::commitment::model_elements;

/// Errors from the proving pipeline.
#[derive(Debug, thiserror::Error)]
pub enum ProveError {
    /// Inference failed before any proving happened.
    #[error("inference failed: {0}")]
    Inference(#[from] zkml_common::ZkmlError),

    /// The journal committed by the guest could not be decoded.
    #[error("journal: {0}")]
    Journal(#[from] zkml_common::journal::JournalError),

    /// The journal does not match what native inference produced.
    #[error("journal does not match native inference: {0}")]
    JournalMismatch(String),

    /// Building the bundle failed.
    #[error("bundle: {0}")]
    Bundle(#[from] zkml_common::bundle::BundleError),

    /// The zkVM refused to prove or verify.
    #[error("zkvm: {0}")]
    Zkvm(String),

    /// Groth16 compression needs x86_64 Linux, unless built with CUDA.
    #[error("Groth16 compression through Docker requires x86_64 Linux (current: {os} {arch}). Build with --features cuda to use the native GPU prover instead, or use a Linux x86_64 machine or WSL2.")]
    UnsupportedPlatform {
        /// Operating system of the current build.
        os: &'static str,
        /// Architecture of the current build.
        arch: &'static str,
    },

    /// Docker is required by the Groth16 step but is not usable.
    #[error("Docker is required for Groth16 compression but is not available: {0}. Build with --features cuda to compress without Docker.")]
    DockerUnavailable(String),

    /// Dev mode produces fake receipts, which carry no seal.
    #[error(
        "RISC0_DEV_MODE=1 produces a fake receipt with no seal; unset it to compress to Groth16"
    )]
    DevModeHasNoSeal,

    /// A remote proving backend was requested but is not wired up yet.
    #[error("remote proving backend is not available: {0}")]
    RemoteBackend(String),

    /// Serialization of the bundle failed.
    #[error("serialization: {0}")]
    Serialization(String),
}

/// Where the Groth16 proof is produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProverBackend {
    /// Compress locally. Requires x86_64 Linux with Docker.
    Local,
    /// Send the work to a remote proving market.
    ///
    /// RISC Zero shut down Bonsai in December 2025; the replacement is the
    /// Boundless market. The client is not implemented yet, so this variant
    /// currently fails with [`ProveError::RemoteBackend`].
    Remote {
        /// Endpoint of the proving market.
        endpoint: String,
    },
}

impl ProverBackend {
    /// Parse a backend name from the CLI.
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "local" => Some(ProverBackend::Local),
            "boundless" | "remote" => Some(ProverBackend::Remote {
                endpoint: "https://boundless.network".to_string(),
            }),
            _ => None,
        }
    }
}

/// Commit to a model's parameters (the on-chain `initialize` value).
pub fn model_commitment(model: &Model) -> Commitment {
    commitment_hash(&model_elements(model))
}

/// Commit to a set of input features (a proof public input).
pub fn input_commitment(inputs: &[FixedPoint]) -> Commitment {
    let elements: Vec<i64> = inputs.iter().map(|x| x.value).collect();
    commitment_hash(&elements)
}

/// Build the journal for a model and its inputs, using native inference.
///
/// This is the same computation the guest performs; the host uses it to
/// cross-check what the guest committed.
pub fn journal_for(model: &Model, inputs: &[FixedPoint]) -> Result<JournalV1, ProveError> {
    let (output, class_label) = crate::inference::try_run_inference_with_decision(model, inputs)?;
    Ok(JournalV1 {
        model_kind: ModelKind::of(model),
        model_hash: model_commitment(model),
        input_hash: input_commitment(inputs),
        output: output.value,
        class_label,
    })
}

/// Generate a legacy v1 bundle: public inputs with no proof.
///
/// Kept so existing callers and tests keep working. It carries no
/// cryptographic evidence; use [`prove_groth16`] for that.
#[deprecated(note = "v1 bundles carry no proof; use prove_groth16 and VerificationBundleV2")]
pub fn generate_proof(model: &Model, inputs: &[FixedPoint]) -> Result<VerificationBundle, String> {
    let journal = journal_for(model, inputs).map_err(|e| e.to_string())?;

    Ok(VerificationBundle {
        proof: Groth16Proof { data: Vec::new() },
        public_inputs: PublicInputs {
            model_hash: journal.model_hash,
            input_hash: journal.input_hash,
            output: journal.output.to_le_bytes().to_vec(),
            class_label: journal.class_label,
        },
    })
}

/// True when this build compresses without Docker, using the native CUDA
/// prover in `risc0-groth16` instead of the x86_64 Docker image.
pub fn native_groth16_available() -> bool {
    cfg!(feature = "cuda")
}

/// A short name for the compression path this build uses, for logs and for the
/// bundle metadata.
pub fn groth16_backend_name() -> &'static str {
    if native_groth16_available() {
        "cuda"
    } else {
        "docker"
    }
}

/// True when the current build can run local Groth16 compression.
pub fn groth16_platform_supported() -> bool {
    native_groth16_available() || cfg!(all(target_os = "linux", target_arch = "x86_64"))
}

/// Fail early when the platform cannot run local Groth16 compression.
pub fn check_groth16_platform() -> Result<(), ProveError> {
    if groth16_platform_supported() {
        Ok(())
    } else {
        Err(ProveError::UnsupportedPlatform {
            os: std::env::consts::OS,
            arch: std::env::consts::ARCH,
        })
    }
}

/// Fail early when Docker is not usable.
///
/// A CUDA build never reaches the Docker path, so this returns `Ok` there
/// rather than demanding a daemon it will not use.
pub fn check_docker() -> Result<(), ProveError> {
    if native_groth16_available() {
        return Ok(());
    }
    match std::process::Command::new("docker")
        .arg("info")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
    {
        Ok(status) if status.success() => Ok(()),
        Ok(status) => Err(ProveError::DockerUnavailable(format!(
            "`docker info` exited with {status}"
        ))),
        Err(e) => Err(ProveError::DockerUnavailable(format!(
            "could not run `docker`: {e}"
        ))),
    }
}

/// True when `RISC0_DEV_MODE` asks for fake receipts.
pub fn dev_mode_enabled() -> bool {
    matches!(
        std::env::var("RISC0_DEV_MODE").as_deref(),
        Ok("1") | Ok("true")
    )
}

#[cfg(feature = "zkvm")]
mod zkvm_prove {
    use super::*;
    use risc0_zkvm::{default_prover, ExecutorEnv, ProverOpts, Receipt};
    use std::time::Instant;
    use zkml_methods::{ZKML_GUEST_ELF, ZKML_GUEST_ID};

    /// Everything one proving run produced.
    pub struct ProveOutput {
        /// The receipt, verified against the guest image id.
        pub receipt: Receipt,
        /// 4-byte selector plus the 256-byte Groth16 proof.
        pub seal: Vec<u8>,
        /// Raw journal bytes committed by the guest.
        pub journal: Vec<u8>,
        /// Decoded journal.
        pub journal_v1: JournalV1,
        /// Guest image id.
        pub image_id: [u8; 32],
        /// Stage timings.
        pub timings: ProveTimings,
        /// Total guest cycles.
        pub cycles: u64,
    }

    /// The guest image id as bytes.
    pub fn image_id() -> [u8; 32] {
        let mut out = [0u8; 32];
        for (i, word) in ZKML_GUEST_ID.iter().enumerate() {
            out[i * 4..(i + 1) * 4].copy_from_slice(&word.to_le_bytes());
        }
        out
    }

    fn build_env(model: &Model, inputs: &[FixedPoint]) -> Result<ExecutorEnv<'static>, ProveError> {
        let inputs_owned: Vec<FixedPoint> = inputs.to_vec();
        ExecutorEnv::builder()
            .write(model)
            .map_err(|e| ProveError::Zkvm(format!("writing model: {e}")))?
            .write(&inputs_owned)
            .map_err(|e| ProveError::Zkvm(format!("writing inputs: {e}")))?
            .build()
            .map_err(|e| ProveError::Zkvm(format!("building env: {e}")))
    }

    /// Run inference inside the zkVM and return a verified STARK receipt.
    ///
    /// `RISC0_DEV_MODE=1` makes this fast and insecure, which is what CI uses.
    pub fn generate_receipt(
        model: &Model,
        inputs: &[FixedPoint],
    ) -> Result<(Receipt, JournalV1), ProveError> {
        let env = build_env(model, inputs)?;
        let prove_info = default_prover()
            .prove(env, ZKML_GUEST_ELF)
            .map_err(|e| ProveError::Zkvm(format!("prove failed: {e}")))?;
        let receipt = prove_info.receipt;
        receipt
            .verify(ZKML_GUEST_ID)
            .map_err(|e| ProveError::Zkvm(format!("receipt verification failed: {e}")))?;
        let journal = decode_journal(&receipt)?;
        cross_check_native(model, inputs, &journal)?;
        Ok((receipt, journal))
    }

    /// Prove and compress to Groth16.
    pub fn prove_groth16(
        model: &Model,
        inputs: &[FixedPoint],
        backend: &ProverBackend,
    ) -> Result<ProveOutput, ProveError> {
        if let ProverBackend::Remote { endpoint } = backend {
            return Err(ProveError::RemoteBackend(format!(
                "{endpoint}: the Boundless client is not implemented yet"
            )));
        }
        if dev_mode_enabled() {
            return Err(ProveError::DevModeHasNoSeal);
        }
        check_groth16_platform()?;
        check_docker()?;

        let started = Instant::now();
        let env = build_env(model, inputs)?;
        let prove_started = Instant::now();
        let prove_info = default_prover()
            .prove_with_opts(env, ZKML_GUEST_ELF, &ProverOpts::groth16())
            .map_err(|e| ProveError::Zkvm(format!("groth16 prove failed: {e}")))?;
        let prove_ms = prove_started.elapsed().as_millis() as u64;

        let receipt = prove_info.receipt;
        receipt
            .verify(ZKML_GUEST_ID)
            .map_err(|e| ProveError::Zkvm(format!("receipt verification failed: {e}")))?;

        let journal_v1 = decode_journal(&receipt)?;
        cross_check_native(model, inputs, &journal_v1)?;
        let seal = encode_seal(&receipt)?;

        Ok(ProveOutput {
            journal: receipt.journal.bytes.clone(),
            journal_v1,
            seal,
            image_id: image_id(),
            timings: ProveTimings {
                execute_ms: 0,
                prove_ms,
                compress_ms: 0,
                total_ms: started.elapsed().as_millis() as u64,
            },
            cycles: prove_info.stats.total_cycles,
            receipt,
        })
    }

    /// Selector plus proof bytes, in the layout the on-chain verifier reads.
    ///
    /// Mirrors `encode_seal` from `risc0-ethereum-contracts`: the selector is
    /// the first four bytes of the verifier parameters digest, so a verifier can
    /// tell which verification key the proof was made for.
    pub fn encode_seal(receipt: &Receipt) -> Result<Vec<u8>, ProveError> {
        let groth16 = receipt
            .inner
            .groth16()
            .map_err(|_| ProveError::DevModeHasNoSeal)?;
        let params = groth16.verifier_parameters.as_bytes();
        let mut seal = Vec::with_capacity(4 + groth16.seal.len());
        seal.extend_from_slice(&params[..4]);
        seal.extend_from_slice(&groth16.seal);
        Ok(seal)
    }

    /// Decode the journal a receipt carries.
    pub fn decode_journal(receipt: &Receipt) -> Result<JournalV1, ProveError> {
        Ok(JournalV1::decode(&receipt.journal.bytes)?)
    }

    /// Check the guest journal against native inference.
    fn cross_check_native(
        model: &Model,
        inputs: &[FixedPoint],
        journal: &JournalV1,
    ) -> Result<(), ProveError> {
        let expected = journal_for(model, inputs)?;
        if expected != *journal {
            return Err(ProveError::JournalMismatch(format!(
                "guest committed {journal:?}, native inference produced {expected:?}"
            )));
        }
        Ok(())
    }

    /// Package a proving run as a bundle.
    pub fn bundle_from_output(output: &ProveOutput) -> Result<VerificationBundleV2, ProveError> {
        Ok(VerificationBundleV2::new(
            ProofSystemId::Risc0Groth16,
            output.image_id,
            output.seal.clone(),
            output.journal.clone(),
            BundleMeta {
                prover_version: env!("CARGO_PKG_VERSION").to_string(),
                created_at: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(0),
                cycles: output.cycles,
                timings: output.timings,
            },
        )?)
    }

    /// Verify a bundle locally, reconstructing the receipt from its parts.
    ///
    /// This is the same check the on-chain verifier performs, minus the
    /// pairing being executed by Soroban host functions.
    pub fn verify_bundle(bundle: &VerificationBundleV2) -> Result<(), ProveError> {
        use risc0_zkvm::sha::Digest;
        use risc0_zkvm::{Groth16Receipt, InnerReceipt, Journal, MaybePruned, ReceiptClaim};

        let journal = Journal::new(bundle.journal.clone());
        let image: Digest = Digest::try_from(bundle.image_id)
            .map_err(|e| ProveError::Zkvm(format!("bad image id: {e}")))?;
        let claim = ReceiptClaim::ok(image, MaybePruned::Value(bundle.journal.clone()));

        let mut selector = [0u8; 4];
        selector.copy_from_slice(&bundle.seal[..4]);
        let params = risc0_zkvm::Groth16ReceiptVerifierParameters::default();
        let params_digest = {
            use risc0_zkvm::sha::Digestible;
            params.digest()
        };
        if params_digest.as_bytes()[..4] != selector {
            return Err(ProveError::Zkvm(format!(
                "seal selector {:02x?} does not match this RISC Zero version",
                selector
            )));
        }

        let receipt = risc0_zkvm::Receipt::new(
            InnerReceipt::Groth16(Groth16Receipt::new(
                bundle.proof_bytes().to_vec(),
                MaybePruned::Value(claim),
                params_digest,
            )),
            journal.bytes,
        );
        receipt
            .verify(image)
            .map_err(|e| ProveError::Zkvm(format!("bundle verification failed: {e}")))
    }
}

#[cfg(feature = "zkvm")]
pub use zkvm_prove::{
    bundle_from_output, decode_journal, encode_seal, generate_receipt, image_id, prove_groth16,
    verify_bundle, ProveOutput,
};

/// Serialize a legacy bundle to JSON.
pub fn bundle_to_json(bundle: &VerificationBundle) -> Result<String, String> {
    serde_json::to_string_pretty(bundle).map_err(|e| e.to_string())
}

/// Deserialize a legacy bundle from JSON.
pub fn bundle_from_json(s: &str) -> Result<VerificationBundle, String> {
    serde_json::from_str(s).map_err(|e| e.to_string())
}

/// Serialize a v2 bundle to JSON.
pub fn bundle_v2_to_json(bundle: &VerificationBundleV2) -> Result<String, ProveError> {
    serde_json::to_string_pretty(bundle).map_err(|e| ProveError::Serialization(e.to_string()))
}

/// Deserialize a v2 bundle from JSON.
pub fn bundle_v2_from_json(s: &str) -> Result<VerificationBundleV2, ProveError> {
    serde_json::from_str(s).map_err(|e| ProveError::Serialization(e.to_string()))
}

/// A deterministic identifier for a legacy bundle, derived from its public inputs.
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

#[cfg(test)]
mod tests {
    use super::*;
    use zkml_common::models::LogisticRegression;

    fn fp(x: f64) -> FixedPoint {
        FixedPoint::quantize(x)
    }

    fn model() -> Model {
        Model::LogisticRegression(LogisticRegression {
            weights: vec![fp(0.5), fp(-0.25)],
            bias: fp(0.1),
            decision_threshold: fp(0.0),
        })
    }

    #[test]
    fn journal_matches_native_inference() {
        let inputs = vec![fp(1.0), fp(2.0)];
        let journal = journal_for(&model(), &inputs).unwrap();
        assert_eq!(journal.model_kind, ModelKind::LogisticRegression);
        assert_eq!(journal.model_hash, model_commitment(&model()));
        assert_eq!(journal.input_hash, input_commitment(&inputs));
        // 0.5*1.0 + (-0.25)*2.0 + 0.1 = 0.1 >= 0.0
        assert_eq!(journal.class_label, 1);
    }

    #[test]
    fn journal_round_trips_through_bytes() {
        let journal = journal_for(&model(), &[fp(1.0), fp(2.0)]).unwrap();
        assert_eq!(JournalV1::decode(&journal.encode()).unwrap(), journal);
    }

    #[test]
    fn journal_errors_on_wrong_feature_count() {
        let err = journal_for(&model(), &[fp(1.0)]).unwrap_err();
        assert!(matches!(err, ProveError::Inference(_)));
    }

    #[test]
    #[allow(deprecated)]
    fn legacy_bundle_is_populated() {
        let bundle = generate_proof(&model(), &[fp(1.0), fp(2.0)]).unwrap();
        assert_ne!(bundle.public_inputs.model_hash, [0u8; 32]);
        assert_eq!(bundle.public_inputs.output.len(), 8);
        assert_eq!(bundle.public_inputs.class_label, 1);
    }

    #[test]
    #[allow(deprecated)]
    fn legacy_bundle_json_round_trips() {
        let bundle = generate_proof(&model(), &[fp(1.0), fp(2.0)]).unwrap();
        let json = bundle_to_json(&bundle).unwrap();
        let restored = bundle_from_json(&json).unwrap();
        assert_eq!(
            restored.public_inputs.model_hash,
            bundle.public_inputs.model_hash
        );
    }

    #[test]
    fn backend_parsing() {
        assert_eq!(ProverBackend::parse("local"), Some(ProverBackend::Local));
        assert!(matches!(
            ProverBackend::parse("boundless"),
            Some(ProverBackend::Remote { .. })
        ));
        assert_eq!(ProverBackend::parse("nope"), None);
    }

    #[test]
    fn platform_check_matches_target() {
        let result = check_groth16_platform();
        // A CUDA build compresses natively, so no platform is excluded. A
        // Docker build only works on the architecture the image is published
        // for, and must say so rather than fail deep inside the prover.
        if native_groth16_available() || cfg!(all(target_os = "linux", target_arch = "x86_64")) {
            assert!(result.is_ok());
        } else {
            assert!(matches!(
                result,
                Err(ProveError::UnsupportedPlatform { .. })
            ));
        }
    }

    #[test]
    fn the_backend_name_says_which_path_compression_takes() {
        let name = groth16_backend_name();
        assert!(matches!(name, "cuda" | "docker"), "got {name}");
        assert_eq!(name == "cuda", native_groth16_available());
    }

    #[test]
    fn docker_is_not_required_by_a_cuda_build() {
        if native_groth16_available() {
            assert!(
                check_docker().is_ok(),
                "a CUDA build must not demand Docker"
            );
        }
    }
}
