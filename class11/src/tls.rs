//! A Global Offset Table and static thread-local storage for the pie PIE.
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
/// Additional program-header slot for the unwind index, after PT_TLS if present.
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
    /// A pie error, including fixup failures with their relocation context.
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
    /// A fold alias is not a direct mapping between two distinct live Text
    /// inputs, or its representative is itself aliased.
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
/// split sections placed in the TLS template. Mergeable read-only inputs use
/// unwind split policy; other inputs follow the layout storage table.
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
    let _ = sections;
    todo!("C11.1: place initialized and zero-filled thread-local storage")
}

impl Tls {
    /// TP offset of byte `offset` of `section`: its template offset minus
    /// `thread_pointer`. Variant II TLS puts the block below the thread
    /// pointer, so this is negative for every byte of the template. `None`
    /// for a section outside the template.
    pub fn tpoff(&self, section: SectionId, offset: u64) -> Option<i64> {
        let _ = (section, offset);
        todo!("C11.1: convert a template position into a thread-pointer offset")
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
    let _ = (code, offset, kind, target);
    todo!("C11.2: select GOT, relaxation, TLS, or ordinary fixup lowering")
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
        todo!("C11.3: assign each distinct GOT value one slot")
    }
    /// Encode the table for an image planned with a [`GOT`] reservation of
    /// exactly [`Got::size`] bytes. Constants are stored; image addresses are
    /// zero in the file with a RELATIVE record each, in slot order.
    pub fn encode(&self, image: &crate::layout::Image) -> Result<(Vec<u8>, Vec<Relative>), Error> {
        let _ = image;
        todo!("C11.3: store constants and request load-time addresses")
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
    let _ = (&image, records, tls, entry_rva);
    todo!("C11.4: install the TLS startup stage and describe the template")
}

/// A relocated static-PIE layout before its ELF headers and startup descriptors
/// are serialized. Keep this result with the [`crate::gc::Prepared`] that produced
/// it: placements refer to the selected input identities in that plan.
#[derive(Debug, PartialEq, Eq)]
pub struct Lowered {
    /// layout planning coordinates: placement addresses still include
    /// `crate::executable::BASE`. Patched PIE fields and map addresses use RVAs.
    pub image: crate::layout::Image,
    /// Ordered load-time fixups, including image-valued GOT slots.
    pub relative: Vec<Relative>,
    /// The TLS template and its input-section offsets, if TLS is present.
    pub tls: Option<Tls>,
    /// Retained mapping of non-TLS split inputs to shared read-only groups.
    pub merged: crate::unwind::Merged,
    /// User entry RVA, before the relocation/TLS startup stages.
    pub entry_rva: u64,
    /// Complete deterministic PIE map, with synthetic GOT/TLS rows.
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

/// Lay out TLS, lower live relocations, and build the GOT from a validated plan.
/// Split sections are TLS inputs or unwind mergeable inputs; annotation
/// metadata remains available in `prepared`; supported unwind records are rebuilt.
/// Return the planned and patched image without serializing ELF/startup headers.
pub fn lower_prepared(prepared: &crate::gc::Prepared<'_>) -> Result<Lowered, Error> {
    lower_with_unwind(prepared, true)
}

/// Lower with an explicit runtime-unwind policy. Only a caller restricted to
/// abort-only programs that do not require backtraces may disable preservation.
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

/// Whether output programs require runtime unwind metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnwindPolicy {
    Preserve,
    /// Only for programs that do not use unwinding or runtime backtraces.
    AbortOnly,
}

/// Lower with compatible fold aliases and a requested input Text order.
/// Aliases map directly to retained representatives. The caller must establish
/// instruction, reference, address-identity and unwind compatibility first.
/// Keep synthetic header reservations ahead of the requested Text inputs.
/// An invalid order entry is `Merge(crate::unwind::Error::InvalidOrderSection(id))`.
pub fn lower_with(
    prepared: &crate::gc::Prepared<'_>,
    aliases: &BTreeMap<SectionId, SectionId>,
    order: &[SectionId],
    unwind: UnwindPolicy,
) -> Result<Lowered, Error> {
    let _ = (prepared, aliases, order, unwind);
    todo!("C11.5: lower the validated plan while retaining layout for later stages")
}

/// Supplied composition: prepare inputs with `classify`, lower, then serialize.
/// `classify` must classify TLS sections as `Role::Split`, as [`policy`] does.
pub fn link_with(
    inputs: &[crate::archive::Input<'_>],
    entry: &[u8],
    keep: &[&[u8]],
    classify: crate::gc::Classifier,
) -> Result<crate::gc::Linked, Error> {
    let prepared = crate::gc::prepare_with(inputs, entry, keep, classify)?;
    lower_prepared(&prepared)?.emit()
}

/// Explicit abort-only composition used by driver's controlled fixture.
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
/// GOT references or mergeable inputs produce exactly pie's output.
pub fn link(
    inputs: &[crate::archive::Input<'_>],
    entry: &[u8],
    keep: &[&[u8]],
) -> Result<crate::gc::Linked, Error> {
    link_with(inputs, entry, keep, policy)
}
