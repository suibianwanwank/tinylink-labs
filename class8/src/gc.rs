//! Section reachability, dead stripping, and deterministic link maps.
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SectionId {
    pub file: usize,
    pub section: u16,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    Section(SectionId),
    Common(Vec<u8>),
    Absolute,
    Undefined(Vec<u8>),
    Invalid,
}
pub type Graph = BTreeMap<SectionId, Vec<Target>>;
#[derive(Debug, PartialEq, Eq)]
pub struct Live {
    pub sections: BTreeSet<SectionId>,
    pub commons: BTreeSet<Vec<u8>>,
}
#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    MissingNode(SectionId),
    Undefined(Vec<u8>),
    InvalidSymbol,
    Archives(crate::archive::Error),
    Output(crate::layout::Error),
    InvalidKeep(Vec<u8>),
}
impl From<crate::archive::Error> for Error {
    fn from(e: crate::archive::Error) -> Self {
        Self::Archives(e)
    }
}
impl From<crate::layout::Error> for Error {
    fn from(e: crate::layout::Error) -> Self {
        Self::Output(e)
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapRow {
    pub id: SectionId,
    pub name: Vec<u8>,
    pub size: u64,
    pub address: Option<u64>,
}
#[derive(Debug, PartialEq, Eq)]
pub struct Linked {
    pub bytes: Vec<u8>,
    pub map: String,
}

/// Validate roots in supplied order, then traverse iteratively. Each section is
/// visited once. Inspect edges only when their source is live; an unreachable
/// Undefined/Invalid/missing section target does not fail tracing. Visit roots
/// and outgoing edges in supplied order using depth-first traversal. Keep COMMON
/// names separately. Absolute targets add no storage. Empty roots yield no live
/// nodes. Duplicate roots and cycles are legal. Return the first encountered
/// error in that traversal order; see README for examples.
pub fn trace(graph: &Graph, roots: &[SectionId]) -> Result<Live, Error> {
    let _ = (graph, roots);
    todo!("C8.1: compute live storage without resolving dead references")
}

/// Sort rows by (file, section), independent of caller order. Emit one ASCII
/// line per allocated input section, including empty and discarded sections.
/// Format and byte escaping are specified in README. Include only live COMMON
/// allocations, sorted by raw byte name, after section rows. End every line in
/// a newline, including the header. The provided caller supplies unique row IDs.
pub fn render_map(rows: &[MapRow], commons: &BTreeMap<Vec<u8>, u64>) -> String {
    let _ = (rows, commons);
    todo!("C8.2: explain retained and discarded storage with a stable byte-safe map")
}

/// How the front end treats one allocated input section.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// Ordinary storage: a graph node and one layout contribution of this kind.
    Storage(crate::layout::Kind),
    /// A graph node whose bytes the backend splits and places itself, such as
    /// an `SHF_MERGE` section. It is never a layout contribution.
    Split,
    /// Metadata describing other sections, such as `.eh_frame`. It is not a
    /// graph node, root, or edge source; the backend decides which of its
    /// records survive from the liveness of the sections they describe.
    Annotation,
}

/// Classifies an allocated section from its header and name. `None` rejects
/// the section as unsupported, even if nothing references it.
pub type Classifier = fn(&crate::sections::Section, &[u8]) -> Option<Role>;

/// The layout storage policy: exact type/flags pairs, independent of name.
/// Supplied infrastructure; `prepare` uses it.
pub fn storage_policy(section: &crate::sections::Section, _name: &[u8]) -> Option<Role> {
    use crate::layout::Kind;
    let kind = match (section.kind, section.flags) {
        (1, 6) => Kind::Text,
        (1, 2) => Kind::ReadOnly,
        (1, 3) => Kind::Data,
        (8, 3) => Kind::Bss,
        _ => return None,
    };
    Some(Role::Storage(kind))
}

/// An allocated input section whose placement is owned by the backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Special<'a> {
    pub id: SectionId,
    /// Section name from the input's section-name table.
    pub name: &'a [u8],
    /// ELF section type (`sh_type`), e.g. PROGBITS=1 or NOBITS=8.
    pub kind: u32,
    pub flags: u64,
    /// Memory size (`sh_size`); for NOBITS this exceeds `bytes.len()`.
    pub size: u64,
    pub entry_size: u64,
    pub alignment: u64,
    pub bytes: &'a [u8],
}

