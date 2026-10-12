//! Contract tests for the pure merge and unwind building blocks.
use std::collections::BTreeMap;

use class14::gc::{SectionId, Special};
use class14::unwind::{
    Error, Fde, Frame, MergeError, Piece, Record, UnwindError, EH_FRAME, EH_FRAME_HDR,
};

const STRINGS: u64 = 0x32;
const CONSTANTS: u64 = 0x12;

fn id(file: usize, section: u16) -> SectionId {
    SectionId { file, section }
}
fn special(
    id: SectionId,
    flags: u64,
    entry_size: u64,
    alignment: u64,
    bytes: &[u8],
) -> Special<'_> {
    Special {
        id,
        name: b".rodata.test",
        kind: 1,
        flags,
        size: bytes.len() as u64,
        entry_size,
        alignment,
        bytes,
    }
}
fn piece(offset: u64, size: u64) -> Piece {
    Piece { offset, size }
}

#[test]
fn split_strings_and_constants() {
    let strings = special(id(0, 1), STRINGS, 1, 1, b"ab\0\0c\0");
    assert_eq!(
        class14::unwind::split(&strings),
        Ok(vec![piece(0, 3), piece(3, 1), piece(4, 2)])
    );
    assert_eq!(
        class14::unwind::split(&special(id(0, 1), STRINGS, 1, 0, b"")),
        Ok(vec![])
    );
    let constants = special(id(0, 2), CONSTANTS, 4, 4, &[1, 2, 3, 4, 5, 6, 7, 8]);
    assert_eq!(
        class14::unwind::split(&constants),
        Ok(vec![piece(0, 4), piece(4, 4)])
    );
    let wide = special(id(0, 3), CONSTANTS, 16, 0, &[0; 32]);
    assert_eq!(
        class14::unwind::split(&wide),
        Ok(vec![piece(0, 16), piece(16, 16)])
    );
}

#[test]
fn split_rejects_unsupported_shapes() {
    let check = |flags, entry_size, alignment, bytes: &[u8], expected| {
        let section = special(id(0, 1), flags, entry_size, alignment, bytes);
        assert_eq!(
            class14::unwind::split(&section),
            Err(expected),
            "{flags:#x}"
        );
    };
    check(STRINGS, 1, 1, b"ab", MergeError::Unterminated);
    check(STRINGS, 2, 2, b"a\0\0\0", MergeError::Unsupported);
    check(CONSTANTS, 0, 1, b"", MergeError::Unsupported);
    check(0x13, 8, 8, &[0; 8], MergeError::Unsupported);
    check(0x16, 8, 8, &[0; 8], MergeError::Unsupported);
    check(0x22, 1, 1, b"\0", MergeError::Unsupported);
    check(CONSTANTS, 8, 8192, &[0; 8], MergeError::Unsupported);
    check(CONSTANTS, 8, 3, &[0; 8], MergeError::Unsupported);
    check(CONSTANTS, 8, 8, &[0; 12], MergeError::Unsupported);
    // Shape is checked before string termination.
    check(STRINGS, 2, 2, b"ab", MergeError::Unsupported);
}

fn constants(values: &[u64]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_le_bytes()).collect()
}

