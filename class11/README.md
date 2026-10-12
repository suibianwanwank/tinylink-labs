# Class 11: Indirection that the linker can see through

Implementation: [`src/tls.rs`](src/tls.rs).


Compilers emit position-independent code without knowing where a symbol will
end up. Two mechanisms carry that uncertainty to the linker.

The **Global Offset Table** (GOT) is an array of addresses. Code compiled with
`-fPIC` loads the address of external data from a GOT slot
(`mov x@GOTPCREL(%rip), %rax`) because, in a shared library, `x` might live in
another module. A static PIE resolves its ordinary definitions within the image; ABS symbols
and unresolved weak references remain constants. For image definitions it can
either fill a slot with the final address or, better, rewrite the load into
`lea x(%rip), %rax` and drop the slot. That rewrite is *relaxation*: the
assembler marks eligible instructions with `R_X86_64_GOTPCRELX` or
`R_X86_64_REX_GOTPCRELX` so the linker knows which bytes it may change.

**Thread-local storage** (TLS) gives every thread its own copy of each
`__thread` variable. In the GNU x86-64 Linux convention, the `%fs` base is the
thread pointer. It points at a Thread Control Block whose first word stores
that pointer itself; each thread's TLS block
sits immediately below it ("variant II"). A variable is reached at a constant
negative offset from `%fs`, the *TP offset*. This stage supports two models:
local-exec, where the offset is a link-time constant in the instruction
(`R_X86_64_TPOFF32`), and initial-exec, where it is loaded from a GOT slot
(`R_X86_64_GOTTPOFF`). The linker lays out the TLS *template* that every block
copies, and publishes it in a `PT_TLS` program header.

The kernel does not create the main thread's TLS block; libc normally does.
This course has no libc, so the supplied `tls_startup.S` does it, running after
class9's relocation stage.

## Deliverables

Implement C11.1–C11.5 in [`src/tls.rs`](src/tls.rs), with private helpers as needed. Public
APIs, the supplied `policy`, `Got` accessors, startup assembly, build script,
CLI, and tests are not student edit areas. Reuse `class11::gc::prepare_with`, class6
planning, and class9's `patch`, `encode_relative`, and `emit`.

| Task | Responsibility | Points |
|---|---|---|
| C11.1 | TLS template layout and TP offsets | 15 |
| C11.2 | Choose how each relocation is lowered | 15 |
| C11.3 | GOT slot assignment and encoding | 15 |
| C11.4 | TLS startup descriptor and `PT_TLS` | 20 |
| C11.5 | Cumulative static-PIE integration | 35 |

Run `python3 scripts/grade.py class11` on native x86-64 Linux. The build script
needs GNU `as` and `objcopy`, as in class9. The native tests compile
freestanding C with `-fpie` and `-fPIC`; one starts a second thread with the raw
`clone` system call.

## Section policy

`policy` is supplied: PROGBITS (`.tdata`) or NOBITS (`.tbss`) with flags exactly
WRITE|ALLOC|TLS (`TLS_FLAGS`, 0x403) is `Role::Split`. Read-only mergeable
inputs use class10's split policy; all other inputs follow class6 storage.
TLS and mergeable sections are graph nodes, so class8 GC applies to them.

## C11.1: TLS template

`tls_layout` validates every section first: flags exactly `TLS_FLAGS`; type 1
with `bytes.len() == size`, or type 8 with no bytes; alignment (0 means 1) a
power of two no greater than 4096. A violation is `InvalidTls(id)`. A repeated
identity is `Output(DuplicateIdentity)`.

Place every PROGBITS section in supplied order, then every NOBITS section in
supplied order, each at the next multiple of its own alignment. Initialized
bytes and the zero padding between them form `image`; NOBITS sections only
extend `memory_size`. Initialized data must come first because a thread block
is created by copying `image` and zero-filling the rest.

