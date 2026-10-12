//! A Global Offset Table and static thread-local storage for the class9 PIE.
//!
//! Position-independent code reaches data it cannot prove local through GOT
//! slots, and thread-local variables through offsets from the thread pointer
//! (`%fs`). This backend builds the GOT, relaxes indirect loads when the
//! target is in the image, lays out the TLS template, and adds a supplied
//! startup stage that installs the main thread's TLS block. README is the
//! contract.
use std::collections::BTreeMap;

use crate::gc::{Role, SectionId, Special};
use crate::pie::Relative;

/// Supplied TLS startup: 128 bytes of code, then the descriptor at byte 128.
pub const TLS_STARTUP: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/tls_startup.bin"));
/// Size of the [`TLS_BOOT`] reservation: code plus six u64 descriptor fields.
pub const TLS_STARTUP_SIZE: u64 = 176;
/// Synthetic GOT: Data, alignment 8, eight bytes per slot.
pub const GOT: (usize, u16) = (usize::MAX, 2);
/// TLS initialization image: Data, aligned to the TLS alignment.
pub const TLS_IMAGE: (usize, u16) = (usize::MAX, 3);
/// Main-thread TLS block plus its eight-byte TCB: Bss, aligned likewise.
pub const TLS_AREA: (usize, u16) = (usize::MAX, 4);
/// The supplied TLS startup: Text, alignment 16, [`TLS_STARTUP_SIZE`] bytes.
pub const TLS_BOOT: (usize, u16) = (usize::MAX, 5);
/// Reserved 56-byte Text slot that extends the program-header table.
pub const PHDR: (usize, u16) = (usize::MAX, 6);
pub const UNWIND_PHDR: (usize, u16) = (usize::MAX, 7);
pub const EH_FRAME: (usize, u16) = (usize::MAX, 8);
pub const EH_FRAME_HDR: (usize, u16) = (usize::MAX, 9);
/// `PT_TLS` program-header type.
pub const PT_TLS: u32 = 7;
/// `SHF_WRITE | SHF_ALLOC | SHF_TLS`: the only supported TLS section flags.
pub const TLS_FLAGS: u64 = 0x403;

/// The TLS template: where each TLS input section lives within it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tls {
    /// Offset of each TLS input section within the template.
    pub offsets: BTreeMap<SectionId, u64>,
    /// Initialized bytes (`.tdata` contents); `.tbss` follows, zero-filled.
    pub image: Vec<u8>,
    /// Template size in memory, including `.tbss`.
    pub memory_size: u64,
    /// Template alignment: at least 8, so the TCB word is aligned.
    pub alignment: u64,
    /// Distance from the start of a TLS block to the thread pointer:
    /// `memory_size` rounded up to `alignment`.
    pub thread_pointer: u64,
}

/// What a bound relocation symbol is, for lowering purposes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// Storage in the image: its address depends on the load bias.
    Image,
    /// A link-time constant: an ABS symbol or an unresolved WEAK zero.
    Absolute,
    /// A thread-local variable: a section in the TLS template.
    Tls,
}

/// How a relocation is lowered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lowering {
    /// An ordinary field handled by `crate::pie::patch`.
    Fixup,
    /// A PC-relative reference to the GOT slot for the target.
    Got,
    /// Rewrite the `mov` opcode two bytes before the field into `lea`, then
    /// treat the field as PC32 to the target itself.
    Relax,
    /// A static `%fs`-relative offset: the target's TP offset plus the addend.
    ThreadOffset,
}

/// Why a relocation cannot be lowered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferenceError {
    /// A TLS relocation to ordinary storage, or the reverse.
    TlsMismatch,
    /// A dynamic TLS model (GD, LD, TLSDESC) or a TLS kind outside this stage.
    UnsupportedTls,
}

/// The value one GOT slot must hold at run time.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Slot {
    /// The load-time address of this byte offset in a storage section.
    Section(SectionId, u64),
    /// The load-time address of a COMMON allocation.
    Common(Vec<u8>),
    /// A link-time constant: ABS value, zero, or a TP offset as two's complement.
    Constant(u64),
}

/// GOT slots in first-use order. Each distinct [`Slot`] appears once.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Got {
    slots: Vec<Slot>,
    index: BTreeMap<Slot, usize>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    FrontEnd(crate::gc::Error),
    Output(crate::layout::Error),
    /// A class9 error, including fixup failures with their relocation context.
    Pie(crate::pie::Error),
    Merge(crate::unwind::Error),
    /// A TLS section with unsupported type, flags, size, or alignment.
    InvalidTls(SectionId),
    Reference {
        section: SectionId,
        offset: u64,
        kind: u32,
        cause: ReferenceError,
    },
    /// A reserved synthetic placement is missing or has the wrong shape.
    InvalidReservation,
    InvalidAlias {
        from: SectionId,
        to: SectionId,
    },
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
impl From<crate::pie::Error> for Error {
    fn from(e: crate::pie::Error) -> Self {
        Self::Pie(e)
    }
}