#[test]
fn merge_shares_pieces_in_first_use_order() {
    let ab = constants(&[0xa, 0xb]);
    let bc = constants(&[0xb, 0xc]);
    let sections = [
        special(id(0, 4), STRINGS, 1, 1, b"hello\0world\0"),
        special(id(0, 5), CONSTANTS, 8, 8, &ab),
        special(id(1, 3), STRINGS, 1, 0, b"world\0hi\0hello\0"),
        special(id(1, 4), STRINGS, 1, 4, b"x\0yz\0"),
        special(id(2, 1), CONSTANTS, 8, 8, &bc),
    ];
    let merged = class14::unwind::merge(&sections).unwrap();
    let keys: Vec<_> = merged
        .groups
        .iter()
        .map(|g| (g.flags, g.entry_size, g.alignment))
        .collect();
    assert_eq!(keys, [(STRINGS, 1, 1), (CONSTANTS, 8, 8), (STRINGS, 1, 4)]);
    assert_eq!(merged.groups[0].bytes, b"hello\0world\0hi\0");
    assert_eq!(merged.groups[1].bytes, constants(&[0xa, 0xb, 0xc]));
    assert_eq!(merged.groups[2].bytes, b"x\0\0\0yz\0");
    assert_eq!(
        merged.sections[&id(1, 3)],
        (
            0,
            vec![(piece(0, 6), 6), (piece(6, 3), 12), (piece(9, 6), 0)]
        )
    );
    assert_eq!(
        merged.sections[&id(2, 1)],
        (1, vec![(piece(0, 8), 8), (piece(8, 8), 16)])
    );
    assert_eq!(
        merged.sections[&id(1, 4)],
        (2, vec![(piece(0, 2), 0), (piece(2, 3), 4)])
    );
    assert_eq!(class14::unwind::merge(&sections).unwrap(), merged);
    assert_eq!(
        class14::unwind::merge(&[]).unwrap(),
        class14::unwind::Merged {
            groups: vec![],
            sections: BTreeMap::new()
        }
    );

    let duplicate = [sections[0], sections[0]];
    assert_eq!(
        class14::unwind::merge(&duplicate),
        Err(Error::Output(class14::layout::Error::DuplicateIdentity))
    );
    let broken = [sections[0], special(id(3, 1), STRINGS, 1, 1, b"open")];
    assert_eq!(
        class14::unwind::merge(&broken),
        Err(Error::Merge {
            section: id(3, 1),
            cause: MergeError::Unterminated
        })
    );
    // 16385 distinct pieces at 4096-byte alignment exceed class14::executable::MAX_IMAGE.
    let many: Vec<u8> = (0..16_385u32)
        .flat_map(|i| [1 + (i % 255) as u8, 1 + (i / 255) as u8, 0])
        .collect();
    assert_eq!(
        class14::unwind::merge(&[special(id(0, 1), STRINGS, 1, 4096, &many)]),
        Err(Error::TooLarge)
    );
}

#[test]
fn locate_maps_interior_offsets() {
    let sections = [
        special(id(0, 4), STRINGS, 1, 1, b"hello\0world\0"),
        special(id(1, 3), STRINGS, 1, 1, b"world\0hi\0hello\0"),
        special(id(1, 4), STRINGS, 1, 4, b"x\0yz\0"),
    ];
    let merged = class14::unwind::merge(&sections).unwrap();
    assert_eq!(class14::unwind::locate(&merged, id(1, 3), 7), Ok((0, 13)));
    assert_eq!(class14::unwind::locate(&merged, id(1, 3), 0), Ok((0, 6)));
    assert_eq!(class14::unwind::locate(&merged, id(0, 4), 11), Ok((0, 11)));
    assert_eq!(class14::unwind::locate(&merged, id(1, 4), 2), Ok((1, 4)));
    let invalid = |section, offset| {
        Err(Error::Merge {
            section,
            cause: MergeError::InvalidReference(offset),
        })
    };
    assert_eq!(
        class14::unwind::locate(&merged, id(0, 4), 12),
        invalid(id(0, 4), 12)
    );
    assert_eq!(
        class14::unwind::locate(&merged, id(0, 4), u64::MAX),
        invalid(id(0, 4), u64::MAX)
    );
    assert_eq!(
        class14::unwind::locate(&merged, id(9, 9), 0),
        invalid(id(9, 9), 0)
    );
}

/// A CIE with augmentation "zR" unless overridden, padded to a multiple of 4.
fn cie_with(version: u8, augmentation: &[u8], data: &[u8]) -> Vec<u8> {
    let mut body = vec![0, 0, 0, 0, version];
    body.extend_from_slice(augmentation);
    body.extend_from_slice(&[0, 1, 0x78]);
    body.push(16); // Return-address column: one byte in v1, a ULEB128 in v3.
    body.push(data.len() as u8);
    body.extend_from_slice(data);
    body.extend_from_slice(&[0x0c, 0x07, 0x08, 0x90, 0x01]);
    while !body.len().is_multiple_of(4) {
        body.push(0);
    }
    framed(body)
}
fn cie() -> Vec<u8> {
    cie_with(1, b"zR", &[0x1b])
}
fn framed(body: Vec<u8>) -> Vec<u8> {
    let mut out = (body.len() as u32).to_le_bytes().to_vec();
    out.extend(body);
    out
}
/// An FDE whose CIE pointer field holds `pointer`.
fn fde(pointer: u32, range: u32) -> Vec<u8> {
    let mut body = pointer.to_le_bytes().to_vec();
    body.extend_from_slice(&[0; 4]);
    body.extend_from_slice(&range.to_le_bytes());
    body.extend_from_slice(&[0, 0, 0, 0]);
    framed(body)
}
fn pc32(offset: u64) -> class14::symbols::Rela {
    class14::symbols::Rela {
        offset,
        symbol: 1,
        kind: 2,
        addend: 0,
    }
}
/// CIE at 0, FDEs at 24 and 44, then a CIE (v3) at 64 and an FDE using it.
fn section() -> (Vec<u8>, Vec<u64>) {
    let mut bytes = cie();
    let mut fdes = Vec::new();
    for range in [6, 9] {
        fdes.push(bytes.len() as u64);
        let pointer = bytes.len() as u32 + 4;
        bytes.extend(fde(pointer, range));
    }
    let second = bytes.len() as u32;
    bytes.extend(cie_with(3, b"zR", &[0x1b]));
    fdes.push(bytes.len() as u64);
    let pointer = bytes.len() as u32 + 4 - second;
    bytes.extend(fde(pointer, 1));
    bytes.extend_from_slice(&[0; 4]);
    (bytes, fdes)
}