`alignment` is the largest section alignment, but at least 8 so the TCB word is
aligned. `thread_pointer` is `memory_size` rounded up to `alignment`; TP offsets
are therefore `template offset - thread_pointer`. Any template position or
`thread_pointer + 8` beyond `class11::executable::MAX_IMAGE` is `TooLarge`. `tpoff` returns
`None` for a section outside the template.

### Worked template: three different sizes

For the inputs in `tls_layout_orders_initialized_before_zero_fill`, the first
input is NOBITS, but initialized inputs are placed first. All numbers below are
bytes; section IDs identify the original file and section.

| ID | Type | Size | Alignment | Template interval |
|---|---|---|---|---|
| (0,4) | PROGBITS | 4 | 4 | [0,4) |
| (0,5) | PROGBITS | 3 | 0, normalized to 1 | [4,7) |
| (0,3) | NOBITS | 8 | 8 | [8,16) |
| (1,2) | NOBITS | 4 | 16 | [16,20) |

`image.len()` is 7; `memory_size` is 20. The one-byte gap at [7,8) is memory-only
zero padding. Maximum alignment 16 places TP at template offset 32, leaving
[20,32) as another zero-filled gap before the TCB word. Thus `TLS_AREA` occupies
40 bytes including [32,40), whose first word startup sets to TP itself.
`PT_TLS.p_filesz` is 7 and `p_memsz` is 20: its description excludes the final
alignment gap and TCB word, which belong to the constructed thread block.

The first initialized byte has TP offset `0 - 32 = -32`. Byte 3 of section (1,2)
has template position `16 + 3 = 19`, hence TP offset `19 - 32 = -13`. Both
threads use these same signed offsets against their own different TP addresses.
The layout contract checks image length, memory size, TP position, and the signed
offsets. Emission tests separately check the area reservation and PT_TLS fields.
The second-thread native test checks that a runtime can construct another block
from the PT_TLS description rather than copying the main thread's state.

## C11.2: lowering

`lower` receives the bytes of the section containing the relocation, the field
offset, the relocation kind, and what the symbol is (`Image`, `Absolute`, or
`Tls`). It never changes the bytes.

| Kind | Image | Absolute | Tls |
|---|---|---|---|
| NONE (0) | Fixup | Fixup | Fixup |
| TPOFF32 (23) | TlsMismatch | TlsMismatch | ThreadOffset |
| GOTTPOFF (22) | TlsMismatch | TlsMismatch | Got |
| DTPMOD64, DTPOFF64, TPOFF64, TLSGD, TLSLD, DTPOFF32 (16–21), TLSDESC (34–36) | UnsupportedTls | UnsupportedTls | UnsupportedTls |
| GOTPCREL (9) | Got | Got | TlsMismatch |
| GOTPCRELX (41), REX_GOTPCRELX (42) | Relax if `mov`, else Got | Got | TlsMismatch |
| any other kind | Fixup | Fixup | TlsMismatch |

Rows are checked in this order. "Relax if `mov`" means: `offset >= 2`, the
field lies inside `code`, and `code[offset - 2]` is `0x8b`, the `mov r, r/m`
opcode that precedes the ModRM byte. Changing it to `0x8d` turns the load into
`lea` with the same operands; the field then holds `S + A - P` for the symbol
itself. Other instructions (`call *`, `jmp *`, arithmetic) are never rewritten,
and plain GOTPCREL is never relaxed because the assembler did not promise that
the instruction may be rewritten. An absolute target cannot be relaxed either:
`lea` would produce a PC-relative address, not the constant. General- and
local-dynamic TLS require a different resolution sequence or relaxation; this
backend implements neither. TLSDESC has a separate descriptor/call protocol and
is also rejected. The supplied fixtures use explicit options such as `-fpie`
to obtain the supported models; check the generated relocation records.

## C11.3: the GOT

`Got::offset` returns `8 * index` of a slot, appending the slot on first use, so
equal `Slot` values share one entry and slot order is first-use order. A table
that would exceed `class11::executable::MAX_IMAGE` is `TooLarge`.

