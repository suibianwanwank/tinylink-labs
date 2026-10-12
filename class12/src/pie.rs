//! Freestanding static PIE with explicit startup-relative relocations.
pub const STARTUP: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/startup.bin"));
pub const BOOT: (usize, u16) = (usize::MAX, 0);
pub const RELA: (usize, u16) = (usize::MAX, 1);
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Value {
    Absolute(u64),
    ImageRva(u64),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Relative {
    pub offset: u64,
    pub addend: i64,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fixup {
    pub offset: u64,
    pub kind: u32,
    pub value: Value,
    pub addend: i64,
    pub place_rva: u64,
    pub writable: bool,
}
#[derive(Debug, PartialEq, Eq)]
pub enum PatchError {
    Relocation(crate::relocate::Error),
    NonPicReference,
    TextRelocation,
}
impl From<crate::relocate::Error> for PatchError {
    fn from(e: crate::relocate::Error) -> Self {
        Self::Relocation(e)
    }
}
#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    FrontEnd(crate::gc::Error),
    Output(crate::layout::Error),
    Fixup {
        section: crate::gc::SectionId,
        offset: u64,
        kind: u32,
        cause: PatchError,
    },
    InvalidRuntime,
    InvalidRelative {
        offset: u64,
    },
    InvalidEntry,
    TooLarge,
}
impl From<crate::gc::Error> for Error {
    fn from(e: crate::gc::Error) -> Self {
        Self::FrontEnd(e)
    }
}
impl From<crate::layout::Error> for Error {
    fn from(e: crate::layout::Error) -> Self {
        Self::Output(e)
    }
}
/// Apply a link-time fixup, or zero an eight-byte field and return its RELATIVE
/// record. All coordinates are RVAs, never crate::executable::BASE-based addresses. NONE
/// ignores all arguments. R64 to image storage requires writable=true. PC32 and
/// PLT32 to image storage are static; PC-relative ABS/zero and narrow absolute
/// references to image storage are NonPicReference. Absolute R64/32/32S are static.
/// Use class5 for static field checks. Failure leaves bytes unchanged.
pub fn patch(bytes: &mut [u8], fixup: Fixup) -> Result<Option<Relative>, PatchError> {
    let Fixup {
        offset,
        kind,
        value,
        addend,
        place_rva,
        writable,
    } = fixup;
    if kind == 0 {
        return Ok(None);
    }
    match (kind, value) {
        (1, Value::ImageRva(value)) => {
            if !writable {
                return Err(PatchError::TextRelocation);
            }
            let end = offset
                .checked_add(8)
                .ok_or(crate::relocate::Error::PatchBounds)?;
            if end > bytes.len() as u64 {
                return Err(crate::relocate::Error::PatchBounds.into());
            }
            let addend = value.wrapping_add(addend as u64) as i64;
            bytes[offset as usize..end as usize].fill(0);
            Ok(Some(Relative {
                offset: place_rva,
                addend,
            }))
        }
        (2 | 4, Value::Absolute(_)) | (10 | 11, Value::ImageRva(_)) => {
            Err(PatchError::NonPicReference)
        }
        (_, Value::Absolute(value) | Value::ImageRva(value)) => {
            crate::relocate::apply(bytes, offset, kind, value, addend, place_rva)?;
            Ok(None)
        }
    }
}

/// Sort by destination RVA and emit ELF64 RELA records (r_info=8, symbol=0).
/// Reject overlapping eight-byte destinations, checked-end overflow, or writes
/// past crate::executable::MAX_IMAGE. Do not constrain signed addends or require alignment.
pub fn encode_relative(records: &[Relative]) -> Result<Vec<u8>, Error> {
    let mut ordered = records.to_vec();
    ordered.sort_by_key(|r| r.offset);
    let mut previous_end = 0;
    let mut out = Vec::new();
    for r in ordered {
        let end = r
            .offset
            .checked_add(8)
            .filter(|end| *end <= crate::executable::MAX_IMAGE)
            .ok_or(Error::InvalidRelative { offset: r.offset })?;
        if r.offset < previous_end {
            return Err(Error::InvalidRelative { offset: r.offset });
        }
        previous_end = end;
        out.extend_from_slice(&r.offset.to_le_bytes());
        out.extend_from_slice(&8u64.to_le_bytes());
        out.extend_from_slice(&r.addend.to_le_bytes());
    }
    Ok(out)
}

/// Consume an image produced by crate::layout::plan, with only real payloads patched.
/// BOOT reserves 160 Text bytes, aligned to 16; RELA reserves exactly 24*N
/// ReadOnly bytes, aligned to 8. Validate these placements and the user entry.
/// Every relative write must fit inside one Data placement (not BSS or padding).
/// Fill the supplied startup descriptor and RELA table. Emit via class6, then
/// convert ET_EXEC coordinates to zero-based ET_DYN. Preserve RX/R/RW and NX
/// stack. No PT_INTERP or PT_DYNAMIC; the supplied startup owns relocation.
pub fn emit(
    image: crate::layout::Image,
    records: &[Relative],
    entry_rva: u64,
) -> Result<Vec<u8>, Error> {
    let mut image = image;
    let encoded = encode_relative(records)?;
    let boot = image.placements.get(&BOOT).ok_or(Error::InvalidRuntime)?;
    let table = image.placements.get(&RELA).ok_or(Error::InvalidRuntime)?;
    if STARTUP.len() != 160
        || boot.kind != crate::layout::Kind::Text
        || boot.size != 160
        || boot.address % 16 != 0
        || table.kind != crate::layout::Kind::ReadOnly
        || table.size != encoded.len() as u64
        || table.address % 8 != 0
    {
        return Err(Error::InvalidRuntime);
    }
    let boot_file = boot.file_offset.ok_or(Error::InvalidRuntime)? as usize;
    let table_file = table.file_offset.ok_or(Error::InvalidRuntime)? as usize;
    let (boot_rva, table_rva) = (
        boot.address - crate::executable::BASE,
        table.address - crate::executable::BASE,
    );
    if !image.placements.iter().any(|(id, p)| {
        *id != BOOT
            && p.kind == crate::layout::Kind::Text
            && entry_rva >= p.address - crate::executable::BASE
            && entry_rva - (p.address - crate::executable::BASE) < p.size
    }) {
        return Err(Error::InvalidEntry);
    }
    for r in records {
        if !image.placements.values().any(|p| {
            p.kind == crate::layout::Kind::Data
                && r.offset >= p.address - crate::executable::BASE
                && r.offset - (p.address - crate::executable::BASE) <= p.size
                && p.size - (r.offset - (p.address - crate::executable::BASE)) >= 8
        }) {
            return Err(Error::InvalidRelative { offset: r.offset });
        }
    }
    image.bytes[boot_file..boot_file + 160].copy_from_slice(STARTUP);
    image.bytes[table_file..table_file + encoded.len()].copy_from_slice(&encoded);
    for (offset, value) in [
        (128, boot_rva),
        (136, table_rva),
        (144, records.len() as u64),
        (152, entry_rva),
    ] {
        image.bytes[boot_file + offset..boot_file + offset + 8]
            .copy_from_slice(&value.to_le_bytes());
    }
    let mut bytes = crate::layout::emit(image, crate::executable::BASE + boot_rva)?;
    bytes[16..18].copy_from_slice(&3u16.to_le_bytes());
    bytes[24..32].copy_from_slice(&boot_rva.to_le_bytes());
    for i in 0..3 {
        for field in [16, 24] {
            let start = 64 + i * 56 + field;
            let v = u64::from_le_bytes(bytes[start..start + 8].try_into().unwrap())
                - crate::executable::BASE;
            bytes[start..start + 8].copy_from_slice(&v.to_le_bytes());
        }
    }
    Ok(bytes)
}

/// Reuse crate::gc::prepare, reserve startup/table storage, plan live contributions,
/// classify bound symbols as Absolute or ImageRva, patch each live relocation,
/// and emit. Keep archive/GC/root policy unchanged. Map user sections and COMMON
/// using RVAs, excluding the two synthetic contributions; prefix the map header
/// with TINYLINK PIE MAP 1. README specifies record counting and limits.
pub fn link(
    inputs: &[crate::archive::Input<'_>],
    entry: &[u8],
    keep: &[&[u8]],
) -> Result<crate::gc::Linked, Error> {
    use crate::layout::{Contribution, Kind};
    use crate::resolve::{Binding, Selection, SymbolId};
    use crate::symbols::Definition;
    let prepared = crate::gc::prepare(inputs, entry, keep)?;
    let bind = |file: usize, symbol: u32| {
        crate::resolve::bind(
            prepared.tables(),
            prepared.globals(),
            SymbolId {
                file,
                symbol: symbol as usize,
            },
        )
        .map_err(crate::layout::Error::from)
    };
    let mut count = 0usize;
    for (source, records) in prepared.relocations() {
        for r in records {
            if r.kind != 1 {
                continue;
            }
            let needs_bias = match bind(source.file, r.symbol)? {
                Binding::Zero => false,
                Binding::Selected(Selection::Common { .. }) => true,
                Binding::Selected(Selection::Definition(id)) => matches!(
                    prepared.tables()[id.file][id.symbol].definition,
                    Definition::Section(_)
                ),
            };
            if needs_bias {
                count = count.checked_add(1).ok_or(Error::TooLarge)?;
            }
        }
    }
    let table_size = count
        .checked_mul(24)
        .filter(|n| *n as u64 <= crate::executable::MAX_IMAGE)
        .ok_or(Error::TooLarge)?;
    let table_bytes = vec![0; table_size];
    let mut contributions: Vec<_> = prepared
        .contributions()
        .iter()
        .map(|c| Contribution {
            file: c.file,
            section: c.section,
            kind: c.kind,
            alignment: c.alignment,
            bytes: c.bytes,
            memory_size: c.memory_size,
        })
        .collect();
    contributions.push(Contribution {
        file: BOOT.0,
        section: BOOT.1,
        kind: Kind::Text,
        alignment: 16,
        bytes: STARTUP,
        memory_size: STARTUP.len() as u64,
    });
    contributions.push(Contribution {
        file: RELA.0,
        section: RELA.1,
        kind: Kind::ReadOnly,
        alignment: 8,
        bytes: &table_bytes,
        memory_size: table_bytes.len() as u64,
    });
    let commons: Vec<_> = prepared
        .globals()
        .iter()
        .filter_map(|(name, s)| match s {
            Selection::Common {
                size, alignment, ..
            } if prepared.live().commons.contains(name) => Some(crate::layout::Common {
                name,
                size: *size,
                alignment: *alignment,
            }),
            _ => None,
        })
        .collect();
    let mut image = crate::layout::plan(&contributions, &commons)?;
    let (entry, entry_offset) = prepared.entry();
    let entry_rva = image.placements[&(entry.file, entry.section)].address
        - crate::executable::BASE
        + entry_offset;
    let mut relative = Vec::new();
    for (source, records) in prepared.relocations() {
        let p = &image.placements[&(source.file, source.section)];
        let (file, size, rva, writable) = (
            p.file_offset,
            p.size,
            p.address - crate::executable::BASE,
            p.kind == Kind::Data,
        );
        for r in records {
            if r.kind == 0 {
                continue;
            }
            let start = file.ok_or(crate::layout::Error::InvalidRelocationTarget)? as usize;
            let value = match bind(source.file, r.symbol)? {
                Binding::Zero => Value::Absolute(0),
                Binding::Selected(Selection::Common { .. }) => Value::ImageRva(
                    image.common_addresses[prepared.tables()[source.file][r.symbol as usize].name]
                        - crate::executable::BASE,
                ),
                Binding::Selected(Selection::Definition(id)) => {
                    let s = &prepared.tables()[id.file][id.symbol];
                    match s.definition {
                        Definition::Absolute => Value::Absolute(s.value),
                        Definition::Section(sec) => Value::ImageRva(
                            image.placements[&(id.file, sec)].address - crate::executable::BASE
                                + s.value,
                        ),
                        _ => return Err(Error::FrontEnd(crate::gc::Error::InvalidSymbol)),
                    }
                }
            };
            let place_rva = rva.checked_add(r.offset).ok_or(Error::Fixup {
                section: *source,
                offset: r.offset,
                kind: r.kind,
                cause: PatchError::Relocation(crate::relocate::Error::PatchBounds),
            })?;
            let fixup = Fixup {
                offset: r.offset,
                kind: r.kind,
                value,
                addend: r.addend,
                place_rva,
                writable,
            };
            if let Some(record) = patch(&mut image.bytes[start..start + size as usize], fixup)
                .map_err(|cause| Error::Fixup {
                    section: *source,
                    offset: r.offset,
                    kind: r.kind,
                    cause,
                })?
            {
                relative.push(record);
            }
        }
    }
    let mut rows = prepared.map_rows().to_vec();
    for row in &mut rows {
        row.address = image
            .placements
            .get(&(row.id.file, row.id.section))
            .map(|p| p.address - crate::executable::BASE);
    }
    let commons = image
        .common_addresses
        .iter()
        .map(|(name, address)| (name.clone(), address - crate::executable::BASE))
        .collect();
    let map =
        crate::gc::render_map(&rows, &commons).replacen("TINYLINK MAP 1", "TINYLINK PIE MAP 1", 1);
    Ok(crate::gc::Linked {
        bytes: emit(image, &relative, entry_rva)?,
        map,
    })
}