#[test]
fn frames_decode_cies_and_fdes() {
    let (bytes, fdes) = section();
    assert_eq!(fdes, [24, 44, 88]);
    let relocations = [
        pc32(fdes[2] + 8),
        class14::symbols::Rela {
            offset: 3,
            symbol: 0,
            kind: 0,
            addend: 0,
        },
        pc32(fdes[0] + 8),
        pc32(fdes[1] + 8),
    ];
    let expected = vec![
        Frame {
            offset: 0,
            size: 24,
            record: Record::Cie,
        },
        Frame {
            offset: 24,
            size: 20,
            record: Record::Fde {
                cie: 0,
                relocation: 2,
            },
        },
        Frame {
            offset: 44,
            size: 20,
            record: Record::Fde {
                cie: 0,
                relocation: 3,
            },
        },
        Frame {
            offset: 64,
            size: 24,
            record: Record::Cie,
        },
        Frame {
            offset: 88,
            size: 20,
            record: Record::Fde {
                cie: 64,
                relocation: 0,
            },
        },
    ];
    assert_eq!(
        class14::unwind::frames(&bytes, &relocations),
        Ok(expected.clone())
    );
    // The terminator is optional; an empty section has no records.
    assert_eq!(
        class14::unwind::frames(&bytes[..bytes.len() - 4], &relocations),
        Ok(expected)
    );
    assert_eq!(class14::unwind::frames(&[], &[]), Ok(vec![]));
    assert_eq!(class14::unwind::frames(&[0; 4], &[]), Ok(vec![]));
}

#[test]
fn frames_reject_malformed_unsupported_and_relocations() {
    use UnwindError::{Malformed, Relocation, Unsupported};
    let (bytes, fdes) = section();
    let relocations: Vec<_> = fdes.iter().map(|f| pc32(f + 8)).collect();
    let starts = [0u64, 24, 44, 64, 88, 108];
    // Every cut inside a record is reported at that record, without panicking.
    for cut in 1..bytes.len() as u64 {
        if starts.contains(&cut) {
            continue;
        }
        let record = *starts.iter().rev().find(|s| **s < cut).unwrap();
        let kept: Vec<_> = relocations
            .iter()
            .copied()
            .filter(|r| r.offset < cut)
            .collect();
        assert_eq!(
            class14::unwind::frames(&bytes[..cut as usize], &kept),
            Err(Malformed(record)),
            "cut at {cut}"
        );
    }
    let mut long = bytes.clone();
    long[..4].copy_from_slice(&u32::MAX.to_le_bytes());
    assert_eq!(
        class14::unwind::frames(&long, &relocations),
        Err(Unsupported(0))
    );
    for unsupported in [
        cie_with(2, b"zR", &[0x1b]),
        cie_with(1, b"zPLR", &[0x1b]),
        cie_with(1, b"zR", &[0x03]),
        cie_with(1, b"zR", &[0x1b, 0]),
        cie_with(1, b"", &[]),
    ] {
        assert_eq!(
            class14::unwind::frames(&unsupported, &[]),
            Err(Unsupported(0))
        );
    }
    let short = framed(vec![0, 0]);
    assert_eq!(class14::unwind::frames(&short, &[]), Err(Malformed(0)));
    let mut stray = cie();
    stray.extend_from_slice(&[0; 4]);
    stray.extend(cie());
    assert_eq!(class14::unwind::frames(&stray, &[]), Err(Malformed(24)));

    // An FDE must point back to a CIE, not to another FDE or past the start.
    let mut to_fde = bytes[..64].to_vec();
    to_fde[48..52].copy_from_slice(&24u32.to_le_bytes());
    assert_eq!(
        class14::unwind::frames(&to_fde, &relocations[..2]),
        Err(Malformed(44))
    );
    let mut forward = bytes[..44].to_vec();
    forward[28..32].copy_from_slice(&64u32.to_le_bytes());
    assert_eq!(
        class14::unwind::frames(&forward, &relocations[..1]),
        Err(Malformed(24))
    );

    // Framing errors precede relocation errors.
    assert_eq!(
        class14::unwind::frames(&long, &[pc32(1)]),
        Err(Unsupported(0))
    );
    assert_eq!(
        class14::unwind::frames(&bytes, &relocations[..2]),
        Err(Relocation(96))
    );
    let mut extra = relocations.clone();
    extra.push(pc32(4));
    assert_eq!(class14::unwind::frames(&bytes, &extra), Err(Relocation(4)));
    let mut twice = relocations.clone();
    twice.push(pc32(32));
    assert_eq!(class14::unwind::frames(&bytes, &twice), Err(Relocation(32)));
    let mut wide = relocations.clone();
    wide[0].kind = 1;
    assert_eq!(class14::unwind::frames(&bytes, &wide), Err(Relocation(32)));
}

