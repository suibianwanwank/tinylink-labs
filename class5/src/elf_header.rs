//! Checked byte ranges and the supported ELF64 relocatable input format.

const ELF_HEADER_SIZE: u64 = 64;
const SECTION_HEADER_SIZE: u64 = 64;
const ELFCLASS64: u8 = 2;
const ELFDATA2LSB: u8 = 1;
const EV_CURRENT: u8 = 1;
const ET_REL: u16 = 1;
const EM_X86_64: u16 = 62;
const SHN_LORESERVE: u16 = 0xff00;

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
    /// Byte offset of the section header table from the start of the file.
    pub section_offset: u64,
    /// Number of entries, including the reserved null entry at index zero.
    pub section_count: u16,
    /// Section index of the section-name string table.
    pub section_names_index: u16,
}

/// Check addition before bounds, so an overflowing range is distinguishable
/// from a representable range outside the file. An empty range at EOF is valid.
pub fn checked_slice(input: &[u8], offset: u64, size: u64) -> Result<&[u8], Error> {
    let end = offset.checked_add(size).ok_or(Error::RangeOverflow)?;
    let start = usize::try_from(offset).map_err(|_| Error::OutOfBounds)?;
    let end = usize::try_from(end).map_err(|_| Error::OutOfBounds)?;
    input.get(start..end).ok_or(Error::OutOfBounds)
}

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

/// Validate the ELF64 little-endian x86-64 ET_REL header in contract order.
/// Only ordinary, nonempty section tables are supported: reserved counts and
/// indices require extended numbering, which this input subset excludes.
pub fn parse_header(input: &[u8]) -> Result<Header, Error> {
    if input.len() < ELF_HEADER_SIZE as usize {
        return Err(Error::Truncated);
    }
    if &input[..4] != b"\x7fELF" {
        return Err(Error::BadMagic);
    }
    if input[4] != ELFCLASS64 {
        return Err(Error::UnsupportedClass);
    }
    if input[5] != ELFDATA2LSB {
        return Err(Error::UnsupportedEndian);
    }
    if input[6] != EV_CURRENT || read_u32(input, 20)? != u32::from(EV_CURRENT) {
        return Err(Error::UnsupportedVersion);
    }
    if read_u16(input, 16)? != ET_REL {
        return Err(Error::UnsupportedType);
    }
    if read_u16(input, 18)? != EM_X86_64 {
        return Err(Error::UnsupportedMachine);
    }
    if u64::from(read_u16(input, 52)?) != ELF_HEADER_SIZE {
        return Err(Error::InvalidHeaderSize);
    }
    if u64::from(read_u16(input, 58)?) != SECTION_HEADER_SIZE {
        return Err(Error::InvalidSectionEntrySize);
    }
    let section_count = read_u16(input, 60)?;
    let section_names_index = read_u16(input, 62)?;
    if !(1..SHN_LORESERVE).contains(&section_count)
        || !(1..section_count).contains(&section_names_index)
    {
        return Err(Error::UnsupportedExtendedNumbering);
    }
    let section_offset = read_u64(input, 40)?;
    // A u16 count times a 64-byte entry cannot overflow u64. The file range can.
    checked_slice(
        input,
        section_offset,
        u64::from(section_count) * SECTION_HEADER_SIZE,
    )?;
    Ok(Header {
        section_offset,
        section_count,
        section_names_index,
    })
}

/// Return one table entry, checking the index before arithmetic and file bounds.
/// Recheck the range even if the header was validated against a different input.
pub fn section_header<'a>(input: &'a [u8], header: &Header, index: u16) -> Result<&'a [u8], Error> {
    if index >= header.section_count {
        return Err(Error::OutOfBounds);
    }
    let offset = header
        .section_offset
        .checked_add(u64::from(index) * SECTION_HEADER_SIZE)
        .ok_or(Error::RangeOverflow)?;
    checked_slice(input, offset, SECTION_HEADER_SIZE)
}
