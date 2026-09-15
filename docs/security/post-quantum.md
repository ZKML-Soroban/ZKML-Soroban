---
title: "Post-quantum readiness"
description: "What a cryptographically relevant quantum computer breaks in zkml-soroban, what it does not, and the migration plan."
icon: "atom"
---

zkml-soroban records machine learning results that lenders, anchors and auditors may rely on for
years. This page explains how the project stays trustworthy when large quantum computers arrive.

<Note>
Post-quantum readiness is a roadmap track. The current verifier uses Groth16 over BN254, which is
not quantum resistant. Nothing on this page is deployed yet.
</Note>

## Exposure

A cryptographically relevant quantum computer (CRQC) running Shor's algorithm solves discrete
logarithms on elliptic curves. Grover's algorithm only gives a quadratic speedup against hash
functions.

| Component | Primitive | Impact of a CRQC |
| --------- | --------- | ---------------- |
| Proof verification | Groth16 on BN254 | **Broken.** Soundness relies on discrete log hardness, so an attacker could forge proofs for results the model never produced. |
| Model and input commitments | Poseidon over BN254 Fr | Weakened, not broken. Binding comes from the hash, not from curve discrete log. |
| Replay protection | SHA-256 nullifiers | Weakened, not broken. |
| zkVM execution proof | RISC Zero STARK (FRI, hash based) | Believed quantum resistant. The Groth16 wrapper around it is not. |
| Account signatures | ed25519 (Stellar protocol) | **Broken, and it affects us.** A recovered admin key could call `set_verification_key` and make the contract accept anything. Protocol-level migration is covered by Stellar's Quantum Preparedness Plan. |
| Witness privacy | Groth16 zero knowledge | Not exposed. Groth16 is perfectly zero knowledge, so a quantum attacker can forge proofs but cannot recover private inputs from existing proofs. |

Past verifications stay meaningful as historical records: they were checked when forging was
infeasible. The risk is accepting **new** forged proofs once a CRQC exists.

Stellar's [Quantum Preparedness Plan](https://stellar.org/blog/foundation-news/introducing-the-quantum-preparedness-plan)
covers account signatures but states that it does not yet address pairing-based zero-knowledge
protocols such as SNARKs over BN254, and that there is no drop-in post-quantum replacement for
them. That gap is exactly what this track works on.

## Principles

1. **Crypto-agility first.** Consumers ask "is this subject attested by this model?", never "is
   this Groth16 proof valid?". The proof system can then change behind a stable interface.
2. **Versioned everything.** Proof systems, commitment schemes and public input layouts carry
   explicit version identifiers.
3. **Hybrid before replacement.** High-value models can require two proofs, one of them hash
   based, before a full migration. Note that a RISC Zero STARK and its Groth16 wrapper prove the
   same zkVM execution, so they are not fully independent systems.
4. **Fail closed.** If a proof system is deprecated, the verifier rejects it; `set_pause` and key
   rotation are the emergency levers.

## Migration path

<Frame>
  <img src="/diagrams/07-post-quantum.svg" alt="Post-quantum exposure and crypto-agile migration path" />
</Frame>

| Stage | Change | Status |
| ----- | ------ | ------ |
| 1. Crypto-agile verifier | Proof system identifier in the bundle and public inputs; contract dispatches per system. | Planned |
| 2. Versioned commitments | Hash-based (SHA-256 or Keccak) commitment option next to Poseidon; re-attestation of stored records. | Planned |
| 3. Hybrid mode | Registry flag that requires two independent proofs per inference. | Planned |
| 4. Post-quantum verification | Research spike: verify a hash-based STARK proof on Soroban, implementing the required field (BabyBear) and hash (Poseidon2) arithmetic in WASM and splitting the work across several transactions. Output is a go / no-go report with CPU measurements. | Research |
| 5. Post-quantum admin keys | Move admin control to a contract account that authenticates with ML-DSA once those host functions ship. | Planned |

## Constraints on Soroban

- Soroban exposes BN254, BLS12-381, SHA-256, Keccak-256 and Poseidon host functions. As of
  September 2026 there are no post-quantum host functions on mainnet; ML-DSA-44 and ML-DSA-65
  signature verification is planned in Stage 1 of the Quantum Preparedness Plan. There is no
  planned host function for post-quantum proof verification.
- A full STARK verification likely exceeds the per-transaction CPU budget, which is why stage 4
  explores incremental verification across transactions.
- Stage 1 must land after real Groth16 proofs are verified end to end (see
  [known limitations](/security/known-limitations)).

## References

- [Stellar Quantum Preparedness Plan](https://stellar.org/blog/foundation-news/introducing-the-quantum-preparedness-plan): Stellar's migration roadmap, including ML-DSA host functions.
- [NIST FIPS 203, 204 and 205](https://csrc.nist.gov/projects/post-quantum-cryptography): standardized post-quantum KEM and signatures (August 2024).
- [NIST IR 8547 (initial public draft)](https://csrc.nist.gov/pubs/ir/8547/ipd): transition timeline away from quantum-vulnerable algorithms.
- [RISC Zero proof system](https://dev.risczero.com/proof-system/): STARK-based receipts and the Groth16 wrapper.
- [Threat model](/security/threat-model)
