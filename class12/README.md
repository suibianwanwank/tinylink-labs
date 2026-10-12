# Class 12: Make the program debuggable

Implementation: [`src/debug.rs`](src/debug.rs).


The kernel reads the ELF header and program headers to load an executable;
it does not use section names to decide what to map. Debuggers, `nm`, `addr2line`, and
`objdump` need more: section headers that say where `.text` and
`.debug_info` are, a symbol table, and DWARF whose addresses and cross-section
offsets are final. Up to now every non-allocated section was dropped. This
class preserves its upstream file prefix except for the four section-table fields
of the ELF header, and appends what tools need. The existing program headers,
entry address, code, and data layout stay unchanged; the added data does not
extend a PT_LOAD mapping.

DWARF in a relocatable object is full of relocations. `DW_AT_low_pc` of a
function is an `R_X86_64_64` against its code section. The offset of a string
in `.debug_str` is an `R_X86_64_32` against `.debug_str` itself, because the
linker concatenates every object's `.debug_str` and the offsets move. And
when GC removes a function, its DWARF entry remains and must be marked
"discarded" with a tombstone value rather than pointing at whatever code now
occupies that address.

## Deliverables

Implement C12.1–C12.5 in [`src/debug.rs`](src/debug.rs), with private helpers as needed. Public
APIs, CLI, and tests are not student edit areas. Reuse the class10 and class11
prepare/lower interfaces and class5 relocation. The supplied `Lowered` enum
accepts a plain class8 image, class10's ET_EXEC merge/unwind image, or class11's
PIE/TLS/merge/unwind image. `finish` uses the same debug pipeline without repeating
layout. The supplied `link` uses class10; `link_pie` and the CLI's `--pie` use class11.

| Task | Responsibility | Points |
|---|---|---|
| C12.1 | Gather `.debug_*` inputs into output sections | 15 |
| C12.2 | Relocate one debug field | 15 |
| C12.3 | Encode `.symtab` and `.strtab` | 10 |
| C12.4 | Append sections and the section header table | 20 |
| C12.5 | Cumulative integration | 40 |

Run `python3 scripts/grade.py class12` on native x86-64 Linux. The native tests
need GDB, `nm`, `addr2line`, `readelf`, and GNU `ld` (as an oracle), and a
system where the test user may `ptrace` its own children. The C debug fixtures are compiled
with `-g -Og -fno-asynchronous-unwind-tables`, so call-frame information comes
in `.debug_frame` rather than `.eh_frame`.

## C12.1: debug sections

Consider only `metadata` entries whose name starts with `.debug_`; everything
else (`.comment`, `.note.GNU-stack`) is ignored. Group inputs by exact name in
order of first appearance. Each input starts at the next multiple of its
normalized alignment (0 means 1) in its group, padded with zeros; inputs keep
supplied order. The group records the first input's flags and entry size and
the largest alignment.

Flags must be 0 or `DEBUG_STRINGS` (MERGE|STRINGS, used by `.debug_str` and
`.debug_line_str`), and the same for every input of a group. Compressed
sections (`SHF_COMPRESSED`, from `-gz`) and alignments above 4096 are also
`UnsupportedDebug(id)`. Debug strings are concatenated, not deduplicated:
correctness does not require merging them. A repeated identity is
`Output(DuplicateIdentity)`. A group larger than `u32::MAX` bytes is
`TooLarge`, because 32-bit DWARF stores section offsets in four bytes.

## C12.2: one debug field

`r.offset` is relative to the input debug section, not the combined output
section. In `finish`, pass only that input's region to `relocate_debug`:
if it starts at output offset 32, a relocation at input offset 8 writes output
bytes 40..44 for a four-byte field. Resolve the target independently: a string
contribution starting at output offset 4, with symbol value 0 and addend 4,
produces target offset 8, not patch position 40 or a file address. Relocation
width does not decide whether a target is an address or a section offset.

`relocate_debug` receives the bytes of one input's region of an output section
and the relocation's resolved target. NONE does nothing. Ordinary targets use
`R_X86_64_64` (1) or `R_X86_64_32` (10). `TlsOffset` requires
`R_X86_64_DTPOFF64` (17) or `R_X86_64_DTPOFF32` (21). An unknown kind or a
kind/target mismatch is `Unsupported(kind)` before bounds are examined.

