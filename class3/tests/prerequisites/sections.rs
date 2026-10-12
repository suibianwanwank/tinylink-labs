use class3::sections::*;

#[test]
fn section_validation_priority() {
    let mut b = fixture();
    b[240..248].copy_from_slice(&3u64.to_le_bytes());
    b[248..256].copy_from_slice(&3u64.to_le_bytes());
    assert_eq!(section(&b, &header(), 2), Err(Error::InvalidAlignment));
    assert_eq!(
        section(&b[..255], &header(), 2),
        Err(Error::Input(class3::elf_header::Error::OutOfBounds))
    );
}

#[test]
fn object_validation_is_staged() {
    let mut b = fixture();
    // A late header defect precedes even an invalid name-table type.
    b[132] = 1;
    b[240..248].copy_from_slice(&3u64.to_le_bytes());
    assert_eq!(sections(&b), Err(Error::InvalidAlignment));
    b[240..248].copy_from_slice(&16u64.to_le_bytes());
    b[152..160].copy_from_slice(&u64::MAX.to_le_bytes());
    assert_eq!(sections(&b), Err(Error::InvalidNameTable));
    b[132] = 3;
    assert_eq!(
        sections(&b),
        Err(Error::Input(class3::elf_header::Error::RangeOverflow))
    );

    let mut b = fixture();
    b[192..196].copy_from_slice(&u32::MAX.to_le_bytes());
    b[216..224].copy_from_slice(&u64::MAX.to_le_bytes());
    // In each entry, payload validation precedes name resolution.
    assert_eq!(
        sections(&b),
        Err(Error::Input(class3::elf_header::Error::RangeOverflow))
    );
    b[216..224].copy_from_slice(&280u64.to_le_bytes());
    assert_eq!(sections(&b), Err(Error::BadStringOffset));
}

#[test]
fn name_table_boundaries_and_entry_order() {
    for (offset, value) in [(256, 1), (272, 1)] {
        let mut b = fixture();
        b[offset] = value;
        assert_eq!(sections(&b), Err(Error::InvalidNameTable));
    }
    let mut b = fixture();
    b[160..168].copy_from_slice(&0u64.to_le_bytes());
    assert_eq!(sections(&b), Err(Error::InvalidNameTable));
    let mut b = fixture();
    // Entry 0's bad name must win over entry 2's bad payload.
    b[64..68].copy_from_slice(&u32::MAX.to_le_bytes());
    b[216..224].copy_from_slice(&u64::MAX.to_le_bytes());
    assert_eq!(sections(&b), Err(Error::BadStringOffset));
}

