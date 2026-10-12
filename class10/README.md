# Class 10: Rewrite sections instead of copying them

Implementation: [`src/unwind.rs`](src/unwind.rs).


The earlier storage backends place each retained ordinary input section in one
contiguous output range, copy its bytes, and apply patches. Two common kinds of
compiler output require a finer mapping between input and output.

A string literal used in two translation units is emitted twice, in sections
marked `SHF_MERGE|SHF_STRINGS`. The compiler is telling the linker that these
sections are bags of independent pieces and that identical pieces may share one
address. C makes that observable: `"abc" == "abc"` across files may be true.

`.eh_frame` holds unwind records: Common Information Entries (CIEs) shared by
many Frame Description Entries (FDEs), each describing a code address range.
The fixtures use one FDE per function. An FDE is only meaningful while the code
range it describes remains in the output. After class8's GC drops a
function, its FDE must go too, and the remaining records need a sorted search
index, `.eh_frame_hdr`, that a runtime unwinder finds through `PT_GNU_EH_FRAME`.

Both features rewrite input sections into new synthetic output sections. That
requires a front end that can hand such sections to the backend instead of
treating them as plain storage: class8's `prepare_with` and its `Role`s.

## Deliverables

Implement C10.1–C10.5 in [`src/unwind.rs`](src/unwind.rs), with private helpers as needed. Public
APIs, the supplied `policy`, CLI, and tests are not student edit areas. Reuse
`class10::gc::prepare_with`, class6 planning and emission, and class5 relocation; do
not reparse maps, reimplement GC, or invoke another linker.

| Task | Responsibility | Points |
|---|---|---|
| C10.1 | Split a mergeable section into pieces | 10 |
| C10.2 | Deduplicate pieces and map references through them | 20 |
| C10.3 | Frame and validate `.eh_frame` records with their relocations | 20 |
| C10.4 | Emit `.eh_frame` and its `.eh_frame_hdr` index | 20 |
| C10.5 | Cumulative linking with merged storage and unwind tables | 30 |

Run `python3 scripts/grade.py class10` on native x86-64 Linux. The native tests
compile freestanding C with the system `cc`, and link one fixture with GNU `ld`
as an independent oracle for the fixture itself.

## Section policy

`policy` is supplied. It classifies `.eh_frame` (type PROGBITS or
`SHT_X86_64_UNWIND`, flags exactly ALLOC) as `Role::Annotation`; PROGBITS with
flags ALLOC|MERGE (0x12) or ALLOC|MERGE|STRINGS (0x32) as `Role::Split`;
everything else as class6 storage. Any other allocated flag combination,
including MERGE together with WRITE or EXECINSTR, stays unsupported.

## C10.1: pieces

`split` validates the section shape before its contents:

- flags exactly 0x12 with `entry_size >= 1`, or 0x32 with `entry_size == 1`
  (wide strings are unsupported);
- normalized alignment (0 means 1) a power of two no greater than 4096;
- for constants, a length that is a multiple of `entry_size`.

Any violation is `MergeError::Unsupported`. A nonempty string section whose last
byte is not NUL is `Unterminated`. Constants are cut every `entry_size` bytes.
Strings are cut after every NUL, so each piece includes its terminator, and the
zero padding a compiler places between aligned strings becomes empty-string
pieces. An empty section has no pieces.

## C10.2: merging and lookup

`merge` groups sections by `(flags, entry_size, normalized alignment)`. Section
names do not matter: `.rodata.f.str1.1` and `.rodata.str1.1` share a group.
Groups appear in the order their key first appears in the supplied list.

Within a group, process sections in supplied order and pieces in offset order.
A piece whose bytes equal an earlier piece of the same group reuses that
piece's group offset. Otherwise it is appended at the next multiple of the
group alignment, with zero padding. Thus every piece keeps the alignment its
input section promised, and the result is deterministic. Only whole pieces are
shared; suffix ("tail") merging is not performed.

Reject duplicate section identities with `Output(DuplicateIdentity)`. A group
that would exceed `class10::executable::MAX_IMAGE` is `TooLarge`; check before growing it
past that bound. A split error is reported as `Error::Merge` with the section.

`locate` maps an input offset to `(group, group offset)` through the piece that
contains it, preserving the offset within the piece. Offsets equal to a piece's
end belong to the next piece, if any; an offset outside every piece, or in an
unknown section, is `InvalidReference(offset)`.

### Worked input-to-output mapping

