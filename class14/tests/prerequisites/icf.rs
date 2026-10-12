//! Contract tests for identical code folding.
use class14::debug::Aliases;
use class14::gc::SectionId;
use class14::icf::{Candidate, Edge, Reference};

fn id(file: usize, section: u16) -> SectionId {
    SectionId { file, section }
}
fn candidate(id: SectionId, bytes: &[u8], references: Vec<Reference>) -> Candidate<'_> {
    Candidate {
        id,
        bytes,
        alignment: 16,
        references,
        unwind: Vec::new(),
        significant: false,
    }
}
/// A PLT32 call at offset 1 to the start of `target`.
fn call(target: SectionId) -> Reference {
    Reference {
        offset: 1,
        kind: 4,
        addend: -4,
        target: Edge::Section {
            section: target,
            offset: 0,
        },
    }
}
fn aliases(pairs: &[(SectionId, SectionId)]) -> Aliases {
    pairs.iter().copied().collect()
}
const CALL: &[u8] = &[0xe8, 0, 0, 0, 0, 0xc3];

#[test]
fn identical_sections_fold_to_the_smallest_id() {
    let code = [0xb8, 1, 0, 0, 0, 0xc3];
    let candidates = [
        candidate(id(1, 4), &code, vec![]),
        candidate(id(0, 7), &code, vec![]),
        candidate(id(2, 1), &code, vec![]),
        candidate(id(0, 9), &[0xc3], vec![]),
    ];
    assert_eq!(
        class14::icf::fold(&candidates),
        aliases(&[(id(1, 4), id(0, 7)), (id(2, 1), id(0, 7))])
    );
    let mut aligned = candidates.clone();
    aligned[2].alignment = 32;
    assert_eq!(
        class14::icf::fold(&aligned),
        aliases(&[(id(1, 4), id(0, 7))])
    );
    let mut taken = candidates.clone();
    taken[1].significant = true;
    assert_eq!(class14::icf::fold(&taken), aliases(&[(id(2, 1), id(1, 4))]));
    assert_eq!(class14::icf::fold(&[]), Aliases::new());
}

#[test]
fn fold_compares_raw_relocated_bytes_conservatively() {
    // RELA supplies the addend separately. Initial field contents need not be
    // zero; this contract compares those raw bytes rather than normalizing them.
    let first = [0xe8, 1, 2, 3, 4, 0xc3];
    let different = [0xe8, 1, 2, 3, 9, 0xc3];
    let target = id(9, 1);
    let candidates = [
        candidate(id(0, 1), &first, vec![call(target)]),
        candidate(id(0, 2), &first, vec![call(target)]),
        candidate(id(0, 3), &different, vec![call(target)]),
    ];
    assert_eq!(
        class14::icf::fold(&candidates),
        aliases(&[(id(0, 2), id(0, 1))])
    );
}

#[test]
fn references_must_agree() {
    let other = id(9, 1);
    let reference = |kind, addend, target| Reference {
        offset: 1,
        kind,
        addend,
        target,
    };
    let at = |offset| Edge::Section {
        section: other,
        offset,
    };
    let pairs: [(Reference, Reference, bool); 7] = [
        (reference(4, -4, at(0)), reference(4, -4, at(0)), true),
        (reference(4, -4, at(0)), reference(4, -4, at(8)), false),
        (reference(4, -4, at(0)), reference(2, -4, at(0)), false),
        (reference(4, -4, at(0)), reference(4, 0, at(0)), false),
        (
            reference(1, 0, Edge::Constant(5)),
            reference(1, 0, Edge::Constant(5)),
            true,
        ),
        (
            reference(1, 0, Edge::Constant(5)),
            reference(1, 0, Edge::Constant(6)),
            false,
        ),
        (
            reference(1, 0, Edge::Common(b"a".to_vec())),
            reference(1, 0, Edge::Common(b"b".to_vec())),
            false,
        ),
    ];
    for (left, right, folds) in pairs {
        let candidates = [
            candidate(id(0, 1), CALL, vec![left.clone()]),
            candidate(id(0, 2), CALL, vec![right.clone()]),
        ];
        let expected = if folds {
            aliases(&[(id(0, 2), id(0, 1))])
        } else {
            Aliases::new()
        };
        assert_eq!(
            class14::icf::fold(&candidates),
            expected,
            "{left:?} {right:?}"
        );
    }
    let extra = [
        candidate(id(0, 1), CALL, vec![call(other)]),
        candidate(id(0, 2), CALL, vec![call(other), call(other)]),
    ];
    assert_eq!(class14::icf::fold(&extra), Aliases::new());
}

