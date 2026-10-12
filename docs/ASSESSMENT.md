# Assessment and maintenance

## Student completion

Implement the source linked from the assignment README and any allowed private
helpers. Preserve public signatures, module boundaries, fixtures, and supplied
test assertions.
Additional student tests are welcome. The objective is correct behavior across the
specified input domain, not matching a small list of fixture bytes.

Run the stage tests and all prerequisite tests. No ignored tests, hardcoded fixture
answers, delegation to a system linker, or alternate answer-enabled build counts
as completion. Native executable checks must run on x86-64 Linux.

## Grading model

Each released class publishes task groups and point weights totaling 100. Tests
report correctness by group. A build failure earns no correctness credit. A stage
is complete only when every required group and cumulative regression passes;
partial points diagnose progress rather than certify a usable implementation.
No hidden-test service is deployed yet. Do not describe local tests as a remote
autograder or promise unpublished grading coverage.

## Instructor release audit

- Compile the unchanged starter and confirm task-specific failures.
- Run a separately stored reference against every public test in debug and release.
- Introduce representative faults and establish that tests reject them.
- Check truncation, numeric boundaries, ordering, malformed records, and failures.
- Add real compiler inputs and execute emitted files when the stage supports them.
- Record toolchain, commands, outcomes, and unresolved limitations.
- Run the repository language audit, documentation link checks, and formatting.

Each snapshot publishes completed references for its prerequisites. The current
assignment remains unfinished; its reference is validated separately before
becoming a prerequisite in the next snapshot.

## Local score report

```sh
python3 scripts/grade.py class0
python3 scripts/grade.py class1
```

The runner awards a task group's points only when every mapped test passes. It
also runs the selected snapshot's assignment and prerequisite tests in debug and release, including its native
integration tests. A 100-point local subtotal without cumulative passes is not
completion. The runner rejects non-Linux/non-x86-64 hosts and bounds each cargo
invocation to 900 seconds. It is a local feedback tool, not tamper-resistant
assessment: maintainers must run submissions against clean, trusted tests.
