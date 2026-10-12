# Class 5: Link Across Object Boundaries

Implementation: [`src/relocate.rs`](src/relocate.rs).


Produce one executable from several code-only ELF objects. References now cross
input sections and files. A correct symbol name lookup is not enough: you must
translate the selected symbol's section-relative value through the contribution
layout and write the appropriate displacement or address into the caller.

## Prerequisites and boundaries

The completed modules from classes 0–4 are supplied. Implement C5.1–C5.3 in [`src/relocate.rs`](src/relocate.rs). The supplied CLI accepts
`class5 -o output input.o ...`; it performs I/O, not linking. Keep public APIs,
provided tests, and dependencies unchanged. Private helpers and extra tests are
allowed. Do not invoke a system linker or use an external ELF implementation.

Code sections are PROGBITS with flags exactly ALLOC|EXECINSTR (6). Other allocated
sections must be empty. Nonallocated metadata can be omitted, but relocation
tables must still satisfy the policy below. COMMON storage, data/BSS, TLS, GOT,
and dynamic linking are later-stage work, not implicit fallbacks.

## C5.1: Merge contributions (20 points)

Walk contributions in supplied order. Normalize zero alignment to one and reject
non-powers-of-two or alignments above 4096. Round the current merged length up to
each contribution's alignment, zero-pad, and copy bytes. Record its relative
start under `(file, section)` even for empty contributions. A section index alone
is not globally unique. Reject duplicate identities.

Maximum merged length is `class5::executable::MAX_IMAGE - class5::executable::PAGE`, reserving enough room
for the executable headers at any supported alignment. Check before allocation.
The final merged byte vector must be nonempty. Return the maximum normalized
alignment as well: class4 uses it to align the whole code image.

## C5.2: Apply relocation (35 points)

`offset` indexes the mutable byte slice. `place` is the runtime address of the
field, not its file offset and not the next instruction address. `symbol` is the
selected runtime value. `addend` is signed. Compute using a wide signed intermediate and reduce the result modulo 2^64.
For a 32-bit field, require that zero extension (32) or sign extension
(PC32/PLT32/32S) reconstructs that 64-bit value. Truncating an out-of-range
32-bit result is wrong; full-width 64-bit arithmetic is modular.

| Kind | Numeric value | Calculation | Encoded range |
|---|---|---|---|
| NONE | 0 | No operation | No field |
| 64 | 1 | S + A | Full 64-bit result |
| PC32 | 2 | S + A - P | i32 |
| PLT32 | 4 | S + A - P | i32, static direct binding here |
| 32 | 10 | S + A | u32 |
| 32S | 11 | S + A | i32 |

The signed ranges describe the interpretation of the encoded bit pattern, not a
comparison of the original unsigned address with `i32::MAX`. For example,
`S=0xfffffffffffffff0, A=0` has the 64-bit representation of -16. Its low word is
`0xfffffff0`; sign extension recovers S, so 32S accepts it and writes
`f0 ff ff ff`. Zero extension gives `0x00000000fffffff0`, so 32 rejects it.
Conversely, `S=0x0000000080000000, A=0` is accepted by 32 and rejected by 32S.
The same low four bytes can represent different 64-bit values under these rules.

NONE succeeds without consulting offset or values. For other kinds, reject an
unsupported kind before checking field bounds, then check the numeric range.
Write exactly four or eight little-endian bytes. An error must leave the entire
slice unchanged. This makes error handling independent of partial writes.

## C5.3: Connect the pipeline (45 points)

For each file in order:

1. Parse sections through class1. Collect every PROGBITS/flags=6 section,
   including empty ones. Reject any other nonempty SHF_ALLOC section.
2. Require exactly one SYMTAB (SymbolTableCount otherwise). Parse through class2.
   Reject any COMMON symbol as UnsupportedCommon; storage allocation comes later.
3. Retain section lists and symbol tables in input order. Merge the contributions
   through C5.1 and build one global map with class3.

Choose `entry_name` from the global map. Absence is MissingEntry. It must select a
section definition in an emitted contribution; its value must be strictly inside
that input section. Otherwise return InvalidEntry. Entry offset is contribution
offset plus symbol value. Call class5::executable::layout using merged length, maximum
alignment, and that offset. Define `code_base = BASE + layout.code_offset`.

