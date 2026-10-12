use class15::symbols::*;

#[test]
fn symbol_validation_priority() {
    let (mut b, mut s) = fixture();
    s[3].entry_size = 0;
    s[3].link = 99;
    assert_eq!(symbols(&b, &s, 3), Err(Error::InvalidEntrySize));
    s[3].entry_size = 24;
    b[16] = 1;
    assert_eq!(symbols(&b, &s, 3), Err(Error::InvalidIndex));
    s[3].link = 2;
    b[6] = 1;
    assert_eq!(symbols(&b, &s, 3), Err(Error::InvalidStringTable));
    b[6] = 0;
    s[3].info = 4;
    assert_eq!(symbols(&b, &s, 3), Err(Error::InvalidNullSymbol));
    b[16] = 0;
    assert_eq!(symbols(&b, &s, 3), Err(Error::InvalidSymbolOrder));
}

#[test]
fn symbol_record_validation_priority() {
    let (mut b, s) = fixture();
    b[68] = 2; // Local symbol after sh_info.
    b[70..72].copy_from_slice(&0xffffu16.to_le_bytes());
    b[64..68].copy_from_slice(&99u32.to_le_bytes());
    assert_eq!(symbols(&b, &s, 3), Err(Error::InvalidSymbolOrder));
    b[68] = 0x12;
    assert_eq!(symbols(&b, &s, 3), Err(Error::InvalidSymbolSection));
    b[70..72].copy_from_slice(&0u16.to_le_bytes());
    assert_eq!(
        symbols(&b, &s, 3),
        Err(Error::Input(class15::sections::Error::BadStringOffset))
    );
}

