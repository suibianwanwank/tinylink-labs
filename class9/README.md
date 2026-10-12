# Class 9: Link before the load address is known

Implementation: [`src/pie.rs`](src/pie.rs).


An ET_EXEC pointer contains a fixed virtual address. A PIE can be mapped at a
different base on every execution. A call displacement within the image survives
that change; an address stored in a data pointer does not. This project makes
that distinction explicit and builds an executable that repairs its pointers
before entering user code.

## Deliverables

Implement C9.1–C9.4 in [`src/pie.rs`](src/pie.rs), with private helpers as needed. Public APIs,
provided startup assembly, build support, CLI, and tests are not student edit
areas. Class8 exposes an immutable `Prepared` link plan: reuse it to preserve
archive selection, symbol precedence, GC, roots, and input validation. Do not
reparse a diagnostic map or invoke another linker.

| Task | Responsibility | Points |
|---|---|---|
| C9.1 | Static fixups versus load-time relative pointers | 30 |
| C9.2 | Canonical ELF64 RELA encoding | 15 |
| C9.3 | Startup descriptor and ET_DYN emission | 25 |
| C9.4 | Cumulative native static-PIE integration | 30 |

Run `python3 scripts/grade.py class9` on native x86-64 Linux. This class additionally
needs GNU `objcopy`; the provided Cargo build script assembles `startup.S` and
extracts its self-contained `.text` bytes. It neither links student inputs nor
supplies the missing algorithms. Cross-target builds are rejected. No musl,
libc, dynamic loader, or compiler runtime is required for the fixtures.

## Coordinate model

An RVA is an address relative to the image's first load segment, whose virtual
address is zero. If Linux chooses load bias B, image address V becomes B+V.
An ABS symbol remains an integer independent of B. Unresolved WEAK is ABS zero.
Section and COMMON definitions are image-relative, even when their numerical RVA
happens to be zero. Do not infer the category from the integer value.

Class6 still plans addresses relative to its fixed BASE. This backend subtracts
BASE when constructing an RVA. It does not change earlier classes' contracts.
Do not subtract BASE from an ABS symbol or add it to a runtime relocation addend.

## C9.1: fixup contract

`Fixup.offset` indexes the supplied contribution byte slice. `place_rva` is the
RVA of that exact patch site. The caller guarantees their relationship to layout.
`writable` means the destination is initialized Data, not merely file-backed.
Each error leaves the supplied bytes unchanged.

| Relocation | Symbol category | Action |
|---|---|---|
| NONE (0) | Any | No write or record; ignore every other field |
| 64 (1) | ImageRva | Zero eight bytes; return a RELATIVE record; require writable |
| 64 (1) | Absolute | Static S+A, modulo 2^64 |
| PC32 (2), PLT32 (4) | ImageRva | Static S+A-P with class5 signed-range checking |
| PC32 (2), PLT32 (4) | Absolute | NonPicReference: the unknown bias does not cancel |
| 32 (10), 32S (11) | Absolute | Static class5 width and range checks |
| 32 (10), 32S (11) | ImageRva | NonPicReference: a narrow runtime address is not supported |
| Any other kind | Any | Class5 unsupported-relocation error |

A runtime record uses destination `place_rva` and addend equal to image symbol
RVA plus the input signed addend, modulo 2^64 and stored as an i64 bit pattern.
Startup writes B+A modulo 2^64. There is no in-place addend. Runtime writes to
Text or ReadOnly are TextRelocation errors; do not temporarily make code writable.
If a fixup violates multiple rules, error priority is unspecified except NONE's
unconditional no-op. Bounds checking must not panic or write partially.

### Reading a fixup result

Take a standalone contribution slice whose RVA starts at `0xffd`. Its byte index
3 corresponds to image RVA `0x1000`: these are two coordinates for the same
field. With kind64, `ImageRva(0x3000)`, addend -8, and a writable destination,
`patch` clears slice bytes `[3,11)` and returns:

```text
Relative { offset: 0x1000, addend: 0x2ff8 }
```

`offset` in that record denotes the destination, while `addend` denotes the
target value before adding the load bias. If B is `0x70000000`, startup writes
`0x70002ff8` at `0x70001000`, as bytes `f8 2f 00 70 00 00 00 00`. The slice index
3 is not stored in the record. This is a patch-unit example; final emission must
still verify that the destination is inside initialized Data.

Changing the target to `Absolute(0x3000)` keeps the arithmetic result `0x2ff8`
independent of B: write it statically and return no runtime record. For a PC32
image target at `0x1100`, field RVA `0x1000`, and addend -4, both addresses move
with B, leaving displacement `0xfc` and bytes `fc 00 00 00`. The contract tests
check these categories separately so a coincidentally correct integer cannot
stand in for the required interpretation.

## C9.2: relative relocation table

Sort by destination RVA. Each record is 24 little-endian bytes: r_offset (u64),
r_info (u64, symbol zero and type R_X86_64_RELATIVE=8), r_addend (i64). Reject
any pair whose eight-byte destination intervals overlap, including duplicates.
Reject addition overflow and ends beyond class9::executable::MAX_IMAGE. Adjacent destinations
and unaligned destinations are valid. Addends may have any i64 value. Input order
must not change output bytes. Empty input produces an empty table.

## C9.3: image and startup

