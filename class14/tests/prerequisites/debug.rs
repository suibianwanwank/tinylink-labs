//! Contract tests for debug collection, debug relocation, symbols, and headers.
use class14::debug::{
    Contents, DebugError, DebugTarget, Error, OutputSection, OutputSymbol, DEBUG_STRINGS, SHN_ABS,
};
use class14::gc::{SectionId, Special};

fn id(file: usize, section: u16) -> SectionId {
    SectionId { file, section }
}
fn meta<'a>(
    id: SectionId,
    name: &'a [u8],
    flags: u64,
    alignment: u64,
    bytes: &'a [u8],
) -> Special<'a> {
    Special {
        id,
        name,
        kind: 1,
        flags,
        size: bytes.len() as u64,
        entry_size: u64::from(flags == DEBUG_STRINGS),
        alignment,
        bytes,
    }
}

#[test]
fn collect_groups_by_name_in_first_appearance_order() {
    let metadata = [
        meta(id(0, 5), b".debug_info", 0, 1, b"AAAA"),
        meta(id(0, 6), b".comment", 0x30, 1, b"GCC\0"),
        meta(id(0, 7), b".debug_str", DEBUG_STRINGS, 1, b"x\0"),
        meta(id(1, 4), b".debug_info", 0, 1, b"BBB"),
        meta(id(1, 5), b".debug_aranges", 0, 4, b"CC"),
        meta(id(2, 3), b".debug_aranges", 0, 4, b"DD"),
        meta(id(2, 4), b".debug_str", DEBUG_STRINGS, 0, b"yz\0"),
    ];
    let debug = class14::debug::collect_debug(&metadata).unwrap();
    let names: Vec<_> = debug.sections.iter().map(|s| s.name.as_slice()).collect();
    assert_eq!(
        names,
        [&b".debug_info"[..], b".debug_str", b".debug_aranges"]
    );
    assert_eq!(debug.sections[0].bytes, b"AAAABBB");
    assert_eq!(debug.sections[1].bytes, b"x\0yz\0");
    assert_eq!(
        (debug.sections[1].flags, debug.sections[1].entry_size),
        (DEBUG_STRINGS, 1)
    );
    assert_eq!(debug.sections[2].bytes, b"CC\0\0DD");
    assert_eq!(debug.sections[2].alignment, 4);
    assert_eq!(debug.sections[0].alignment, 1);
    let offsets: Vec<_> = debug.offsets.iter().map(|(k, v)| (*k, *v)).collect();
    assert_eq!(
        offsets,
        [
            (id(0, 5), (0, 0)),
            (id(0, 7), (1, 0)),
            (id(1, 4), (0, 4)),
            (id(1, 5), (2, 0)),
            (id(2, 3), (2, 4)),
            (id(2, 4), (1, 2)),
        ]
    );
    assert!(!debug.offsets.contains_key(&id(0, 6)));
    let empty = class14::debug::collect_debug(&[meta(id(0, 1), b".comment", 0, 1, b"x")]).unwrap();
    assert!(empty.sections.is_empty() && empty.offsets.is_empty());
}

#[test]
fn collect_rejects_compressed_and_flagged_sections() {
    let reject = |section: Special<'_>| {
        assert_eq!(
            class14::debug::collect_debug(&[section]),
            Err(Error::UnsupportedDebug(section.id))
        );
    };
    reject(meta(id(0, 1), b".debug_info", 0x800, 1, b"z"));
    reject(meta(id(0, 1), b".debug_info", 0x10, 1, b"z"));
    reject(meta(id(0, 1), b".debug_info", 0, 8192, b"z"));
    let mixed = [
        meta(id(0, 1), b".debug_str", DEBUG_STRINGS, 1, b"a\0"),
        meta(id(1, 1), b".debug_str", 0, 1, b"b\0"),
    ];
    assert_eq!(
        class14::debug::collect_debug(&mixed),
        Err(Error::UnsupportedDebug(id(1, 1)))
    );
    let twice = [
        meta(id(0, 1), b".debug_info", 0, 1, b"a"),
        meta(id(0, 1), b".debug_info", 0, 1, b"a"),
    ];
    assert_eq!(
        class14::debug::collect_debug(&twice),
        Err(Error::Output(class14::layout::Error::DuplicateIdentity))
    );
    // Non-debug metadata is never inspected, whatever its flags.
    assert!(class14::debug::collect_debug(&[meta(id(0, 1), b".note.x", 0x800, 3, b"")]).is_ok());
}

