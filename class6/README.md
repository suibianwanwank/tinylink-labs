# Class 6: File Bytes, Memory Bytes, and Permissions

Implementation: [`src/layout.rs`](src/layout.rs).


Extend the static linker to initialized globals, read-only constants, BSS, and
COMMON storage. A memory region can exist without occupying bytes in the file.
The output must describe that distinction to the kernel while maintaining
read/execute, read-only, and read/write protection boundaries.

## Implementation surface

Complete C6.1–C6.3 in [`src/layout.rs`](src/layout.rs). The CLI is supplied. Reuse class1 parsing,
class2 symbol/RELA decoding, class3 resolution, and class5 relocation arithmetic.
Keep earlier tests passing: class5 remains a bounded earlier milestone; do not
rewrite it to silently accept all the new formats. Public types and provided
tests are fixed; private helpers and student tests are allowed.

## C6.1: Storage plan (35 points)

`Contribution` records an input identity, kind, alignment, file bytes, and memory
size. Validate contributions before packing. Zero alignment means one; otherwise
it must be a power of two no greater than 4096. Initialized kinds require
memory_size == bytes.len(). BSS requires an empty byte slice but may have a large
memory_size. Reject duplicate `(file, section)` identities even for empty inputs.

Validate COMMON entries after contributions: alignment as above, nonempty name,
and no duplicate name. `InvalidSize` covers empty COMMON names. The pipeline
supplies already-merged COMMON requirements; the planner must not merge them a
second time. Validate metadata before testing for at least one nonempty Text.

Pack groups in this order, retaining supplied order within each group:

1. **Text:** begin after 64 ELF-header bytes and four 56-byte program headers
   (288 bytes total). Align each contribution, zero-pad, then append its bytes.
2. **ReadOnly:** round the current file length up to a 4096-byte boundary. Pack
   read-only contributions with their individual alignments.
3. **Data:** round file length to a 4096-byte boundary again. Pack initialized
   writable contributions, then stop growing the file.
4. **Bss:** use a separate memory cursor initialized to file length. Align and
   advance it without appending bytes. Record no file offset for these entries.
5. **COMMON:** continue the memory cursor, with one allocation per supplied name.

Keep zero-size placements, including any alignment they require. Empty ReadOnly
and Data groups still begin on page boundaries. Their segment sizes can be zero;
if ReadOnly is empty its start can equal Data's start. Header/data padding in the
file is zero. Memory-only padding after initialized Data is zero-filled by the
kernel along with BSS.

All file offsets and memory-end offsets relative to BASE are bounded by
`class6::executable::MAX_IMAGE` (64 MiB). Reject overflow or excess with TooLarge before
allocation. Require at least one nonempty Text contribution.

Return three segments in RX/R/RW order:

- RX begins at file offset zero and covers headers through the last Text byte.
- R begins at the ReadOnly group's page-aligned start and covers that group.
- RW begins at Data's page-aligned start. Its file_size ends after initialized
  Data, but its memory_size extends through BSS and COMMON.

For this fixed-base layout, virtual address is BASE + relative offset. Record
both coordinates explicitly. BSS/COMMON addresses have no backing file bytes.

### Worked storage plan

Consider these contributions, supplied in Data, Bss, Text, ReadOnly order, all
from file 0. A COMMON named `large` requires 8192 bytes with alignment 4096.
The planner groups contributions by kind before assigning their positions.
Intervals below are half-open; all offsets and sizes in the table are decimal.

| Section | Kind | Alignment | File bytes | Memory size | Planned file interval | Virtual start |
|---|---|---|---|---|---|---|
| 1 | Text | 16 | 7 | 7 | [288, 295) | 0x400120 |
| 2 | ReadOnly | 4 | 4 | 4 | [4096, 4100) | 0x401000 |
| 3 | Data | 8 | 8 | 8 | [8192, 8200) | 0x402000 |
| 4 | Bss | 32 | 0 | 64 | None | 0x402020 |
| `large` | COMMON | 4096 | 0 | 8192 | None | 0x403000 |

The file ends at 8200, immediately after Data. The memory cursor rounds 8200 up
to 8224 for Bss, advances to 8288 after its 64 bytes, rounds up to 12288 for
COMMON, and ends at 20480. Neither memory alignment gap grows the file.

| Segment | Relative start | File size | Memory size |
|---|---|---|---|
| RX | 0 | 295 | 295 |
| R | 4096 | 4 | 4 |
| RW | 8192 | 8 | 12288 |

The RW memory size is `20480 - 8192 = 12288`, not the COMMON size and not the
total image size. It includes initialized Data, Bss, COMMON, and the intervening
memory alignment gaps. The kernel zero-fills the tail after Data. By contrast,
the gaps before ReadOnly and Data are actual zero padding in the output file.
`separate_file_and_memory` checks these placements and segment lengths directly;
native zero-fill tests additionally check the resulting process memory.

## C6.2: Serialize the image (25 points)

