"""
zkml-soroban 07: Post-quantum readiness.
What a large quantum computer breaks, and the crypto-agile path around it.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
import graphviz
from _style import *

g = graphviz.Digraph("post-quantum")
g.attr(**base_graph_attr(
    rankdir="LR",
    label=hl("Post-quantum readiness",
             "The verifier dispatches on a proof-system id, so cryptography can change without breaking consumers"),
))
g.attr("node", **base_node_attr())
g.attr("edge", **base_edge_attr())

with g.subgraph(name="cluster_risk") as c:
    c.attr(**cluster_attr("Exposure to a quantum computer", "Shor breaks, Grover weakens", B_DANGER))
    c.node("groth", hl("Groth16 on BN254", "soundness broken: proofs can be forged"), fillcolor=F_DANGER, color=B_DANGER)
    c.node("sig", hl("ed25519 account signatures", "broken, Stellar protocol level"), fillcolor=F_DANGER, color=B_DANGER)
    c.node("hash", hl("Poseidon, SHA-256, Keccak", "weakened only"), fillcolor=F_SUCCESS, color=B_SUCCESS)

g.node("agile", hl("Crypto-agile verifier", "proof_system id in public inputs"),
       fillcolor=F_ONCHAIN, color=B_ONCHAIN, penwidth="2.4")

with g.subgraph(name="cluster_path") as c:
    c.attr(**cluster_attr("Migration path", "same consumer interface at every stage", B_ZK))
    c.node("m1", hl("Today", "Groth16 / BN254"), fillcolor=F_ZK, color=B_ZK)
    c.node("m2", hl("Hybrid", "Groth16 + hash-based proof"), fillcolor=F_ZK, color=B_ZK)
    c.node("m3", hl("Post-quantum", "hash-based STARK verification"), fillcolor=F_ZK, color=B_ZK)

g.node("consumers", hl("Consumers unchanged", "is_attested(subject, model_id), planned"), fillcolor=F_SUCCESS, color=B_SUCCESS)

g.edge("groth", "agile", style="dashed", color=E_DANGER)
g.edge("hash", "agile", style="dashed", color=E_SUCCESS)
g.edge("agile", "m1")
g.edge("m1", "m2")
g.edge("m2", "m3")
g.edge("m3", "consumers")

render(g, "07-post-quantum")
