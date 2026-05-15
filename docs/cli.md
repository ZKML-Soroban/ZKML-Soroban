# Prover CLI

The `zkml-prover` binary runs a model against an input vector from the command
line, which is handy for smoke-testing imported models.

## Build

```bash
cargo build -p zkml-prover
```

## Run

```bash
cargo run -p zkml-prover -- examples/models/credit_lr.json "0.5,0.2,0.9,0.1"
```

The command imports the JSON model, runs inference, and prints the dequantized
output value.