#[test]
fn folding_propagates_through_calls() {
    let leaf = [0x31, 0xc0, 0xc3];
    let candidates = [
        candidate(id(0, 1), CALL, vec![call(id(0, 3))]),
        candidate(id(0, 2), CALL, vec![call(id(0, 4))]),
        candidate(id(0, 3), &leaf, vec![]),
        candidate(id(0, 4), &leaf, vec![]),
        candidate(id(1, 1), CALL, vec![call(id(1, 3))]),
        candidate(id(1, 2), CALL, vec![call(id(1, 4))]),
        candidate(id(1, 3), &[0x90, 0xc3], vec![]),
        candidate(id(1, 4), &[0xcc, 0xc3], vec![]),
    ];
    assert_eq!(
        class14::icf::fold(&candidates),
        aliases(&[(id(0, 2), id(0, 1)), (id(0, 4), id(0, 3))])
    );
    // A chain three levels deep needs repeated refinement.
    let chain = [
        candidate(id(0, 1), CALL, vec![call(id(0, 2))]),
        candidate(id(0, 2), CALL, vec![call(id(0, 3))]),
        candidate(id(0, 3), &leaf, vec![]),
        candidate(id(1, 1), CALL, vec![call(id(1, 2))]),
        candidate(id(1, 2), CALL, vec![call(id(1, 3))]),
        candidate(id(1, 3), &[0x90, 0xc3], vec![]),
    ];
    assert_eq!(class14::icf::fold(&chain), Aliases::new());
}

#[test]
fn recursion_reaches_the_greatest_fixed_point() {
    let candidates = [
        candidate(id(0, 1), CALL, vec![call(id(0, 2))]),
        candidate(id(0, 2), CALL, vec![call(id(0, 1))]),
        candidate(id(1, 1), CALL, vec![call(id(1, 2))]),
        candidate(id(1, 2), CALL, vec![call(id(1, 1))]),
        candidate(id(2, 1), CALL, vec![call(id(2, 1))]),
        candidate(id(2, 2), CALL, vec![call(id(2, 2))]),
    ];
    // Every section is a call to a member of the same class, so one class
    // remains: all six sections are interchangeable.
    assert_eq!(
        class14::icf::fold(&candidates),
        aliases(&[
            (id(0, 2), id(0, 1)),
            (id(1, 1), id(0, 1)),
            (id(1, 2), id(0, 1)),
            (id(2, 1), id(0, 1)),
            (id(2, 2), id(0, 1)),
        ])
    );
    let mut pinned = candidates.clone();
    pinned[0].significant = true;
    // With 0:1 distinct, 1:2 (which calls the class of 1:1) can no longer be
    // shown equal to 0:2 (which calls 0:1), and so on around each cycle.
    assert_eq!(
        class14::icf::fold(&pinned),
        aliases(&[
            (id(1, 2), id(1, 1)),
            (id(2, 1), id(1, 1)),
            (id(2, 2), id(1, 1)),
        ])
    );
}

#[test]
fn significant_targets_and_input_order() {
    let leaf = [0x31, 0xc0, 0xc3];
    let mut candidates = vec![
        candidate(id(0, 1), &leaf, vec![]),
        candidate(id(0, 2), &leaf, vec![]),
        candidate(id(0, 3), CALL, vec![call(id(0, 1))]),
        candidate(id(0, 4), CALL, vec![call(id(0, 1))]),
        candidate(id(0, 5), CALL, vec![call(id(0, 2))]),
    ];
    candidates[0].significant = true;
    candidates[1].significant = true;
    let expected = aliases(&[(id(0, 4), id(0, 3))]);
    assert_eq!(class14::icf::fold(&candidates), expected);
    candidates.reverse();
    assert_eq!(class14::icf::fold(&candidates), expected);
}

#[test]
fn folding_requires_equal_runtime_unwind_descriptions() {
    use class14::icf::UnwindDescription;
    let mut candidates = vec![
        candidate(id(0, 1), &[0xc3], vec![]),
        candidate(id(0, 2), &[0xc3], vec![]),
        candidate(id(0, 3), &[0xc3], vec![]),
    ];
    let description = UnwindDescription {
        start: 0,
        cie: vec![1, 2],
        fde: vec![3, 4],
    };
    candidates[0].unwind.push(description.clone());
    candidates[1].unwind.push(description);
    assert_eq!(
        class14::icf::fold(&candidates),
        std::collections::BTreeMap::from([(id(0, 2), id(0, 1))])
    );
    candidates[1].unwind[0].fde[1] = 9;
    assert!(class14::icf::fold(&candidates).is_empty());
    candidates[1].unwind[0].fde[1] = 4;
    candidates[1].unwind[0].start = 1;
    assert!(class14::icf::fold(&candidates).is_empty());
    candidates[1].unwind[0].start = 0;
    candidates[1].unwind[0].cie[0] = 9;
    assert!(class14::icf::fold(&candidates).is_empty());
}
