//! Mergeable constants, unwind records, and the `.eh_frame_hdr` search index.
//!
//! This backend extends the class8 ET_EXEC pipeline with two kinds of input
//! that a linker must rewrite rather than copy: `SHF_MERGE` sections, whose
//! identical pieces are shared across objects, and `.eh_frame`, whose records
//! survive only when the code they describe survives. README is the contract.
use std::collections::BTreeMap;

use crate::gc::{Role, SectionId, Special};

/// Reserved 56-byte Text slot that extends class6's program-header table.
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
/// follows the class6 storage table.
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
    const ALLOC_MERGE: u64 = 0x12;
    const ALLOC_MERGE_STRINGS: u64 = 0x32;
    let strings = match (section.flags, section.entry_size) {
        (ALLOC_MERGE, 1..) => false,
        (ALLOC_MERGE_STRINGS, 1) => true,
        _ => return Err(MergeError::Unsupported),
    };
    let alignment = section.alignment.max(1);
    if !alignment.is_power_of_two() || alignment > 4096 {
        return Err(MergeError::Unsupported);
    }
    let bytes = section.bytes;
    if strings {
        if bytes.last().is_some_and(|b| *b != 0) {
            return Err(MergeError::Unterminated);
        }
        let mut pieces = Vec::new();
        let mut start = 0;
        for (i, b) in bytes.iter().enumerate() {
            if *b == 0 {
                pieces.push(Piece {
                    offset: start as u64,
                    size: (i + 1 - start) as u64,
                });
                start = i + 1;
            }
        }
        Ok(pieces)
    } else {
        let size = section.entry_size;
        if !(bytes.len() as u64).is_multiple_of(size) {
            return Err(MergeError::Unsupported);
        }
        Ok((0..bytes.len() as u64 / size)
            .map(|i| Piece {
                offset: i * size,
                size,
            })
            .collect())
    }
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
    use std::collections::btree_map::Entry;
    let mut groups: Vec<Group> = Vec::new();
    let mut keys = BTreeMap::new();
    let mut seen: Vec<BTreeMap<&[u8], u64>> = Vec::new();
    let mut out = BTreeMap::new();
    for section in sections {
        let pieces = split(section).map_err(|cause| Error::Merge {
            section: section.id,
            cause,
        })?;
        let key = (section.flags, section.entry_size, section.alignment.max(1));
        let g = *keys.entry(key).or_insert_with(|| {
            groups.push(Group {
                flags: key.0,
                entry_size: key.1,
                alignment: key.2,
                bytes: Vec::new(),
            });
            seen.push(BTreeMap::new());
            groups.len() - 1
        });
        let Entry::Vacant(slot) = out.entry(section.id) else {
            return Err(crate::layout::Error::DuplicateIdentity.into());
        };
        let mut placed = Vec::with_capacity(pieces.len());
        for piece in pieces {
            let bytes = &section.bytes[piece.offset as usize..(piece.offset + piece.size) as usize];
            let group = &mut groups[g];
            let offset = match seen[g].get(bytes) {
                Some(offset) => *offset,
                None => {
                    let len = group.bytes.len() as u64;
                    let start = len
                        .div_ceil(group.alignment)
                        .checked_mul(group.alignment)
                        .ok_or(Error::TooLarge)?;
                    let end = start.checked_add(piece.size).ok_or(Error::TooLarge)?;
                    if end > crate::executable::MAX_IMAGE {
                        return Err(Error::TooLarge);
                    }
                    group.bytes.resize(start as usize, 0);
                    group.bytes.extend_from_slice(bytes);
                    seen[g].insert(bytes, start);
                    start
                }
            };
            placed.push((piece, offset));
        }
        slot.insert((g, placed));
    }
    Ok(Merged {
        groups,
        sections: out,
    })
}

