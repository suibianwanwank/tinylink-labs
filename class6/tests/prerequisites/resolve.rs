use class6::resolve::*;
use class6::symbols::{Definition, Symbol};

#[test]
fn validation_precedes_symbol_selection() {
    for (binding, expected) in [
        (3, Error::UnsupportedBinding(id(0, 0))),
        (2, Error::UnsupportedCommonBinding(id(0, 0))),
        (1, Error::InvalidCommonAlignment(id(0, 0))),
    ] {
        assert_eq!(
            build_globals(&[vec![sym(b"", binding, Definition::Common, 3, 0)]]),
            Err(expected)
        );
    }
    // A strong definition does not excuse malformed metadata in a later COMMON.
    let files = vec![
        vec![sym(b"x", 1, Definition::Absolute, 0, 0)],
        vec![sym(b"x", 1, Definition::Common, 3, 0)],
    ];
    assert_eq!(
        build_globals(&files),
        Err(Error::InvalidCommonAlignment(id(1, 0)))
    );
}

#[test]
fn first_conflict_follows_input_order() {
    let files = vec![
        vec![
            sym(b"z", 1, Definition::Absolute, 1, 0),
            sym(b"a", 1, Definition::Absolute, 2, 0),
        ],
        vec![
            sym(b"z", 1, Definition::Absolute, 3, 0),
            sym(b"a", 1, Definition::Absolute, 4, 0),
        ],
    ];
    assert_eq!(
        build_globals(&files),
        Err(Error::Duplicate {
            name: b"z".to_vec(),
            first: id(0, 0),
            second: id(1, 0)
        })
    );
}
fn sym(name: &[u8], binding: u8, definition: Definition, value: u64, size: u64) -> Symbol<'_> {
    Symbol {
        name,
        binding,
        definition,
        value,
        size,
        kind: 0,
        visibility: 0,
    }
}
fn id(file: usize, symbol: usize) -> SymbolId {
    SymbolId { file, symbol }
}
#[test]
fn strong_replaces_weak_in_both_orders() {
    for reverse in [false, true] {
        let mut files = vec![
            vec![sym(b"f", 2, Definition::Section(1), 0, 0)],
            vec![sym(b"f", 1, Definition::Section(2), 9, 0)],
        ];
        if reverse {
            files.reverse();
        }
        let winner = if reverse { id(0, 0) } else { id(1, 0) };
        let g = build_globals(&files).unwrap();
        assert_eq!(g[b"f".as_slice()], Selection::Definition(winner));
        for f in 0..2 {
            assert_eq!(
                bind(&files, &g, id(f, 0)),
                Ok(Binding::Selected(Selection::Definition(winner)))
            );
        }
    }
}
#[test]
fn first_weak_and_duplicate_strong() {
    let files = vec![
        vec![sym(b"f", 2, Definition::Absolute, 17, 0)],
        vec![sym(b"f", 2, Definition::Section(1), 0, 0)],
    ];
    assert_eq!(
        build_globals(&files).unwrap()[b"f".as_slice()],
        Selection::Definition(id(0, 0))
    );
    let files = vec![
        vec![sym(b"f", 1, Definition::Absolute, 17, 0)],
        vec![sym(b"f", 1, Definition::Section(1), 0, 0)],
    ];
    assert_eq!(
        build_globals(&files),
        Err(Error::Duplicate {
            name: b"f".to_vec(),
            first: id(0, 0),
            second: id(1, 0)
        })
    );
}
#[test]
fn common_merge_and_precedence() {
    let mut files = vec![
        vec![sym(b"x", 2, Definition::Section(1), 0, 0)],
        vec![sym(b"x", 1, Definition::Common, 16, 4)],
        vec![sym(b"x", 1, Definition::Common, 8, 32)],
    ];
    assert_eq!(
        build_globals(&files).unwrap()[b"x".as_slice()],
        Selection::Common {
            first: id(1, 0),
            size: 32,
            alignment: 16
        }
    );
    files.push(vec![sym(b"x", 1, Definition::Section(2), 0, 1)]);
    assert_eq!(
        build_globals(&files).unwrap()[b"x".as_slice()],
        Selection::Definition(id(3, 0))
    );
    files.reverse();
    assert_eq!(
        build_globals(&files).unwrap()[b"x".as_slice()],
        Selection::Definition(id(0, 0))
    );
}
#[test]
fn local_names_never_collide() {
    let files = vec![
        vec![sym(b"x", 0, Definition::Section(1), 0, 0)],
        vec![
            sym(b"x", 0, Definition::Section(2), 0, 0),
            sym(b"x", 1, Definition::Absolute, 23, 0),
        ],
    ];
    let g = build_globals(&files).unwrap();
    assert_eq!(g.len(), 1);
    for (file, symbol) in [(0, 0), (1, 0)] {
        assert_eq!(
            bind(&files, &g, id(file, symbol)),
            Ok(Binding::Selected(Selection::Definition(id(file, symbol))))
        );
    }
}
#[test]
fn unresolved_is_checked_on_use() {
    let files = vec![vec![
        sym(b"missing", 1, Definition::Undefined, 0, 0),
        sym(b"hook", 2, Definition::Undefined, 0, 0),
        sym(b"", 0, Definition::Undefined, 0, 0),
    ]];
    let g = build_globals(&files).unwrap();
    assert!(g.is_empty());
    assert_eq!(
        bind(&files, &g, id(0, 0)),
        Err(Error::Undefined {
            name: b"missing".to_vec(),
            reference: id(0, 0)
        })
    );
    assert_eq!(bind(&files, &g, id(0, 1)), Ok(Binding::Zero));
    assert!(matches!(
        bind(&files, &g, id(0, 2)),
        Err(Error::Undefined { .. })
    ));
    assert_eq!(
        bind(&files, &g, id(1, 0)),
        Err(Error::InvalidIndex(id(1, 0)))
    );
    assert_eq!(
        bind(&files, &g, id(0, 3)),
        Err(Error::InvalidIndex(id(0, 3)))
    );
}
#[test]
fn malformed_symbols_are_rejected() {
    for (binding, definition, value, expected) in [
        (
            3,
            Definition::Undefined,
            0,
            Error::UnsupportedBinding(id(0, 0)),
        ),
        (
            2,
            Definition::Common,
            8,
            Error::UnsupportedCommonBinding(id(0, 0)),
        ),
        (
            1,
            Definition::Common,
            3,
            Error::InvalidCommonAlignment(id(0, 0)),
        ),
    ] {
        assert_eq!(
            build_globals(&[vec![sym(b"x", binding, definition, value, 0)]]),
            Err(expected)
        );
    }
    assert_eq!(
        build_globals(&[vec![sym(b"", 1, Definition::Undefined, 0, 0)]]),
        Err(Error::EmptyGlobalName(id(0, 0)))
    );
    let files = vec![vec![sym(b"\xff", 1, Definition::Common, 0, 0)]];
    assert_eq!(
        build_globals(&files).unwrap()[b"\xff".as_slice()],
        Selection::Common {
            first: id(0, 0),
            size: 0,
            alignment: 1
        }
    );
}