#[test]
fn opaque_sections_and_full_width_fields() {
    let mut b = fixture();
    b[196..200].copy_from_slice(&0x7000_0001u32.to_le_bytes());
    b[200..208].copy_from_slice(&0x8000_0000_0000_0006u64.to_le_bytes());
    b[208..216].copy_from_slice(&u64::MAX.to_le_bytes()); // ignored sh_addr
    b[232..236].copy_from_slice(&0x1234_5678u32.to_le_bytes());
    b[236..240].copy_from_slice(&0x8765_4321u32.to_le_bytes());
    let s = section(&b, &header(), 2).unwrap();
    assert_eq!(s.kind, 0x7000_0001);
    assert_eq!(s.flags, 0x8000_0000_0000_0006);
    assert_eq!(s.link, 0x1234_5678);
    assert_eq!(s.info, 0x8765_4321);
    assert_eq!(payload(&b, &s), Ok(&b[280..282]));
    b[216..224].copy_from_slice(&(1u64 << 32).to_le_bytes());
    let s = section(&b, &header(), 2).unwrap();
    assert_eq!(s.offset, 1u64 << 32);
    assert_eq!(
        payload(&b, &s),
        Err(Error::Input(class3::elf_header::Error::OutOfBounds))
    );
}
fn fixture() -> Vec<u8> {
    let mut b = vec![0; 288];
    b[..7].copy_from_slice(b"\x7fELF\x02\x01\x01");
    b[16..18].copy_from_slice(&1u16.to_le_bytes());
    b[18..20].copy_from_slice(&62u16.to_le_bytes());
    b[20..24].copy_from_slice(&1u32.to_le_bytes());
    b[40..48].copy_from_slice(&64u64.to_le_bytes());
    b[52..54].copy_from_slice(&64u16.to_le_bytes());
    b[58..60].copy_from_slice(&64u16.to_le_bytes());
    b[60..62].copy_from_slice(&3u16.to_le_bytes());
    b[62..64].copy_from_slice(&1u16.to_le_bytes());
    let names = b"\0.shstrtab\0.text\0";
    b[128..132].copy_from_slice(&1u32.to_le_bytes());
    b[132..136].copy_from_slice(&3u32.to_le_bytes());
    b[152..160].copy_from_slice(&256u64.to_le_bytes());
    b[160..168].copy_from_slice(&(names.len() as u64).to_le_bytes());
    b[176..184].copy_from_slice(&1u64.to_le_bytes());
    b[192..196].copy_from_slice(&11u32.to_le_bytes());
    b[196..200].copy_from_slice(&1u32.to_le_bytes());
    b[200..208].copy_from_slice(&6u64.to_le_bytes());
    b[216..224].copy_from_slice(&280u64.to_le_bytes());
    b[224..232].copy_from_slice(&2u64.to_le_bytes());
    b[240..248].copy_from_slice(&16u64.to_le_bytes());
    b[256..256 + names.len()].copy_from_slice(names);
    b[280..282].copy_from_slice(&[0x90, 0xc3]);
    b
}
fn header() -> class3::elf_header::Header {
    class3::elf_header::Header {
        section_offset: 64,
        section_count: 3,
        section_names_index: 1,
    }
}
#[test]
fn decode_section() {
    let b = fixture();
    let s = section(&b, &header(), 2).unwrap();
    assert_eq!(
        s,
        Section {
            name_offset: 11,
            kind: 1,
            flags: 6,
            offset: 280,
            size: 2,
            link: 0,
            info: 0,
            alignment: 16,
            entry_size: 0
        }
    );
    assert_eq!(payload(&b, &s).unwrap(), &[0x90, 0xc3]);
}
#[test]
fn names_are_bytes() {
    assert_eq!(string_at(b"\0ab\0\xff\0", 0), Ok(&b""[..]));
    assert_eq!(string_at(b"\0ab\0\xff\0", 2), Ok(&b"b"[..]));
    assert_eq!(string_at(b"\0ab\0\xff\0", 4), Ok(&b"\xff"[..]));
    assert_eq!(string_at(b"abc", 0), Err(Error::UnterminatedString));
    assert_eq!(string_at(b"abc\0", 4), Err(Error::BadStringOffset));
    assert_eq!(string_at(b"", 0), Err(Error::BadStringOffset));
}
#[test]
fn memory_only_sections() {
    let s = Section {
        name_offset: 0,
        kind: 8,
        flags: 3,
        offset: u64::MAX,
        size: 4096,
        link: 0,
        info: 0,
        alignment: 16,
        entry_size: 0,
    };
    assert_eq!(payload(&[], &s), Ok(&b""[..]));
    assert_eq!(payload(&[], &Section { kind: 0, ..s }), Ok(&b""[..]));
}
#[test]
fn reject_metadata() {
    let mut b = fixture();
    b[240..248].copy_from_slice(&3u64.to_le_bytes());
    assert_eq!(section(&b, &header(), 2), Err(Error::InvalidAlignment));
    b[240..248].copy_from_slice(&0u64.to_le_bytes());
    b[248..256].copy_from_slice(&3u64.to_le_bytes());
    assert_eq!(section(&b, &header(), 2), Err(Error::InvalidEntrySize));
}
#[test]
fn complete_object() {
    let b = fixture();
    let s = sections(&b).unwrap();
    assert_eq!(s.len(), 3);
    assert_eq!(s[0].1, b"");
    assert_eq!(s[1].1, b".shstrtab");
    assert_eq!(s[2].1, b".text");
}
#[test]
fn reject_name_table_and_file_range() {
    let mut b = fixture();
    b[132] = 1;
    assert_eq!(sections(&b), Err(Error::InvalidNameTable));
    let mut b = fixture();
    b[256] = 1;
    assert_eq!(sections(&b), Err(Error::InvalidNameTable));
    let mut b = fixture();
    b[192..196].copy_from_slice(&100u32.to_le_bytes());
    assert_eq!(sections(&b), Err(Error::BadStringOffset));
    let b = fixture();
    assert_eq!(
        sections(&b[..281]),
        Err(Error::Input(class3::elf_header::Error::OutOfBounds))
    );
}
