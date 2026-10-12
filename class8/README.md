# Class 8: Link only reachable storage

Implementation: [`src/gc.rs`](src/gc.rs).


A library member is an extraction unit, not necessarily a useful output unit.
One extracted object can contain a used function, an unused function, and data
referenced by neither. This project makes the section the unit of liveness.
The result must still be a working ELF executable; a graph demonstration alone
is not a solution.

## Deliverables and edit boundary

Implement `C8.1`–`C8.3` in [`src/gc.rs`](src/gc.rs). Private helpers may be added there. Keep
public interfaces and provided tests unchanged. Earlier class implementations
remain prerequisites. The CLI and the ET_EXEC lowering in `link` are supplied
infrastructure. No external crates or invocations of `ld`, compiler drivers, or
other linkers inside the implementation.

| Task | Responsibility | Points |
|---|---|---|
| C8.1 | Iterative traversal with deferred reference errors | 25 |
| C8.2 | Deterministic, byte-safe map serialization | 15 |
| C8.3 | Validated live link plan and cumulative backend integration | 60 |

The integration group depends on both preceding tasks. It is not independently
solvable by substituting a prebuilt binary. Run `python3 scripts/grade.py class8`
on native x86-64 Linux after completing the prerequisites.

## Front-end and backend boundary

`prepare_with` is task C8.3. It returns a `Prepared` state without assigning
output addresses. Its contributions and relocation groups contain only live
sections; its map rows contain every allocated section of selected objects,
initially with no assigned address. Its symbol tables and global selections
retain stable input identities. It includes the validated entry section and
offset. No section data is copied merely to build the plan.

The supplied `prepare` calls `prepare_with` with the supplied `storage_policy`,
which is exactly class6's type/flags table. The supplied `link` consumes that
state, uses the student implementations of class6 layout and class5 relocation,
then emits ET_EXEC and a map. Its planning and relocation step is exposed as
`lower_exec`, and its map rows as `placed_rows`, for backends that only add
metadata to the same image. Later backends reuse `prepare_with` instead of
duplicating archive selection and GC. The state has private fields and exposes
immutable accessors. Backends cannot mutate the validated graph or manufacture a
`Prepared` value through the public API.

### Section roles

The classifier receives every allocated section header and its name. `None`
means unsupported and fails with `Output(UnsupportedSection)`, even for an
unreferenced section. Non-allocated sections are never passed to it.

| Role | Graph node | In `contributions` | Exposed through |
|---|---|---|---|
| `Storage(kind)` | Yes | Yes, when live | `contributions` |
| `Split` | Yes | No | `split`, when live |
| `Annotation` | No | No | `annotations`, always |

A `Split` section is traced exactly like storage: it can be a root through
`keep`, it is live when referenced, and its own relocation group appears in
`relocations` when live. Only the backend decides how to place its bytes.

An `Annotation` section is never a root, node, or edge source, so its
relocations neither keep sections alive nor raise deferred reference errors.
Its RELA groups are still parsed and validated with the same ownership rules,
and are returned through `annotation_relocations` for every selected object.
A symbol defined inside an annotation is not storage: a live reference to it is
`Error::InvalidSymbol`, and `keep` naming it is `InvalidKeep`. The entry must
select `Storage(Text)`. `Special` records the header fields a backend needs and
borrows the section payload. Under `storage_policy` both new lists are empty.

### Metadata

`metadata` lists every non-allocated PROGBITS section (type 1, no ALLOC flag)
of every selected object, live or not, as a `Special`; `metadata_relocations`
lists the RELA groups that target them. Metadata never takes part in GC, and
the class8 backend ignores it. Later backends use it for debug information.

## Pipeline and observable behavior

1. Call class7 selection in input order. Archive extraction happens before GC.
   A reference from a subsequently discarded section can still extract a member.
   Do not revisit earlier archives or invent new extraction rules here.
2. Parse sections and the single SYMTAB of every selected object. Resolve globals
   using class3 before GC: duplicate strong definitions remain errors even if
   their sections would otherwise be discarded.
3. Classify every allocated section with the supplied classifier (class6's exact
   kind/flag policy under `prepare`). Reject unsupported allocated sections even
   if unreferenced. GC is not a way to make unsupported input features silently
   disappear. Non-allocated sections are
   metadata, not output candidates. Parse every RELA table using class2, including
   tables whose source section will be dead or metadata. Reject nonempty REL.
   RELA ownership must use that object's sole SYMTAB. Malformed records are not
   excused by deadness. Valid relocations targeting non-allocated metadata do not
   create graph edges or output patches; debug information support comes later.
