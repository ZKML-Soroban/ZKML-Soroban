//! End-to-end Groth16 bundle tests.
//!
//! The fast tests here only exercise the guards and the bundle plumbing, so
//! they run anywhere. The real proving test is `#[ignore]`d because it needs
//! x86_64 Linux with Docker and takes minutes:
//!
//! ```text
//! RISC0_DEV_MODE=0 cargo test -p zkml-prover --features groth16 \
//!     --test groth16_bundle real_groth16 -- --ignored --nocapture
//! ```

#![cfg(feature = "groth16")]

use zkml_common::bundle::{AnyBundle, ProofSystemId, VerificationBundleV2};
use zkml_common::fixed_point::FixedPoint;
use zkml_common::models::Model;
use zkml_common::risc0::SEAL_LEN;
use zkml_prover::model_io::import_json;
use zkml_prover::prover::{
    bundle_from_output, bundle_v2_from_json, bundle_v2_to_json, image_id, prove_groth16,
    verify_bundle, ProveError, ProverBackend,
};

const CREDIT: &str = "../../examples/models/credit_lr.json";

fn credit_model() -> (Model, Vec<FixedPoint>) {
    let model = import_json(&std::fs::read(CREDIT).expect("credit_lr.json is readable"))
        .expect("credit_lr.json parses");
    let inputs = [0.5, 0.2, 0.9, 0.1]
        .iter()
        .copied()
        .map(FixedPoint::quantize)
        .collect();
    (model, inputs)
}

#[test]
fn dev_mode_is_refused_because_it_produces_no_seal() {
    // SAFETY: single-threaded test process.
    std::env::set_var("RISC0_DEV_MODE", "1");
    let (model, inputs) = credit_model();

    match prove_groth16(&model, &inputs, &ProverBackend::Local) {
        Err(ProveError::DevModeHasNoSeal) => {}
        other => panic!(
            "dev mode must be refused, got {other:?}",
            other = other.err()
        ),
    }
}

#[test]
fn the_boundless_backend_reports_that_it_is_not_implemented() {
    let (model, inputs) = credit_model();
    let backend = ProverBackend::parse("boundless").expect("boundless is a known backend name");

    match prove_groth16(&model, &inputs, &backend) {
        Err(ProveError::RemoteBackend(message)) => {
            assert!(message.contains("not implemented"), "got: {message}");
        }
        other => panic!("expected RemoteBackend, got {other:?}", other = other.err()),
    }
}

#[test]
fn a_bundle_round_trips_through_json_and_bytes() {
    // A syntactically valid bundle with a dummy seal: the plumbing under test
    // is serialization, not the proof itself.
    let journal = zkml_common::journal::JournalV1 {
        model_kind: zkml_common::journal::ModelKind::LogisticRegression,
        model_hash: [1u8; 32],
        input_hash: [2u8; 32],
        output: -42,
        class_label: 1,
    };
    let bundle = VerificationBundleV2::new(
        ProofSystemId::Risc0Groth16,
        image_id(),
        vec![7u8; SEAL_LEN],
        journal.encode().to_vec(),
        Default::default(),
    )
    .expect("a well-formed bundle");

    let json = bundle_v2_to_json(&bundle).expect("serializes");
    let back = bundle_v2_from_json(&json).expect("deserializes");
    assert_eq!(back, bundle);

    let bytes = bundle.to_bytes();
    assert_eq!(
        VerificationBundleV2::from_bytes(&bytes).expect("unpacks"),
        bundle
    );

    // The untagged enum must pick v2, not the legacy shape.
    match serde_json::from_str::<AnyBundle>(&json).expect("AnyBundle parses v2") {
        AnyBundle::V2(parsed) => assert_eq!(parsed, bundle),
        AnyBundle::V1(_) => panic!("a v2 bundle must not parse as v1"),
    }

    // The 80 bytes the contract hashes are derived from the journal.
    let public_inputs = back.journal_v1().expect("journal").public_inputs_bytes();
    assert_eq!(public_inputs.len(), 80);
    assert_eq!(&public_inputs[..32], &journal.model_hash);
    assert_eq!(&public_inputs[32..64], &journal.input_hash);
    assert_eq!(
        bundle.public_inputs().expect("legacy view").model_hash,
        journal.model_hash
    );
    assert_eq!(bundle.selector(), [7u8; 4]);
    assert_eq!(bundle.proof_bytes().len(), 256);
}

