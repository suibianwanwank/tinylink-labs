//! Stage 0: establish the ELF input boundary.
//! Implement the three TODOs. Tests define observable behavior.

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    Truncated,
    BadMagic,
    UnsupportedClass,
    UnsupportedEndian,
    UnsupportedVersion,
    UnsupportedType,
    UnsupportedMachine,
    InvalidHeaderSize,
    InvalidSectionEntrySize,
    UnsupportedExtendedNumbering,
    RangeOverflow,
    OutOfBounds,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Header {
    pub section_offset: u64,
    pub section_count: u16,
    pub section_names_index: u16,
}

/// Return a file slice without integer truncation, overflow, or panic.
/// Addition overflow -> RangeOverflow; a range beyond the input -> OutOfBounds.
/// A zero-sized range at input.len() is valid.
pub fn checked_slice(input: &[u8], offset: u64, size: u64) -> Result<&[u8], Error> {
    let _ = (input, offset, size);
    todo!("C0.1: validate the range before converting indices to usize")
}

/// Decode the 64-byte ELF64 little-endian relocatable x86-64 header.
/// Validate in this order: length, magic, class, endian, identification version,
/// e_version, e_type, e_machine, e_ehsize, e_shentsize, numbering, table range.
/// This course stage requires an ordinary, nonempty section table (no extended
/// numbering): 0 < e_shnum < 0xff00; 0 < e_shstrndx < e_shnum.
/// The section headers themselves are parsed in class1.
pub fn parse_header(input: &[u8]) -> Result<Header, Error> {
    let _ = input;
    todo!("C0.2: decode and validate the supported input format")
}

/// Fetch one complete section-header entry using the already validated header.
/// An index outside [0, section_count) returns OutOfBounds.
/// Still check the bytes: callers may provide a different or truncated input.
pub fn section_header<'a>(input: &'a [u8], header: &Header, index: u16) -> Result<&'a [u8], Error> {
    let _ = (input, header, index);
    todo!("C0.3: checked table indexing; each ELF64 entry occupies 64 bytes")
}

// Supplied integer readers; their byte-range checks use C0.1.
/// Decode exactly eight little-endian bytes.
pub fn u64_from_le_bytes(input: &[u8]) -> Result<u64, Error> {
    let bytes = input.try_into().map_err(|_| Error::Truncated)?;
    Ok(u64::from_le_bytes(bytes))
}

/// Decode exactly four little-endian bytes.
pub fn u32_from_le_bytes(input: &[u8]) -> Result<u32, Error> {
    let bytes = input.try_into().map_err(|_| Error::Truncated)?;
    Ok(u32::from_le_bytes(bytes))
}

/// Read a little-endian integer at a checked file offset.
pub fn read_u16(input: &[u8], offset: u64) -> Result<u16, Error> {
    let bytes = checked_slice(input, offset, 2)?;
    Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
}

/// Read a little-endian integer at a checked file offset.
pub fn read_u32(input: &[u8], offset: u64) -> Result<u32, Error> {
    u32_from_le_bytes(checked_slice(input, offset, 4)?)
}

/// Read a little-endian integer at a checked file offset.
pub fn read_u64(input: &[u8], offset: u64) -> Result<u64, Error> {
    u64_from_le_bytes(checked_slice(input, offset, 8)?)
}
