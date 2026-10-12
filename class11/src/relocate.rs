//! Code-only multi-object static linking with checked x86-64 relocation.
use std::collections::BTreeMap;
#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    Sections(crate::sections::Error),
    Symbols(crate::symbols::Error),
    Resolution(crate::resolve::Error),
    Output(crate::executable::Error),
    InvalidAlignment,
    DuplicateSection,
    EmptyCode,
    TooLarge,
    UnsupportedSections,
    SymbolTableCount,
    UnsupportedRelocation(u32),
    InvalidRelocationTarget,
    InvalidSymbol,
    UnsupportedCommon,
    MissingEntry,
    InvalidEntry,
    PatchBounds,
    RelocationOverflow,
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
impl From<crate::executable::Error> for Error {
    fn from(e: crate::executable::Error) -> Self {
        Self::Output(e)
    }
}

pub struct CodeSection<'a> {
    pub file: usize,
    pub section: u16,
    pub alignment: u64,
    pub bytes: &'a [u8],
}
#[derive(Debug, PartialEq, Eq)]
pub struct Merged {
    pub bytes: Vec<u8>,
    pub offsets: BTreeMap<(usize, u16), u64>,
    pub alignment: u64,
}
/// In supplied order, align each contribution, zero-fill padding, and append it.
/// Preserve (file,section) offsets even for empty contributions. Normalize
/// alignment 0 to 1; accept only powers of two <=4096. Reject duplicate identity.
/// Bound merged size to crate::executable::MAX_IMAGE - crate::executable::PAGE before allocation.
/// Check each alignment, duplicate identity, then size. Empty final bytes fail.
pub fn merge(code: &[CodeSection<'_>]) -> Result<Merged, Error> {
    let mut bytes = Vec::new();
    let mut offsets = BTreeMap::new();
    let mut alignment = 1;
    for c in code {
        let a = c.alignment.max(1);
        if a > 4096 || !a.is_power_of_two() {
            return Err(Error::InvalidAlignment);
        }
        if offsets.contains_key(&(c.file, c.section)) {
            return Err(Error::DuplicateSection);
        }
        let offset = (bytes.len() as u64)
            .checked_add(a - 1)
            .ok_or(Error::TooLarge)?
            & !(a - 1);
        let end = offset
            .checked_add(c.bytes.len() as u64)
            .ok_or(Error::TooLarge)?;
        if end > crate::executable::MAX_IMAGE - crate::executable::PAGE {
            return Err(Error::TooLarge);
        }
        bytes.resize(offset as usize, 0);
        bytes.extend_from_slice(c.bytes);
        offsets.insert((c.file, c.section), offset);
        alignment = alignment.max(a);
    }
    if bytes.is_empty() {
        return Err(Error::EmptyCode);
    }
    Ok(Merged {
        bytes,
        offsets,
        alignment,
    })
}
/// Patch one field, atomically on success. NONE(0) is an unconditional no-op.
/// 64(1): S+A in u64; PC32(2)/PLT32(4): S+A-P in i32; 32(10): S+A in u32;
/// 32S(11): S+A in i32. Use a wide signed intermediate, then reduce modulo 2^64. 32 requires zero
/// extension and PC32/PLT32/32S require sign extension back to that u64 value.
/// Error order: supported kind, field bounds, value range. Write little-endian.
/// offset indexes this slice; place is a VIRTUAL address. On error leave bytes
/// unchanged. PLT32 binds directly here because all definitions are static.
pub fn apply(
    bytes: &mut [u8],
    offset: u64,
    kind: u32,
    symbol: u64,
    addend: i64,
    place: u64,
) -> Result<(), Error> {
    if kind == 0 {
        return Ok(());
    }
    let width = match kind {
        1 => 8,
        2 | 4 | 10 | 11 => 4,
        _ => return Err(Error::UnsupportedRelocation(kind)),
    };
    let end = offset.checked_add(width).ok_or(Error::PatchBounds)?;
    if end > bytes.len() as u64 {
        return Err(Error::PatchBounds);
    }
    let value = (symbol as i128) + (addend as i128)
        - if kind == 2 || kind == 4 {
            place as i128
        } else {
            0
        };
    let value = value as u64;
    if kind == 10 && value > u32::MAX as u64 {
        return Err(Error::RelocationOverflow);
    }
    if matches!(kind, 2 | 4 | 11) && (value as i32 as i64 as u64) != value {
        return Err(Error::RelocationOverflow);
    }
    bytes[offset as usize..end as usize].copy_from_slice(&value.to_le_bytes()[..width as usize]);
    Ok(())
}
/// Parse each input through classes1-2; resolve all symbols through class3.
/// Allow any number of PROGBITS/flags=6 code sections; other SHF_ALLOC sections
/// must be empty. One SYMTAB per file, no COMMON definitions. Merge code in file
/// then section order. Compute output addresses through crate::executable::layout.
/// Apply RELA tables in file/section/record order, then call crate::executable::executable.
/// README specifies record ownership, unsupported metadata, and entry semantics.
pub fn link(inputs: &[&[u8]], entry_name: &[u8]) -> Result<Vec<u8>, Error> {
    let mut all_sections = Vec::new();
    let mut tables = Vec::new();
    let mut table_indices = Vec::new();
    let mut code = Vec::new();
    for (file, input) in inputs.iter().enumerate() {
        let sections: Vec<_> = crate::sections::sections(input)?
            .into_iter()
            .map(|(s, _)| s)
            .collect();
        for (index, s) in sections.iter().enumerate() {
            if s.kind == 1 && s.flags == 6 {
                code.push(CodeSection {
                    file,
                    section: index as u16,
                    alignment: s.alignment,
                    bytes: crate::sections::payload(input, s)?,
                });
            } else if s.flags & 2 != 0 && s.size != 0 {
                return Err(Error::UnsupportedSections);
            }
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
        let table = crate::symbols::symbols(input, &sections, indices[0])?;
        if table
            .iter()
            .any(|s| s.definition == crate::symbols::Definition::Common)
        {
            return Err(Error::UnsupportedCommon);
        }
        tables.push(table);
        table_indices.push(indices[0]);
        all_sections.push(sections);
    }
    let mut merged = merge(&code)?;
    let globals = crate::resolve::build_globals(&tables)?;
    let selected = globals.get(entry_name).ok_or(Error::MissingEntry)?;
    let crate::resolve::Selection::Definition(entry) = selected else {
        return Err(Error::InvalidEntry);
    };
    let entry_symbol = &tables[entry.file][entry.symbol];
    let crate::symbols::Definition::Section(entry_section) = entry_symbol.definition else {
        return Err(Error::InvalidEntry);
    };
    let entry_start = *merged
        .offsets
        .get(&(entry.file, entry_section))
        .ok_or(Error::InvalidEntry)?;
    if entry_symbol.value >= all_sections[entry.file][entry_section as usize].size {
        return Err(Error::InvalidEntry);
    }
    let entry_offset = entry_start + entry_symbol.value;
    let layout =
        crate::executable::layout(merged.bytes.len() as u64, merged.alignment, entry_offset)?;
    let code_base = crate::executable::BASE + layout.code_offset;
    let symbol_value = |binding: crate::resolve::Binding| -> Result<u64, Error> {
        match binding {
            crate::resolve::Binding::Zero => Ok(0),
            crate::resolve::Binding::Selected(crate::resolve::Selection::Common { .. }) => {
                Err(Error::UnsupportedCommon)
            }
            crate::resolve::Binding::Selected(crate::resolve::Selection::Definition(id)) => {
                let s = &tables[id.file][id.symbol];
                match s.definition {
                    crate::symbols::Definition::Absolute => Ok(s.value),
                    crate::symbols::Definition::Section(section) => {
                        let offset = *merged
                            .offsets
                            .get(&(id.file, section))
                            .ok_or(Error::InvalidSymbol)?;
                        if s.value > all_sections[id.file][section as usize].size {
                            return Err(Error::InvalidSymbol);
                        }
                        Ok(code_base + offset + s.value)
                    }
                    _ => Err(Error::InvalidSymbol),
                }
            }
        }
    };
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
            let target_start = *merged
                .offsets
                .get(&(file, s.info as u16))
                .ok_or(Error::InvalidRelocationTarget)?;
            let target_size = sections[s.info as usize].size;
            for r in records {
                if r.kind == 0 {
                    continue;
                }
                let value = symbol_value(crate::resolve::bind(
                    &tables,
                    &globals,
                    crate::resolve::SymbolId {
                        file,
                        symbol: r.symbol as usize,
                    },
                )?)?;
                let place = code_base
                    .checked_add(target_start)
                    .and_then(|p| p.checked_add(r.offset))
                    .ok_or(Error::PatchBounds)?;
                let slice =
                    &mut merged.bytes[target_start as usize..(target_start + target_size) as usize];
                apply(slice, r.offset, r.kind, value, r.addend, place)?;
            }
        }
    }
    Ok(crate::executable::executable(
        &merged.bytes,
        merged.alignment,
        entry_offset,
    )?)
}
