# Class 15: Capstone, investigate linker robustness

Implementation: [`src/fuzz.rs`](src/fuzz.rs).


A linker reads files it did not write: build caches, downloaded archives,
output of buggy tools. Malformed or unsupported inputs should receive a
diagnostic rather than cause a panic, an out-of-bounds access, or a hang.
Supported valid inputs should link deterministically. A mutation may still
be valid, touch irrelevant padding, or leave the original bytes unchanged;
it need not be rejected. Tests written while implementing a feature
check the inputs you thought of. This class explores additional inputs by mutating valid objects deterministically
and running the selected class10, class11, class13 ET_EXEC/PIE, and class14
backends on the results. Passing a finite survey is evidence for these samples
and mutations, not a proof for every input or every failure mode.

Passing usually means returning to earlier classes and fixing them: an
unchecked slice in class1, an `unwrap` on a placement in class9, an addition
that overflows in class12. That is the point of a capstone. The reference
implementation passes the same suite.

## Deliverables

Implement C15.1–C15.2 in [`src/fuzz.rs`](src/fuzz.rs). C15.3 has no new code of its own: it
grades the whole linker you have built.

| Task | Responsibility | Points |
|---|---|---|
| C15.1 | Deterministic generator and mutator | 20 |
| C15.2 | Survey harness that records panics | 20 |
| C15.3 | Whole-linker robustness, determinism, and differential checks | 60 |

Run `python3 scripts/grade.py class15` on native x86-64 Linux. The native tests
need everything earlier classes needed, including stable `rustc`. They take a
few minutes in debug builds.

## C15.1: generator and mutator

`Rng` is xorshift64*, specified in the API documentation, so that a failing
case is identified by its seed and iteration and can be replayed exactly.
`below(n)` is `next_u64() % n`.

`mutate` copies the input; an empty input is returned unchanged. Otherwise it
draws `below(5)` to choose one operation, then draws its parameters in the
order listed:

| Draw | Operation |
|---|---|
| 0 | `at = below(len)`, `bit = below(8)`: flip that bit |
| 1 | `at = below(len)`, then set that byte to one of `00 01 7f 80 ff` chosen by `below(5)` |
| 2 | if `len >= 4`: `at = 4 * below(len / 4)`, then write one of 0, 1, `0x7fff_ffff`, `0x8000_0000`, `0xffff_ffff`, `len` (as u32), chosen by `below(6)`, little-endian; else nothing |
| 3 | if `len >= 8`: `at = 8 * below(len / 8)`, then write one of 0, 1, `i64::MAX`, `1 << 63`, `u64::MAX`, `len` (as u64), chosen by `below(6)`; else nothing |
| 4 | truncate to `below(len)` bytes |

Boundary values are where length and offset arithmetic overflows, and
aligned 4- and 8-byte writes hit ELF header and table fields directly, which
is why this small menu finds real bugs quickly.

## C15.2: survey

`survey` creates `Rng::new(seed)` and, for iteration `i` from 0, uses sample
`i % corpus.len()`, chooses one of its inputs with `below(input count)`,
mutates it with the same generator, and calls `link` with the sample's inputs
in order, the chosen one replaced. Each call runs under `catch_unwind`.
`Ok(true)` counts as accepted, `Ok(false)` as rejected, and a panic is
recorded with its iteration, sample, input, and message: the payload if it
is a `&str` or `String`, otherwise `<non-string panic>`. An empty corpus or zero iterations returns an empty report without calling
the linker. Every nonempty survey sample must contain at least one input.
Count each invocation exactly once: accepted + rejected + panics.len() must
equal the iteration count.

Install a silent panic hook for the duration and restore the previous one
afterwards, so expected panics do not flood the output. The hook is
process-wide; tests that need their own panic output should not run a survey
concurrently. `catch_unwind` catches unwinding Rust panics, not aborts,
process signals, hangs, or allocation exhaustion. This in-process harness does
not provide isolation for those failures; sizes must still be checked before
allocation. A harness investigating them needs process isolation and budgets.

## C15.3: what is graded

- 5,000 mutated links over samples with merged strings and unwind
  tables, TLS with a GOT-using archive member, and debug information with
  foldable functions, through class10, class11, both class13 ET_EXEC/PIE
  entries, and the class14
  policy, with no panics. Each valid sample first links with the backend that
  supports it.
- Mutated `rustc` output and the real `core` and `compiler_builtins` rlibs,
  through class15::driver::link with explicit abort-only lowering, with no panics.
- Linking twice, and linking copies of the inputs at different addresses,
  produces identical output for class10, class11, and class13, and the
  results run.
- Three C programs give the same exit status and output when linked by class13
  and by GNU ld.

