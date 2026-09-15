"""
Shared design system for zkml-soroban diagrams.

Transparent background, light cards with strong borders so every diagram reads
well on both the light and dark Mintlify themes.
"""

from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
OUTPUT = ROOT / "docs" / "diagrams"

# Background and typography
BGCOLOR = "transparent"
FONT = "Helvetica"
T_DARK = "#1E293B"  # slate-900, primary text
T_MED = "#475569"  # slate-600, secondary text and edge labels

# Node fills (light, opaque cards)
F_DEFAULT = "#F8FAFC"  # slate-50
F_OFFCHAIN = "#EFF6FF"  # blue-50, off-chain prover side
F_ZK = "#F5F3FF"  # violet-50, zero-knowledge machinery
F_ONCHAIN = "#FAF5FF"  # purple-50, Stellar / Soroban
F_DECISION = "#FFFBEB"  # amber-50, checks and gates
F_SUCCESS = "#ECFDF5"  # emerald-50, accepted / recorded
F_DANGER = "#FEF2F2"  # red-50, rejected / at risk
F_DATA = "#F0F9FF"  # sky-50, data artifacts

# Borders
B_DEFAULT = "#64748B"
B_OFFCHAIN = "#2563EB"
B_ZK = "#7C3AED"
B_ONCHAIN = "#7D00FF"
B_DECISION = "#D97706"
B_SUCCESS = "#059669"
B_DANGER = "#DC2626"
B_DATA = "#0284C7"

# Edges
E_DEFAULT = "#94A3B8"
E_SUCCESS = "#059669"
E_DANGER = "#DC2626"


def render(g, name: str) -> None:
    """Render a graph to SVG and PNG under docs/diagrams/."""
    OUTPUT.mkdir(parents=True, exist_ok=True)
    (OUTPUT / f"{name}.svg").write_bytes(g.pipe(format="svg"))
    (OUTPUT / f"{name}.png").write_bytes(g.pipe(format="png"))
    print(f"  rendered {name} (.svg + .png)")


def base_graph_attr(**extra):
    return {
        "bgcolor": BGCOLOR,
        "fontname": FONT,
        "fontsize": "14",
        "fontcolor": T_DARK,
        "labelloc": "t",
        "labeljust": "l",
        "pad": "0.5",
        "nodesep": "0.5",
        "ranksep": "0.7",
        "dpi": "150",
        **extra,
    }


def base_node_attr(**extra):
    return {
        "shape": "box",
        "style": "filled,rounded",
        "fillcolor": F_DEFAULT,
        "color": B_DEFAULT,
        "fontname": FONT,
        "fontsize": "11",
        "fontcolor": T_DARK,
        "margin": "0.2,0.12",
        "penwidth": "1.6",
        **extra,
    }


def base_edge_attr(**extra):
    return {
        "color": E_DEFAULT,
        "fontname": FONT,
        "fontsize": "10",
        "fontcolor": T_MED,
        "arrowsize": "0.8",
        "penwidth": "1.4",
        **extra,
    }


def cluster_attr(title: str, subtitle: str, color: str):
    return {
        "label": hl(title, subtitle),
        "style": "rounded,dashed",
        "color": color,
        "fontcolor": color,
        "fontname": FONT,
        "fontsize": "12",
        "penwidth": "2",
        "margin": "16",
    }


def _safe(text: str) -> str:
    """Escape text for Graphviz HTML-like labels."""
    return (
        text.replace("&", "&amp;")
        .replace("<", "&lt;")
        .replace(">", "&gt;")
        .replace("\n", "<BR/>")
    )


def hl(title: str, subtitle: str = "", subtitle2: str = "") -> str:
    """HTML label: bold title plus up to two smaller subtitle lines."""
    s = f"<B>{_safe(title)}</B>"
    for line in (subtitle, subtitle2):
        if line:
            s += f'<BR/><FONT POINT-SIZE="9" COLOR="{T_MED}">{_safe(line)}</FONT>'
    return f"<{s}>"
