use class11::pie::{Error, Fixup, PatchError, Relative, Value};
#[test]
fn competing_failures_preserve_bytes_without_fixing_priority() {
    for (kind, value) in [
        (999, Value::ImageRva(8)),
        (1, Value::ImageRva(8)),
        (10, Value::ImageRva(8)),
        (10, Value::Absolute(u64::MAX)),
    ] {
        let mut bytes = [0xa5; 8];
        let mut f = fixup(kind, value);
        f.offset = u64::MAX;
        f.writable = false;
        // This API deliberately leaves priority unspecified for competing errors.
        assert!(class11::pie::patch(&mut bytes, f).is_err());
        assert_eq!(bytes, [0xa5; 8]);
    }
}
fn fixup(kind: u32, value: Value) -> Fixup {
    Fixup {
        offset: 0,
        kind,
        value,
        addend: 0,
        place_rva: 0x1000,
        writable: true,
    }
}
#[test]
fn static_displacements_and_absolute_values() {
    let mut bytes = [0xa5; 16];
    let mut f = fixup(4, Value::ImageRva(0x1100));
    f.addend = -4;
    assert_eq!(class11::pie::patch(&mut bytes, f).unwrap(), None);
    assert_eq!(&bytes[..4], &252i32.to_le_bytes());
    assert_eq!(&bytes[4..], &[0xa5; 12]);
    f = fixup(1, Value::Absolute(u64::MAX));
    f.addend = 2;
    f.writable = false;
    assert_eq!(class11::pie::patch(&mut bytes, f).unwrap(), None);
    assert_eq!(&bytes[..8], &1u64.to_le_bytes());
    f = fixup(11, Value::Absolute(u64::MAX));
    class11::pie::patch(&mut bytes, f).unwrap();
    assert_eq!(&bytes[..4], &(-1i32).to_le_bytes());
    f = fixup(10, Value::Absolute(42));
    class11::pie::patch(&mut bytes, f).unwrap();
    assert_eq!(&bytes[..4], &42u32.to_le_bytes());
    let before = bytes;
    f.value = Value::Absolute(1 << 32);
    assert!(class11::pie::patch(&mut bytes, f).is_err());
    assert_eq!(bytes, before);
}
#[test]
fn runtime_pointer_and_failure_atomicity() {
    let mut bytes = [0xa5; 16];
    let mut f = fixup(1, Value::ImageRva(0x3000));
    f.offset = 3;
    f.addend = -8;
    assert_eq!(
        class11::pie::patch(&mut bytes, f).unwrap(),
        Some(Relative {
            offset: 0x1000,
            addend: 0x2ff8
        })
    );
    assert_eq!(&bytes[3..11], &[0; 8]);
    assert_eq!(&bytes[..3], &[0xa5; 3]);
    assert_eq!(&bytes[11..], &[0xa5; 5]);
    f.value = Value::ImageRva(1);
    f.addend = -2;
    assert_eq!(
        class11::pie::patch(&mut bytes, f).unwrap().unwrap().addend,
        -1
    );
    for offset in [9, u64::MAX] {
        f.offset = offset;
        let before = bytes;
        assert!(class11::pie::patch(&mut bytes, f).is_err());
        assert_eq!(bytes, before);
    }
}
#[test]
fn pic_restrictions_and_none() {
    for (kind, value, expected) in [
        (2, Value::Absolute(0), PatchError::NonPicReference),
        (4, Value::Absolute(4), PatchError::NonPicReference),
        (10, Value::ImageRva(8), PatchError::NonPicReference),
        (11, Value::ImageRva(8), PatchError::NonPicReference),
        (1, Value::ImageRva(8), PatchError::TextRelocation),
    ] {
        let mut bytes = [0xa5; 8];
        let mut f = fixup(kind, value);
        f.writable = false;
        assert_eq!(class11::pie::patch(&mut bytes, f), Err(expected));
        assert_eq!(bytes, [0xa5; 8]);
    }
    let mut empty = [];
    let mut f = fixup(0, Value::ImageRva(u64::MAX));
    f.offset = u64::MAX;
    f.place_rva = u64::MAX;
    f.writable = false;
    assert_eq!(class11::pie::patch(&mut empty, f).unwrap(), None);
    let mut bytes = [0xa5; 8];
    assert!(matches!(
        class11::pie::patch(&mut bytes, fixup(999, Value::ImageRva(4))),
        Err(PatchError::Relocation(_))
    ));
    assert_eq!(bytes, [0xa5; 8]);
}
#[test]
fn relative_record_encoding() {
    let records = [
        Relative {
            offset: 0x3011,
            addend: -7,
        },
        Relative {
            offset: 0x3009,
            addend: i64::MIN,
        },
    ];
    let encoded = class11::pie::encode_relative(&records).unwrap();
    assert_eq!(encoded.len(), 48);
    for (index, r) in [records[1], records[0]].iter().enumerate() {
        let i = index * 24;
        assert_eq!(&encoded[i..i + 8], &r.offset.to_le_bytes());
        assert_eq!(&encoded[i + 8..i + 16], &8u64.to_le_bytes());
        assert_eq!(&encoded[i + 16..i + 24], &r.addend.to_le_bytes());
    }
    assert_eq!(
        class11::pie::encode_relative(&[records[1], records[0]]).unwrap(),
        encoded
    );
    assert!(class11::pie::encode_relative(&[]).unwrap().is_empty());
}
#[test]
fn relative_record_rejections() {
    for offsets in [
        [8, 8],
        [8, 15],
        [u64::MAX, 0],
        [class11::executable::MAX_IMAGE - 7, 0],
    ] {
        let records = offsets.map(|offset| Relative { offset, addend: 0 });
        assert!(matches!(
            class11::pie::encode_relative(&records),
            Err(Error::InvalidRelative { .. })
        ));
    }
    assert!(class11::pie::encode_relative(&[Relative {
        offset: class11::executable::MAX_IMAGE - 8,
        addend: i64::MAX
    }])
    .is_ok());
}
fn planned() -> class11::layout::Image {
    use class11::layout::{Contribution, Kind};
    class11::layout::plan(
        &[
            Contribution {
                file: 0,
                section: 1,
                kind: Kind::Text,
                alignment: 1,
                bytes: &[0x90, 0xc3],
                memory_size: 2,
            },
            Contribution {
                file: 0,
                section: 2,
                kind: Kind::Data,
                alignment: 8,
                bytes: &[0; 16],
                memory_size: 16,
            },
            Contribution {
                file: class11::pie::BOOT.0,
                section: class11::pie::BOOT.1,
                kind: Kind::Text,
                alignment: 16,
                bytes: class11::pie::STARTUP,
                memory_size: 160,
            },
            Contribution {
                file: class11::pie::RELA.0,
                section: class11::pie::RELA.1,
                kind: Kind::ReadOnly,
                alignment: 8,
                bytes: &[0; 24],
                memory_size: 24,
            },
        ],
        &[],
    )
    .unwrap()
}
fn u64at(b: &[u8], i: usize) -> u64 {
    u64::from_le_bytes(b[i..i + 8].try_into().unwrap())
}
#[test]
fn pie_headers_and_descriptor() {
    let image = planned();
    let boot = &image.placements[&class11::pie::BOOT];
    let table = &image.placements[&class11::pie::RELA];
    let (boot_file, boot_rva, table_file, table_rva) = (
        boot.file_offset.unwrap() as usize,
        boot.address - class11::executable::BASE,
        table.file_offset.unwrap() as usize,
        table.address - class11::executable::BASE,
    );
    let entry = image.placements[&(0, 1)].address - class11::executable::BASE + 1;
    let offset = image.placements[&(0, 2)].address - class11::executable::BASE;
    let segments = image
        .segments
        .iter()
        .map(|s| (s.flags, s.offset, s.file_size, s.memory_size))
        .collect::<Vec<_>>();
    let out = class11::pie::emit(
        image,
        &[Relative {
            offset,
            addend: entry as i64,
        }],
        entry,
    )
    .unwrap();
    assert_eq!(&out[16..18], &3u16.to_le_bytes());
    assert_eq!(u64at(&out, 24), boot_rva);
    assert_eq!(&out[56..58], &4u16.to_le_bytes());
    for (i, (flags, offset, file_size, memory_size)) in segments.iter().enumerate() {
        let h = 64 + 56 * i;
        assert_eq!(&out[h..h + 4], &1u32.to_le_bytes());
        assert_eq!(&out[h + 4..h + 8], &flags.to_le_bytes());
        for field in [8, 16, 24] {
            assert_eq!(u64at(&out, h + field), *offset);
        }
        assert_eq!(u64at(&out, h + 32), *file_size);
        assert_eq!(u64at(&out, h + 40), *memory_size);
    }
    assert_eq!(
        &out[boot_file..boot_file + 128],
        &class11::pie::STARTUP[..128]
    );
    for (field, value) in [(128, boot_rva), (136, table_rva), (144, 1), (152, entry)] {
        assert_eq!(u64at(&out, boot_file + field), value);
    }
    assert_eq!(u64at(&out, table_file), offset);
    assert_eq!(u64at(&out, table_file + 8), 8);
    assert_eq!(u64at(&out, table_file + 16), entry);
    assert_eq!(&out[236..240], &6u32.to_le_bytes());
}
#[test]
fn emission_rejects_bad_destinations() {
    let image = planned();
    let entry = image.placements[&(0, 1)].address - class11::executable::BASE;
    let data = image.placements[&(0, 2)].address - class11::executable::BASE;
    for offset in [entry, data - 1, data + 9] {
        assert!(matches!(
            class11::pie::emit(planned(), &[Relative { offset, addend: 0 }], entry),
            Err(Error::InvalidRelative { .. })
        ));
    }
    assert_eq!(
        class11::pie::emit(
            planned(),
            &[Relative {
                offset: data,
                addend: 0
            }],
            entry + 2
        ),
        Err(Error::InvalidEntry)
    );
    let boot_entry = image.placements[&class11::pie::BOOT].address - class11::executable::BASE;
    assert_eq!(
        class11::pie::emit(
            planned(),
            &[Relative {
                offset: data,
                addend: 0
            }],
            boot_entry
        ),
        Err(Error::InvalidEntry)
    );
    assert_eq!(
        class11::pie::emit(planned(), &[], entry),
        Err(Error::InvalidRuntime)
    );
    let mut missing = planned();
    missing.placements.remove(&class11::pie::BOOT);
    assert_eq!(
        class11::pie::emit(
            missing,
            &[Relative {
                offset: data,
                addend: 0
            }],
            entry
        ),
        Err(Error::InvalidRuntime)
    );
}
