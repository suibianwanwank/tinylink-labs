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
    for root in roots {
        if !graph.contains_key(root) {
            return Err(Error::MissingNode(*root));
        }
    }
    let mut live = Live {
        sections: BTreeSet::new(),
        commons: BTreeSet::new(),
    };
    let mut stack: Vec<_> = roots.iter().rev().map(|id| Target::Section(*id)).collect();
    while let Some(edge) = stack.pop() {
        match edge {
            Target::Section(id) => {
                let edges = graph.get(&id).ok_or(Error::MissingNode(id))?;
                if live.sections.insert(id) {
                    stack.extend(edges.iter().rev().cloned());
                }
            }
            Target::Common(name) => {
                live.commons.insert(name);
            }
            Target::Absolute => {}
            Target::Undefined(name) => return Err(Error::Undefined(name)),
            Target::Invalid => return Err(Error::InvalidSymbol),
        }
    }
    Ok(live)
}

/// Sort rows by (file, section), independent of caller order. Emit one ASCII
/// line per allocated input section, including empty and discarded sections.
/// Format and byte escaping are specified in README. Include only live COMMON
/// allocations, sorted by raw byte name, after section rows. End every line in
/// a newline, including the header. The provided caller supplies unique row IDs.
pub fn render_map(rows: &[MapRow], commons: &BTreeMap<Vec<u8>, u64>) -> String {
    use std::fmt::Write;
    let escape = |name: &[u8]| -> String {
        if name.is_empty() {
            return "-".into();
        }
        let mut out = String::new();
        for &b in name {
            if (0x21..=0x7e).contains(&b) && b != 92 && b != 45 {
                out.push(b as char);
            } else {
                write!(out, "\\x{b:02x}").unwrap();
            }
        }
        out
    };
    let mut ordered: Vec<_> = rows.iter().collect();
    ordered.sort_by_key(|r| r.id);
    let mut out = String::from("TINYLINK MAP 1\n");
    for r in ordered {
        match r.address {
            Some(a) => writeln!(
                out,
                "KEEP {}:{} 0x{a:016x} {} {}",
                r.id.file,
                r.id.section,
                r.size,
                escape(&r.name)
            )
            .unwrap(),
            None => writeln!(
                out,
                "DROP {}:{} - {} {}",
                r.id.file,
                r.id.section,
                r.size,
                escape(&r.name)
            )
            .unwrap(),
        }
    }
    for (name, a) in commons {
        writeln!(out, "COMMON 0x{a:016x} {}", escape(name)).unwrap();
    }
    out
}

/// How the front end treats one allocated input section.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// Ordinary storage: a graph node and one class6 contribution of this kind.
    Storage(crate::layout::Kind),
    /// A graph node whose bytes the backend splits and places itself, such as
    /// an `SHF_MERGE` section. It is never a class6 contribution.
    Split,
    /// Metadata describing other sections, such as `.eh_frame`. It is not a
    /// graph node, root, or edge source; the backend decides which of its
    /// records survive from the liveness of the sections they describe.
    Annotation,
}

/// Classifies an allocated section from its header and name. `None` rejects
/// the section as unsupported, even if nothing references it.
pub type Classifier = fn(&crate::sections::Section, &[u8]) -> Option<Role>;

