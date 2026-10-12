# Tinylink Labs

Build a linker in Rust through a sequence of implementation projects. The target
is native x86-64 Linux. Students implement the linking algorithms; provided
support code handles fixtures and test execution. The finished linker must emit
and run real ELF programs without delegating linking to another linker.

This is an independent course, not a CMU offering or an endorsed adaptation.
Its project structure is informed by publicly documented systems-course practices:
explicit contracts, bounded implementation tasks, automated correctness checks,
and cumulative integration projects. See [design notes](docs/DESIGN.md).

## Prerequisites

Rust 1.93 or newer; ownership, slices, enums, and error handling; integer representation and
machine addresses; basic familiarity with object files and command-line tools.
Rust language instruction and shell tutorials are outside the assignments.

For the chapter-to-task prerequisites and the shared backend path, use the
[reading and implementation guide](docs/LEARNING-PATH.md). Theory and laboratory
numbers are independent; read the mechanism linked for the current task.

## Working model

Each `classN` is a complete project containing the completed earlier modules,
one new assignment, and its tests. There are no dependencies on other classes.
Open a class directory and implement the source linked in its README.

```sh
cd class2
cargo test --test prerequisites  # Completed earlier modules.
cargo test                      # This assignment plus prerequisite regression.
```

The root workspace is a convenience; a class directory also builds when copied
outside the repository. Commands such as `cargo test -p class2` still work from
the root. All executable integration checks run on native x86-64 Linux.

Earlier modules contain reference implementations. Each chapter supplies only
its own new task as unfinished work. You may replace an earlier module with your
own solution, preserving the public interfaces and rerunning prerequisite tests.
Changes in one class never silently change another class's snapshot.

See [snapshot organization](docs/ARCHITECTURE.md) for the module map. The supplied
`cc`, `as`, `readelf`, `objdump`, and `ld` invocations create fixtures or reference
results; student code must not delegate its linking work to another linker.

## Course map

[The syllabus](docs/SYLLABUS.md) defines the implementation milestones and
prerequisites. Each class README specifies its APIs, supported input subset,
error behavior, and acceptance criteria. The subject of a theory chapter can be
broader than the feature implemented by a class; use the class contract to decide
what your implementation must accept or reject.

| Package | Implementation milestone |
|---|---|
| class0 | Checked ELF input boundary |
| class1 | Sections, contents, and string tables |
| class2 | Symbols and RELA records |
| class3 | Global symbol selection and reference binding |
| class4 | Minimal native ELF executable |
| class5 | Multi-object code and x86-64 relocations |
| class6 | Protected data, BSS, and COMMON storage |
| class7 | Archives and lazy member extraction |
| class8 | Reachability GC and deterministic link maps |
| class9 | Freestanding static PIE and startup relocation |
| class10 | Merged constants and rebuilt unwind records |
| class11 | PIE with merge/unwind, GOT, relaxation, and static TLS |
| class12 | Debug information, symbols, and section headers for ET_EXEC/PIE |
| class13 | Safe code folding and ordering before shared lowering |
| class14 | A rustc-facing driver for the cumulative abort-only static PIE |
| class15 | Robustness surveys, determinism, and differential validation |

The stages preserve input identities and accumulate completed earlier modules. Class10
introduces merge/unwind rewriting in ET_EXEC; class11 incorporates it into PIE.
Class12 attaches debug metadata to the completed layout, and class13 selects
folds and order before that layout is produced. Class14 uses the same cumulative
pipeline with an explicit abort-only runtime policy. See the
[composition guide](docs/LEARNING-PATH.md#how-the-implementation-accumulates) for
these boundaries; choosing abort-only omits runtime unwind metadata, not ordinary
debug information.

See [assessment](docs/ASSESSMENT.md) for submission boundaries and release gates.
Class handouts, student API comments, and assessment diagnostics use English.
The companion theory articles are available in English and Chinese.
