# Commitments

zkml-soroban binds each proof to a specific model and input through 32-byte
commitments.

- **Model commitment**: a Poseidon hash over the quantized weights, biases, and
  structural parameters. Computed once and stored on-chain at `initialize`.
- **Input commitment**: a Poseidon hash over the quantized input features,
  supplied as a public input alongside the proof.

The verifier checks both commitments as part of the public inputs, ensuring a
proof cannot be replayed against a different model or input.

> Note: the current off-chain `commit_i64` is a deterministic placeholder. It
> will be swapped for a Poseidon sponge that matches the CAP-0075 host function
> so off-chain and on-chain commitments are identical.
