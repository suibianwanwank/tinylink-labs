//! Contract tests for TLS layout, relocation lowering, the GOT, and emission.
use std::collections::BTreeMap;

use class11::gc::{SectionId, Special};
use class11::tls::{
    Error, Got, Lowering, ReferenceError, Slot, Target, Tls, GOT, PHDR, PT_TLS, TLS_AREA, TLS_BOOT,
    TLS_FLAGS, TLS_IMAGE, TLS_STARTUP, TLS_STARTUP_SIZE,
};

fn id(file: usize, section: u16) -> SectionId {
    SectionId { file, section }
}
fn tdata(id: SectionId, alignment: u64, bytes: &[u8]) -> Special<'_> {
    Special {
        id,
        name: b".tdata",
        kind: 1,
        flags: TLS_FLAGS,
        size: bytes.len() as u64,
        entry_size: 0,
        alignment,
        bytes,
    }
}
fn tbss(id: SectionId, alignment: u64, size: u64) -> Special<'static> {
    Special {
        id,
        name: b".tbss",
        kind: 8,
        flags: TLS_FLAGS,
        size,
        entry_size: 0,
        alignment,
        bytes: &[],
    }
}

#[test]
fn tls_layout_orders_initialized_before_zero_fill() {
    let sections = [
        tbss(id(0, 3), 8, 8),
        tdata(id(0, 4), 4, &[1, 2, 3, 4]),
        tdata(id(0, 5), 0, &[9, 9, 9]),
        tbss(id(1, 2), 16, 4),
    ];
    let tls = class11::tls::tls_layout(&sections).unwrap();
    assert_eq!(
        tls,
        Tls {
            offsets: BTreeMap::from([(id(0, 4), 0), (id(0, 5), 4), (id(0, 3), 8), (id(1, 2), 16)]),
            image: vec![1, 2, 3, 4, 9, 9, 9],
            memory_size: 20,
            alignment: 16,
            thread_pointer: 32,
        }
    );
    assert_eq!(tls.tpoff(id(0, 4), 0), Some(-32));
    assert_eq!(tls.tpoff(id(1, 2), 3), Some(-13));
    assert_eq!(tls.tpoff(id(9, 9), 0), None);
    assert_eq!(
        class11::tls::tls_layout(&[]).unwrap(),
        Tls {
            offsets: BTreeMap::new(),
            image: vec![],
            memory_size: 0,
            alignment: 8,
            thread_pointer: 0,
        }
    );
    // The thread pointer stays a multiple of the alignment even when the
    // template ends exactly on it.
    let exact = class11::tls::tls_layout(&[tdata(id(0, 1), 8, &[7; 16])]).unwrap();
    assert_eq!((exact.memory_size, exact.thread_pointer), (16, 16));
}

#[test]
fn tls_layout_rejects_bad_sections() {
    let invalid = |section: Special<'_>| {
        assert_eq!(
            class11::tls::tls_layout(&[section]),
            Err(Error::InvalidTls(section.id))
        );
    };
    invalid(Special {
        flags: 3,
        ..tdata(id(0, 1), 1, &[1])
    });
    invalid(Special {
        kind: 4,
        ..tdata(id(0, 1), 1, &[1])
    });
    invalid(Special {
        size: 2,
        ..tdata(id(0, 1), 1, &[1])
    });
    invalid(Special {
        bytes: &[0],
        ..tbss(id(0, 1), 1, 1)
    });
    invalid(tbss(id(0, 1), 8192, 1));
    invalid(tbss(id(0, 1), 3, 1));
    assert_eq!(
        class11::tls::tls_layout(&[tbss(id(0, 1), 1, 1), tbss(id(0, 1), 1, 1)]),
        Err(Error::Output(class11::layout::Error::DuplicateIdentity))
    );
    assert_eq!(
        class11::tls::tls_layout(&[tbss(id(0, 1), 8, class11::executable::MAX_IMAGE)]),
        Err(Error::TooLarge)
    );
    assert_eq!(
        class11::tls::tls_layout(&[tbss(id(0, 1), 1, 8), tbss(id(0, 2), 1, u64::MAX)]),
        Err(Error::TooLarge)
    );
}

#[test]
fn tls_metadata_precedes_size_calculation() {
    let sections = [tbss(id(0, 1), 1, u64::MAX), tbss(id(0, 2), 3, 1)];
    assert_eq!(
        class11::tls::tls_layout(&sections),
        Err(Error::InvalidTls(id(0, 2)))
    );
}

