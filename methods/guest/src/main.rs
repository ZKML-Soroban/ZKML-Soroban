//! zkml inference guest program.
//!
//! Reads a quantized model and input features from the host, runs the shared
//! `zkml_common::inference` engine, and commits public journal fields:
//! `(model_hash, input_hash, output_raw)`.
//!
//! STARK → Groth16 compression is out of scope here (issue #11).

#![no_main]

use risc0_zkvm::guest::env;
use serde::{Deserialize, Serialize};
use zkml_common::commitment::{commitment_hash, model_elements, Commitment};
use zkml_common::fixed_point::FixedPoint;
use zkml_common::inference::run_inference;
use zkml_common::models::Model;

risc0_zkvm::guest::entry!(main);

/// Public journal payload committed by the guest.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GuestJournal {
    pub model_hash: Commitment,
    pub input_hash: Commitment,
    pub output: i64,
}

fn main() {
    let model: Model = env::read();
    let inputs: Vec<FixedPoint> = env::read();

    let output = run_inference(&model, &inputs);

    let model_hash = commitment_hash(&model_elements(&model));
    let input_hash = commitment_hash(&inputs.iter().map(|x| x.value).collect::<Vec<_>>());

    env::commit(&GuestJournal {
        model_hash,
        input_hash,
        output: output.value,
    });
}

