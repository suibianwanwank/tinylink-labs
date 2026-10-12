//! Debug information, symbols, and section headers for ET_EXEC or static PIE.
//!
//! A debugger needs more than loadable segments: it finds DWARF and symbols
//! through section headers. This backend keeps the class8 image unchanged,
//! relocates every `.debug_*` input into its output section, and appends those
//! sections, `.symtab`, and a section header table. README is the contract.
use std::collections::BTreeMap;

use crate::gc::{SectionId, Special};

/// `SHF_MERGE | SHF_STRINGS`, used by `.debug_str` and `.debug_line_str`.
pub const DEBUG_STRINGS: u64 = 0x30;
/// `SHN_ABS`: the section index of an absolute symbol.
pub const SHN_ABS: u16 = 0xfff1;

/// One output debug section: same-named inputs, concatenated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebugSection {
    pub name: Vec<u8>,
    /// Flags of the first input: 0, or [`DEBUG_STRINGS`].
    pub flags: u64,
    /// Entry size of the first input.
    pub entry_size: u64,
    /// Largest normalized input alignment.
    pub alignment: u64,
    pub bytes: Vec<u8>,
}

/// Output debug sections and where each input landed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Debug {
    /// In order of first appearance of each name.
    pub sections: Vec<DebugSection>,
    /// For each input: output section index and byte offset within it.
    pub offsets: BTreeMap<SectionId, (usize, u64)>,
}

/// What a debug relocation refers to after layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebugTarget {
    /// A final virtual address: live storage, a COMMON, an ABS value, or zero.
    Address(u64),
    /// Byte `offset` of output debug section `index`. DWARF stores such
    /// references as section-relative offsets, not addresses.
    Debug {
        index: usize,
        offset: u64,
    },
    TlsOffset(u64),
    /// Storage discarded by GC: write the tombstone.
    Discarded,
}

/// Why one debug relocation failed.
#[derive(Debug, PartialEq, Eq)]
pub enum DebugError {
    /// Only NONE, R_X86_64_64, and R_X86_64_32 occur in supported debug data.
    Unsupported(u32),
    /// Field bounds or value range, as class5 reports them.
    Relocation(crate::relocate::Error),
}

/// One symbol to write into `.symtab`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OutputSymbol<'a> {
    pub name: &'a [u8],
    /// `st_info`: binding in the high nibble, type in the low nibble.
    pub info: u8,
    /// Output section index, or [`SHN_ABS`].
    pub section: u16,
    pub value: u64,
    pub size: u64,
}

/// Encoded `.symtab` and `.strtab`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolTable {
    pub symtab: Vec<u8>,
    pub strtab: Vec<u8>,
    /// Index of the first non-local symbol: `.symtab`'s `sh_info`.
    pub first_global: u32,
}

/// Where a section's bytes come from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Contents {
    /// Already in the emitted file, e.g. `.text` inside a load segment.
    File { offset: u64, size: u64 },
    /// Appended after the existing file.
    Data(Vec<u8>),
    /// Occupies memory only, like `.bss`.
    NoBits { size: u64 },
}

/// One entry of the output section header table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputSection {
    pub name: Vec<u8>,
    pub kind: u32,
    pub flags: u64,
    pub address: u64,
    pub link: u32,
    pub info: u32,
    pub alignment: u64,
    pub entry_size: u64,
    pub contents: Contents,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    FrontEnd(crate::gc::Error),
    Output(crate::layout::Error),
    MergeUnwind(crate::unwind::Error),
    PieTls(crate::tls::Error),
    /// A `.debug_*` input that is compressed or has unsupported flags.
    UnsupportedDebug(SectionId),
    /// A debug relocation and its failure.
    Debug {
        section: SectionId,
        offset: u64,
        kind: u32,
        cause: DebugError,
    },
    /// A debug relocation naming non-debug metadata such as `.comment`.
    InvalidDebugReference {
        section: SectionId,
        offset: u64,
    },
    /// A symbol name containing NUL.
    InvalidName,
    /// Malformed input to `attach_sections`.
    InvalidSection,
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

impl From<crate::unwind::Error> for Error {
    fn from(e: crate::unwind::Error) -> Self {
        Self::MergeUnwind(e)
    }
}

