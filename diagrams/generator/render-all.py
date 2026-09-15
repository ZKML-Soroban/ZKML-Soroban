"""
Render every zkml-soroban diagram to docs/diagrams/ (SVG + PNG).

Usage, from the repository root:
    uv run --with graphviz python diagrams/generator/render-all.py
"""
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]

errors = []
for script in sorted(HERE.glob("[0-9][0-9]-*.py")):
    result = subprocess.run([sys.executable, str(script)], cwd=ROOT, capture_output=True, text=True)
    if result.returncode != 0:
        print(f"\nFAILED {script.name}\n{result.stderr}")
        errors.append(script.name)
    elif result.stdout:
        print(result.stdout.strip())

if errors:
    print(f"\n{len(errors)} script(s) failed: {errors}")
    sys.exit(1)

print(f"\nAll diagrams rendered to {ROOT / 'docs' / 'diagrams'}")