`emit` consumes an Image returned by `plan`, after relocation has modified only
payload bytes. Its metadata, length, and padding must be preserved by callers;
arbitrarily fabricated Image metadata is outside this internal-stage interface.
Reject an entry address that is not strictly inside a nonempty Text contribution.
Headers and alignment padding are not valid entry points.

Write the same ELF64/x86-64/ET_EXEC header fields as class4, except phnum=4.
Program headers start at offset 64, with entry size 56:

- Entries 0–2: PT_LOAD=1, flags from the plan, p_offset=segment offset,
  p_vaddr=p_paddr=BASE+offset, p_filesz=file_size, p_memsz=memory_size,
  p_align=4096.
- Entry 3 at offset 232: PT_GNU_STACK=0x6474e551, flags=6, alignment=16;
  other fields zero.

Unspecified ELF fields and identification bytes are zero. No section table is
emitted. Preserve already-relocated payload bytes. Zero the header area before
serializing; do not regenerate payload from the original inputs.

## C6.3: Cumulative linking (40 points)

Classify every SHF_ALLOC section using exact type/flags pairs:

| Type | Flags | Kind |
|---|---|---|
| PROGBITS=1 | ALLOC\|EXECINSTR=6 | Text |
| PROGBITS=1 | ALLOC=2 | ReadOnly |
| PROGBITS=1 | WRITE\|ALLOC=3 | Data |
| NOBITS=8 | WRITE\|ALLOC=3 | Bss |

Unsupported allocated sections are rejected even if empty. This deliberately
excludes TLS, group flags, merge flags, and allocated property notes until those
features receive a defined policy. Nonallocated metadata is not emitted.

For each input, parse and classify sections, require one SYMTAB, and parse its
symbols. Build the class3 global map before planning. Gather only selections
that remain COMMON after strong/weak resolution, in byte-name map order. Supply
their merged size and alignment to the planner. A strong definition overriding
COMMON must not cause a redundant COMMON allocation.

The selected entry must be a section definition inside Text. Resolve relocation
symbols as follows: Zero bindings give zero; absolute definitions give their
value; section definitions give placement.address + value (allow an end label,
but not a value beyond section size); selected COMMON uses common_addresses by
name. A definition in omitted storage is InvalidSymbol.

Process RELA in file/section/record order. Require its owning SYMTAB to match the
selected table and its target to have file-backed placement. Reject targets in
BSS or omitted metadata with InvalidRelocationTarget. Nonempty REL is rejected;
empty REL is ignored. NONE records are no-ops. Bound all other writes to the
specific target contribution and use class6::relocate::apply for arithmetic and field
checks. Thus a data pointer can refer to BSS, but BSS cannot itself contain a
stored relocation field. Finish with `emit`.

Error wrappers retain earlier errors. In the pipeline, section policy precedes
symbol-table count, global resolution precedes planning, and entry checks precede
relocation application. In `plan`, metadata checks precede packing/resource checks.

## Completion

```sh
cargo test -p class6
python3 scripts/grade.py class6
cargo run -p class6 -- -o program start.o globals.o
```

Native tests link code compiled with the explicit fixture flags from class5.
They check initialized globals, BSS zeroing across pages, shared COMMON allocation,
a pointer in Data targeting BSS, and CLI output. Separate probes must receive
SIGSEGV when writing Text/ReadOnly or executing Data. Core dumps are disabled and
all probes are time-bounded. Structural tests separately inspect segment flags
and file-versus-memory sizes, so a random crash does not establish correctness.

Explain how alignment gaps after initialized Data become zero-filled memory, and
why allocating a zero-filled vector of BSS size would generate a needlessly large
file. State which operations need a file offset and which need a virtual address.

## Validate before allocating

Validate all contributions, then COMMON metadata, then the nonempty Text requirement, before packing. An input without Text may therefore report InvalidAlignment first. A huge early BSS must not trigger allocation or TooLarge before malformed metadata in a later contribution is checked.

Contract coverage: `metadata_validation_precedes_packing`.

## Method map: separate file size from memory size

`plan` validates every contribution before allocating the output image. `Kind::Text`, `ReadOnly`, and `Data` have stored bytes whose length equals memory size; `Kind::Bss` has no stored bytes but does consume memory. COMMON entries are validated separately and allocated after BSS. This is why `file_offset: None` for BSS is correct rather than a missing implementation.

The three load groups are packed in fixed order. Each group’s file cursor is aligned independently, while the data segment’s memory size includes trailing BSS and COMMON zero-fill. `Placement.address` is a virtual address (`BASE + offset`); `file_offset` is a byte index. Keeping both coordinates prevents the classic “write BSS into the file” error.

`emit` serializes the plan into program headers and payload bytes, checking the entry falls inside initialized Text rather than padding or BSS. `link` only assembles the earlier parsers and planner; it does not invent a new layout policy.

## Snapshot boundary

This directory is a complete, independently buildable project. Earlier modules
contain reference implementations; this assignment owns `src/layout.rs`.
The public interfaces stay stable so you can substitute your own earlier modules.

Run `cargo test --test prerequisites` from this directory to check the supplied
foundation independently of this assignment.
