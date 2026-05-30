# Threat Model

## Assets

- Model confidentiality: weights must not be recoverable from a proof.
- Input confidentiality: input features must not be revealed on-chain.
- Result integrity: a recorded result must correspond to the committed model.

## Adversaries and mitigations

| Threat | Mitigation |
| ------ | ---------- |
| Forged inference result | Groth16 proof verified on-chain via BN254 pairing. |
| Proof replay against another model | Model commitment fixed at `initialize` and checked in public inputs. |
| Proof replay against another input | Input commitment carried in public inputs. |
| Malformed public inputs | Length and structure validated before recording. |
| Non-determinism between prover and circuit | All arithmetic is fixed-point integer math. |

## Out of scope (current phase)

- Side-channel attacks on the off-chain prover.
- Denial of service through expensive proof submission (rate limiting is a
  deployment concern).