| Target | Value written |
|---|---|
| `Address(a)` | a + addend |
| `Debug { offset, .. }` | offset + addend: a section offset, not an address |
| `TlsOffset(offset)` | offset + addend: module-relative TLS offset |
| `Discarded` | the tombstone; the addend is ignored |

The tombstone is 1 in `.debug_ranges` and `.debug_loc`, where a (0, 0) pair
would end a DWARF 4 list early, and 0 everywhere else. That is GNU ld's
convention; GDB ignores code ranges that start at 0 when no section is
there. Use `class12::relocate::apply` for width, range, and bounds checks (place 0), so
the bytes are unchanged on error and `R_X86_64_32` values must fit in u32.
DTPOFF64 uses the 64-bit field check; DTPOFF32 uses the signed 32-bit check
(class5 kinds 1 and 11 respectively), rather than the unsigned debug-offset
check. Discarded is only valid with ordinary kinds. Neither a TLS module offset
nor a TP-relative offset is an image virtual address.

## C12.3: symbol table

`encode_symbols` writes a null entry, then every local symbol (binding 0, the
high nibble of `info`) in supplied order, then every other symbol in supplied
order. ELF requires locals first; `first_global` is the index of the first
non-local entry and becomes `.symtab`'s `sh_info`. Each entry is 24 bytes:
`st_name` (u32), `st_info`, `st_other` (0), `st_shndx` (u16), `st_value`,
`st_size`.

`strtab` begins with one NUL. The first symbol with a given name appends it
with a terminating NUL; later symbols with the same name reuse that offset. An
empty name uses offset 0. A name containing NUL is `InvalidName`. Tables whose
offsets do not fit in u32 are `TooLarge`.

## C12.4: section headers

`attach_sections` requires an ELF file of at least 64 bytes with the ELF magic
and no section table yet (`e_shoff` and `e_shnum` zero), else
`InvalidSection`. Header 0 is null; `sections[i]` becomes header `i + 1`; a
generated `.shstrtab` is last.

- `File { offset, size }` must lie inside the input file (`InvalidSection`
  otherwise, including overflow). The header records it as given.
- `NoBits { size }` records the input file's length as `sh_offset`, where it
  would begin if stored, and occupies no bytes.
- `Data(bytes)` is appended at the next multiple of the section alignment
  (0 means 1), padded with zeros, in supplied order.

Alignment must be 0 or a power of two, and names must not contain NUL
(`InvalidSection`). `.shstrtab` holds a NUL, then each section name with a NUL
in supplied order, then `.shstrtab`, without sharing. It is appended after the
data with alignment 1; the header table follows at the next multiple of 8.
Finally set `e_shoff`, `e_shentsize` (64), `e_shnum` (count including null and
`.shstrtab`), and `e_shstrndx` (the last index). A total count greater than or
equal to `SHN_LORESERVE` (0xff00) is `TooLarge`: ELF requires extended numbering
at that boundary, and this task emits ordinary numbering only. The null entry
and generated `.shstrtab` count toward the limit.

The only mutable ranges in the original ELF prefix are `40..48` (`e_shoff`),
`58..60` (`e_shentsize`), `60..62` (`e_shnum`), and `62..64` (`e_shstrndx`).
In particular, `52..54` is `e_ehsize`, the ELF header's own size; it is not
the size of a section-header entry and must remain unchanged.

## C12.5: integration

`finish` is the task; the supplied `link` prepares with `class12::unwind::policy`,
calls `class12::unwind::lower_prepared`, and calls `finish` with no aliases.
`lowered.into()` yields `Lowered`; use `image()` to borrow placements, and
`emit()` only after resolving debug references and building output sections.
The supplied `link_pie` prepares with class11's policy and passes its complete
lowered result to the same `finish` task. Each variant uses its own serializer,
preserving its program headers. A later backend that folds
sections passes `aliases`: an aliased section has no placement, and its
symbols live at the same offset in the aliased-to section.

1. Borrow the image from `lowered`. Both `MergeUnwind` and `PieTls` provide a
   retained `merged` mapping and upstream map; `Plain` has no merge mapping.
   For `PieTls`, retain its TLS template and apply the coordinate rules below.
