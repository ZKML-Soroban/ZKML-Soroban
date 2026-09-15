"""
zkml-soroban 05: Commitment scheme.
How model parameters and inputs become 32-byte Poseidon commitments.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
import graphviz
from _style import *

g = graphviz.Digraph("commitments")
g.attr(**base_graph_attr(
    rankdir="LR",
    label=hl("Commitment scheme", "Poseidon over BN254 Fr, circom parameters, width t = 3"),
))
g.attr("node", **base_node_attr())
g.attr("edge", **base_edge_attr())

g.node("model", hl("Quantized model", "weights, biases, thresholds, topology"), fillcolor=F_DATA, color=B_DATA)
g.node("inputs", hl("Input features", "Q16.16 raw i64 values"), fillcolor=F_DATA, color=B_DATA)
g.node("elements", hl("Field elements", "negative v maps to r - |v|"), fillcolor=F_OFFCHAIN, color=B_OFFCHAIN)
g.node("chunks", hl("Chunks of 2", "zero padded"), fillcolor=F_OFFCHAIN, color=B_OFFCHAIN)
g.node("chain", hl("Chained Poseidon", "h = P(c0 + h + index, c1)"), fillcolor=F_ZK, color=B_ZK, penwidth="2.2")
g.node("out", hl("Commitment", "32 bytes, little-endian"), fillcolor=F_SUCCESS, color=B_SUCCESS)

g.edge("model", "elements", label="model_elements()")
g.edge("inputs", "elements")
g.edge("elements", "chunks")
g.edge("chunks", "chain")
g.edge("chain", "chain", label="next chunk", style="dashed")
g.edge("chain", "out")

render(g, "05-commitments")
