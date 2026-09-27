"""
zkml-soroban 08: KYC demo data flow.
From the synthetic dataset to the result recorded on Stellar.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
import graphviz
from _style import *

g = graphviz.Digraph("kyc-demo-flow")
g.attr(**base_graph_attr(
    rankdir="TB",
    label=hl("KYC demo data flow", "examples/kyc-demo, from synthetic records to a verified score"),
))
g.attr("node", **base_node_attr(width="3.6"))
g.attr("edge", **base_edge_attr())

steps = [
    ("dataset", "generate_dataset.py", "1000 synthetic applicants, 10 features", F_DATA, B_DATA),
    ("train", "train_model.py", "DecisionTreeClassifier, depth 5", F_OFFCHAIN, B_OFFCHAIN),
    ("onnx", "kyc_decision_tree.onnx", "core opset 17, checked by check_model.py", F_DATA, B_DATA),
    ("import", "import_onnx", "extract the tree, quantize to Q16.16", F_OFFCHAIN, B_OFFCHAIN),
    ("infer", "Inference and commitments", "model_hash and input_hash, Poseidon", F_OFFCHAIN, B_OFFCHAIN),
    ("prove", "Proof", "zkVM journal, then Groth16 seal", F_ZK, B_ZK),
    ("verify", "verify_inference", "Soroban contract on Stellar", F_ONCHAIN, B_ONCHAIN),
    ("record", "InferenceRecord", "risk tier stored, verified event emitted", F_SUCCESS, B_SUCCESS),
]

for node_id, title, subtitle, fill, border in steps:
    g.node(node_id, hl(title, subtitle), fillcolor=fill, color=border)

for current, following in zip(steps, steps[1:]):
    g.edge(current[0], following[0])

g.node(
    "offchain",
    hl("Off chain"),
    shape="note",
    fillcolor=F_DEFAULT,
    color=B_DEFAULT,
    width="1.6",
)
g.node(
    "onchain",
    hl("On chain"),
    shape="note",
    fillcolor=F_DEFAULT,
    color=B_DEFAULT,
    width="1.6",
)
g.edge("offchain", "dataset", style="dashed", arrowhead="none")
g.edge("onchain", "verify", style="dashed", arrowhead="none")

for note, step in (("offchain", "dataset"), ("onchain", "verify")):
    with g.subgraph() as s:
        s.attr(rank="same")
        s.node(note)
        s.node(step)

render(g, "08-kyc-demo-flow")
