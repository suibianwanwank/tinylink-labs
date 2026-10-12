# Validation evidence

Current release: [independent snapshot validation](2026-10-12-release.md).
The records below describe earlier release checks.

## 2026-10-08: class0 and class1

Target: native x86-64 Linux, rustc 1.98.1. The reference was materialized in a
separate temporary directory; student sources still contain their implementation
holes. No solution is imported by student packages.

- Unchanged starter: workspace compiles; eight class0 contract tests and six
  class1 contract tests fail at unimplemented tasks as expected.
- Reference: eight class0 tests, six class1 contract tests, and one native assembler
  integration test pass in both debug and release.
- Local grader: 100/100 and cumulative completion for each reference package.
- Mutations: rejecting a valid EOF boundary and reading NOBITS as stored bytes are
  detected by the contract suite. These are representative checks, not exhaustive
  mutation coverage.
- Repository language scan: no CJK text in current tracked/unignored text files.
- Formatting: `cargo fmt --all --check` passes.

The log records an instructor audit, not evidence that later classes are ready.
The private reference harness is retained separately from the student repository.
Git history before this reconstruction includes older Chinese documentation;
current working-tree content is English. History has not been rewritten.

## Remaining course work

The optional platform projects and the matching laboratory article rewrite
remain outstanding. The syllabus
preserves their scope. No completion claim applies to these undelivered stages.

## 2026-10-08: class2

The class2 reference passes eight contract tests and a native assembler integration
test in debug and release. All three stage graders report 100/100 with cumulative
completion on the reference. The student skeleton still contains both task holes.

The native test verifies the relocation-owned symbol table, target offset, PLT32
kind, signed addend, and undefined symbol name produced by GNU as. Mutations that
swap the binding nibble or read the relocation kind as its symbol index are
rejected. Class0/class1 regression and their two earlier mutation checks also pass.
The class2 starter compiles on native Linux and fails all eight contract tests as
expected. See class2-linux.log for reference evidence.

## 2026-10-08: class3

The stage now provides two student tasks, six contract tests, and a two-object
native integration test. A caller and provider are independently assembled,
parsed using classes1–2, and resolved through the student's class3 interface.
The native integration checks the selected definition identity, not an assigned
address or a final executable; those are subsequent stages.

The independent reference passes all class0–class3 tests in debug and release.
All four local graders return 100/100 with cumulative completion. Two new
mutations (last weak wins and COMMON size minimum) are rejected, alongside the
four earlier mutations. The starter compiles on native Linux and all six class3
contract tests fail at task holes, as intended. See class3-linux.log.

Class8–class15 and the article reconstruction remain unfinished. These early
packages do not yet emit an executable and must not be described as a finished
linker or finished course.

## 2026-10-08: class4

The stage now supplies layout, serialization, and single-input linking tasks,
four structural contract tests, and five native integration tests. Student code
must produce ET_EXEC bytes itself; the provided CLI only reads/writes files.

The private reference passes cumulative debug/release tests and all five local
graders report complete. Linux executes generated syscall programs returning
0, 42, 93, and the CLI output returning 17. GNU readelf accepts the outputs.
Tests additionally check custom code-section/entry names, a missing symbol table,
nonzero entry offsets, and rejection of relocations or data/BSS not supported by
this stage. Processes have a three-second timeout.

Entry-offset omission and accidental RWX segment permissions are rejected by new
mutations; the six earlier mutation checks remain passing. The starter compiles
on native Linux, and four class4 contract tests fail at the expected task holes.
See class4-linux.log. Current text files pass the language audit and Rust sources
pass formatting checks.

The executable milestone is intentionally narrow: it does not yet link multiple
objects or patch references. Those abilities and the remaining course modules
are still required before the course can be considered complete.

## 2026-10-08: class5

Three implementation tasks now cover aligned contribution merging, atomic x86-64
relocation, and cumulative multi-object linking. Six contract tests and five
native tests verify signed ranges, full-width modular arithmetic, patch bounds,
local contribution identity, and executable behavior.

On native Linux, independently assembled startup code calls a C function compiled
with the documented code-only flags. Both file orders execute and return 42;
the tests inspect the displacement sign to distinguish forward/backward calls.
Absolute calls, unresolved weak symbols, immutable inputs, CLI linking, forbidden
data/COMMON, and prevention of writes into the next contribution are checked.

