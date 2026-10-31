//! The verifier's golden fixtures must describe what this tree builds.
//!
//! `crates/zkml-verifier/testdata/` holds a real receipt and the parameters it
//! is verified against: the guest image id, the control root, the BN254
//! control id, the seal selector and the universal verifying key. The contract
//! tests read all of them back from the files, so they stay green after a
//! guest change or a RISC Zero bump even though a real receipt from the new
//! build would no longer verify. These tests compare the files with what this
//! tree builds, so such a change fails CI until the fixtures are regenerated
//! (see CONTRIBUTING.md, "Changing the zkVM guest").
//!
//! The image id check needs the reproducible guest build
//! (`ZKML_GUEST_DOCKER=1`): a local build's image id is specific to the machine,
//! so the test skips it there and says so. CI runs it with the Docker build.

#![cfg(feature = "zkvm")]

use std::path::PathBuf;

use zkml_common::bundle::to_hex;
use zkml_prover::cli::cmd_export_vk;
use zkml_prover::prover::{guest_build_is_reproducible, image_id};

fn fixture(name: &str) -> serde_json::Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../zkml-verifier/testdata")
        .join(name);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("reading {}: {e}", path.display()));
    serde_json::from_str(&text).expect("valid json")
}

/// Everything in `export-vk --format json` except the build-dependent image id.
fn risc0_parameters(mut value: serde_json::Value) -> serde_json::Value {
    value
        .as_object_mut()
        .expect("a json object")
        .remove("image_id");
    value
}

#[test]
fn risc0_vk_fixture_matches_the_pinned_risc0_version() {
    let mut out = Vec::new();
    cmd_export_vk("json", None, &mut out).expect("export-vk");
    let exported: serde_json::Value = serde_json::from_slice(&out).expect("export-vk prints json");
    assert_eq!(
        risc0_parameters(exported),
        risc0_parameters(fixture("risc0_vk.json")),
        "risc0_vk.json differs from `export-vk --format json` for the pinned RISC Zero \
         version; regenerate crates/zkml-verifier/testdata/ as described in \
         CONTRIBUTING.md, \"Changing the zkVM guest\""
    );
}

#[test]
fn golden_receipt_was_made_by_the_reproducible_guest() {
    if !guest_build_is_reproducible() {
        eprintln!(
            "skipped: the guest is a local build, whose image id is specific to this machine; \
             rerun with ZKML_GUEST_DOCKER=1 to check the golden receipt's image id"
        );
        return;
    }
    let bundle = fixture("groth16_bundle.json");
    assert_eq!(
        bundle["image_id"].as_str().expect("image_id"),
        to_hex(&image_id()),
        "groth16_bundle.json was proved by a different guest build; regenerate \
         crates/zkml-verifier/testdata/ as described in CONTRIBUTING.md, \"Changing the zkVM guest\""
    );
    assert_eq!(
        fixture("risc0_vk.json")["image_id"],
        bundle["image_id"],
        "risc0_vk.json and groth16_bundle.json name different guests"
    );
}
