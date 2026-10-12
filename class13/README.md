# Class 13: Fold identical code, then choose the order

Implementation: [`src/icf.rs`](src/icf.rs).


Compilers emit many functions that are byte-for-byte identical: template
instantiations over types with the same layout, small wrappers, duplicated
helpers in different translation units. Identical Code Folding (ICF) keeps one
copy of each and points every reference at it.

Folding must preserve both execution and associated runtime metadata.
Even identical instructions cannot share one unwind record if their CIE/FDE
descriptions differ. It must also preserve observable address identity. C guarantees that two distinct functions compare
unequal, so `&twin_a == &twin_b` must stay false. This project conservatively pins
non-call references and explicitly kept functions. Direct calls do not compare
function identity, while a function pointer can expose it.

Equality is recursive: two functions that call two different but identical
helpers are themselves identical once the helpers fold. The algorithm starts
optimistically, assuming every plausible pair is equal, and splits classes
until every member of a class references the same classes. That yields the
largest consistent folding, which also handles recursion.

Ordering is the other half of the class: placing named functions first is
the mechanism behind hot/cold splitting and startup-locality tuning.

## Deliverables

Implement C13.1–C13.3 in [`src/icf.rs`](src/icf.rs), with private helpers as needed. Public
APIs, `describe`, CLI, and tests are not student edit areas. Reuse
`class13::gc::prepare_with`, shared class10/class11 lowering, and `class13::debug::finish`,
which now takes the folding result as `Aliases`.

| Task | Responsibility | Points |
|---|---|---|
| C13.1 | Partition refinement to the largest safe folding | 40 |
| C13.2 | Decode and normalize runtime unwind descriptions | 25 |
| C13.3 | Candidates, address significance, and integration | 35 |

`link` supplies ET_EXEC with class10's policy; `link_pie` supplies static PIE
with class11's policy and `UnwindPolicy::Preserve`. The CLI selects PIE with
`--pie`. Implement C13.3 in `link_with`, where `classify` controls input roles
and `Backend` selects output layout. Both paths retain merge mappings, runtime
unwind information, symbols and DWARF; the PIE path also retains GOT, TLS and
load-time relocation. ICF analyzes input identities before either layout.

Run `python3 scripts/grade.py class13` on native x86-64 Linux. Native tests
need the same tools as class12.

## C13.1: folding

Two candidates are *equivalent* when neither is significant and they have the
same bytes, the same normalized alignment, identical `unwind` vectors, the same number of references,
and, position by position, references with the same offset, kind, addend,
and an equivalent target:

- a section target that is a candidate is equivalent to another candidate
  target at the same offset when the two candidates are equivalent;
- any other target (a section that is not a candidate, a COMMON, a constant)
  must be equal.

`fold` returns the largest such equivalence: start with one class per
distinct (bytes, alignment, unwind descriptions, reference shape) among non-significant candidates,
where shape ignores which candidate a reference targets; give each significant
candidate its own class; then repeatedly split classes by the classes their
references point to until no class splits. Each class maps every member except
its smallest `SectionId` to that representative. The result must not depend on
the order of `candidates`.

This contract compares both raw bytes and relocation references. Raw relocated
fields need not be zero in ELF: explicit-addend RELA records carry their addends
separately, and the original field contents are not a universal format
guarantee. Here equal bytes preserve all non-relocated instructions, while
equal reference shapes and equivalent targets make the final patches equal.
Requiring raw bytes to match is conservative: differing original field bytes
prevent folding even if the patches would erase that difference. No field
normalization is part of this task. Sections with equal bytes that differ only
in which equivalent helper they reference can fold; the contract tests also
cover mutually recursive groups.

`folding_propagates_through_calls` includes two three-section chains. Their
callers have equal bytes, but the leaves differ. Separating the leaves must
eventually separate both levels of callers; one target-comparison pass need
not suffice. Conversely, `recursion_reaches_the_greatest_fixed_point` contains
equally shaped cycles and self-calls: their different section IDs alone are not
grounds for separation. Class IDs describe a temporary partition, not output
addresses, and need not have stable numeric values between passes. Pick the
smallest SectionId as representative only after the relation stabilizes.

`fold_compares_raw_relocated_bytes_conservatively` covers nonzero initial patch
fields. Equal raw bytes with equal references can fold; different raw bytes
remain separate under this task's contract even if relocation would overwrite
the difference. It does not require implementing relocation-field normalization.

## C13.2: runtime unwind compatibility

Implement `unwind_descriptions(prepared)`. Process annotations in input order
with `class13::unwind::frames` and their annotation relocation group (empty if absent).
Unsupported LSDA, personality, or record encodings are rejected by class10;
propagate these failures as `MergeUnwind(error)`. Do not silently discard them.

For every FDE, bind its initial-location relocation through class3. It must
select a section definition, otherwise return class10's `FdeTarget` with the
annotation identity and FDE input offset. Ignore dead targets. Live targets
must be Text storage and `symbol.value + addend` must fit in u64, with the
same `FdeTarget` diagnostic on failure.