All class0–class5 reference tests pass in debug and release; all six local graders
report 100/100 with cumulative completion. Ignoring addends and allowing writes
past a target contribution are rejected mutations, bringing the maintained audit
to ten representative faults. The starter compiles on Linux and all six class5
contract tests fail at their task holes. See class5-linux.log.

A Linux compiler default initially introduced an allocated GNU property note;
fixtures now explicitly disable control-flow protection and document that choice.
A separate infrastructure failure exhausted /tmp with previous validation builds;
only disposable course build outputs were removed, and validation now cleans its
own build directory on exit. The final recorded validation passed after these fixes.

The implementation still supports code-only static inputs. Writable data, BSS,
archives, later topics, and the rewritten laboratory articles remain required.

## 2026-10-08: class6

The course now includes protected storage planning, three-load-segment ELF
serialization, and cumulative relocation across code/data/BSS/COMMON. Five
contract tests inspect coordinates, alignment, resource limits, file-versus-memory
sizes, entry bounds, and exact header flags. Seven native tests check C globals,
zero-fill across pages, shared COMMON, COMMON overridden by strong definitions,
Data pointers to BSS, CLI output, and TLS rejection.

Native permission probes separately establish that Text and ReadOnly cannot be
written and Data cannot be executed (SIGSEGV, with core dumps disabled). Correct
programs return 42; readelf accepts their program headers. Debug and release
reference tests and all seven stage graders pass. Two additional mutations
(p_memsz incorrectly equal to p_filesz and writable ReadOnly) are rejected,
bringing the maintained representative fault audit to twelve cases.

The starter compiles on Linux and its five class6 contract tests fail at the
implementation holes as intended. See class6-linux.log. The reference remains
separate from student code; no answer flag or fallback linker is introduced.

Archives and the remaining classes, optional projects, and the matching article
reconstruction remain outstanding. Completion of storage support does not certify
the entire course.

## 2026-10-08: class7

Three student tasks cover GNU/System V archive framing, deterministic member
selection, and integration with class6. Four contract tests exercise sizes,
padding, long-name offsets, metadata handling, and malformed names. Six native
tests use GNU as/ar and check extraction order, a dependency requiring another
archive-local scan, an unused member with duplicate entry/TLS, earlier versus
repeated archives, weak references, competing providers, malformed unused ELF
metadata, and CLI execution.

The independent reference passes cumulative debug/release tests. All eight stage
graders report 100/100 with cumulative completion. Removing the fixed-point rescan
and treating undefined weak symbols as extraction requests are detected mutations;
the twelve prior representative mutations are still rejected. Starter compilation
passes on Linux, with four expected class7 contract failures at task holes.
See class7-linux.log.

The archive policy is intentionally documented as a subset: no thin/BSD archives,
backward archive references, group rescanning, or COMMON-to-strong search upgrades.
The symbol-index payload is not trusted or consumed; ELF member symbols determine
selection. These limits are visible in the handout rather than hidden in tests.
Remaining classes and the laboratory article rewrite are still required.

## 2026-10-08: class8

Three tasks cover iterative section reachability, a byte-safe deterministic map,
and cumulative linking with archive selection before GC. Four contract tests
check cycles, dead errors, root order, a 30,001-node chain, map ordering, and name
escaping. Nine native tests cover code/data/BSS dependencies, dead cycles, explicit
roots, weak and COMMON storage, selected strong definitions, archive extraction
before GC, relocation error boundaries, NONE, metadata, CLI maps, and unsupported
allocated sections. One supported fixture is also linked with GNU ld --gc-sections;
both generated programs return 42.

The independent reference passes all class0–class8 tests in debug and release.
All nine local graders report 100/100 with cumulative completion. Traversing dead
nodes and treating R_X86_64_NONE as a dependency are rejected mutations; the
fourteen earlier mutations remain detected. The unchanged starter compiles on
Linux, and its four class8 contract tests fail at the expected implementation
holes. See class8-linux.log for the successful final reference run.

Repeated cumulative runs exposed a transient Linux ETXTBSY spawn failure in the
provided fixture execution helpers, including class5. Direct-execution helpers
in classes4, 5, 7, and 8 now retry only that error for at most three seconds.
Other spawn errors, signals, exit mismatches, and execution timeouts remain
failures. This changes test infrastructure, not student requirements or assertions.
The final cumulative run passed after that correction. This audit does not claim
exhaustive fault coverage or certification of the remaining course stages.