const A: (usize, u16) = (0, 1);
const B: (usize, u16) = (1, 1);
const DATA: (usize, u16) = (0, 2);

fn planned(eh_size: u64, hdr_size: u64) -> class14::layout::Image {
    use class14::layout::{Contribution, Kind};
    let text_a = [0x90; 32];
    let text_b = [0xc3; 16];
    let eh = vec![0; eh_size as usize];
    let hdr = vec![0; hdr_size as usize];
    let contribution = |(file, section): (usize, u16), kind, alignment, bytes: &[u8]| {
        let memory_size = bytes.len() as u64;
        (file, section, kind, alignment, bytes.to_vec(), memory_size)
    };
    let parts = [
        contribution(A, Kind::Text, 16, &text_a),
        contribution(B, Kind::Text, 16, &text_b),
        contribution(DATA, Kind::ReadOnly, 8, &[0; 8]),
        contribution(EH_FRAME, Kind::ReadOnly, 8, &eh),
        contribution(EH_FRAME_HDR, Kind::ReadOnly, 4, &hdr),
    ];
    let contributions: Vec<_> = parts
        .iter()
        .map(
            |(file, section, kind, alignment, bytes, memory_size)| Contribution {
                file: *file,
                section: *section,
                kind: *kind,
                alignment: *alignment,
                bytes,
                memory_size: *memory_size,
            },
        )
        .collect();
    class14::layout::plan(&contributions, &[]).unwrap()
}
fn u32at(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(b[at..at + 4].try_into().unwrap())
}
fn i32at(b: &[u8], at: usize) -> i64 {
    i64::from(i32::from_le_bytes(b[at..at + 4].try_into().unwrap()))
}

#[test]
fn unwind_shares_cies_and_sorts_search_table() {
    let first = cie();
    let same = cie();
    let other = cie_with(3, b"zR", &[0x1b]);
    let (b0, a16, a0) = (fde(0xdead, 16), fde(0xbeef, 8), fde(7, 16));
    let fdes = [
        Fde {
            cie: &first,
            fde: &b0,
            target: B,
            start: 0,
        },
        Fde {
            cie: &same,
            fde: &a16,
            target: A,
            start: 16,
        },
        Fde {
            cie: &other,
            fde: &a0,
            target: A,
            start: 0,
        },
    ];
    let (eh_size, hdr_size) = class14::unwind::unwind_sizes(&fdes).unwrap();
    assert_eq!((eh_size, hdr_size), (24 + 20 + 20 + 24 + 20 + 4, 12 + 24));
    assert_eq!(class14::unwind::unwind_sizes(&[]), Ok((0, 0)));
    let image = planned(eh_size, hdr_size);
    let empty = class14::unwind::emit_unwind(&[], &image.placements).unwrap();
    assert!(empty.eh_frame.is_empty() && empty.hdr.is_empty());

    let unwind = class14::unwind::emit_unwind(&fdes, &image.placements).unwrap();
    let eh = image.placements[&EH_FRAME].address;
    let hdr = image.placements[&EH_FRAME_HDR].address;
    let (a, b) = (image.placements[&A].address, image.placements[&B].address);
    let out = &unwind.eh_frame;
    assert_eq!(out.len() as u64, eh_size);
    assert_eq!(&out[..24], first.as_slice());
    assert_eq!(&out[64..88], other.as_slice());
    assert_eq!(&out[out.len() - 4..], &[0; 4]);
    // CIE pointers are relative to the pointer field; locations to their field.
    for (at, cie, initial, range) in [(24, 0, b, 16), (44, 0, a + 16, 8), (88, 64, a, 16)] {
        assert_eq!(u32at(out, at + 4) as usize, at + 4 - cie);
        assert_eq!(
            eh as i64 + at as i64 + 8 + i32at(out, at + 8),
            initial as i64
        );
        assert_eq!(u32at(out, at + 12), range);
    }
    let h = &unwind.hdr;
    assert_eq!(h.len() as u64, hdr_size);
    assert_eq!(&h[..4], &[1, 0x1b, 0x03, 0x3b]);
    assert_eq!(hdr as i64 + 4 + i32at(h, 4), eh as i64);
    assert_eq!(u32at(h, 8), 3);
    let table: Vec<_> = (0..3)
        .map(|i| {
            (
                hdr as i64 + i32at(h, 12 + 8 * i),
                hdr as i64 + i32at(h, 16 + 8 * i),
            )
        })
        .collect();
    let eh = eh as i64;
    assert_eq!(
        table,
        [
            (a as i64, eh + 88),
            (a as i64 + 16, eh + 44),
            (b as i64, eh + 24)
        ]
    );
    assert_eq!(
        class14::unwind::emit_unwind(&fdes, &image.placements),
        Ok(unwind)
    );
}

