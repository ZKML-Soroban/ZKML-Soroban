# Diagrams

Source of truth for the diagrams embedded in the documentation and the root README.
Each diagram is a small Python script that uses [Graphviz](https://graphviz.org) through the
[`graphviz`](https://pypi.org/project/graphviz/) package. Shared colors and helpers live in
`generator/_style.py`.

## Render

Requirements: Python 3.10+, the Graphviz `dot` binary on `PATH`, and the `graphviz` Python package.

```bash
# from the repository root, with uv (no global install of the Python package)
uv run --with graphviz python diagrams/generator/render-all.py

# or with pip
pip install graphviz
python diagrams/generator/render-all.py
```

Output goes to `docs/diagrams/` as SVG and PNG. Commit the rendered files together with the
script change. Documentation pages reference them as `/diagrams/<name>.svg`.

## Catalog

| # | Script | Shows |
| - | ------ | ----- |
| 1 | `01-system-architecture.py` | Off-chain prover, zkVM, on-chain verifier and consumers |
| 2 | `02-proof-lifecycle.py` | Ordered steps from model registration to a verified result |
| 3 | `03-verify-inference.py` | Every contract check in order and the error it returns |
| 4 | `04-public-inputs.py` | The 80-byte public input record and its BN254 scalars |
| 5 | `05-commitments.py` | How parameters and inputs become Poseidon commitments |
| 6 | `06-roadmap.py` | Delivery phases and the post-quantum track |
| 7 | `07-post-quantum.py` | Quantum exposure and the crypto-agile migration path |

## Style rules

- One idea per diagram, about ten nodes at most.
- Colors carry meaning: blue off-chain, violet zero-knowledge, purple Stellar, amber checks,
  green accepted, red rejected or at risk.
- Transparent background so diagrams work on light and dark themes.