The input Image is produced by class9::layout::plan. Only real contribution payload bytes
may have been patched; metadata and byte length retain the planner's invariants.
This is an internal lowering API, not a parser for arbitrary caller-created Images.
Two reserved identities cannot collide with actual selected-object ordinals:

| Identity | Kind | Alignment | Exact memory/file size |
|---|---|---|---|
| BOOT=(usize::MAX,0) | Text | 16 | 160 |
| RELA=(usize::MAX,1) | ReadOnly | 8 | 24 * record count |

Validate their presence, kind, file backing, address alignment, and exact sizes.
Every relative destination must fit entirely inside one initialized Data placement,
not a load-segment gap, BSS, or the reserved contributions. This storage check is
additional to C9.2's numeric checks. The user entry RVA must identify an actual
byte in a non-BOOT Text contribution; padding and one-past-the-end are invalid.

Copy STARTUP into BOOT, copy encoded records into RELA, then fill four u64 fields
at BOOT byte offsets 128, 136, 144, and 152: BOOT RVA, RELA RVA, record count, and
user entry RVA. The assembly derives B using its RIP-relative anchor and BOOT RVA,
applies the records, clears rdx, and jumps to the user entry without changing rsp.
The descriptor and relocation table are read-only at runtime.

Use class9::layout::emit with BOOT's fixed-base address, then change e_type to ET_DYN (3),
e_entry to BOOT RVA, and the three PT_LOAD virtual/physical addresses to RVAs.
File offsets, file/memory sizes, alignments, segment flags, and GNU_STACK remain
unchanged. No PT_INTERP or PT_DYNAMIC is emitted. This is a freestanding,
self-relocating static PIE; it is not a dlopen-able shared library. The kernel
maps segments but does not apply these relocations. Our supplied startup locates
them through its descriptor, not dynamic tags. A section header table, dynamic
symbol lookup, RELRO, IFUNC, and general runtime-loader compatibility are outside
this stage. Later assignments must not silently claim those features.

## C9.4: cumulative lowering

Obtain `Prepared` from class8. Count live R64 references whose selected target is
Section or COMMON; this is the exact number of runtime records. Reserve the two
synthetic contributions after the real contributions, retaining input order.
Check count multiplication and allocation bounds before creating the table buffer.
Use class6 layout, then classify each bound reference and pass its source slice
to `patch`. Never patch across contribution boundaries. NONE does not require
resolving a symbol or obtaining file-backed storage. Other live BSS source
relocations are rejected as in earlier classes. Allocate only live COMMONs.

`Error::Fixup` retains source section identity, offset, kind, and PatchError, so
CLI failures identify the offending relocation. Structural and resolution errors
retain the class8/class6 error context. `emit` validates final record destinations
and emits the image. Do not ignore an emit error or fall back to ET_EXEC.

The map uses the class8 format, except its header is `TINYLINK PIE MAP 1` and
addresses are RVAs. It lists real user sections and live COMMONs; synthetic BOOT
and RELA contributions are omitted. Map coordinates must remain deterministic
across runs even though runtime addresses vary.

## Acceptance and reference material

The release gate includes structural ELF checks, runtime pointer initialization,
PC-relative calls, BSS and COMMON, negative and full-width addends, explicit ABS
and unresolved WEAK, unchanged initial stack, GC/archive integration, and failures
for non-PIC or read-only runtime patches. An ASLR fixture reports its runtime
entry address across multiple executions; merely changing e_type does not pass.
If Linux ASLR is disabled, the ASLR check fails explicitly rather than silently
skipping the central acceptance condition.

The [x86-64 psABI](https://gitlab.com/x86-psABIs/x86-64-ABI/-/blob/master/x86-64-ABI/object-files.tex)
defines relocation encodings and arithmetic. The
[Linux ELF loader](https://github.com/torvalds/linux/blob/master/fs/binfmt_elf.c)
shows the ET_DYN loading paths. The table and descriptor above are the precise
course contract; the descriptor is course support infrastructure, not an ELF ABI
extension that other loaders are expected to understand.

## Do not invent unspecified precedence

This API does not specify which competing error wins. The reference implementation’s incidental order is not an extra requirement. NONE is an unconditional no-op, and every failure preserves all destination bytes. Multi-defect tests assert rejection and byte preservation without requiring a particular error variant.

Contract coverage: `competing_failures_preserve_bytes_without_fixing_priority`.

## Method map: static values versus load-time values

`patch` handles relocations whose result is known before the loader runs and emits `Relative` records for writable image pointers whose final address depends on load bias. `Value::ImageRva` is relative to the image; `Value::Absolute` is already an integer. Never infer the category from whether the numeric value happens to be zero.

`encode_relative` sorts records by destination RVA and writes canonical 24-byte ELF64 RELA entries (`r_offset`, `r_info`, signed `r_addend`). It rejects overlapping destinations and image-range overflow before producing bytes. `emit` reserves startup code, the relative table, and descriptors before serializing the ET_DYN image.

This class deliberately leaves competing error priority unspecified for `patch`; its strong guarantees are NONE as a no-op, no partial writes, and no panic on bad bounds.

## Snapshot boundary

This directory is a complete, independently buildable project. Earlier modules
contain reference implementations; this assignment owns `src/pie.rs`.
The public interfaces stay stable so you can substitute your own earlier modules.

Run `cargo test --test prerequisites` from this directory to check the supplied
foundation independently of this assignment.
