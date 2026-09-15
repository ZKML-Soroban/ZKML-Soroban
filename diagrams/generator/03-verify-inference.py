"""
zkml-soroban 03: verify_inference decision flow.
Every check the contract performs, in order, and the error each one returns.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
import graphviz
from _style import *

g = graphviz.Digraph("verify-inference")
g.attr(**base_graph_attr(
    rankdir="TB",
    nodesep="0.35",
    ranksep="0.45",
    label=hl("verify_inference", "Checks in execution order (contract VERSION 5)"),
))
g.attr("node", **base_node_attr())
g.attr("edge", **base_edge_attr())

g.node("start", hl("Transaction", "proof_a, proof_b, proof_c, public_inputs"),
       fillcolor=F_ONCHAIN, color=B_ONCHAIN)

checks = [
    ("init", "Initialized?", "ContractNotInitialized (1)"),
    ("paused", "Not paused?", "VerificationFailed (7)"),
    ("points", "Proof points well formed?", "MalformedProofA (3, also for proof_c) / MalformedProofB (4)"),
    ("layout", "Public inputs exactly 80 bytes?", "PublicInputsTooShort (2) / InvalidPublicInputLength (8)"),
    ("model", "model_hash matches registered?", "VerificationFailed (7)"),
    ("vk", "Verification key well formed, 5 IC points?", "MalformedVerificationKey (6) / VerificationKeyLengthMismatch (9)"),
    ("pairing", "Groth16 pairing check holds?", "VerificationFailed (7)"),
    ("nullifier", "Nullifier unused?", "ProofAlreadyUsed (10)"),
]

previous = "start"
for node_id, question, error in checks:
    error_id = f"{node_id}_err"
    g.node(node_id, hl(question), style="filled", fillcolor=F_DECISION, color=B_DECISION)
    g.node(error_id, hl(error), fillcolor=F_DANGER, color=B_DANGER, fontsize="10")
    if previous == "start":
        g.edge(previous, node_id)
    else:
        g.edge(previous, node_id, label="yes", color=E_SUCCESS, fontcolor=E_SUCCESS)
    g.edge(node_id, error_id, label="no", color=E_DANGER, fontcolor=E_DANGER)
    with g.subgraph() as s:
        s.attr(rank="same")
        s.node(node_id)
        s.node(error_id)
    previous = node_id

g.node("ok", hl("Accepted", "store nullifier, record result, bump TTL, emit verified"),
       fillcolor=F_SUCCESS, color=B_SUCCESS, penwidth="2.4")
g.edge(previous, "ok", label="yes", color=E_SUCCESS, fontcolor=E_SUCCESS)

render(g, "03-verify-inference")