impl From<crate::tls::Error> for Error {
    fn from(e: crate::tls::Error) -> Self {
        Self::PieTls(e)
    }
}

/// Concatenate every metadata section whose name starts with `.debug_`,
/// grouped by exact name in order of first appearance. Ignore other
/// metadata. README defines alignment, flags, and size limits.
pub fn collect_debug(metadata: &[Special<'_>]) -> Result<Debug, Error> {
    use std::collections::btree_map::Entry;
    let mut sections: Vec<DebugSection> = Vec::new();
    let mut by_name: BTreeMap<&[u8], usize> = BTreeMap::new();
    let mut offsets = BTreeMap::new();
    let mut total_bytes = 0u64;
    for input in metadata.iter().filter(|m| m.name.starts_with(b".debug_")) {
        let alignment = input.alignment.max(1);
        if (input.flags != 0 && input.flags != DEBUG_STRINGS)
            || !alignment.is_power_of_two()
            || alignment > 4096
        {
            return Err(Error::UnsupportedDebug(input.id));
        }
        let index = *by_name.entry(input.name).or_insert_with(|| {
            sections.push(DebugSection {
                name: input.name.to_vec(),
                flags: input.flags,
                entry_size: input.entry_size,
                alignment: 1,
                bytes: Vec::new(),
            });
            sections.len() - 1
        });
        let output = &mut sections[index];
        if output.flags != input.flags {
            return Err(Error::UnsupportedDebug(input.id));
        }
        let Entry::Vacant(slot) = offsets.entry(input.id) else {
            return Err(crate::layout::Error::DuplicateIdentity.into());
        };
        let start = (output.bytes.len() as u64).div_ceil(alignment) * alignment;
        let end = start
            .checked_add(input.bytes.len() as u64)
            .ok_or(Error::TooLarge)?;
        let growth = end - output.bytes.len() as u64;
        total_bytes = total_bytes.checked_add(growth).ok_or(Error::TooLarge)?;
        if total_bytes > crate::executable::MAX_IMAGE {
            return Err(Error::TooLarge);
        }
        output.alignment = output.alignment.max(alignment);
        output.bytes.resize(start as usize, 0);
        output.bytes.extend_from_slice(input.bytes);
        slot.insert((index, start));
    }
    Ok(Debug { sections, offsets })
}

/// Apply one relocation `r` to the bytes of one input's region of an output
/// debug section named `section_name`. Discarded targets receive the
/// tombstone defined in README; the addend is then ignored. Leave `bytes`
/// unchanged on error.
pub fn relocate_debug(
    bytes: &mut [u8],
    section_name: &[u8],
    r: &crate::symbols::Rela,
    target: DebugTarget,
) -> Result<(), DebugError> {
    if r.kind == 0 {
        return Ok(());
    }
    let kind = match (r.kind, target) {
        (17, DebugTarget::TlsOffset(_)) => 1,
        (21, DebugTarget::TlsOffset(_)) => 11,
        (1 | 10, DebugTarget::TlsOffset(_)) => return Err(DebugError::Unsupported(r.kind)),
        (1 | 10, _) => r.kind,
        _ => return Err(DebugError::Unsupported(r.kind)),
    };
    let (value, addend) = match target {
        DebugTarget::Address(address) => (address, r.addend),
        DebugTarget::TlsOffset(offset) => (offset, r.addend),
        DebugTarget::Debug { offset, .. } => (offset, r.addend),
        DebugTarget::Discarded => {
            let tombstone =
                u64::from(section_name == b".debug_ranges" || section_name == b".debug_loc");
            (tombstone, 0)
        }
    };
    crate::relocate::apply(bytes, r.offset, kind, value, addend, 0).map_err(DebugError::Relocation)
}

/// Encode a null symbol, every local symbol (binding 0) in supplied order,
/// then every other symbol in supplied order. Names are NUL-terminated in
/// `strtab`; identical names share one string, and an empty name uses offset 0.
pub fn encode_symbols(symbols: &[OutputSymbol<'_>]) -> Result<SymbolTable, Error> {
    let mut strtab = vec![0u8];
    let mut names: BTreeMap<&[u8], u32> = BTreeMap::new();
    let mut symtab = vec![0u8; 24];
    let ordered = symbols
        .iter()
        .filter(|s| s.info >> 4 == 0)
        .chain(symbols.iter().filter(|s| s.info >> 4 != 0));
    for symbol in ordered {
        if symbol.name.contains(&0) {
            return Err(Error::InvalidName);
        }
        let name = if symbol.name.is_empty() {
            0
        } else if let Some(offset) = names.get(symbol.name) {
            *offset
        } else {
            let offset = u32::try_from(strtab.len()).map_err(|_| Error::TooLarge)?;
            strtab.extend_from_slice(symbol.name);
            strtab.push(0);
            names.insert(symbol.name, offset);
            offset
        };
        symtab.extend_from_slice(&name.to_le_bytes());
        symtab.push(symbol.info);
        symtab.push(0);
        symtab.extend_from_slice(&symbol.section.to_le_bytes());
        symtab.extend_from_slice(&symbol.value.to_le_bytes());
        symtab.extend_from_slice(&symbol.size.to_le_bytes());
    }
    let locals = symbols.iter().filter(|s| s.info >> 4 == 0).count();
    let first_global = u32::try_from(1 + locals).map_err(|_| Error::TooLarge)?;
    if u32::try_from(strtab.len()).is_err() {
        return Err(Error::TooLarge);
    }
    Ok(SymbolTable {
        symtab,
        strtab,
        first_global,
    })
}

/// Append `Data` sections, a generated `.shstrtab`, and a section header
/// table to an emitted ELF that has none, then point the ELF header at it.
/// Section `i` of `sections` becomes header `i + 1`; header 0 is null.
pub fn attach_sections(elf: Vec<u8>, sections: &[OutputSection]) -> Result<Vec<u8>, Error> {
    let mut elf = elf;
    let u16at = |b: &[u8], at: usize| u16::from_le_bytes(b[at..at + 2].try_into().unwrap());
    if elf.len() < 64 || &elf[..4] != b"\x7fELF" || elf[40..48] != [0; 8] || u16at(&elf, 60) != 0 {
        return Err(Error::InvalidSection);
    }
    if sections.len().checked_add(2).ok_or(Error::TooLarge)? >= 0xff00 {
        return Err(Error::TooLarge);
    }
    // Bound padding and metadata before allocation, just like the load image.
    let checked_end = |offset: u64, size: u64| -> Result<usize, Error> {
        let end = offset.checked_add(size).ok_or(Error::TooLarge)?;
        if end > crate::executable::MAX_IMAGE {
            return Err(Error::TooLarge);
        }
        usize::try_from(end).map_err(|_| Error::TooLarge)
    };
    let original = elf.len() as u64;
    checked_end(original, 0)?;
    let mut names = vec![0u8];
    let mut headers = vec![0u8; 64];
    let name_offset = |name: &[u8], names: &mut Vec<u8>| -> Result<u32, Error> {
        if name.contains(&0) {
            return Err(Error::InvalidSection);
        }
        checked_end(names.len() as u64, name.len() as u64 + 1)?;
        let offset = u32::try_from(names.len()).map_err(|_| Error::TooLarge)?;
        names.extend_from_slice(name);
        names.push(0);
        Ok(offset)
    };
    let header = |headers: &mut Vec<u8>,
                  name: u32,
                  kind: u32,
                  flags: u64,
                  address: u64,
                  offset: u64,
                  size: u64,
                  link: u32,
                  info: u32,
                  alignment: u64,
                  entry_size: u64| {
        headers.extend_from_slice(&name.to_le_bytes());
        headers.extend_from_slice(&kind.to_le_bytes());
        for value in [flags, address, offset, size] {
            headers.extend_from_slice(&value.to_le_bytes());
        }
        headers.extend_from_slice(&link.to_le_bytes());
        headers.extend_from_slice(&info.to_le_bytes());
        headers.extend_from_slice(&alignment.to_le_bytes());
        headers.extend_from_slice(&entry_size.to_le_bytes());
    };
    for s in sections {
        if s.alignment != 0 && !s.alignment.is_power_of_two() {
            return Err(Error::InvalidSection);
        }
        let name = name_offset(&s.name, &mut names)?;
        let (offset, size) = match &s.contents {
            Contents::File { offset, size } => {
                if offset.checked_add(*size).is_none_or(|end| end > original) {
                    return Err(Error::InvalidSection);
                }
                (*offset, *size)
            }
            Contents::NoBits { size } => (original, *size),
            Contents::Data(bytes) => {
                let alignment = s.alignment.max(1);
                let start = (elf.len() as u64)
                    .checked_add(alignment - 1)
                    .ok_or(Error::TooLarge)?
                    & !(alignment - 1);
                checked_end(start, bytes.len() as u64)?;
                elf.resize(usize::try_from(start).map_err(|_| Error::TooLarge)?, 0);
                elf.extend_from_slice(bytes);
                (start, bytes.len() as u64)
            }
        };
        header(
            &mut headers,
            name,
            s.kind,
            s.flags,
            s.address,
            offset,
            size,
            s.link,
            s.info,
            s.alignment,
            s.entry_size,
        );
    }
    let shstrtab_name = name_offset(b".shstrtab", &mut names)?;
    let names_offset = elf.len() as u64;
    checked_end(names_offset, names.len() as u64)?;
    elf.extend_from_slice(&names);
    header(
        &mut headers,
        shstrtab_name,
        3,
        0,
        0,
        names_offset,
        names.len() as u64,
        0,
        0,
        1,
        0,
    );
    let table = (elf.len() as u64).checked_add(7).ok_or(Error::TooLarge)? & !7;
    checked_end(table, headers.len() as u64)?;
    elf.resize(usize::try_from(table).map_err(|_| Error::TooLarge)?, 0);
    elf.extend_from_slice(&headers);
    let count = sections.len() as u16 + 2;
    elf[40..48].copy_from_slice(&table.to_le_bytes());
    elf[58..60].copy_from_slice(&64u16.to_le_bytes());
    elf[60..62].copy_from_slice(&count.to_le_bytes());
    elf[62..64].copy_from_slice(&(count - 1).to_le_bytes());
    Ok(elf)
}

/// Sections folded into another by a later backend: each key's bytes are not
/// in the image, and its symbols resolve into the value section at the same
/// offset. Empty for class12 itself.
pub type Aliases = BTreeMap<SectionId, SectionId>;

/// An executable whose addresses have already been assigned. Neither variant
/// permits this stage to repeat layout. The plain variant also supports the
/// ICF backend's existing interface.
#[derive(Debug, PartialEq, Eq)]
pub enum Lowered {
    Plain(crate::gc::Lowered),
    MergeUnwind(crate::unwind::Lowered),
    PieTls(crate::tls::Lowered),
}
impl From<crate::gc::Lowered> for Lowered {
    fn from(value: crate::gc::Lowered) -> Self {
        Self::Plain(value)
    }
}
impl From<crate::unwind::Lowered> for Lowered {
    fn from(value: crate::unwind::Lowered) -> Self {
        Self::MergeUnwind(value)
    }
}
impl From<crate::tls::Lowered> for Lowered {
    fn from(value: crate::tls::Lowered) -> Self {
        Self::PieTls(value)
    }
}

impl Lowered {
    /// Placements and patched storage; ELF headers are serialized by `emit`.
    pub fn image(&self) -> &crate::layout::Image {
        match self {
            Self::Plain(value) => &value.image,
            Self::MergeUnwind(value) => &value.image,
            Self::PieTls(value) => &value.image,
        }
    }

    /// Supplied serializer: retain the upstream program-header policy,
    /// including `PT_GNU_EH_FRAME`. Append debug sections to the returned bytes.
    pub fn emit(self) -> Result<crate::gc::Linked, Error> {
        match self {
            Self::Plain(value) => Ok(crate::gc::Linked {
                bytes: crate::layout::emit(value.image, value.entry_address)?,
                map: String::new(),
            }),
            Self::MergeUnwind(value) => Ok(value.emit()?),
            Self::PieTls(value) => Ok(value.emit()?),
        }
    }
}

/// Relocate debug information, build `.symtab`, and attach section headers to
/// an image lowered from `prepared`. Debug references to aliased sections are
/// discarded; symbols and map rows of aliased sections use their target's
/// placement. README defines resolution, symbols, and output sections.
pub fn finish(
    prepared: &crate::gc::Prepared<'_>,
    lowered: impl Into<Lowered>,
    aliases: &Aliases,
) -> Result<crate::gc::Linked, Error> {
    let lowered = lowered.into();
    use crate::layout::Kind;
    use crate::resolve::{Binding, Selection, SymbolId};
    use crate::symbols::Definition;
    let image = lowered.image();
    let merged = match &lowered {
        Lowered::Plain(_) => None,
        Lowered::MergeUnwind(value) => Some(&value.merged),
        Lowered::PieTls(value) => Some(&value.merged),
    };
    let pie = matches!(lowered, Lowered::PieTls(_));
    let tls = match &lowered {
        Lowered::PieTls(value) => value.tls.as_ref(),
        _ => None,
    };
    let coordinate = |value: u64| -> Result<u64, Error> {
        if pie {
            value
                .checked_sub(crate::executable::BASE)
                .ok_or(Error::InvalidSection)
        } else {
            Ok(value)
        }
    };
    let unwind_ids = if pie {
        [crate::tls::EH_FRAME, crate::tls::EH_FRAME_HDR]
    } else {
        [crate::unwind::EH_FRAME, crate::unwind::EH_FRAME_HDR]
    };
    let excluded: Vec<_> = if pie {
        vec![
            crate::tls::PHDR,
            crate::tls::UNWIND_PHDR,
            crate::tls::EH_FRAME,
            crate::tls::EH_FRAME_HDR,
            crate::tls::TLS_IMAGE,
        ]
    } else if merged.is_some() {
        vec![
            crate::unwind::PHDR,
            crate::unwind::EH_FRAME,
            crate::unwind::EH_FRAME_HDR,
        ]
    } else {
        Vec::new()
    };
    let tables = prepared.tables();
    let address = |file: usize,
                   section: u16,
                   offset: u64,
                   use_alias: bool|
     -> Result<Option<(Kind, u64)>, Error> {
        let key = SectionId { file, section };
        if let Some(merged) = merged.filter(|m| m.sections.contains_key(&key)) {
            let (group, within) = crate::unwind::locate(merged, key, offset)?;
            let id = (usize::MAX, crate::unwind::MERGE_BASE + group as u16);
            let placement = image
                .placements
                .get(&id)
                .ok_or(crate::unwind::Error::InvalidReservation)?;
            return Ok(Some((
                placement.kind,
                coordinate(
                    placement
                        .address
                        .checked_add(within)
                        .ok_or(Error::TooLarge)?,
                )?,
            )));
        }
        let placement = image.placements.get(&(file, section)).or_else(|| {
            if use_alias {
                aliases
                    .get(&key)
                    .and_then(|to| image.placements.get(&(to.file, to.section)))
            } else {
                None
            }
        });
        placement
            .map(|p| {
                let address = p.address.checked_add(offset).ok_or(Error::TooLarge)?;
                Ok((p.kind, coordinate(address)?))
            })
            .transpose()
    };
    let mut debug = collect_debug(prepared.metadata())?;
    let allocated: std::collections::BTreeSet<_> =
        prepared.map_rows().iter().map(|r| r.id).collect();
    let sizes: BTreeMap<SectionId, u64> = prepared
        .metadata()
        .iter()
        .map(|m| (m.id, m.bytes.len() as u64))
        .collect();
    for (target, records) in prepared.metadata_relocations() {
        let Some(&(index, base)) = debug.offsets.get(target) else {
            continue;
        };
        let mut resolved = Vec::with_capacity(records.len());
        for r in records {
            if r.kind == 0 {
                continue;
            }
            let mut r = *r;
            let reference = SymbolId {
                file: target.file,
                symbol: r.symbol as usize,
            };
            let to = match crate::resolve::bind(tables, prepared.globals(), reference) {
                Err(crate::resolve::Error::Undefined { name, .. }) => {
                    return Err(crate::gc::Error::Undefined(name).into())
                }
                Err(e) => return Err(crate::layout::Error::from(e).into()),
                Ok(Binding::Zero) => DebugTarget::Address(0),
                Ok(Binding::Selected(Selection::Common { .. })) => {
                    match image
                        .common_addresses
                        .get(tables[target.file][r.symbol as usize].name)
                    {
                        Some(address) => DebugTarget::Address(coordinate(*address)?),
                        None => DebugTarget::Discarded,
                    }
                }
                Ok(Binding::Selected(Selection::Definition(id))) => {
                    let s = &tables[id.file][id.symbol];
                    match s.definition {
                        Definition::Absolute => DebugTarget::Address(s.value),
                        Definition::Section(section) => {
                            let key = SectionId {
                                file: id.file,
                                section,
                            };
                            if let Some(base) = tls.and_then(|t| t.offsets.get(&key)) {
                                DebugTarget::TlsOffset(
                                    base.checked_add(s.value).ok_or(Error::TooLarge)?,
                                )
                            } else if let Some(&(i, b)) = debug.offsets.get(&key) {
                                DebugTarget::Debug {
                                    index: i,
                                    offset: b + s.value,
                                }
                            } else if merged.is_some_and(|m| m.sections.contains_key(&key)) {
                                let coordinate = if s.kind == 3 {
                                    let value = i128::from(s.value) + i128::from(r.addend);
                                    r.addend = 0;
                                    u64::try_from(value).map_err(|_| Error::TooLarge)?
                                } else {
                                    s.value
                                };
                                let (_, value) = address(id.file, section, coordinate, false)?
                                    .ok_or(crate::unwind::Error::InvalidReservation)?;
                                DebugTarget::Address(value)
                            } else if let Some((_, value)) =
                                address(id.file, section, s.value, false)?
                            {
                                DebugTarget::Address(value)
                            } else if allocated.contains(&key)
                                || prepared.split().iter().any(|s| s.id == key)
                            {
                                DebugTarget::Discarded
                            } else {
                                return Err(Error::InvalidDebugReference {
                                    section: *target,
                                    offset: r.offset,
                                });
                            }
                        }
                        _ => return Err(crate::gc::Error::InvalidSymbol.into()),
                    }
                }
            };
            resolved.push((r, to));
        }
        let output = &mut debug.sections[index];
        let end = (base + sizes[target]) as usize;
        let region = &mut output.bytes[base as usize..end];
        for (r, to) in resolved {
            relocate_debug(region, &output.name, &r, to).map_err(|cause| Error::Debug {
                section: *target,
                offset: r.offset,
                kind: r.kind,
                cause,
            })?;
        }
    }
    // Allocated output sections: one per class6 kind that has storage.
    let commons: Vec<(u64, u64, u64)> = prepared
        .globals()
        .iter()
        .filter_map(|(name, s)| match s {
            Selection::Common {
                size, alignment, ..
            } => image
                .common_addresses
                .get(name)
                .map(|a| (*a, *size, *alignment)),
            _ => None,
        })
        .collect();
    let mut alignments: BTreeMap<(usize, u16), u64> = prepared
        .contributions()
        .iter()
        .map(|c| ((c.file, c.section), c.alignment.max(1)))
        .collect();
    if let Some(merged) = merged {
        for (group, value) in merged.groups.iter().enumerate() {
            alignments.insert(
                (usize::MAX, crate::unwind::MERGE_BASE + group as u16),
                value.alignment,
            );
        }
    }
    let kinds = [
        (Kind::Text, &b".text"[..], 1u32, 6u64),
        (Kind::ReadOnly, b".rodata", 1, 2),
        (Kind::Data, b".data", 1, 3),
        (Kind::Bss, b".bss", 8, 3),
    ];
    let mut sections = Vec::new();
    let mut index_of = BTreeMap::new();
    for (kind, name, sh_type, flags) in kinds {
        let mut extents: Vec<(u64, u64, Option<u64>, u64)> = image
            .placements
            .iter()
            .filter(|(id, p)| p.kind == kind && !excluded.contains(id))
            .map(|(id, p)| {
                (
                    p.address,
                    p.address + p.size,
                    p.file_offset,
                    alignments.get(id).copied().unwrap_or(1),
                )
            })
            .collect();
        if kind == Kind::Bss {
            extents.extend(
                commons
                    .iter()
                    .map(|(a, size, align)| (*a, a + size, None, (*align).max(1))),
            );
        }
        let Some(&(start, _, offset, _)) = extents.iter().min_by_key(|e| e.0) else {
            continue;
        };
        let end = extents.iter().map(|e| e.1).max().unwrap();
        let alignment = extents.iter().map(|e| e.3).max().unwrap();
        let contents = match offset {
            Some(offset) => Contents::File {
                offset,
                size: end - start,
            },
            None => Contents::NoBits { size: end - start },
        };
        index_of.insert(kind as u8, sections.len() as u16 + 1);
        sections.push(OutputSection {
            name: name.to_vec(),
            kind: sh_type,
            flags,
            address: coordinate(start)?,
            link: 0,
            info: 0,
            alignment,
            entry_size: 0,
            contents,
        });
    }
    if merged.is_some() {
        for (id, name, alignment) in [
            (unwind_ids[0], &b".eh_frame"[..], 8),
            (unwind_ids[1], &b".eh_frame_hdr"[..], 4),
        ] {
            if let Some(p) = image.placements.get(&id) {
                sections.push(OutputSection {
                    name: name.to_vec(),
                    kind: 1,
                    flags: 2,
                    address: coordinate(p.address)?,
                    link: 0,
                    info: 0,
                    alignment,
                    entry_size: 0,
                    contents: Contents::File {
                        offset: p.file_offset.ok_or(Error::InvalidSection)?,
                        size: p.size,
                    },
                });
            }
        }
    }
    let mut tls_indices = BTreeMap::new();
    if let Some(tls) = tls {
        let p = image
            .placements
            .get(&crate::tls::TLS_IMAGE)
            .ok_or(Error::InvalidSection)?;
        let initialized = tls.image.len() as u64;
        let address = coordinate(p.address)?;
        let data_index = sections.len() as u16 + 1;
        sections.push(OutputSection {
            name: b".tdata".to_vec(),
            kind: 1,
            flags: crate::tls::TLS_FLAGS,
            address,
            link: 0,
            info: 0,
            alignment: tls.alignment,
            entry_size: 0,
            contents: Contents::File {
                offset: p.file_offset.ok_or(Error::InvalidSection)?,
                size: initialized,
            },
        });
        let has_nobits = prepared
            .split()
            .iter()
            .any(|s| s.kind == 8 && tls.offsets.contains_key(&s.id));
        let bss_index = if tls.memory_size > initialized || has_nobits {
            let index = sections.len() as u16 + 1;
            sections.push(OutputSection {
                name: b".tbss".to_vec(),
                kind: 8,
                flags: crate::tls::TLS_FLAGS,
                address: address.checked_add(initialized).ok_or(Error::TooLarge)?,
                link: 0,
                info: 0,
                alignment: 1,
                entry_size: 0,
                contents: Contents::NoBits {
                    size: tls.memory_size - initialized,
                },
            });
            Some(index)
        } else {
            None
        };
        for section in prepared
            .split()
            .iter()
            .filter(|s| tls.offsets.contains_key(&s.id))
        {
            let index = if section.kind == 8 {
                bss_index.ok_or(Error::InvalidSection)?
            } else {
                data_index
            };
            tls_indices.insert(section.id, index);
        }
    }
    let kind_index = |kind: Kind| index_of[&(kind as u8)];
    let symbol_location =
        |file: usize, section: u16, value: u64| -> Result<Option<(u16, u64)>, Error> {
            let key = SectionId { file, section };
            if let Some(base) = tls.and_then(|t| t.offsets.get(&key)) {
                return Ok(Some((
                    tls_indices[&key],
                    base.checked_add(value).ok_or(Error::TooLarge)?,
                )));
            }
            Ok(address(file, section, value, true)?.map(|(kind, value)| (kind_index(kind), value)))
        };
    let mut symbols = Vec::new();
    for (file, table) in tables.iter().enumerate() {
        for s in table.iter().skip(1) {
            if s.binding != 0 || s.kind == 3 || s.kind == 4 || s.name.is_empty() {
                continue;
            }
            match s.definition {
                Definition::Section(section) => {
                    if let Some((section, value)) = symbol_location(file, section, s.value)? {
                        symbols.push(OutputSymbol {
                            name: s.name,
                            info: s.kind,
                            section,
                            value,
                            size: s.size,
                        });
                    }
                }
                Definition::Absolute => symbols.push(OutputSymbol {
                    name: s.name,
                    info: s.kind,
                    section: SHN_ABS,
                    value: s.value,
                    size: s.size,
                }),
                _ => {}
            }
        }
    }
    for (name, selection) in prepared.globals() {
        match selection {
            Selection::Definition(id) => {
                let s = &tables[id.file][id.symbol];
                let info = (s.binding << 4) | s.kind;
                match s.definition {
                    Definition::Section(section) => {
                        if let Some((section, value)) = symbol_location(id.file, section, s.value)?
                        {
                            symbols.push(OutputSymbol {
                                name,
                                info,
                                section,
                                value,
                                size: s.size,
                            });
                        }
                    }
                    Definition::Absolute => symbols.push(OutputSymbol {
                        name,
                        info,
                        section: SHN_ABS,
                        value: s.value,
                        size: s.size,
                    }),
                    _ => {}
                }
            }
            Selection::Common { size, .. } => {
                if let Some(address) = image.common_addresses.get(name) {
                    symbols.push(OutputSymbol {
                        name,
                        info: (1 << 4) | 1,
                        section: kind_index(Kind::Bss),
                        value: coordinate(*address)?,
                        size: *size,
                    });
                }
            }
        }
    }
    let table = encode_symbols(&symbols)?;
    for d in &debug.sections {
        sections.push(OutputSection {
            name: d.name.clone(),
            kind: 1,
            flags: d.flags,
            address: 0,
            link: 0,
            info: 0,
            alignment: d.alignment,
            entry_size: d.entry_size,
            contents: Contents::Data(d.bytes.clone()),
        });
    }
    let strtab_index = sections.len() as u32 + 2;
    sections.push(OutputSection {
        name: b".symtab".to_vec(),
        kind: 2,
        flags: 0,
        address: 0,
        link: strtab_index,
        info: table.first_global,
        alignment: 8,
        entry_size: 24,
        contents: Contents::Data(table.symtab),
    });
    sections.push(OutputSection {
        name: b".strtab".to_vec(),
        kind: 3,
        flags: 0,
        address: 0,
        link: 0,
        info: 0,
        alignment: 1,
        entry_size: 0,
        contents: Contents::Data(table.strtab),
    });
    let mut rows = crate::gc::placed_rows(prepared, image);
    for row in &mut rows {
        if row.address.is_none() {
            row.address = address(row.id.file, row.id.section, 0, true)?.map(|(_, value)| value);
        }
    }
    let mut map = match &lowered {
        Lowered::Plain(_) => crate::gc::render_map(&rows, &image.common_addresses),
        Lowered::MergeUnwind(value) => value.map.clone(),
        Lowered::PieTls(value) => value.map.clone(),
    };
    for d in &debug.sections {
        map.push_str(&format!(
            "DEBUG {} {}\n",
            d.bytes.len(),
            String::from_utf8_lossy(&d.name)
        ));
    }
    let bytes = attach_sections(lowered.emit()?.bytes, &sections)?;
    Ok(crate::gc::Linked { bytes, map })
}

/// Supplied: prepare with class10's policy, lower with class10, then [`finish`]
/// without aliases. The original file prefix matches class10 byte for byte, except the
/// four section-table fields of the ELF header: e_shoff, e_shentsize, e_shnum,
/// and e_shstrndx. Existing PT_LOAD ranges do not grow.
pub fn link(
    inputs: &[crate::archive::Input<'_>],
    entry: &[u8],
    keep: &[&[u8]],
) -> Result<crate::gc::Linked, Error> {
    let prepared = crate::gc::prepare_with(inputs, entry, keep, crate::unwind::policy)?;
    let lowered = crate::unwind::lower_prepared(&prepared)?;
    finish(&prepared, lowered, &Aliases::new())
}

/// Supplied cumulative PIE path: use class11's retained layout, then attach
/// debug information without repeating placement or serializing prematurely.
pub fn link_pie(
    inputs: &[crate::archive::Input<'_>],
    entry: &[u8],
    keep: &[&[u8]],
) -> Result<crate::gc::Linked, Error> {
    let prepared = crate::gc::prepare_with(inputs, entry, keep, crate::tls::policy)?;
    let lowered = crate::tls::lower_prepared(&prepared)?;
    finish(&prepared, lowered, &Aliases::new())
}