#[test]
fn a_seal_of_the_wrong_length_is_rejected() {
    let journal = zkml_common::journal::JournalV1 {
        model_kind: zkml_common::journal::ModelKind::LogisticRegression,
        model_hash: [0u8; 32],
        input_hash: [0u8; 32],
        output: 0,
        class_label: 0,
    };
    for len in [0usize, 4, SEAL_LEN - 1, SEAL_LEN + 1] {
        let result = VerificationBundleV2::new(
            ProofSystemId::Risc0Groth16,
            image_id(),
            vec![0u8; len],
            journal.encode().to_vec(),
            Default::default(),
        );
        assert!(result.is_err(), "a {len}-byte seal must be rejected");
    }
}

#[test]
fn a_seal_from_another_risc0_version_is_rejected_by_its_selector() {
    // The selector identifies the verifying key the proof was made for. A
    // wrong one is caught before the pairing check, so the error says what is
    // actually wrong instead of "verification failed".
    let journal = zkml_common::journal::JournalV1 {
        model_kind: zkml_common::journal::ModelKind::LogisticRegression,
        model_hash: [3u8; 32],
        input_hash: [4u8; 32],
        output: 7,
        class_label: 0,
    };
    let mut seal = vec![0u8; SEAL_LEN];
    seal[..4].copy_from_slice(&[0xde, 0xad, 0xbe, 0xef]);

    let bundle = VerificationBundleV2::new(
        ProofSystemId::Risc0Groth16,
        image_id(),
        seal,
        journal.encode().to_vec(),
        Default::default(),
    )
    .expect("a well-formed bundle");

    match verify_bundle(&bundle) {
        Err(ProveError::Zkvm(message)) => {
            assert!(message.contains("selector"), "got: {message}");
        }
        other => panic!("expected a selector error, got {other:?}"),
    }
}

/// Real Groth16 proving. Needs x86_64 Linux with Docker; takes minutes.
#[test]
#[ignore = "real Groth16 proving needs x86_64 Linux with Docker and takes minutes"]
fn real_groth16_bundle_verifies() {
    // SAFETY: single-threaded test process.
    std::env::set_var("RISC0_DEV_MODE", "0");
    let (model, inputs) = credit_model();

    let output = prove_groth16(&model, &inputs, &ProverBackend::Local).expect("groth16 prove");
    assert_eq!(output.seal.len(), SEAL_LEN, "seal is selector + proof");
    assert_eq!(output.image_id, image_id());
    println!(
        "seal {} bytes, {} cycles, {} ms total",
        output.seal.len(),
        output.cycles,
        output.timings.total_ms
    );

    let bundle = bundle_from_output(&output).expect("bundle");
    verify_bundle(&bundle).expect("the bundle verifies against risc0's own verifier");

    // And it survives a round trip through the on-disk format.
    let json = bundle_v2_to_json(&bundle).expect("serializes");
    let back = bundle_v2_from_json(&json).expect("deserializes");
    verify_bundle(&back).expect("a round-tripped bundle still verifies");

    // Every byte of the proof and of the journal is bound to the verification.
    // Flipping one anywhere must break it: the journal only reaches the proof
    // through the claim digest, so a journal edit has to fail the pairing.
    for (what, index) in [("proof", 4usize), ("proof", SEAL_LEN - 1)] {
        let mut tampered = bundle.clone();
        tampered.seal[index] ^= 0x01;
        assert!(
            verify_bundle(&tampered).is_err(),
            "a flipped {what} byte at {index} must not verify"
        );
    }
    for index in [8usize, 72, 80] {
        let mut tampered = bundle.clone();
        tampered.journal[index] ^= 0x01;
        assert!(
            verify_bundle(&tampered).is_err(),
            "a flipped journal byte at {index} must not verify"
        );
    }

    // A proof made for one program must not verify against another image id.
    let mut wrong_image = bundle.clone();
    wrong_image.image_id[0] ^= 0x01;
    assert!(
        verify_bundle(&wrong_image).is_err(),
        "a different image id must not verify"
    );
}
