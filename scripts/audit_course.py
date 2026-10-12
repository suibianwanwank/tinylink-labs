#!/usr/bin/env python3
"""Check independent snapshots and assignment boundaries."""
from pathlib import Path
import ast
import argparse
import re
import sys

ROOT = Path(__file__).resolve().parents[1]
MODULES = ["elf_header", "sections", "symbols", "resolve", "executable",
           "relocate", "layout", "archive", "gc", "pie", "unwind", "tls",
           "debug", "icf", "driver", "fuzz"]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--starter', action='store_true', help='Require every assignment task hole for publication')
    args = parser.parse_args()
    errors = []
    for i, assigned in enumerate(MODULES):
        package = ROOT / f"class{i}"
        manifest = (package / "Cargo.toml").read_text()
        if re.search(r'path\s*=|workspace\s*=\s*true', manifest):
            errors.append(f"class{i}: manifest must be standalone")
        if f'implementation = "src/{assigned}.rs"' not in manifest:
            errors.append(f"class{i}: missing assignment metadata")
        for j, module in enumerate(MODULES[:i + 1]):
            path = package / "src" / f"{module}.rs"
            if not path.is_file():
                errors.append(f"class{i}: missing {module}")
                continue
            text = path.read_text()
            if re.search(r'\b(?:class\d+|tinylink)::', text):
                errors.append(f"{path.relative_to(ROOT)}: cross-stage namespace")
            if j < i and re.search(r'\b(?:todo|unimplemented)!', text):
                errors.append(f"class{i}: unfinished prerequisite {module}")
            if j < i and not (package / 'tests/prerequisites' / f'{module}.rs').is_file():
                errors.append(f"class{i}: missing prerequisite contract for {module}")
            if args.starter and j < i:
                expected = (ROOT / f'class{j}/tests/contract.rs').read_text().replace(
                    f'class{j}::', f'class{i}::')
                copied = package / 'tests/prerequisites' / f'{module}.rs'
                if copied.is_file() and copied.read_text() != expected:
                    errors.append(f'class{i}: prerequisite contract for {module} is stale')
        if i and not (package / 'tests/prerequisites.rs').is_file():
            errors.append(f"class{i}: missing prerequisite test target")
    tree = ast.parse((ROOT / "scripts/grade.py").read_text())
    groups = next(ast.literal_eval(node.value) for node in tree.body
                  if isinstance(node, ast.Assign)
                  and any(isinstance(t, ast.Name) and t.id == "GROUPS" for t in node.targets))
    for i, assigned in enumerate(MODULES):
        if args.starter:
            text = (ROOT / f'class{i}/src/{assigned}.rs').read_text()
            # C15.3 assesses cumulative backends; it is not a function to implement.
            tasks = {row[0] for row in groups.get(f'class{i}', [])} - {'C15.3'}
            holes = set(re.findall(r'todo!\("(C\d+\.\d+):', text))
            if holes != tasks:
                errors.append(f'class{i}: starter task holes {sorted(holes)} differ from assigned tasks {sorted(tasks)}')
        if sum(row[1] for row in groups.get(f"class{i}", [])) != 100:
            errors.append(f"class{i}: task weights must total 100")
    # Ignore explicitly localized diagram assets and generated/runtime directories.
    for folder in [*(ROOT / f'class{i}' for i in range(16)), ROOT / 'docs', ROOT / 'scripts']:
        for path in folder.rglob('*'):
            if not path.is_file() or path.suffix not in ('.rs', '.md', '.py', '.toml'):
                continue
            if any(part in ('target', '__pycache__') for part in path.parts):
                continue
            if re.search(r'[\u3400-\u4dbf\u4e00-\u9fff\uf900-\ufaff]', path.read_text()):
                errors.append(f"CJK text in {path.relative_to(ROOT)}")
    if args.starter:
        for j, module in enumerate(MODULES[:-1]):
            reference = (ROOT / f'class{j + 1}/src/{module}.rs').read_bytes()
            for i in range(j + 2, len(MODULES)):
                if (ROOT / f'class{i}/src/{module}.rs').read_bytes() != reference:
                    errors.append(f'class{i}: reference module {module} differs between starter snapshots')
    if errors:
        print('\n'.join(errors), file=sys.stderr)
        return 1
    print('PASS: 16 independent snapshots, completed prerequisites, and assessment boundaries')
    return 0


if __name__ == '__main__':
    sys.exit(main())
