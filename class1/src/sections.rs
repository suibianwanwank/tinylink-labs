//! Parse section metadata using the student's completed class0 boundary.

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
    let _ = (input, header, index);
    todo!("C1.1: decode one section header and validate its metadata")
}

/// SHT_NULL (0) and SHT_NOBITS (8) have no payload here: return an empty slice,
/// without validating their offset against file length. Other kinds use
/// crate::elf_header::checked_slice. NOBITS size describes memory, not stored zero bytes.
pub fn payload<'a>(input: &'a [u8], section: &Section) -> Result<&'a [u8], Error> {
    let _ = (input, section);
    todo!("C1.2: distinguish file-backed data from memory-only storage")
}

/// Return bytes before the first NUL at or after offset. Offset must be strictly
/// inside the table, including for an empty string. Non-UTF-8 names are valid.
pub fn string_at(table: &[u8], offset: u32) -> Result<&[u8], Error> {
    let _ = (table, offset);
    todo!("C1.3: bounded NUL-terminated names without UTF-8 assumptions")
}

/// Parse all sections in table order and pair them with borrowed names.
/// Use crate::elf_header::parse_header, then section, payload, and string_at above.
/// Decode all headers first. The name section must have type SHT_STRTAB (3).
/// Validate its nonempty payload starts and ends with NUL, then each section's
/// payload bounds and name in table order. Fail on the first error.
pub fn sections(input: &[u8]) -> Result<Vec<(Section, &[u8])>, Error> {
    let _ = input;
    todo!("C1.4: compose the input boundary, sections, and names")
}