Process relocation sections in file, section, then record order:

- Reject any nonempty SHT_REL with InvalidRelocationTarget. Empty REL is ignored.
- Decode every SHT_RELA using class2, including empty tables. Its sh_link must be
  the file's selected SYMTAB. Its sh_info must identify an emitted code section;
  otherwise return InvalidRelocationTarget. This stage rejects debug relocations
  rather than guessing which are safe to discard.
- NONE records do not bind their symbol or patch bytes. Other records use
  class5::resolve::bind on the symbol index owned by that table.
- Zero bindings yield S=0. ABS definitions yield their original value. Section
  definitions yield code_base + contribution offset + symbol value. For general
  references a value equal to section size is allowed (an end label); greater
  values or definitions in omitted sections yield InvalidSymbol. COMMON yields
  UnsupportedCommon.
- P is code_base + target contribution offset + r_offset. Invoke C5.2 on the
  target contribution's slice, using r_offset, so a malformed patch cannot spill
  into the next contribution. Bounds failure takes precedence over value overflow.
  Check the offset/address additions; an out-of-range target is PatchBounds.

Finally pass the patched merged code to class5::executable::executable. Do not patch the input
objects or count their original file offsets as output positions. The input byte
slices are immutable, which makes repeat linking deterministic.

Errors from earlier classes propagate in wrapper variants. Merge errors precede
global-resolution errors; entry errors precede relocation application. Within a
file, allocated-section policy precedes symbol-table validation. No stable order
is promised between unrelated malformed relocation records beyond traversal order.

## Completion

```sh
cargo test -p class5
python3 scripts/grade.py class5
cargo run -p class5 -- -o program start.o answer.o
```

Native tests assemble an entry point and compile a separate C function, link
objects in both orders, execute the outputs, and inspect patched branches. They
also cover missing definitions, forbidden data, target bounds, negative addends,
unsigned versus signed truncation, and atomic patch failures. Whole-stage grading
includes all prior stages in debug and release.

Explain in your design notes why P refers to the displacement field and why a
call's usual addend is -4. State which coordinates each offset in your pipeline
uses; naming everything `address` obscures real implementation errors.

## Reference

[x86-64 psABI, Object Files](https://gitlab.com/x86-psABIs/x86-64-ABI/-/blob/master/x86-64-ABI/object-files.tex), relocation definitions and extension checks.

## Fixture compiler options

The native C fixture uses `-ffunction-sections`, `-fno-pie`,
`-fno-stack-protector`, `-fno-asynchronous-unwind-tables`, `-fno-unwind-tables`, and
`-fcf-protection=none`. This isolates code contributions and avoids allocated
unwind/property metadata that this milestone explicitly does not support. The
last option matters on toolchains with control-flow protection enabled by default.
Later stages expand accepted inputs; silently dropping allocated metadata is not
an acceptable shortcut in this stage.

## Relocation failure order and atomic writes

NONE returns without accessing the destination. Otherwise reject unsupported kinds before checking patch bounds, then check the numeric result. A 32-bit relocation with both an out-of-bounds destination and an unrepresentable value returns PatchBounds. Every error leaves the entire byte slice unchanged: calculate and validate before writing any part of the field. The competing-error test uses sentinel bytes to verify this guarantee.

## Method map: merge, patch, link many objects

`merge` assigns every input code section a stable contribution identity and lays them out in input order. Alignment inserts padding before a contribution; it never changes the contribution’s logical bytes. Duplicate identities and unsupported section shapes are rejected before any output plan is used.

`apply` implements the small relocation subset introduced here. It selects a field width from the relocation kind, checks the destination interval, computes `S + A` or `S + A - P` with the required signed/range rules, and only then writes the complete field. Bounds errors take precedence over numeric overflow where the contract says so, and every error leaves the destination unchanged.

`link` composes class1–3 parsing, symbol resolution, layout, and relocation application. Relocations are processed in file/section/record order so two inputs with the same bytes but different order can intentionally produce different contribution addresses while remaining deterministic.

## Snapshot boundary

This directory is a complete, independently buildable project. Earlier modules
contain reference implementations; this assignment owns `src/relocate.rs`.
The public interfaces stay stable so you can substitute your own earlier modules.

Run `cargo test --test prerequisites` from this directory to check the supplied
foundation independently of this assignment.