A failure message names the backend, seed, iteration, sample, and input.
Keep the corpus order and source version with that information. Replay from
iteration zero with the seed, consuming the sample-input selection draw and
all mutation draws at every iteration through the failure. Mutating only the
failing sample after reseeding changes the generator state. Preserve the
resulting bytes when reducing a counterexample.

### Replay example: a draw for `below(1)` still advances the generator

The `survey_counts_outcomes_and_catches_panics` contract uses this corpus with
seed 9:

```text
sample 0: [b"first input", b"second"]
sample 1: [b"alone"]
```

Its synthetic backend accepts every two-input call and panics on a one-input
call whose bytes differ from `b"alone"`. This isolates the harness's replay
behavior from ELF parsing. The first two iterations consume these draws:

| Iteration | Draw | Returned u64 (hex) | Use and result |
|---|---|---|---|
| 0 | 1 | `d19729ca73ad803f` | Input selection: `% 2 = 1`, selects `second` |
| 0 | 2 | `0f9b119e0f1e483f` | Operation: `% 5 = 0`, bit flip |
| 0 | 3 | `8c0bd398c6dd4166` | Byte offset: `% 6 = 2` |
| 0 | 4 | `8820d6e1cb9bf685` | Bit: `% 8 = 5`, changes `c` (0x63) to `C` (0x43) |
| 1 | 5 | `c637fc958dfc6579` | Input selection: `% 1 = 0`, selects `alone` |
| 1 | 6 | `54c97e6e3c3a90f9` | Operation: `% 5 = 2`, aligned four-byte write |
| 1 | 7 | `ef8e36f21194fe91` | Write position: `4 * (% 1) = 0` |
| 1 | 8 | `2c49589366cc46ce` | Value choice: `% 6 = 4`, writes u32::MAX |

Iteration 0 passes `[b"first input", b"seCond"]` and is accepted. Iteration 1
passes the single input `ff ff ff ff 65`; its length remains five. The report
records iteration 1, sample 1, input 0, and message `bad 5`.

Both draws whose modulus is one must still occur. Returning zero without
advancing `Rng`, reseeding for each sample, or beginning replay directly at
iteration 1 changes later mutations. Each iteration starts from a copy of the
original corpus input; mutations do not accumulate in the corpus. Reproduce the
eight draws before comparing the failure, then retain the exact bytes if
reducing the case. This example establishes reproducible harness behavior;
it says nothing about whether those bytes are a valid ELF object.

The native suite first links unmodified samples with backends supporting them.
No panics and at least one rejection would also be satisfied by a backend
that rejects everything; these checks need the valid-input baseline. Accepted
mutations are not all executed: their semantic correctness is not proven by
the survey count. The determinism and differential fixtures test separate
properties on their valid, runnable inputs.

Panics are not the only failure mode. A malformed size can also make a
linker allocate a huge buffer or loop for a long time. The earlier classes'
size limits (`class15::executable::MAX_IMAGE`, the GOT and table bounds) exist for that
reason; keep checking sizes before allocating.

## References

- [AFL: interesting values and deterministic mutation stages](https://lcamtuf.coredump.cx/afl/technical_details.txt)
- [Rust: `std::panic::catch_unwind`](https://doc.rust-lang.org/std/panic/fn.catch_unwind.html)
- [Marsaglia, "Xorshift RNGs" (2003)](https://www.jstatsoft.org/article/view/v008i14)

## Zero iterations means no execution

With iterations=0, return an empty report without invoking the linker or changing the corpus. This is not evidence of robustness; the report must reflect that no cases ran. Completion still needs seeded, replayable failures, differential checks, and native execution tests.

Contract coverage: `zero_iterations_do_not_call_linker_or_change_corpus`.

## Method map: make robustness measurable

`Rng` is a deterministic xorshift64* generator. A fixed seed is part of the test artifact: without it a panic cannot be replayed. `mutate` applies one bounded mutation from the documented menu while preserving the input ownership rules; it may shorten or corrupt bytes but never grows beyond the source.

`survey` chooses corpus/sample/input/iteration in a fixed schedule, catches both string and non-string panic payloads, silences the panic hook for the duration, and records enough coordinates to replay the exact case. Zero iterations must call the linker zero times and return an empty report. C15.3 then runs these mutations through the full backend and compares behavior with GNU ld.

## Snapshot boundary

This directory is a complete, independently buildable project. Earlier modules
contain reference implementations; this assignment owns `src/fuzz.rs`.
The public interfaces stay stable so you can substitute your own earlier modules.

Run `cargo test --test prerequisites` from this directory to check the supplied
foundation independently of this assignment.
