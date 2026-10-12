use class0::elf_header::*;

#[test]
fn validation_order_all_distinct_errors() {
    let defects = [
        (0, 0, Error::BadMagic),
        (4, 1, Error::UnsupportedClass),
        (5, 2, Error::UnsupportedEndian),
        (6, 0, Error::UnsupportedVersion),
        (20, 0, Error::UnsupportedVersion),
        (16, 2, Error::UnsupportedType),
        (18, 0, Error::UnsupportedMachine),
        (52, 0, Error::InvalidHeaderSize),
        (58, 0, Error::InvalidSectionEntrySize),
        (62, 0, Error::UnsupportedExtendedNumbering),
    ];
    for (i, (offset, value, expected)) in defects.iter().enumerate() {
        for (later, bad, _) in &defects[i + 1..] {
            let mut b = fixture();
            b[*offset] = *value;
            b[*later] = *bad;
            assert_eq!(
                parse_header(&b).as_ref().unwrap_err(),
                expected,
                "defects at {offset} and {later}"
            );
        }
        for table_offset in [1000u64, u64::MAX - 63] {
            let mut b = fixture();
            b[*offset] = *value;
            b[40..48].copy_from_slice(&table_offset.to_le_bytes());
            assert_eq!(
                parse_header(&b).as_ref().unwrap_err(),
                expected,
                "field {offset} must precede table range validation"
            );
        }
    }
    let mut b = fixture();
    b[0] = 0;
    assert_eq!(parse_header(&b[..63]), Err(Error::Truncated));
}

#[test]
fn large_table_uses_wide_arithmetic() {
    // 1024 * 64 exceeds u16, but is a valid table size.
    let mut b = fixture();
    b.resize(64 + 1024 * 64, 0);
    b[60..62].copy_from_slice(&1024u16.to_le_bytes());
    let h = parse_header(&b).unwrap();
    assert_eq!(h.section_count, 1024);
    assert_eq!(section_header(&b, &h, 1023), Ok(&b[65536..65600]));
    // Wrapping u16 multiplication yields zero and would accept this truncation.
    assert_eq!(parse_header(&b[..64]), Err(Error::OutOfBounds));
}

#[test]
fn section_index_precedes_bytes_and_arithmetic() {
    let b = vec![0; 256];
    let h = Header {
        section_offset: 64,
        section_count: 2,
        section_names_index: 1,
    };
    // Extra bytes after the table do not make index == count valid.
    assert_eq!(section_header(&b, &h, 2), Err(Error::OutOfBounds));
    let h = Header {
        section_offset: u64::MAX,
        ..h
    };
    assert_eq!(section_header(&b, &h, 2), Err(Error::OutOfBounds));
    assert_eq!(section_header(&b, &h, 0), Err(Error::RangeOverflow));
}

#[test]
fn table_position_is_not_fixed() {
    let mut b = fixture();
    b.resize(448, 0);
    b[40..48].copy_from_slice(&256u64.to_le_bytes());
    b[60..62].copy_from_slice(&3u16.to_le_bytes());
    b[62..64].copy_from_slice(&2u16.to_le_bytes());
    b[320] = 91;
    let h = parse_header(&b).unwrap();
    assert_eq!(section_header(&b, &h, 1), Ok(&b[320..384]));
    assert_eq!(section_header(&b, &h, 2), Ok(&b[384..448]));
}

fn fixture() -> Vec<u8> {
    let mut b = vec![0; 192];
    b[..7].copy_from_slice(b"\x7fELF\x02\x01\x01");
    b[16..18].copy_from_slice(&1u16.to_le_bytes());
    b[18..20].copy_from_slice(&62u16.to_le_bytes());
    b[20..24].copy_from_slice(&1u32.to_le_bytes());
    b[40..48].copy_from_slice(&64u64.to_le_bytes());
    b[52..54].copy_from_slice(&64u16.to_le_bytes());
    b[58..60].copy_from_slice(&64u16.to_le_bytes());
    b[60..62].copy_from_slice(&2u16.to_le_bytes());
    b[62..64].copy_from_slice(&1u16.to_le_bytes());
    b
}

#[test]
fn valid_header() {
    assert_eq!(
        parse_header(&fixture()),
        Ok(Header {
            section_offset: 64,
            section_count: 2,
            section_names_index: 1,
        })
    );
}

