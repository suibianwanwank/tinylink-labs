use class7::layout::*;
#[test]
fn metadata_validation_precedes_packing() {
    assert_eq!(
        plan(&[c(Kind::Bss, 1, 3, &[], 0)], &[]),
        Err(Error::InvalidAlignment)
    );
    assert_eq!(
        plan(&[c(Kind::Data, 1, 1, &[1], 2)], &[]),
        Err(Error::InvalidSize)
    );
    assert_eq!(
        plan(
            &[],
            &[Common {
                name: b"",
                alignment: 3,
                size: 0
            }]
        ),
        Err(Error::InvalidAlignment)
    );
    assert_eq!(
        plan(
            &[],
            &[Common {
                name: b"",
                alignment: 1,
                size: 0
            }]
        ),
        Err(Error::InvalidSize)
    );
    // Validate later metadata before trying to allocate an earlier huge BSS.
    assert_eq!(
        plan(
            &[
                c(Kind::Bss, 1, 1, &[], u64::MAX),
                c(Kind::Text, 2, 3, &[1], 1)
            ],
            &[]
        ),
        Err(Error::InvalidAlignment)
    );
}
fn c(kind: Kind, section: u16, alignment: u64, bytes: &[u8], size: u64) -> Contribution<'_> {
    Contribution {
        file: 0,
        section,
        kind,
        alignment,
        bytes,
        memory_size: size,
    }
}
#[test]
fn separate_file_and_memory() {
    let p = plan(
        &[
            c(Kind::Data, 3, 8, &[3; 8], 8),
            c(Kind::Bss, 4, 32, &[], 64),
            c(Kind::Text, 1, 16, &[1; 7], 7),
            c(Kind::ReadOnly, 2, 4, &[2; 4], 4),
        ],
        &[Common {
            name: b"large",
            alignment: 4096,
            size: 8192,
        }],
    )
    .unwrap();
    assert_eq!(
        p.placements[&(0, 1)].address,
        class7::executable::BASE + 288
    );
    assert_eq!(p.placements[&(0, 2)].file_offset, Some(4096));
    assert_eq!(p.placements[&(0, 3)].file_offset, Some(8192));
    assert_eq!(p.placements[&(0, 4)].file_offset, None);
    assert_eq!(
        p.placements[&(0, 4)].address,
        class7::executable::BASE + 8224
    );
    assert_eq!(
        p.common_addresses[b"large".as_slice()],
        class7::executable::BASE + 12288
    );
    assert_eq!(p.bytes.len(), 8200);
    assert_eq!(
        p.segments,
        [
            Segment {
                flags: 5,
                offset: 0,
                file_size: 295,
                memory_size: 295
            },
            Segment {
                flags: 4,
                offset: 4096,
                file_size: 4,
                memory_size: 4
            },
            Segment {
                flags: 6,
                offset: 8192,
                file_size: 8,
                memory_size: 12288
            }
        ]
    );
}
#[test]
fn stable_kind_order_and_empty_groups() {
    let p = plan(
        &[c(Kind::Text, 3, 64, &[3], 1), c(Kind::Text, 1, 32, &[1], 1)],
        &[],
    )
    .unwrap();
    assert_eq!(p.placements[&(0, 3)].file_offset, Some(320));
    assert_eq!(p.placements[&(0, 1)].file_offset, Some(352));
    assert_eq!(&p.bytes[321..352], &[0; 31]);
    assert_eq!(p.segments[1].offset, 4096);
    assert_eq!(p.segments[1].memory_size, 0);
    assert_eq!(p.segments[2].offset, 4096);
    assert_eq!(p.bytes.len(), 4096);
}
#[test]
fn validation_and_resource_bounds() {
    assert_eq!(plan(&[], &[]), Err(Error::EmptyText));
    assert_eq!(
        plan(&[c(Kind::Text, 1, 3, &[1], 1)], &[]),
        Err(Error::InvalidAlignment)
    );
    assert_eq!(
        plan(&[c(Kind::Text, 1, 1, &[1], 2)], &[]),
        Err(Error::InvalidSize)
    );
    assert_eq!(
        plan(&[c(Kind::Bss, 1, 1, &[1], 1)], &[]),
        Err(Error::InvalidSize)
    );
    assert_eq!(
        plan(
            &[c(Kind::Text, 1, 1, &[1], 1), c(Kind::Bss, 1, 1, &[], 1)],
            &[]
        ),
        Err(Error::DuplicateIdentity)
    );
    assert_eq!(
        plan(
            &[
                c(Kind::Text, 1, 1, &[1], 1),
                c(Kind::Bss, 2, 1, &[], u64::MAX)
            ],
            &[]
        ),
        Err(Error::TooLarge)
    );
    assert_eq!(
        plan(
            &[c(Kind::Text, 1, 1, &[1], 1)],
            &[Common {
                name: b"x",
                alignment: 1,
                size: class7::executable::MAX_IMAGE
            }]
        ),
        Err(Error::TooLarge)
    );
}
fn u64at(b: &[u8], i: usize) -> u64 {
    u64::from_le_bytes(b[i..i + 8].try_into().unwrap())
}
fn u32at(b: &[u8], i: usize) -> u32 {
    u32::from_le_bytes(b[i..i + 4].try_into().unwrap())
}
#[test]
fn headers_preserve_payload_and_permissions() {
    let mut p = plan(
        &[
            c(Kind::Text, 1, 16, &[0x90, 0xc3], 2),
            c(Kind::Data, 2, 8, &[1; 8], 8),
            c(Kind::Bss, 3, 16, &[], 32),
        ],
        &[],
    )
    .unwrap();
    let data = p.placements[&(0, 2)].file_offset.unwrap() as usize;
    p.bytes[data] = 99;
    let entry = p.placements[&(0, 1)].address + 1;
    let segments = p
        .segments
        .iter()
        .map(|s| (s.flags, s.offset, s.file_size, s.memory_size))
        .collect::<Vec<_>>();
    let b = emit(p, entry).unwrap();
    assert_eq!(u64at(&b, 24), entry);
    assert_eq!(&b[56..58], &4u16.to_le_bytes());
    for (i, (flags, offset, filesz, memsz)) in segments.into_iter().enumerate() {
        let h = 64 + i * 56;
        assert_eq!(u32at(&b, h), 1);
        assert_eq!(u32at(&b, h + 4), flags);
        assert_eq!(u64at(&b, h + 8), offset);
        assert_eq!(u64at(&b, h + 16), class7::executable::BASE + offset);
        assert_eq!(u64at(&b, h + 32), filesz);
        assert_eq!(u64at(&b, h + 40), memsz);
        assert_eq!(u64at(&b, h + 48), 4096);
    }
    assert_eq!(u32at(&b, 232), 0x6474e551);
    assert_eq!(u32at(&b, 236), 6);
    assert_eq!(b[data], 99);
    assert_eq!(&b[288..290], &[0x90, 0xc3]);
}
#[test]
fn entry_must_be_in_text_not_padding() {
    for entry in [
        class7::executable::BASE,
        class7::executable::BASE + 319,
        class7::executable::BASE + 321,
        class7::executable::BASE + 4096,
    ] {
        let p = plan(&[c(Kind::Text, 1, 64, &[0xc3], 1)], &[]).unwrap();
        assert_eq!(emit(p, entry), Err(Error::InvalidEntry));
    }
}