/// Validated front-end state produced by [`prepare_with`].
///
/// Storage contributions, split sections, and relocation groups contain only
/// live sections. Annotations and their relocations are retained for every
/// selected object because their liveness is decided by the backend. Map rows
/// retain every allocated section, including discarded ones. All identities use
/// the same selected-object ordering. Fields are private; backends receive
/// immutable views and cannot construct or alter a plan.
pub struct Prepared<'a> {
    contributions: Vec<crate::layout::Contribution<'a>>,
    split: Vec<Special<'a>>,
    annotations: Vec<Special<'a>>,
    annotation_relocations: Vec<(SectionId, Vec<crate::symbols::Rela>)>,
    metadata: Vec<Special<'a>>,
    metadata_relocations: Vec<(SectionId, Vec<crate::symbols::Rela>)>,
    tables: Vec<Vec<crate::symbols::Symbol<'a>>>,
    globals: crate::resolve::Globals,
    relocations: Vec<(SectionId, Vec<crate::symbols::Rela>)>,
    live: Live,
    map_rows: Vec<MapRow>,
    entry: SectionId,
    entry_offset: u64,
}

impl<'a> Prepared<'a> {
    /// Live `Role::Storage` sections in file, then section-index order.
    pub fn contributions(&self) -> &[crate::layout::Contribution<'a>] {
        &self.contributions
    }
    /// Live `Role::Split` sections in file, then section-index order.
    pub fn split(&self) -> &[Special<'a>] {
        &self.split
    }
    /// Every `Role::Annotation` section of the selected objects, in order.
    pub fn annotations(&self) -> &[Special<'a>] {
        &self.annotations
    }
    /// RELA groups whose target is an annotation, in file/section order.
    pub fn annotation_relocations(&self) -> &[(SectionId, Vec<crate::symbols::Rela>)] {
        &self.annotation_relocations
    }
    /// Every non-allocated PROGBITS section of the selected objects, such as
    /// `.debug_info` or `.comment`, in file, then section-index order.
    pub fn metadata(&self) -> &[Special<'a>] {
        &self.metadata
    }
    /// RELA groups whose target is a [`Prepared::metadata`] section.
    pub fn metadata_relocations(&self) -> &[(SectionId, Vec<crate::symbols::Rela>)] {
        &self.metadata_relocations
    }
    /// Symbol tables of the selected objects, indexed by selected ordinal.
    pub fn tables(&self) -> &[Vec<crate::symbols::Symbol<'a>>] {
        &self.tables
    }
    /// Global symbol selections over [`Prepared::tables`].
    pub fn globals(&self) -> &crate::resolve::Globals {
        &self.globals
    }
    /// RELA groups whose target section is a live graph node.
    pub fn relocations(&self) -> &[(SectionId, Vec<crate::symbols::Rela>)] {
        &self.relocations
    }
    /// The traced live set.
    pub fn live(&self) -> &Live {
        &self.live
    }
    /// One unplaced row per allocated section of every selected object.
    pub fn map_rows(&self) -> &[MapRow] {
        &self.map_rows
    }
    /// The entry section and the entry symbol's offset within it.
    pub fn entry(&self) -> (SectionId, u64) {
        (self.entry, self.entry_offset)
    }
}

/// Select archives, validate selected objects and RELA structure, resolve
/// globals, classify allocated sections with `classify`, and trace the entry
/// plus explicit roots. Return the live plan without assigning addresses or
/// applying patches. README specifies roles, root policy, dead-reference
/// errors, and validation boundaries.
pub fn prepare_with<'a>(
    inputs: &[crate::archive::Input<'a>],
    entry: &[u8],
    keep: &[&[u8]],
    classify: Classifier,
) -> Result<Prepared<'a>, Error> {
    let _ = (inputs, entry, keep, classify);
    todo!("C8.3: select and validate the live ELF link plan before assigning addresses")
}