#[test]
fn file_ranges() {
    let b = [10, 20, 30];
    assert_eq!(checked_slice(&b, 1, 2), Ok(&b[1..]));
    assert_eq!(checked_slice(&b, 3, 0), Ok(&b[3..]));
    assert_eq!(checked_slice(&b, 4, 0), Err(Error::OutOfBounds));
    assert_eq!(checked_slice(&b, 2, 2), Err(Error::OutOfBounds));
    assert_eq!(checked_slice(&b, u64::MAX, 1), Err(Error::RangeOverflow));
}

#[test]
fn header_truncation_never_panics() {
    let b = fixture();
    for n in 0..64 {
        assert_eq!(parse_header(&b[..n]), Err(Error::Truncated));
    }
    for n in 64..192 {
        assert_eq!(parse_header(&b[..n]), Err(Error::OutOfBounds));
    }
}

#[test]
fn rejected_formats() {
    for (offset, value, error) in [
        (0, 0, Error::BadMagic),
        (4, 1, Error::UnsupportedClass),
        (5, 2, Error::UnsupportedEndian),
        (6, 0, Error::UnsupportedVersion),
        (20, 0, Error::UnsupportedVersion),
        (16, 2, Error::UnsupportedType),
        (18, 183, Error::UnsupportedMachine),
        (52, 63, Error::InvalidHeaderSize),
        (58, 63, Error::InvalidSectionEntrySize),
        (60, 0, Error::UnsupportedExtendedNumbering),
        (62, 0, Error::UnsupportedExtendedNumbering),
        (62, 2, Error::UnsupportedExtendedNumbering),
    ] {
        let mut b = fixture();
        b[offset] = value;
        assert_eq!(parse_header(&b), Err(error), "offset {offset}");
    }
}

#[test]
fn section_table_overflow() {
    let mut b = fixture();
    b[40..48].copy_from_slice(&(u64::MAX - 63).to_le_bytes());
    assert_eq!(parse_header(&b), Err(Error::RangeOverflow));
}

#[test]
fn section_indexing() {
    let mut b = fixture();
    b[128] = 77;
    let h = Header {
        section_offset: 64,
        section_count: 2,
        section_names_index: 1,
    };
    assert_eq!(section_header(&b, &h, 0), Ok(&b[64..128]));
    assert_eq!(section_header(&b, &h, 1), Ok(&b[128..192]));
    assert_eq!(section_header(&b, &h, 2), Err(Error::OutOfBounds));
    assert_eq!(section_header(&b[..150], &h, 1), Err(Error::OutOfBounds));
}

#[test]
fn fabricated_header_overflow() {
    let h = Header {
        section_offset: u64::MAX - 32,
        section_count: 2,
        section_names_index: 1,
    };
    assert_eq!(section_header(&[], &h, 1), Err(Error::RangeOverflow));
}

#[test]
fn validation_priority_and_full_width_fields() {
    let mut b = fixture();
    b[5] = 2;
    b[4] = 1;
    assert_eq!(parse_header(&b), Err(Error::UnsupportedClass));
    for (offset, error) in [
        (17, Error::UnsupportedType),
        (19, Error::UnsupportedMachine),
        (23, Error::UnsupportedVersion),
        (53, Error::InvalidHeaderSize),
        (59, Error::InvalidSectionEntrySize),
    ] {
        let mut b = fixture();
        b[offset] = 1;
        assert_eq!(parse_header(&b), Err(error));
    }
}

#[test]
fn ordinary_section_numbering_boundary() {
    const ORDINARY_LIMIT: u16 = 0xff00;
    let mut b = fixture();
    b.resize(64 + usize::from(ORDINARY_LIMIT - 1) * 64, 0);
    b[60..62].copy_from_slice(&(ORDINARY_LIMIT - 1).to_le_bytes());
    b[62..64].copy_from_slice(&(ORDINARY_LIMIT - 2).to_le_bytes());
    let header = parse_header(&b).unwrap();
    assert_eq!(header.section_count, ORDINARY_LIMIT - 1);
    assert_eq!(header.section_names_index, ORDINARY_LIMIT - 2);

    for count in [ORDINARY_LIMIT, ORDINARY_LIMIT + 1, u16::MAX] {
        b.resize(64 + usize::from(count) * 64, 0);
        b[60..62].copy_from_slice(&count.to_le_bytes());
        b[62..64].copy_from_slice(&1u16.to_le_bytes());
        assert_eq!(parse_header(&b), Err(Error::UnsupportedExtendedNumbering));
        // Numbering is checked before the table range, even with no table bytes.
        assert_eq!(
            parse_header(&b[..64]),
            Err(Error::UnsupportedExtendedNumbering)
        );
    }
}
