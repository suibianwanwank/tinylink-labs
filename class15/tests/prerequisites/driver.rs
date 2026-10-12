//! Contract tests for command-line parsing, library search, and archives.
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use class15::driver::{Error, InputSpec, Invocation};

#[cfg(unix)]
#[test]
fn non_utf8_paths_are_preserved() {
    use std::os::unix::ffi::OsStringExt;
    let filename = OsString::from_vec(vec![b'-', 0xff, b'.', b'o']);
    let mut words = args(&["-o", "out"]);
    words.push(filename.clone());
    let parsed = class15::driver::parse(&words).unwrap();
    assert_eq!(
        parsed.inputs,
        vec![InputSpec::Path(PathBuf::from(filename))]
    );
}

fn args(words: &[&str]) -> Vec<OsString> {
    words.iter().map(OsString::from).collect()
}
fn path(p: &str) -> InputSpec {
    InputSpec::Path(PathBuf::from(p))
}
fn library(name: &str, verbatim: bool) -> InputSpec {
    InputSpec::Library {
        name: name.to_string(),
        verbatim,
    }
}

/// The command line `rustc -C linker-flavor=ld` produced for a no_std binary.
const RUSTC: &[&str] = &[
    "/tmp/rustc1/symbols.o",
    "main.main.cgu.0.rcgu.o",
    "--as-needed",
    "-Bstatic",
    "/sysroot/lib/libcore-1.rlib",
    "/sysroot/lib/libcompiler_builtins-2.rlib",
    "-L",
    "/tmp/rustc1/raw-dylibs",
    "-Bdynamic",
    "--eh-frame-hdr",
    "-z",
    "noexecstack",
    "-L",
    "/sysroot/lib",
    "-o",
    "main",
    "--gc-sections",
    "-pie",
    "-z",
    "relro",
    "-z",
    "now",
    "-O1",
];

#[test]
fn parse_accepts_rustc_command_lines() {
    assert_eq!(
        class15::driver::parse(&args(RUSTC)),
        Ok(Invocation {
            output: PathBuf::from("main"),
            entry: b"_start".to_vec(),
            inputs: vec![
                path("/tmp/rustc1/symbols.o"),
                path("main.main.cgu.0.rcgu.o"),
                path("/sysroot/lib/libcore-1.rlib"),
                path("/sysroot/lib/libcompiler_builtins-2.rlib"),
            ],
            search: vec![
                PathBuf::from("/tmp/rustc1/raw-dylibs"),
                PathBuf::from("/sysroot/lib")
            ],
        })
    );
    let forms = args(&[
        "--output=out",
        "-e",
        "main",
        "-Ldir1",
        "--library-path=dir2",
        "-lfoo",
        "-l",
        "bar",
        "-l:libexact.a",
        "--library=baz",
        "start.o",
        "--entry=real",
        "-static",
        "--no-as-needed",
        "--pic-executable",
        "-O0",
    ]);
    assert_eq!(
        class15::driver::parse(&forms),
        Ok(Invocation {
            output: PathBuf::from("out"),
            entry: b"real".to_vec(),
            inputs: vec![
                library("foo", false),
                library("bar", false),
                library("libexact.a", true),
                library("baz", false),
                path("start.o"),
            ],
            search: vec![PathBuf::from("dir1"), PathBuf::from("dir2")],
        })
    );
}

#[test]
fn parse_rejects_unsupported_options() {
    let unsupported = |words: &[&str], option: &str| {
        let mut all = args(&["-o", "out", "in.o"]);
        all.extend(args(words));
        assert_eq!(
            class15::driver::parse(&all),
            Err(Error::UnsupportedOption(option.to_string()))
        );
    };
    unsupported(&["-shared"], "-shared");
    unsupported(&["-no-pie"], "-no-pie");
    unsupported(&["--no-gc-sections"], "--no-gc-sections");
    unsupported(&["--dynamic-linker", "/lib/ld.so"], "--dynamic-linker");
    unsupported(&["-z", "execstack"], "-z execstack");
    unsupported(&["-"], "-");
    assert_eq!(
        class15::driver::parse(&args(&["in.o"])),
        Err(Error::InvalidOutput)
    );
    assert_eq!(
        class15::driver::parse(&args(&["-o", "a", "-o", "b", "in.o"])),
        Err(Error::InvalidOutput)
    );
    assert_eq!(
        class15::driver::parse(&args(&["-o", "a"])),
        Err(Error::NoInputs)
    );
    assert_eq!(
        class15::driver::parse(&args(&["in.o", "-o"])),
        Err(Error::MissingArgument("-o".to_string()))
    );
    assert_eq!(
        class15::driver::parse(&args(&["-o", "a", "in.o", "-L"])),
        Err(Error::MissingArgument("-L".to_string()))
    );
}

