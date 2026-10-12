use class14::relocate::*;
#[test]
fn relocation_validation_priority_and_atomicity() {
    for (offset, kind, expected) in [
        (u64::MAX, 999, Error::UnsupportedRelocation(999)),
        (u64::MAX, 10, Error::PatchBounds),
        (6, 10, Error::PatchBounds),
        (0, 10, Error::RelocationOverflow),
    ] {
        let mut bytes = [0xa5; 8];
        assert_eq!(
            apply(&mut bytes, offset, kind, u64::MAX, 0, 0),
            Err(expected)
        );
        assert_eq!(bytes, [0xa5; 8], "failure must not partially write a field");
    }
}
#[test]
fn aligned_contributions() {
    let m = merge(&[
        CodeSection {
            file: 0,
            section: 1,
            alignment: 1,
            bytes: &[1, 2, 3],
        },
        CodeSection {
            file: 1,
            section: 1,
            alignment: 8,
            bytes: &[4, 5],
        },
        CodeSection {
            file: 1,
            section: 2,
            alignment: 0,
            bytes: &[],
        },
    ])
    .unwrap();
    assert_eq!(m.bytes, vec![1, 2, 3, 0, 0, 0, 0, 0, 4, 5]);
    assert_eq!(m.alignment, 8);
    assert_eq!(m.offsets[&(0, 1)], 0);
    assert_eq!(m.offsets[&(1, 1)], 8);
    assert_eq!(m.offsets[&(1, 2)], 10);
}
#[test]
fn merge_rejections() {
    assert_eq!(merge(&[]), Err(Error::EmptyCode));
    assert_eq!(
        merge(&[CodeSection {
            file: 0,
            section: 1,
            alignment: 3,
            bytes: &[1]
        }]),
        Err(Error::InvalidAlignment)
    );
    assert_eq!(
        merge(&[CodeSection {
            file: 0,
            section: 1,
            alignment: 8192,
            bytes: &[1]
        }]),
        Err(Error::InvalidAlignment)
    );
    let code = [
        CodeSection {
            file: 0,
            section: 1,
            alignment: 1,
            bytes: &[1],
        },
        CodeSection {
            file: 0,
            section: 1,
            alignment: 1,
            bytes: &[],
        },
    ];
    assert_eq!(merge(&code), Err(Error::DuplicateSection));
}
#[test]
fn signed_and_absolute_patches() {
    let mut b = [0xa5; 16];
    apply(&mut b, 4, 2, 0x400100, -4, 0x400110).unwrap();
    assert_eq!(&b[4..8], &(-20i32).to_le_bytes());
    assert_eq!(&b[..4], &[0xa5; 4]);
    assert_eq!(&b[8..], &[0xa5; 8]);
    apply(&mut b, 4, 4, 0x400100, -4, 0x400110).unwrap();
    assert_eq!(&b[4..8], &(-20i32).to_le_bytes());
    apply(&mut b, 8, 1, u64::MAX, -1, 0).unwrap();
    assert_eq!(&b[8..], &(u64::MAX - 1).to_le_bytes());
    apply(&mut b, 0, 10, u32::MAX as u64, 0, 0).unwrap();
    assert_eq!(&b[..4], &u32::MAX.to_le_bytes());
    apply(&mut b, 0, 11, 0, -1, 0).unwrap();
    assert_eq!(&b[..4], &(-1i32).to_le_bytes());
}
#[test]
fn exact_signed_boundaries() {
    let mut b = [0; 4];
    apply(&mut b, 0, 2, i32::MAX as u64, 0, 0).unwrap();
    assert_eq!(b, i32::MAX.to_le_bytes());
    apply(&mut b, 0, 2, 0, 0, 1u64 << 31).unwrap();
    assert_eq!(b, i32::MIN.to_le_bytes());
    for (kind, s, a, p) in [
        (2, 1u64 << 31, 0, 0),
        (2, 0, 0, (1u64 << 31) + 1),
        (10, 1u64 << 32, 0, 0),
        (10, 0, -1, 0),
        (11, 1u64 << 31, 0, 0),
    ] {
        let mut b = [0x55; 8];
        assert_eq!(
            apply(&mut b, 0, kind, s, a, p),
            Err(Error::RelocationOverflow)
        );
        assert_eq!(b, [0x55; 8]);
    }
}
#[test]
fn patch_bounds_and_unknown_kind() {
    for (offset, kind) in [(1, 1), (5, 2), (u64::MAX, 1)] {
        let mut b = [7; 8];
        assert_eq!(
            apply(&mut b, offset, kind, 0, 0, 0),
            Err(Error::PatchBounds)
        );
        assert_eq!(b, [7; 8]);
    }
    let mut b = [7; 8];
    assert_eq!(
        apply(&mut b, 0, 999, 0, 0, 0),
        Err(Error::UnsupportedRelocation(999))
    );
    apply(&mut b, u64::MAX, 0, u64::MAX, i64::MAX, 0).unwrap();
    assert_eq!(b, [7; 8]);
}

#[test]
fn full_width_modular_arithmetic() {
    let mut b = [0; 8];
    apply(&mut b, 0, 1, u64::MAX, 1, 0).unwrap();
    assert_eq!(b, [0; 8]);
    apply(&mut b, 0, 1, 0, -1, 0).unwrap();
    assert_eq!(b, u64::MAX.to_le_bytes());
    apply(&mut b, 0, 11, u64::MAX - 15, 0, 0).unwrap();
    assert_eq!(&b[..4], &(-16i32).to_le_bytes());
}
