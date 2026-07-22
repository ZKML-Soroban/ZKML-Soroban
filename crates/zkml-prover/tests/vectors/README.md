# Golden vectors (prover)

Self-describing JSON cases used to cross-check native inference against the
RISC Zero guest journal (issue #10).

## Format

Each file contains a `cases` array. Every case has:

| Field | Meaning |
| ----- | ------- |
| `name` | Stable identifier |
| `comment` | Human-readable intent |
| `model` | JSON model (same schema as `model_io::JsonModel` / `docs/model-format.md`) |
| `inputs` | Feature vector as `f64` (quantized with default Q16.16) |
| `expected_output_raw` | Expected `FixedPoint::value` (raw scaled integer) |

## Relation to issue #8

Issue #8 tracks a fuller golden-vector suite. These files use a compatible
layout so the suites can be merged without reshaping callers. Expected
outputs are raw Q16.16 integers to avoid float ambiguity:

- `0.0` → `0`
- `1.0` → `65536` (`1 << 16`)
- `0.25` → `16384`

## Regenerating expected values

```bash
# After loading a case via model_io + FixedPoint::quantize:
# expected_output_raw = run_inference(&model, &inputs).value
```