#[test]
fn relocation_validation_priority() {
    let (b, mut s) = fixture();
    s[4].entry_size = 0;
    s[4].info = 0;
    s[4].link = 1;
    s[4].offset = u64::MAX;
    assert_eq!(relocations(&b, &s, 4), Err(Error::InvalidEntrySize));
    s[4].entry_size = 24;
    assert_eq!(relocations(&b, &s, 4), Err(Error::InvalidIndex));
    s[4].info = 1;
    assert_eq!(relocations(&b, &s, 4), Err(Error::WrongSectionType));
    // Empty tables must still validate their ownership metadata.
    s[4].size = 0;
    assert_eq!(relocations(&b, &s, 4), Err(Error::WrongSectionType));
}
fn sec(
    kind: u32,
    offset: u64,
    size: u64,
    link: u32,
    info: u32,
    entry_size: u64,
) -> class15::sections::Section {
    class15::sections::Section {
        name_offset: 0,
        kind,
        flags: 0,
        offset,
        size,
        link,
        info,
        alignment: 1,
        entry_size,
    }
}
fn fixture() -> (Vec<u8>, Vec<class15::sections::Section>) {
    let mut b = vec![0; 128];
    b[..7].copy_from_slice(b"\0entry\0");
    // Three symbols: null, local section symbol, global undefined reference.
    b[40 + 4] = 3;
    b[40 + 6..40 + 8].copy_from_slice(&1u16.to_le_bytes());
    b[64..68].copy_from_slice(&1u32.to_le_bytes());
    b[68] = 0x12;
    // RELA refers to symbol 2, PC32, with addend -4.
    b[88..96].copy_from_slice(&7u64.to_le_bytes());
    b[96..104].copy_from_slice(&((2u64 << 32) | 2).to_le_bytes());
    b[104..112].copy_from_slice(&(-4i64).to_le_bytes());
    let s = vec![
        sec(0, 0, 0, 0, 0, 0),
        sec(1, 120, 8, 0, 0, 0),
        sec(3, 0, 7, 0, 0, 0),
        sec(2, 16, 72, 2, 2, 24),
        sec(4, 88, 24, 3, 1, 24),
    ];
    (b, s)
}
#[test]
fn decode_symbols() {
    let (b, s) = fixture();
    let syms = symbols(&b, &s, 3).unwrap();
    assert_eq!(syms.len(), 3);
    assert_eq!(syms[0].definition, Definition::Undefined);
    assert_eq!(syms[1].definition, Definition::Section(1));
    assert_eq!(syms[1].kind, 3);
    assert_eq!(
        syms[2],
        Symbol {
            name: b"entry",
            binding: 1,
            kind: 2,
            visibility: 0,
            definition: Definition::Undefined,
            value: 0,
            size: 0
        }
    );
}
#[test]
fn special_definitions_and_widths() {
    let (mut b, s) = fixture();
    for (raw, expected) in [
        (0xfff1u16, Definition::Absolute),
        (0xfff2, Definition::Common),
    ] {
        b[70..72].copy_from_slice(&raw.to_le_bytes());
        b[72..80].copy_from_slice(&(1u64 << 48).to_le_bytes());
        b[80..88].copy_from_slice(&999u64.to_le_bytes());
        b[69] = 0xff;
        let out = symbols(&b, &s, 3).unwrap();
        assert_eq!(out[2].definition, expected);
        assert_eq!(out[2].value, 1u64 << 48);
        assert_eq!(out[2].size, 999);
        assert_eq!(out[2].visibility, 3);
    }
}
#[test]
fn reject_symbol_structure() {
    let (b, mut s) = fixture();
    s[3].entry_size = 0;
    assert_eq!(symbols(&b, &s, 3), Err(Error::InvalidEntrySize));
    let (b, mut s) = fixture();
    s[3].link = 99;
    assert_eq!(symbols(&b, &s, 3), Err(Error::InvalidIndex));
    let (b, mut s) = fixture();
    s[3].info = 4;
    assert_eq!(symbols(&b, &s, 3), Err(Error::InvalidSymbolOrder));
    let (mut b, s) = fixture();
    b[16] = 1;
    assert_eq!(symbols(&b, &s, 3), Err(Error::InvalidNullSymbol));
    let (mut b, s) = fixture();
    b[68] = 2;
    assert_eq!(symbols(&b, &s, 3), Err(Error::InvalidSymbolOrder));
    let (mut b, s) = fixture();
    b[70..72].copy_from_slice(&0xffffu16.to_le_bytes());
    assert_eq!(symbols(&b, &s, 3), Err(Error::InvalidSymbolSection));
}
#[test]
fn reject_strings_and_truncation() {
    let (mut b, s) = fixture();
    b[6] = 1;
    assert_eq!(symbols(&b, &s, 3), Err(Error::InvalidStringTable));
    let (mut b, s) = fixture();
    b[64..68].copy_from_slice(&99u32.to_le_bytes());
    assert_eq!(
        symbols(&b, &s, 3),
        Err(Error::Input(class15::sections::Error::BadStringOffset))
    );
    let (b, s) = fixture();
    for end in 16..88 {
        assert!(symbols(&b[..end], &s, 3).is_err());
    }
}
#[test]
fn decode_rela() {
    let (b, s) = fixture();
    assert_eq!(
        relocations(&b, &s, 4),
        Ok(vec![Rela {
            offset: 7,
            symbol: 2,
            kind: 2,
            addend: -4
        }])
    );
    let (mut b, s) = fixture();
    b[104..112].copy_from_slice(&i64::MIN.to_le_bytes());
    b[96..104].copy_from_slice(&((2u64 << 32) | 0xfffffff0).to_le_bytes());
    let r = relocations(&b, &s, 4).unwrap();
    assert_eq!(r[0].addend, i64::MIN);
    assert_eq!(r[0].kind, 0xfffffff0);
}
#[test]
fn reject_relocation_ownership() {
    let (b, mut s) = fixture();
    s[4].kind = 9;
    assert_eq!(relocations(&b, &s, 4), Err(Error::UnsupportedRel));
    let (b, mut s) = fixture();
    s[4].info = 0;
    assert_eq!(relocations(&b, &s, 4), Err(Error::InvalidIndex));
    let (b, mut s) = fixture();
    s[4].link = 1;
    assert_eq!(relocations(&b, &s, 4), Err(Error::WrongSectionType));
    let (mut b, s) = fixture();
    b[96..104].copy_from_slice(&(3u64 << 32).to_le_bytes());
    assert_eq!(relocations(&b, &s, 4), Err(Error::InvalidIndex));
    let (b, mut s) = fixture();
    s[4].size = 23;
    assert_eq!(relocations(&b, &s, 4), Err(Error::InvalidEntrySize));
}

#[test]
fn symbol_names_and_null_entry_are_not_shortcuts() {
    let (mut b, s) = fixture();
    b[1] = 0xff;
    assert_eq!(symbols(&b, &s, 3).unwrap()[2].name, b"\xffntry");
    for offset in 16..40 {
        let (mut b, s) = fixture();
        b[offset] = 1;
        assert_eq!(symbols(&b, &s, 3), Err(Error::InvalidNullSymbol));
    }
    let (b, mut s) = fixture();
    s[3].size = 0;
    assert_eq!(symbols(&b, &s, 3), Err(Error::InvalidNullSymbol));
    let (mut b, mut s) = fixture();
    s[3].info = 3;
    b[68] = 2;
    assert_eq!(symbols(&b, &s, 3).unwrap()[2].binding, 0);
}

#[test]
fn relocation_empty_table_and_truncation() {
    let (b, mut s) = fixture();
    s[4].size = 0;
    assert_eq!(relocations(&b, &s, 4), Ok(vec![]));
    let (b, s) = fixture();
    for end in 88..112 {
        assert!(relocations(&b[..end], &s, 4).is_err());
    }
    assert_eq!(relocations(&b, &s, 99), Err(Error::InvalidIndex));
    let (b, mut s) = fixture();
    s[4].entry_size = 16;
    assert_eq!(relocations(&b, &s, 4), Err(Error::InvalidEntrySize));
}
