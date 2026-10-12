//! Identical code folding and section ordering.
//!
//! Templates, inlined helpers, and copy-pasted code leave many byte-identical
//! functions in a program. Folding keeps one copy and redirects every
//! reference to it, subject to this project's conservative address-significance
//! policy. Non-call references and explicitly kept functions pin their targets;
//! no compiler-provided address-significance table is consumed here. Ordering
//! places chosen functions first, the basis of hot/cold layout. README is the
//! contract.
use crate::debug::Aliases;
use crate::gc::SectionId;

/// What a relocation in a candidate refers to.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Edge {
    /// Byte `offset` of a section; it need not be a candidate.
    Section { section: SectionId, offset: u64 },
    /// A COMMON allocation, by name.
    Common(Vec<u8>),
    /// An ABS value, or zero for an unresolved WEAK reference.
    Constant(u64),
}

/// One non-NONE relocation of a candidate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reference {
    pub offset: u64,
    pub kind: u32,
    pub addend: i64,
    pub target: Edge,
}

/// Runtime unwind bytes used for conservative folding equivalence.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct UnwindDescription {
    /// Initial location relative to the input code section.
    pub start: u64,
    /// Complete supported CIE, including its record length.
    pub cie: Vec<u8>,
    /// Complete FDE with only its CIE pointer and initial-location field zeroed.
    /// Range and unwind instructions remain part of exact comparison.
    pub fde: Vec<u8>,
}

/// A live Text section considered for folding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate<'a> {
    pub id: SectionId,
    pub bytes: &'a [u8],
    /// Normalized alignment (at least 1).
    pub alignment: u64,
    /// In ascending offset order.
    pub references: Vec<Reference>,
    /// Supported unwind records, sorted by (start, CIE bytes, normalized FDE).
    /// Empty means no runtime unwind description; it differs from nonempty.
    pub unwind: Vec<UnwindDescription>,
    /// Pinned by the significance policy: do not share its address with another
    /// section, even when its bytes and references match.
    pub significant: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    FrontEnd(crate::gc::Error),
    Output(crate::layout::Error),
    Finish(crate::debug::Error),
    MergeUnwind(crate::unwind::Error),
    PieTls(crate::tls::Error),
    /// An order entry that does not name a live Text section definition.
    InvalidOrder(Vec<u8>),
    /// A section passed to [`lower`] in `order` that is not live Text.
    InvalidOrderSection(SectionId),
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
impl From<crate::debug::Error> for Error {
    fn from(e: crate::debug::Error) -> Self {
        Self::Finish(e)
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

/// Final image kind and its runtime-unwind requirement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    EtExec,
    StaticPie(crate::tls::UnwindPolicy),
}

/// Partition candidates into classes of interchangeable sections, as the
/// largest relation consistent with README's equivalence rules, and map every
/// member of a class except the smallest `SectionId` to that representative.
/// Significant candidates never join a class with another section.
pub fn fold(candidates: &[Candidate<'_>]) -> Aliases {
    use std::collections::BTreeMap;
    #[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
    enum Target<'e> {
        Class(usize, u64),
        Fixed(&'e Edge),
    }
    fn target<'e>(
        index: &BTreeMap<SectionId, usize>,
        class: &[usize],
        edge: &'e Edge,
    ) -> Target<'e> {
        match edge {
            Edge::Section { section, offset } => match index.get(section) {
                Some(&j) => Target::Class(class[j], *offset),
                None => Target::Fixed(edge),
            },
            _ => Target::Fixed(edge),
        }
    }
    let index: BTreeMap<SectionId, usize> = candidates
        .iter()
        .enumerate()
        .map(|(i, c)| (c.id, i))
        .collect();
    // Optimistic start: non-significant candidates with equal bytes, alignment,
    // and relocation shape are presumed equal, whatever their candidate targets.
    let mut initial: BTreeMap<_, usize> = BTreeMap::new();
    let mut class: Vec<usize> = Vec::with_capacity(candidates.len());
    for (i, c) in candidates.iter().enumerate() {
        let shape: Vec<_> = c
            .references
            .iter()
            .map(|r| {
                let fixed = match &r.target {
                    Edge::Section { section, .. } if index.contains_key(section) => None,
                    other => Some(other),
                };
                (r.offset, r.kind, r.addend, fixed)
            })
            .collect();
        let key = (
            c.significant.then_some(i),
            c.bytes,
            c.alignment,
            &c.unwind,
            shape,
        );
        let next = initial.len();
        class.push(*initial.entry(key).or_insert(next));
    }
    // Refine: split any class whose members reference different classes.
    // Classes only split, so the count reaching a fixed point ends the loop.
    let mut count = initial.len();
    loop {
        let mut signatures: BTreeMap<(usize, Vec<Target<'_>>), usize> = BTreeMap::new();
        let mut next_class = Vec::with_capacity(candidates.len());
        for (i, c) in candidates.iter().enumerate() {
            let targets = c
                .references
                .iter()
                .map(|r| target(&index, &class, &r.target))
                .collect();
            let next = signatures.len();
            next_class.push(*signatures.entry((class[i], targets)).or_insert(next));
        }
        let stable = signatures.len() == count;
        count = signatures.len();
        class = next_class;
        if stable {
            break;
        }
    }
    let mut representative: BTreeMap<usize, SectionId> = BTreeMap::new();
    for (i, c) in candidates.iter().enumerate() {
        let entry = representative.entry(class[i]).or_insert(c.id);
        *entry = (*entry).min(c.id);
    }
    candidates
        .iter()
        .enumerate()
        .filter_map(|(i, c)| {
            let to = representative[&class[i]];
            (to != c.id).then_some((c.id, to))
        })
        .collect()
}

/// Decode supported annotations and associate normalized unwind descriptions
/// with their live Text targets. Reject unsupported records through class10;
/// README specifies binding, normalization, and deterministic ordering.
pub fn unwind_descriptions(
    prepared: &crate::gc::Prepared<'_>,
) -> Result<std::collections::BTreeMap<SectionId, Vec<UnwindDescription>>, Error> {
    use crate::resolve::{Binding, Selection, SymbolId};
    use crate::symbols::Definition;
    use std::collections::BTreeMap;
    let mut output: BTreeMap<SectionId, Vec<UnwindDescription>> = BTreeMap::new();
    for annotation in prepared.annotations() {
        let records = prepared
            .annotation_relocations()
            .iter()
            .find(|(id, _)| *id == annotation.id)
            .map(|(_, records)| records.as_slice())
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
            let reference = SymbolId {
                file: annotation.id.file,
                symbol: r.symbol as usize,
            };
            let Ok(Binding::Selected(Selection::Definition(id))) =
                crate::resolve::bind(prepared.tables(), prepared.globals(), reference)
            else {
                return Err(wrong().into());
            };
            let symbol = &prepared.tables()[id.file][id.symbol];
            let Definition::Section(section) = symbol.definition else {
                return Err(wrong().into());
            };
            let target = SectionId {
                file: id.file,
                section,
            };
            if !prepared.live().sections.contains(&target) {
                continue;
            }
            if !prepared.contributions().iter().any(|c| {
                c.file == target.file
                    && c.section == target.section
                    && c.kind == crate::layout::Kind::Text
            }) {
                return Err(wrong().into());
            }
            let start = symbol
                .value
                .checked_add_signed(r.addend)
                .ok_or_else(wrong)?;
            let cie = frames
                .iter()
                .find(|f| f.offset == cie)
                .expect("validated CIE reference");
            let mut fde = annotation.bytes
                [frame.offset as usize..(frame.offset + frame.size) as usize]
                .to_vec();
            fde[4..12].fill(0);
            output.entry(target).or_default().push(UnwindDescription {
                start,
                cie: annotation.bytes[cie.offset as usize..(cie.offset + cie.size) as usize]
                    .to_vec(),
                fde,
            });
        }
    }
    for descriptions in output.values_mut() {
        descriptions.sort();
    }
    Ok(output)
}

