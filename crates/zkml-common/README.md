# zkml-common

Shared, deterministic building blocks for [zkml-soroban](https://github.com/ZKML-Soroban/ZKML-Soroban):
provable machine learning inference verified on Stellar through Soroban smart contracts.

The same code runs in three places, so their results must match bit for bit:

- natively in the off-chain prover (`zkml-prover`),
- inside the RISC Zero zkVM guest that produces the proof,
- in tests that cross-check both.

## What is inside

| Module | Purpose |
| ------ | ------- |
| `fixed_point` | Q16.16 `FixedPoint` with checked and saturating arithmetic. No floats in the inference path. |
| `activation` | ZK-friendly activations: ReLU, leaky ReLU, ReLU6, hard sigmoid, hard swish, hardtanh. |
| `models` | `DecisionTree`, `LogisticRegression`, `TinyMLP` and structural validation. |
| `inference` | Fixed-point inference, class decisions (threshold, argmax) and fallible batch inference. |
| `commitment` | Poseidon (BN254, circom parameters) commitments binding model parameters and inputs. |
| `merkle` | Domain-separated Merkle tree with inclusion proofs over commitments. |
| `proof` | `PublicInputs`, `Groth16Proof` and `VerificationBundle` wire types. |
| `tensor` | Minimal row-major tensor used between dense layers. |

## Usage

```toml
[dependencies]
zkml-common = "0.0.1"
```

```rust
use zkml_common::fixed_point::FixedPoint;
use zkml_common::inference::run_inference_with_decision;
use zkml_common::models::{LogisticRegression, Model};

let model = Model::LogisticRegression(LogisticRegression {
    weights: vec![FixedPoint::quantize(0.5), FixedPoint::quantize(-0.25)],
    bias: FixedPoint::quantize(0.1),
    decision_threshold: FixedPoint::quantize(0.0),
});

let inputs = [FixedPoint::quantize(1.0), FixedPoint::quantize(2.0)];
let (score, class_label) = run_inference_with_decision(&model, &inputs);
assert_eq!(class_label, 1);
println!("score = {}", score.dequantize());
```

## Features

| Feature | Default | Description |
| ------- | ------- | ----------- |
| `std` | yes | Standard library support. The zkVM guest disables it; a fully `no_std` target build is not supported yet. |

## Status

Pre-1.0 and under active development. Commitment encodings are consensus-critical and may
change between `0.0.x` releases; pin an exact version if you persist commitments.

## License

Apache-2.0