/// `mov foo@GOTPCREL(%rip), %rax` with REX.W, and the same without REX.
const MOV_REX: [u8; 7] = [0x48, 0x8b, 0x05, 0, 0, 0, 0];
const MOV: [u8; 6] = [0x8b, 0x05, 0, 0, 0, 0];
/// `call *foo@GOTPCREL(%rip)`.
const CALL: [u8; 6] = [0xff, 0x15, 0, 0, 0, 0];

#[test]
fn lowering_selects_got_relaxation_and_thread_offsets() {
    use Lowering::*;
    use Target::*;
    let cases: [(&[u8], u64, u32, Target, Lowering); 15] = [
        (&MOV_REX, 3, 42, Image, Relax),
        (&MOV, 2, 41, Image, Relax),
        (&MOV_REX, 3, 42, Absolute, Got),
        (&CALL, 2, 41, Image, Got),
        (&MOV_REX, 3, 9, Image, Got),
        (&MOV_REX, 1, 42, Image, Got),
        (&MOV_REX, 99, 42, Image, Got),
        (&MOV_REX, 3, 22, Tls, Got),
        (&MOV_REX, 3, 23, Tls, ThreadOffset),
        (&MOV_REX, 3, 0, Tls, Fixup),
        (&MOV_REX, 3, 1, Image, Fixup),
        (&MOV_REX, 3, 2, Absolute, Fixup),
        (&MOV_REX, 3, 4, Image, Fixup),
        (&MOV_REX, 3, 11, Absolute, Fixup),
        (&MOV_REX, 3, 3, Image, Fixup),
    ];
    for (code, offset, kind, target, expected) in cases {
        assert_eq!(
            class11::tls::lower(code, offset, kind, target),
            Ok(expected),
            "kind {kind} offset {offset} {target:?}"
        );
    }
}

#[test]
fn lowering_rejects_tls_mismatches_and_models() {
    use ReferenceError::*;
    use Target::*;
    for (kind, target) in [
        (23, Image),
        (23, Absolute),
        (22, Image),
        (22, Absolute),
        (1, Tls),
        (2, Tls),
        (4, Tls),
        (9, Tls),
        (11, Tls),
        (41, Tls),
        (42, Tls),
    ] {
        assert_eq!(
            class11::tls::lower(&MOV_REX, 3, kind, target),
            Err(TlsMismatch),
            "kind {kind} {target:?}"
        );
    }
    for kind in [16, 17, 18, 19, 20, 21, 34, 35, 36] {
        for target in [Image, Absolute, Tls] {
            assert_eq!(
                class11::tls::lower(&MOV_REX, 3, kind, target),
                Err(UnsupportedTls),
                "kind {kind}"
            );
        }
    }
}

#[test]
fn got_deduplicates_in_first_use_order() {
    let mut got = Got::new();
    let a = Slot::Section(id(0, 1), 0);
    assert_eq!(got.size(), 0);
    assert_eq!(got.offset(a.clone()), Ok(0));
    assert_eq!(got.offset(Slot::Constant(5)), Ok(8));
    assert_eq!(got.offset(a.clone()), Ok(0));
    assert_eq!(got.offset(Slot::Common(b"c".to_vec())), Ok(16));
    assert_eq!(got.offset(Slot::Constant(5)), Ok(8));
    assert_eq!(got.offset(Slot::Section(id(0, 1), 4)), Ok(24));
    assert_eq!(got.size(), 32);
    assert_eq!(
        got.slots(),
        [
            a,
            Slot::Constant(5),
            Slot::Common(b"c".to_vec()),
            Slot::Section(id(0, 1), 4)
        ]
    );
}

fn plan_with_got(got_size: u64) -> class11::layout::Image {
    use class11::layout::{Common, Contribution, Kind};
    let got = vec![0; got_size as usize];
    let contributions = [
        Contribution {
            file: 0,
            section: 1,
            kind: Kind::Text,
            alignment: 16,
            bytes: &[0x90; 16],
            memory_size: 16,
        },
        Contribution {
            file: GOT.0,
            section: GOT.1,
            kind: Kind::Data,
            alignment: 8,
            bytes: &got,
            memory_size: got_size,
        },
    ];
    let commons = [Common {
        name: b"c",
        alignment: 8,
        size: 8,
    }];
    class11::layout::plan(&contributions, &commons).unwrap()
}