## 2026-10-08: class9

Four tasks cover static versus load-bias-dependent fixups, canonical RELATIVE
record encoding, self-relocating ET_DYN emission, and cumulative lowering from
the class8 link plan. Seven contract tests check fixup categories, failure
atomicity, NONE, record ordering and overlap, descriptor fields, zero-based
program headers, and rejected relocation destinations. Eight native tests cover
pointers to code, data, and BSS, COMMON, unresolved WEAK, explicit ABS values,
negative and full-width addends, archive calls, the unchanged initial stack, a
compiler-generated PIE data pointer, CLI maps, and rejection of non-PIC and
read-only runtime patches. One fixture reports its runtime entry address over
several executions and fails if Linux ASLR does not move it.

Class8 now separates the front end from address assignment. Its C8.3 task is
`prepare`, which returns an immutable `Prepared` link plan with private fields;
the supplied ET_EXEC `link` consumes it. Class9 reuses that plan rather than
duplicating archive selection, root policy, or GC. The class8 observable
behavior and its tests are unchanged apart from added plan invariants.

The workspace forbids `unsafe` code in every package. Test platform guards are
compile-time assertions. Strict `cargo clippy --workspace --all-targets -- -D
warnings` now runs on the unchanged starter and on the materialized reference;
both pass. A reference-only lint was fixed in the private reference rather than
suppressed in student source.

The independent reference passes all class0–class9 tests in debug and release.
All ten local graders report 100/100 with cumulative completion. Dropping the
input addend from runtime records and writing a zero record count into the
startup descriptor are rejected mutations; the sixteen earlier mutations remain
detected. The unchanged starter compiles on Linux and its seven class9 contract
tests fail at the expected task holes. See class9-linux.log.

The output is a freestanding static PIE with course-specific startup; it has no
PT_DYNAMIC, PT_INTERP, RELRO, or dynamic-loader compatibility, and the handout
says so. Mergeable data, unwind metadata, and the remaining classes are still
required before the course can be described as complete.

## 2026-10-08: class10

Five tasks cover splitting `SHF_MERGE` sections, deterministic piece
deduplication with reference mapping, `.eh_frame` record framing, emission of
`.eh_frame` and a sorted `.eh_frame_hdr`, and cumulative ET_EXEC integration.
Eight contract tests check string and constant pieces, shape rejection, group
keys and first-use order, alignment padding, interior lookups, every truncation
of an unwind section, unsupported CIEs, CIE pointer validation, relocation
coverage, CIE sharing, PC-relative fields, table order, range and overlap
errors, and reservation checks.

Six native tests compile freestanding C with GCC. String and floating-point
constants have one address across translation units under `-fno-pie` and
`-fpie` code generation in both input orders. A self-checking program reads its
auxiliary vector, finds `PT_GNU_EH_FRAME`, binary-searches the table, and
verifies that its live functions are covered by FDEs with decodable CIEs; the
same object linked by GNU ld with `--eh-frame-hdr` also passes, and both outputs
have the same FDE ranges. Dead merge sections and FDEs of collected functions
disappear, inputs without the new features reproduce class8 byte for byte, and
relocated merge sections, wide strings, references past a piece, personality
routines, and FDEs describing data are rejected.

Class8 gained `prepare_with`, a `Classifier`, and `Role::{Storage, Split,
Annotation}`; the supplied `prepare` passes the unchanged class6 policy. C8.3 is
now `prepare_with`, and a new native test, graded with C8.3, checks the
split/annotation contract. `class2::Rela` now derives `Clone` and `Copy`.

The independent reference passes all class0–class10 tests in debug and release,
strict Clippy, and all eleven local graders at 100/100 with cumulative
completion. Disabling piece deduplication and leaving the search table unsorted
are rejected mutations; the eighteen earlier mutations remain detected. The
unchanged starter compiles on Linux and its eight class10 contract tests fail
at the task holes. See class10-linux.log.

Tail merging, piece-level GC within a merge section, C++ personality and LSDA
records, and `.eh_frame` in PIE output are outside this stage, as the handout
states. Classes 11–15 and the article rewrite remain outstanding.

