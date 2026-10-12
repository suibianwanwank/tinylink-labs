//! Debug information, symbols, and section headers for ET_EXEC or static PIE.
//!
//! A debugger needs more than loadable segments: it finds DWARF and symbols
//! through section headers. This backend keeps the lowered executable unchanged,
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
    /// An image address (VA for ET_EXEC, RVA for PIE), or an unchanged
    /// constant such as an ABS value or unresolved WEAK zero.
    Address(u64),
    /// Byte `offset` of output debug section `index`. DWARF stores such
    /// references as section-relative offsets, not addresses.
    Debug { index: usize, offset: u64 },
    /// Module-relative offset in the TLS template, not a VA or TP offset.
    /// Used by DTPOFF relocations in DWARF TLS location expressions.
    TlsOffset(u64),
    /// Storage discarded by GC: write the tombstone.
    Discarded,
}

/// Why one debug relocation failed.
#[derive(Debug, PartialEq, Eq)]
pub enum DebugError {
    /// Unsupported kind, or a kind incompatible with the resolved target.
    Unsupported(u32),
    /// Field bounds or value range, as relocate reports them.
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
/// metadata. Bound the combined groups, including padding, by
/// executable::MAX_IMAGE before allocation. README defines alignment and flags.
pub fn collect_debug(metadata: &[Special<'_>]) -> Result<Debug, Error> {
    let _ = metadata;
    todo!("C12.1: gather same-named debug sections into output sections")
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
    let _ = (&bytes, section_name, r, target);
    todo!("C12.2: write image coordinates, section/TLS offsets, or tombstones")
}

/// Encode a null symbol, every local symbol (binding 0) in supplied order,
/// then every other symbol in supplied order. Names are NUL-terminated in
/// `strtab`; identical names share one string, and an empty name uses offset 0.
pub fn encode_symbols(symbols: &[OutputSymbol<'_>]) -> Result<SymbolTable, Error> {
    let _ = symbols;
    todo!("C12.3: serialize locals-first ELF64 symbols and their string table")
}

/// Append `Data` sections, a generated `.shstrtab`, and a section header
/// table to an emitted ELF that has none, then point the ELF header at it.
/// Section `i` of `sections` becomes header `i + 1`; header 0 is null.
/// Check the final file against executable::MAX_IMAGE before allocating padding
/// or appended bytes; return TooLarge when the size or arithmetic exceeds it.
pub fn attach_sections(elf: Vec<u8>, sections: &[OutputSection]) -> Result<Vec<u8>, Error> {
    let _ = (&elf, sections);
    todo!("C12.4: describe the file with section headers for tools and debuggers")
}

/// Sections folded into another by a later backend: each key's bytes are not
/// in the image, and its symbols resolve into the value section at the same
/// offset. Empty for debug itself.
pub type Aliases = BTreeMap<SectionId, SectionId>;

/// An executable whose addresses have already been assigned. Neither variant
/// permits this stage to repeat layout. The plain variant also supports the
/// ICF backend's existing interface.
#[derive(Debug, PartialEq, Eq)]
pub enum Lowered {
    Plain(crate::gc::Lowered),
    MergeUnwind(crate::unwind::Lowered),
    /// PIE/TLS/merge/unwind layout; planning VAs still include crate::executable::BASE.
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
    let _ = (prepared, &lowered, aliases);
    todo!("C12.5: attach relocated debug information to a lowered image")
}

/// Supplied: prepare with unwind's policy, lower with unwind, then [`finish`]
/// without aliases. The original file prefix matches unwind byte for byte, except the
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

/// Supplied cumulative PIE path: use tls's retained layout, then attach
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
