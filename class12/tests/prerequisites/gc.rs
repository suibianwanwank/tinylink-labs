use class12::gc::{Error, Graph, MapRow, SectionId, Target};
use std::collections::{BTreeMap, BTreeSet};
fn id(section: u16) -> SectionId {
    SectionId { file: 0, section }
}
#[test]
fn all_roots_are_validated_before_traversal() {
    let graph = Graph::from([(id(1), vec![Target::Undefined(b"reachable".to_vec())])]);
    assert_eq!(
        class12::gc::trace(&graph, &[id(1), id(99), id(98)]),
        Err(Error::MissingNode(id(99)))
    );
    assert_eq!(
        class12::gc::trace(&graph, &[id(1)]),
        Err(Error::Undefined(b"reachable".to_vec()))
    );
}
#[test]
fn roots_cycles_and_storage() {
    let g = Graph::from([
        (
            id(1),
            vec![Target::Section(id(2)), Target::Common(b"shared".to_vec())],
        ),
        (id(2), vec![Target::Section(id(1)), Target::Absolute]),
        (id(3), vec![Target::Common(b"dead".to_vec())]),
    ]);
    let live = class12::gc::trace(&g, &[id(1), id(1)]).unwrap();
    assert_eq!(live.sections, BTreeSet::from([id(1), id(2)]));
    assert_eq!(live.commons, BTreeSet::from([b"shared".to_vec()]));
    assert!(class12::gc::trace(&g, &[]).unwrap().sections.is_empty());
}
#[test]
fn dead_errors_are_not_live_errors() {
    let g = Graph::from([
        (id(1), vec![Target::Absolute]),
        (id(2), vec![Target::Undefined(b"missing".to_vec())]),
        (id(3), vec![Target::Invalid]),
        (id(4), vec![Target::Section(id(99))]),
    ]);
    assert_eq!(class12::gc::trace(&g, &[id(1)]).unwrap().sections.len(), 1);
    assert_eq!(
        class12::gc::trace(&g, &[id(2)]),
        Err(Error::Undefined(b"missing".to_vec()))
    );
    assert_eq!(class12::gc::trace(&g, &[id(3)]), Err(Error::InvalidSymbol));
    assert_eq!(
        class12::gc::trace(&g, &[id(4)]),
        Err(Error::MissingNode(id(99)))
    );
    assert_eq!(
        class12::gc::trace(&g, &[id(1), id(88)]),
        Err(Error::MissingNode(id(88)))
    );
}
#[test]
fn traversal_order_and_deep_graph() {
    let mut g = Graph::from([
        (
            id(1),
            vec![Target::Section(id(2)), Target::Undefined(b"later".to_vec())],
        ),
        (id(2), vec![Target::Undefined(b"first".to_vec())]),
    ]);
    assert_eq!(
        class12::gc::trace(&g, &[id(1)]),
        Err(Error::Undefined(b"first".to_vec()))
    );
    // A recursive implementation can exhaust the Rust test thread's stack.
    g.clear();
    for i in 0..30000u16 {
        g.insert(id(i), vec![Target::Section(id(i + 1))]);
    }
    g.insert(id(30000), vec![]);
    assert_eq!(
        class12::gc::trace(&g, &[id(0)]).unwrap().sections.len(),
        30001
    );
}
#[test]
fn map_is_ordered_and_byte_safe() {
    let rows = vec![
        MapRow {
            id: id(3),
            name: b".dead\n\xff".to_vec(),
            size: 19,
            address: None,
        },
        MapRow {
            id: SectionId {
                file: 1,
                section: 1,
            },
            name: b"a\\b\t".to_vec(),
            size: 0,
            address: Some(0x402000),
        },
        MapRow {
            id: id(1),
            name: b".text.start".to_vec(),
            size: 12,
            address: Some(0x400120),
        },
    ];
    let commons = BTreeMap::from([(b"z".to_vec(), 0x403010), (b"a b".to_vec(), 0x403000)]);
    assert_eq!(
        class12::gc::render_map(&rows, &commons),
        concat!(
            "TINYLINK MAP 1\n",
            "KEEP 0:1 0x0000000000400120 12 .text.start\n",
            "DROP 0:3 - 19 .dead\\x0a\\xff\n",
            "KEEP 1:1 0x0000000000402000 0 a\\x5cb\\x09\n",
            "COMMON 0x0000000000403000 a\\x20b\n",
            "COMMON 0x0000000000403010 z\n",
        )
    );
    let first = class12::gc::render_map(&rows, &commons);
    let mut reversed = rows;
    reversed.reverse();
    assert_eq!(class12::gc::render_map(&reversed, &commons), first);
    assert_eq!(
        class12::gc::render_map(
            &[
                MapRow {
                    id: id(0),
                    name: vec![],
                    size: 0,
                    address: None
                },
                MapRow {
                    id: id(1),
                    name: b"-".to_vec(),
                    size: 0,
                    address: None
                },
            ],
            &BTreeMap::new()
        ),
        "TINYLINK MAP 1\nDROP 0:0 - 0 -\nDROP 0:1 - 0 \\x2d\n"
    );
    assert_eq!(
        class12::gc::render_map(&[], &BTreeMap::new()),
        "TINYLINK MAP 1\n"
    );
}
