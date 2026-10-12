//! The first executable: one nonempty allocated code section, no relocations.
pub const BASE: u64 = 0x400000;
pub const PAGE: u64 = 4096;
pub const HEADER_BYTES: u64 = 64 + 2 * 56;
pub const MAX_IMAGE: u64 = 64 * 1024 * 1024;
#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    Sections(crate::sections::Error),
    Symbols(crate::symbols::Error),
    Resolution(crate::resolve::Error),
    InvalidAlignment,
    InvalidEntry,
    TooLarge,
    UnsupportedSections,
    UnsupportedRelocations,
    SymbolTableCount,
    MissingEntry,
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
#[derive(Debug, PartialEq, Eq)]
pub struct Layout {
    pub code_offset: u64,
    pub file_size: u64,
    pub entry_address: u64,
}
/// Validate alignment first (power of two, 1..=PAGE), then code_len>0 and
/// entry_offset<code_len. Align HEADER_BYTES up to alignment, append code_len,
/// and reject arithmetic overflow or file_size>MAX_IMAGE as TooLarge.
/// Entry address is BASE + code_offset + entry_offset, not a file offset.
pub fn layout(code_len: u64, alignment: u64, entry_offset: u64) -> Result<Layout, Error> {
    if alignment == 0 || alignment > PAGE || !alignment.is_power_of_two() {
        return Err(Error::InvalidAlignment);
    }
    if code_len == 0 || entry_offset >= code_len {
        return Err(Error::InvalidEntry);
    }
    let code_offset = (HEADER_BYTES + alignment - 1) & !(alignment - 1);
    let file_size = code_offset.checked_add(code_len).ok_or(Error::TooLarge)?;
    if file_size > MAX_IMAGE {
        return Err(Error::TooLarge);
    }
    Ok(Layout {
        code_offset,
        file_size,
        entry_address: BASE + code_offset + entry_offset,
    })
}
/// Emit ELF64 ET_EXEC, x86-64, version 1, OSABI 0, no section table.
/// Two program headers at offset 64: RX PT_LOAD for the ENTIRE file at BASE,
/// followed by RW/non-executable PT_GNU_STACK. All reserved/padding bytes zero.
/// layout() defines code position and entry. See README for every header field.
pub fn executable(code: &[u8], alignment: u64, entry_offset: u64) -> Result<Vec<u8>, Error> {
    let l = layout(code.len() as u64, alignment, entry_offset)?;
    let mut b = vec![0; l.file_size as usize];
    b[..7].copy_from_slice(b"\x7fELF\x02\x01\x01");
    for (i, v) in [(16, 2u16), (18, 62), (52, 64), (54, 56), (56, 2)] {
        b[i..i + 2].copy_from_slice(&v.to_le_bytes());
    }
    for (i, v) in [(20, 1u32), (64, 1), (68, 5), (120, 0x6474e551), (124, 6)] {
        b[i..i + 4].copy_from_slice(&v.to_le_bytes());
    }
    for (i, v) in [
        (24, l.entry_address),
        (32, 64),
        (80, BASE),
        (88, BASE),
        (96, l.file_size),
        (104, l.file_size),
        (112, PAGE),
        (168, 16),
    ] {
        b[i..i + 8].copy_from_slice(&v.to_le_bytes());
    }
    b[l.code_offset as usize..].copy_from_slice(code);
    Ok(b)
}
/// Use classes1-3 to link a single input. Require exactly one nonempty SHF_ALLOC
/// section, with SHT_PROGBITS and flags exactly SHF_ALLOC|SHF_EXECINSTR (6).
/// Other allocated sections must be empty. Reject ANY nonempty SHT_REL/SHT_RELA
/// table (including debug relocations at this stage). Require one SHT_SYMTAB.
/// Select entry_name through build_globals. It must select a section definition
/// in the code section with value<size. Normalize section alignment 0 to 1.
/// Error order: parse sections, allocated-section policy, relocation policy,
/// table count, symbols/resolution, entry identity, executable layout/writing.
pub fn link_single(input: &[u8], entry_name: &[u8]) -> Result<Vec<u8>, Error> {
    let named = crate::sections::sections(input)?;
    let sections: Vec<_> = named.into_iter().map(|(s, _)| s).collect();
    let allocated: Vec<_> = sections
        .iter()
        .enumerate()
        .filter(|(_, s)| s.flags & 2 != 0 && s.size != 0)
        .collect();
    if allocated.len() != 1 {
        return Err(Error::UnsupportedSections);
    }
    let (code_index, code) = allocated[0];
    if code.kind != 1 || code.flags != 6 {
        return Err(Error::UnsupportedSections);
    }
    if sections
        .iter()
        .any(|s| (s.kind == 4 || s.kind == 9) && s.size != 0)
    {
        return Err(Error::UnsupportedRelocations);
    }
    let symtabs: Vec<_> = sections
        .iter()
        .enumerate()
        .filter(|(_, s)| s.kind == 2)
        .collect();
    if symtabs.len() != 1 {
        return Err(Error::SymbolTableCount);
    }
    let files = vec![crate::symbols::symbols(input, &sections, symtabs[0].0)?];
    let globals = crate::resolve::build_globals(&files)?;
    let selected = globals.get(entry_name).ok_or(Error::MissingEntry)?;
    let crate::resolve::Selection::Definition(id) = selected else {
        return Err(Error::InvalidEntry);
    };
    let symbol = &files[id.file][id.symbol];
    if symbol.definition != crate::symbols::Definition::Section(code_index as u16)
        || symbol.value >= code.size
    {
        return Err(Error::InvalidEntry);
    }
    executable(
        crate::sections::payload(input, code)?,
        code.alignment.max(1),
        symbol.value,
    )
}
