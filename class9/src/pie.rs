//! Freestanding static PIE with explicit startup-relative relocations.
pub const STARTUP: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/startup.bin"));
pub const BOOT: (usize, u16) = (usize::MAX, 0);
pub const RELA: (usize, u16) = (usize::MAX, 1);
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Value {
    Absolute(u64),
    ImageRva(u64),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Relative {
    pub offset: u64,
    pub addend: i64,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fixup {
    pub offset: u64,
    pub kind: u32,
    pub value: Value,
    pub addend: i64,
    pub place_rva: u64,
    pub writable: bool,
}
#[derive(Debug, PartialEq, Eq)]
pub enum PatchError {
    Relocation(crate::relocate::Error),
    NonPicReference,
    TextRelocation,
}
impl From<crate::relocate::Error> for PatchError {
    fn from(e: crate::relocate::Error) -> Self {
        Self::Relocation(e)
    }
}
#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    FrontEnd(crate::gc::Error),
    Output(crate::layout::Error),
    Fixup {
        section: crate::gc::SectionId,
        offset: u64,
        kind: u32,
        cause: PatchError,
    },
    InvalidRuntime,
    InvalidRelative {
        offset: u64,
    },
    InvalidEntry,
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
/// Apply a link-time fixup, or zero an eight-byte field and return its RELATIVE
/// record. All coordinates are RVAs, never crate::executable::BASE-based addresses. NONE
/// ignores all arguments. R64 to image storage requires writable=true. PC32 and
/// PLT32 to image storage are static; PC-relative ABS/zero and narrow absolute
/// references to image storage are NonPicReference. Absolute R64/32/32S are static.
/// Use relocate for static field checks. Failure leaves bytes unchanged.
pub fn patch(bytes: &mut [u8], fixup: Fixup) -> Result<Option<Relative>, PatchError> {
    let _ = (&bytes, fixup);
    todo!("C9.1: distinguish load-bias-dependent pointers from static displacements")
}

/// Sort by destination RVA and emit ELF64 RELA records (r_info=8, symbol=0).
/// Reject overlapping eight-byte destinations, checked-end overflow, or writes
/// past crate::executable::MAX_IMAGE. Do not constrain signed addends or require alignment.
pub fn encode_relative(records: &[Relative]) -> Result<Vec<u8>, Error> {
    let _ = records;
    todo!("C9.2: serialize a canonical non-overlapping relative relocation table")
}

/// Consume an image produced by crate::layout::plan, with only real payloads patched.
/// BOOT reserves 160 Text bytes, aligned to 16; RELA reserves exactly 24*N
/// ReadOnly bytes, aligned to 8. Validate these placements and the user entry.
/// Every relative write must fit inside one Data placement (not BSS or padding).
/// Fill the supplied startup descriptor and RELA table. Emit via layout, then
/// convert ET_EXEC coordinates to zero-based ET_DYN. Preserve RX/R/RW and NX
/// stack. No PT_INTERP or PT_DYNAMIC; the supplied startup owns relocation.
pub fn emit(
    image: crate::layout::Image,
    records: &[Relative],
    entry_rva: u64,
) -> Result<Vec<u8>, Error> {
    let _ = (&image, records, entry_rva);
    todo!("C9.3: emit a self-relocating ELF image with zero-based program headers")
}

/// Reuse crate::gc::prepare, reserve startup/table storage, plan live contributions,
/// classify bound symbols as Absolute or ImageRva, patch each live relocation,
/// and emit. Keep archive/GC/root policy unchanged. Map user sections and COMMON
/// using RVAs, excluding the two synthetic contributions; prefix the map header
/// with TINYLINK PIE MAP 1. README specifies record counting and limits.
pub fn link(
    inputs: &[crate::archive::Input<'_>],
    entry: &[u8],
    keep: &[&[u8]],
) -> Result<crate::gc::Linked, Error> {
    let _ = (inputs, entry, keep);
    todo!("C9.4: lower the live link plan to a freestanding static PIE")
}
