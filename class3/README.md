# Class 3: Global Symbol Resolution

Implementation: [`src/resolve.rs`](src/resolve.rs).


Choose the definition used by each reference across multiple input objects. This
stage computes identities, not addresses. A selected definition remains a pair
of input-file and symbol indices until class5 and class6 assign output positions.

## Interface and prerequisites

The completed modules from classes 0–2 are supplied. Each element of `files` is one validated symbol table from
one input object; retain its original symbol order, including the null entry.
The current pipeline selects one static symbol table per input. Supporting more
than one per file later requires extending the identity, not silently mixing
indices from different tables.

Implement C3.1 and C3.2 in [`src/resolve.rs`](src/resolve.rs). Keep public types, supplied tests, and
Cargo dependencies unchanged. Private helpers and additional tests are allowed.
No invocation of another linker or import of its resolution implementation is
permitted.

## C3.1: Build the global map (65 points)

Scan files in command-line order and symbols in table order. Validate a symbol
before merging it: binding, COMMON binding, COMMON alignment, then non-local
name. Supported bindings are LOCAL (0), GLOBAL (1), WEAK (2). COMMON supports
GLOBAL only in this course. Normalize alignment zero to one; other alignments
must be powers of two. Every non-local name must be nonempty, including undefined
names. Local empty names are valid, notably the null symbol and section symbols.

LOCAL entries never enter the global map. Undefined symbols do not provide a
definition. For a named definition, apply this precedence:

| Incoming definition | Existing weak | Existing COMMON | Existing strong |
|---|---|---|---|
| Weak section/absolute | Keep first | Keep COMMON | Keep strong |
| Global COMMON | Replace weak | Merge | Keep strong |
| Global section/absolute | Replace weak | Replace COMMON | Duplicate error |

COMMON merging takes the maximum size and maximum alignment independently. Keep
the first COMMON's identity for diagnostics; do not sum sizes. Two strong
definitions conflict even if their values are equal or no relocation uses them.
Return the first conflict in scan order with both identities and the byte name.

The output is a BTreeMap keyed by owned name bytes. Iteration is lexicographic and
independent of hash randomization. Do not require UTF-8. Weak ties are stable in
input order. This is a static linker: visibility metadata stays with the original
symbol and does not implement runtime interposition at this stage.

## C3.2: Bind one reference (35 points)

### An identity example

These are entries in two already-validated tables; both also have their null
entry at index 0. Numeric positions below are input identities, not addresses.

| File | Symbol index | Name | Binding | Definition | Value | Size |
|---|---|---|---|---|---|---|
| 0 | 1 | helper | LOCAL | Section(1) | 0 | 6 |
| 0 | 2 | helper | GLOBAL | Undefined | 0 | 0 |
| 0 | 3 | buf | GLOBAL | Common | 8 | 16 |
| 1 | 1 | helper | GLOBAL | Section(1) | 4 | 6 |
| 1 | 2 | buf | GLOBAL | Common | 16 | 32 |

`Globals[b"helper"]` is `Definition(SymbolId { file: 1, symbol: 1 })`.
`Globals[b"buf"]` is `Common { first: SymbolId { file: 0, symbol: 3 },
size: 32, alignment: 16 }`. The two COMMON requirements reserve one later
allocation, using independent maxima; their `value` fields are alignments.

Binding file0's symbol1 returns `Selected(Definition(file0:symbol1))`, since
LOCAL identity is independent of the same global name. Binding file0's symbol2
returns `Selected(Definition(file1:symbol1))`. Layout later converts that
selected definition's section and value into an address. Resolution does not
add a virtual base or use the symbol index as a byte offset.

Call `bind` only after `build_globals` succeeds, using exactly the same unmodified
input tables. Validate the file/symbol indices before reading them.

- A defined LOCAL binds to its own identity, including an absolute local.
- An undefined LOCAL is an error; it cannot be satisfied by an unrelated global.
- Any non-local reference looks up its name in the selected map. This includes
  references whose own table entry is a weak definition overridden elsewhere.
- An unresolved WEAK binds to Zero. An unresolved GLOBAL produces Undefined.

A name can occur as an undefined symbol without being referenced. Building the
map must succeed in that case. The later relocation loop calls `bind` for records
it actually applies; that is when a required missing definition becomes an error.
COMMON selections describe storage to allocate later. Do not allocate BSS here.

## Worked example: input identity versus selected definition

The following two input symbol tables retain their own indices. Entry zero is
the null symbol in each table; the other entries shown here have valid metadata.
Names denote byte strings, and `Section(n)` refers to a section of that input
file, not an output section or address.

