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
    let _ = (sections, commons);
    todo!("C6.1: separate file occupancy from memory occupancy and assign segments")
}
/// Consume an Image produced by plan(), with only payload bytes subsequently
/// patched. Metadata and byte length must remain intact. Entry must lie strictly
/// inside a Text contribution, not header or padding. Serialize ELF and four
/// program headers: three PT_LOAD in segment order plus RW PT_GNU_STACK.
/// No output section table. Preserve relocated payload and zero-filled padding.
pub fn emit(image: Image, entry_address: u64) -> Result<Vec<u8>, Error> {
    let _ = (&image, entry_address);
    todo!("C6.2: serialize independently protected load segments and memory sizes")
}
/// Parse and resolve with earlier classes, classify allocated sections by the
/// exact kind/flag policy in README, allocate selected COMMON once, relocate
/// file-backed target contributions using crate::relocate::apply, then emit(). Reject
/// relocations into BSS or omitted metadata. The entry must select Text storage.
pub fn link(inputs: &[&[u8]], entry_name: &[u8]) -> Result<Vec<u8>, Error> {
    let _ = (inputs, entry_name);
    todo!("C6.3: bind code and data addresses across initialized and zero-fill storage")
}