2. `collect_debug(prepared.metadata())`.
3. For every RELA group targeting a collected debug input, in order, skip NONE
   and resolve each record's symbol with class3:
   - unresolved WEAK is `Address(0)`; an undefined GLOBAL is
     `FrontEnd(Undefined(name))`;
   - a COMMON is `Address` of its allocation, or `Discarded` if it was not
     allocated;
   - ABS is `Address(value)`;
   - a retained TLS definition is `TlsOffset(template offset + value)`;
   - a definition in a debug input is `Debug { index, offset: input offset +
     value }`; in placed storage it is `Address(address + value)`; in
     allocated storage without a placement (removed by GC, or aliased) it is
     `Discarded`; in other metadata it is `InvalidDebugReference { section,
     offset }`. An aliased copy's DWARF is discarded so that only one DIE
     claims the shared code.
   Apply `relocate_debug`; failures are `Debug { section, offset, kind, cause }`.
4. Allocated output sections, one per class6 kind that has placements, in the
   order `.text` (AX), `.rodata` (A), `.data` (WA), `.bss` (WA, NOBITS):
   address and size span the lowest placement through the highest end,
   `.bss` also covering allocated COMMONs; file offset is that of the lowest
   placement; alignment is the largest contribution or COMMON alignment.
5. Symbols: for each selected object in order, its named local symbols other
   than SECTION and FILE that are defined in placed storage or ABS; then each
   global selection in byte-name order: definitions in placed storage, ABS
   definitions, and allocated COMMONs (as GLOBAL OBJECT in `.bss`).
   Definitions in an aliased section use the aliased-to placement; other
   unplaced definitions are omitted. Ordinary image values are final VAs for
   ET_EXEC and RVAs for PIE; `st_shndx` selects the output section holding the
   placement, or `SHN_ABS`. TLS definitions use the separate rules below.
6. Sections: the allocated ones, the debug sections (PROGBITS with their
   flags), `.symtab` (link to `.strtab`, info `first_global`, alignment 8,
   entry size 24), and `.strtab`. Emit with `Lowered::emit` and attach them.

For merged storage, an input section has no single output base. Resolve a
symbol's `st_value` through `class12::unwind::locate`, then add the corresponding
synthetic group placement address (`MERGE_BASE + group`, file `usize::MAX`).
For a debug relocation against a SECTION symbol, translate `st_value + addend`
as one input coordinate and set the relocation addend to zero. For other
symbols translate `st_value`, leaving the addend for `relocate_debug`.
A negative or overflowing input coordinate is `TooLarge`; class10 failures
are `MergeUnwind(error)`. A split section absent from the retained mapping
receives a tombstone. Named symbols in retained merged pieces use the same
mapping when constructing `.symtab`.

When describing allocated sections, exclude class10's PHDR reservation and
`.eh_frame`/`.eh_frame_hdr` from the aggregate `.text`/`.rodata` extents.
Describe the two unwind placements separately as allocated PROGBITS sections,
with alignment 8 and 4 respectively. Include synthetic merged groups in
`.rodata` with their group alignment. This preserves both runtime lookup via
program headers and tool lookup via section names.

For a plain image, the map is class8's map, from `class12::gc::placed_rows`, where aliased rows take
their target's address, followed by one `DEBUG <size> <name>` line per debug
output section. For merge/unwind images preserve the upstream map verbatim
and append those DEBUG lines. Without debug input the output still gains `.symtab` and
section headers, so `nm` works on every class12 executable.

## Acceptance and references

Native tests run a GDB session (breakpoint by function name, argument and
global values, line lookup, backtrace, `finish`, exit) on the class12 output
and on GNU ld's output for the same objects, and require the sessions to match
after removing addresses and process IDs. They also compare `addr2line` results
for function symbols. A two-CU fixture exchanges caller/callee input order and
checks a breakpoint in the callee, both source filenames in the backtrace,
arguments, globals, return values, and warning-free DWARF decoding. Each CU must
remain readable when its debug contributions start after another input's bytes.
Other checks require warning-free `readelf --debug-dump` output and distinguish
a collected function's zero `DW_AT_low_pc` from a kept function's live address.
They compare the original prefix with the upstream image byte for byte, except
for the four permitted ELF-header fields. Compressed debug sections,
PC-relative debug relocations, and references to `.comment` must be rejected.

