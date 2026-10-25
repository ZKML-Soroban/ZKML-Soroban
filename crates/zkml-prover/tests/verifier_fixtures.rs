//! The verifier's RISC Zero fixture must match the pinned RISC Zero version.
//!
//! `crates/zkml-verifier/testdata/risc0_vk.json` holds the parameters the
//! golden receipt is verified against: the control root, the BN254 control id,
//! the seal selector and the universal verifying key. The contract test that
//! uses them reads them back from the file, so it stays green after a RISC Zero
//! bump even though a real receipt from the new version would no longer verify
//! against them. This test compares the file with what `export-vk` derives
//! from the RISC Zero version this tree pins, so such a bump fails CI until the
//! fixtures are regenerated (see CONTRIBUTING.md, "Changing the zkVM guest").
//!
//! The image id is deliberately left out. The guest is built on the host with
//! `risc0_build::embed_methods()`, so its binary, and with it the image id,
//! depends on the machine that builds it: the CI runners and a local Linux
//! machine get different ids from the same commit. Checking it needs a
//! reproducible (Docker) guest build first.

#![cfg(feature = "zkvm")]

use std::path::PathBuf;

use zkml_prover::cli::cmd_export_vk;

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
