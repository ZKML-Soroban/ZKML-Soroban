# Engineering Devlog

A running, chronological log of development decisions and progress on
zkml-soroban. Newest entries are appended at the bottom.

## Week 1 (2026-04)

Kicked off the project scaffold. The workspace is split into three crates so
the off-chain prover and the on-chain verifier can share type definitions
through `zkml-common` without pulling Soroban dependencies into the prover or
RISC Zero dependencies into the contract.

## Week 2 (2026-05)

Fleshed out the `zkml-common` numeric core: fixed-point operations now cover
addition, subtraction, multiplication, and saturating variants, each with
tests. Added a shared `ZkmlError` type, a quantized ReLU activation, and a
minimal `Tensor` for moving data between dense layers. Next up is wiring these
into actual MLP inference in the prover.

### Note: matmul accumulation

The dense layer accumulates in `i64` after a per-product right shift. For the
tiny models we target (at most a few dozen neurons per layer) this keeps us
well inside the `i64` range. Larger layers would need an `i128` accumulator;
tracked as a follow-up if Phase 2 introduces bigger circuits.

## Week 4 (2026-05)

The off-chain pipeline now runs end to end for the supported model families:
import from JSON, quantize, infer, commit, and assemble a `VerificationBundle`.
On-chain, the verifier parses public inputs, records results, counts
verifications, and emits an event. The remaining gap is the real cryptography:
RISC Zero proving off-chain and the BN254 pairing check on-chain, both tracked
for the next phase.

## End of May (2026-05)

Closed out May with the numeric core hardened (checked add/sub/mul/div, dot
product, hex helpers), batch inference in the prover, and a full set of repo
hygiene additions: CI with fmt and clippy gates, issue and PR templates,
CODEOWNERS, Dependabot, a threat model, and security notes. The codebase is now
contributor-ready ahead of the Phase 1 cryptography work.
