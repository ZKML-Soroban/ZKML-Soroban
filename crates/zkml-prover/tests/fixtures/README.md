# ONNX import fixtures

Small `.onnx` files used by `tests/onnx_import.rs` to exercise the importer
foundation (protobuf parse, opset check, operator allowlist).

## Files

| File | Purpose |
| ---- | ------- |
| `decision_tree_valid.onnx` | Core opset 17 + `ai.onnx.ml` 3 + `TreeEnsembleClassifier`. Validation passes; import returns `ExtractionNotImplemented` until issue #5. |
| `skl2onnx_like_tree.onnx` | Same realistic opset pair with an skl2onnx-style producer/graph name. Guards against regressing to an ml=17 floor. |
| `linear_classifier_valid.onnx` | Core 18 + `ai.onnx.ml` 1 + `LinearClassifier`. Extraction deferred (issue #6). |
| `unsupported_conv.onnx` | Core 17 + `Conv`. Must fail with `UnsupportedOperator { op_type: "Conv" }`. |
| `low_opset_tree.onnx` | Core 13 + ml 3 + tree op. Must fail with `UnsupportedOpset` on the core domain. |
| `tinymlp_valid.onnx` | Core 17 + Gemm + Relu + Gemm. 2-layer MLP matching the golden network in `tinymlp_inference.rs`. Validation passes and imports into a TinyMLP. |
| `skl2onnx_real_tree.onnx` | Written by skl2onnx, not by this crate. An iris tree of depth 2. Guards the wire types in `proto.rs` against drifting from the ONNX schema. |
| `onnx_helper_mlp.onnx` | Written by the official `onnx` Python library: Gemm, Relu, Gemm with real initialisers. Guards `TensorProto`, which the attribute-only fixtures never touch. |

## How these fixtures were generated

Every file except `skl2onnx_real_tree.onnx` and `onnx_helper_mlp.onnx` is a
**synthetic `ModelProto` encoding** written with the same `prost` field tags the
importer decodes with.
That makes them self-consistent, so a field declared with the wrong wire type
stays invisible to them: this is how `AttributeProto.floats` sat as `double`
instead of `float`, `GraphProto.input` on tag 3 instead of 11, and the whole of
`TensorProto` sat one tag out of place, while every test passed. The two real
files exist to close that blind spot, and neither is produced by the generator
below, so the recipes for both are kept here.

Opset pairs mirror real exporters: **never** set `ai.onnx.ml` to 17 (that
domain tops out around 5).

Regenerate them with:

```bash
cargo run -p zkml-prover --example generate_onnx_fixtures
```

### Regenerating the real fixtures

Both are committed, so this is only needed if they have to change. Both need
Python tooling:

```bash
pip install "scikit-learn>=1.4" "skl2onnx>=1.16" "onnx>=1.15"
```

#### `skl2onnx_real_tree.onnx`

An iris tree of depth 2, written by skl2onnx. Its repeated attributes are
non-packed, which is how official ONNX writes them and `prost` does not, so
this file could not have come from this crate.

```python
from skl2onnx import convert_sklearn
from skl2onnx.common.data_types import FloatTensorType
from sklearn.datasets import load_iris
from sklearn.tree import DecisionTreeClassifier

X, y = load_iris(return_X_y=True)
clf = DecisionTreeClassifier(max_depth=2, random_state=0).fit(X, y)
onx = convert_sklearn(
    clf,
    initial_types=[("X", FloatTensorType([None, X.shape[1]]))],
    # Core domain 17; ml domain stays in 1–5 (skl2onnx rejects ml=17).
    target_opset={"": 17, "ai.onnx.ml": 3},
    options={type(clf): {"zipmap": False}},
)
with open("crates/zkml-prover/tests/fixtures/skl2onnx_real_tree.onnx", "wb") as f:
    f.write(onx.SerializeToString())
```

This produces an equivalent file rather than an identical one: skl2onnx names
the graph with a fresh random identifier on every export, so the bytes differ
while everything the importer reads is the same. The `onnx.helper` recipe below
is byte for byte reproducible.

#### `onnx_helper_mlp.onnx`

A two layer MLP built with the official `onnx` library, so its weights arrive
as real `TensorProto` initialisers. `onnx.checker` validates it before it is
written.

```python
import numpy as np
import onnx
from onnx import helper, numpy_helper, TensorProto as TP

W1 = np.array([[1.0, 0.5], [-0.5, 1.0]], dtype=np.float32)
B1 = np.array([0.0, 0.1], dtype=np.float32)
W2 = np.array([[1.0], [-1.0]], dtype=np.float32)
B2 = np.array([0.0], dtype=np.float32)

graph = helper.make_graph(
    [
        helper.make_node("Gemm", ["X", "W1", "B1"], ["h"], name="layer1"),
        helper.make_node("Relu", ["h"], ["a"], name="act"),
        helper.make_node("Gemm", ["a", "W2", "B2"], ["Y"], name="layer2"),
    ],
    "tiny_mlp",
    [helper.make_tensor_value_info("X", TP.FLOAT, [None, 2])],
    [helper.make_tensor_value_info("Y", TP.FLOAT, [None, 1])],
    initializer=[
        numpy_helper.from_array(W1, "W1"),
        numpy_helper.from_array(B1, "B1"),
        numpy_helper.from_array(W2, "W2"),
        numpy_helper.from_array(B2, "B2"),
    ],
)
model = helper.make_model(graph, opset_imports=[helper.make_opsetid("", 17)])
model.ir_version = 9
onnx.checker.check_model(model)
onnx.save(model, "crates/zkml-prover/tests/fixtures/onnx_helper_mlp.onnx")
```

## Design note

Fixtures stay tiny (a few hundred bytes) so reviews stay readable and CI stays
fast. Full weight tensors belong with the extraction issues (#5 / #6).