fn rela(offset: u64, kind: u32, addend: i64) -> class14::symbols::Rela {
    class14::symbols::Rela {
        offset,
        symbol: 1,
        kind,
        addend,
    }
}

#[test]
fn relocate_writes_addresses_offsets_and_tombstones() {
    let mut bytes = [0xaa; 16];
    class14::debug::relocate_debug(
        &mut bytes,
        b".debug_info",
        &rela(0, 1, 0x10),
        DebugTarget::Address(0x401000),
    )
    .unwrap();
    assert_eq!(u64::from_le_bytes(bytes[..8].try_into().unwrap()), 0x401010);
    class14::debug::relocate_debug(
        &mut bytes,
        b".debug_info",
        &rela(8, 10, 3),
        DebugTarget::Debug {
            index: 2,
            offset: 0x40,
        },
    )
    .unwrap();
    assert_eq!(u32::from_le_bytes(bytes[8..12].try_into().unwrap()), 0x43);
    assert_eq!(&bytes[12..], &[0xaa; 4]);
    for (name, tombstone) in [
        (&b".debug_info"[..], 0u64),
        (b".debug_rnglists", 0),
        (b".debug_ranges", 1),
        (b".debug_loc", 1),
    ] {
        let mut field = [0xaa; 8];
        class14::debug::relocate_debug(&mut field, name, &rela(0, 1, 0x20), DebugTarget::Discarded)
            .unwrap();
        assert_eq!(u64::from_le_bytes(field), tombstone);
        let mut narrow = [0xaa; 4];
        class14::debug::relocate_debug(&mut narrow, name, &rela(0, 10, -5), DebugTarget::Discarded)
            .unwrap();
        assert_eq!(u64::from(u32::from_le_bytes(narrow)), tombstone);
    }
    let mut untouched = [0xaa; 4];
    class14::debug::relocate_debug(
        &mut untouched,
        b".debug_info",
        &rela(99, 0, 0),
        DebugTarget::Discarded,
    )
    .unwrap();
    assert_eq!(untouched, [0xaa; 4]);
}

#[test]
fn tls_debug_fields_use_module_offsets_and_signed_dtpoff32() {
    let mut bytes = [0xaa; 16];
    class14::debug::relocate_debug(
        &mut bytes,
        b".debug_info",
        &rela(0, 17, 4),
        DebugTarget::TlsOffset(16),
    )
    .unwrap();
    assert_eq!(u64::from_le_bytes(bytes[..8].try_into().unwrap()), 20);
    class14::debug::relocate_debug(
        &mut bytes,
        b".debug_info",
        &rela(8, 21, -1),
        DebugTarget::TlsOffset(0),
    )
    .unwrap();
    assert_eq!(&bytes[8..12], &[0xff; 4]);
    assert_eq!(&bytes[12..], &[0xaa; 4]);
    let before = bytes;
    assert_eq!(
        class14::debug::relocate_debug(
            &mut bytes,
            b".debug_info",
            &rela(0, 21, 0),
            DebugTarget::TlsOffset(1 << 31),
        ),
        Err(DebugError::Relocation(
            class14::relocate::Error::RelocationOverflow
        ))
    );
    for (kind, target) in [
        (21, DebugTarget::Address(8)),
        (1, DebugTarget::TlsOffset(8)),
    ] {
        assert_eq!(
            class14::debug::relocate_debug(&mut bytes, b".debug_info", &rela(99, kind, 0), target),
            Err(DebugError::Unsupported(kind))
        );
    }
    assert_eq!(bytes, before);
}