Suppose two string sections have the same group key `(0x32, 1, 1)`. Offsets in
this table are bytes relative to the named input section or to output group 0;
they are not virtual addresses.

| Input section | Input piece | Input offset | Size, including NUL | Group offset |
|---|---|---:|---:|---:|
| `(0, 4)` | `hello\0` | 0 | 6 | 0 |
| `(0, 4)` | `world\0` | 6 | 6 | 6 |
| `(1, 3)` | `world\0` | 0 | 6 | 6 |
| `(1, 3)` | `hi\0` | 6 | 3 | 12 |
| `(1, 3)` | `hello\0` | 9 | 6 | 0 |

The first section establishes `hello\0world\0`. In the second, `world\0`
reuses offset 6, `hi\0` appends at offset 12, and `hello\0` reuses offset 0.
The resulting bytes are `hello\0world\0hi\0`, length 15. Each input identity
still has its own mapping; neither its name nor its old offset becomes an
output address.

For `(1, 3)` at input offset 7, the containing piece starts at 6, so the
within-piece offset is 1. Its group offset is 12, giving
`locate(..., SectionId { file: 1, section: 3 }, 7) == Ok((0, 13))`.
At input offset 9, the next piece begins: lookup returns `(0, 0)`, not `(0, 15)`.
At input offset 15, no piece contains that byte, so lookup fails. These
half-open ranges distinguish a piece's last byte from the byte after it.

Alignment can introduce a separate change. With group key `(0x32, 1, 4)`, the
input bytes `x\0yz\0` contain pieces at input offsets 0 and 2. Output piece
starts are 0 and 4, and the output is `x\0\0\0yz\0`. Thus input offset 2 maps
to output offset 4; copying the old offset would address padding. This is the
case exercised by `locate_maps_interior_offsets`.

### Which input offset does a relocation reference?

A relocation names a symbol S and an addend A. For an ordinary symbol, the
referenced input offset is S's value; the addend is applied after mapping. That
is how `.LC0+7` points seven bytes into a string. For an `STT_SECTION` symbol
(type 3), the referenced offset is value + A, because the assembler encodes
"piece at offset k" as "section + k". After mapping, the backend passes
`address - A` as the symbol value so that class5's S+A arithmetic yields the
mapped address. GNU ld and LLD use the same rule.

Assemblers keep a local symbol, rather than a section symbol, whenever the
addend is nonzero and the offset would be ambiguous. A PC-relative reference
through a section symbol whose value + A falls outside every piece is rejected
as `InvalidReference` rather than guessed.

## C10.3: unwind records

The supported subset covers the selected x86-64 fixture options. Other compiler
options can produce additional augmentation fields or encodings:

- 32-bit DWARF framing; a length of `0xffffffff` (64-bit DWARF) is unsupported;
- CIE version 1 or 3, augmentation exactly `"zR"`, augmentation data length 1,
  FDE pointer encoding `0x1b` (`DW_EH_PE_pcrel | DW_EH_PE_sdata4`);
- FDEs whose CIE pointer leads back to a CIE that starts earlier in the same
  section;
- relocations: exactly one `R_X86_64_PC32` at each FDE's initial-location field
  (FDE offset + 8), `R_X86_64_NONE` anywhere, nothing else.

Personality routines and LSDAs (augmentation letters P and L) belong to C++
exception handling and are rejected as unsupported.

Scan records from offset 0. Each record is a 4-byte length followed by that
many bytes; its first body word is 0 for a CIE and the CIE pointer for an FDE.
The CIE pointer is the distance from the pointer field back to the CIE. A zero
length word is a terminator only as the final four bytes of the section.

Errors carry input offsets and are checked in this order:

1. framing, in section order: `Malformed(record)` for fewer than four bytes,
   a record running past the end, a length below 4, a misplaced terminator,
   CIE fields or an FDE's fixed fields and augmentation data that do not fit,
   an overlong LEB128, or a CIE pointer that does not lead back to a CIE;
   `Unsupported(record)` for the subset limits above;
2. relocations, in supplied order: `Relocation(r.offset)` for any non-NONE
   relocation that is not the first PC32 at an FDE initial-location field;
3. coverage, in section order: `Relocation(fde + 8)` for an FDE without one.

`Frame` records each record's offset and size (length field included). An
`Fde` names its CIE's offset and the index of its relocation.

## C10.4: output records and search index

`unwind_sizes` depends only on record bytes. `.eh_frame` holds each distinct
CIE byte string once, every FDE, and a 4-byte zero terminator. `.eh_frame_hdr`
is 12 bytes plus 8 per FDE. With no FDEs both are empty and the backend emits
no unwind output at all.

