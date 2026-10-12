# Class 14: Be the linker that rustc runs

Implementation: [`src/driver.rs`](src/driver.rs).


Compiler drivers such as `cc`, `clang`, and `rustc` normally invoke a linker
*executable* and describe the job with command-line options: inputs in order,
library search directories, `-l` names, the output path, and flags such as
`-pie` and `--gc-sections`. A linker is only usable if it understands that
interface well enough and refuses clearly what it cannot do.

This class turns the cumulative class13 static-PIE backend into such an executable and
uses it as `rustc`'s linker for a real `#![no_std]` program. `rustc` passes
its generated glue object, the program's object, and the `.rlib` archives of
`core` and `compiler_builtins`. An `.rlib` archive carries object files and
compiler metadata; in the tested toolchain, the metadata members (`lib.rmeta`)
are ELF objects with one excluded section. LLVM output also carries COMDAT membership, mergeable constants, and
`.eh_frame`. Classes10–13 already handle merging and unwind metadata; this
adapter adds a bounded LLVM section policy and selects abort-only semantics.

## Deliverables

Implement C14.1–C14.4 in [`src/driver.rs`](src/driver.rs), with private helpers as needed. Public
APIs, the supplied `policy`, `main`, the Rust fixture, and tests are not
student edit areas. Use the supplied `class14::driver::link` adapter. It selects class13 static PIE with
the supplied LLVM section policy and explicit abort-only runtime semantics.

| Task | Responsibility | Points |
|---|---|---|
| C14.1 | Parse a GNU-ld-style command line | 25 |
| C14.2 | Library search | 10 |
| C14.3 | Response files | 15 |
| C14.4 | Drive the link from a command line | 50 |

Run `python3 scripts/grade.py class14` on native x86-64 Linux. The native tests
need stable `rustc` with the `x86_64-unknown-linux-gnu` standard library,
`ar`, `readelf`, and GDB with permission to trace its own child processes.

## How rustc calls a linker

With `-C linker=PATH -C linker-flavor=ld`, `rustc` invokes `PATH` with GNU ld
options. For a `#![no_std]`, `#![no_main]` binary the command line looks like:

```text
symbols.o main.main.cgu.0.rcgu.o --as-needed -Bstatic libcore-*.rlib
libcompiler_builtins-*.rlib -L raw-dylibs -Bdynamic --eh-frame-hdr
-z noexecstack -L <sysroot>/lib -o main --gc-sections -pie -z relro -z now -O1
```

(`symbols.o` is a small object `rustc` generates.) The `linker-flavor=ld`
choice matters: `gcc` flavor addresses a compiler driver, while `ld` flavor
addresses the linker directly. Compiler-driver arguments and linker arguments
are different interfaces; linker-specific flags may need `-Wl,` forwarding
when a compiler driver is used.

## C14.1: options

Process arguments in order. A non-option argument is an input file. An
argument that is not valid UTF-8 is an input file too; options are ASCII.

| Option | Meaning here |
|---|---|
| `-o PATH`, `--output=PATH` | Output; exactly one is required (`InvalidOutput`) |
| `-e SYM`, `--entry SYM`, `--entry=SYM` | Entry symbol; default `_start`; the last one wins |
| `-L DIR`, `-LDIR`, `--library-path=DIR` | Append a search directory |
| `-l NAME`, `-lNAME`, `--library=NAME` | Library input, searched as `libNAME.a` |
| `-l:FILE` (any of those forms) | Verbatim library input `FILE` |
| `-z noexecstack`, `-z relro`, `-z now` | Accepted; see below |
| `-pie`, `--pic-executable`, `--gc-sections`, `--eh-frame-hdr` | Accepted; the backend always behaves this way or omits it |
| `--as-needed`, `--no-as-needed`, `-Bstatic`, `-Bdynamic`, `-static` | Accepted; only static archives are ever linked |
| `-O0` … `-O3` | Accepted; no effect |

