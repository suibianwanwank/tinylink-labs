use class9::executable::*;
#[test]
fn layout_validation_priority() {
    assert_eq!(layout(0, 3, 0), Err(Error::InvalidAlignment));
    assert_eq!(layout(u64::MAX, 3, u64::MAX), Err(Error::InvalidAlignment));
    assert_eq!(layout(u64::MAX, 1, u64::MAX), Err(Error::InvalidEntry));
    assert_eq!(layout(u64::MAX, 1, 0), Err(Error::TooLarge));
}
fn u16at(b: &[u8], i: usize) -> u16 {
    u16::from_le_bytes(b[i..i + 2].try_into().unwrap())
}
fn u32at(b: &[u8], i: usize) -> u32 {
    u32::from_le_bytes(b[i..i + 4].try_into().unwrap())
}
fn u64at(b: &[u8], i: usize) -> u64 {
    u64::from_le_bytes(b[i..i + 8].try_into().unwrap())
}
#[test]
fn layout_positions() {
    assert_eq!(
        layout(12, 16, 3),
        Ok(Layout {
            code_offset: 176,
            file_size: 188,
            entry_address: BASE + 179
        })
    );
    assert_eq!(
        layout(1, 4096, 0),
        Ok(Layout {
            code_offset: 4096,
            file_size: 4097,
            entry_address: BASE + 4096
        })
    );
    for alignment in [1, 2, 4, 8, 16, 32, 64, 128, 256, 512, 1024, 2048, 4096] {
        let l = layout(51, alignment, 50).unwrap();
        assert_eq!(l.code_offset % alignment, 0);
        assert!(l.code_offset >= HEADER_BYTES);
        assert!(l.code_offset - HEADER_BYTES < alignment);
        assert_eq!(l.file_size, l.code_offset + 51);
    }
}
#[test]
fn layout_rejections() {
    for align in [0, 3, 8192, u64::MAX] {
        assert_eq!(layout(1, align, 0), Err(Error::InvalidAlignment));
    }
    assert_eq!(layout(0, 1, 0), Err(Error::InvalidEntry));
    assert_eq!(layout(10, 1, 10), Err(Error::InvalidEntry));
    assert_eq!(layout(u64::MAX, 16, 0), Err(Error::TooLarge));
    assert_eq!(layout(MAX_IMAGE, 16, 0), Err(Error::TooLarge));
    assert_eq!(
        layout(MAX_IMAGE - HEADER_BYTES, 16, 0).unwrap().file_size,
        MAX_IMAGE
    );
}
#[test]
fn elf_and_program_headers() {
    let code = [0x90, 0xc3];
    let b = executable(&code, 64, 1).unwrap();
    assert_eq!(&b[..16], b"\x7fELF\x02\x01\x01\0\0\0\0\0\0\0\0\0");
    assert_eq!(u16at(&b, 16), 2);
    assert_eq!(u16at(&b, 18), 62);
    assert_eq!(u32at(&b, 20), 1);
    assert_eq!(u64at(&b, 24), BASE + 193);
    assert_eq!(u64at(&b, 32), 64);
    assert_eq!(u64at(&b, 40), 0);
    assert_eq!(u32at(&b, 48), 0);
    assert_eq!(u16at(&b, 52), 64);
    assert_eq!(u16at(&b, 54), 56);
    assert_eq!(u16at(&b, 56), 2);
    assert_eq!(&b[58..64], &[0; 6]);
    assert_eq!(u32at(&b, 64), 1);
    assert_eq!(u32at(&b, 68), 5);
    assert_eq!(u64at(&b, 72), 0);
    assert_eq!(u64at(&b, 80), BASE);
    assert_eq!(u64at(&b, 88), BASE);
    assert_eq!(u64at(&b, 96), 194);
    assert_eq!(u64at(&b, 104), 194);
    assert_eq!(u64at(&b, 112), PAGE);
    assert_eq!(u32at(&b, 120), 0x6474e551);
    assert_eq!(u32at(&b, 124), 6);
    assert_eq!(&b[128..168], &[0; 40]);
    assert_eq!(u64at(&b, 168), 16);
    assert_eq!(&b[176..192], &[0; 16]);
    assert_eq!(&b[192..], &code);
}
#[test]
fn deterministic_output_and_entry_bounds() {
    let code: Vec<_> = (0..255).collect();
    assert_eq!(executable(&code, 32, 0), executable(&code, 32, 0));
    assert_eq!(executable(&code, 16, 255), Err(Error::InvalidEntry));
    assert_eq!(executable(&[], 1, 0), Err(Error::InvalidEntry));
}
