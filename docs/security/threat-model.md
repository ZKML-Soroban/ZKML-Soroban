---
title: "Threat model"
description: "Assets, adversaries, mitigations, and what is out of scope."
icon: "user-shield"
---

## Assets

- **Result integrity:** a recorded result must come from the committed model
  evaluated on the committed input.
- **Model confidentiality:** weights are not published on-chain; only their
  commitment is.
- **Input confidentiality:** input features are not published on-chain; only
  their commitment is.
- **Contract control:** only the admin can change the key, the model, the admin,
  or the pause flag.

## Trust assumptions

- Groth16 over BN254 is sound and the verification key was generated honestly.
- The prover operator sees model and inputs in the clear (zkVM proving is not
  confidential computing).
- The admin key is held securely; the admin can replace the model commitment and
  verification key at any time.

## Threats and mitigations

| Threat | Mitigation | Status |
| ------ | ---------- | ------ |
| Forged inference result | Groth16 pairing check with BN254 host functions | Implemented (contract side; no production proofs yet) |
| Proof for a different model | `model_hash` public input must equal the stored commitment | Implemented |
| Input substitution | `input_hash` is a public input bound by the proof | Implemented (contract side; no production proofs yet) |
| Replay of an accepted proof | SHA-256 nullifier over the public inputs in persistent storage | Implemented |
| Malformed or oversized public inputs | Exact 80-byte layout, per-field length checks | Implemented |
| Verification key / input count mismatch | `ic.len()` must equal public inputs + 1 | Implemented |
| Unauthorized re-initialization or key change | `initialize` once with admin auth; admin-gated setters | Implemented |
| Compromised model or key in production | `set_pause` emergency stop, key rotation | Implemented |
| Contract archival from inactivity | Instance TTL bumps on init and successful verification | Implemented |
| Non-determinism between prover and guest | Fixed-point integer math, shared inference engine, journal cross-check | Implemented |
| Malicious tree causing infinite traversal | Structural validation and bounded iteration | Implemented |
| Arithmetic overflow changing results | Checked `i128` intermediates, static overflow bounds in `validate` | Implemented |
| Accepting RISC Zero proofs with mismatched public inputs | Route A verifier adapter | Pending |
| Proof forgery by a quantum computer (Shor on BN254) | Crypto-agile verifier, hybrid and hash-based proofs, see [post-quantum readiness](/security/post-quantum) | Planned |

## Out of scope (current phase)

- Side-channel attacks on the off-chain prover.
- Denial of service through expensive submissions (a deployment concern).
- Model extraction through repeated queries of a public scoring service.
- Quantum adversaries: BN254 pairings and Groth16 are not post-quantum secure.

See also [Known limitations](/security/known-limitations) and
[Security notes](/security/security-notes).
