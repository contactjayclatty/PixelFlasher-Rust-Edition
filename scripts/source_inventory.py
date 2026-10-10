"""Regenerate the source function inventory from the pinned upstream checkout."""
import ast
import json
from pathlib import Path
import subprocess
import sys

project = Path(__file__).resolve().parents[1]
reference = Path(sys.argv[1]) if len(sys.argv) > 1 else project.parent / 'references/PixelFlasher'
revision = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=reference, text=True).strip()
if revision != '081286d6baf8ac969df29e8b0ab3a5b6ae4cfb54':
    raise SystemExit('Reference revision changed. Review the parity baseline before regenerating.')
rows = []
for path in sorted(reference.glob('*.py')):
    text = path.read_text(encoding='utf-8-sig')
    tree = ast.parse(text)
    functions = [{'name': n.name, 'line': n.lineno, 'end': n.end_lineno, 'class': None}
                 for n in tree.body if isinstance(n, (ast.FunctionDef, ast.AsyncFunctionDef))]
    for cls in tree.body:
        if isinstance(cls, ast.ClassDef):
            functions.extend({'name': n.name, 'line': n.lineno, 'end': n.end_lineno, 'class': cls.name}
                             for n in cls.body if isinstance(n, (ast.FunctionDef, ast.AsyncFunctionDef)))
    rows.append({'file': path.name, 'lines': len(text.splitlines()), 'functions': functions})
(project / 'docs/source-inventory.json').write_text(json.dumps(rows, indent=2), encoding='utf-8')
print(json.dumps({'revision': revision, 'lines': sum(r['lines'] for r in rows),
                  'functions': sum(len(r['functions']) for r in rows)}))
