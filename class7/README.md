# Class 7: Archives and Lazy Extraction

Implementation: [`src/archive.rs`](src/archive.rs).


A static library is a container of object files, not one larger object. Link only
the members required by unresolved global names. A member pulled for one name
may introduce another requirement, so extraction is a stateful operation rather
than a one-pass filename filter.

## Implementation surface

Implement C7.1–C7.3 in [`src/archive.rs`](src/archive.rs). Reuse earlier ELF parsing, symbol selection,
and class6 linking. The supplied CLI distinguishes objects/archives by magic and
supports `class7 -o output start.o library.a`. No library search path, `-l`, thin
archive resolution, group rescanning, or whole-archive option is provided here.
Keep interfaces, dependencies, and supplied tests unchanged. Private helpers and
additional tests are allowed. Do not call ar, nm, or ld from implementation code.

## C7.1: Archive framing (35 points)

Accept GNU/System V archives beginning with the eight bytes `!<arch>\n`. A thin
archive magic `!<thin>\n` returns ThinArchive. Other magic returns BadMagic. An
empty ordinary archive is valid.

Every member consists of a 60-byte header, payload, and an optional newline pad:

| Offset within header | Width | Meaning |
|---|---|---|
| 0 | 16 | Name, right-padded with ASCII spaces |
| 16 | 12 | Timestamp, ignored metadata |
| 28 / 34 | 6 each | UID/GID, ignored metadata |
| 40 | 8 | Mode, ignored metadata |
| 48 | 10 | Payload byte count, unsigned decimal, right-padded spaces |
| 58 | 2 | Exact terminator: backtick then newline |

The size field must contain at least one digit, with only digits before trailing
spaces. Reject signs and leading whitespace. Check arithmetic and file bounds
before slicing. An odd-sized payload must be followed by a newline byte, which
is not part of the payload. Reject trailing incomplete headers.

Scan framing before resolving names. Interpret trimmed header names as follows:

- `short.o/`: remove the final slash and require a nonempty name.
- `//`: one GNU long-name table; a second one is DuplicateNameTable.
- `/decimal`: an offset into that table, which may occur later in the archive.
  The offset must start an entry: zero, or immediately after a `/\n` terminator.
  Its nonempty name ends at the next `/\n`. A suffix offset is not accepted.
- `/` and `/SYM64/`: symbol-index metadata, omitted from the result.
- `#1/...`: unsupported BSD extended names, returned as UnsupportedNameFormat.

Return borrowed names/payloads, original member-header offsets, and archive
order. Duplicate ordinary member names are allowed; identity is the member
position, not its spelling. Names need not be UTF-8. Symbol-index contents are
opaque because this assignment discovers exports from ELF member symbol tables.
Do not trust index offsets or pretend to validate a format you never consume.

## C7.2: Extraction fixed point (45 points)

`Input` explicitly distinguishes objects and archives. Return borrowed object
payloads in extraction order; do not copy them or flatten whole archives.

Append each explicit object immediately, parse its one SYMTAB, and validate the
selected symbol set using class3. At an archive:

1. Parse its ordinary members and every member's ELF symbol table. Malformed ELF
   in an unused member still fails this course's eager metadata scan. Storage
   policy (including unsupported allocated sections) is deferred until selection.
2. Scan unselected members in archive order. Compute required names from selected
   objects' GLOBAL/UNDEF symbols minus all names already defined in the current
   class3 map. This uses symbol-table requirements, not relocation reachability.
3. Extract a member if it has a GLOBAL or WEAK definition of a required name.
   COMMON is a definition too. Recompute requirements after each extraction,
   before considering the next member. Validate the new selected symbol set.
4. Rescan this archive while at least one member was extracted. Never extract
   the same member twice in one archive occurrence. Then continue with the next
   command-line input. Do not automatically revisit earlier archives.

An unresolved WEAK symbol does not request extraction. A weak definition already
selected can satisfy a GLOBAL reference; a later archive definition is not pulled
merely to make it stronger. Explicit objects still enter unconditionally and may
replace weak definitions through class3 resolution.

This assignment deliberately does not implement COMMON-to-strong upgrades during
archive search: an existing COMMON is already a definition. It also does not
implement LLD-style backward archive references or GNU group rescanning. The
contract is a deterministic introductory archive policy, not a claim of complete
GNU ld compatibility. Repeating an archive as another input processes it again.

## C7.3: Link selected members (20 points)

Call `select`, then class7::layout::link. Entry lookup happens afterward and does not force
archive extraction by itself. Missing definitions used by selected relocations
are therefore diagnosed by the existing linker. Nonselected members cannot add
storage, cause duplicate global definitions, or introduce unsupported TLS into
the final link. Member payloads remain immutable.

Task score groups are cumulative: native selection checks also confirm the
selected object set actually links. Selection output is asserted separately in
those tests to make extraction mistakes distinguishable from relocation errors.

## Completion

```sh
cargo test -p class7
python3 scripts/grade.py class7
cargo run -p class7 -- -o program start.o library.a
```

The native fixtures use GNU as/ar. A dependency is placed earlier than its caller
inside the archive, forcing a second scan. An unused member contains a duplicate
entry and unsupported TLS storage; correct selection keeps it out. Other tests
cover long names, repeated archives, weak references, competing providers, CLI
execution, truncation, and padding. Cumulative grading runs earlier classes in
both debug and release.

Explain why recomputing requirements only after a complete scan can over-extract
members, and why a per-archive fixed point does not mean rescanning the entire
command line. Describe how an index could accelerate discovery without changing
the selection contract.

## References

[GNU ar](https://sourceware.org/binutils/docs/binutils/ar.html) and
[GNU ld options](https://sourceware.org/binutils/docs/ld/Options.html) describe
archive tools and input-order behavior. This handout states its restricted policy
explicitly; later extensions must add contracts and tests before changing it.

## Frame the archive before resolving names

Scan all member framing before name resolution. A malformed later member header must be found before an invalid earlier name. header_offset identifies an archive member; duplicate filenames are valid and must not be used to deduplicate members.

Contract coverage: `framing_precedes_name_resolution`.

## Method map: parse an archive, then select members

`members` parses the ar(5) framing: the global magic, 60-byte member headers, decimal size, and odd-length newline padding. Framing is validated before resolving short names or the long-name table, because a malformed header is a structural error even when an earlier name is also invalid. `header_offset` is retained so duplicate filenames remain distinguishable.

`select` models the linker’s archive scan. Start with undefined global names, scan members in archive order, extract a member only when it defines a currently required name, and repeat until a fixed point. A weak reference alone does not extract a member. Metadata members are recognized but not passed to the ELF parser.

`link` is a thin composition layer; the important learning target is that archive extraction is demand-driven, not “load every object in the archive.”

## Snapshot boundary

This directory is a complete, independently buildable project. Earlier modules
contain reference implementations; this assignment owns `src/archive.rs`.
The public interfaces stay stable so you can substitute your own earlier modules.

Run `cargo test --test prerequisites` from this directory to check the supplied
foundation independently of this assignment.