## 2026-10-08: class11

Five tasks cover the TLS template and TP offsets, per-relocation lowering
(GOT, `mov`-to-`lea` relaxation, local-exec, initial-exec), GOT slot assignment
and encoding, the supplied TLS startup stage with `PT_TLS`, and cumulative
integration over class9's static PIE. Eight contract tests check template
order, alignment and thread-pointer rounding, section validation, the complete
lowering table including relaxation edge cases, GOT deduplication and
RELATIVE records, the startup descriptor, the fifth program header, and
reservation and entry checks.

Six native tests compile freestanding C. Initialized, zero-filled, static, and
initial-exec variables hold their initial values in both input orders, a
pointer stored in `.tdata` is relocated, and `&counter` lies below `%fs:0`. A
second thread created with `clone(CLONE_SETTLS)` receives a block built only
from `PT_TLS` and sees initial values while the main thread's copy differs. GOT
access under `-fPIC` uses two slots with relaxation and four without, including
an ABS symbol and an unresolved WEAK reference, across ASLR runs. Inputs without
the new features reproduce class9 byte for byte; general-dynamic TLS and
mismatched references are rejected.

Class8's `Special` now records the section type and size, which a NOBITS
section needs, and `SectionId` derives `Hash`. The class6 native helper runs
fixtures through `sh` to disable core dumps; it now retries the shell's exit
status 126 for up to three seconds, because the ETXTBSY race fixed earlier in
other helpers also appears there as a failed `exec`. One cumulative run hit that
race before the fix. Assertions and student requirements are unchanged.

The independent reference passes all class0–class11 tests in debug and release,
strict Clippy, and all twelve graders at 100/100 with cumulative completion.
Disabling relaxation and omitting thread-pointer rounding are rejected
mutations; the twenty earlier mutations remain detected. The unchanged starter
compiles on Linux and its eight class11 contract tests fail at the task holes.
See class11-linux.log.

Dynamic TLS models, TLS descriptors, IE-to-LE instruction relaxation, RELRO,
and relaxation of non-`mov` GOT forms are outside this stage, as the handout
states. Classes 12–15 and the article rewrite remain outstanding.

## 2026-10-08: class12

Five tasks cover gathering `.debug_*` inputs, relocating one debug field
(addresses, section-relative offsets, tombstones), encoding `.symtab` and
`.strtab`, appending sections and a section header table, and cumulative
integration over class8's image. Eight contract tests check grouping order,
alignment padding, flag and compression rejection, every relocation target
form and tombstone value, overflow and bounds atomicity, locals-first symbol
order with shared names, exact section-header layout, and malformed inputs.

Six native tests compile freestanding C with `-g -Og`. A scripted GDB session
(breakpoint by name, argument and global values, line lookup, backtrace,
`finish`, exit status) on the class12 output matches the same session on GNU
ld's output after removing addresses and process IDs. `addr2line` results for
functions match GNU ld's, `readelf --debug-dump` decodes all sections without
warnings, a collected function's DIE has `DW_AT_low_pc` 0 while kept ones do
not, and the loadable image is class8's byte for byte apart from the three
section-header fields. Compressed debug sections, PC-relative debug
relocations, and references into `.comment` are rejected.

Class8 now exposes non-allocated PROGBITS sections and their relocations
through `Prepared::metadata`, records section names in `Special`, and exposes
its supplied ET_EXEC lowering as `lower_exec` and `placed_rows`; `link` uses
them, with unchanged behavior and tests.

The independent reference passes all class0–class12 tests in debug and release,
strict Clippy, and all thirteen graders at 100/100 with cumulative completion.
Ignoring addends of section-relative debug references and writing a nonzero
value instead of the tombstone are rejected mutations; the twenty-two earlier
mutations remain detected. The unchanged starter compiles on Linux and its
eight class12 contract tests fail at the task holes. See class12-linux.log.

String merging for `.debug_str`, DWARF 64, split DWARF, `.eh_frame` together
with debug output, and debug information for TLS variables are outside this
stage. Classes 13–15 and the article rewrite remain outstanding.

## 2026-10-08: class13

Three tasks cover partition refinement to the largest safe folding, lowering a
folded and ordered plan, and integration with address significance and
`class12::finish`. Five contract tests check representatives, alignment and
significance, every reference field, propagation through calls, a chain that
needs repeated refinement, mutual and self recursion with and without a pinned
member, significant targets, and independence from input order.

