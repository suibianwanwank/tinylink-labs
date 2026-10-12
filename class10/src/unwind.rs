//! Mergeable constants, unwind records, and the `.eh_frame_hdr` search index.
//!
//! This backend extends the gc ET_EXEC pipeline with two kinds of input
//! that a linker must rewrite rather than copy: `SHF_MERGE` sections, whose
//! identical pieces are shared across objects, and `.eh_frame`, whose records
//! survive only when the code they describe survives. README is the contract.
use std::collections::BTreeMap;

use crate::gc::{Role, SectionId, Special};

/// Reserved 56-byte Text slot that extends layout's program-header table.
pub const PHDR: (usize, u16) = (usize::MAX, 0);
/// Synthetic `.eh_frame` contribution: ReadOnly, alignment 8.
pub const EH_FRAME: (usize, u16) = (usize::MAX, 1);
/// Synthetic `.eh_frame_hdr` contribution: ReadOnly, alignment 4.
pub const EH_FRAME_HDR: (usize, u16) = (usize::MAX, 2);
/// Merged group `g` is the ReadOnly contribution `(usize::MAX, MERGE_BASE + g)`.
pub const MERGE_BASE: u16 = 16;
/// `PT_GNU_EH_FRAME` program-header type.
pub const PT_GNU_EH_FRAME: u32 = 0x6474_e550;

/// A byte range of one input section that is merged as a unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Piece {
    /// Offset of the piece within its input section.
    pub offset: u64,
    /// Length in bytes; a string piece includes its terminating NUL.
    pub size: u64,
}

/// One output group of merged pieces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Group {
    pub flags: u64,
    pub entry_size: u64,
    /// Normalized alignment: at least 1. Every piece starts at a multiple of it.
    pub alignment: u64,
    pub bytes: Vec<u8>,
}

/// Result of merging every live split section.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Merged {
    /// Groups in order of first appearance of their key.
    pub groups: Vec<Group>,
    /// For each input section: its group index and, in ascending input-offset
    /// order, each piece with the offset where its bytes live in that group.
    pub sections: BTreeMap<SectionId, (usize, Vec<(Piece, u64)>)>,
}

/// Why a split section or a reference into it was rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeError {
    /// Flags, entry size, or alignment outside the supported subset.
    Unsupported,
    /// A string section whose final byte is not NUL.
    Unterminated,
    /// A split section carries a relocation other than NONE.
    Relocated,
    /// This input offset does not lie inside any piece.
    InvalidReference(u64),
}

/// Why an `.eh_frame` input section was rejected. Offsets are input offsets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnwindError {
    /// Malformed framing or fields in the record starting at this offset.
    Malformed(u64),
    /// A well-formed record outside the supported subset.
    Unsupported(u64),
    /// A relocation, or a missing relocation, at this offset.
    Relocation(u64),
}

/// One record of an `.eh_frame` input section.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Record {
    Cie,
    /// `cie` is the input offset of the CIE this FDE uses. `relocation` indexes
    /// the supplied relocation slice: the PC32 at the FDE's initial location.
    Fde {
        cie: u64,
        relocation: usize,
    },
}

/// A record and its extent, including the four-byte length field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Frame {
    pub offset: u64,
    pub size: u64,
    pub record: Record,
}

/// One retained FDE, before output addresses are assigned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fde<'a> {
    /// The CIE record this FDE uses, including its length field.
    pub cie: &'a [u8],
    /// The FDE record, including its length field.
    pub fde: &'a [u8],
    /// Identity of the Text contribution this FDE describes.
    pub target: (usize, u16),
    /// Initial location relative to the start of `target`.
    pub start: u64,
}

/// Output unwind sections. Both are empty when there are no FDEs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unwind {
    pub eh_frame: Vec<u8>,
    pub hdr: Vec<u8>,
}

/// Planned, relocated storage and rebuilt unwind records before ELF headers
/// are serialized. Retain the merge mapping so later metadata stages can map
/// input offsets into the same output groups instead of laying out code again.
#[derive(Debug, PartialEq, Eq)]
pub struct Lowered {
    /// Final storage and unwind bytes; ELF headers are still reserved zeros.
    pub image: crate::layout::Image,
    /// Entry virtual address in the fixed-address ET_EXEC image.
    pub entry_address: u64,
    /// Input-piece offsets mapped into the output's merged groups.
    pub merged: Merged,
    /// Storage, dropped-input, and synthetic-section rows for this layout.
    pub map: String,
}

