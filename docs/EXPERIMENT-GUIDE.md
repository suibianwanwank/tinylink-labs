# Laboratory writing standard

The article and repository handout have different responsibilities. The article
briefly states the goal, starting point, implementation tasks, and acceptance
meaning. Link generic format knowledge, derivations, and explanatory diagrams to
the relevant theory chapter. Explain project-specific structures in the lab only
when needed to understand its task. Do not duplicate a theory tutorial in every
lab or expand it with unrelated setup instructions.

The repository README and API documentation must answer these five questions:

1. What linker problem does this function solve?
2. What coordinates and widths does it consume or produce?
3. Which invariant does it establish for the next class?
4. What special cases are deliberate, and why?
5. Which competing errors are ordered, and which priorities are intentionally unspecified?

The method map in each class README is the source of truth for the function-level
contract. The article links that contract and explains what the tests establish.
A tiny example belongs wherever it makes the concept easier to understand;
generic examples belong in theory, while project-specific coordinates may need
an example in the handout. Examples and diagrams are not a per-article quota.
Numeric constants such as
64 are never left unexplained: identify whether 64 means ELF header bytes, one
ELF64 section-header entry, a field width, an alignment, or an address value.

Do not present a test name as an explanation. State the observable behavior first,
then show the test that pins it down. Keep format rules (ELF ABI) separate from
course policy (diagnostic order, supported subset, or deterministic output).
