"""
zkml-soroban 06: Roadmap.
Delivery phases and the post-quantum readiness track.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
import graphviz
from _style import *

g = graphviz.Digraph("roadmap")
g.attr(**base_graph_attr(
    rankdir="LR",
    label=hl("Roadmap", "From a zkVM MVP to native circuits, an ecosystem and post-quantum readiness"),
))
g.attr("node", **base_node_attr(width="2.6"))
g.attr("edge", **base_edge_attr())

phases = [
    ("p1", "Phase 1: MVP", "RISC Zero zkVM + Groth16", "real proofs, testnet KYC demo", F_OFFCHAIN, B_OFFCHAIN),
    ("p2", "Phase 2: Native circuits", "BN254 circuits per model, model registry", "smaller proofs, cheaper verification", F_ZK, B_ZK),
    ("p3", "Phase 3: Ecosystem", "attestations, SDKs", "hosted prover, anchor integrations", F_ONCHAIN, B_ONCHAIN),
]
for node_id, title, line1, line2, fill, border in phases:
    g.node(node_id, hl(title, line1, line2), fillcolor=fill, color=border)
g.edge("p1", "p2")
g.edge("p2", "p3")

g.node("pq", hl("Post-quantum track", "crypto-agile verifier, hash-based commitments",
                "hybrid proofs, PQ verification research"),
       fillcolor=F_DECISION, color=B_DECISION, penwidth="2.2")
g.edge("p1", "pq", style="dashed", label="starts now")
g.edge("pq", "p3", style="dashed")

render(g, "06-roadmap")