Six native tests compile freestanding C with `-g -Og`. Three identical
functions across two objects fold to one address and the program still
returns 42; address-taken functions keep distinct addresses while identical
called-only ones fold, and `keep` prevents folding. GDB stops in a folded
function, and only the representative's DIE keeps a real `DW_AT_low_pc`.
Ordered functions come first, a folded name places its representative, bad
order names are rejected, and without folds the output equals class12's.

`class12::link` now calls a new C12.5 entry point, `finish(prepared, lowered,
aliases)`, so class13 reuses class12's debug, symbol, and section-header
integration. With no aliases its behavior and tests are unchanged. The
syllabus lists class13's prerequisites as classes 8 and 12: folding is defined
over the ET_EXEC path with debug output, not over class10's merged and unwind
sections. `class3/README.md` now attributes output positions to classes 5
and 6.

The independent reference passes all class0–class13 tests in debug and release,
strict Clippy, and all fourteen graders at 100/100 with cumulative completion.
Ignoring address significance and stopping refinement after one pass are
rejected mutations; the twenty-four earlier mutations remain detected. The
unchanged starter compiles on Linux, the lockfile is current, and its five
class13 contract tests fail at the task hole. See class13-linux.log.

## 2026-10-08: class14

Four tasks cover GNU-ld-style option parsing, library search, response-file
expansion with GNU quoting, and running a link from a command line through
`class11::link_with` with a supplied LLVM section policy. Five contract tests
check the exact command line `rustc -C linker-flavor=ld` produces, every
accepted option form, rejected options and arguments, search order and
verbatim names, quoting, escapes, empty and unterminated arguments, unreadable
files, and the nesting limit.

Three native tests use stable rustc 1.98.1. A `#![no_std]` program that
formats `sum=42 [2, 3] 1.50` through `core::fmt` is compiled with
`-C linker=<class14> -C linker-flavor=ld`; the result prints that line and
exits 42 in each of several ASLR runs, is ET_DYN, and has no interpreter or
dynamic section. Linking pulls in `core` and `compiler_builtins` objects with
COMDAT groups, mergeable constant pools, GOT references, and `.eh_frame`.
Assembly links with a `-l` library from a `-L` directory, directly and through
a response file; missing libraries, `-shared`, and unresolved references fail.

Class11's C11.5 entry point is now `link_with(inputs, entry, keep, classify)`;
the supplied `link` passes `policy`, so class11 behavior and tests are
unchanged. During design, rustc's `.rlib` metadata members turned out to be
ELF objects, so no archive rewriting is needed and the planned task for it was
replaced with response files.

The validation host's `/tmp` is a 3.7 GB tmpfs. Two runs of this cumulative
validation failed with `No space left on device` or dropped connections; the
instructor harness now builds under the home directory and removes each build
directory on exit. The final runs passed. The independent reference passes all
class0–class14 tests in debug and release, strict Clippy, and all fifteen
graders at 100/100 with cumulative completion. Disabling backslash escapes and
searching directories in reverse are rejected mutations; the twenty-six
earlier mutations remain detected. The unchanged starter compiles, the
lockfile is current, and its five class14 contract tests fail at the task
holes. See class14-linux.log.

## 2026-10-09: class15

Two tasks cover a deterministic xorshift64* generator with a fixed mutation
menu, and a survey harness that runs mutated links under `catch_unwind` and
records every panic with enough context to replay it. Three contract tests
check generator output against known values, the exact mutation sequence for
a fixed seed, bounds and determinism, outcome counting, recorded panic
context, replay of a recorded case, non-string payloads, and empty corpora.

C15.3 grades the whole linker. About 4,000 mutated links over samples with
merged strings and unwind tables, TLS with a GOT-using archive member, and
debug information with foldable functions run through class10, class11,
class13, and the class14 policy without a panic, and mutated rustc output
with the real `core` and `compiler_builtins` rlibs runs through the class14
policy without a panic. Repeated links and links of copied inputs are
byte-identical and run. Three C programs give the same exit status and output
under class13 and GNU ld.

