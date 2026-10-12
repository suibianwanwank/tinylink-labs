# Course design

## Design references

The public [CMU 15-445 Project 0 specification](https://15445.courses.cs.cmu.edu/fall2025/project0/)
provides concrete API tasks, constraints, testing instructions, and grading criteria.
The [CMU 15-213 C Programming Lab handout](https://www.cs.cmu.edu/~213/labs/cprogramminglab.pdf)
likewise separates the programming contract from supplied support and evaluation.
These are design references, not source material to copy. No CMU tests, assignment
code, branding, or institutional grading claims are included here.

## Principles

1. One evolving program: all milestones contribute to a linker or an explicitly
   identified extension of its output pipeline.
2. Contract before code: accepted inputs, errors, ordering, and unsupported cases
   are stated before a student implements an algorithm.
3. Student work is substantive: parsing, resolution, layout, relocation, archive
   selection, and optimization. File copying and command-line boilerplate are not
   promoted to standalone projects.
4. Supplied tests are observable specifications, not the implementation in disguise.
   Boundary cases and independent oracles matter more than raw test counts.
5. Completion requires a cumulative end-to-end program, not isolated helpers only.
6. Completed prerequisite references ship inside each snapshot. Current-task
   references validate feasibility privately. Student builds have no answer feature,
   fallback to a system linker, or dependency on a solved final-stage crate.
7. The course language is English. Technical precision and realistic interfaces
   take precedence over conversational transitions and motivational filler.

## Assignment size

Split a stage when it introduces multiple independently substantial algorithms.
Do not force the old article numbering onto the new dependency graph. Preserve
coverage through the syllabus mapping rather than retaining an incoherent order.