#[test]
fn libraries_are_searched_in_order() {
    let dirs = [PathBuf::from("first"), PathBuf::from("second")];
    let present = [
        PathBuf::from("second/libfoo.a"),
        PathBuf::from("first/libfoo.a"),
        PathBuf::from("second/libbar.a"),
        PathBuf::from("second/exact.o"),
        PathBuf::from("first/libfoo.so"),
    ];
    let exists = |p: &Path| present.iter().any(|q| q == p);
    assert_eq!(
        class15::driver::find_library("foo", false, &dirs, &exists),
        Ok(PathBuf::from("first/libfoo.a"))
    );
    assert_eq!(
        class15::driver::find_library("bar", false, &dirs, &exists),
        Ok(PathBuf::from("second/libbar.a"))
    );
    assert_eq!(
        class15::driver::find_library("exact.o", true, &dirs, &exists),
        Ok(PathBuf::from("second/exact.o"))
    );
    // Shared libraries are never chosen: this driver only links statically.
    assert_eq!(
        class15::driver::find_library("libfoo.so", false, &dirs, &exists),
        Err(Error::LibraryNotFound("libfoo.so".to_string()))
    );
    assert_eq!(
        class15::driver::find_library("baz", false, &dirs, &exists),
        Err(Error::LibraryNotFound("baz".to_string()))
    );
    assert_eq!(
        class15::driver::find_library("foo", false, &[], &exists),
        Err(Error::LibraryNotFound("foo".to_string()))
    );
}

fn reader(files: &[(&str, &[u8])]) -> impl Fn(&Path) -> Option<Vec<u8>> {
    let files: Vec<(PathBuf, Vec<u8>)> = files
        .iter()
        .map(|(name, bytes)| (PathBuf::from(name), bytes.to_vec()))
        .collect();
    move |path| {
        files
            .iter()
            .find(|(p, _)| p == path)
            .map(|(_, b)| b.clone())
    }
}

#[test]
fn response_files_follow_gnu_quoting() {
    let read = reader(&[
        (
            "args",
            b"-o out\n'a b'  \"c d\"\te\\ f g\\\"h a'b c'd 'x\\'y'\n''\n@inner\nlast\\",
        ),
        ("inner", b"x\r\ny  \"unterminated z"),
    ]);
    let expanded =
        class15::driver::expand(&args(&["first", "@args", "@missing", "@", "tail"]), &read)
            .unwrap();
    assert_eq!(
        expanded,
        args(&[
            "first",
            "-o",
            "out",
            "a b",
            "c d",
            "e f",
            "g\"h",
            "ab cd",
            "x'y",
            "",
            "x",
            "y",
            "unterminated z",
            "last",
            "@missing",
            "@",
            "tail",
        ])
    );
    assert_eq!(class15::driver::expand(&[], &read), Ok(vec![]));
    let empty = reader(&[("empty", b"  \n\t ")]);
    assert_eq!(
        class15::driver::expand(&args(&["a", "@empty", "b"]), &empty),
        Ok(args(&["a", "b"]))
    );
}

#[test]
fn response_files_nest_up_to_sixteen_levels() {
    let mut files: Vec<(String, Vec<u8>)> = (0..16)
        .map(|i| (format!("f{i}"), format!("@f{}", i + 1).into_bytes()))
        .collect();
    files.push(("f16".into(), b"end".to_vec()));
    let borrowed: Vec<(&str, &[u8])> = files
        .iter()
        .map(|(n, b)| (n.as_str(), b.as_slice()))
        .collect();
    let read = reader(&borrowed);
    assert_eq!(
        class15::driver::expand(&args(&["@f1"]), &read),
        Ok(args(&["end"]))
    );
    assert_eq!(
        class15::driver::expand(&args(&["@f0"]), &read),
        Err(Error::ResponseFileDepth)
    );
    let looping = reader(&[("self", b"x @self")]);
    assert_eq!(
        class15::driver::expand(&args(&["@self"]), &looping),
        Err(Error::ResponseFileDepth)
    );
}