Every other argument starting with `-`, including a lone `-`, is
`UnsupportedOption` with the argument text; for `-z` it is `-z KEYWORD`. That
includes `-shared`, `-no-pie`, `--no-gc-sections`, and `--dynamic-linker`. A
missing argument for an option that takes one is `MissingArgument(option)`.
No inputs is `NoInputs`.

Accepting an option means promising its effect. `-z noexecstack` holds because
class6 always emits a non-executable `PT_GNU_STACK`. `-z relro` and `-z now`
are accepted although the output has no `PT_GNU_RELRO`: with no dynamic loader
the only memory written during startup is the GOT and the data pointers that
class9's startup relocates, and it stays writable. That is a deliberate, documented weakening, not an
oversight. `--eh-frame-hdr` is accepted although no unwind index is emitted,
because the supported programs use `panic=abort` and never unwind.

## C14.2: library search

`find_library` tries each directory in order and returns the first
`dir/libNAME.a` (or `dir/NAME` when verbatim) for which `exists` is true;
otherwise `LibraryNotFound(NAME)`. Shared libraries are never considered,
which is why `-Bdynamic` can be accepted without effect. Missing directories
are not errors: `rustc` routinely passes a `raw-dylibs` directory that does
not exist.

## C14.3: response files

Command lines have a length limit. When `rustc` would exceed it, it writes
the arguments to a file and passes `@FILE` instead; GCC, Clang, and GNU ld all
accept this. `expand` replaces each argument `@PATH` whose file `read` can
supply with the arguments in that file, in place. An argument whose file
cannot be read, and a lone `@`, stay as written; that is GNU's behavior.

A file is split into arguments with these rules, applied byte by byte:

- outside quotes, space, tab, newline, carriage return, vertical tab, and form
  feed separate arguments, and runs of them produce no empty arguments;
- `'` and `"` start and end quoted text, which can contain separators and the
  other quote character, and can sit inside a word (`a'b c'd` is `ab cd`);
  quotes can produce an empty argument (`''`);
- `\` makes the next byte literal, inside or outside quotes; a trailing `\`
  at the end of the file is dropped;
- an unterminated quote extends to the end of the file.

Expanded arguments are themselves expanded. Expanding a file found at nesting
depth 16 (the top-level arguments are depth 0) is `ResponseFileDepth`, which
also stops a file that names itself. Arguments are bytes, not necessarily
UTF-8.

## C14.4: running a link

Expand response files (reading with `std::fs::read`), parse; for each input
in order, resolve libraries with `find_library` (using
`Path::is_file`), read the file (`Io` with the path), and pass files starting with
`!<arch>\n` as archives. Link with
`class14::driver::link(inputs, entry)`, write the output, and make it
executable (mode 0755). Link errors are `Link(e)`.

### Section policy

`policy` is supplied. It masks `SHF_GROUP` before classifying member sections;
it does not implement group selection by COMDAT signature. The supported Rust
fixture uses class3's weak global selection and class8's section reachability.
These mechanisms do not replace general COMDAT group semantics: a group can
contain associated data and local references as well as global definitions.
Supporting other group patterns requires a separate selection contract.

Mergeable constants and strings retain `Role::Split`; the class10 piece mapping
is used by class11 layout and class12 debug relocation. This driver currently supports the
provided freestanding `panic=abort` program rather than general Rust linking.
The explicit abort-only composition drops `.eh_frame` annotations; sections referenced only from them,
such as `.gcc_except_table`, can then be removed by GC. That policy does not
preserve stack unwinding, exception handlers, or general backtraces. It must
not be used as an implementation rule for programs that require those features.
The driver retains the cumulative merge/GOT/TLS/PIE/ICF/debug pipeline.
Class13 applies its conservative significance policy before folding; class12
emits section headers, symbols and supported debug metadata. The explicit
AbortOnly selection skips runtime unwind decoding and comparison. It does not
disable debug information or ordinary input validation.
Everything else follows `class14::tls::policy`.

## The Rust program

`tests/fixtures/no_std_fmt.rs` formats `sum=42 [2, 3] 1.50` with `core::fmt`
into raw `write` system calls, indexes a slice through `black_box` (keeping a
bounds check and `core`'s panic path), and exits with the sum. With no libc it
supplies `memcpy`, `memmove`, `memset`, `memcmp`, and `bcmp` in assembly so
that LLVM cannot turn them into calls to themselves. It is compiled with:

Run from the repository root on native x86-64 Linux after completing the
required classes:

```sh
cargo build --locked -p class14 --bin class14
course_linker="$PWD/target/debug/class14"
mkdir -p target/reader-class14
rustc --edition 2024 \
      -C panic=abort -C opt-level=2 -C force-unwind-tables=no \
      -C linker-flavor=ld -C linker="$course_linker" \
      class14/tests/fixtures/no_std_fmt.rs \
      -o target/reader-class14/no_std_fmt