#[test]
fn relocate_rejects_kinds_and_overflow() {
    let mut bytes = [0x55; 8];
    for kind in [2, 4, 9, 11, 21, 23] {
        assert_eq!(
            class14::debug::relocate_debug(
                &mut bytes,
                b".debug_info",
                &rela(0, kind, 0),
                DebugTarget::Address(1)
            ),
            Err(DebugError::Unsupported(kind))
        );
    }
    assert_eq!(
        class14::debug::relocate_debug(
            &mut bytes,
            b".debug_info",
            &rela(0, 10, 0),
            DebugTarget::Address(1 << 32)
        ),
        Err(DebugError::Relocation(
            class14::relocate::Error::RelocationOverflow
        ))
    );
    assert_eq!(
        class14::debug::relocate_debug(
            &mut bytes,
            b".debug_info",
            &rela(1, 1, 0),
            DebugTarget::Address(0)
        ),
        Err(DebugError::Relocation(
            class14::relocate::Error::PatchBounds
        ))
    );
    assert_eq!(
        class14::debug::relocate_debug(
            &mut bytes,
            b".debug_info",
            &rela(6, 10, 0),
            DebugTarget::Discarded
        ),
        Err(DebugError::Relocation(
            class14::relocate::Error::PatchBounds
        ))
    );
    assert_eq!(bytes, [0x55; 8]);
}

#[test]
fn debug_patch_truncation_is_atomic() {
    for (kind, width) in [(1, 8), (10, 4)] {
        for len in 0..width {
            let mut bytes = vec![0xa5; len];
            let before = bytes.clone();
            assert_eq!(
                class14::debug::relocate_debug(
                    &mut bytes,
                    b".debug_info",
                    &rela(0, kind, 0),
                    DebugTarget::Address(0)
                ),
                Err(DebugError::Relocation(
                    class14::relocate::Error::PatchBounds
                ))
            );
            assert_eq!(bytes, before);
        }
    }
}

fn symbol(name: &[u8], info: u8, value: u64) -> OutputSymbol<'_> {
    OutputSymbol {
        name,
        info,
        section: 1,
        value,
        size: 4,
    }
}
fn entry(symtab: &[u8], i: usize) -> (u32, u8, u16, u64, u64) {
    let e = &symtab[24 * i..24 * i + 24];
    (
        u32::from_le_bytes(e[..4].try_into().unwrap()),
        e[4],
        u16::from_le_bytes(e[6..8].try_into().unwrap()),
        u64::from_le_bytes(e[8..16].try_into().unwrap()),
        u64::from_le_bytes(e[16..24].try_into().unwrap()),
    )
}

#[test]
fn symbols_put_locals_first_and_share_names() {
    let symbols = [
        symbol(b"main", 0x12, 0x401000),
        symbol(b"helper", 0x02, 0x401010),
        OutputSymbol {
            section: SHN_ABS,
            ..symbol(b"limit", 0x10, 64)
        },
        symbol(b"helper", 0x01, 0x402000),
        symbol(b"", 0x00, 7),
        symbol(b"main", 0x22, 0x401000),
    ];
    let table = class14::debug::encode_symbols(&symbols).unwrap();
    assert_eq!(table.first_global, 4);
    assert_eq!(table.symtab.len(), 24 * 7);
    assert_eq!(&table.symtab[..24], &[0; 24]);
    assert_eq!(table.strtab, b"\0helper\0main\0limit\0");
    let entries: Vec<_> = (1..7).map(|i| entry(&table.symtab, i)).collect();
    assert_eq!(
        entries,
        [
            (1, 0x02, 1, 0x401010, 4),
            (1, 0x01, 1, 0x402000, 4),
            (0, 0x00, 1, 7, 4),
            (8, 0x12, 1, 0x401000, 4),
            (13, 0x10, SHN_ABS, 64, 4),
            (8, 0x22, 1, 0x401000, 4),
        ]
    );
    let only_null = class14::debug::encode_symbols(&[]).unwrap();
    assert_eq!(
        (only_null.symtab, only_null.strtab, only_null.first_global),
        (vec![0; 24], vec![0], 1)
    );
}