4. Root the selected entry definition's section. It must be nonempty Text and
   contain the entry byte. Each `keep` name must resolve to a section definition
   in allocated storage; missing, ABS, COMMON, or metadata roots are InvalidKeep.
   Exports and symbol visibility alone do not make roots. There is no automatic
   retention for constructors, SHF_GNU_RETAIN, or linker-script rules in this class.
5. Each non-NONE relocation from an allocated section creates a dependency on
   the **selected definition**, not necessarily the symbol's own input section.
   Section definitions keep their whole section; selected COMMON keeps that
   allocation; ABS and unresolved WEAK keep no storage. Undefined GLOBAL and
   invalid definition targets become deferred errors on that edge. Ignore NONE
   entirely, including its symbol and offset for semantic purposes (class2 still
   validates the record's symbol index). Symbol values may equal a target's size
   for an end label, but may not exceed it. A symbol's declared size does not
   determine reachability. Relocation addends do not select a different section.
6. Trace roots, plan only live contributions in original file/section order, and
   allocate only live selected COMMON names in raw-byte order. Apply relocations
   from live file-backed sections using class5, bounded to each contribution.
   Live relocations targeting BSS are InvalidRelocationTarget. Unsupported kinds,
   overflows, and patch bounds in dead sections are not applied or reported.
   Errors in live sections are reported. Emit with class6 and construct the map.

`Error::Archives` wraps selection errors. Later structural, layout, and relocation
errors use `Error::Output` with the corresponding class6 variant. A missing entry
uses Output(MissingEntry); an invalid entry uses Output(InvalidEntry). A live
undefined GLOBAL uses `Error::Undefined` with the exact symbol-name bytes. An
invalid live symbol value or definition uses `Error::InvalidSymbol`. Unless a
traversal rule below says otherwise, tests do not prescribe error priority when
an input violates several unrelated structural rules.

## Graph contract

Section IDs are `(selected-object ordinal, ELF section index)`. Archive members
use their position in the selected object list. Graph nodes include all allocated
sections, including empty sections. Edges preserve relocation-section order and
record order. This identity survives layout changes and avoids collisions between
identically named sections in different objects.

`trace` first validates all roots exist, in caller order. It then performs
iterative depth-first traversal: complete the first root's first edge subtree
before its second edge; finish that root before the next root. Inspect each node
once. Cycles and duplicate roots terminate. A missing target node is an error
only when traversing its live edge. Undefined and Invalid edges are likewise
errors only when visited. Empty roots succeed even if the graph contains dead
errors. Use an explicit work stack; the test graph exceeds normal recursive
thread-stack capacity. Aim for O((V + E) log V) time and O(V + E) storage with the
provided ordered containers.

### Reading a graph and its result

`Graph` is a `BTreeMap<SectionId, Vec<Target>>`. A key identifies one input
section; its vector is that section's outgoing references in contract order.
The key order does not replace the order of roots or outgoing references.
`Live.sections` contains retained section IDs, while `Live.commons` contains
retained COMMON names. An absolute value has no storage to retain.

This example uses one selected object, ordinal zero:

```rust
use class8::gc::{Graph, SectionId, Target};

let a = SectionId { file: 0, section: 1 };
let b = SectionId { file: 0, section: 2 };
let c = SectionId { file: 0, section: 3 };
let graph = Graph::from([
    (a, vec![Target::Section(b), Target::Common(b"shared".to_vec())]),
    (b, vec![Target::Section(a), Target::Absolute]),
    (c, vec![Target::Undefined(b"missing".to_vec())]),
]);
```

With roots `[a]`, tracing visits `a`, follows its first edge to `b`, and sees the
back-edge to the already visited `a`. That cycle introduces no new storage.
`Absolute` adds nothing; returning to `a`'s second edge retains the COMMON name
`shared`. The successful result is therefore:

| Result field | Value |
|---|---|
| `sections` | `{a, b}` |
| `commons` | `{b"shared"}` |

The reference to `missing` belongs to `c`, which is unreachable from these
roots, so it does not fail this trace. Adding `c` as a root changes the result
to `Err(Error::Undefined(b"missing".to_vec()))`. This postponement is limited
to reference semantics: `prepare_with` must still reject malformed section,
symbol, and RELA records in a selected object, even if their source is dead.

### Why error order requires depth-first traversal

Consider a different graph with these ordered edges:

```text
a: [Section(b), Undefined("later")]
b: [Undefined("first")]
```

For roots `[a]`, the result is `Undefined("first")`: the subtree reached through
`a`'s first edge is completed before inspecting its second edge. Scanning all
of `a`'s edges first would incorrectly report `later`. A LIFO stack can also
reverse sibling order if work is pushed in forward order. Choose a work-state
representation that preserves the specified order; merely using a stack does
not establish the contract.

Root validation is a separate earlier phase. If the roots are `[a, absent]`,
and `absent` is not a graph key, the result is `MissingNode(absent)` before
traversing `a`, even when `a` would reach an undefined symbol. A section target
missing from the graph is different: it raises `MissingNode` only when its
source edge is traversed. These distinctions explain the tests
`all_roots_are_validated_before_traversal`, `dead_errors_are_not_live_errors`,
and `traversal_order_and_deep_graph`; they are observable API requirements,
not incidental container behavior.

## Map format

`render_map` receives unique section IDs and sorts them by `(file, section)`.
The first line is `TINYLINK MAP 1`. Each section then emits exactly one line:

```text
KEEP 0:1 0x0000000000400120 17 .text.start
DROP 0:5 - 64 .text.unused
COMMON 0x0000000000403000 shared
```

IDs and sizes are unsigned decimal. Addresses have `0x` followed by exactly 16
lowercase hexadecimal digits. An empty name is `-`. In names, only ASCII bytes
0x21–0x7e other than backslash and hyphen are literal; every other byte is escaped
as `\xhh` with lowercase hexadecimal. Escaping hyphen makes the empty-name token
unambiguous. Do not use lossy UTF-8 decoding. COMMON lines follow section lines
in raw-byte name order and report only allocated live COMMONs. Every line ends
with one newline. No timestamps, host paths, pointers, or hash iteration order.

The map includes all allocated sections from **selected** objects, even empty or
discarded ones. It does not list unselected archive members. `address: None`
means discarded. Zero-sized retained sections still have assigned addresses.
The map is diagnostic output and is not embedded in the executable.

## Acceptance evidence

Tests must establish transitive code/data reachability, unreachable cycles,
dead undefined references versus live failures, retained COMMON/BSS, explicit
roots, archive selection before GC, deterministic maps, native execution, and
structural validation of dead inputs. A smaller file alone is insufficient:
page alignment can leave file size unchanged after eliminating a small section.
Inspect map membership and verify observable execution as well.

A GNU `ld --gc-sections` comparison is useful for supported fixtures, but this
course deliberately does not implement GNU linker scripts or all ELF retention
rules. See [GNU ld options](https://sourceware.org/binutils/docs/ld/Options.html)
for `--gc-sections`, `--print-gc-sections`, and `-Map`. This handout defines the
assignment's bounded contract; external documentation does not enlarge it.

## Validate roots before traversing edges

Check every root exists, in caller order, before DFS. An undefined edge reachable from the first root does not take priority over a missing later root. Once roots are valid, depth-first edge order determines the first reachable error. Dead-section application errors remain outside the live traversal.

Contract coverage: `all_roots_are_validated_before_traversal`.

## Method map: graph, roots, lowering

`trace` first validates every root in caller order, then walks edges iteratively. The iterative stack is intentional: a real object graph can be deeper than the Rust thread stack. Undefined symbols and malformed targets matter only when reached from a live root; an error in dead code must not reject the link.

`render_map` turns the result into a deterministic debugging artifact. Sort by stable section identity, escape arbitrary bytes, and use fixed-width addresses. The map is not cosmetic: it is evidence for why a section survived or disappeared and allows later tests to compare two links byte-for-byte.

`prepare_with` parses archives, classifies sections, resolves symbols, and records the graph without assigning final addresses. `lower_exec` and `placed_rows` bridge that abstract result to class6’s storage planner and map output. Keeping graph reachability before placement is what makes garbage collection possible.

## Snapshot boundary

This directory is a complete, independently buildable project. Earlier modules
contain reference implementations; this assignment owns `src/gc.rs`.
The public interfaces stay stable so you can substitute your own earlier modules.

Run `cargo test --test prerequisites` from this directory to check the supplied
foundation independently of this assignment.