#[test]
fn got_encodes_constants_and_relative_records() {
    let mut got = Got::new();
    for slot in [
        Slot::Section(id(0, 1), 0),
        Slot::Constant(5),
        Slot::Common(b"c".to_vec()),
        Slot::Section(id(0, 1), 16),
    ] {
        got.offset(slot).unwrap();
    }
    let image = plan_with_got(32);
    let base = class11::executable::BASE;
    let got_rva = image.placements[&GOT].address - base;
    let text = image.placements[&(0, 1)].address - base;
    let common = image.common_addresses[&b"c".to_vec()] - base;
    let (bytes, records) = got.encode(&image).unwrap();
    let mut expected = vec![0; 32];
    expected[8..16].copy_from_slice(&5u64.to_le_bytes());
    assert_eq!(bytes, expected);
    let relative = |offset: u64, addend: u64| class11::pie::Relative {
        offset,
        addend: addend as i64,
    };
    assert_eq!(
        records,
        [
            relative(got_rva, text),
            relative(got_rva + 16, common),
            relative(got_rva + 24, text + 16)
        ]
    );
    assert_eq!(Got::new().encode(&plan_with_got(0)), Ok((vec![], vec![])));

    assert_eq!(
        got.encode(&plan_with_got(40)),
        Err(Error::InvalidReservation)
    );
    let mut missing = plan_with_got(32);
    missing.placements.remove(&GOT);
    assert_eq!(got.encode(&missing), Err(Error::InvalidReservation));
    for slot in [
        Slot::Section(id(5, 5), 0),
        Slot::Section(id(0, 1), 17),
        Slot::Common(b"other".to_vec()),
    ] {
        let mut one = Got::new();
        one.offset(slot).unwrap();
        assert_eq!(
            one.encode(&plan_with_got(8)),
            Err(Error::Output(class11::layout::Error::InvalidSymbol))
        );
    }
}

fn sample_tls() -> Tls {
    Tls {
        offsets: BTreeMap::from([(id(0, 3), 0)]),
        image: vec![1, 2, 3, 4],
        memory_size: 20,
        alignment: 16,
        thread_pointer: 32,
    }
}

/// Plan an image with every class9 and class11 reservation. `phdr_first`
/// places the program-header slot before user code, as required.
fn plan_with_tls(tls: &Tls, phdr_first: bool, area: u64) -> class11::layout::Image {
    use class11::layout::{Contribution, Kind};
    fn reserve<'a>(
        (file, section): (usize, u16),
        kind: Kind,
        alignment: u64,
        bytes: &'a [u8],
        memory_size: u64,
    ) -> Contribution<'a> {
        Contribution {
            file,
            section,
            kind,
            alignment,
            bytes,
            memory_size,
        }
    }
    let boot = vec![0; TLS_STARTUP_SIZE as usize];
    let user = reserve((0, 1), Kind::Text, 1, &[0x90, 0xc3], 2);
    let phdr = reserve(PHDR, Kind::Text, 8, &[0; 56], 56);
    let mut contributions = if phdr_first {
        vec![phdr, user]
    } else {
        vec![user, phdr]
    };
    contributions.extend([
        reserve(
            class11::pie::BOOT,
            Kind::Text,
            16,
            class11::pie::STARTUP,
            160,
        ),
        reserve(class11::pie::RELA, Kind::ReadOnly, 8, &[], 0),
        reserve(
            TLS_IMAGE,
            Kind::Data,
            tls.alignment,
            &tls.image,
            tls.image.len() as u64,
        ),
        reserve(TLS_AREA, Kind::Bss, tls.alignment, &[], area),
        reserve(TLS_BOOT, Kind::Text, 16, &boot, TLS_STARTUP_SIZE),
    ]);
    class11::layout::plan(&contributions, &[]).unwrap()
}
fn u16at(b: &[u8], at: usize) -> u16 {
    u16::from_le_bytes(b[at..at + 2].try_into().unwrap())
}
fn u32at(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(b[at..at + 4].try_into().unwrap())
}
fn u64at(b: &[u8], at: usize) -> u64 {
    u64::from_le_bytes(b[at..at + 8].try_into().unwrap())
}

