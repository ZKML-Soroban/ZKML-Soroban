"""
zkml-soroban 02: Proof lifecycle.
The ordered steps from model registration to a verified on-chain result.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
import graphviz
from _style import *

g = graphviz.Digraph("proof-lifecycle")
g.attr(**base_graph_attr(
    rankdir="TB",
    label=hl("Proof lifecycle", "One-time setup, then one proof per inference"),
))
g.attr("node", **base_node_attr(width="3.4"))
g.attr("edge", **base_edge_attr())

steps = [
    ("s1", "1. Register model", "initialize(admin, model_hash, vk)", F_ONCHAIN, B_ONCHAIN),
    ("s2", "2. Commit inputs", "input_hash = Poseidon(features)", F_OFFCHAIN, B_OFFCHAIN),
    ("s3", "3. Prove inference", "zkVM journal: hashes, output, class", F_ZK, B_ZK),
    ("s4", "4. Compress proof", "Groth16 seal: A, B, C", F_ZK, B_ZK),
    ("s5", "5. Submit transaction", "verify_inference(proof, public_inputs)", F_ONCHAIN, B_ONCHAIN),
    ("s6", "6. Record and emit", "InferenceRecord + verified event", F_SUCCESS, B_SUCCESS),
]
for node_id, title, subtitle, fill, border in steps:
    g.node(node_id, hl(title, subtitle), fillcolor=fill, color=border)

for current, following in zip(steps, steps[1:]):
    g.edge(current[0], following[0])

g.node("setup", hl("Once per model"), shape="note", fillcolor=F_DEFAULT, color=B_DEFAULT, width="1.6")
g.node("each", hl("Once per inference"), shape="note", fillcolor=F_DEFAULT, color=B_DEFAULT, width="1.6")
g.edge("setup", "s1", style="dashed", arrowhead="none")
g.edge("each", "s2", style="dashed", arrowhead="none")

for note, step in (("setup", "s1"), ("each", "s2")):
    with g.subgraph() as s:
        s.attr(rank="same")
        s.node(note)
        s.node(step)

render(g, "02-proof-lifecycle")