| File | Symbol index | Name | Binding | Definition |
|---|---|---|---|---|
| 0 | 0 | Empty | LOCAL | Undefined (null entry) |
| 0 | 1 | `scratch` | LOCAL | Section(1) |
| 0 | 2 | `f` | WEAK | Section(1) |
| 0 | 3 | `g` | GLOBAL | Undefined |
| 0 | 4 | `optional` | WEAK | Undefined |
| 0 | 5 | `missing` | GLOBAL | Undefined |
| 1 | 0 | Empty | LOCAL | Undefined (null entry) |
| 1 | 1 | `scratch` | LOCAL | Section(2) |
| 1 | 2 | `f` | GLOBAL | Section(2) |
| 1 | 3 | `g` | GLOBAL | Section(2) |

After `build_globals`, the map contains exactly two entries:

```text
b"f" -> Selection::Definition(SymbolId { file: 1, symbol: 2 })
b"g" -> Selection::Definition(SymbolId { file: 1, symbol: 3 })
```

`scratch` is local to each file. Neither undefined name supplies a definition,
so `optional` and `missing` do not enter the map. Building the map succeeds even
though `missing` has no provider. With that map, individual calls to `bind` give:

| Reference identity | Result | Reason |
|---|---|---|
| (0, 1) | Selected Definition(0, 1) | A local definition keeps its own identity |
| (1, 1) | Selected Definition(1, 1) | The same local name in another file is independent |
| (0, 2) | Selected Definition(1, 2) | The strong `f` replaces the weak definition |
| (0, 3) | Selected Definition(1, 3) | The undefined `g` finds its provider |
| (0, 4) | Zero | An unresolved weak reference permits zero |
| (0, 5) | Undefined { name: b"missing", reference: (0, 5) } | A required reference has no provider |

Pairs in this result table abbreviate `SymbolId { file, symbol }`. The selected
`f` is identified by (1, 2); neither its section-relative value nor its final
address is encoded in that pair. Layout and relocation consume this identity
later. The null entry is retained for indexing, but is not a definition to bind.

## Completion and evidence

```sh
cargo test -p class0 -p class1 -p class2 -p class3
cargo test -p class0 -p class1 -p class2 -p class3 --release
python3 scripts/grade.py class3
```

Contract tests cover both input orders, duplicate strong definitions, local name
isolation, common size/alignment maxima, malformed symbols, unresolved weak
references, and byte names. The native integration assembles separate caller and
provider files, parses them through classes1–2, and binds the caller's relocation
to the provider's definition. It does not manufacture a solved table.

In your implementation notes, explain why resolving references while reading the
first file would be premature, and why COMMON cannot be treated as an ordinary
strong definition at this stage.

## Reference

[System V ABI: Symbol Table](https://gabi.xinuos.com/elf/05-symtab.html).
The course restricts supported inputs as documented above; it does not claim to
implement every ELF symbol extension or dynamic linking rule.

## Validation precedes winner selection

A strong definition does not excuse malformed COMMON metadata later in the input. Validate each symbol before deciding whether it changes the selected definition. Report the first conflict in file and symbol order, not lexical name order: a duplicate z encountered before a duplicate a is the first diagnostic.

Contract coverage: `validation_precedes_symbol_selection`, `first_conflict_follows_input_order`.

## Method map: choose definitions, then bind references

`build_globals` scans files in command-line order and symbols in table order. It builds a map only for non-local names; local symbols are identified by `(file, symbol)` and never collide with another object’s local name. For a global name, strong definitions beat COMMON, and COMMON beats weak definitions according to the table in this README. Duplicate strong definitions return the first conflict with both identities.

`bind` is deliberately separate from `build_globals`. Building the map cannot report an undefined symbol that no relocation uses. Binding a specific `SymbolId` resolves it to the selected definition, zero for an unresolved weak reference, or an error for an unresolved required reference. This separation mirrors a real linker: collect first, resolve uses later.

A COMMON symbol’s size is the maximum size seen and its alignment is the maximum alignment seen; do not sum them or let input order change the result. Validate each symbol’s binding and COMMON metadata before allowing a later definition to replace it. The first diagnostic follows input order, not lexical name order.

## Snapshot boundary

This directory is a complete, independently buildable project. Earlier modules
contain reference implementations; this assignment owns `src/resolve.rs`.
The public interfaces stay stable so you can substitute your own earlier modules.

Run `cargo test --test prerequisites` from this directory to check the supplied
foundation independently of this assignment.
