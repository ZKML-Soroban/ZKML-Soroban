# Benchmarks

Baseline numbers for native (non-proving) inference, measured with the `timing`
feature on a developer laptop. These are sanity references, not optimized
figures; the proving cost dominates once the zkVM is integrated.

| Model               | Inputs | Native inference |
| ------------------- | ------ | ---------------- |
| Logistic regression | 4      | < 1 us           |
| Decision tree       | 3      | < 1 us           |
| Tiny MLP (8-8-1)    | 8      | a few us         |

Proving-time benchmarks will be added once the RISC Zero pipeline lands.

---

## On-Chain Verifier Resource Budget (Soroban)

Resource budget consumed by the `ZkmlVerifierContract` during a complete `verify_inference` call (including public input L vector assembly via G1 operations and BN254 pairing check over Protocol 25 host functions), measured via the Soroban SDK cost estimation harness (`env.cost_estimate().budget()`):

| Verification Operation | CPU Instructions | Memory (Bytes) | Regression Threshold (Max CPU) |
| ---------------------- | ---------------- | -------------- | ------------------------------ |
| Full `verify_inference` (BN254 Pairing + L Assembly) | 21,164,342 | 250,277 | 50,000,000 |

### Resource Budget Breakdown & Notes
- **L Scalar Mult & Assembly**: 4 G1 scalar multiplications and 3 G1 additions to assemble public input accumulator $L = \text{IC}_0 + \sum x_i \cdot \text{IC}_i$.
- **Pairing Check**: 4-pair BN254 pairing check $e(-A, B) \cdot e(\alpha, \beta) \cdot e(L, \gamma) \cdot e(C, \delta) = 1$ executed via Protocol 25 BN254 host function (`CAP-0074`).
- **Regression Threshold**: Enforced by automated test harness (`test_verifier_accept_path_and_resource_budget`) to fail the build if verification cost regresses sharply.