impl Lowered {
    /// Supplied serialization of this stage's ET_EXEC image. The reserved
    /// program-header slot describes the already rebuilt unwind search index.
    pub fn emit(self) -> Result<crate::gc::Linked, Error> {
        let hdr = self
            .image
            .placements
            .get(&EH_FRAME_HDR)
            .map(|p| {
                p.file_offset
                    .map(|offset| (offset, p.address, p.size))
                    .ok_or(Error::InvalidReservation)
            })
            .transpose()?;
        let reserved = self
            .image
            .placements
            .get(&PHDR)
            .map(|p| (p.file_offset, p.address, p.size, p.kind));
        if hdr.is_some()
            && reserved
                != Some((
                    Some(crate::layout::HEADER_BYTES),
                    crate::executable::BASE + crate::layout::HEADER_BYTES,
                    56,
                    crate::layout::Kind::Text,
                ))
        {
            return Err(Error::InvalidReservation);
        }
        let mut bytes = crate::layout::emit(self.image, self.entry_address)?;
        if let Some((offset, address, size)) = hdr {
            bytes[56..58].copy_from_slice(&5u16.to_le_bytes());
            let at = crate::layout::HEADER_BYTES as usize;
            let header = &mut bytes[at..at + 56];
            header[0..4].copy_from_slice(&PT_GNU_EH_FRAME.to_le_bytes());
            header[4..8].copy_from_slice(&4u32.to_le_bytes());
            for (i, value) in [offset, address, address, size, size, 4].iter().enumerate() {
                header[8 + i * 8..16 + i * 8].copy_from_slice(&value.to_le_bytes());
            }
        }
        Ok(crate::gc::Linked {
            bytes,
            map: self.map,
        })
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    FrontEnd(crate::gc::Error),
    Output(crate::layout::Error),
    Merge {
        section: SectionId,
        cause: MergeError,
    },
    Unwind {
        section: SectionId,
        cause: UnwindError,
    },
    /// The FDE at this input offset does not describe a Text section definition.
    FdeTarget {
        section: SectionId,
        offset: u64,
    },
    /// `fdes[index]` is malformed or describes bytes outside its target.
    InvalidFde {
        index: usize,
    },
    /// `fdes[index]` overlaps or duplicates an earlier initial location.
    OverlappingFde {
        index: usize,
    },
    /// A reserved synthetic placement is missing or has the wrong shape.
    InvalidReservation,
    /// An ordering entry does not resolve to retained Text storage.
    InvalidOrderSection(SectionId),
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

/// Section policy for this backend. Supplied.
///
/// `.eh_frame` (PROGBITS or `SHT_X86_64_UNWIND`, flags ALLOC) is an annotation;
/// PROGBITS with ALLOC|MERGE or ALLOC|MERGE|STRINGS is split; everything else
/// follows the layout storage table.
pub fn policy(section: &crate::sections::Section, name: &[u8]) -> Option<Role> {
    const SHT_X86_64_UNWIND: u32 = 0x7000_0001;
    match (section.kind, section.flags) {
        (1 | SHT_X86_64_UNWIND, 2) if name == b".eh_frame" => Some(Role::Annotation),
        (1, 0x12 | 0x32) => Some(Role::Split),
        _ => crate::gc::storage_policy(section, name),
    }
}

/// Split one section into merge pieces, in ascending offset order.
///
/// Flags must be exactly ALLOC|MERGE (fixed-size constants, `entry_size` >= 1)
/// or ALLOC|MERGE|STRINGS (`entry_size` == 1). Alignment 0 means 1; it must be
/// a power of two no greater than 4096. Constants are cut every `entry_size`
/// bytes; strings are cut after every NUL, and a nonempty string section must
/// end with NUL. An empty section has no pieces.
pub fn split(section: &Special<'_>) -> Result<Vec<Piece>, MergeError> {
    let _ = section;
    todo!("C10.1: cut a mergeable section into self-contained pieces")
}

/// Merge pieces of every supplied section into groups keyed by
/// `(flags, entry_size, normalized alignment)`.
///
/// Process sections in supplied order and pieces in offset order. A piece whose
/// bytes already occur as a piece of the same group reuses that offset;
/// otherwise append it at the next multiple of the group alignment, padding
/// with zeros. Reject duplicate section identities with
/// `Output(DuplicateIdentity)`, and any group larger than `crate::executable::MAX_IMAGE`
/// with `TooLarge`, before allocating it. Split errors carry the section.
pub fn merge(sections: &[Special<'_>]) -> Result<Merged, Error> {
    let _ = sections;
    todo!("C10.2: deduplicate pieces deterministically and record their new offsets")
}

/// Translate an input offset of a merged section to `(group, group offset)`.
/// The offset may point inside a piece; a piece's end is not inside it.
/// Unknown sections and offsets outside every piece are `InvalidReference`.
pub fn locate(merged: &Merged, section: SectionId, offset: u64) -> Result<(usize, u64), Error> {
    let _ = (merged, section, offset);
    todo!("C10.2: map a reference through the piece that contains it")
}

/// Decode and validate an `.eh_frame` input section against its relocations.
///
/// Supported subset: 32-bit DWARF framing, CIE version 1 or 3, augmentation
/// exactly "zR" with FDE pointer encoding 0x1b (pcrel|sdata4). Each FDE refers
/// to an earlier CIE of the same section. A zero length word terminates the
/// section only as its final four bytes. Every non-NONE relocation must be the
/// only PC32 at some FDE's initial-location field, and every FDE needs one.
/// README fixes the error offsets and the order in which rules are checked.
pub fn frames(
    bytes: &[u8],
    relocations: &[crate::symbols::Rela],
) -> Result<Vec<Frame>, UnwindError> {
    let _ = (bytes, relocations);
    todo!("C10.3: frame CIE and FDE records and bind each FDE to its relocation")
}

/// Output sizes `(eh_frame, eh_frame_hdr)` for `fdes`, independent of addresses:
/// each distinct CIE byte string once, every FDE, and a four-byte terminator;
/// the header is 12 bytes plus 8 per FDE. No FDEs means `(0, 0)`.
pub fn unwind_sizes(fdes: &[Fde<'_>]) -> Result<(u64, u64), Error> {
    let _ = fdes;
    todo!("C10.4: size the unwind output before layout")
}

/// Emit `.eh_frame` and `.eh_frame_hdr` for an image planned with the
/// [`EH_FRAME`] and [`EH_FRAME_HDR`] reservations sized by [`unwind_sizes`].
///
/// Write CIEs on first use, rewrite each FDE's CIE pointer and initial
/// location, append a terminator, and build a header whose search table is
/// sorted by initial location. README defines encodings and range checks.
pub fn emit_unwind(
    fdes: &[Fde<'_>],
    placements: &BTreeMap<(usize, u16), crate::layout::Placement>,
) -> Result<Unwind, Error> {
    let _ = (fdes, placements);
    todo!("C10.4: write relocated unwind records and their binary-search index")
}

/// Merge live split sections, retain FDEs of live code, plan storage, and
/// relocate through merged offsets. Input section identities belong to this
/// immutable prepared input; synthetic placements and merged offsets belong
/// to the returned output image. README specifies the stage contract.
pub fn lower_prepared(prepared: &crate::gc::Prepared<'_>) -> Result<Lowered, Error> {
    lower_with(prepared, &BTreeMap::new(), &[])
}

/// Lower using a validated folding relation and Text ordering. Aliases map
/// directly to retained representatives; this stage redirects code references,
/// drops folded copies' FDEs, and preserves merge mappings. The caller must
/// establish compatible unwind semantics before allowing a fold.
pub fn lower_with(
    prepared: &crate::gc::Prepared<'_>,
    aliases: &BTreeMap<SectionId, SectionId>,
    order: &[SectionId],
) -> Result<Lowered, Error> {
    let _ = (prepared, aliases, order);
    todo!("C10.5: lower split and annotation sections through the gc plan")
}

/// Supplied: prepare with [`policy`], lower without serializing, then emit the
/// ET_EXEC image and `PT_GNU_EH_FRAME`. Inputs without split or unwind sections
/// produce exactly gc's output.
pub fn link(
    inputs: &[crate::archive::Input<'_>],
    entry: &[u8],
    keep: &[&[u8]],
) -> Result<crate::gc::Linked, Error> {
    let prepared = crate::gc::prepare_with(inputs, entry, keep, policy)?;
    lower_prepared(&prepared)?.emit()
}
