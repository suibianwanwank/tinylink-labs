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
    let _ = code;
    todo!("C5.1: assign contribution offsets without conflating file identities")
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
    let _ = (&bytes, offset, kind, symbol, addend, place);
    todo!("C5.2: calculate, range-check, and write x86-64 relocation fields")
}
/// Parse each input through classes1-2; resolve all symbols through resolve.
/// Allow any number of PROGBITS/flags=6 code sections; other SHF_ALLOC sections
/// must be empty. One SYMTAB per file, no COMMON definitions. Merge code in file
/// then section order. Compute output addresses through crate::executable::layout.
/// Apply RELA tables in file/section/record order, then call crate::executable::executable.
/// README specifies record ownership, unsupported metadata, and entry semantics.
pub fn link(inputs: &[&[u8]], entry_name: &[u8]) -> Result<Vec<u8>, Error> {
    let _ = (inputs, entry_name);
    todo!("C5.3: connect global identities, contribution layout, and relocations")
}