/// Translate an input offset of a merged section to `(group, group offset)`.
/// The offset may point inside a piece; a piece's end is not inside it.
/// Unknown sections and offsets outside every piece are `InvalidReference`.
pub fn locate(merged: &Merged, section: SectionId, offset: u64) -> Result<(usize, u64), Error> {
    let invalid = || Error::Merge {
        section,
        cause: MergeError::InvalidReference(offset),
    };
    let Some((group, pieces)) = merged.sections.get(&section) else {
        return Err(invalid());
    };
    let i = pieces.partition_point(|(p, _)| p.offset <= offset);
    let Some((piece, out)) = i.checked_sub(1).map(|i| pieces[i]) else {
        return Err(invalid());
    };
    if offset - piece.offset >= piece.size {
        return Err(invalid());
    }
    Ok((*group, out + (offset - piece.offset)))
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
    fn uleb(b: &[u8], at: &mut usize, end: usize) -> Option<u64> {
        let mut value = 0u64;
        for shift in (0..64).step_by(7) {
            if *at >= end {
                return None;
            }
            let byte = b[*at];
            *at += 1;
            let part = u64::from(byte & 0x7f);
            if shift == 63 && part > 1 {
                return None;
            }
            value |= part << shift;
            if byte & 0x80 == 0 {
                return Some(value);
            }
        }
        None
    }
    fn sleb(b: &[u8], at: &mut usize, end: usize) -> Option<()> {
        for group in 0..10 {
            if *at >= end {
                return None;
            }
            let byte = b[*at];
            *at += 1;
            // The tenth group has only the sign bit left in a 64-bit signed
            // encoding. All remaining payload bits must extend that sign.
            if group == 9 && !matches!(byte, 0x00 | 0x7f) {
                return None;
            }
            if byte & 0x80 == 0 {
                return Some(());
            }
        }
        None
    }
    let u32at = |at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
    let mut frames = Vec::new();
    let mut cies = std::collections::BTreeSet::new();
    let mut pos = 0usize;
    while pos < bytes.len() {
        let here = pos as u64;
        let malformed = UnwindError::Malformed(here);
        if bytes.len() - pos < 4 {
            return Err(malformed);
        }
        let length = u32at(pos);
        if length == 0 {
            if pos + 4 != bytes.len() {
                return Err(malformed);
            }
            break;
        }
        if length == u32::MAX {
            return Err(UnwindError::Unsupported(here));
        }
        let end = pos
            .checked_add(4 + length as usize)
            .filter(|e| *e <= bytes.len())
            .ok_or(malformed)?;
        if length < 4 {
            return Err(malformed);
        }
        let id = u32at(pos + 4);
        let mut at = pos + 8;
        if id == 0 {
            let version = *bytes.get(at).filter(|_| at < end).ok_or(malformed)?;
            at += 1;
            if version != 1 && version != 3 {
                return Err(UnwindError::Unsupported(here));
            }
            let nul = bytes[at..end]
                .iter()
                .position(|b| *b == 0)
                .ok_or(malformed)?;
            if &bytes[at..at + nul] != b"zR" {
                return Err(UnwindError::Unsupported(here));
            }
            at += nul + 1;
            uleb(bytes, &mut at, end).ok_or(malformed)?;
            sleb(bytes, &mut at, end).ok_or(malformed)?;
            if version == 1 {
                if at >= end {
                    return Err(malformed);
                }
                at += 1;
            } else {
                uleb(bytes, &mut at, end).ok_or(malformed)?;
            }
            let augmentation = uleb(bytes, &mut at, end).ok_or(malformed)?;
            if augmentation != 1 {
                return Err(UnwindError::Unsupported(here));
            }
            let encoding = *bytes.get(at).filter(|_| at < end).ok_or(malformed)?;
            if encoding != 0x1b {
                return Err(UnwindError::Unsupported(here));
            }
            cies.insert(here);
            frames.push(Frame {
                offset: here,
                size: (end - pos) as u64,
                record: Record::Cie,
            });
        } else {
            let cie = (here + 4)
                .checked_sub(u64::from(id))
                .filter(|c| cies.contains(c))
                .ok_or(malformed)?;
            at += 8;
            if at >= end {
                return Err(malformed);
            }
            let augmentation = uleb(bytes, &mut at, end).ok_or(malformed)?;
            if augmentation > (end - at) as u64 {
                return Err(malformed);
            }
            frames.push(Frame {
                offset: here,
                size: (end - pos) as u64,
                record: Record::Fde {
                    cie,
                    relocation: usize::MAX,
                },
            });
        }
        pos = end;
    }
    let mut fields: BTreeMap<u64, usize> = frames
        .iter()
        .enumerate()
        .filter(|(_, f)| matches!(f.record, Record::Fde { .. }))
        .map(|(i, f)| (f.offset + 8, i))
        .collect();
    for (index, r) in relocations.iter().enumerate() {
        if r.kind == 0 {
            continue;
        }
        let frame = fields
            .remove(&r.offset)
            .filter(|_| r.kind == 2)
            .ok_or(UnwindError::Relocation(r.offset))?;
        if let Record::Fde { relocation, .. } = &mut frames[frame].record {
            *relocation = index;
        }
    }
    if let Some((offset, _)) = fields.into_iter().next() {
        return Err(UnwindError::Relocation(offset));
    }
    Ok(frames)
}

