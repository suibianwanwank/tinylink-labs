//! GNU/System V archives and deterministic left-to-right member extraction.
#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    BadMagic,
    ThinArchive,
    TruncatedHeader,
    InvalidHeader,
    InvalidSize,
    TruncatedMember,
    InvalidPadding,
    InvalidName,
    UnsupportedNameFormat,
    DuplicateNameTable,
    Sections(crate::sections::Error),
    Symbols(crate::symbols::Error),
    Resolution(crate::resolve::Error),
    Output(crate::layout::Error),
    SymbolTableCount,
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
impl From<crate::layout::Error> for Error {
    fn from(e: crate::layout::Error) -> Self {
        Self::Output(e)
    }
}
#[derive(Debug, PartialEq, Eq)]
pub struct Member<'a> {
    pub name: &'a [u8],
    pub bytes: &'a [u8],
    pub header_offset: u64,
}
#[derive(Clone, Copy)]
pub enum Input<'a> {
    Object(&'a [u8]),
    Archive(&'a [u8]),
}

/// Parse !<arch>\n plus 60-byte member headers. Validate terminator, unsigned
/// decimal size, payload bounds, and newline odd-byte padding. Support short
/// names ending '/', and GNU /decimal references into one // table. Name offsets
/// must start entries terminated by /\n. Resolve names after scanning the archive.
/// Omit /, /SYM64/, and // metadata members from results; metadata payloads are
/// opaque and never used to select objects. Reject thin and BSD extended names.
/// Preserve member order, duplicate regular names, and non-UTF-8 name bytes.
pub fn members(input: &[u8]) -> Result<Vec<Member<'_>>, Error> {
    let _ = input;
    todo!("C7.1: parse archive framing and resolve GNU member names safely")
}

/// Select object payloads without copying them. Append explicit objects in input
/// order. At each archive, parse member symbol tables (one SYMTAB per object),
/// then repeatedly scan unselected members in archive order. Extract a member
/// if it defines a currently unresolved GLOBAL name. WEAK undefined symbols do
/// not request extraction. Definitions may be GLOBAL/WEAK/COMMON. Existing
/// selected definitions satisfy requests; COMMON alone does not request upgrade.
/// After each extraction recompute requests before examining the next member.
/// Revisit this archive until no progress, never revisit earlier archives.
/// Parse all member symbol tables even if unused; layout storage policy applies
/// only to selected members. Validate selected symbols through resolve each time.
/// See README for the deliberate scope of this deterministic archive policy.
pub fn select<'a>(inputs: &[Input<'a>]) -> Result<Vec<&'a [u8]>, Error> {
    let _ = inputs;
    todo!("C7.2: compute archive-local extraction fixed points without whole-archive linking")
}

/// Use select(), then the cumulative layout linker. The entry is resolved after
/// selection and does not itself force archive extraction. Do not invoke ar/ld.
pub fn link(inputs: &[Input<'_>], entry: &[u8]) -> Result<Vec<u8>, Error> {
    let _ = (inputs, entry);
    todo!("C7.3: link only the object members selected by the archive algorithm")
}
