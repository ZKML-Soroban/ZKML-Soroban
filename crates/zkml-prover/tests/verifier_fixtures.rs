//! The verifier's golden fixtures must describe the guest this tree builds.
//!
//! `crates/zkml-verifier/testdata/` holds a real Groth16 receipt and the
//! RISC Zero parameters it verifies against. The contract test that uses them
//! reads the image id out of the bundle itself, so it stays green after any
//! change to the guest, to `zkml-common`, or to the RISC Zero version, even
//! though the receipt no longer matches what a deployment would register.
//! These tests compare the fixtures with what this tree builds, so such a
//! change fails CI until the fixtures are regenerated (see CONTRIBUTING.md,
//! "Changing the zkVM guest").
//!
//! Dev mode is enough: the image id and the verifier parameters come from the
//! build, not from proving.

#![cfg(feature = "zkvm")]

use std::path::PathBuf;

use zkml_common::bundle::to_hex;
use zkml_prover::cli::cmd_export_vk;
use zkml_prover::prover::image_id;

fn fixture(name: &str) -> serde_json::Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../zkml-verifier/testdata")
        .join(name);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("reading {}: {e}", path.display()));
    serde_json::from_str(&text).expect("valid json")
}

const REGENERATE: &str = "regenerate crates/zkml-verifier/testdata/ as described in \
                          CONTRIBUTING.md, \"Changing the zkVM guest\"";

#[test]
fn golden_receipt_was_made_by_this_guest() {
    let bundle = fixture("groth16_bundle.json");
    assert_eq!(
        bundle["image_id"].as_str().expect("image_id"),
        to_hex(&image_id()),
        "groth16_bundle.json was proved by a different guest build; {REGENERATE}"
    );
}

#[test]
fn risc0_vk_fixture_matches_export_vk() {
    let mut out = Vec::new();
    cmd_export_vk("json", None, &mut out).expect("export-vk");
    let exported: serde_json::Value = serde_json::from_slice(&out).expect("export-vk prints json");
    assert_eq!(
        exported,
        fixture("risc0_vk.json"),
        "risc0_vk.json differs from `export-vk --format json`; {REGENERATE}"
    );
}