A slot names its value independently of layout, because the GOT is sized before
addresses exist. `Section(id, offset)` and `Common(name)` are image addresses;
`Constant` holds an ABS value, zero for an unresolved WEAK, or a TP offset as a
two's-complement u64.

`Got::encode` requires a file-backed Data `GOT` placement of exactly
`Got::size()` bytes (`InvalidReservation`), unless the table is empty, in which
case it returns two empty vectors. Constants are stored little-endian. An image
slot is stored as zero and produces `Relative { offset: slot RVA, addend:
target RVA }`, in slot order; class9's startup writes the load-time address. A
section slot whose placement is missing or whose offset exceeds its size, or an
unknown COMMON, is `Output(InvalidSymbol)`.

## C11.4: startup and `PT_TLS`

Without TLS, `emit` is exactly `class11::pie::emit`. With TLS, the image must contain:

| Identity | Kind | Alignment | Size |
|---|---|---|---|
| `PHDR` | Text, at file offset `HEADER_BYTES` and address `BASE + HEADER_BYTES` | 8 | 56 |
| `TLS_IMAGE` | Data, file-backed | `tls.alignment` | `tls.image.len()` |
| `TLS_AREA` | Bss | `tls.alignment` | `thread_pointer + 8` |
| `TLS_BOOT` | Text, file-backed | 16 | `TLS_STARTUP_SIZE` (176) |

Anything else is `InvalidReservation`. The user entry must be a byte of a Text
placement other than `class11::pie::BOOT`, `TLS_BOOT`, and `PHDR`, else
`Pie(InvalidEntry)`.

Copy `TLS_STARTUP` into `TLS_BOOT` and fill its descriptor, six u64 fields at
byte 128: `TLS_BOOT` RVA, `TLS_IMAGE` RVA, `TLS_AREA` RVA, image size,
`thread_pointer`, user entry RVA. Then call `class11::pie::emit` with `TLS_BOOT` as
its entry, so execution runs class9's relocation stage, then the TLS stage,
then user code. Finally set `e_phnum` to 5 and write `PT_TLS` into the `PHDR`
slot: flags R, offset and addresses of `TLS_IMAGE` (addresses are RVAs),
`p_filesz` the image size, `p_memsz` the template's memory size, `p_align` its
alignment.

The startup copies the image, which already holds load-time addresses, into the
area, stores the thread pointer into its own first word, and calls
`arch_prctl(ARCH_SET_FS, tp)`. If that fails, the process exits with status
127. It preserves the initial stack pointer and clears `%rdx`.

## C11.5: integration

`lower_with` is the task. Supplied `lower_prepared` enables preservation;
`lower_with_unwind` selects an explicit unwind policy and empty folding/order
inputs. The supplied `link_with` prepares inputs with
`class11::gc::prepare_with`, passes that validated plan to `lower_prepared`, and
consumes its result through `Lowered::emit`. The supplied `link` chooses
`policy`; a later driver can choose another classifier, which must keep TLS
sections as `Role::Split`. Mergeable inputs may also be Split.

The stages have separate results: `Prepared` identifies selected definitions
and live input sections; `Lowered` adds placements, patched bytes, the TLS
template, complete class10 `Merged` piece mapping, load-time relocation
records, and the entry RVA. ELF headers and
startup descriptors are serialized only by `Lowered::emit`. Metadata remains
accessible through the same `Prepared`, so a later stage does not have to parse
an already-emitted file to recover the input identities. Supported `.eh_frame` annotations are retained and rebuilt in the same layout.

The later ICF stage supplies `aliases` and `order`. An alias must map a live
Text input directly to a different live Text representative, which must not
itself be aliased. Reject a self-alias, chain, cycle, missing input, or non-Text
endpoint as `InvalidAlias { from, to }`, in BTreeMap order. Establish this shape
before layout. The caller is responsible for instruction/reference equivalence,
address significance and compatible unwind descriptions; these checks cannot
be inferred from the alias map alone.

