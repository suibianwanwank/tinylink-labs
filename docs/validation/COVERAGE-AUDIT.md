# Coverage and teaching audit — ongoing

The goal is observable contracts backed by tests and teaching, not a numerical
coverage target. Preserve student implementations in class0/class1. Run reference
validation in separate Linux directories. Never publish the instructor solution.

| Classes | Current evidence | Remaining work |
|---|---|---|
| 0 | Pairwise errors, wide arithmetic, table/index boundaries; six shared mutations | Check every full-width field and course-policy/ELF distinctions |
| 1 | Staged errors, name boundaries, opaque metadata | Further valid-boundary cases; Chinese figure render verified |
| 2–3 | Added ownership/record priority, validation-before-selection and conflict-order tests | Complete final cross-check |
| 4–5 | Added priority/atomicity cases, Linux regression | Mutation checks for these new cases |
| 6–15 | Full contract/native debug and release regression; priority/boundary additions where missing | Final review and publication |

Completed draft work: bilingual region/section-header diagrams in theory02 and
labs00/01; diagnostic-policy explanations in labs00/01/04/05. Diagrams are vector
assets under docs/figures and blog-drafts/course-labs/figures. No online publication
of this audit yet. Ensure online asset URLs, both article languages, private
visibility, and repository publication are verified before claiming completion.

Linux materialized reference from the latest run is recorded on the first line
of 2026-10-10-class0-5.log. Full-course materialized reference: /home/linuxuser/tinylink-audit-all.Kw97nX.
Early mutation workspace is
/home/linuxuser/tinylink-audit.PYkBeh. These are disposable audit directories,
not the user's /home/linuxuser/projects/tinylink-labs project.

## Final local audit status for this pass

`cargo fmt --all --check`, Python syntax checks, and `git diff --check` pass on
the modified workspace. The student tree still contains intentional incomplete
implementations and platform-gated native tests; its test run is therefore not a
replacement for the isolated instructor Linux evidence. New contract tests are
expected to fail against those TODOs until each class is implemented.

The teaching assets are repository-local SVGs. They explicitly distinguish the
file byte ranges, the section-header table entries, each entry's 64-byte field
layout, and the payload ranges selected by `sh_offset`/`sh_size`; prose explains
`sh_name`/`e_shstrndx`, NOBITS, ordering, error precedence, atomicity, and
unspecified priority.

## Editorial pass: method-first laboratory design

Every class README now includes a method map. Each public function is explained
by purpose, input/output coordinates, invariant handed to the next class, special
values, and failure behavior. The explanations distinguish format requirements
from course policy. In particular, class0/class1 explain the separate meanings of
64-byte section-header entries, 8-byte ELF64 fields, table size, and payload
coordinates; class12 explains 64-bit relocation names versus 8-byte field widths.

The sparse late lab drafts (12–15) were expanded with implementation walkthroughs
for actual public APIs. The current bilingual drafts were synchronized into
`blog-drafts/course-labs/manifests/rebuild-wild-linker-learning.json`, so a later
publish run cannot silently restore the old short versions.

The figure assets remain as SVG source in the repository.