./target/reader-class14/no_std_fmt
printf 'exit=%s\n' "$?"
readelf -hW target/reader-class14/no_std_fmt
readelf -lW target/reader-class14/no_std_fmt
```

Expected stdout is `sum=42 [2, 3] 1.50` followed by a newline, and exit status
42 is intentional. Capture that status immediately; a shell configured with
`set -e` must allow the nonzero exit before checking it. The absolute linker
path prevents executable-search ambiguity. `linker-flavor=ld` selects the
argument language implemented by this driver. No musl target or cross compiler
is needed: the fixture uses the installed native target's `core` and
`compiler_builtins`, without linking libc.

The fixture supplies its own process entry shim. Linux jumps to `_start`
without a function-call return address. Its naked assembly aligns `rsp` to 16
bytes, then calls `rust_entry`, giving compiler-generated code the ordinary
x86-64 System V call-entry condition `(rsp + 8) % 16 == 0`. This startup
obligation is separate from `no_std`, name mangling, and the panic strategy.


Linking it pulls in `core`'s formatting machinery: thousands of relocations,
GOT references, data pointers that become RELATIVE records, and mergeable
constant pools.

## Acceptance and references

Native tests link the Rust program through `rustc` and check its output, exit
status over several ASLR runs, and program headers; link assembly with a
`-l` library from a `-L` directory, directly and through a response file;
reject a
missing library, `-shared`, and an unresolved reference; and show that
COMDAT, mergeable, and `.eh_frame` sections that class11's policy rejects link
under the driver's policy.

- [rustc: codegen options `linker` and `linker-flavor`](https://doc.rust-lang.org/rustc/codegen-options/index.html)
- [GNU ld command-line options](https://sourceware.org/binutils/docs/ld/Options.html)
- [The Embedded Rust Book: `#![no_std]`](https://docs.rust-embedded.org/book/intro/no-std.html)

## Cumulative validation

A combined assembly fixture uses a real GOT load, initialized TLS, duplicate
mergeable constants, two foldable functions, and independently checked debug
references. It verifies one FOLD row, shared constants, folded-copy tombstones,
PIE RVAs, STT_TLS module offsets and section identities, five program headers
including PT_TLS, CLI/library byte equality, and repeated execution. Runtime
unwind sections are deliberately omitted by the driver policy.

A separate `rustc -C debuginfo=2` build of the real no_std fixture must execute
correctly, decode without DWARF warnings, and support a GDB source breakpoint
at `rust_entry`. The assembly `_start` shim establishes the call-entry ABI;
`rust_entry` is the compiler-generated function with source-line information. Debug information is not a promise of runtime exception unwinding.
The metadata and source-line checks exercise the class12 path that an executable
exit-status check alone cannot establish.

## Snapshot boundary

This directory is a complete, independently buildable project. Earlier modules
contain reference implementations; this assignment owns `src/driver.rs`.
The public interfaces stay stable so you can substitute your own earlier modules.

Run `cargo test --test prerequisites` from this directory to check the supplied
foundation independently of this assignment.