/// Output sizes `(eh_frame, eh_frame_hdr)` for `fdes`, independent of addresses:
/// each distinct CIE byte string once, every FDE, and a four-byte terminator;
/// the header is 12 bytes plus 8 per FDE. No FDEs means `(0, 0)`.
pub fn unwind_sizes(fdes: &[Fde<'_>]) -> Result<(u64, u64), Error> {
    if fdes.is_empty() {
        return Ok((0, 0));
    }
    let mut cies = std::collections::BTreeSet::new();
    let mut size = 4u64;
    for fde in fdes {
        if cies.insert(fde.cie) {
            size = size
                .checked_add(fde.cie.len() as u64)
                .ok_or(Error::TooLarge)?;
        }
        size = size
            .checked_add(fde.fde.len() as u64)
            .ok_or(Error::TooLarge)?;
    }
    let hdr = (fdes.len() as u64)
        .checked_mul(8)
        .and_then(|n| n.checked_add(12))
        .ok_or(Error::TooLarge)?;
    if size > crate::executable::MAX_IMAGE || hdr > crate::executable::MAX_IMAGE {
        return Err(Error::TooLarge);
    }
    Ok((size, hdr))
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
    use crate::layout::Kind;
    let (eh_size, hdr_size) = unwind_sizes(fdes)?;
    if fdes.is_empty() {
        return Ok(Unwind {
            eh_frame: Vec::new(),
            hdr: Vec::new(),
        });
    }
    let reserved = |id, size| {
        placements
            .get(&id)
            .filter(|p| p.kind == Kind::ReadOnly && p.size == size && p.file_offset.is_some())
            .ok_or(Error::InvalidReservation)
    };
    let eh_address = reserved(EH_FRAME, eh_size)?.address;
    let hdr_address = reserved(EH_FRAME_HDR, hdr_size)?.address;
    let u32at = |b: &[u8], at: usize| u32::from_le_bytes(b[at..at + 4].try_into().unwrap());
    let mut out = Vec::with_capacity(eh_size as usize);
    let mut cies: BTreeMap<&[u8], u64> = BTreeMap::new();
    let mut table = Vec::with_capacity(fdes.len());
    for (index, fde) in fdes.iter().enumerate() {
        let invalid = || Error::InvalidFde { index };
        let framed = |b: &[u8], cie: bool| {
            b.len() >= 8 && u32at(b, 0) as usize == b.len() - 4 && (u32at(b, 4) == 0) == cie
        };
        if !framed(fde.cie, true) || !framed(fde.fde, false) || fde.fde.len() < 17 {
            return Err(invalid());
        }
        let target = placements
            .get(&fde.target)
            .filter(|p| p.kind == Kind::Text)
            .ok_or_else(invalid)?;
        let range = u64::from(u32at(fde.fde, 12));
        if fde
            .start
            .checked_add(range)
            .is_none_or(|end| end > target.size)
        {
            return Err(invalid());
        }
        let cie = match cies.get(fde.cie) {
            Some(at) => *at,
            None => {
                let at = out.len() as u64;
                out.extend_from_slice(fde.cie);
                cies.insert(fde.cie, at);
                at
            }
        };
        let at = out.len() as u64;
        out.extend_from_slice(fde.fde);
        let pointer = u32::try_from(at + 4 - cie).map_err(|_| Error::TooLarge)?;
        out[at as usize + 4..at as usize + 8].copy_from_slice(&pointer.to_le_bytes());
        let initial = target.address + fde.start;
        crate::relocate::apply(
            &mut out[at as usize..],
            8,
            2,
            initial,
            0,
            eh_address + at + 8,
        )
        .map_err(crate::layout::Error::from)?;
        table.push((initial, range, eh_address + at, index));
    }
    out.extend_from_slice(&[0; 4]);
    table.sort_by_key(|entry| entry.0);
    for pair in table.windows(2) {
        let (previous, current) = (pair[0], pair[1]);
        if current.0 == previous.0 || previous.0 + previous.1 > current.0 {
            return Err(Error::OverlappingFde { index: current.3 });
        }
    }
    let relative =
        |to: u64, from: u64| i32::try_from(to as i64 - from as i64).map_err(|_| Error::TooLarge);
    let mut hdr = vec![1, 0x1b, 0x03, 0x3b];
    hdr.extend_from_slice(&relative(eh_address, hdr_address + 4)?.to_le_bytes());
    hdr.extend_from_slice(&(fdes.len() as u32).to_le_bytes());
    for (initial, _, fde, _) in &table {
        hdr.extend_from_slice(&relative(*initial, hdr_address)?.to_le_bytes());
        hdr.extend_from_slice(&relative(*fde, hdr_address)?.to_le_bytes());
    }
    Ok(Unwind { eh_frame: out, hdr })
}

/// Merge live split sections, retain FDEs of live code, plan storage, and
/// relocate through merged offsets. Input section identities belong to this
/// immutable prepared input; synthetic placements and merged offsets belong
/// to the returned output image. README specifies the stage contract.
pub fn lower_prepared(prepared: &crate::gc::Prepared<'_>) -> Result<Lowered, Error> {
    lower_with(prepared, &BTreeMap::new(), &[])
}

pub fn lower_with(
    prepared: &crate::gc::Prepared<'_>,
    aliases: &BTreeMap<SectionId, SectionId>,
    order: &[SectionId],
) -> Result<Lowered, Error> {
    let resolve = |id: SectionId| aliases.get(&id).copied().unwrap_or(id);
    use crate::layout::{Contribution, Kind};
    use crate::resolve::{Binding, Selection, SymbolId};
    use crate::symbols::Definition;
    let tables = prepared.tables();
    let globals = prepared.globals();
    let bind = |file: usize, symbol: u32| {
        crate::resolve::bind(
            tables,
            globals,
            SymbolId {
                file,
                symbol: symbol as usize,
            },
        )
    };
    let split_ids: std::collections::BTreeSet<_> = prepared.split().iter().map(|s| s.id).collect();
    for (source, records) in prepared.relocations() {
        if split_ids.contains(source) && records.iter().any(|r| r.kind != 0) {
            return Err(Error::Merge {
                section: *source,
                cause: MergeError::Relocated,
            });
        }
    }
    let merged = merge(prepared.split())?;
    if merged.groups.len() > usize::from(u16::MAX - MERGE_BASE) {
        return Err(Error::TooLarge);
    }
    let annotation_relocations: BTreeMap<_, _> = prepared
        .annotation_relocations()
        .iter()
        .map(|(id, r)| (*id, r.as_slice()))
        .collect();
    let text: std::collections::BTreeSet<_> = prepared
        .contributions()
        .iter()
        .filter(|c| c.kind == Kind::Text)
        .map(|c| (c.file, c.section))
        .collect();
    let mut fdes = Vec::new();
    let mut described = std::collections::BTreeSet::new();
    for annotation in prepared.annotations() {
        let relocations = annotation_relocations
            .get(&annotation.id)
            .copied()
            .unwrap_or(&[]);
        let frames = frames(annotation.bytes, relocations).map_err(|cause| Error::Unwind {
            section: annotation.id,
            cause,
        })?;
        for frame in &frames {
            let Record::Fde { cie, relocation } = frame.record else {
                continue;
            };
            let wrong = || Error::FdeTarget {
                section: annotation.id,
                offset: frame.offset,
            };
            let r = &relocations[relocation];
            let Ok(Binding::Selected(Selection::Definition(id))) =
                bind(annotation.id.file, r.symbol)
            else {
                return Err(wrong());
            };
            let symbol = &tables[id.file][id.symbol];
            let Definition::Section(section) = symbol.definition else {
                return Err(wrong());
            };
            let target = SectionId {
                file: id.file,
                section,
            };
            if !prepared.live().sections.contains(&target) || aliases.contains_key(&target) {
                continue;
            }
            if !text.contains(&(target.file, target.section)) {
                return Err(wrong());
            }
            let start = symbol
                .value
                .checked_add_signed(r.addend)
                .ok_or_else(wrong)?;
            let record = |offset: u64, size: u64| {
                &annotation.bytes[offset as usize..(offset + size) as usize]
            };
            let cie_frame = frames.iter().find(|f| f.offset == cie).unwrap();
            fdes.push(Fde {
                cie: record(cie_frame.offset, cie_frame.size),
                fde: record(frame.offset, frame.size),
                target: (target.file, target.section),
                start,
            });
            described.insert(annotation.id);
        }
    }
    let (eh_size, hdr_size) = unwind_sizes(&fdes)?;
    let phdr = [0u8; 56];
    let eh_bytes = vec![0u8; eh_size as usize];
    let hdr_bytes = vec![0u8; hdr_size as usize];
    fn reserve(id: (usize, u16), kind: Kind, alignment: u64, bytes: &[u8]) -> Contribution<'_> {
        Contribution {
            file: id.0,
            section: id.1,
            kind,
            alignment,
            bytes,
            memory_size: bytes.len() as u64,
        }
    }
    let mut contributions = Vec::new();
    if !fdes.is_empty() {
        contributions.push(reserve(PHDR, Kind::Text, 8, &phdr));
    }
    let kept: Vec<_> = prepared
        .contributions()
        .iter()
        .filter(|c| {
            !aliases.contains_key(&SectionId {
                file: c.file,
                section: c.section,
            })
        })
        .collect();
    let mut ordered = std::collections::BTreeSet::new();
    fn copy_storage<'a>(c: &Contribution<'a>) -> Contribution<'a> {
        Contribution {
            file: c.file,
            section: c.section,
            kind: c.kind,
            alignment: c.alignment,
            bytes: c.bytes,
            memory_size: c.memory_size,
        }
    }
    for id in order {
        let id = resolve(*id);
        let c = kept
            .iter()
            .find(|c| c.file == id.file && c.section == id.section && c.kind == Kind::Text)
            .ok_or(Error::InvalidOrderSection(id))?;
        if ordered.insert(id) {
            contributions.push(copy_storage(c));
        }
    }
    contributions.extend(
        kept.iter()
            .filter(|c| {
                !ordered.contains(&SectionId {
                    file: c.file,
                    section: c.section,
                })
            })
            .map(|c| copy_storage(c)),
    );
    for (g, group) in merged.groups.iter().enumerate() {
        contributions.push(reserve(
            (usize::MAX, MERGE_BASE + g as u16),
            Kind::ReadOnly,
            group.alignment,
            &group.bytes,
        ));
    }
    if !fdes.is_empty() {
        contributions.push(reserve(EH_FRAME, Kind::ReadOnly, 8, &eh_bytes));
        contributions.push(reserve(EH_FRAME_HDR, Kind::ReadOnly, 4, &hdr_bytes));
    }
    let commons: Vec<_> = globals
        .iter()
        .filter_map(|(name, s)| match s {
            Selection::Common {
                size, alignment, ..
            } if prepared.live().commons.contains(name) => Some(crate::layout::Common {
                name,
                size: *size,
                alignment: *alignment,
            }),
            _ => None,
        })
        .collect();
    let mut image = crate::layout::plan(&contributions, &commons)?;
    let group_addresses: Vec<u64> = (0..merged.groups.len())
        .map(|g| image.placements[&(usize::MAX, MERGE_BASE + g as u16)].address)
        .collect();
    let (entry, entry_offset) = prepared.entry();
    let entry = resolve(entry);
    let entry_address = image.placements[&(entry.file, entry.section)].address + entry_offset;
    let mut patches = Vec::new();
    for (source, records) in prepared.relocations() {
        if split_ids.contains(source) || aliases.contains_key(source) {
            continue;
        }
        let p = &image.placements[&(source.file, source.section)];
        for r in records {
            if r.kind == 0 {
                continue;
            }
            let start = p
                .file_offset
                .ok_or(crate::layout::Error::InvalidRelocationTarget)?;
            let value = match bind(source.file, r.symbol).map_err(crate::layout::Error::from)? {
                Binding::Zero => 0,
                Binding::Selected(Selection::Common { .. }) => {
                    image.common_addresses[tables[source.file][r.symbol as usize].name]
                }
                Binding::Selected(Selection::Definition(id)) => {
                    let s = &tables[id.file][id.symbol];
                    match s.definition {
                        Definition::Absolute => s.value,
                        Definition::Section(sec)
                            if split_ids.contains(&SectionId {
                                file: id.file,
                                section: sec,
                            }) =>
                        {
                            let target = SectionId {
                                file: id.file,
                                section: sec,
                            };
                            let through_section = s.kind == 3;
                            let offset = if through_section {
                                s.value.checked_add_signed(r.addend)
                            } else {
                                Some(s.value)
                            };
                            let offset = offset.ok_or(Error::Merge {
                                section: target,
                                cause: MergeError::InvalidReference(s.value),
                            })?;
                            let (g, at) = locate(&merged, target, offset)?;
                            let address = group_addresses[g] + at;
                            if through_section {
                                address.wrapping_sub(r.addend as u64)
                            } else {
                                address
                            }
                        }
                        Definition::Section(sec) => {
                            let to = resolve(SectionId {
                                file: id.file,
                                section: sec,
                            });
                            image.placements[&(to.file, to.section)].address + s.value
                        }
                        _ => return Err(crate::gc::Error::InvalidSymbol.into()),
                    }
                }
            };
            let place = p
                .address
                .checked_add(r.offset)
                .ok_or(crate::layout::Error::Relocation(
                    crate::relocate::Error::PatchBounds,
                ))?;
            patches.push((
                start as usize,
                p.size as usize,
                r.offset,
                r.kind,
                value,
                r.addend,
                place,
            ));
        }
    }
    for (start, size, offset, kind, value, addend, place) in patches {
        crate::relocate::apply(
            &mut image.bytes[start..start + size],
            offset,
            kind,
            value,
            addend,
            place,
        )
        .map_err(crate::layout::Error::from)?;
    }
    let unwind = emit_unwind(&fdes, &image.placements)?;
    let mut copy = |id: (usize, u16), bytes: &[u8]| {
        if let Some(p) = image.placements.get(&id) {
            let at = p.file_offset.unwrap() as usize;
            image.bytes[at..at + bytes.len()].copy_from_slice(bytes);
        }
    };
    copy(EH_FRAME, &unwind.eh_frame);
    copy(EH_FRAME_HDR, &unwind.hdr);
    let mut rows = prepared.map_rows().to_vec();
    let eh_address = image.placements.get(&EH_FRAME).map(|p| p.address);
    for row in &mut rows {
        let id = resolve(row.id);
        row.address = if let Some(p) = image.placements.get(&(id.file, id.section)) {
            Some(p.address)
        } else if let Some((g, _)) = merged.sections.get(&row.id) {
            Some(group_addresses[*g])
        } else if described.contains(&row.id) {
            eh_address
        } else {
            None
        };
    }
    let mut map = crate::gc::render_map(&rows, &image.common_addresses);
    for (g, group) in merged.groups.iter().enumerate() {
        let family = if group.flags & 0x20 != 0 {
            "str"
        } else {
            "cst"
        };
        map.push_str(&format!(
            "SYNTHETIC 0x{:016x} {} .rodata.{family}{}.{}\n",
            group_addresses[g],
            group.bytes.len(),
            group.entry_size,
            group.alignment
        ));
    }
    for (id, name) in [(EH_FRAME, ".eh_frame"), (EH_FRAME_HDR, ".eh_frame_hdr")] {
        if let Some(p) = image.placements.get(&id) {
            map.push_str(&format!(
                "SYNTHETIC 0x{:016x} {} {name}\n",
                p.address, p.size
            ));
        }
    }
    Ok(Lowered {
        image,
        entry_address,
        merged,
        map,
    })
}

/// Supplied: prepare with [`policy`], lower without serializing, then emit the
/// ET_EXEC image and `PT_GNU_EH_FRAME`. Inputs without split or unwind sections
/// produce exactly class8's output.
pub fn link(
    inputs: &[crate::archive::Input<'_>],
    entry: &[u8],
    keep: &[&[u8]],
) -> Result<crate::gc::Linked, Error> {
    let prepared = crate::gc::prepare_with(inputs, entry, keep, policy)?;
    lower_prepared(&prepared)?.emit()
}