#[test]
fn symbols_reject_embedded_nul() {
    assert_eq!(
        class14::debug::encode_symbols(&[symbol(b"a\0b", 0x12, 0)]),
        Err(Error::InvalidName)
    );
}

/// A 64-byte ELF header followed by `payload`, with no section table.
fn elf(payload: &[u8]) -> Vec<u8> {
    let mut out = vec![0u8; 64];
    out[..4].copy_from_slice(b"\x7fELF");
    out.extend_from_slice(payload);
    out
}
fn section(name: &[u8], kind: u32, alignment: u64, contents: Contents) -> OutputSection {
    OutputSection {
        name: name.to_vec(),
        kind,
        flags: 0,
        address: 0,
        link: 0,
        info: 0,
        alignment,
        entry_size: 0,
        contents,
    }
}
fn header(file: &[u8], index: usize) -> (u32, u32, u64, u64, u64) {
    let table = u64::from_le_bytes(file[40..48].try_into().unwrap()) as usize;
    let h = &file[table + 64 * index..table + 64 * index + 64];
    let u64at = |at: usize| u64::from_le_bytes(h[at..at + 8].try_into().unwrap());
    (
        u32::from_le_bytes(h[..4].try_into().unwrap()),
        u32::from_le_bytes(h[4..8].try_into().unwrap()),
        u64at(24),
        u64at(32),
        u64at(48),
    )
}

#[test]
fn attach_appends_data_and_section_table() {
    let input = elf(&[1, 2, 3, 4, 5]);
    let sections = [
        OutputSection {
            flags: 6,
            address: 0x401000,
            ..section(
                b".text",
                1,
                4,
                Contents::File {
                    offset: 64,
                    size: 5,
                },
            )
        },
        section(b".bss", 8, 8, Contents::NoBits { size: 100 }),
        section(b".debug_info", 1, 1, Contents::Data(b"DW".to_vec())),
        OutputSection {
            link: 5,
            info: 1,
            entry_size: 24,
            ..section(b".symtab", 2, 8, Contents::Data(vec![9; 24]))
        },
        section(b".strtab", 3, 1, Contents::Data(b"\0".to_vec())),
    ];
    let out = class14::debug::attach_sections(input.clone(), &sections).unwrap();
    assert_eq!(&out[..40], &input[..40]);
    assert_eq!(&out[64..69], &[1, 2, 3, 4, 5]);
    let u16at = |at: usize| u16::from_le_bytes(out[at..at + 2].try_into().unwrap());
    assert_eq!((u16at(58), u16at(60), u16at(62)), (64, 7, 6));
    assert_eq!(header(&out, 0), (0, 0, 0, 0, 0));
    assert_eq!(header(&out, 1), (1, 1, 64, 5, 4));
    assert_eq!(header(&out, 2), (7, 8, 69, 100, 8));
    // Data starts at the end of the input, aligned per section.
    assert_eq!(header(&out, 3), (12, 1, 69, 2, 1));
    assert_eq!(&out[69..71], b"DW");
    assert_eq!(header(&out, 4), (24, 2, 72, 24, 8));
    assert_eq!(&out[72..96], &[9; 24]);
    assert_eq!(header(&out, 5), (32, 3, 96, 1, 1));
    let names = b"\0.text\0.bss\0.debug_info\0.symtab\0.strtab\0.shstrtab\0";
    assert_eq!(header(&out, 6), (40, 3, 97, names.len() as u64, 1));
    assert_eq!(&out[97..97 + names.len()], names);
    let table = u64::from_le_bytes(out[40..48].try_into().unwrap());
    assert_eq!(table, (97 + names.len() as u64).div_ceil(8) * 8);
    assert_eq!(out.len() as u64, table + 7 * 64);
    let symtab = &out[table as usize + 4 * 64..table as usize + 5 * 64];
    assert_eq!(u32::from_le_bytes(symtab[40..44].try_into().unwrap()), 5);
    assert_eq!(u32::from_le_bytes(symtab[44..48].try_into().unwrap()), 1);
    assert_eq!(u64::from_le_bytes(symtab[56..64].try_into().unwrap()), 24);
    let text = &out[table as usize + 64..table as usize + 128];
    assert_eq!(u64::from_le_bytes(text[8..16].try_into().unwrap()), 6);
    assert_eq!(
        u64::from_le_bytes(text[16..24].try_into().unwrap()),
        0x401000
    );
}

