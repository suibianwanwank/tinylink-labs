//! Symbols and explicit-addend relocations retain their input-file identities.
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
/// Use class1 helpers for ranges/names. Preserve non-UTF-8 names and symbol types.
pub fn symbols<'a>(
    input: &'a [u8],
    sections: &[crate::sections::Section],
    index: usize,
) -> Result<Vec<Symbol<'a>>, Error> {
    let _ = (input, sections, index);
    todo!("C2.1: decode symbols, linked names, definitions, and local ordering")
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
    let _ = (input, sections, index);
    todo!("C2.2: resolve table ownership and decode explicit-addend records")
}