/// [`prepare_with`] using [`storage_policy`]. Supplied.
pub fn prepare<'a>(
    inputs: &[crate::archive::Input<'a>],
    entry: &[u8],
    keep: &[&[u8]],
) -> Result<Prepared<'a>, Error> {
    prepare_with(inputs, entry, keep, storage_policy)
}

/// A planned and relocated ET_EXEC image, ready for `crate::layout::emit`.
#[derive(Debug, PartialEq, Eq)]
pub struct Lowered {
    pub image: crate::layout::Image,
    pub entry_address: u64,
}

/// Supplied ET_EXEC lowering of a plan: allocate live COMMONs, plan storage
/// with layout, and apply live relocations with relocate. Backends that emit
/// extra metadata reuse it instead of copying it. The layout and relocate
/// algorithms remain student prerequisites; this contains no fallback.
pub fn lower_exec(prepared: &Prepared<'_>) -> Result<Lowered, Error> {
    use crate::resolve::{Binding, Selection, SymbolId};
    use crate::symbols::Definition;
    let (tables, globals) = (&prepared.tables, &prepared.globals);
    let commons: Vec<_> = globals
        .iter()
        .filter_map(|(name, s)| match s {
            Selection::Common {
                size, alignment, ..
            } if prepared.live.commons.contains(name) => Some(crate::layout::Common {
                name,
                size: *size,
                alignment: *alignment,
            }),
            _ => None,
        })
        .collect();
    let mut image = crate::layout::plan(&prepared.contributions, &commons)?;
    let entry = prepared.entry;
    let entry_address =
        image.placements[&(entry.file, entry.section)].address + prepared.entry_offset;
    for (source, records) in &prepared.relocations {
        let p = &image.placements[&(source.file, source.section)];
        let (address, size, start) = (p.address, p.size, p.file_offset);
        for r in records {
            if r.kind == 0 {
                continue;
            }
            let start = start.ok_or(crate::layout::Error::InvalidRelocationTarget)?;
            let reference = SymbolId {
                file: source.file,
                symbol: r.symbol as usize,
            };
            let value = match crate::resolve::bind(tables, globals, reference)
                .map_err(crate::layout::Error::from)?
            {
                Binding::Zero => 0,
                Binding::Selected(Selection::Common { .. }) => {
                    image.common_addresses[tables[source.file][r.symbol as usize].name]
                }
                Binding::Selected(Selection::Definition(id)) => {
                    let s = &tables[id.file][id.symbol];
                    match s.definition {
                        Definition::Absolute => s.value,
                        Definition::Section(sec) => {
                            image.placements[&(id.file, sec)].address + s.value
                        }
                        _ => return Err(Error::InvalidSymbol),
                    }
                }
            };
            let place = address
                .checked_add(r.offset)
                .ok_or(crate::layout::Error::Relocation(
                    crate::relocate::Error::PatchBounds,
                ))?;
            crate::relocate::apply(
                &mut image.bytes[start as usize..(start + size) as usize],
                r.offset,
                r.kind,
                value,
                r.addend,
                place,
            )
            .map_err(crate::layout::Error::from)?;
        }
    }
    Ok(Lowered {
        image,
        entry_address,
    })
}

/// The gc map rows of `prepared` with addresses from a lowered image.
/// Supplied.
pub fn placed_rows(prepared: &Prepared<'_>, image: &crate::layout::Image) -> Vec<MapRow> {
    let mut rows = prepared.map_rows.clone();
    for row in &mut rows {
        row.address = image
            .placements
            .get(&(row.id.file, row.id.section))
            .map(|p| p.address);
    }
    rows
}

/// Supplied ET_EXEC backend: [`prepare`], [`lower_exec`], a map, and
/// `crate::layout::emit`. Later classes reuse `prepare_with` and replace this
/// address-dependent backend.
pub fn link(
    inputs: &[crate::archive::Input<'_>],
    entry: &[u8],
    keep: &[&[u8]],
) -> Result<Linked, Error> {
    let prepared = prepare(inputs, entry, keep)?;
    let Lowered {
        image,
        entry_address,
    } = lower_exec(&prepared)?;
    let map = render_map(&placed_rows(&prepared, &image), &image.common_addresses);
    Ok(Linked {
        bytes: crate::layout::emit(image, entry_address)?,
        map,
    })
}