Validate each order entry against live Text inputs in supplied order, returning
`Merge(class11::unwind::Error::InvalidOrderSection(id))` on the first invalid entry.
Replace aliases by representatives and ignore duplicate requests stably.
Requested retained Text inputs precede unrequested storage; preserve input
order among unrequested contributions. Synthetic PHDR reservations must precede
all of them. Remove aliased contributions and their source relocation groups;
redirect targets, the entry, and map rows through the direct representative,
preserving each symbol's section-relative offset. When preserving unwind data,
retain only FDEs of representative code. TLS offsets and merged-piece mappings
do not acquire code-alias coordinates. With empty aliases/order, output must
remain identical to `lower_prepared`.

`UnwindPolicy::Preserve` decodes and rebuilds supported runtime annotations.
`AbortOnly` deliberately omits them; it is only valid for programs that do not
need unwinding or runtime backtraces. It does not relax ordinary relocation,
TLS, merge, or address-identity rules.

1. Partition `prepared.split()` into exact TLS_FLAGS inputs and mergeable
   inputs. Apply `tls_layout` to only the TLS inputs, and class11::unwind::merge to the
   others. Reject a non-NONE relocation carried by a mergeable input as
   `Merge(Merge { cause: Relocated, .. })`; NONE needs no patch. Retain the
   mapping in `Lowered.merged`. Propagate class10 failures through `Merge`.
   Reject more groups than fit above MERGE_BASE as TooLarge. Do not reparse
   or reselect the inputs.
2. For each live relocation in group and record order, skipping NONE: a source
   that is BSS or `.tbss` is `Output(InvalidRelocationTarget)`. Bind the symbol
   as class9 does. A section definition inside the template is `Tls` with
   `tpoff(section, value)`; other section definitions and COMMONs are `Image`;
   For a named symbol in merged storage, class11::unwind::locate maps its input value
   to `(group, within)`; the image slot becomes Section of synthetic
   `(usize::MAX, MERGE_BASE + group)` at `within`. ABS and unresolved WEAK
   are `Absolute`. Lower it; errors become
   `Reference { section, offset, kind, cause }`. For `Got`, reserve the slot:
   the image slot, `Constant(value)`, or `Constant(tp offset)`.
   For a SECTION symbol (type 3) in merged storage, map `st_value + A` as
   an input coordinate and replace the record's addend with zero after mapping.
   Use checked signed addition; a negative/overflowing coordinate is
   `Merge(Merge { cause: InvalidReference(st_value), .. })`. A coordinate outside
   every input piece is the `locate` diagnostic. This is the same conservative
   SECTION rule as class10; it does not infer an x86 instruction's end from its
   relocation field. A PC32 SECTION addend of -4 at input offset zero is rejected.
   Named symbols retain their original addend after mapping the symbol value.
   Do not confuse a file offset, input piece coordinate, group offset, or RVA.
3. The number of RELATIVE records is the number of `Fixup` R_X86_64_64
   references to image targets plus the number of image slots in the GOT.
4. Contributions, in order: `PHDR` if TLS exists; class8's live storage;
   merged ReadOnly groups with their group alignment and class10 identities;
   `class11::pie::BOOT` and `class11::pie::RELA` as in class9; `GOT` if nonempty;
   `TLS_IMAGE`, `TLS_AREA`, and `TLS_BOOT` if TLS exists. Plan with class6.
5. Patch with RVAs. Relocations in `.tdata` patch the copy of their section in
   `TLS_IMAGE` and count as writable Data. `Fixup` uses `class11::pie::patch`.
   `Relax` changes the opcode and patches PC32 to the target. `Got` patches
   PC32 to the slot. `ThreadOffset` applies `class11::relocate::apply` with kind 32S to
   the TP offset and addend. Patch failures are `Pie(Fixup { .. })`.
6. Encode the GOT into its placement and append its records. Return `Lowered`
   with that image, records, optional TLS template, user entry RVA and map.
   Do not call `emit` inside `lower_prepared`.

