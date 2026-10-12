use class9::archive::*;
#[test]
fn framing_precedes_name_resolution() {
    let mut ar = b"!<arch>\n".to_vec();
    append(&mut ar, b"bad-name", b"ab");
    let second = ar.len();
    append(&mut ar, b"valid/", b"cd");
    ar[second + 58] = b'?';
    assert_eq!(members(&ar), Err(Error::InvalidHeader));
    ar[second + 58] = b'`';
    assert_eq!(members(&ar), Err(Error::InvalidName));
}
fn append(ar: &mut Vec<u8>, name: &[u8], payload: &[u8]) {
    let mut h = [b' '; 60];
    h[..name.len()].copy_from_slice(name);
    let n = payload.len().to_string();
    h[48..48 + n.len()].copy_from_slice(n.as_bytes());
    h[58..].copy_from_slice(b"`\n");
    ar.extend_from_slice(&h);
    ar.extend_from_slice(payload);
    if payload.len() % 2 == 1 {
        ar.push(b'\n');
    }
}
#[test]
fn short_names_offsets_and_padding() {
    let mut ar = b"!<arch>\n".to_vec();
    append(&mut ar, b"one.o/", b"abc");
    append(&mut ar, b"one.o/", b"xy");
    assert_eq!(
        members(&ar),
        Ok(vec![
            Member {
                name: b"one.o",
                bytes: b"abc",
                header_offset: 8
            },
            Member {
                name: b"one.o",
                bytes: b"xy",
                header_offset: 72
            }
        ])
    );
    assert_eq!(members(b"!<arch>\n"), Ok(vec![]));
}
#[test]
fn long_names_and_opaque_metadata() {
    let mut ar = b"!<arch>\n".to_vec();
    append(&mut ar, b"/", b"opaque-index");
    append(&mut ar, b"/0", b"object");
    append(&mut ar, b"//", b"long object name.o/\n\xff.o/\n");
    append(&mut ar, b"/20", b"second");
    append(&mut ar, b"/SYM64/", b"unused-index");
    let m = members(&ar).unwrap();
    assert_eq!(m.len(), 2);
    assert_eq!(m[0].name, b"long object name.o");
    assert_eq!(m[1].name, b"\xff.o");
}
#[test]
fn malformed_framing() {
    assert_eq!(members(b"!<thin>\n"), Err(Error::ThinArchive));
    assert_eq!(members(b"garbage"), Err(Error::BadMagic));
    let mut ar = b"!<arch>\n".to_vec();
    append(&mut ar, b"a/", b"abc");
    for end in 9..68 {
        assert_eq!(members(&ar[..end]), Err(Error::TruncatedHeader));
    }
    for end in 68..71 {
        assert_eq!(members(&ar[..end]), Err(Error::TruncatedMember));
    }
    assert_eq!(members(&ar[..71]), Err(Error::InvalidPadding));
    let mut bad = ar.clone();
    bad[66] = b'?';
    assert_eq!(members(&bad), Err(Error::InvalidHeader));
    let mut bad = ar.clone();
    bad[56] = b'-';
    assert_eq!(members(&bad), Err(Error::InvalidSize));
    let mut bad = ar;
    bad[71] = 0;
    assert_eq!(members(&bad), Err(Error::InvalidPadding));
}
#[test]
fn malformed_names() {
    for (name, expected) in [
        (b"/0".as_slice(), Error::InvalidName),
        (b"#1/4".as_slice(), Error::UnsupportedNameFormat),
        (b"bad".as_slice(), Error::InvalidName),
    ] {
        let mut ar = b"!<arch>\n".to_vec();
        append(&mut ar, name, b"abcd");
        assert_eq!(members(&ar), Err(expected));
    }
    let mut ar = b"!<arch>\n".to_vec();
    append(&mut ar, b"//", b"long.o/\n");
    append(&mut ar, b"/1", b"x");
    assert_eq!(members(&ar), Err(Error::InvalidName));
    let mut ar = b"!<arch>\n".to_vec();
    append(&mut ar, b"//", b"x/\n");
    append(&mut ar, b"//", b"y/\n");
    assert_eq!(members(&ar), Err(Error::DuplicateNameTable));
}