For each live target, retain an `UnwindDescription` containing that input
start, the complete CIE bytes, and complete FDE bytes with only `[4,12)`
zeroed. These eight bytes hold the record-relative CIE pointer and relocated
initial location; they vary with input placement. Keep the FDE's range,
augmentation, and unwind instructions unchanged. Retain padding and the record length as well; different encodings may prevent
folding even when they describe equivalent behavior. Sort each target's vector
lexicographically by `(start, cie, fde)`; preserve duplicate records.

The folding relation compares these normalized vectors exactly. A section
with no FDE cannot fold with one carrying an FDE. Different start, CIE,
range, or unwind instructions prevent folding. This conservative equality
can miss semantically equivalent encodings; it does not claim to interpret
all DWARF programs or support language-specific exception tables.

`lower` is supplied and delegates to `class13::unwind::lower_with`; `lower_pie` delegates
to `class13::tls::lower_with`. These stages omit
folded copies, redirects code and entry references, honors the requested Text
order, merges constants, and rebuilds unwind tables with only retained FDEs.
The normalized comparison must establish compatibility before this call.

## C13.3: integration

1. `class13::gc::prepare_with` using `classify`. The candidates are the live Text contributions.
2. Bind every live non-NONE relocation, in group and record order, to an
   `Edge`. A reference of any kind other than `R_X86_64_PLT32` (4) to a
   candidate makes that candidate significant: data pointers, PC-relative
   address computations, and absolute fields all expose an address. Candidates
   named by `keep` are significant too. A candidate's references are its own
   records, sorted by offset.
3. Collect `unwind_descriptions`, supplying each candidate its vector (empty
   when absent), then `fold` and resolve `order` names to the sections of their selected
   definitions. A name that does not select a section definition in live Text
   storage is `InvalidOrder(name)`.
4. Select `lower` for `Backend::EtExec` or `lower_pie` for
   `Backend::StaticPie(unwind)`, then `class13::debug::finish` with the aliases:
   DWARF of folded copies is
   tombstoned, and their symbols take the representative's address.
5. Append `describe(&aliases)` lines (`FOLD file:section file:section`) to the
   map.

Without folds or order, output equals class12's corresponding `link` or
`link_pie` result.

An explicitly selected `Backend::StaticPie(UnwindPolicy::AbortOnly)` is for
freestanding programs without runtime unwinding or backtraces. It skips
`unwind_descriptions` and gives candidates empty unwind vectors; class11 omits
the annotations. Debug data and address significance are still processed.
Normal `link` and `link_pie` must reject unsupported personality/LSDA encodings;
they never choose this policy implicitly.

This is a conservative policy. It does not inspect `.llvm_addrsig` sections
(produced by Clang's `-faddrsig`), so a PC32 `call` from older assemblers also
pins its target. Data sections are never folded, and folding works on whole
sections, which `-ffunction-sections` makes per function.

## Acceptance and references

Native tests check that three identical functions across two objects run
correctly at one address with two `FOLD` lines; that address-taken functions
keep distinct addresses while identical called-only functions fold, and that
`keep` prevents folding; that GDB stops in a folded function with only the
representative's DWARF claiming the code; that ordered functions come first
and a folded name places its representative; that bad order names are
rejected; that class12's output is reproduced without folds; and the CLI's
`--order`.

- S. Tallam, C. Coutant, I. L. Taylor, X. D. Li, C. Demetriou, "Safe ICF: Pointer Safe and Unwinding Aware Identical Code Folding in Gold", GCC Developers' Summit, 2010
- [LLD: `--icf=safe`, `--symbol-ordering-file`](https://lld.llvm.org/)
- Hopcroft-style partition refinement, as used for DFA minimization

The combined native test uses identical instructions with either matching CFI
or one changed CFA offset. Matching descriptions fold; changed descriptions
retain distinct addresses. Both variants retain merged constants, the unwind
program header, tool-readable FDEs, ordered code, and the same runtime result.

The PIE combination adds initialized TLS and real GOT loads to both twins.
Matching unwind descriptions produce one retained FDE; differing descriptions
retain both. It checks the six program headers, TLS symbol coordinates,
folded-copy debug tombstones, merged constants, named ordering, repeated runtime
execution, and CLI/library byte and map equality. Separate PIE fixtures check
observable function-pointer inequality and a GDB breakpoint in folded code.
Invalid aliases and invalid order entries must return structured errors before
layout. With no folds, PIE output must reproduce class12's PIE result exactly.

## Snapshot boundary

This directory is a complete, independently buildable project. Earlier modules
contain reference implementations; this assignment owns `src/icf.rs`.
The public interfaces stay stable so you can substitute your own earlier modules.

Run `cargo test --test prerequisites` from this directory to check the supplied
foundation independently of this assignment.
