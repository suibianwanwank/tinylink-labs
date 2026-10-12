//! Three load segments, file-backed storage, and zero-initialized memory.
use std::collections::BTreeMap;
pub const HEADER_BYTES: u64 = 64 + 4 * 56;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Text,
    ReadOnly,
    Data,
    Bss,
}
pub struct Contribution<'a> {
    pub file: usize,
    pub section: u16,
    pub kind: Kind,
    pub alignment: u64,
    pub bytes: &'a [u8],
    pub memory_size: u64,
}
pub struct Common<'a> {
    pub name: &'a [u8],
    pub alignment: u64,
    pub size: u64,
}
#[derive(Debug, PartialEq, Eq)]
pub struct Placement {
    pub kind: Kind,
    pub file_offset: Option<u64>,
    pub address: u64,
    pub size: u64,
}
#[derive(Debug, PartialEq, Eq)]
pub struct Segment {
    pub flags: u32,
    pub offset: u64,
    pub file_size: u64,
    pub memory_size: u64,
}
#[derive(Debug, PartialEq, Eq)]
pub struct Image {
    pub bytes: Vec<u8>,
    pub placements: BTreeMap<(usize, u16), Placement>,
    pub common_addresses: BTreeMap<Vec<u8>, u64>,
    pub segments: [Segment; 3],
}
#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    Sections(crate::sections::Error),
    Symbols(crate::symbols::Error),
    Resolution(crate::resolve::Error),
    Relocation(crate::relocate::Error),
    InvalidAlignment,
    InvalidSize,
    DuplicateIdentity,
    EmptyText,
    TooLarge,
    InvalidEntry,
    UnsupportedSection,
    SymbolTableCount,
    MissingEntry,
    InvalidSymbol,
    InvalidRelocationTarget,
}
impl From<crate::sections::Error> for Error {
    fn from(e: crate::sections::Error) -> Self {
        Self::Sections(e)
    }
}
impl From<crate::symbols::Error> for Error {
    fn from(e: crate::symbols::Error) -> Self {
        Self::Symbols(e)
    }
}
impl From<crate::resolve::Error> for Error {
    fn from(e: crate::resolve::Error) -> Self {
        Self::Resolution(e)
    }
}
impl From<crate::relocate::Error> for Error {
    fn from(e: crate::relocate::Error) -> Self {
        Self::Relocation(e)
    }
}

