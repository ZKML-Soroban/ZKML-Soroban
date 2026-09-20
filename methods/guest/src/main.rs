//! zkml inference guest program.
//!
//! Reads a quantized model and input features from the host, runs the shared
//! `zkml_common::inference` engine, and commits the fixed 96-byte
//! [`JournalV1`] layout: `(model_kind, model_hash, input_hash, output, class_label)`.
//!
//! Committing raw bytes rather than a serde struct keeps the journal a stable
//! wire format: the Soroban verifier parses it without serde, and the claim
//! digest that the Groth16 receipt binds is computed over exactly these bytes.

#![no_main]

use risc0_zkvm::guest::env;
use zkml_common::commitment::{commitment_hash, model_elements};
use zkml_common::fixed_point::FixedPoint;
use zkml_common::inference::run_inference_with_decision;
use zkml_common::journal::{JournalV1, ModelKind};
use zkml_common::models::Model;

risc0_zkvm::guest::entry!(main);

fn main() {
    let model: Model = env::read();
    let inputs: Vec<FixedPoint> = env::read();

    let (output, class_label) = run_inference_with_decision(&model, &inputs);

    let journal = JournalV1 {
        model_kind: ModelKind::of(&model),
        model_hash: commitment_hash(&model_elements(&model)),
        input_hash: commitment_hash(&inputs.iter().map(|x| x.value).collect::<Vec<_>>()),
        output: output.value,
        class_label,
    };

    env::commit_slice(&journal.encode());
}