Placement addresses retain class6's `class11::executable::BASE` convention. Convert them to
RVAs for PIE patches, RELATIVE addends, the entry and the map; do not subtract
the base twice or change placement metadata in place. The pre-emission image
still has class6's zeroed header reservation. Final serialization consumes it
and writes the ELF header, startup records and PT_TLS when applicable.

Inputs without TLS sections, GOT references, mergeable inputs, or runtime unwind
annotations produce exactly class9's bytes and map. This baseline requirement
does not apply to inputs that add unwind headers or other new synthetic storage.

### Map

The class9 PIE map, where TLS rows show their address inside the template
(`TLS_IMAGE` RVA plus template offset; `.tbss` rows lie past the stored image),
followed by `SYNTHETIC` lines for `.got`, `.tls.image`, and `.tls.area` when
present, then merged groups in group order using class10's
`.rodata.str{entry_size}.{alignment}` or `.rodata.cst{entry_size}.{alignment}`
names. A live merged input row shows its group's RVA, not the location of
every individual piece. Startup and program-header reservations are not listed.

## Acceptance and references

Native tests check initialized, zero-filled, static, and external TLS variables
in both input orders; a pointer stored in `.tdata`; `&counter` computed from
`%fs:0`; a second thread created with `CLONE_SETTLS` whose block is built only
from `PT_TLS`, showing independent copies; GOT access under `-fPIC` with and
without relaxation, including an ABS symbol and an unresolved WEAK reference,
across ASLR runs; byte-identical class9 output without the new features;
rejection of general-dynamic TLS and mismatched references; and the CLI map.

`lowering_preserves_layout_before_startup_serialization` inspects the retained
plan before emission: the header reservation is still zero, the GOT contains
the constant TP offset, and an absolute entry pointer remains zero with one
pending RELATIVE record. It then emits the same layout, checks ET_DYN and the
TLS program header count, and runs it repeatedly. The test distinguishes
compile/link-time patching from load-time address repair; returning a final
ELF file in place of the staged result does not meet this contract.