#[test]
fn attach_respects_extended_numbering_boundary() {
    // Ordinary e_shnum is valid below SHN_LORESERVE. The generated null and
    // .shstrtab entries make the output count two greater than the input count.
    let empty = section(b".empty", 1, 1, Contents::Data(Vec::new()));
    let mut sections = vec![empty.clone(); 0xff00 - 3];
    let out = class14::debug::attach_sections(elf(&[]), &sections).unwrap();
    assert_eq!(u16::from_le_bytes(out[60..62].try_into().unwrap()), 0xfeff);
    assert_eq!(u16::from_le_bytes(out[62..64].try_into().unwrap()), 0xfefe);

    sections.push(empty.clone());
    assert_eq!(
        class14::debug::attach_sections(elf(&[]), &sections).err(),
        Some(Error::TooLarge)
    );
    sections.push(empty);
    assert_eq!(
        class14::debug::attach_sections(elf(&[]), &sections).err(),
        Some(Error::TooLarge)
    );
}

#[test]
fn attach_rejects_bad_inputs() {
    let file = || elf(&[0; 8]);
    let check = |input: Vec<u8>, sections: &[OutputSection]| {
        assert_eq!(
            class14::debug::attach_sections(input, sections),
            Err(Error::InvalidSection)
        );
    };
    check(vec![0; 10], &[]);
    let mut not_elf = file();
    not_elf[0] = 0;
    check(not_elf, &[]);
    let mut has_table = file();
    has_table[40] = 1;
    check(has_table, &[]);
    let mut has_count = file();
    has_count[60] = 1;
    check(has_count, &[]);
    check(
        file(),
        &[section(
            b".text",
            1,
            1,
            Contents::File {
                offset: 70,
                size: 3,
            },
        )],
    );
    check(
        file(),
        &[section(
            b".text",
            1,
            1,
            Contents::File {
                offset: u64::MAX,
                size: 2,
            },
        )],
    );
    check(file(), &[section(b"a\0b", 1, 1, Contents::Data(vec![]))]);
    check(file(), &[section(b".data", 1, 3, Contents::Data(vec![1]))]);
    assert!(class14::debug::attach_sections(file(), &[]).is_ok());
}

#[test]
fn attach_bounds_alignment_before_allocating() {
    for alignment in [1 << 63, class14::executable::MAX_IMAGE] {
        let sections = [section(
            b".debug_info",
            1,
            alignment,
            Contents::Data(vec![1]),
        )];
        assert_eq!(
            class14::debug::attach_sections(elf(&[]), &sections),
            Err(Error::TooLarge)
        );
    }
}

#[test]
fn collect_bounds_combined_metadata_before_appending() {
    let payload = vec![0; (class14::executable::MAX_IMAGE / 2 + 1) as usize];
    let metadata = [
        meta(id(0, 1), b".debug_info", 0, 1, &payload),
        meta(id(0, 2), b".debug_line", 0, 1, &payload),
    ];
    assert_eq!(
        class14::debug::collect_debug(&metadata).err(),
        Some(Error::TooLarge)
    );
}
