# Proof Bundle Wire Format

A `VerificationBundle` is the single artifact handed to the verifier. It
serializes to JSON with two fields.

```json
{
  "proof": { "data": [/* Groth16 proof bytes */] },
  "public_inputs": {
    "model_hash": [/* 32 bytes */],
    "input_hash": [/* 32 bytes */],
    "output": [/* output bytes */]
  }
}
```

`bundle_id` derives a stable 32-byte identifier from the public inputs, which
off-chain services can use to index or de-duplicate submissions.
