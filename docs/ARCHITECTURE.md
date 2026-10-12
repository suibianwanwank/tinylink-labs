# Independent course snapshots

Each class owns a complete source tree. Modules from preceding classes are
provided as reference implementations; only the current module is assigned.
No class imports another class or a shared solution crate. The root Cargo
workspace groups projects for convenience and is not required to build them.

| Class | Assigned module | Source |
|---|---|---|
| class0 | `elf_header` | [src/elf_header.rs](../class0/src/elf_header.rs) |
| class1 | `sections` | [src/sections.rs](../class1/src/sections.rs) |
| class2 | `symbols` | [src/symbols.rs](../class2/src/symbols.rs) |
| class3 | `resolve` | [src/resolve.rs](../class3/src/resolve.rs) |
| class4 | `executable` | [src/executable.rs](../class4/src/executable.rs) |
| class5 | `relocate` | [src/relocate.rs](../class5/src/relocate.rs) |
| class6 | `layout` | [src/layout.rs](../class6/src/layout.rs) |
| class7 | `archive` | [src/archive.rs](../class7/src/archive.rs) |
| class8 | `gc` | [src/gc.rs](../class8/src/gc.rs) |
| class9 | `pie` | [src/pie.rs](../class9/src/pie.rs) |
| class10 | `unwind` | [src/unwind.rs](../class10/src/unwind.rs) |
| class11 | `tls` | [src/tls.rs](../class11/src/tls.rs) |
| class12 | `debug` | [src/debug.rs](../class12/src/debug.rs) |
| class13 | `icf` | [src/icf.rs](../class13/src/icf.rs) |
| class14 | `driver` | [src/driver.rs](../class14/src/driver.rs) |
| class15 | `fuzz` | [src/fuzz.rs](../class15/src/fuzz.rs) |

Within a snapshot, use functional module names such as
`crate::elf_header::checked_slice` and `crate::sections::Section`.
`src/lib.rs` declares the modules and re-exports the assigned module. Tests and
commands operate entirely on this snapshot.

`tests/contract.rs` and `tests/native.rs` check the assignment. From class1,
`tests/prerequisites.rs` runs the contracts for all completed earlier modules.
Use `cargo test --test prerequisites` to check the foundation separately.
The grader checks only the selected snapshot in debug and release; unfinished
assignments in neighboring directories do not affect its result.

To continue with your own work, replace the corresponding earlier module inside
the next snapshot and rerun its prerequisite tests. No automatic copy operation
overwrites a solution. Reference implementations become visible in the following
chapter, so this course is intended for self-study, not secret-answer assessment.

From class9, the local build script assembles the supplied startup code. These
snapshots require native x86-64 Linux. Earlier executable integration tests also
require Linux. No cross compiler or platform-selection feature is needed.