`emit_unwind` requires `EH_FRAME` and `EH_FRAME_HDR` placements that are
file-backed ReadOnly storage of exactly those sizes (`InvalidReservation`
otherwise). For each FDE, in supplied order, it validates that both records are
framed consistently (length field equals length minus 4; CIE body word 0; FDE
body word nonzero; FDE at least 17 bytes), that the target is a Text placement,
and that `start + range` fits inside it, where range is the FDE's u32 at offset
12. Violations are `InvalidFde { index }`.

Output: emit the FDE's CIE if these CIE bytes have not been emitted yet, then
the FDE. Rewrite the CIE pointer to the distance from the FDE's pointer field
to the emitted CIE, and the initial location to the PC-relative distance from
its field to `target address + start`. End with the terminator.

The header, with H its address:

| Offset | Field | Encoding |
|---|---|---|
| 0 | version 1, then encodings 0x1b, 0x03, 0x3b | bytes |
| 4 | `.eh_frame` address minus (H + 4) | sdata4 |
| 8 | FDE count | udata4 |
| 12 | (initial location − H, FDE address − H) pairs | sdata4 each |

Sort pairs by initial location; ties keep supplied order. Initial locations
must be strictly increasing, and each FDE's range must end at or before the
next initial location. The first FDE violating this, in sorted order, is
`OverlappingFde { index }` using its supplied index. An unwinder binary-searches
this table, so ambiguity is an error rather than a warning.

### Worked record relocation and index construction

This example starts with two input `.eh_frame` sections. Each contains the same
24-byte CIE at offset 0 and one 20-byte FDE at offset 24. There is no input
terminator. The CIE is the version-1 `zR` CIE constructed by `cie()` in
[`tests/contract.rs`](tests/contract.rs). In each FDE:

| Field | Offset within input section | Initial value |
|---|---:|---|
| Length | 24 | 16: body bytes, excluding this four-byte field |
| CIE pointer | 28 | 28: the pointer field points back to offset 0 |
| Initial location | 32 | 0, with one PC32 relocation here |
| Range | 36 | B: 4 bytes; A: 6 bytes |
| Augmentation length and remaining bytes | 40–43 | All zero |

Parsing each section therefore yields `Frame { offset: 0, size: 24,
record: Cie }` and `Frame { offset: 24, size: 20, record: Fde { cie: 0,
relocation: 0 } }` when the supplied relocation slice has its one PC32 at
index 0. The integer stored in the CIE-pointer field is a backward distance;
`Record::Fde.cie` is the resolved input offset. They are not the same value.

After binding the two references to their code, supply FDE B first and FDE A
second. B describes `(1, 1)` at `start = 1`; A describes `(0, 1)` at `start = 2`.
Use these explicit placements to evaluate `emit_unwind` in isolation:

| Contribution | Kind | File offset | Virtual address | Size |
|---|---|---:|---:|---:|
| A `(0, 1)` | Text | `0x1000` | `0x401000` | 32 |
| B `(1, 1)` | Text | `0x1020` | `0x401020` | 16 |
| `EH_FRAME` | ReadOnly | `0x2000` | `0x402000` | 68 |
| `EH_FRAME_HDR` | ReadOnly | `0x2080` | `0x402080` | 28 |

These are given inputs to the emission API, not addresses that the planner is
required to choose. The two CIE byte strings are equal, so output size is
`24 + 20 + 20 + 4 = 68` bytes. The search header needs `12 + 2 × 8 = 28` bytes.
The output `.eh_frame` is:

```text
section offset  0              24          44          64   68
                | shared CIE  | FDE B     | FDE A     | 0  |
width           | 24 bytes    | 20 bytes  | 20 bytes  | 4  |
```

The CIE is emitted once on first use. The FDEs keep supplied order; sharing a
CIE does not sort the records. Both CIE pointers now refer to the shared CIE:

| FDE | Output pointer field offset | New backward distance | Initial-location field VA | Target VA | Signed PC-relative value |
|---|---:|---:|---:|---:|---:|
| B | 28 | `28 − 0 = 28` | `0x402020` | `0x401020 + 1` | `−4095` |
| A | 48 | `48 − 0 = 48` | `0x402034` | `0x401000 + 2` | `−4146` |

For example, A's initial location decodes as `0x402034 + (−4146) = 0x401002`.
The base is the initial-location field itself, not the start of its FDE and
not the following field. Its range remains 6; range is a length rather than
another PC-relative pointer.

