//! Symbols and explicit-addend relocations retain their input-file identities.

use crate::elf_header::{read_u16, read_u32, read_u64};

const SHT_SYMTAB: u32 = 2;
const SHT_STRTAB: u32 = 3;
const SHT_RELA: u32 = 4;
const SHT_REL: u32 = 9;
const RECORD_SIZE: usize = 24;
const STB_LOCAL: u8 = 0;
const SHN_UNDEF: u16 = 0;
const SHN_LORESERVE: u16 = 0xff00;
const SHN_ABS: u16 = 0xfff1;
const SHN_COMMON: u16 = 0xfff2;

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    Input(crate::sections::Error),
    InvalidIndex,
    WrongSectionType,
    InvalidEntrySize,
    InvalidStringTable,
    InvalidSymbolSection,
    InvalidSymbolOrder,
    InvalidNullSymbol,
    UnsupportedRel,
}
impl From<crate::sections::Error> for Error {
    fn from(error: crate::sections::Error) -> Self {
        Self::Input(error)
    }
}
impl From<crate::elf_header::Error> for Error {
    fn from(error: crate::elf_header::Error) -> Self {
        Self::Input(crate::sections::Error::Input(error))
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Definition {
    Undefined,
    Section(u16),
    Absolute,
    Common,
}
#[derive(Debug, PartialEq, Eq)]
pub struct Symbol<'a> {
    pub name: &'a [u8],
    pub binding: u8,
    pub kind: u8,
    pub visibility: u8,
    pub definition: Definition,
    pub value: u64,
    pub size: u64,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rela {
    pub offset: u64,
    pub symbol: u32,
    pub kind: u32,
    pub addend: i64,
}
/// Parse a selected SHT_SYMTAB (2). Return entries in original order, including
/// entry zero. Its sh_link must select SHT_STRTAB (3); string payload must be
/// nonempty and start/end with NUL. Require sh_entsize=24 and size%24=0.
/// Require at least one symbol. Entry zero must be 24 zero bytes.
/// sh_info is the first non-local index, in 1..=count. Exactly the symbols
/// before sh_info have STB_LOCAL (0). Ordinary section indices must be in range;
/// support UNDEF (0), ABS (0xfff1), COMMON (0xfff2), reject other reserved indices.
/// Use section helpers for ranges/names. Preserve non-UTF-8 names and symbol types.
pub fn symbols<'a>(
    input: &'a [u8],
    sections: &[crate::sections::Section],
    index: usize,
) -> Result<Vec<Symbol<'a>>, Error> {
    let table = sections.get(index).ok_or(Error::InvalidIndex)?;
    if table.kind != SHT_SYMTAB {
        return Err(Error::WrongSectionType);
    }
    if table.entry_size != RECORD_SIZE as u64 || table.size % RECORD_SIZE as u64 != 0 {
        return Err(Error::InvalidEntrySize);
    }
    let names = sections
        .get(table.link as usize)
        .ok_or(Error::InvalidIndex)?;
    if names.kind != SHT_STRTAB {
        return Err(Error::WrongSectionType);
    }
    let records = crate::sections::payload(input, table)?;
    let strings = crate::sections::payload(input, names)?;
    if strings.first() != Some(&0) || strings.last() != Some(&0) {
        return Err(Error::InvalidStringTable);
    }
    if records.len() < RECORD_SIZE || records[..RECORD_SIZE].iter().any(|b| *b != 0) {
        return Err(Error::InvalidNullSymbol);
    }
    let count = records.len() / RECORD_SIZE;
    if table.info == 0 || table.info as usize > count {
        return Err(Error::InvalidSymbolOrder);
    }
    records
        .as_chunks::<RECORD_SIZE>()
        .0
        .iter()
        .enumerate()
        .map(|(i, b)| {
            let binding = b[4] >> 4;
            if (binding == STB_LOCAL) != (i < (table.info as usize)) {
                return Err(Error::InvalidSymbolOrder);
            }
            let raw = read_u16(b, 6)?;
            let definition = match raw {
                SHN_UNDEF => Definition::Undefined,
                SHN_ABS => Definition::Absolute,
                SHN_COMMON => Definition::Common,
                x if x < SHN_LORESERVE && usize::from(x) < sections.len() => Definition::Section(x),
                _ => return Err(Error::InvalidSymbolSection),
            };
            let name = crate::sections::string_at(strings, read_u32(b, 0)?)?;
            Ok(Symbol {
                name,
                binding,
                kind: b[4] & 15,
                visibility: b[5] & 3,
                definition,
                value: read_u64(b, 8)?,
                size: read_u64(b, 16)?,
            })
        })
        .collect()
}
/// Parse SHT_RELA (4), using its sh_link to select a symbol table and sh_info to
/// select a non-null target section. Require 24-byte entries. Validate the symbol
/// table through symbols(), then every r_info symbol index. Split r_info into
/// high 32-bit symbol index and low 32-bit relocation kind; addend is signed.
/// Preserve offsets and unknown kinds: write widths and target bounds belong to
/// relocation application. Reject SHT_REL (9) explicitly as UnsupportedRel.
pub fn relocations(
    input: &[u8],
    sections: &[crate::sections::Section],
    index: usize,
) -> Result<Vec<Rela>, Error> {
    let table = sections.get(index).ok_or(Error::InvalidIndex)?;
    if table.kind == SHT_REL {
        return Err(Error::UnsupportedRel);
    }
    if table.kind != SHT_RELA {
        return Err(Error::WrongSectionType);
    }
    if table.entry_size != RECORD_SIZE as u64 || table.size % RECORD_SIZE as u64 != 0 {
        return Err(Error::InvalidEntrySize);
    }
    if table.info == 0 || table.info as usize >= sections.len() {
        return Err(Error::InvalidIndex);
    }
    let symbol_table = symbols(input, sections, table.link as usize)?;
    let bytes = crate::sections::payload(input, table)?;
    bytes
        .as_chunks::<RECORD_SIZE>()
        .0
        .iter()
        .map(|b| {
            let info = read_u64(b, 8)?;
            let symbol = (info >> 32) as u32;
            if symbol as usize >= symbol_table.len() {
                return Err(Error::InvalidIndex);
            }
            Ok(Rela {
                offset: read_u64(b, 0)?,
                symbol,
                kind: info as u32,
                addend: read_u64(b, 16)? as i64,
            })
        })
        .collect()
}
