---
title: "Prover CLI"
description: "Commit, infer, prove, validate, and inspect models with the zkml-prover binary."
icon: "terminal"
---

The `zkml-prover` binary exposes the off-chain pipeline (commitments,
inference, bundle export, quantization validation) as subcommands.

> **Breaking change (issue #44).** The old positional form
> `zkml-prover <model.json> "<inputs>"` was removed. Use `infer`. The old form
> silently dropped input fields that failed to parse.

## Build

```bash
cargo build -p zkml-prover
```

## Commands

```text
zkml-prover <COMMAND>

commit        <MODEL>                           Model commitment as 64-char hex
infer         <MODEL> -i <CSV>                  Commitment + dequantized output + raw Q16.16
prove         <MODEL> -i <CSV> [-o <FILE>]      Verification bundle JSON (stdout or file)
                      [--groth16]               Produce a real proof (needs the groth16 feature)
                      [--backend local|boundless]
verify-bundle <FILE>                            Verify a v2 bundle locally (needs zkvm)
export-vk     [--format json|soroban-args]      What the contract needs to verify receipts
              [-o <FILE>]
validate      <MODEL> [--dataset <FILE>]
                      [--max-input-magnitude <F>]    default 1.0
                      [--min-agreement <F>]          default 0.99
inspect       <MODEL>                           Kind, features, structure, commitment, validity
```

`<MODEL>` is a file in the [JSON exchange format](/guides/model-format). The CLI
does not accept `.onnx` files; convert them with the library importer first
(see [ONNX import](/guides/onnx-import)).

`-i/--input` is one comma-separated string, for example `"0.5,0.2,0.9,0.1"`.

### commit

Prints the model commitment, the value registered on-chain at `initialize`.

```bash
cargo run -p zkml-prover -- commit examples/models/credit_lr.json
```

```text
73e881eda85b98eef6a08eec16e3210330c6bd53a182eab7944c87d9c4fce710
```

### infer

```bash
cargo run -p zkml-prover -- infer examples/models/credit_lr.json -i "0.5,0.2,0.9,0.1"
```

```text
model commitment: 73e881eda85b98eef6a08eec16e3210330c6bd53a182eab7944c87d9c4fce710
output: 0.5319976806640625
output (raw Q16.16): 34865
```

The raw value is `FixedPoint::value`, the integer that goes into the public
inputs. The dequantized value is for humans.

### prove

Writes a verification bundle as JSON to stdout, or to `-o <FILE>`.

Without `--groth16` the bundle is the legacy v1 shape: it carries the public
inputs but no proof, and the command says so on stderr. It is useful for
inspecting what would be proven, and for nothing else.

```bash
cargo run -p zkml-prover -- prove examples/models/credit_lr.json \
  -i "0.5,0.2,0.9,0.1" -o bundle.json
```

```text
warning: this is a legacy v1 bundle and carries no proof. Use --groth16 for a real one.
wrote verification bundle to bundle.json
```

With `--groth16` the model runs inside the zkVM, the receipt is compressed to a
Groth16 proof, and the result is a [v2 bundle](/reference/bundle-format) with a
260-byte seal. This needs the `groth16` feature, x86_64 Linux with Docker, and a
few minutes:

```bash
cargo run -p zkml-prover --features groth16 -- prove examples/models/credit_lr.json \
  -i "0.5,0.2,0.9,0.1" --groth16 -o bundle.json
```

```text
proving in the zkVM and compressing to Groth16, this takes minutes
seal: 260 bytes
cycles: 1048576
proving time: 214731 ms
wrote verification bundle (v2) to bundle.json
```

`--backend` selects where proving happens. `local` is the default; `boundless`
is reserved for remote proving and currently returns an error, because Bonsai
was shut down in December 2025 and the Boundless client is not written yet.

The journal in the bundle binds the model commitment, the input commitment, the
raw output and the decision label, so `model_hash` still equals
`model_commitment(&model)`.

### verify-bundle

Verifies a v2 bundle with RISC Zero's own verifier: it rebuilds the receipt
claim from the journal and the image id, checks the seal selector against the
RISC Zero version this binary was built with, and runs the pairing check.

```bash
cargo run -p zkml-prover --features zkvm -- verify-bundle bundle.json
```

```text
bundle verified
image id    : 2f1c...
model hash  : 73e881ed...
input hash  : 9a0b...
output      : 34865
class label : 1
```

This is the same check the contract performs, minus the pairing being run by
Soroban host functions. Run it before submitting a bundle on-chain: a local
failure costs nothing, an on-chain failure costs fees.

### export-vk

Prints what the contract needs to verify receipts: the guest image id, the
control root, the BN254 control id, the seal selector, and RISC Zero's universal
verifying key. `--format json` (the default) is machine-readable.
`--format soroban-args` prints the two admin calls that register them, ready to
run once `<CONTRACT>` and `<ADMIN>` are filled in:

```bash
cargo run -p zkml-prover --features zkvm -- export-vk --format soroban-args
```

```text
# 1. The guest and RISC Zero's parameters.
stellar contract invoke --id <CONTRACT> --source-account <ADMIN> -- set_risc0_config --config '{"image_id":"…",…}'

# 2. RISC Zero's universal verifying key.
stellar contract invoke --id <CONTRACT> --source-account <ADMIN> -- set_risc0_vk --vk '{"alpha":"…",…,"ic":[…]}'
```

These are not arguments to `initialize`, which takes the key for the
native-circuit path. See
[the verifier contract](/reference/verifier-contract#verifying-a-risc-zero-receipt).

The values change whenever the guest, `zkml-common` or the pinned RISC Zero
version changes, so re-export after any of them.

### validate

Runs the quantization validation passes from `zkml_prover::quantization`:

1. **Range check:** no parameter is `i64::MIN` (which cannot be negated).
2. **Static overflow bounds:** worst-case intermediates fit in `i64` for inputs
   bounded by `--max-input-magnitude`.
3. **Accuracy** (only with `--dataset`): compares quantized inference against
   recorded float outputs at a `1e-4` tolerance and fails below
   `--min-agreement`.

```bash
cargo run -p zkml-prover -- validate examples/models/credit_lr.json --dataset dataset.json
```

```text
model: examples/models/credit_lr.json
range check: ok
overflow bounds: ok (max input magnitude 1)
accuracy: ok over 2 sample(s)
  agreement:      100.00% (threshold 99.00%)
  max deviation:  0e0
  mean deviation: 0e0
```

Without `--dataset` the accuracy pass is skipped and reported explicitly:

```text
accuracy: not checked (no --dataset; only range and overflow bounds ran)
```

#### Dataset schema

An array of samples. `inputs` is the raw feature vector; `expected` is the output
of the **original floating-point model** for it (not the training label).

```json
[
  { "inputs": [0.5, 0.2, 0.9, 0.1], "expected": 0.31 },
  { "inputs": [0.0, 0.0, 0.0, 0.0], "expected": -0.20 }
]
```

> A dataset with ground-truth labels instead of float model outputs reports low
> agreement that looks like a quantization failure. Check the dataset first.

### inspect

```bash
cargo run -p zkml-prover -- inspect examples/models/kyc_tree.json
```

```text
model: examples/models/kyc_tree.json
kind: decision_tree
features: 3
nodes: 5 (2 split, 3 leaf)
commitment: 55541e9a9593c81c1531f0067aa7534a8ee8e98e058afa99a0b190a5def23a2f
structure: valid
```

`structure` reports the result of `DecisionTree::validate` or
`TinyMLP::validate` instead of failing, so `inspect` works on broken models.

## Input validation

Every `--input` field must be present and parse. Errors name the 1-based
position and the offending token:

| Input              | Error                                                   |
| ------------------ | ------------------------------------------------------- |
| `""`, `"   "`      | empty input                                             |
| `"0.5,,0.2"`       | field 2 is empty                                        |
| `"0.5,o.2,0.9"`    | field 2 (`'o.2'`) is not a number                       |
| `"nan"`, `"inf"`   | not finite (`"nan".parse::<f64>()` succeeds in Rust)    |
| `"1e30"`           | exceeds the Q16.16 range of +/- 1.4e14                  |

The vector length is checked against the model's feature count:

```text
error: examples/models/credit_lr.json expects 4 feature(s), got 2
```

## Exit codes

| Code | Meaning                                                          |
| ---- | ---------------------------------------------------------------- |
| `0`  | Success                                                          |
| `2`  | Bad invocation: argument errors, malformed `--input`             |
| `1`  | Everything else: IO, model import, validation failure, inference, proving |

Errors go to stderr prefixed with `error: `; normal output goes to stdout.
