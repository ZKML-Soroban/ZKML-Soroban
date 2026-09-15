"""
zkml-soroban 01: System architecture.
Off-chain prover, zero-knowledge proving, on-chain verification and consumers.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
import graphviz
from _style import *

g = graphviz.Digraph("system-architecture")
g.attr(**base_graph_attr(
    rankdir="LR",
    label=hl("zkml-soroban: System Architecture",
             "Inference runs off-chain, correctness is verified on Stellar"),
))
g.attr("node", **base_node_attr())
g.attr("edge", **base_edge_attr())

g.node("model", hl("Trained model", "ONNX or JSON"), fillcolor=F_DATA, color=B_DATA)
g.node("inputs", hl("Private inputs", "user features"), fillcolor=F_DATA, color=B_DATA)

with g.subgraph(name="cluster_offchain") as c:
    c.attr(**cluster_attr("Off-chain", "zkml-prover + zkml-common", B_OFFCHAIN))
    c.node("import", hl("Import and quantize", "Q16.16 fixed point"), fillcolor=F_OFFCHAIN, color=B_OFFCHAIN)
    c.node("commit", hl("Poseidon commitments", "model_hash, input_hash"), fillcolor=F_OFFCHAIN, color=B_OFFCHAIN)
    c.node("zkvm", hl("RISC Zero zkVM", "deterministic inference"), fillcolor=F_ZK, color=B_ZK)
    c.node("groth16", hl("Groth16 wrapper", "STARK to SNARK (pending)"), fillcolor=F_ZK, color=B_ZK)

with g.subgraph(name="cluster_onchain") as c:
    c.attr(**cluster_attr("Stellar / Soroban", "zkml-verifier contract", B_ONCHAIN))
    c.node("verify", hl("verify_inference", "BN254 pairing check (CAP-0074)"),
           fillcolor=F_ONCHAIN, color=B_ONCHAIN, penwidth="2.4")
    c.node("state", hl("Contract state", "record, nullifiers, events"), fillcolor=F_SUCCESS, color=B_SUCCESS)

g.node("consumers", hl("Consumers", "lending pools, anchors, auditors"), fillcolor=F_DEFAULT, color=B_DEFAULT)

g.edge("model", "import")
g.edge("inputs", "commit")
g.edge("import", "commit")
g.edge("commit", "zkvm")
g.edge("zkvm", "groth16", label="receipt")
g.edge("groth16", "verify", label="proof + 80-byte public inputs",
       color=B_ONCHAIN, fontcolor=B_ONCHAIN, penwidth="2")
g.edge("verify", "state", label="if valid", color=E_SUCCESS, fontcolor=E_SUCCESS)
g.edge("state", "consumers", label="query / events")

render(g, "01-system-architecture")
