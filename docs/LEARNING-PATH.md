# Reading and implementation path

The theory series explains mechanisms; each laboratory specifies one bounded
implementation task. Their numbers are independent. Lab 00 parses ELF, so it
starts with Theory 02, rather than Theory 00. Later labs combine mechanisms from
several theory chapters. Read the relevant mechanism before implementing its
interface, then use the class README for the exact supported subset and error
contract. A general theory chapter is not a promise that the teaching linker
supports every format or runtime feature it discusses.

The course assumes working Rust knowledge: borrowing, slices, enums, `Result`,
checked integer operations, and Cargo. Start with Theory 00
for the compile/link distinction. Theory 01
provides historical context and can be read independently.

## Core path

Work through class0 to class15 in order. The right column names the knowledge
needed for the new task; earlier mechanisms remain prerequisites throughout.
Links open the English articles. The blog's language selector opens the Chinese
version of the same article.

| Class | New implementation | Read first |
|---|---|---|
| 0 | Validate the ELF boundary and select section descriptors | Theory 02: ELF header bytes |
| 1 | Decode sections, contents and names | Theory 02: section-header fields |
| 2 | Decode symbols and RELA records | Theory 02: symbols and RELA |
| 3 | Select global definitions and bind references | Theory 03: symbol resolution |
| 4 | Emit a minimal executable | Theory 05: layout, Theory 06: loading and entry |
| 5 | Patch cross-object references | Theory 04: relocation coordinates and arithmetic |
| 6 | Lay out permissions, initialized data, BSS and COMMON | Theory 05: file versus memory storage, Theory 06: load mappings |
| 7 | Parse archives and extract required members | Theory 03: archive container and member selection |
| 8 | Mark reachable sections and explain KEEP/DROP | Theory 05: section GC |
| 9 | Produce a self-relocating static PIE | Theory 04: relative versus absolute references, Theory 06: startup responsibilities |
| 10 | Merge constants and rebuild unwind tables | Theory 05: merge mappings, Theory 08: CIE/FDE records |
| 11 | Add GOT access, relaxation and static TLS to PIE | Theory 07: GOT, Theory 09: TLS coordinates and models |
| 12 | Attach relocated debug records, symbols and section headers | Theory 11: debug coordinates and file organization |
| 13 | Fold eligible code and choose code order | Theory 12: equivalence and address identity, unwind compatibility |
| 14 | Accept rustc's command line and link native no_std code | Theory 14: Rust artifacts, driver arguments and startup |
| 15 | Investigate robustness, determinism and behavior | Theory 16: layered validation and counterexamples |

## How the implementation accumulates

Classes 0–8 establish parsing, symbol selection, relocations, storage, archive
selection and reachability. Class9 adds the self-relocating PIE path. Class10
uses the shared class8 frontend to introduce merge and unwind rewriting in
ET_EXEC; class11 brings these stages into PIE together with GOT and TLS. Class12
attaches debug metadata to either completed layout. Class13 folds and orders code
before that shared lowering, then finishes debug metadata against the result.
Class14 supplies the compiler-facing driver for the cumulative static-PIE path.

Each class is a complete snapshot with reference implementations of earlier
modules. There are no inter-class Cargo dependencies. To reuse your own solution,
replace its corresponding module within the new snapshot, preserve the public
interface, and run the prerequisite tests. Runtime policy remains explicit: class14's supplied
abort-only policy omits runtime unwind metadata, while the preserving backends
must retain supported unwind descriptions and reject unsupported ones.

## Additional theory

Theory 10
extends layout to linker scripts and kernel/ROM startup;
Theory 13
examines ABI evolution; and
Theory 15
compares Mach-O and PE/COFF. They deepen the model but do not add hidden tasks to
the native x86-64 ELF core. Dynamic linking, full language exception support,
other architectures and other executable formats require their own explicit
implementation contracts.