Let `H = 0x402080`, the header's virtual address. Its first four bytes are
`01 1b 03 3b`. The pointer at header offset 4 uses its own field as base,
so it stores `0x402000 − (H + 4) = −132`; the count at offset 8 is 2.
The search pairs use **H**, as specified by encoding `0x3b`, and are sorted
by the decoded initial code address:

| Header offset | Function VA − H | FDE VA − H |
|---:|---:|---:|
| 12 | `0x401002 − H = −4222` | `0x40202c − H = −84` |
| 20 | `0x401021 − H = −4191` | `0x402018 − H = −104` |

Thus A's pair precedes B's even though B's FDE was emitted first. A covers
`[0x401002, 0x401008)` and B covers `[0x401021, 0x401025)`; both fit their Text
placements and do not overlap. The rewritten fields and sorted pairs establish
what `unwind_shares_cies_and_sorts_search_table` checks: correct decoding from
three different bases, rather than merely the presence of an unwind section.

## C10.5: integration

Implement `lower_prepared(&class10::gc::Prepared) -> Result<Lowered, Error>`.
The supplied `link` prepares inputs with `class10::gc::prepare_with` and this
backend's `policy`, calls `lower_prepared`, then serializes with `Lowered::emit`.
Lowering borrows the prepared input and must not alter its metadata. It returns
the planned image, entry address, complete `Merged` mapping, and rendered map.
Later metadata stages can use the mapping without reopening files or assigning
different code addresses.

1. Use the supplied prepared input's tables, selections, live storage, split
   sections, annotations, and relocations; do not prepare the objects again.
2. A live split section with any non-NONE relocation is
   `Merge { cause: Relocated }`: its bytes would no longer be self-contained
   pieces. Then `merge(prepared.split())`.
3. For each annotation in order, `frames` with its relocation group (an absent
   group is empty). For each FDE, bind the relocation's symbol. It must select a
   section definition, else `FdeTarget { section, offset }` with the FDE's input
   offset. If that section is dead, drop the FDE. If it is live, it must be
   Text storage and value + addend must not be negative, else `FdeTarget`.
   Retained FDEs keep annotation order, then record order.
4. Contributions, in order: `PHDR` (Text, alignment 8, 56 zero bytes) only when
   an FDE is retained; class8's live storage; each merged group as ReadOnly
   `(usize::MAX, MERGE_BASE + g)` with the group alignment; then `EH_FRAME`
   (alignment 8) and `EH_FRAME_HDR` (alignment 4) when an FDE is retained.
   More groups than fit after `MERGE_BASE` is `TooLarge`.
5. Plan with class6, using live COMMONs exactly as class8 does. Apply the
   relocations of live storage sections with class5, mapping split targets as
   described above. Then `emit_unwind` and copy its bytes into the reservations.
6. Return `Lowered { image, entry_address, merged, map }`. The image contains
   final storage patches and unwind bytes; its reserved header bytes have not
   yet been serialized. `Merged` retains every input-piece mapping from C10.2.

### Folding and ordering without another layout

The supplied `lower_prepared` calls `lower_with(prepared, empty_aliases, [])`.
C10.5 is implemented in `lower_with`; default behavior is unchanged.
The optional inputs let later stages reuse merging and unwind emission instead
of maintaining another copy of layout and relocation logic.

`aliases` maps each folded Text section directly to its retained representative.
The caller establishes that code, references, address identity, and unwind
semantics permit that fold. This stage does not establish equivalence.
Omit aliased storage and its relocation groups; resolve references and the entry
through the representative at the original symbol offset. Drop FDEs describing
an aliased copy, retaining the representative's FDEs. Dropping an FDE here is
safe only under the caller's compatibility precondition: it is not a proof
that equal machine instructions imply equal unwind descriptions.

Resolve each `order` entry through aliases. Emit those retained Text
contributions first, ignoring duplicates, then the other live storage in input
order. A resolved entry not in retained Text is `InvalidOrderSection(id)`.
The PHDR reservation stays before all code; synthetic merged/unwind storage
retains the ordering above. Aliased map rows use the representative's address.

`folded_ordering_retains_merge_mapping_and_one_fde_per_copy` supplies two
code sections with equal instructions and equal CFI, folds one into the other,
and orders the representative before the entry. It checks redirected calls,
merged constants, a two-entry unwind index instead of duplicate FDEs, entry
redirection, invalid ordering, and exit status 42.

