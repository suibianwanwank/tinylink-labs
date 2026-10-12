# Class 4: Emit the First Executable

Implementation: [`src/executable.rs`](src/executable.rs).


Produce an ELF file that the Linux kernel can execute. Classes0–3 already define
how to read an object and select a symbol. Connect that path to a bounded layout
and an ELF writer. The resulting file must run directly: invoking the system
linker, wrapping another executable, or emitting a script is not an implementation.

## Scope and supplied code

Implement C4.1–C4.3 in [`src/executable.rs`](src/executable.rs). `src/main.rs` supplies command-line I/O and
executable permission handling. Do not spend assignment effort rebuilding those
facilities. Keep public interfaces, fixtures, and provided tests unchanged.

This stage supports one nonempty allocated code section, no relocation records,
and one static symbol table. It intentionally rejects data/BSS payloads and
unresolved patch sites instead of producing a plausible but wrong executable.
Class5 adds relocation; class6 expands memory layout. Nonallocated metadata can
be omitted. Even relocations for nonallocated debug sections are rejected here.
These are explicit milestone limits, not final-linker limitations.

## C4.1: Plan the image (25 points)

Use the constants and error order in the API contract. The ELF header occupies
64 bytes; two program headers occupy 112 bytes. Round their combined size up to
the requested code alignment. Padding is zero. The code is copied once, at that
file offset. File size is code offset plus code length; no trailing padding is
required. The maximum image size is 64 MiB, an explicit course resource limit.

`BASE=0x400000` is the virtual address corresponding to file offset zero.
`entry_address = BASE + code_offset + entry_offset`. Input symbol values remain
relative to their input section. Adding a virtual base is a layout decision, not
part of symbol resolution.

Alignment must be a power of two from 1 to 4096. Code must be nonempty, and entry
must identify a byte inside it. Reject one-past-end entries. Check arithmetic
before allocating or indexing. Overflow and excess size both return TooLarge.

## C4.2: Serialize ELF (35 points)

Use little-endian field writes into a zero-initialized image. Do not serialize a
Rust struct or depend on its padding. All unspecified fields remain zero.

ELF header:

| Offset | Width | Value |
|---|---|---|
| 0 | 16 | ELF magic, class=2, data=1, version=1; remaining identification bytes zero |
| 16 | 2 | ET_EXEC=2 |
| 18 | 2 | EM_X86_64=62 |
| 20 | 4 | version=1 |
| 24 | 8 | planned entry virtual address |
| 32 | 8 | program-header offset=64 |
| 40 | 8 | section-header offset=0 |
| 48 | 4 | flags=0 |
| 52 | 2 | ELF-header size=64 |
| 54 | 2 | program-header entry size=56 |
| 56 | 2 | program-header count=2 |
| 58 / 60 / 62 | 2 each | section entry size/count/name index=0 |

Each program header has fields type/u32 at +0, flags/u32 at +4, then u64 fields
file offset, virtual address, physical address, file size, memory size, alignment
at +8, +16, +24, +32, +40, +48 respectively.

- Entry 0, offset 64: PT_LOAD=1, flags=5 (read/execute), file offset=0,
  virtual and physical address=BASE, file and memory size=planned file size,
  alignment=4096. It maps headers, padding, and code in one RX segment.
- Entry 1, offset 120: PT_GNU_STACK=0x6474e551, flags=6 (read/write),
  alignment=16; other fields zero. It requests a non-executable stack.

There is no section table. Execution uses the program headers; an executable does
not need an output section table for this milestone. Do not feed this ET_EXEC
back into class0, whose contract specifically accepts ET_REL input objects.

## C4.3: Connect the pipeline (40 points)

1. Parse named sections through class1. Select nonempty SHF_ALLOC entries.
   Exactly one must exist, with type PROGBITS=1 and flags exactly 6.
2. Reject every nonempty REL or RELA section. Empty tables are harmless here.
3. Require exactly one SYMTAB=2; parse it through class2 and pass the table to
   class4::resolve::build_globals. Missing or multiple tables return SymbolTableCount.
4. Look up `entry_name`. No selected definition gives MissingEntry. A COMMON,
   ABS, or wrong-section entry gives InvalidEntry. A section-relative value at
   or beyond the code size also gives InvalidEntry.
5. Pass the code payload, alignment (normalize 0 to 1), and entry offset to your
   executable writer. Do not search specifically for a section named `.text`.

Class1/2/3 errors propagate through the corresponding wrapper variants. The API
comment specifies validation order so malformed inputs have stable diagnostics.

## Completion

Run on native x86-64 Linux with GNU binutils available:

```sh
cargo test -p class4
python3 scripts/grade.py class4
cargo run -p class4 -- input.o program
./program
```

The command example assumes an input satisfying the contract, not an arbitrary
C program with a standard-library startup path. The test suite assembles its own
syscall-only inputs. It checks field values, alignment boundaries, nonzero entry
offsets, `readelf` acceptance, three exit statuses, CLI execution, and rejected
unsupported inputs. Each generated process is killed after three seconds if it
fails to terminate; a crash or timeout is a failure, not an expected answer.

In your design notes distinguish a section's input file offset, the output code
offset, and its virtual address. Explain why program-header congruence is met by
mapping file offset zero at a page-aligned BASE.

## References

[System V ABI: ELF Header](https://gabi.xinuos.com/elf/02-eheader.html) and
[Program Loading](https://gabi.xinuos.com/elf/07-pheader.html). The fixed base,
resource bound, supported flags, and GNU stack descriptor are explicit course
and Linux choices, not universal properties of all ELF executables.

## Layout validation priority

Alignment is checked before entry validity, and entry validity before image-size arithmetic. With invalid alignment and an entry outside the code, return InvalidAlignment. With valid alignment, an invalid entry and excessive code length, return InvalidEntry. This order is the course API policy; it prevents resource-size calculations from hiding a more immediate contract violation.

## Method map: plan bytes, serialize bytes, link one object

`layout(code_len, alignment, entry_offset)` is pure arithmetic. It decides where code begins after the ELF header and two program headers, rounds that position up to the requested alignment, checks the entry is inside code, and enforces `MAX_IMAGE`. It does not allocate or write bytes. `BASE` is the virtual address used by this teaching backend; `code_offset` is a file offset, so they must not be confused.

`executable(code, alignment, entry_offset)` materializes the plan. It writes an ELF64 ET_EXEC header, two program headers, zero padding, and the code bytes. The first PT_LOAD deliberately covers the complete file in this stage, while PT_GNU_STACK describes a non-executable stack. Reserved bytes are zero so output is deterministic.

`link_single` is orchestration: parse sections, enforce this class’s narrow input policy, resolve the entry symbol, call `layout`, then call `executable`. The order matters because an unsupported input feature should not be hidden by a later layout failure.

## Snapshot boundary

This directory is a complete, independently buildable project. Earlier modules
contain reference implementations; this assignment owns `src/executable.rs`.
The public interfaces stay stable so you can substitute your own earlier modules.

Run `cargo test --test prerequisites` from this directory to check the supplied
foundation independently of this assignment.