/// Prevalidate contributions in supplied order: alignment (zero->one, power of
/// two <=4096), size (initialized size==bytes.len; BSS bytes empty), unique key.
/// Validate COMMON alignments and unique, nonempty names after contributions.
/// Pack Text, ReadOnly, Data, then Bss, preserving input order within each kind.
/// Text starts after HEADER_BYTES. RO and Data each start on a fresh page.
/// Append COMMON after BSS in supplied order. No BSS/COMMON bytes go in the file.
/// All file and relative memory positions are bounded by crate::executable::MAX_IMAGE.
/// Return RX/R/RW segments; Text includes headers, others start at page boundaries.
/// Require at least one nonempty Text contribution. README defines empty groups.
pub fn plan(sections: &[Contribution<'_>], commons: &[Common<'_>]) -> Result<Image, Error> {
    use std::collections::BTreeSet;
    let align = |n: u64, a: u64| -> Result<u64, Error> {
        let a = a.max(1);
        let v = n.checked_add(a - 1).ok_or(Error::TooLarge)? & !(a - 1);
        if v > crate::executable::MAX_IMAGE {
            Err(Error::TooLarge)
        } else {
            Ok(v)
        }
    };
    let mut keys = BTreeSet::new();
    for s in sections {
        let a = s.alignment.max(1);
        if !a.is_power_of_two() || a > 4096 {
            return Err(Error::InvalidAlignment);
        }
        if (s.kind == Kind::Bss && !s.bytes.is_empty())
            || (s.kind != Kind::Bss && s.memory_size != s.bytes.len() as u64)
        {
            return Err(Error::InvalidSize);
        }
        if !keys.insert((s.file, s.section)) {
            return Err(Error::DuplicateIdentity);
        }
    }
    let mut names = BTreeSet::new();
    for c in commons {
        let a = c.alignment.max(1);
        if !a.is_power_of_two() || a > 4096 {
            return Err(Error::InvalidAlignment);
        }
        if c.name.is_empty() {
            return Err(Error::InvalidSize);
        }
        if !names.insert(c.name) {
            return Err(Error::DuplicateIdentity);
        }
    }
    if !sections
        .iter()
        .any(|s| s.kind == Kind::Text && s.memory_size != 0)
    {
        return Err(Error::EmptyText);
    }
    let mut bytes = vec![0; HEADER_BYTES as usize];
    let mut placements = BTreeMap::new();
    let mut segments = [5, 4, 6].map(|flags| Segment {
        flags,
        offset: 0,
        file_size: 0,
        memory_size: 0,
    });
    for (kind, segment) in [Kind::Text, Kind::ReadOnly, Kind::Data]
        .into_iter()
        .zip(&mut segments)
    {
        let start = if kind == Kind::Text {
            0
        } else {
            let v = align(bytes.len() as u64, 4096)?;
            bytes.resize(v as usize, 0);
            v
        };
        for s in sections.iter().filter(|s| s.kind == kind) {
            let offset = align(bytes.len() as u64, s.alignment)?;
            let end = offset.checked_add(s.memory_size).ok_or(Error::TooLarge)?;
            if end > crate::executable::MAX_IMAGE {
                return Err(Error::TooLarge);
            }
            bytes.resize(offset as usize, 0);
            bytes.extend_from_slice(s.bytes);
            placements.insert(
                (s.file, s.section),
                Placement {
                    kind,
                    file_offset: Some(offset),
                    address: crate::executable::BASE + offset,
                    size: s.memory_size,
                },
            );
        }
        let size = bytes.len() as u64 - start;
        *segment = Segment {
            flags: segment.flags,
            offset: start,
            file_size: size,
            memory_size: size,
        };
    }
    let mut memory_end = bytes.len() as u64;
    let mut common_addresses = BTreeMap::new();
    for s in sections.iter().filter(|s| s.kind == Kind::Bss) {
        let offset = align(memory_end, s.alignment)?;
        memory_end = offset.checked_add(s.memory_size).ok_or(Error::TooLarge)?;
        if memory_end > crate::executable::MAX_IMAGE {
            return Err(Error::TooLarge);
        }
        placements.insert(
            (s.file, s.section),
            Placement {
                kind: Kind::Bss,
                file_offset: None,
                address: crate::executable::BASE + offset,
                size: s.memory_size,
            },
        );
    }
    for c in commons {
        let offset = align(memory_end, c.alignment)?;
        memory_end = offset.checked_add(c.size).ok_or(Error::TooLarge)?;
        if memory_end > crate::executable::MAX_IMAGE {
            return Err(Error::TooLarge);
        }
        common_addresses.insert(c.name.to_vec(), crate::executable::BASE + offset);
    }
    segments[2].memory_size = memory_end - segments[2].offset;
    Ok(Image {
        bytes,
        placements,
        common_addresses,
        segments,
    })
}
/// Consume an Image produced by plan(), with only payload bytes subsequently
/// patched. Metadata and byte length must remain intact. Entry must lie strictly
/// inside a Text contribution, not header or padding. Serialize ELF and four
/// program headers: three PT_LOAD in segment order plus RW PT_GNU_STACK.
/// No output section table. Preserve relocated payload and zero-filled padding.
pub fn emit(image: Image, entry_address: u64) -> Result<Vec<u8>, Error> {
    if !image.placements.values().any(|p| {
        p.kind == Kind::Text && entry_address >= p.address && entry_address - p.address < p.size
    }) {
        return Err(Error::InvalidEntry);
    }
    let mut b = image.bytes;
    b[..HEADER_BYTES as usize].fill(0);
    b[..7].copy_from_slice(b"\x7fELF\x02\x01\x01");
    for (i, v) in [(16, 2u16), (18, 62), (52, 64), (54, 56), (56, 4)] {
        b[i..i + 2].copy_from_slice(&v.to_le_bytes());
    }
    b[20..24].copy_from_slice(&1u32.to_le_bytes());
    b[24..32].copy_from_slice(&entry_address.to_le_bytes());
    b[32..40].copy_from_slice(&64u64.to_le_bytes());
    for (i, s) in image.segments.iter().enumerate() {
        let h = 64 + i * 56;
        b[h..h + 4].copy_from_slice(&1u32.to_le_bytes());
        b[h + 4..h + 8].copy_from_slice(&s.flags.to_le_bytes());
        for (offset, v) in [
            (8, s.offset),
            (16, crate::executable::BASE + s.offset),
            (24, crate::executable::BASE + s.offset),
            (32, s.file_size),
            (40, s.memory_size),
            (48, 4096),
        ] {
            b[h + offset..h + offset + 8].copy_from_slice(&v.to_le_bytes());
        }
    }
    b[232..236].copy_from_slice(&0x6474e551u32.to_le_bytes());
    b[236..240].copy_from_slice(&6u32.to_le_bytes());
    b[280..288].copy_from_slice(&16u64.to_le_bytes());
    Ok(b)
}
/// Parse and resolve with earlier classes, classify allocated sections by the
/// exact kind/flag policy in README, allocate selected COMMON once, relocate
/// file-backed target contributions using crate::relocate::apply, then emit(). Reject
/// relocations into BSS or omitted metadata. The entry must select Text storage.
pub fn link(inputs: &[&[u8]], entry_name: &[u8]) -> Result<Vec<u8>, Error> {
    let mut all_sections = Vec::new();
    let mut tables = Vec::new();
    let mut table_indices = Vec::new();
    let mut contributions = Vec::new();
    for (file, input) in inputs.iter().enumerate() {
        let sections: Vec<_> = crate::sections::sections(input)?
            .into_iter()
            .map(|(s, _)| s)
            .collect();
        for (index, s) in sections.iter().enumerate() {
            if s.flags & 2 == 0 {
                continue;
            }
            let kind = match (s.kind, s.flags) {
                (1, 6) => Kind::Text,
                (1, 2) => Kind::ReadOnly,
                (1, 3) => Kind::Data,
                (8, 3) => Kind::Bss,
                _ => return Err(Error::UnsupportedSection),
            };
            contributions.push(Contribution {
                file,
                section: index as u16,
                kind,
                alignment: s.alignment,
                bytes: crate::sections::payload(input, s)?,
                memory_size: s.size,
            });
        }
        let indices: Vec<_> = sections
            .iter()
            .enumerate()
            .filter(|(_, s)| s.kind == 2)
            .map(|(i, _)| i)
            .collect();
        if indices.len() != 1 {
            return Err(Error::SymbolTableCount);
        }
        tables.push(crate::symbols::symbols(input, &sections, indices[0])?);
        table_indices.push(indices[0]);
        all_sections.push(sections);
    }
    let globals = crate::resolve::build_globals(&tables)?;
    let commons: Vec<_> = globals
        .iter()
        .filter_map(|(name, sel)| match sel {
            crate::resolve::Selection::Common {
                size, alignment, ..
            } => Some(Common {
                name,
                alignment: *alignment,
                size: *size,
            }),
            _ => None,
        })
        .collect();
    let mut image = plan(&contributions, &commons)?;
    let selected = globals.get(entry_name).ok_or(Error::MissingEntry)?;
    let crate::resolve::Selection::Definition(id) = selected else {
        return Err(Error::InvalidEntry);
    };
    let symbol = &tables[id.file][id.symbol];
    let crate::symbols::Definition::Section(sec) = symbol.definition else {
        return Err(Error::InvalidEntry);
    };
    let p = image
        .placements
        .get(&(id.file, sec))
        .ok_or(Error::InvalidEntry)?;
    if p.kind != Kind::Text || symbol.value >= p.size {
        return Err(Error::InvalidEntry);
    }
    let entry = p.address + symbol.value;
    for (file, sections) in all_sections.iter().enumerate() {
        for (index, s) in sections.iter().enumerate() {
            if s.kind == 9 && s.size != 0 {
                return Err(Error::InvalidRelocationTarget);
            }
            if s.kind != 4 {
                continue;
            }
            let records = crate::symbols::relocations(inputs[file], sections, index)?;
            if s.link as usize != table_indices[file] {
                return Err(Error::InvalidRelocationTarget);
            }
            let target = image
                .placements
                .get(&(file, s.info as u16))
                .ok_or(Error::InvalidRelocationTarget)?;
            let start = target.file_offset.ok_or(Error::InvalidRelocationTarget)?;
            let (address, size) = (target.address, target.size);
            for r in records {
                if r.kind == 0 {
                    continue;
                }
                let reference = crate::resolve::SymbolId {
                    file,
                    symbol: r.symbol as usize,
                };
                let value = match crate::resolve::bind(&tables, &globals, reference)? {
                    crate::resolve::Binding::Zero => 0,
                    crate::resolve::Binding::Selected(crate::resolve::Selection::Common {
                        ..
                    }) => *image
                        .common_addresses
                        .get(tables[file][r.symbol as usize].name)
                        .ok_or(Error::InvalidSymbol)?,
                    crate::resolve::Binding::Selected(crate::resolve::Selection::Definition(
                        id,
                    )) => {
                        let sym = &tables[id.file][id.symbol];
                        match sym.definition {
                            crate::symbols::Definition::Absolute => sym.value,
                            crate::symbols::Definition::Section(sec) => {
                                let p = image
                                    .placements
                                    .get(&(id.file, sec))
                                    .ok_or(Error::InvalidSymbol)?;
                                if sym.value > p.size {
                                    return Err(Error::InvalidSymbol);
                                }
                                p.address + sym.value
                            }
                            _ => return Err(Error::InvalidSymbol),
                        }
                    }
                };
                let place = address
                    .checked_add(r.offset)
                    .ok_or(Error::Relocation(crate::relocate::Error::PatchBounds))?;
                crate::relocate::apply(
                    &mut image.bytes[start as usize..(start + size) as usize],
                    r.offset,
                    r.kind,
                    value,
                    r.addend,
                    place,
                )?;
            }
        }
    }
    emit(image, entry)
}
