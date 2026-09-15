---
title: "Examples"
description: "Run the bundled credit scoring and KYC example models."
icon: "flask"
---

`examples/models/` contains ready-to-run models in the
[JSON exchange format](/guides/model-format). See the [CLI reference](/guides/cli)
for every subcommand.

## Credit scoring (logistic regression)

```bash
cargo run -p zkml-prover -- infer examples/models/credit_lr.json -i "0.5,0.2,0.9,0.1"
```

The score is compared with `decision_threshold` (`0.0`) to produce
`class_label`.

Export a verification bundle for the same evaluation:

```bash
cargo run -p zkml-prover -- prove examples/models/credit_lr.json \
  -i "0.5,0.2,0.9,0.1" -o bundle.json
```

The proof bytes in the bundle are empty until Groth16 compression lands.

## KYC risk (decision tree)

```bash
cargo run -p zkml-prover -- infer examples/models/kyc_tree.json -i "0.6,0.1,0.0"
```

The tree returns leaf value `1.0` for the high-risk branch and `0.0` otherwise.

Inspect structure and commitment without running inference:

```bash
cargo run -p zkml-prover -- inspect examples/models/kyc_tree.json
```

## End-to-end KYC demo

A larger synthetic KYC scenario (dataset generation, training, ONNX export,
deployment) lives in `examples/kyc-demo/`. See the
[KYC demo walkthrough](/guides/kyc-demo).
