"""
zkml-soroban 04: Public inputs layout.
The 80-byte public input record and how each field maps to a BN254 scalar.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
import graphviz
from _style import *

g = graphviz.Digraph("public-inputs")
g.attr(**base_graph_attr(
    rankdir="TB",
    label=hl("Public inputs", "80 bytes, little-endian, one BN254 scalar per field"),
))
g.attr("node", **base_node_attr())
g.attr("edge", **base_edge_attr())


def cell(port: str, name: str, span: str, width: int) -> str:
    return (
        f'<TD PORT="{port}" BGCOLOR="{F_DATA}" WIDTH="{width}">'
        f'<B>{name}</B><BR/><FONT POINT-SIZE="9" COLOR="{T_MED}">{span}</FONT></TD>'
    )


cells = "".join([
    cell("m", "model_hash", "bytes 0..32", 220),
    cell("i", "input_hash", "bytes 32..64", 220),
    cell("o", "output", "64..72", 80),
    cell("c", "class_label", "72..80", 80),
])
table = (
    f'<<TABLE BORDER="0" CELLBORDER="1" CELLSPACING="0" CELLPADDING="10" COLOR="{B_DATA}">'
    f"<TR>{cells}</TR></TABLE>>"
)
g.node("record", table, shape="plain")

scalars = [
    ("m", "x1", "Poseidon commitment"),
    ("i", "x2", "Poseidon commitment"),
    ("o", "x3", "i64, raw Q16.16"),
    ("c", "x4", "i64 class label"),
]
g.node("L", hl("L = IC0 + x1 IC1 + x2 IC2 + x3 IC3 + x4 IC4",
               "the verification key needs exactly 5 IC points"),
       fillcolor=F_ONCHAIN, color=B_ONCHAIN, penwidth="2.2")

for port, scalar, meaning in scalars:
    g.node(scalar, hl(scalar, meaning), fillcolor=F_ZK, color=B_ZK)
    g.edge(f"record:{port}:s", scalar)
    g.edge(scalar, "L")

render(g, "04-public-inputs")
