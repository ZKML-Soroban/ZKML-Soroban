# Engineering Devlog

A running, chronological log of development decisions and progress on
zkml-soroban. Newest entries are appended at the bottom.

## Week 1 (2026-04)

Kicked off the project scaffold. The workspace is split into three crates so
the off-chain prover and the on-chain verifier can share type definitions
through `zkml-common` without pulling Soroban dependencies into the prover or
RISC Zero dependencies into the contract.
