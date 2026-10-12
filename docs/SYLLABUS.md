# Syllabus

Sixteen implementation projects build a native x86-64 Linux ELF linker in Rust.
The early milestones establish checked input models and an ET_EXEC output;
later stages add self-relocating PIE, rewritten metadata, TLS, code folding, and
a compiler-facing driver. The capstone investigates the combined implementations
with deterministic input surveys and independent behavioral checks.

The table is a navigation aid. Each class README supplies the exact API contract,
input subset, error rules, and acceptance criteria. Prerequisites are cumulative:
for example, completing class6 requires the parsing and binding stages already
used by class5. The [reading guide](LEARNING-PATH.md) links the relevant theory
before each new implementation task.

| Class | Student responsibility | Integration evidence | Prior classes |
|---|---|---|---|
| 0 | Checked bytes and supported ELF header | Reject malformed inputs without panic | None |
| 1 | Sections, section names, string bounds | Inspect compiler-produced objects | 0 |
| 2 | Symbols and RELA records | Resolve record indices to their owning sections | 1 |
| 3 | Global resolution, weak/common rules | Multiple inputs with precise diagnostics | 2 |
| 4 | Layout and minimal executable writer | Run a single-object syscall program | 1–3 |
| 5 | x86-64 relocation and cross-object linking | Call a C function from an assembly entry | 2–4 |
| 6 | Segment permissions, data/BSS, alignment | Mutable globals and zero-initialized storage | 4–5 |
| 7 | Archive parsing and lazy extraction | Pull only required library members | 3–6 |
| 8 | Reachability GC and map diagnostics | Drop unreachable sections without broken references | 5–7 |
| 9 | Static PIE and startup relocation | Execute at randomized load addresses | 5–8 |
| 10 | Mergeable data and unwind metadata | Correct merged references and unwind structure | 5–8 |
| 11 | GOT and static TLS | Data access and thread-local behavior | 5–10 |
| 12 | Debug sections and source mapping | ET_EXEC/PIE coordinates, TLS symbols, merged references and preserved unwind headers | 8, 10–11 |
| 13 | ICF and order control | ET_EXEC/PIE folding with GOT/TLS, compatible CIE/FDEs, distinct addresses and debug tombstones | 8, 10–12 |
| 14 | Rust/no_std driver integration | Rust-generated object linked by student pipeline | 7–13 |
| 15 | Capstone robustness and differential validation | Reproducible native Linux regression suite | 0–14 |

## Scope and extensions

The core implementation uses native x86-64 Linux. It emits ELF files for the
kernel to load; writing a userspace loader is not part of these assignments.
System assemblers, compilers, and linkers generate fixtures or provide comparison
results. Student implementation code does not delegate its work to those tools.

ET_EXEC and static PIE share merge/unwind, debug, and folding stages. Runtime
unwind preservation remains an explicit policy; the supported Rust driver uses
abort-only semantics. It preserves ordinary debug metadata while omitting runtime
unwind records. A working `no_std` fixture does not establish general Rust `std`,
C++ exception, or dynamic-loader support.

Linker scripts, kernel/ROM startup, ABI evolution, and Mach-O/PE comparisons are
additional theory topics. They introduce no hidden implementation tasks in the
core track. A different architecture, executable format, or runtime protocol
requires a separate input contract and validation on its intended platform.

## Acceptance

Use each class's contract tests for individual APIs and its native fixtures for
integration behavior. Keep earlier classes passing as later consumers exercise
them with larger inputs. [Assessment](ASSESSMENT.md) defines edit boundaries and
scoring; the per-class handouts explain what the checks observe and what their
results establish.

Validation records in `docs/validation/` identify tested versions, fixtures, and
outcomes. Their scope matters: a reference test pass is evidence for its tested
contract, rather than proof for every possible input or runtime feature. The
student implementation must satisfy the handout and the tests together.