- [ELF Handling for Thread-Local Storage (Drepper)](https://www.akkadia.org/drepper/tls.pdf)
- [x86-64 psABI: TLS and GOT relocations, relaxation](https://gitlab.com/x86-psABIs/x86-64-ABI)
- [arch_prctl(2)](https://man7.org/linux/man-pages/man2/arch_prctl.2.html),
  [clone(2)](https://man7.org/linux/man-pages/man2/clone.2.html)

## TLS metadata precedes size arithmetic

Validate every TLS section’s type, flags, size relationship, and alignment before sizing the template and thread block. A huge early tbss followed by alignment=3 reports the latter InvalidTls first. Grouping PROGBITS before NOBITS is a layout rule, not permission to skip input validation.

Contract coverage: `tls_metadata_precedes_size_calculation`.

## Method map: lower indirection and build TLS

`tls_layout` validates all TLS sections first, then places initialized PROGBITS before zero-filled NOBITS, rounding each section by its own alignment. The result stores the template’s file size, memory size, and thread-pointer offsets; it is not ordinary BSS because each thread receives a private copy.

`lower` uses the kind, target category, and the precise byte checks in C11.2.
The test forms are `8b /r disp32` (six bytes) and `48 8b /r disp32` (seven
bytes); `48` is a one-byte REX.W prefix. In both, the opcode is two bytes before
the displacement field. This interface does not perform a full instruction
decode. A `Fixup` result delegates validation to class9 and is not a promise
that every otherwise-unlisted kind is supported. `Got::offset` deduplicates
slots in first-use order.

The student task `lower_with` reserves and patches GOT/TLS/startup pieces while
preserving the plan's placement information. The supplied `lower_prepared`
and `lower_with_unwind` wrappers delegate to it. `emit` serializes startup descriptors and
adds PT_TLS. `link_with` composes preparation, lowering and emission and accepts
a classifier from class14 for Rust/LLVM sections.

## Coordinate check: a pointer into the second constant

Suppose a mergeable input holds two four-byte constants, both 21. They start
at input offsets 0 and 4, but the merged group holds only one four-byte piece.
`locate(input, 0)` and `locate(input, 4)` therefore both return `(group, 0)`.
A writable `.quad section + 4` uses a SECTION symbol with `st_value=0` and
`A=4`. Its final load-time address must refer to the shared piece:

| Quantity | Value | Coordinate system |
|---|---:|---|
| SECTION symbol value | 0 | Input section |
| Original addend | 4 | Input section |
| Coordinate passed to locate | 4 | Input section |
| Mapped offset | 0 | Merged output group |
| Group placement | `G` | class6 fixed-base planning address |
| Runtime record addend | `G - BASE` | PIE RVA |
| Address written by startup | `load_bias + G - BASE` | Runtime VA |

The original addend has already selected a piece before layout. Applying +4
again after mapping would point past the shared constant. The native test
checks the RELATIVE record's addend directly and dereferences the initialized
pointer at runtime, alongside TLS access. This separates a correct mapping
from a program that happens to exit successfully.

The combined native fixture `merged_constants_tls_and_got_share_one_pie_layout`
retains two identical four-byte constants, accesses a merged address via GOT,
compares it with a load-time-patched pointer to the other constant, and reads
a TLS variable. It checks the retained piece mapping, ET_DYN and PT_TLS,
synthetic map entries, and repeated runtime result 42. Runtime unwind metadata is covered by the following combined fixture; debug
sections are not emitted at this stage.

## Runtime unwind metadata in a PIE

The policy accepts class10's supported `.eh_frame` annotation subset. C11.5
uses `frames`, binds each FDE to live Text, computes its checked input start,
and discards FDEs for dead code. Propagate class10 framing/target errors through
`Merge`. Reserve `EH_FRAME=(MAX,8)` and `EH_FRAME_HDR=(MAX,9)` as ReadOnly,
aligned 8 and 4 with sizes from `unwind_sizes`. They follow other synthetic
storage. Reserve `UNWIND_PHDR=(MAX,7)` as 56-byte Text immediately after PHDR
if TLS exists, otherwise first at HEADER_BYTES. These IDs avoid class9's BOOT
and RELA, class11's GOT/TLS, and merge groups starting at 16.

Reuse `emit_unwind` by building a temporary placement view with the two class11
unwind placements under class10's expected keys. Keep original image identities
and addresses unchanged. All other FDE targets retain their real input keys.
Copy resulting bytes into the reserved contributions. The encodings depend on
address differences, so subtracting the same load bias from both addresses does
not require runtime relocation of these fields.

The supplied Lowered::emit retains PT_TLS and adds PT_GNU_EH_FRAME in the
reserved slot. It records RVAs in p_vaddr/p_paddr, file offsets in p_offset,
and emits five headers without TLS or six with it. The entire program-header
table remains contiguous inside the RX mapping. The lower-level C11.4 `emit`
interface remains the startup/TLS serializer; use Lowered::emit for the full
pipeline. Append synthetic unwind map rows after merged groups, and show an
annotation's EH_FRAME RVA only if it contributed a retained FDE.

The native test checks TLS-present and TLS-absent outputs, the index's pointer
to .eh_frame, initial function RVA, program-header fields, repeated execution,
and rejection of a malformed header reservation. These checks establish valid
address relationships; language-specific exception handling remains outside the
supported class10 encoding subset.

`lower_with_unwind(prepared, false)` omits annotation records under an explicit
abort-only policy. `link_abort_only` exposes that composition for class14's
controlled no_std fixture; normal `link` and `link_with` preserve supported
unwind information and reject unsupported records. This distinction prevents
unsupported exception metadata from silently bypassing normal validation.

## Snapshot boundary

This directory is a complete, independently buildable project. Earlier modules
contain reference implementations; this assignment owns `src/tls.rs`.
The public interfaces stay stable so you can substitute your own earlier modules.

Run `cargo test --test prerequisites` from this directory to check the supplied
foundation independently of this assignment.
