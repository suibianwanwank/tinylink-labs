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
    let _ = (code_len, alignment, entry_offset);
    todo!("C4.1: plan bounded file positions and the virtual entry address")
}
/// Emit ELF64 ET_EXEC, x86-64, version 1, OSABI 0, no section table.
/// Two program headers at offset 64: RX PT_LOAD for the ENTIRE file at BASE,
/// followed by RW/non-executable PT_GNU_STACK. All reserved/padding bytes zero.
/// layout() defines code position and entry. See README for every header field.
pub fn executable(code: &[u8], alignment: u64, entry_offset: u64) -> Result<Vec<u8>, Error> {
    let _ = (code, alignment, entry_offset);
    todo!("C4.2: serialize headers and copy code into the planned file image")
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
    let _ = (input, entry_name);
    todo!("C4.3: connect validated objects and selected entry to executable output")
}
