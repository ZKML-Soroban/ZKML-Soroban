//! zkml inference guest program.
//!
//! Reads a quantized model, input features and the model's opening from the
//! host, runs the shared `zkml_common::inference` engine, and commits the
//! fixed 96-byte [`JournalV1`] layout:
//! `(model_kind, model_hash, input_hash, output, class_label)`.
//!
//! The opening is a private input. It only enters the two hiding commitments
//! and is never written to the journal, so the public outputs reveal neither
//! the model nor the inputs to anyone without it.
//!
//! Committing raw bytes rather than a serde struct keeps the journal a stable
//! wire format: the Soroban verifier parses it without serde, and the claim
//! digest that the Groth16 receipt binds is computed over exactly these bytes.

#![no_main]

use risc0_zkvm::guest::env;
use zkml_common::commitment::{commit_inputs, commit_model, Opening};
use zkml_common::fixed_point::FixedPoint;
use zkml_common::inference::try_run_inference_with_decision;
use zkml_common::journal::{JournalV1, ModelKind};
use zkml_common::models::Model;

risc0_zkvm::guest::entry!(main);

fn main() {
    let model: Model = env::read();
    let inputs: Vec<FixedPoint> = env::read();
    let opening: Opening = env::read();

    // Use the fallible inference path. The infallible one calls `.expect(...)`
    // on a malformed model, a tree that exceeds the iteration bound or an MLP
    // that overflows, which panics inside the guest and aborts proving with no
    // typed reason. `panic!` in the guest still produces no valid proof, but
    // this fails cleanly at the boundary and keeps the guest on the same
    // checked path the host tests exercise.
    let (output, class_label) = match try_run_inference_with_decision(&model, &inputs) {
        Ok(result) => result,
        Err(e) => panic!("inference failed in guest: {e:?}"),
    };

    let journal = JournalV1 {
        model_kind: ModelKind::of(&model),
        model_hash: commit_model(&model, &opening),
        input_hash: commit_inputs(&inputs, &opening),
        output: output.value,
        class_label,
    };

    env::commit_slice(&journal.encode());
}