The reference passed the robustness tests without changes to earlier
classes. To show that the tests detect what they claim, one rejected
mutation removes a single bounds check from class1's string lookup; the
mutated linker panics during the class15 survey. The grader's per-command
timeout is now 900 seconds because the capstone's debug-build tests take
minutes. All sixteen graders report 100/100 with cumulative completion, and
thirty-one representative mutations are rejected. The unchanged starter
compiles and its three class15 contract tests fail at the task holes. See
class15-linux.log.

## 2026-10-10: initial contract coverage audit (in progress)

Added four class0 contract tests and registered them in the local grader:
all pairs of header validation failures, validation before table range checks,
wide table-size arithmetic, logical index bounds with available trailing bytes,
and a table at file offset 256. The handout now explains diagnostic priority
as a course policy and traces the 448-byte layout from file to header to payload.

An isolated instructor reference on prism01 (native Linux x86-64, Rust 1.93.1)
passes 12 class0 contract tests, six class1 contract tests, and one native
assembler integration test in debug and release. See
`2026-10-10-early-contracts.log`. Student implementations were not replaced.
The first validation invocation lacked workspace lint configuration; after
restoring it, both profiles passed.

Still required: mutation checks for new tests; class1 and later-class coverage
audit; diagrams integrated in Chinese and English teaching articles; behavior
contracts aligned with those tests; publication and online verification. This
entry does not claim course-wide coverage or publication.

## 2026-10-10: staged validation and teaching follow-up

Added four class1 tests for header/name-table/payload/name precedence, name-table
boundaries, original table order, and preservation of full-width opaque metadata.
Added class4 layout-priority and class5 relocation-priority/atomicity tests. All
new tests are registered in their existing assessment groups.

Class0–class5 reference tests, including native integration tests, pass in debug
and release on prism01, Rust 1.93.1. See `2026-10-10-class0-5.log`. These are
reference results, not a claim that the user's in-progress implementations pass.

Six deliberate reference mutations were detected: index > instead of >=;
wrapping u16 count multiplication; fixed table offset; missing final string-table
NUL; name validation before payload validation; type before header version.
The narrow-count mutation initially survived the large-table success test, so a
truncated-large-table assertion was added; that mutation now fails. Reproducible
mutation driver: instructor workspace `audit_early_mutations.py` (operates only
on an isolated materialized reference, restoring each source after execution).

Bilingual SVG figures now show the whole 448-byte example, all table entries,
payload coordinates, and all ten ELF64 section-header fields in 16-byte rows.
They are embedded in repository handouts and Chinese/English drafts of theory 02
and labs 00/01. The English rendering was visually checked; raster thumbnails are
only inspection artifacts. Labs 04/05 also explain their newly tested behavior.
Publication and browser verification remain pending; draft relative figure links
must be mapped to served assets before publication.

## 2026-10-10: full-course contract and native regression

Additional tests now cover classes2/3 symbol validation and scan-order conflicts;
class6 metadata before packing; class7 framing before names; class8 all roots
before traversal; class9 multi-error rejection without inventing unspecified
priority; class11 TLS metadata before sizing; class12 every truncated patch
width; class14 non-UTF-8 path preservation; and class15 zero-iteration behavior.
All additions are registered in the local grader and explained in corresponding
English handouts and Chinese/English article drafts.

All 16 contract suites passed in debug and release on native x86-64 Linux,
Rust 1.93.1 (`2026-10-10-all-contracts-final.log`). All 15 native suites also
passed in both profiles (`2026-10-10-all-native.log`), including the real rustc
link and capstone mutation/differential tests. The SSH command returned zero.

Three further targeted mutations were rejected: class2 link lookup before entry
size, class6 EmptyText before metadata, and class8 root checks deferred to DFS.
The instructor driver `audit_order_mutations.py` restores sources after each run.
Both Chinese and English SVG renderings have now been visually inspected.
Existing class10 tests already cover framing before relocation errors and exact
record-offset diagnostics; class13 tests already cover recursive partition
refinement, significance, and input-order independence, so no redundant new
contract tests were added for those modules.

Remaining: review all full-width and reserved-numbering rules against the ABI;
finish publication preparation, publish selected repository changes and article
bodies/assets, then verify both online languages and private access. Current
publication script rebuilds theory text from historical backups, so must not be
used blindly for this update. The media endpoint accepts PNG/JPEG/GIF/WebP, not
SVG; retain vectors in the repository and upload verified PNG versions per post.
