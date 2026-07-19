# ONNX import fixtures

Small `.onnx` files used by `tests/onnx_import.rs` to exercise the importer
foundation (protobuf parse, opset check, operator allowlist).

## Files

| File | Purpose |
| ---- | ------- |
| `decision_tree_valid.onnx` | Opset 17 + `TreeEnsembleClassifier`. Validation passes; import returns `ExtractionNotImplemented` until issue #5. |
| `linear_classifier_valid.onnx` | Opset 18 + `LinearClassifier`. Same extraction deferral (issue #6). |
| `unsupported_conv.onnx` | Opset 17 + `Conv`. Must fail with `UnsupportedOperator { op_type: "Conv" }`. |
| `low_opset_tree.onnx` | Opset 13 + `TreeEnsembleClassifier`. Must fail with `UnsupportedOpset`. |

## How these fixtures were generated

The committed binaries are **synthetic `ModelProto` encodings** written with
the same `prost` field tags as official ONNX. They deliberately omit full
attribute / initializer payloads because parameter extraction is out of scope
for the foundation issue.

Regenerate them with:

```bash
cargo run -p zkml-prover --example generate_onnx_fixtures
```

### Optional: skl2onnx decision tree (reference)

When Python tooling is available, a production-style tree can be exported as
follows (for local experiments; not required by CI):

```bash
pip install "scikit-learn>=1.4" "skl2onnx>=1.16" "onnx>=1.15"
```

```python
# scripts/export_tree_fixture.py (not committed; reference only)
from skl2onnx import convert_sklearn
from skl2onnx.common.data_types import FloatTensorType
from sklearn.datasets import load_iris
from sklearn.tree import DecisionTreeClassifier

X, y = load_iris(return_X_y=True)
clf = DecisionTreeClassifier(max_depth=2, random_state=0).fit(X, y)
onx = convert_sklearn(
    clf,
    initial_types=[("X", FloatTensorType([None, X.shape[1]]))],
    target_opset={"": 17, "ai.onnx.ml": 17},
)
with open("decision_tree_skl2onnx.onnx", "wb") as f:
    f.write(onx.SerializeToString())
```

The foundation importer will accept that file's operators and opset, then
return `ExtractionNotImplemented` until issue #5 lands.

## Design note

Fixtures stay tiny (a few hundred bytes) so reviews stay readable and CI stays
fast. Full weight tensors belong with the extraction issues (#5 / #6).