/// The class6 storage policy: exact type/flags pairs, independent of name.
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
    use crate::layout::{Contribution, Kind};
    use crate::resolve::{Binding, Selection, SymbolId};
    use crate::symbols::Definition;
    let inputs = crate::archive::select(inputs)?;
    let mut all_sections = Vec::new();
    let mut names = Vec::new();
    let mut tables = Vec::new();
    let mut contributions = Vec::new();
    let mut split = Vec::new();
    let mut annotations = Vec::new();
    let mut roles = BTreeMap::new();
    let mut graph = Graph::new();
    let mut relocations = Vec::new();
    let mut annotation_relocations = Vec::new();
    let mut metadata = Vec::new();
    let mut metadata_relocations = Vec::new();
    let mut metadata_ids = std::collections::BTreeSet::new();
    for (file, input) in inputs.iter().enumerate() {
        let parsed = crate::sections::sections(input).map_err(crate::layout::Error::from)?;
        let mut sections = Vec::new();
        let mut section_names = Vec::new();
        for (index, (s, name)) in parsed.into_iter().enumerate() {
            if s.flags & 2 != 0 {
                let id = SectionId {
                    file,
                    section: index as u16,
                };
                let role = classify(&s, name).ok_or(crate::layout::Error::UnsupportedSection)?;
                let bytes =
                    crate::sections::payload(input, &s).map_err(crate::layout::Error::from)?;
                let special = Special {
                    id,
                    name,
                    kind: s.kind,
                    flags: s.flags,
                    size: s.size,
                    entry_size: s.entry_size,
                    alignment: s.alignment,
                    bytes,
                };
                match role {
                    Role::Storage(kind) => {
                        contributions.push(Contribution {
                            file,
                            section: id.section,
                            kind,
                            alignment: s.alignment,
                            bytes,
                            memory_size: s.size,
                        });
                        graph.insert(id, Vec::new());
                    }
                    Role::Split => {
                        split.push(special);
                        graph.insert(id, Vec::new());
                    }
                    Role::Annotation => annotations.push(special),
                }
                roles.insert(id, role);
            } else if s.kind == 1 {
                let id = SectionId {
                    file,
                    section: index as u16,
                };
                let bytes =
                    crate::sections::payload(input, &s).map_err(crate::layout::Error::from)?;
                metadata.push(Special {
                    id,
                    name,
                    kind: s.kind,
                    flags: s.flags,
                    size: s.size,
                    entry_size: s.entry_size,
                    alignment: s.alignment,
                    bytes,
                });
                metadata_ids.insert(id);
            }
            sections.push(s);
            section_names.push(name);
        }
        let indices: Vec<_> = sections
            .iter()
            .enumerate()
            .filter(|(_, s)| s.kind == 2)
            .map(|(i, _)| i)
            .collect();
        if indices.len() != 1 {
            return Err(crate::layout::Error::SymbolTableCount.into());
        }
        tables.push(
            crate::symbols::symbols(input, &sections, indices[0])
                .map_err(crate::layout::Error::from)?,
        );
        for (index, s) in sections.iter().enumerate() {
            if s.kind == 9 && s.size != 0 {
                return Err(crate::layout::Error::InvalidRelocationTarget.into());
            }
            if s.kind != 4 {
                continue;
            }
            let records = crate::symbols::relocations(input, &sections, index)
                .map_err(crate::layout::Error::from)?;
            if s.link as usize != indices[0] {
                return Err(crate::layout::Error::InvalidRelocationTarget.into());
            }
            let target = SectionId {
                file,
                section: s.info as u16,
            };
            match roles.get(&target) {
                Some(Role::Annotation) => annotation_relocations.push((target, records)),
                Some(_) => relocations.push((target, records)),
                None if metadata_ids.contains(&target) => {
                    metadata_relocations.push((target, records))
                }
                None => {}
            }
        }
        names.push(section_names);
        all_sections.push(sections);
    }
    let globals = crate::resolve::build_globals(&tables).map_err(crate::layout::Error::from)?;
    let selected = globals
        .get(entry)
        .ok_or(crate::layout::Error::MissingEntry)?;
    let Selection::Definition(entry_id) = selected else {
        return Err(crate::layout::Error::InvalidEntry.into());
    };
    let entry_symbol = &tables[entry_id.file][entry_id.symbol];
    let Definition::Section(entry_section) = entry_symbol.definition else {
        return Err(crate::layout::Error::InvalidEntry.into());
    };
    let entry_key = SectionId {
        file: entry_id.file,
        section: entry_section,
    };
    let entry_contribution = contributions
        .iter()
        .find(|c| c.file == entry_key.file && c.section == entry_key.section)
        .ok_or(crate::layout::Error::InvalidEntry)?;
    if entry_contribution.kind != Kind::Text || entry_symbol.value >= entry_contribution.memory_size
    {
        return Err(crate::layout::Error::InvalidEntry.into());
    }
    let mut roots = vec![entry_key];
    for name in keep {
        let Some(Selection::Definition(id)) = globals.get(*name) else {
            return Err(Error::InvalidKeep(name.to_vec()));
        };
        let sym = &tables[id.file][id.symbol];
        let Definition::Section(section) = sym.definition else {
            return Err(Error::InvalidKeep(name.to_vec()));
        };
        let key = SectionId {
            file: id.file,
            section,
        };
        if !graph.contains_key(&key) || sym.value > all_sections[id.file][section as usize].size {
            return Err(Error::InvalidKeep(name.to_vec()));
        }
        roots.push(key);
    }
    for (source, records) in &relocations {
        for r in records {
            if r.kind == 0 {
                continue;
            }
            let reference = SymbolId {
                file: source.file,
                symbol: r.symbol as usize,
            };
            let target = match crate::resolve::bind(&tables, &globals, reference) {
                Err(crate::resolve::Error::Undefined { name, .. }) => Target::Undefined(name),
                Err(_) => Target::Invalid,
                Ok(Binding::Zero) => Target::Absolute,
                Ok(Binding::Selected(Selection::Common { .. })) => {
                    Target::Common(tables[source.file][r.symbol as usize].name.to_vec())
                }
                Ok(Binding::Selected(Selection::Definition(id))) => {
                    let sym = &tables[id.file][id.symbol];
                    match sym.definition {
                        Definition::Absolute => Target::Absolute,
                        Definition::Section(section) => {
                            let key = SectionId {
                                file: id.file,
                                section,
                            };
                            if graph.contains_key(&key)
                                && sym.value <= all_sections[id.file][section as usize].size
                            {
                                Target::Section(key)
                            } else {
                                Target::Invalid
                            }
                        }
                        _ => Target::Invalid,
                    }
                }
            };
            graph.get_mut(source).unwrap().push(target);
        }
    }
    let live = trace(&graph, &roots)?;
    let mut map_rows = Vec::new();
    for (file, sections) in all_sections.iter().enumerate() {
        for (index, s) in sections.iter().enumerate() {
            if s.flags & 2 != 0 {
                map_rows.push(MapRow {
                    id: SectionId {
                        file,
                        section: index as u16,
                    },
                    name: names[file][index].to_vec(),
                    size: s.size,
                    address: None,
                });
            }
        }
    }
    contributions.retain(|c| {
        live.sections.contains(&SectionId {
            file: c.file,
            section: c.section,
        })
    });
    split.retain(|s| live.sections.contains(&s.id));
    relocations.retain(|(source, _)| live.sections.contains(source));
    let entry_offset = entry_symbol.value;
    Ok(Prepared {
        contributions,
        split,
        annotations,
        annotation_relocations,
        metadata,
        metadata_relocations,
        tables,
        globals,
        relocations,
        live,
        map_rows,
        entry: entry_key,
        entry_offset,
    })
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
/// with class6, and apply live relocations with class5. Backends that emit
/// extra metadata reuse it instead of copying it. The class6 and class5
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

/// The class8 map rows of `prepared` with addresses from a lowered image.
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
