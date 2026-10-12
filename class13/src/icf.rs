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
    let _ = candidates;
    todo!("C13.1: refine classes of identical sections to a fixed point")
}

/// Decode supported annotations and associate normalized unwind descriptions
/// with their live Text targets. Reject unsupported records through unwind;
/// README specifies binding, normalization, and deterministic ordering.
pub fn unwind_descriptions(
    prepared: &crate::gc::Prepared<'_>,
) -> Result<std::collections::BTreeMap<SectionId, Vec<UnwindDescription>>, Error> {
    let _ = prepared;
    todo!("C13.2: establish exact runtime unwind descriptions before folding")
}

/// Supplied: reuse unwind's merge/unwind lowering with compatible aliases and
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
/// identities as ET_EXEC; tls owns GOT/TLS/runtime relocation and layout.
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
    let _ = (inputs, entry, keep, order, classify, backend);
    todo!("C13.3: find candidates and address-significant code, fold, order, and finish")
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