#[test]
fn tls_startup_descriptor_and_program_header() {
    let tls = sample_tls();
    let image = plan_with_tls(&tls, true, 40);
    let rva = |key| image.placements[&key].address - class11::executable::BASE;
    let file = |key| image.placements[&key].file_offset.unwrap() as usize;
    let (user, boot, tls_boot, template, area) = (
        rva((0, 1)),
        file(class11::pie::BOOT),
        rva(TLS_BOOT),
        rva(TLS_IMAGE),
        rva(TLS_AREA),
    );
    let (tls_boot_file, template_file) = (file(TLS_BOOT), file(TLS_IMAGE));
    let bytes = class11::tls::emit(image, &[], Some(&tls), user + 1).unwrap();
    assert_eq!(u16at(&bytes, 16), 3);
    assert_eq!(u16at(&bytes, 56), 5);
    // Class9's startup enters the TLS stage, which enters user code.
    assert_eq!(u64at(&bytes, boot + 152), tls_boot);
    assert_eq!(TLS_STARTUP.len() as u64, TLS_STARTUP_SIZE);
    assert_eq!(
        &bytes[tls_boot_file..tls_boot_file + 128],
        &TLS_STARTUP[..128]
    );
    let descriptor: Vec<_> = (0..6)
        .map(|i| u64at(&bytes, tls_boot_file + 128 + 8 * i))
        .collect();
    assert_eq!(descriptor, [tls_boot, template, area, 4, 32, user + 1]);
    assert_eq!(&bytes[template_file..template_file + 4], &[1, 2, 3, 4]);
    let h = class11::layout::HEADER_BYTES as usize;
    assert_eq!((u32at(&bytes, h), u32at(&bytes, h + 4)), (PT_TLS, 4));
    let fields: Vec<_> = (0..6).map(|i| u64at(&bytes, h + 8 + 8 * i)).collect();
    assert_eq!(
        fields,
        [template_file as u64, template, template, 4, 20, 16]
    );
    // The four class6 program headers are unchanged in count and position.
    assert_eq!(u32at(&bytes, 64), 1);
    assert_eq!(u32at(&bytes, 64 + 3 * 56), 0x6474_e551);

    // Without TLS, emission is exactly class9's.
    let plain = || {
        class11::layout::plan(
            &[
                class11::layout::Contribution {
                    file: 0,
                    section: 1,
                    kind: class11::layout::Kind::Text,
                    alignment: 1,
                    bytes: &[0xc3],
                    memory_size: 1,
                },
                class11::layout::Contribution {
                    file: class11::pie::BOOT.0,
                    section: class11::pie::BOOT.1,
                    kind: class11::layout::Kind::Text,
                    alignment: 16,
                    bytes: class11::pie::STARTUP,
                    memory_size: 160,
                },
                class11::layout::Contribution {
                    file: class11::pie::RELA.0,
                    section: class11::pie::RELA.1,
                    kind: class11::layout::Kind::ReadOnly,
                    alignment: 8,
                    bytes: &[],
                    memory_size: 0,
                },
            ],
            &[],
        )
        .unwrap()
    };
    let entry = plain().placements[&(0, 1)].address - class11::executable::BASE;
    assert_eq!(
        class11::tls::emit(plain(), &[], None, entry),
        class11::pie::emit(plain(), &[], entry).map_err(Error::Pie)
    );
}

#[test]
fn emit_rejects_bad_tls_reservations() {
    let tls = sample_tls();
    let entry = |image: &class11::layout::Image, key| {
        image.placements[&key].address - class11::executable::BASE
    };
    let image = plan_with_tls(&tls, false, 40);
    let user = entry(&image, (0, 1));
    assert_eq!(
        class11::tls::emit(image, &[], Some(&tls), user),
        Err(Error::InvalidReservation)
    );
    let image = plan_with_tls(&tls, true, 48);
    let user = entry(&image, (0, 1));
    assert_eq!(
        class11::tls::emit(image, &[], Some(&tls), user),
        Err(Error::InvalidReservation)
    );
    let larger = Tls {
        image: vec![1, 2, 3, 4, 5],
        ..sample_tls()
    };
    let image = plan_with_tls(&tls, true, 40);
    let user = entry(&image, (0, 1));
    assert_eq!(
        class11::tls::emit(image, &[], Some(&larger), user),
        Err(Error::InvalidReservation)
    );
    let mut missing = plan_with_tls(&tls, true, 40);
    let user = entry(&missing, (0, 1));
    missing.placements.remove(&TLS_BOOT);
    assert_eq!(
        class11::tls::emit(missing, &[], Some(&tls), user),
        Err(Error::InvalidReservation)
    );
    for reserved in [TLS_BOOT, class11::pie::BOOT, PHDR] {
        let image = plan_with_tls(&tls, true, 40);
        let at = entry(&image, reserved);
        assert_eq!(
            class11::tls::emit(image, &[], Some(&tls), at),
            Err(Error::Pie(class11::pie::Error::InvalidEntry)),
            "{reserved:?}"
        );
    }
    let image = plan_with_tls(&tls, true, 40);
    let past = entry(&image, (0, 1)) + 2;
    assert_eq!(
        class11::tls::emit(image, &[], Some(&tls), past),
        Err(Error::Pie(class11::pie::Error::InvalidEntry))
    );
}