/// Supplied: reuse class10's merge/unwind lowering with compatible aliases and
/// the requested Text order; do not maintain an independent layout algorithm.
pub fn lower(
    prepared: &crate::gc::Prepared<'_>,
    aliases: &Aliases,
    order: &[SectionId],
) -> Result<crate::unwind::Lowered, Error> {
    match crate::unwind::lower_with(prepared, aliases, order) {
        Err(crate::unwind::Error::InvalidOrderSection(id)) => Err(Error::InvalidOrderSection(id)),
        result => Ok(result?),
    }
}

/// Supplied cumulative PIE lowering. Folding and ordering use the same input
/// identities as ET_EXEC; class11 owns GOT/TLS/runtime relocation and layout.
pub fn lower_pie(
    prepared: &crate::gc::Prepared<'_>,
    aliases: &Aliases,
    order: &[SectionId],
    unwind: crate::tls::UnwindPolicy,
) -> Result<crate::tls::Lowered, Error> {
    match crate::tls::lower_with(prepared, aliases, order, unwind) {
        Err(crate::tls::Error::Merge(crate::unwind::Error::InvalidOrderSection(id))) => {
            Err(Error::InvalidOrderSection(id))
        }
        result => Ok(result?),
    }
}

/// Link with safe ICF and an ordering by symbol name, then attach debug
/// information and section headers with `crate::debug::finish`. The map gains one
/// `FOLD` line per aliased section.
pub fn link(
    inputs: &[crate::archive::Input<'_>],
    entry: &[u8],
    keep: &[&[u8]],
    order: &[&[u8]],
) -> Result<crate::gc::Linked, Error> {
    link_with(
        inputs,
        entry,
        keep,
        order,
        crate::unwind::policy,
        Backend::EtExec,
    )
}

/// Supplied static PIE path with runtime unwind preservation.
pub fn link_pie(
    inputs: &[crate::archive::Input<'_>],
    entry: &[u8],
    keep: &[&[u8]],
    order: &[&[u8]],
) -> Result<crate::gc::Linked, Error> {
    link_with(
        inputs,
        entry,
        keep,
        order,
        crate::tls::policy,
        Backend::StaticPie(crate::tls::UnwindPolicy::Preserve),
    )
}

/// Integrate candidate analysis, folding, ordering, shared lowering and debug
/// metadata. `classify` describes inputs; `backend` describes the output.
/// Abort-only PIE skips runtime unwind decoding and comparison explicitly;
/// it does not relax address-significance or debug-reference rules.
pub fn link_with(
    inputs: &[crate::archive::Input<'_>],
    entry: &[u8],
    keep: &[&[u8]],
    order: &[&[u8]],
    classify: crate::gc::Classifier,
    backend: Backend,
) -> Result<crate::gc::Linked, Error> {
    use crate::layout::Kind;
    use crate::resolve::{Binding, Selection, SymbolId};
    use crate::symbols::Definition;
    let prepared = crate::gc::prepare_with(inputs, entry, keep, classify)?;
    let (tables, globals) = (prepared.tables(), prepared.globals());
    let text: std::collections::BTreeMap<SectionId, &crate::layout::Contribution<'_>> = prepared
        .contributions()
        .iter()
        .filter(|c| c.kind == Kind::Text)
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
    let edge = |file: usize, symbol: u32| -> Result<Edge, Error> {
        let reference = SymbolId {
            file,
            symbol: symbol as usize,
        };
        Ok(
            match crate::resolve::bind(tables, globals, reference)
                .map_err(crate::layout::Error::from)?
            {
                Binding::Zero => Edge::Constant(0),
                Binding::Selected(Selection::Common { .. }) => {
                    Edge::Common(tables[file][symbol as usize].name.to_vec())
                }
                Binding::Selected(Selection::Definition(id)) => {
                    let s = &tables[id.file][id.symbol];
                    match s.definition {
                        Definition::Absolute => Edge::Constant(s.value),
                        Definition::Section(section) => Edge::Section {
                            section: SectionId {
                                file: id.file,
                                section,
                            },
                            offset: s.value,
                        },
                        _ => return Err(crate::gc::Error::InvalidSymbol.into()),
                    }
                }
            },
        )
    };
    let mut significant = std::collections::BTreeSet::new();
    let mut references: std::collections::BTreeMap<SectionId, Vec<Reference>> =
        std::collections::BTreeMap::new();
    for (source, records) in prepared.relocations() {
        for r in records {
            if r.kind == 0 {
                continue;
            }
            let target = edge(source.file, r.symbol)?;
            if let Edge::Section { section, .. } = &target {
                if r.kind != 4 && text.contains_key(section) {
                    significant.insert(*section);
                }
            }
            if text.contains_key(source) {
                references.entry(*source).or_default().push(Reference {
                    offset: r.offset,
                    kind: r.kind,
                    addend: r.addend,
                    target,
                });
            }
        }
    }
    for name in keep {
        if let Some(Selection::Definition(id)) = globals.get(*name) {
            if let Definition::Section(section) = tables[id.file][id.symbol].definition {
                significant.insert(SectionId {
                    file: id.file,
                    section,
                });
            }
        }
    }
    let mut unwind = match backend {
        Backend::StaticPie(crate::tls::UnwindPolicy::AbortOnly) => {
            std::collections::BTreeMap::new()
        }
        _ => unwind_descriptions(&prepared)?,
    };
    let candidates: Vec<Candidate<'_>> = text
        .iter()
        .map(|(id, c)| {
            let mut refs = references.remove(id).unwrap_or_default();
            refs.sort_by_key(|r| r.offset);
            Candidate {
                id: *id,
                bytes: c.bytes,
                alignment: c.alignment.max(1),
                references: refs,
                unwind: unwind.remove(id).unwrap_or_default(),
                significant: significant.contains(id),
            }
        })
        .collect();
    let aliases = fold(&candidates);
    let mut sections = Vec::with_capacity(order.len());
    for name in order {
        let invalid = || Error::InvalidOrder(name.to_vec());
        let Some(Selection::Definition(id)) = globals.get(*name) else {
            return Err(invalid());
        };
        let Definition::Section(section) = tables[id.file][id.symbol].definition else {
            return Err(invalid());
        };
        let section = SectionId {
            file: id.file,
            section,
        };
        if !text.contains_key(&section) {
            return Err(invalid());
        }
        sections.push(section);
    }
    let lowered: crate::debug::Lowered = match backend {
        Backend::EtExec => lower(&prepared, &aliases, &sections)?.into(),
        Backend::StaticPie(unwind) => lower_pie(&prepared, &aliases, &sections, unwind)?.into(),
    };
    let mut linked = crate::debug::finish(&prepared, lowered, &aliases)?;
    for line in describe(&aliases) {
        linked.map.push_str(&line);
        linked.map.push('\n');
    }
    Ok(linked)
}

/// Names of folded sections for diagnostics: `file:section` pairs. Supplied.
pub fn describe(aliases: &Aliases) -> Vec<String> {
    aliases
        .iter()
        .map(|(from, to)| {
            format!(
                "FOLD {}:{} {}:{}",
                from.file, from.section, to.file, to.section
            )
        })
        .collect()
}