- [DWARF 5 standard](https://dwarfstd.org/doc/DWARF5.pdf)
- [ELF gABI: section header and symbol table](https://www.sco.com/developers/gabi/latest/ch4.sheader.html)
- [GDB manual](https://sourceware.org/gdb/current/onlinedocs/gdb.html/)

The combined native fixture retains two identical four-byte constants, CFI,
and debug relocations against both named symbols and SECTION-plus-addend.
It checks that the constants share an address, all debug references follow
the piece mapping, the fifth program header survives, unwind sections remain
identifiable, and the executable still returns 42.

## PIE/TLS composition

`Lowered::PieTls` preserves class11's entire lowered result and serializer.
Keep its `image.placements` in planning coordinates. Convert ordinary image
addresses to RVAs by checked subtraction of `class12::executable::BASE` when writing DWARF,
ordinary `.symtab` values and allocated section `sh_addr`. ABS values and
unresolved WEAK zero are constants: do not subtract BASE from them. Debug-section
references remain section offsets. Preserve the upstream map and PT_TLS / unwind
headers rather than reconstructing them from the section table.

| Value | Meaning | Example: TLS offset 12, TP at block offset 16 |
|---|---|---|
| Ordinary PIE image value | RVA; runtime VA adds load bias | image RVA 0x1000 is unrelated to either TLS value |
| TLS module offset | `tls.offsets[input] + st_value` | 12; written to DWARF DTPOFF and STT_TLS symbols |
| TP-relative offset | module offset minus `thread_pointer` | -4; used by class11's TPOFF/initial-exec code |

A symbol defined in a retained TLS input resolves to `TlsOffset`, using checked
addition. `STT_TLS` (type 6) output symbols keep that module-relative value and
use the index of `.tdata` or `.tbss`, not `.data` or `.bss`. Keep the binding nibble.
Ordinary merged symbols and SECTION-plus-addend references still use class10's
piece mapping before RVA conversion.

Describe `TLS_IMAGE` separately as `.tdata`: PROGBITS, TLS_FLAGS, its file offset,
initialized length, RVA and template alignment. If `memory_size` exceeds that
length or a retained TLS input is NOBITS, describe the remainder as `.tbss`:
NOBITS, TLS_FLAGS, address immediately
after `.tdata`, size `memory_size - initialized`, alignment 1. This description
includes template alignment gaps and does not allocate another TLS block. Its
NOBITS offset is the original emitted-file length under C12.4. `TLS_AREA` is the
ordinary BSS storage used by the provided main-thread startup, not the TLS template.
A zero-sized NOBITS input still needs `.tbss` for its symbols' section identity,
even when the output remainder has size zero.

Exclude `TLS_IMAGE`, `PHDR`, `UNWIND_PHDR`, `EH_FRAME` and `EH_FRAME_HDR` from
ordinary aggregate extents. Use class11's unwind identities to emit the two
separate unwind section descriptions; class10's identities belong only to its
ET_EXEC variant. Other startup/GOT/relocation contributions remain in their
ordinary class6 kind. Do not mutate placements or add the image base to TLS offsets.

The combined native fixture includes two debug CUs, duplicate constants, a real
GOT slot, runtime unwind metadata, and optional initialized/zero-filled TLS.
It checks ordinary RVA fields, module-relative TLS symbols, original-prefix
preservation, repeated execution and GDB inspection of ordinary arguments,
globals and cross-file frames. TLS variables' automatic debugger lookup also
requires a runtime TLS-discovery protocol; the supplied class11 startup does not
implement a complete libc/thread_db protocol. Correct DWARF offsets alone do not
establish that additional capability.

## Snapshot boundary

This directory is a complete, independently buildable project. Earlier modules
contain reference implementations; this assignment owns `src/debug.rs`.
The public interfaces stay stable so you can substitute your own earlier modules.

Run `cargo test --test prerequisites` from this directory to check the supplied
foundation independently of this assignment.

The final file, including appended data, alignment padding, names, and section
headers, is bounded by `executable::MAX_IMAGE` (64 MiB). `attach_sections` returns
`TooLarge` before allocating padding or appending bytes that exceed this bound.
This also prevents a valid but enormous power-of-two alignment from exhausting
memory. `NoBits` sizes describe memory and do not allocate file bytes.

`collect_debug` also bounds the sum of all output debug groups, including
alignment padding, by the same 64 MiB limit before appending each contribution.
The budget is shared across names; splitting data into distinct groups does not
bypass it. An oversized collection returns `TooLarge`.
