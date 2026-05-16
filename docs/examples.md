# Examples

The `examples/models` directory contains ready-to-run models in the JSON
exchange format.

## Credit scoring (logistic regression)

```bash
cargo run -p zkml-prover -- examples/models/credit_lr.json "0.5,0.2,0.9,0.1"
```

A positive output suggests the applicant clears the risk threshold; the verifier
contract compares the raw linear output against a configured cutoff.

## KYC risk (decision tree)

```bash
cargo run -p zkml-prover -- examples/models/kyc_tree.json "0.6,0.1,0.0"
```

The tree returns a leaf value of `1.0` for the high-risk branch and `0.0`
otherwise.
