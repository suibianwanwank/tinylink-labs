#!/usr/bin/env python3
"""Reject CJK text in tracked course sources and documentation."""
from pathlib import Path
import re
import subprocess
import sys

root = Path(__file__).resolve().parents[1]
paths = subprocess.check_output(
    ["git", "ls-files", "--cached", "--others", "--exclude-standard", "-z"],
    cwd=root,
).decode().split("\0")
bad = []
for name in filter(None, paths):
    # Historical theory examples are multilingual; course handouts are English.
    if Path(name).parts[0] == "examples":
        continue
    path = root / name
    if not path.is_file():
        continue
    try:
        content = path.read_text(encoding="utf-8")
    except UnicodeDecodeError:
        continue
    for line, text in enumerate(content.splitlines(), 1):
        if re.search(r"[\u3400-\u9fff\uf900-\ufaff\U00020000-\U000323af]", text):
            bad.append(f"{name}:{line}: non-English CJK text")
print("\n".join(bad) if bad else "PASS: no CJK text in course sources and handouts")
sys.exit(bool(bad))
