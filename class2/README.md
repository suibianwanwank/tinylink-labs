# Class 2: Symbols and RELA Records

Implementation: [`src/symbols.rs`](src/symbols.rs).


Sections describe where bytes live in an input file. Symbols attach names and
meaning to locations; relocation records identify references that cannot be fixed
until layout. Implement the two entry points in [`src/symbols.rs`](src/symbols.rs). The output is still
an input model, not a linked program.

## Prerequisites and allowed changes

Read the preceding assignments; their completed modules are supplied here. Keep their checked accessors in the data path; do not
reimplement an unchecked parser here. Modify the implementation linked above and add
private helpers or student tests. Preserve public interfaces, dependencies, and
provided tests. External object-file libraries are not allowed.

## C2.1: Symbol tables (60 points)

`symbols(input, sections, index)` accepts an index into the current input's section
list. It must identify SHT_SYMTAB, not locate a table by the name `.symtab`.
`sh_link` identifies its associated string table. Record indices belong to the selected
table, not to a global name space. The API selects the table explicitly and does
not assume that symbol-table sections have fixed indices across input files.

Each Elf64_Sym occupies 24 bytes:

| Offset | Width | Meaning |
|---|---|---|
| 0 | 4 | st_name, offset in the linked string table |
| 4 | 1 | st_info: binding in high nibble, type in low nibble |
| 5 | 1 | st_other: visibility in low two bits |
| 6 | 2 | st_shndx |
| 8 | 8 | st_value |
| 16 | 8 | st_size |

All integers are little-endian. Preserve values: for a section definition, value
is a section-relative position; for COMMON it is an alignment requirement; for
ABS it is an absolute value. Resolving those meanings belongs to later classes.
Unknown binding/type values remain representable for later policy decisions.
Names are byte slices, not required to be UTF-8.

Validate in this order: selected section index/type, entry size and divisibility,
linked string-table index/type, payload bounds, string-table NUL boundaries,
nonempty symbol table and all-zero entry zero, sh_info range, then records in
order. Within each record validate local ordering, section index, then name.
A table with no symbols has InvalidNullSymbol. For entry-zero checks inspect all
24 bytes, not just selected decoded fields.

`sh_info` is the first non-local index. Entries before it must be STB_LOCAL;
entries at/after it must not be STB_LOCAL. A table of local-only symbols can have
sh_info equal to its count. Ordinary section indices are below 0xff00 and within
the supplied section list. UNDEF, ABS, and COMMON have explicit enum variants;
all other reserved indices are unsupported at this stage.

## C2.2: Relocation tables (40 points)

`relocations` selects SHT_RELA by index. `sh_link` owns the symbol indices;
`sh_info` owns the target offsets. Neither is an index into the relocation table.
Validate the selected index/type, entry width and divisibility, nonzero target
index/range, linked symbol table via `symbols`, and finally relocation payload.

Each Elf64_Rela occupies 24 bytes:

| Offset | Width | Meaning |
|---|---|---|
| 0 | 8 | r_offset, offset within the target section |
| 8 | 8 | r_info: high 32 bits symbol index, low 32 bits relocation kind |
| 16 | 8 | r_addend, signed two's-complement i64 |

Validate every symbol index. Preserve unknown relocation kinds and offsets:
application will determine the write width and check target bounds. An empty RELA
table is valid if its metadata and linked symbol table are valid. SHT_REL is
explicitly rejected with UnsupportedRel; do not guess implicit addends.

Index errors return InvalidIndex; type errors return WrongSectionType. A linked
string table with missing boundary NUL returns InvalidStringTable. Errors from
class1 propagate as Input. Neither malformed data nor arithmetic should panic.

## Completion

```sh
cargo test -p class0 -p class1 -p class2
cargo test -p class0 -p class1 -p class2 --release
python3 scripts/grade.py class2
```

The synthetic records establish exact field/ownership contracts. The native test
assembles an unresolved call and checks the table selected by that relocation's
own link, rather than assuming a fixed index produced by a particular assembler.

Record in your design notes why a symbol index cannot be used across two object
files, and why converting a negative addend to an unsigned address too early
loses information. These notes accompany the implementation, not separate tasks.

## Establish ownership before interpreting records

Validate a symbol table’s entry size and linked string table before decoding records. Within a record, check local ordering, then section index, then name. A relocation table must validate its target and symbol-table ownership even when it contains no records. sh_link is a section index; the high 32 bits of r_info are a symbol index inside that selected table.

Contract coverage: `symbol_validation_priority`, `symbol_record_validation_priority`, `relocation_validation_priority`.

## Method map: symbols and relocation records

`symbols(input, sections, index)` reads one SHT_SYMTAB selected by section index. First establish ownership: the selected section must be a symbol table, its entry size must be 24, and `sh_link` must identify an SHT_STRTAB. Only then decode records. The all-zero first record is a structural ELF rule, while `sh_info` defines the local/non-local boundary; both must be checked before interpreting names.

Each `Elf64_Sym` is 24 bytes because the fields have mixed widths: `st_name` is a 32-bit string offset, `st_info` and `st_other` are bytes, `st_shndx` is 16 bits, and `st_value`/`st_size` are 64 bits. Preserve the full values. For COMMON, `st_value` is an alignment requirement; treating it as an address loses information needed by class3.

`relocations(input, sections, index)` reads SHT_RELA records. `sh_link` names the symbol table and `sh_info` names the target section; neither is a relocation-record index. Each record is 24 bytes, and `r_info` packs a 32-bit symbol index in its high half and a 32-bit relocation kind in its low half. Keep the signed 64-bit addend exactly; later stages decide whether it fits the selected relocation width.

### Implementation path

Validate ownership and record geometry first, then linked tables and payload bounds, then record-local fields. An empty RELA table is valid only after its metadata and linked symbol table are valid. SHT_REL is rejected because it has an implicit addend and would require a different contract.

## Snapshot boundary

This directory is a complete, independently buildable project. Earlier modules
contain reference implementations; this assignment owns `src/symbols.rs`.
The public interfaces stay stable so you can substitute your own earlier modules.

Run `cargo test --test prerequisites` from this directory to check the supplied
foundation independently of this assignment.