#[test]
fn unwind_rejects_ranges_overlap_and_reservations() {
    let c = cie();
    let records = [fde(1, 16), fde(1, 4), fde(1, 0), fde(1, 33)];
    let make = |index: usize, target, start| Fde {
        cie: &c,
        fde: &records[index],
        target,
        start,
    };
    let check = |fdes: &[Fde<'_>], expected: Error| {
        let (eh, hdr) = class14::unwind::unwind_sizes(fdes).unwrap();
        let image = planned(eh, hdr);
        assert_eq!(
            class14::unwind::emit_unwind(fdes, &image.placements),
            Err(expected)
        );
    };
    check(
        &[make(0, A, 0), make(0, A, 17)],
        Error::InvalidFde { index: 1 },
    );
    check(&[make(3, A, 0)], Error::InvalidFde { index: 0 });
    check(
        &[make(0, B, 0), make(0, DATA, 0)],
        Error::InvalidFde { index: 1 },
    );
    check(&[make(0, (7, 7), 0)], Error::InvalidFde { index: 0 });
    check(&[make(0, A, u64::MAX)], Error::InvalidFde { index: 0 });
    check(
        &[make(1, A, 8), make(0, B, 0), make(0, A, 0)],
        Error::OverlappingFde { index: 0 },
    );
    check(
        &[make(2, A, 4), make(2, A, 4)],
        Error::OverlappingFde { index: 1 },
    );
    let mut torn = fde(1, 4);
    torn[0] = 9;
    check(
        &[Fde {
            cie: &c,
            fde: &torn,
            target: A,
            start: 0,
        }],
        Error::InvalidFde { index: 0 },
    );
    check(
        &[Fde {
            cie: &records[0],
            fde: &records[1],
            target: A,
            start: 0,
        }],
        Error::InvalidFde { index: 0 },
    );
    // Adjacent ranges are fine.
    let adjacent = [make(0, A, 0), make(0, A, 16)];
    let (eh, hdr) = class14::unwind::unwind_sizes(&adjacent).unwrap();
    assert!(class14::unwind::emit_unwind(&adjacent, &planned(eh, hdr).placements).is_ok());
    let wrong = planned(eh + 4, hdr);
    assert_eq!(
        class14::unwind::emit_unwind(&adjacent, &wrong.placements),
        Err(Error::InvalidReservation)
    );
    let mut missing = planned(eh, hdr).placements;
    missing.remove(&EH_FRAME_HDR);
    assert_eq!(
        class14::unwind::emit_unwind(&adjacent, &missing),
        Err(Error::InvalidReservation)
    );
}

#[test]
fn cie_signed_leb128_rejects_overflow() {
    let record = |last| {
        let mut body = vec![0, 0, 0, 0, 1, b'z', b'R', 0, 1];
        body.extend_from_slice(&[0x80; 9]);
        body.extend_from_slice(&[last, 16, 1, 0x1b]);
        framed(body)
    };
    for last in [0x00, 0x7f] {
        assert!(class14::unwind::frames(&record(last), &[]).is_ok());
    }
    for last in [0x01, 0x7e, 0x80, 0xff] {
        assert_eq!(
            class14::unwind::frames(&record(last), &[]),
            Err(UnwindError::Malformed(0))
        );
    }
}