The supplied `Lowered::emit` calls `class10::layout::emit`. When unwind output exists,
the `PHDR` placement must be a 56-byte Text reservation at file offset
`class10::layout::HEADER_BYTES` and address `BASE + HEADER_BYTES` (`InvalidReservation`
otherwise). The serializer sets `e_phnum` to 5 and writes `PT_GNU_EH_FRAME`
there: flags R, offset/addresses/sizes of `EH_FRAME_HDR`, alignment 4. The
program-header table stays contiguous and inside the RX load segment, which
is where the kernel exposes it through `AT_PHDR`.

`lowering_retains_merge_mapping_before_serialization` checks duplicate input
constants mapping to one output offset, immutable input bytes, unsynthesized
ELF header bytes, and rebuilt unwind placements. Emitting that result must
match `link`, expose the unwind index through the program headers, and execute
with the expected result. This stage boundary does not by itself combine the
class9/class11 PIE/TLS path or class12/class13 metadata path with this backend.

Inputs with no split or annotation sections therefore produce exactly
class8's bytes and map.

### Map

Rows are class8's rows. Storage rows use their placement. A live split row
shows its group's address, since its pieces now live there; an annotation row
shows the `.eh_frame` address if at least one of its FDEs was retained. Other
rows are DROP. After class8's lines, append one line per synthetic section:

```text
SYNTHETIC 0x0000000000402000 37 .rodata.str1.1
SYNTHETIC 0x0000000000402028 16 .rodata.cst8.8
SYNTHETIC 0x0000000000402038 92 .eh_frame
SYNTHETIC 0x0000000000402094 28 .eh_frame_hdr
```

Groups come first, in group order, named `.rodata.str{entry_size}.{alignment}`
for strings and `.rodata.cst{entry_size}.{alignment}` for constants; then
`.eh_frame` and `.eh_frame_hdr` if present. The `PHDR` slot is not listed.

## Acceptance and references

Native tests check: string and constant identity across translation units under
both `-fno-pie` and `-fpie` code generation and both input orders; interior
pointers through `.LC0+7`; merged sizes smaller than their inputs; removal of
dead merge sections and dead FDEs; a freestanding C program that walks its own
auxiliary vector to `AT_PHDR`, finds `PT_GNU_EH_FRAME`, binary-searches the
table, and checks that live functions are covered by FDEs whose CIEs decode;
the same fixture linked by GNU ld; byte-identical class8 output for inputs
without the new features; rejection of relocated merge sections, wide strings,
references past a piece, personality routines, and FDEs describing data; and
the CLI map.

- [ELF gABI: sections, `SHF_MERGE`/`SHF_STRINGS`](https://www.sco.com/developers/gabi/latest/ch4.sheader.html)
- [LSB Core: `.eh_frame` and `.eh_frame_hdr`](https://refspecs.linuxfoundation.org/LSB_5.0.0/LSB-Core-generic/LSB-Core-generic/ehframechpt.html)
- [DWARF 5, section 6.4: call frame information](https://dwarfstd.org/doc/DWARF5.pdf)
- [x86-64 psABI](https://gitlab.com/x86-psABIs/x86-64-ABI)

## Method map: rewrite pieces and unwind records

`split` turns a mergeable section into independently addressable pieces: NUL-terminated strings or fixed-width constants. `merge` groups compatible pieces, preserves first-use order, and returns a mapping from each original `(section, offset)` to its merged output offset. A reference into the middle of a piece must map to the corresponding interior offset, not merely to the piece start.

`frames` parses CIE/FDE records and associates relocations with exact record offsets. Framing errors are diagnosed before relocation errors; an FDE whose function was garbage-collected is omitted. `unwind_sizes` computes the search-table size only after ranges are validated and sorted. `emit_unwind` writes `.eh_frame`, `.eh_frame_hdr`, and the PT_GNU_EH_FRAME metadata consumed by the runtime unwinder.

## Snapshot boundary

This directory is a complete, independently buildable project. Earlier modules
contain reference implementations; this assignment owns `src/unwind.rs`.
The public interfaces stay stable so you can substitute your own earlier modules.

Run `cargo test --test prerequisites` from this directory to check the supplied
foundation independently of this assignment.

CIE LEB128 operands must fit their 64-bit unsigned or signed representation.
The tenth SLEB128 group must be `0x00` or `0x7f`; another payload or a continuation
bit is `Malformed` at the CIE offset. Nonminimal encodings that fit remain valid.
