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
