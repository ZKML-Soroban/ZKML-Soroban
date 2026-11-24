//! zkVM cycles must not grow unnoticed.
//!
//! Proving cost follows the guest's cycle count, so a change that makes the
//! guest do more work makes every proof slower and dearer. These tests execute
//! the guest, without proving, on the example models and compare the counts
//! with `cycles-baseline.json`, failing when one grows by more than 5%.
//!
//! Counts are deterministic for a given guest binary, so the comparison needs
//! the reproducible build (`ZKML_GUEST_DOCKER=1`); a local build is skipped,
//! as in `verifier_fixtures`. CI runs it with the Docker build. A change that
//! moves the counts on purpose updates the baseline in the same pull request:
//!
//! ```text
//! ZKML_GUEST_DOCKER=1 ZKML_UPDATE_CYCLES=1 cargo test -p zkml-prover --features zkvm --test cycles_baseline
//! ```

#![cfg(feature = "zkvm")]

use std::path::{Path, PathBuf};

use zkml_prover::cli::{load_model, parse_inputs};
use zkml_prover::prover::{count_cycles, guest_build_is_reproducible};

/// Growth tolerated before the test fails.
const TOLERANCE_PERCENT: u64 = 5;

/// The models and inputs the baseline covers.
const CASES: &[(&str, &str, &str)] = &[
    (
        "credit_lr",
        "../../examples/models/credit_lr.json",
        "0.5,0.2,0.9,0.1",
    ),
    (
        "kyc_tree",
        "../../examples/models/kyc_tree.json",
        "0.6,0.4,0.0",
    ),
];

fn baseline_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("cycles-baseline.json")
}

#[test]
fn guest_cycles_stay_within_the_baseline() {
    if !guest_build_is_reproducible() {
        eprintln!("skipped: the guest was built locally; run with ZKML_GUEST_DOCKER=1");
        return;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let opening = [0x2a; 32];
    let mut measured = serde_json::Map::new();
    for (name, model, input) in CASES {
        let model = load_model(&root.join(model)).expect("example model loads");
        let inputs = parse_inputs(input).expect("valid input");
        let count = count_cycles(&model, &inputs, &opening).expect("guest executes");
        println!(
            "{name}: {} user cycles, {} padded, {} segments",
            count.user_cycles, count.padded_cycles, count.segments
        );
        measured.insert(name.to_string(), serde_json::to_value(count).unwrap());
    }

    if std::env::var_os("ZKML_UPDATE_CYCLES").is_some() {
        let text = serde_json::to_string_pretty(&measured).unwrap();
        std::fs::write(baseline_path(), text + "\n").unwrap();
        println!("wrote {}", baseline_path().display());
        return;
    }

    let baseline: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(baseline_path()).unwrap()).unwrap();
    for (name, now) in &measured {
        let was = baseline[name]["user_cycles"]
            .as_u64()
            .unwrap_or_else(|| panic!("{name} is missing from cycles-baseline.json"));
        let now = now["user_cycles"].as_u64().unwrap();
        let ceiling = was + was * TOLERANCE_PERCENT / 100;
        assert!(
            now <= ceiling,
            "{name} runs {now} user cycles, more than {TOLERANCE_PERCENT}% above the baseline \
             {was}; if that is intended, update cycles-baseline.json (see the module docs)"
        );
    }
}
