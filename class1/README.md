# Class 1: Sections and String Tables

Implementation: [`src/sections.rs`](src/sections.rs).


Turn the ELF boundary from class0 into named sections. The result will feed
symbol parsing and executable layout. Complete C1.1–C1.4 in [`src/sections.rs`](src/sections.rs); your
class0 implementation is a real dependency, not replaced by an instructor copy.

## Data model

`Section` retains file metadata. `offset` is a file offset; `size` may describe
memory rather than file bytes for NOBITS. Neither field is an assigned output
address. Names borrow bytes from the input and need not be valid UTF-8.

The class0 header selects 64-byte entries. Decode these little-endian fields:

| Offset within entry | Width | Field |
|---|---|---|
| 0 | 4 | name_offset |
| 4 | 4 | kind |
| 8 | 8 | flags |
| 24 | 8 | offset |
| 32 | 8 | size |
| 40 | 4 | link |
| 44 | 4 | info |
| 48 | 8 | alignment |
| 56 | 8 | entry_size |

The input `sh_addr` field at offset 16 is not an output address and is unused in
this stage. Unknown section kinds remain opaque metadata, not parsing errors.

## Tasks

- **C1.1, 25 points:** Decode `section`. Validate zero/power-of-two alignment and
  divisibility by a nonzero entry size. Propagate class0 errors as `Error::Input`.
- **C1.2, 20 points:** Implement `payload`. SHT_NULL (0) and SHT_NOBITS (8) return
  empty slices without dereferencing their offsets. All others use checked ranges.
- **C1.3, 20 points:** Implement `string_at`. An offset can point into a suffix of
  another name; NUL termination must occur within the table. Do not decode UTF-8.
- **C1.4, 35 points:** Implement `sections` with the ordering in its API comment.
  Decode headers, validate the selected name table, then validate each payload and
  resolve its name. Names and section entries remain in original table order.

The name table must be SHT_STRTAB (3), nonempty, and start/end with NUL. An invalid
name-table type or missing boundary NUL produces InvalidNameTable. File-range
errors remain Input errors. A name offset at or beyond the end of its string table produces BadStringOffset.

Public interfaces and provided tests are fixed. Private helpers and additional
student tests are allowed. No external ELF parser may implement these tasks.

## Completion

```sh
cargo test -p class0 -p class1
cargo test -p class0 -p class1 --release
```

Complete prerequisite tests as well as current tests. The starter intentionally
fails. Keep logical record validation separate from byte-range validation: a
valid range alone does not prove that the bytes describe the required table.

This stage stops at section metadata. It does not yet parse symbol or relocation
records, choose output addresses, or generate an executable.

For grouped local feedback, run `python3 scripts/grade.py class1` from the repository root.

## Trace bytes before interpreting them

![ELF64 regions and complete section-header layout](../docs/figures/elf64-regions-en.svg)

The file header selects the table. Table entry i begins at `e_shoff + i * 64`.
Inside that entry, `sh_offset` selects payload bytes from the **file start**;
`sh_name` selects name bytes from the **name-table start**. Neither is relative
to the beginning of this entry. Every entry has the same field layout. The
names `.text` and `.shstrtab` are resolved through strings, not stored inline
in the 64-byte entry. NULL and NOBITS entries do not have stored payloads.

## Error precedence is part of the API

`section` checks the complete entry range, then alignment, then entry size.
`sections` first decodes every entry in table order. Only after all headers are
valid does it check the name-table type, its file range, and its first/last NUL.
Finally it walks entries in order, checking each payload before resolving that
entry's name. Thus a late invalid alignment wins over an early invalid name;
an early invalid name wins over a later invalid payload. Do not replace this
with “validate all payloads, then all names.” These are course diagnostic rules,
not a required diagnostic order in the ELF specification.

Unknown section kinds and flag bits are preserved. `sh_link` and `sh_info` are
also preserved at this stage: their meaning depends on section type and belongs
to a later decoder. `sh_addr` is not used for input ET_REL layout. Preserving
metadata does not imply that later linking stages support that feature.

## Method map: descriptor, payload, name, object

`section` decodes one 64-byte descriptor. It validates metadata rather than
accessing payload: alignment must be zero or a power of two, and a nonzero
`sh_entsize` must divide `sh_size`. The ELF field at entry offset +16,
`sh_addr`, is not retained in this API's `Section` value and is not used to
assign output addresses.

`payload` turns the descriptor’s file coordinates into bytes. SHT_NULL and SHT_NOBITS intentionally return an empty slice: NOBITS reserves memory but has no stored file bytes, so reading `input[sh_offset..sh_offset+sh_size]` would be wrong even when the offset is `u64::MAX`. Every other kind goes through class0’s checked range.

`string_at` resolves a byte offset inside a string table. Names are byte sequences, not UTF-8 strings. The offset may point into the middle of a suffix, but the terminating NUL must occur inside the table. An offset outside the table and a missing NUL are different errors because they describe different corruptions.

`sections` is intentionally staged: decode every descriptor in table order; select and validate the name-table descriptor; validate its boundary NULs; then for each section validate its payload before resolving its name. This order prevents a name lookup from hiding a payload-range error and makes diagnostics deterministic.

### The 64-byte distinction

The ELF header’s `e_shoff` points to the first descriptor. Entry `i` starts at `e_shoff + i * e_shentsize`; `sh_offset` inside that entry points somewhere else—the described section’s contents. A section header table is a directory of fixed-size records, not a section containing all `.text` bytes.

## Snapshot boundary

This directory is a complete, independently buildable project. Earlier modules
contain reference implementations; this assignment owns `src/sections.rs`.
The public interfaces stay stable so you can substitute your own earlier modules.

Run `cargo test --test prerequisites` from this directory to check the supplied
foundation independently of this assignment.