impl From<crate::unwind::Error> for Error {
    fn from(e: crate::unwind::Error) -> Self {
        Self::Merge(e)
    }
}

/// Section policy for this backend. Supplied.
///
/// TLS sections (PROGBITS or NOBITS with flags exactly [`TLS_FLAGS`]) are
/// split sections placed in the TLS template; everything else follows the
/// class6 storage table.
pub fn policy(section: &crate::sections::Section, name: &[u8]) -> Option<Role> {
    match (section.kind, section.flags) {
        (1 | 8, TLS_FLAGS) => Some(Role::Split),
        _ if section.flags & 0x10 != 0 => crate::unwind::policy(section, name),
        _ if name == b".eh_frame" => crate::unwind::policy(section, name),
        _ => crate::gc::storage_policy(section, name),
    }
}

/// Lay out the TLS template: every PROGBITS section in supplied order, then
/// every NOBITS section in supplied order, each at a multiple of its own
/// alignment. README defines validation, alignment, and the size bound.
pub fn tls_layout(sections: &[Special<'_>]) -> Result<Tls, Error> {
    let mut offsets = BTreeMap::new();
    let mut image = Vec::new();
    let mut alignment = 8u64;
    let mut checked = Vec::with_capacity(sections.len());
    for s in sections {
        let align = s.alignment.max(1);
        let shaped = s.flags == TLS_FLAGS
            && align.is_power_of_two()
            && align <= 4096
            && match s.kind {
                1 => s.bytes.len() as u64 == s.size,
                8 => s.bytes.is_empty(),
                _ => false,
            };
        if !shaped {
            return Err(Error::InvalidTls(s.id));
        }
        if offsets.insert(s.id, 0).is_some() {
            return Err(crate::layout::Error::DuplicateIdentity.into());
        }
        alignment = alignment.max(align);
        checked.push((s, align));
    }
    let mut cursor = 0u64;
    for nobits in [false, true] {
        for (s, align) in checked.iter().filter(|(s, _)| (s.kind == 8) == nobits) {
            let start = cursor
                .div_ceil(*align)
                .checked_mul(*align)
                .ok_or(Error::TooLarge)?;
            cursor = start
                .checked_add(s.size)
                .filter(|end| *end <= crate::executable::MAX_IMAGE)
                .ok_or(Error::TooLarge)?;
            offsets.insert(s.id, start);
            if !nobits {
                image.resize(start as usize, 0);
                image.extend_from_slice(s.bytes);
            }
        }
    }
    let thread_pointer = cursor.div_ceil(alignment) * alignment;
    if thread_pointer + 8 > crate::executable::MAX_IMAGE {
        return Err(Error::TooLarge);
    }
    Ok(Tls {
        offsets,
        image,
        memory_size: cursor,
        alignment,
        thread_pointer,
    })
}

impl Tls {
    /// TP offset of byte `offset` of `section`: its template offset minus
    /// `thread_pointer`. Variant II TLS puts the block below the thread
    /// pointer, so this is negative for every byte of the template. `None`
    /// for a section outside the template.
    pub fn tpoff(&self, section: SectionId, offset: u64) -> Option<i64> {
        let _ = (section, offset);
        let start = *self.offsets.get(&section)?;
        Some(start as i64 + offset as i64 - self.thread_pointer as i64)
    }
}

/// Choose how to lower one relocation at `offset` in `code`, whose symbol is
/// `target`. NONE is always `Fixup`. README lists every kind and the exact
/// relaxation condition; this function never modifies `code`.
pub fn lower(
    code: &[u8],
    offset: u64,
    kind: u32,
    target: Target,
) -> Result<Lowering, ReferenceError> {
    const MOV: u8 = 0x8b;
    match (kind, target) {
        (0, _) => Ok(Lowering::Fixup),
        (16..=21 | 34..=36, _) => Err(ReferenceError::UnsupportedTls),
        (22, Target::Tls) => Ok(Lowering::Got),
        (23, Target::Tls) => Ok(Lowering::ThreadOffset),
        (22 | 23, _) | (_, Target::Tls) => Err(ReferenceError::TlsMismatch),
        (41 | 42, Target::Image)
            if offset >= 2 && offset <= code.len() as u64 && code[offset as usize - 2] == MOV =>
        {
            Ok(Lowering::Relax)
        }
        (9 | 41 | 42, _) => Ok(Lowering::Got),
        _ => Ok(Lowering::Fixup),
    }
}

impl Got {
    /// An empty table. Supplied.
    pub fn new() -> Self {
        Self::default()
    }
    /// Slots in first-use order. Supplied.
    pub fn slots(&self) -> &[Slot] {
        &self.slots
    }
    /// Table size in bytes. Supplied.
    pub fn size(&self) -> u64 {
        8 * self.slots.len() as u64
    }
    /// Byte offset of the slot holding `slot`, appending it on first use.
    /// A table larger than `crate::executable::MAX_IMAGE` is `TooLarge`.
    pub fn offset(&mut self, slot: Slot) -> Result<u64, Error> {
        let _ = slot;
        if let Some(i) = self.index.get(&slot) {
            return Ok(8 * *i as u64);
        }
        let offset = self.size();
        if offset + 8 > crate::executable::MAX_IMAGE {
            return Err(Error::TooLarge);
        }
        self.index.insert(slot.clone(), self.slots.len());
        self.slots.push(slot);
        Ok(offset)
    }
    /// Encode the table for an image planned with a [`GOT`] reservation of
    /// exactly [`Got::size`] bytes. Constants are stored; image addresses are
    /// zero in the file with a RELATIVE record each, in slot order.
    pub fn encode(&self, image: &crate::layout::Image) -> Result<(Vec<u8>, Vec<Relative>), Error> {
        let _ = image;
        if self.slots.is_empty() {
            return Ok((Vec::new(), Vec::new()));
        }
        let got = image
            .placements
            .get(&GOT)
            .filter(|p| {
                p.kind == crate::layout::Kind::Data
                    && p.size == self.size()
                    && p.file_offset.is_some()
            })
            .ok_or(Error::InvalidReservation)?;
        let base = got.address - crate::executable::BASE;
        let mut bytes = vec![0u8; self.size() as usize];
        let mut records = Vec::new();
        for (i, slot) in self.slots.iter().enumerate() {
            let rva = match slot {
                Slot::Constant(value) => {
                    bytes[8 * i..8 * i + 8].copy_from_slice(&value.to_le_bytes());
                    continue;
                }
                Slot::Section(id, offset) => {
                    let p = image
                        .placements
                        .get(&(id.file, id.section))
                        .filter(|p| *offset <= p.size);
                    p.ok_or(crate::layout::Error::InvalidSymbol)?.address - crate::executable::BASE
                        + offset
                }
                Slot::Common(name) => {
                    image
                        .common_addresses
                        .get(name)
                        .ok_or(crate::layout::Error::InvalidSymbol)?
                        - crate::executable::BASE
                }
            };
            records.push(Relative {
                offset: base + 8 * i as u64,
                addend: rva as i64,
            });
        }
        Ok((bytes, records))
    }
}

/// Emit through `crate::pie::emit`. With `tls`, first validate the TLS
/// reservations, fill [`TLS_BOOT`] with [`TLS_STARTUP`] and its descriptor,
/// enter through it, then add `PT_TLS` as the fifth program header. Without
/// `tls`, this is exactly `crate::pie::emit(image, records, entry_rva)`.
pub fn emit(
    image: crate::layout::Image,
    records: &[Relative],
    tls: Option<&Tls>,
    entry_rva: u64,
) -> Result<Vec<u8>, Error> {
    use crate::layout::Kind;
    let Some(tls) = tls else {
        return Ok(crate::pie::emit(image, records, entry_rva)?);
    };
    let mut image = image;
    let rva = |p: &crate::layout::Placement| p.address - crate::executable::BASE;
    let shaped = |id, kind, size, align: u64| {
        image
            .placements
            .get(&id)
            .filter(|p| p.kind == kind && p.size == size && p.address % align == 0)
    };
    let boot =
        shaped(TLS_BOOT, Kind::Text, TLS_STARTUP_SIZE, 16).filter(|p| p.file_offset.is_some());
    let template = shaped(TLS_IMAGE, Kind::Data, tls.image.len() as u64, tls.alignment)
        .filter(|p| p.file_offset.is_some());
    let area = shaped(TLS_AREA, Kind::Bss, tls.thread_pointer + 8, tls.alignment);
    let phdr = image.placements.get(&PHDR).filter(|p| {
        p.kind == Kind::Text
            && p.size == 56
            && p.file_offset == Some(crate::layout::HEADER_BYTES)
            && p.address == crate::executable::BASE + crate::layout::HEADER_BYTES
    });
    let (Some(boot), Some(template), Some(area), Some(_)) = (boot, template, area, phdr) else {
        return Err(Error::InvalidReservation);
    };
    if TLS_STARTUP.len() as u64 != TLS_STARTUP_SIZE {
        return Err(Error::InvalidReservation);
    }
    let reserved = [crate::pie::BOOT, TLS_BOOT, PHDR];
    let entry_ok = image.placements.iter().any(|(id, p)| {
        !reserved.contains(id)
            && p.kind == Kind::Text
            && entry_rva >= rva(p)
            && entry_rva - rva(p) < p.size
    });
    if !entry_ok {
        return Err(crate::pie::Error::InvalidEntry.into());
    }
    let (boot_rva, boot_file) = (rva(boot), boot.file_offset.unwrap() as usize);
    let (template_rva, template_file) = (rva(template), template.file_offset.unwrap());
    let area_rva = rva(area);
    let startup = &mut image.bytes[boot_file..boot_file + TLS_STARTUP_SIZE as usize];
    startup[..TLS_STARTUP.len()].copy_from_slice(TLS_STARTUP);
    let fields = [
        boot_rva,
        template_rva,
        area_rva,
        tls.image.len() as u64,
        tls.thread_pointer,
        entry_rva,
    ];
    for (i, value) in fields.iter().enumerate() {
        startup[128 + 8 * i..136 + 8 * i].copy_from_slice(&value.to_le_bytes());
    }
    let mut bytes = crate::pie::emit(image, records, boot_rva)?;
    bytes[56..58].copy_from_slice(&5u16.to_le_bytes());
    let mut header = Vec::with_capacity(56);
    header.extend_from_slice(&PT_TLS.to_le_bytes());
    header.extend_from_slice(&4u32.to_le_bytes());
    let size = tls.image.len() as u64;
    for value in [
        template_file,
        template_rva,
        template_rva,
        size,
        tls.memory_size,
        tls.alignment,
    ] {
        header.extend_from_slice(&value.to_le_bytes());
    }
    let at = crate::layout::HEADER_BYTES as usize;
    bytes[at..at + 56].copy_from_slice(&header);
    Ok(bytes)
}

/// A relocated static-PIE layout before ELF/startup serialization.
#[derive(Debug, PartialEq, Eq)]
pub struct Lowered {
    pub image: crate::layout::Image,
    pub relative: Vec<Relative>,
    pub tls: Option<Tls>,
    pub merged: crate::unwind::Merged,
    pub entry_rva: u64,
    pub map: String,
}
impl Lowered {
    /// Supplied final serialization. This consumes the layout so later stages
    /// cannot accidentally use it after startup descriptors have been written.
    pub fn emit(self) -> Result<crate::gc::Linked, Error> {
        let extra = self
            .image
            .placements
            .get(&EH_FRAME_HDR)
            .map(|p| (p.file_offset, p.address, p.size));
        let slot = crate::layout::HEADER_BYTES + if self.tls.is_some() { 56 } else { 0 };
        if extra.is_some()
            && self
                .image
                .placements
                .get(&UNWIND_PHDR)
                .map(|p| (p.file_offset, p.address, p.size, p.kind))
                != Some((
                    Some(slot),
                    crate::executable::BASE + slot,
                    56,
                    crate::layout::Kind::Text,
                ))
        {
            return Err(Error::InvalidReservation);
        }
        let has_tls = self.tls.is_some();
        let mut bytes = emit(
            self.image,
            &self.relative,
            self.tls.as_ref(),
            self.entry_rva,
        )?;
        if let Some((offset, address, size)) = extra {
            let offset = offset.ok_or(Error::InvalidReservation)?;
            let address = address
                .checked_sub(crate::executable::BASE)
                .ok_or(Error::InvalidReservation)?;
            bytes[56..58].copy_from_slice(&(if has_tls { 6u16 } else { 5 }).to_le_bytes());
            let header = &mut bytes[slot as usize..slot as usize + 56];
            header[..4].copy_from_slice(&crate::unwind::PT_GNU_EH_FRAME.to_le_bytes());
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

/// Lower a validated plan without consuming its input metadata or emitting ELF.
pub fn lower_prepared(prepared: &crate::gc::Prepared<'_>) -> Result<Lowered, Error> {
    lower_with_unwind(prepared, true)
}
pub fn lower_with_unwind(
    prepared: &crate::gc::Prepared<'_>,
    preserve_unwind: bool,
) -> Result<Lowered, Error> {
    let unwind = if preserve_unwind {
        UnwindPolicy::Preserve
    } else {
        UnwindPolicy::AbortOnly
    };
    lower_with(prepared, &BTreeMap::new(), &[], unwind)
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnwindPolicy {
    Preserve,
    AbortOnly,
}
pub fn lower_with(
    prepared: &crate::gc::Prepared<'_>,
    aliases: &BTreeMap<SectionId, SectionId>,
    order: &[SectionId],
    unwind: UnwindPolicy,
) -> Result<Lowered, Error> {
    use crate::layout::{Contribution, Kind};
    use crate::pie::{Fixup, PatchError, Value};
    use crate::resolve::{Binding, Selection, SymbolId};
    use crate::symbols::Definition;
    fn reserve(
        id: (usize, u16),
        kind: Kind,
        alignment: u64,
        bytes: &[u8],
        memory_size: u64,
    ) -> Contribution<'_> {
        Contribution {
            file: id.0,
            section: id.1,
            kind,
            alignment,
            bytes,
            memory_size,
        }
    }
    enum Bound {
        Image(Slot),
        Absolute(u64),
        Tls(i64),
    }
    let tables = prepared.tables();
    let (tls_inputs, merge_inputs): (Vec<_>, Vec<_>) = prepared
        .split()
        .iter()
        .copied()
        .partition(|s| s.flags == TLS_FLAGS);
    let tls = if tls_inputs.is_empty() {
        None
    } else {
        Some(tls_layout(&tls_inputs)?)
    };
    let tls_sections: BTreeMap<SectionId, &Special<'_>> =
        tls_inputs.iter().map(|s| (s.id, s)).collect();
    let merged = crate::unwind::merge(&merge_inputs)?;
    if merged.groups.len() > usize::from(u16::MAX - crate::unwind::MERGE_BASE) {
        return Err(Error::TooLarge);
    }
    let storage: BTreeMap<SectionId, &crate::layout::Contribution<'_>> = prepared
        .contributions()
        .iter()
        .map(|c| {
            (
                SectionId {
                    file: c.file,
                    section: c.section,
                },
                c,
            )
        })
        .collect();
    for (&from, &to) in aliases {
        if from == to
            || aliases.contains_key(&to)
            || !storage.get(&from).is_some_and(|c| c.kind == Kind::Text)
            || !storage.get(&to).is_some_and(|c| c.kind == Kind::Text)
        {
            return Err(Error::InvalidAlias { from, to });
        }
    }
    let retained = |id: SectionId| aliases.get(&id).copied().unwrap_or(id);
    let mut priority = BTreeMap::new();
    for &id in order {
        if !storage.get(&id).is_some_and(|c| c.kind == Kind::Text) {
            return Err(crate::unwind::Error::InvalidOrderSection(id).into());
        }
        let next = priority.len();
        priority.entry(retained(id)).or_insert(next);
    }
    let mut fdes = Vec::new();
    let mut described = std::collections::BTreeSet::new();
    for annotation in prepared
        .annotations()
        .iter()
        .filter(|_| unwind == UnwindPolicy::Preserve)
    {
        let records = prepared
            .annotation_relocations()
            .iter()
            .find(|(id, _)| *id == annotation.id)
            .map(|(_, r)| r.as_slice())
            .unwrap_or(&[]);
        let frames = crate::unwind::frames(annotation.bytes, records).map_err(|cause| {
            crate::unwind::Error::Unwind {
                section: annotation.id,
                cause,
            }
        })?;
        for frame in &frames {
            let crate::unwind::Record::Fde { cie, relocation } = frame.record else {
                continue;
            };
            let wrong = || crate::unwind::Error::FdeTarget {
                section: annotation.id,
                offset: frame.offset,
            };
            let r = &records[relocation];
            let Ok(Binding::Selected(Selection::Definition(id))) = crate::resolve::bind(
                tables,
                prepared.globals(),
                SymbolId {
                    file: annotation.id.file,
                    symbol: r.symbol as usize,
                },
            ) else {
                return Err(wrong().into());
            };
            let symbol = &tables[id.file][id.symbol];
            let Definition::Section(section) = symbol.definition else {
                return Err(wrong().into());
            };
            let target = SectionId {
                file: id.file,
                section,
            };
            if !prepared.live().sections.contains(&target) || aliases.contains_key(&target) {
                continue;
            }
            if !storage.get(&target).is_some_and(|c| c.kind == Kind::Text) {
                return Err(wrong().into());
            }
            let start = symbol
                .value
                .checked_add_signed(r.addend)
                .ok_or_else(wrong)?;
            let cie = frames.iter().find(|f| f.offset == cie).unwrap();
            fdes.push(crate::unwind::Fde {
                cie: &annotation.bytes[cie.offset as usize..(cie.offset + cie.size) as usize],
                fde: &annotation.bytes[frame.offset as usize..(frame.offset + frame.size) as usize],
                target: (target.file, target.section),
                start,
            });
            described.insert(annotation.id);
        }
    }
    let (eh_size, hdr_size) = crate::unwind::unwind_sizes(&fdes)?;
    let eh_bytes = vec![0u8; eh_size as usize];
    let hdr_bytes = vec![0u8; hdr_size as usize];
    // Pass 1: bind and lower every live relocation before layout.
    let mut got = Got::new();
    let mut plan = Vec::new();
    let mut runtime = 0usize;
    for (source, records) in prepared.relocations() {
        if aliases.contains_key(source) {
            continue;
        }
        if merged.sections.contains_key(source) {
            if records.iter().any(|r| r.kind != 0) {
                return Err(crate::unwind::Error::Merge {
                    section: *source,
                    cause: crate::unwind::MergeError::Relocated,
                }
                .into());
            }
            continue;
        }
        let code: &[u8] = match (storage.get(source), tls_sections.get(source)) {
            (Some(c), _) => c.bytes,
            (None, Some(s)) => s.bytes,
            (None, None) => return Err(crate::layout::Error::InvalidRelocationTarget.into()),
        };
        let nobits = tls_sections.get(source).is_some_and(|s| s.kind == 8)
            || storage.get(source).is_some_and(|c| c.kind == Kind::Bss);
        for r in records {
            if r.kind == 0 {
                continue;
            }
            if nobits {
                return Err(crate::layout::Error::InvalidRelocationTarget.into());
            }
            let mut r = *r;
            let reference = SymbolId {
                file: source.file,
                symbol: r.symbol as usize,
            };
            let bound = match crate::resolve::bind(tables, prepared.globals(), reference)
                .map_err(crate::layout::Error::from)?
            {
                Binding::Zero => Bound::Absolute(0),
                Binding::Selected(Selection::Common { .. }) => Bound::Image(Slot::Common(
                    tables[source.file][r.symbol as usize].name.to_vec(),
                )),
                Binding::Selected(Selection::Definition(id)) => {
                    let s = &tables[id.file][id.symbol];
                    match s.definition {
                        Definition::Absolute => Bound::Absolute(s.value),
                        Definition::Section(section) => {
                            let target = SectionId {
                                file: id.file,
                                section,
                            };
                            match tls.as_ref().and_then(|t| t.tpoff(target, s.value)) {
                                Some(offset) => Bound::Tls(offset),
                                None if merged.sections.contains_key(&target) => {
                                    let offset = if s.kind == 3 {
                                        s.value.checked_add_signed(r.addend).ok_or(
                                            crate::unwind::Error::Merge {
                                                section: target,
                                                cause: crate::unwind::MergeError::InvalidReference(
                                                    s.value,
                                                ),
                                            },
                                        )?
                                    } else {
                                        s.value
                                    };
                                    let (group, at) =
                                        crate::unwind::locate(&merged, target, offset)?;
                                    if s.kind == 3 {
                                        r.addend = 0;
                                    }
                                    Bound::Image(Slot::Section(
                                        SectionId {
                                            file: usize::MAX,
                                            section: crate::unwind::MERGE_BASE + group as u16,
                                        },
                                        at,
                                    ))
                                }
                                None => Bound::Image(Slot::Section(retained(target), s.value)),
                            }
                        }
                        _ => return Err(crate::gc::Error::InvalidSymbol.into()),
                    }
                }
            };
            let target = match bound {
                Bound::Image(_) => Target::Image,
                Bound::Absolute(_) => Target::Absolute,
                Bound::Tls(_) => Target::Tls,
            };
            let lowering =
                lower(code, r.offset, r.kind, target).map_err(|cause| Error::Reference {
                    section: *source,
                    offset: r.offset,
                    kind: r.kind,
                    cause,
                })?;
            let slot = match (&lowering, &bound) {
                (Lowering::Got, Bound::Image(slot)) => Some(got.offset(slot.clone())?),
                (Lowering::Got, Bound::Absolute(value)) => {
                    Some(got.offset(Slot::Constant(*value))?)
                }
                (Lowering::Got, Bound::Tls(offset)) => {
                    Some(got.offset(Slot::Constant(*offset as u64))?)
                }
                _ => None,
            };
            if lowering == Lowering::Fixup && r.kind == 1 && matches!(bound, Bound::Image(_)) {
                runtime = runtime.checked_add(1).ok_or(Error::TooLarge)?;
            }
            plan.push((*source, r, lowering, bound, slot));
        }
    }
    runtime = runtime
        .checked_add(
            got.slots()
                .iter()
                .filter(|s| !matches!(s, Slot::Constant(_)))
                .count(),
        )
        .ok_or(Error::TooLarge)?;
    let table_size = runtime
        .checked_mul(24)
        .filter(|n| *n as u64 <= crate::executable::MAX_IMAGE)
        .ok_or(Error::TooLarge)?;
    let table = vec![0u8; table_size];
    let got_bytes = vec![0u8; got.size() as usize];
    let phdr = [0u8; 56];
    let boot = vec![0u8; TLS_STARTUP_SIZE as usize];
    let mut contributions = Vec::new();
    if tls.is_some() {
        contributions.push(reserve(PHDR, Kind::Text, 8, &phdr, 56));
    }
    if !fdes.is_empty() {
        contributions.push(reserve(UNWIND_PHDR, Kind::Text, 8, &phdr, 56));
    }
    let mut ordered: Vec<_> = prepared
        .contributions()
        .iter()
        .filter(|c| {
            !aliases.contains_key(&SectionId {
                file: c.file,
                section: c.section,
            })
        })
        .collect();
    ordered.sort_by_key(|c| {
        priority
            .get(&SectionId {
                file: c.file,
                section: c.section,
            })
            .copied()
            .unwrap_or(usize::MAX)
    });
    contributions.extend(ordered.into_iter().map(|c| {
        reserve(
            (c.file, c.section),
            c.kind,
            c.alignment,
            c.bytes,
            c.memory_size,
        )
    }));
    for (group, value) in merged.groups.iter().enumerate() {
        contributions.push(reserve(
            (usize::MAX, crate::unwind::MERGE_BASE + group as u16),
            Kind::ReadOnly,
            value.alignment,
            &value.bytes,
            value.bytes.len() as u64,
        ));
    }
    contributions.push(reserve(
        crate::pie::BOOT,
        Kind::Text,
        16,
        crate::pie::STARTUP,
        crate::pie::STARTUP.len() as u64,
    ));
    contributions.push(reserve(
        crate::pie::RELA,
        Kind::ReadOnly,
        8,
        &table,
        table.len() as u64,
    ));
    if got.size() > 0 {
        contributions.push(reserve(GOT, Kind::Data, 8, &got_bytes, got.size()));
    }
    if let Some(t) = &tls {
        contributions.push(reserve(
            TLS_IMAGE,
            Kind::Data,
            t.alignment,
            &t.image,
            t.image.len() as u64,
        ));
        contributions.push(reserve(
            TLS_AREA,
            Kind::Bss,
            t.alignment,
            &[],
            t.thread_pointer + 8,
        ));
        contributions.push(reserve(TLS_BOOT, Kind::Text, 16, &boot, TLS_STARTUP_SIZE));
    }
    if !fdes.is_empty() {
        contributions.push(reserve(EH_FRAME, Kind::ReadOnly, 8, &eh_bytes, eh_size));
        contributions.push(reserve(
            EH_FRAME_HDR,
            Kind::ReadOnly,
            4,
            &hdr_bytes,
            hdr_size,
        ));
    }
    let commons: Vec<_> = prepared
        .globals()
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
    let rva = |p: &crate::layout::Placement| p.address - crate::executable::BASE;
    let got_rva = image.placements.get(&GOT).map(rva);
    let template_rva = image
        .placements
        .get(&TLS_IMAGE)
        .map(|p| (rva(p), p.file_offset.unwrap()));
    // Pass 2: patch with assigned addresses.
    let mut relative = Vec::new();
    for (source, r, lowering, bound, slot) in plan {
        let (file, size, place_base, writable) =
            match image.placements.get(&(source.file, source.section)) {
                Some(p) => (
                    p.file_offset
                        .ok_or(crate::layout::Error::InvalidRelocationTarget)?,
                    p.size,
                    rva(p),
                    p.kind == Kind::Data,
                ),
                None => {
                    let t = tls.as_ref().unwrap();
                    let (image_rva, image_file) = template_rva.unwrap();
                    let offset = t.offsets[&source];
                    (
                        image_file + offset,
                        tls_sections[&source].size,
                        image_rva + offset,
                        true,
                    )
                }
            };
        let fixup_error = |cause| {
            Error::Pie(crate::pie::Error::Fixup {
                section: source,
                offset: r.offset,
                kind: r.kind,
                cause,
            })
        };
        let place_rva = place_base.checked_add(r.offset).ok_or_else(|| {
            fixup_error(PatchError::Relocation(crate::relocate::Error::PatchBounds))
        })?;
        let bytes = &mut image.bytes[file as usize..(file + size) as usize];
        let value = |bound: &Bound| -> Result<Value, Error> {
            Ok(match bound {
                Bound::Absolute(v) => Value::Absolute(*v),
                Bound::Tls(_) => unreachable!("TLS targets never reach crate::pie::patch"),
                Bound::Image(Slot::Section(id, offset)) => {
                    Value::ImageRva(rva(&image.placements[&(id.file, id.section)]) + offset)
                }
                Bound::Image(Slot::Common(name)) => {
                    Value::ImageRva(image.common_addresses[name] - crate::executable::BASE)
                }
                Bound::Image(Slot::Constant(_)) => {
                    unreachable!("image targets are never constants")
                }
            })
        };
        let (kind, value) = match lowering {
            Lowering::Fixup => (r.kind, value(&bound)?),
            Lowering::Relax => (2, value(&bound)?),
            Lowering::Got => (2, Value::ImageRva(got_rva.unwrap() + slot.unwrap())),
            Lowering::ThreadOffset => {
                let Bound::Tls(offset) = bound else {
                    unreachable!("lowered only for TLS targets")
                };
                crate::relocate::apply(bytes, r.offset, 11, offset as u64, r.addend, place_rva)
                    .map_err(|e| fixup_error(PatchError::Relocation(e)))?;
                continue;
            }
        };
        if lowering == Lowering::Relax {
            bytes[r.offset as usize - 2] = 0x8d;
        }
        let fixup = Fixup {
            offset: r.offset,
            kind,
            value,
            addend: r.addend,
            place_rva,
            writable,
        };
        if let Some(record) = crate::pie::patch(bytes, fixup).map_err(fixup_error)? {
            relative.push(record);
        }
    }
    let (got_encoded, got_records) = got.encode(&image)?;
    if let Some(p) = image.placements.get(&GOT) {
        let at = p.file_offset.unwrap() as usize;
        image.bytes[at..at + got_encoded.len()].copy_from_slice(&got_encoded);
    }
    relative.extend(got_records);
    if !fdes.is_empty() {
        let mut placements: BTreeMap<_, _> = image
            .placements
            .iter()
            .map(|(id, p)| {
                (
                    *id,
                    crate::layout::Placement {
                        kind: p.kind,
                        file_offset: p.file_offset,
                        address: p.address,
                        size: p.size,
                    },
                )
            })
            .collect();
        for (actual, expected) in [
            (EH_FRAME, crate::unwind::EH_FRAME),
            (EH_FRAME_HDR, crate::unwind::EH_FRAME_HDR),
        ] {
            let p = &image.placements[&actual];
            placements.insert(
                expected,
                crate::layout::Placement {
                    kind: p.kind,
                    file_offset: p.file_offset,
                    address: p.address,
                    size: p.size,
                },
            );
        }
        let unwind = crate::unwind::emit_unwind(&fdes, &placements)?;
        for (id, bytes) in [(EH_FRAME, &unwind.eh_frame), (EH_FRAME_HDR, &unwind.hdr)] {
            let at = image.placements[&id].file_offset.unwrap() as usize;
            image.bytes[at..at + bytes.len()].copy_from_slice(bytes);
        }
    }
    let mut rows = prepared.map_rows().to_vec();
    for row in &mut rows {
        let id = retained(row.id);
        row.address = match image.placements.get(&(id.file, id.section)) {
            Some(p) => Some(rva(p)),
            None => tls
                .as_ref()
                .and_then(|t| t.offsets.get(&row.id))
                .map(|offset| template_rva.unwrap().0 + offset)
                .or_else(|| {
                    described
                        .contains(&row.id)
                        .then(|| rva(&image.placements[&EH_FRAME]))
                })
                .or_else(|| {
                    merged.sections.get(&row.id).map(|(group, _)| {
                        rva(&image.placements
                            [&(usize::MAX, crate::unwind::MERGE_BASE + *group as u16)])
                    })
                }),
        };
    }
    let commons = image
        .common_addresses
        .iter()
        .map(|(name, address)| (name.clone(), address - crate::executable::BASE))
        .collect();
    let mut map =
        crate::gc::render_map(&rows, &commons).replacen("TINYLINK MAP 1", "TINYLINK PIE MAP 1", 1);
    for (id, name) in [
        (GOT, ".got"),
        (TLS_IMAGE, ".tls.image"),
        (TLS_AREA, ".tls.area"),
    ] {
        if let Some(p) = image.placements.get(&id) {
            map.push_str(&format!("SYNTHETIC 0x{:016x} {} {name}\n", rva(p), p.size));
        }
    }
    for (group, value) in merged.groups.iter().enumerate() {
        let p = &image.placements[&(usize::MAX, crate::unwind::MERGE_BASE + group as u16)];
        let family = if value.flags & 0x20 != 0 {
            "str"
        } else {
            "cst"
        };
        map.push_str(&format!(
            "SYNTHETIC 0x{:016x} {} .rodata.{family}{}.{}\n",
            rva(p),
            p.size,
            value.entry_size,
            value.alignment
        ));
    }
    for (id, name) in [(EH_FRAME, ".eh_frame"), (EH_FRAME_HDR, ".eh_frame_hdr")] {
        if let Some(p) = image.placements.get(&id) {
            map.push_str(&format!("SYNTHETIC 0x{:016x} {} {name}\n", rva(p), p.size));
        }
    }
    let (entry_id, entry_offset) = prepared.entry();
    let entry_id = retained(entry_id);
    let entry_rva = rva(&image.placements[&(entry_id.file, entry_id.section)]) + entry_offset;
    Ok(Lowered {
        image,
        relative,
        tls,
        merged,
        entry_rva,
        map,
    })
}

/// Prepare, lower, and serialize, preserving the existing API and output.
pub fn link_with(
    inputs: &[crate::archive::Input<'_>],
    entry: &[u8],
    keep: &[&[u8]],
    classify: crate::gc::Classifier,
) -> Result<crate::gc::Linked, Error> {
    let prepared = crate::gc::prepare_with(inputs, entry, keep, classify)?;
    lower_prepared(&prepared)?.emit()
}

/// Explicit abort-only composition used by class14's controlled fixture.
/// This drops annotation records rather than claiming runtime unwind support.
pub fn link_abort_only(
    inputs: &[crate::archive::Input<'_>],
    entry: &[u8],
    keep: &[&[u8]],
    classify: crate::gc::Classifier,
) -> Result<crate::gc::Linked, Error> {
    let prepared = crate::gc::prepare_with(inputs, entry, keep, classify)?;
    lower_with_unwind(&prepared, false)?.emit()
}

/// [`link_with`] using [`policy`]. Supplied. Inputs without TLS sections or
/// GOT references produce exactly class9's output.
pub fn link(
    inputs: &[crate::archive::Input<'_>],
    entry: &[u8],
    keep: &[&[u8]],
) -> Result<crate::gc::Linked, Error> {
    link_with(inputs, entry, keep, policy)
}
