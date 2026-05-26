# FAQ

**Why fixed-point instead of floating-point?**
ZK circuits operate over finite fields and cannot represent IEEE floats. Fixed
-point keeps every operation an exact integer computation that a circuit can
constrain deterministically.

**Why omit the sigmoid in logistic regression?**
The sigmoid is expensive and non-linear to constrain. We compare the raw linear
score against a threshold instead, which is equivalent for a binary decision.

**Why RISC Zero before native circuits?**
RISC Zero lets us prove ordinary Rust inference code without hand-writing a
circuit, so we reach a working end-to-end pipeline quickly. Native BN254 and
Poseidon circuits are a Phase 2 optimization.

**What model sizes are supported?**
Small models: decision trees, logistic regression, and tiny MLPs. The proving
cost grows with the circuit size, so large networks are out of scope.
