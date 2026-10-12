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
    if input.starts_with(b"!<thin>\n") {
        return Err(Error::ThinArchive);
    }
    if !input.starts_with(b"!<arch>\n") {
        return Err(Error::BadMagic);
    }
    let number = |field: &[u8]| -> Result<usize, Error> {
        let end = field
            .iter()
            .rposition(|b| *b != b' ')
            .map(|i| i + 1)
            .unwrap_or(0);
        let digits = &field[..end];
        if digits.is_empty() || !digits.iter().all(u8::is_ascii_digit) {
            return Err(Error::InvalidSize);
        }
        digits.iter().try_fold(0usize, |n, b| {
            n.checked_mul(10)
                .and_then(|n| n.checked_add((b - b'0') as usize))
                .ok_or(Error::InvalidSize)
        })
    };
    let mut offset = 8usize;
    let mut entries = Vec::new();
    let mut names = None;
    while offset < input.len() {
        let end = offset.checked_add(60).ok_or(Error::TruncatedHeader)?;
        let h = input.get(offset..end).ok_or(Error::TruncatedHeader)?;
        if &h[58..] != b"`\n" {
            return Err(Error::InvalidHeader);
        }
        let size = number(&h[48..58])?;
        let payload_end = end.checked_add(size).ok_or(Error::TruncatedMember)?;
        let bytes = input.get(end..payload_end).ok_or(Error::TruncatedMember)?;
        let next = payload_end
            .checked_add(size % 2)
            .ok_or(Error::InvalidPadding)?;
        if size % 2 == 1 && input.get(payload_end) != Some(&b'\n') {
            return Err(Error::InvalidPadding);
        }
        let name_end = h[..16]
            .iter()
            .rposition(|b| *b != b' ')
            .map(|i| i + 1)
            .unwrap_or(0);
        let name = &h[..name_end];
        if name == b"//" {
            if names.replace(bytes).is_some() {
                return Err(Error::DuplicateNameTable);
            }
        } else if name != b"/" && name != b"/SYM64/" {
            entries.push((name, bytes, offset as u64));
        }
        offset = next;
    }
    entries
        .into_iter()
        .map(|(raw, bytes, header_offset)| {
            let name = if raw.starts_with(b"#1/") {
                return Err(Error::UnsupportedNameFormat);
            } else if raw.starts_with(b"/") {
                let start = number(&raw[1..]).map_err(|_| Error::InvalidName)?;
                let table = names.ok_or(Error::InvalidName)?;
                if start >= table.len()
                    || (start != 0 && (start < 2 || &table[start - 2..start] != b"/\n"))
                {
                    return Err(Error::InvalidName);
                }
                let tail = &table[start..];
                let end = tail
                    .windows(2)
                    .position(|w| w == b"/\n")
                    .ok_or(Error::InvalidName)?;
                if end == 0 {
                    return Err(Error::InvalidName);
                }
                &tail[..end]
            } else {
                if raw.len() < 2 || raw.last() != Some(&b'/') {
                    return Err(Error::InvalidName);
                }
                &raw[..raw.len() - 1]
            };
            Ok(Member {
                name,
                bytes,
                header_offset,
            })
        })
        .collect()
}

/// Select object payloads without copying them. Append explicit objects in input
/// order. At each archive, parse member symbol tables (one SYMTAB per object),
/// then repeatedly scan unselected members in archive order. Extract a member
/// if it defines a currently unresolved GLOBAL name. WEAK undefined symbols do
/// not request extraction. Definitions may be GLOBAL/WEAK/COMMON. Existing
/// selected definitions satisfy requests; COMMON alone does not request upgrade.
/// After each extraction recompute requests before examining the next member.
/// Revisit this archive until no progress, never revisit earlier archives.
/// Parse all member symbol tables even if unused; class6 storage policy applies
/// only to selected members. Validate selected symbols through class3 each time.
/// See README for the deliberate scope of this deterministic archive policy.
pub fn select<'a>(inputs: &[Input<'a>]) -> Result<Vec<&'a [u8]>, Error> {
    use std::collections::BTreeSet;
    fn table(bytes: &[u8]) -> Result<Vec<crate::symbols::Symbol<'_>>, Error> {
        let sections: Vec<_> = crate::sections::sections(bytes)?
            .into_iter()
            .map(|(s, _)| s)
            .collect();
        let indices: Vec<_> = sections
            .iter()
            .enumerate()
            .filter(|(_, s)| s.kind == 2)
            .map(|(i, _)| i)
            .collect();
        if indices.len() != 1 {
            return Err(Error::SymbolTableCount);
        }
        Ok(crate::symbols::symbols(bytes, &sections, indices[0])?)
    }
    let mut selected = Vec::new();
    let mut tables = Vec::new();
    for input in inputs {
        match *input {
            Input::Object(bytes) => {
                tables.push(table(bytes)?);
                crate::resolve::build_globals(&tables)?;
                selected.push(bytes);
            }
            Input::Archive(bytes) => {
                let members = members(bytes)?;
                let candidates: Vec<_> = members
                    .iter()
                    .map(|m| table(m.bytes))
                    .collect::<Result<_, _>>()?;
                let mut extracted = vec![false; members.len()];
                loop {
                    let mut progress = false;
                    for (i, candidate) in candidates.iter().enumerate() {
                        if extracted[i] {
                            continue;
                        }
                        let globals = crate::resolve::build_globals(&tables)?;
                        let wanted: BTreeSet<&[u8]> = tables
                            .iter()
                            .flatten()
                            .filter(|s| {
                                s.binding == 1
                                    && s.definition == crate::symbols::Definition::Undefined
                                    && !globals.contains_key(s.name)
                            })
                            .map(|s| s.name)
                            .collect();
                        if candidate.iter().any(|s| {
                            (s.binding == 1 || s.binding == 2)
                                && s.definition != crate::symbols::Definition::Undefined
                                && wanted.contains(s.name)
                        }) {
                            tables.push(table(members[i].bytes)?);
                            crate::resolve::build_globals(&tables)?;
                            selected.push(members[i].bytes);
                            extracted[i] = true;
                            progress = true;
                        }
                    }
                    if !progress {
                        break;
                    }
                }
            }
        }
    }
    Ok(selected)
}

/// Use select(), then the cumulative class6 linker. The entry is resolved after
/// selection and does not itself force archive extraction. Do not invoke ar/ld.
pub fn link(inputs: &[Input<'_>], entry: &[u8]) -> Result<Vec<u8>, Error> {
    let selected = select(inputs)?;
    Ok(crate::layout::link(&selected, entry)?)
}
