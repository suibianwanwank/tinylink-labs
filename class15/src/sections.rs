//! Decode section metadata and borrow names and payloads from the input file.

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    Input(crate::elf_header::Error),
    InvalidAlignment,
    InvalidEntrySize,
    InvalidNameTable,
    BadStringOffset,
    UnterminatedString,
}
impl From<crate::elf_header::Error> for Error {
    fn from(error: crate::elf_header::Error) -> Self {
        Self::Input(error)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Section {
    pub name_offset: u32,
    pub kind: u32,
    pub flags: u64,
    pub offset: u64,
    pub size: u64,
    pub link: u32,
    pub info: u32,
    pub alignment: u64,
    pub entry_size: u64,
}

/// Decode one 64-byte section header obtained through crate::elf_header::section_header.
/// Alignment must be zero or a power of two. A nonzero entry_size must divide
/// size. Validate alignment before entry size. Ignore sh_addr at this stage.
pub fn section(
    input: &[u8],
    header: &crate::elf_header::Header,
    index: u16,
) -> Result<Section, Error> {
    let b = crate::elf_header::section_header(input, header, index)?;
    let u32at = |offset| crate::elf_header::read_u32(b, offset);
    let u64at = |offset| crate::elf_header::read_u64(b, offset);
    let alignment = u64at(48)?;
    let size = u64at(32)?;
    let entry_size = u64at(56)?;
    if alignment != 0 && !alignment.is_power_of_two() {
        return Err(Error::InvalidAlignment);
    }
    if entry_size != 0 && size % entry_size != 0 {
        return Err(Error::InvalidEntrySize);
    }
    Ok(Section {
        name_offset: u32at(0)?,
        kind: u32at(4)?,
        flags: u64at(8)?,
        offset: u64at(24)?,
        size,
        link: u32at(40)?,
        info: u32at(44)?,
        alignment,
        entry_size,
    })
}

/// SHT_NULL (0) and SHT_NOBITS (8) have no payload here: return an empty slice,
/// without validating their offset against file length. Other kinds use
/// crate::elf_header::checked_slice. NOBITS size describes memory, not stored zero bytes.
pub fn payload<'a>(input: &'a [u8], section: &Section) -> Result<&'a [u8], Error> {
    if section.kind == 0 || section.kind == 8 {
        return Ok(&[]);
    }
    Ok(crate::elf_header::checked_slice(
        input,
        section.offset,
        section.size,
    )?)
}

/// Return bytes before the first NUL at or after offset. Offset must be strictly
/// inside the table, including for an empty string. Non-UTF-8 names are valid.
pub fn string_at(table: &[u8], offset: u32) -> Result<&[u8], Error> {
    let start = usize::try_from(offset).map_err(|_| Error::BadStringOffset)?;
    if start >= table.len() {
        return Err(Error::BadStringOffset);
    }
    let tail = &table[start..];
    let end = tail
        .iter()
        .position(|b| *b == 0)
        .ok_or(Error::UnterminatedString)?;
    Ok(&tail[..end])
}

/// Parse all sections in table order and pair them with borrowed names.
/// Use crate::elf_header::parse_header, then section, payload, and string_at above.
/// Decode all headers first. The name section must have type SHT_STRTAB (3).
/// Validate its nonempty payload starts and ends with NUL, then each section's
/// payload bounds and name in table order. Fail on the first error.
pub fn sections(input: &[u8]) -> Result<Vec<(Section, &[u8])>, Error> {
    let h = crate::elf_header::parse_header(input)?;
    let entries = (0..h.section_count)
        .map(|i| section(input, &h, i))
        .collect::<Result<Vec<_>, _>>()?;
    let names = &entries[usize::from(h.section_names_index)];
    if names.kind != 3 {
        return Err(Error::InvalidNameTable);
    }
    let table = payload(input, names)?;
    if table.first() != Some(&0) || table.last() != Some(&0) {
        return Err(Error::InvalidNameTable);
    }
    entries
        .into_iter()
        .map(|s| {
            payload(input, &s)?;
            let name = string_at(table, s.name_offset)?;
            Ok((s, name))
        })
        .collect()
}
