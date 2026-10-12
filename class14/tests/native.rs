use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        const {
            assert!(
                cfg!(all(target_os = "linux", target_arch = "x86_64")),
                "native x86-64 Linux required"
            );
        }
        let path = std::env::temp_dir().join(format!(
            "tinylink-class14-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn object(&self, source: &str) -> Vec<u8> {
        let input = self.0.join("input.s");
        let output = self.0.join("input.o");
        fs::write(&input, source).unwrap();
        let p = Command::new("as")
            .arg(&input)
            .arg("-o")
            .arg(&output)
            .output()
            .unwrap();
        assert!(p.status.success(), "{}", String::from_utf8_lossy(&p.stderr));
        fs::read(output).unwrap()
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn archive(scratch: &Scratch, name: &str, members: &[(&str, &[u8])]) -> PathBuf {
    let mut paths = Vec::new();
    for (member, bytes) in members {
        let p = scratch.0.join(member);
        fs::write(&p, bytes).unwrap();
        paths.push(p);
    }
    let out = scratch.0.join(name);
    let p = Command::new("ar")
        .arg("rcs")
        .arg(&out)
        .args(paths)
        .output()
        .unwrap();
    assert!(p.status.success(), "{}", String::from_utf8_lossy(&p.stderr));
    out
}
/// Run the class14 executable with `args`; return its exit success and stderr.
fn driver(args: &[&std::ffi::OsStr]) -> (bool, String) {
    let p = Command::new(env!("CARGO_BIN_EXE_class14"))
        .args(args)
        .output()
        .unwrap();
    (
        p.status.success(),
        String::from_utf8_lossy(&p.stderr).into_owned(),
    )
}
fn os(text: &str) -> &std::ffi::OsStr {
    std::ffi::OsStr::new(text)
}
/// Run a generated program and capture stdout and the exit status.
fn capture(path: &std::path::Path) -> (String, i32) {
    let output = Command::new(path).output().unwrap();
    (
        String::from_utf8_lossy(&output.stdout).into_owned(),
        output.status.code().expect("program died from a signal"),
    )
}
const FIXTURE: &str = include_str!("fixtures/no_std_fmt.rs");
#[test]
fn rustc_links_a_no_std_program() {
    let s = Scratch::new();
    let source = s.0.join("main.rs");
    fs::write(&source, FIXTURE).unwrap();
    let output = s.0.join("main");
    let p = Command::new("rustc")
        .args([
            "--edition",
            "2024",
            "-C",
            "panic=abort",
            "-C",
            "opt-level=2",
        ])
        .args(["-C", "force-unwind-tables=no", "-C", "linker-flavor=ld"])
        .arg("-C")
        .arg(format!("linker={}", env!("CARGO_BIN_EXE_class14")))
        .arg(&source)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();
    assert!(p.status.success(), "{}", String::from_utf8_lossy(&p.stderr));
    for _ in 0..3 {
        assert_eq!(capture(&output), ("sum=42 [2, 3] 1.50\n".to_string(), 42));
    }
    let readelf = Command::new("readelf")
        .arg("-lW")
        .arg(&output)
        .output()
        .unwrap();
    let text = String::from_utf8(readelf.stdout).unwrap();
    assert!(
        text.contains("DYN") && !text.contains("INTERP") && !text.contains("DYNAMIC"),
        "{text}"
    );
}
#[test]
fn libraries_and_response_files_are_used() {
    let s = Scratch::new();
    let start = s.object(
        ".text\n.globl _start\n_start: call answer; mov %eax,%edi; mov $60,%eax; syscall\n",
    );
    let answer = s.object(".text\n.globl answer\nanswer: mov $42,%eax; ret\n");
    let unused = s.object(".text\n.globl unused\nunused: call missing; ret\n");
    fs::write(s.0.join("start.o"), &start).unwrap();
    let lib = s.0.join("lib");
    fs::create_dir(&lib).unwrap();
    let built = archive(
        &s,
        "libanswer.a",
        &[("answer.o", &answer), ("unused.o", &unused)],
    );
    fs::rename(&built, lib.join("libanswer.a")).unwrap();
    let output = s.0.join("program");
    let (ok, stderr) = driver(&[
        os("-pie"),
        os("--gc-sections"),
        s.0.join("start.o").as_os_str(),
        os("-L"),
        s.0.join("missing-dir").as_os_str(),
        os("-L"),
        lib.as_os_str(),
        os("-lanswer"),
        os("-o"),
        output.as_os_str(),
    ]);
    assert!(ok, "{stderr}");
    assert_eq!(capture(&output), (String::new(), 42));
    // The same command line through a response file, which rustc writes when
    // a command line is too long for the operating system.
    let quoted = |p: &std::path::Path| p.to_str().unwrap().replace(' ', "\\ ");
    let response = s.0.join("linker-arguments");
    fs::write(
        &response,
        format!(
            "-pie\n{}\n-L\n{}\n-lanswer\n-o\n{}\n",
            quoted(&s.0.join("start.o")),
            quoted(&lib),
            quoted(&output)
        ),
    )
    .unwrap();
    fs::remove_file(&output).unwrap();
    let mut at = std::ffi::OsString::from("@");
    at.push(&response);
    let (ok, stderr) = driver(&[&at]);
    assert!(ok, "{stderr}");
    assert_eq!(capture(&output), (String::new(), 42));
    let (ok, stderr) = driver(&[
        s.0.join("start.o").as_os_str(),
        os("-lnothing"),
        os("-o"),
        output.as_os_str(),
    ]);
    assert!(!ok && stderr.contains("LibraryNotFound"), "{stderr}");
    let (ok, stderr) = driver(&[
        s.0.join("start.o").as_os_str(),
        os("-shared"),
        os("-o"),
        output.as_os_str(),
    ]);
    assert!(!ok && stderr.contains("UnsupportedOption"), "{stderr}");
    // Without the library the reference stays undefined.
    let (ok, stderr) = driver(&[
        s.0.join("start.o").as_os_str(),
        os("-o"),
        output.as_os_str(),
    ]);
    assert!(!ok && stderr.contains("Undefined"), "{stderr}");
}
#[test]
fn llvm_sections_follow_the_driver_policy() {
    let s = Scratch::new();
    let object = s.object(concat!(
        ".section .text.start,\"ax\",@progbits\n.globl _start\n_start:\n.cfi_startproc\n",
        "lea constant(%rip),%rax; mov (%rax),%edi; add tail(%rip),%edi; mov $60,%eax; syscall\n.cfi_endproc\n",
        ".section .rodata.cst8,\"aM\",@progbits,8\nconstant: .quad 40\n",
        ".section .rodata.str1.1,\"aMS\",@progbits,1\n.asciz \"unused\"\n",
        ".section .data.tail,\"awG\",@progbits,tail,comdat\n.globl tail\ntail: .long 2\n",
    ));
    let path = s.0.join("input.o");
    fs::write(&path, &object).unwrap();
    let output = s.0.join("program");
    let (ok, stderr) = driver(&[path.as_os_str(), os("-o"), output.as_os_str()]);
    assert!(ok, "{stderr}");
    assert_eq!(capture(&output), (String::new(), 42));
    // The class11 policy alone rejects these sections.
    assert!(
        class14::tls::link(&[class14::archive::Input::Object(&object)], b"_start", &[]).is_err()
    );
}

#[test]
fn driver_preserves_cumulative_pie_features() {
    let s = Scratch::new();
    let object = s.object(
        r#"
.section .text.start,"ax",@progbits
.globl _start
_start: call twin_a; mov %eax,%ebx; call twin_b; add %ebx,%eax
mov %eax,%edi; mov $60,%eax; syscall
.section .text.a,"ax",@progbits
.globl twin_a
.type twin_a,@function
twin_a:
.cfi_startproc
lea value_copy@GOTPCREL(%rip),%rax
mov (%rax),%rax
mov (%rax),%eax
add %fs:counter@tpoff,%eax
ret
.cfi_endproc
.section .text.b,"ax",@progbits
.globl twin_b
.type twin_b,@function
twin_b:
.cfi_startproc
lea value_copy@GOTPCREL(%rip),%rax
mov (%rax),%rax
mov (%rax),%eax
add %fs:counter@tpoff,%eax
ret
.cfi_endproc
.section .rodata.values,"aM",@progbits,4
.globl value,value_copy
.type value,@object
.type value_copy,@object
value: .long 20
.size value,4
value_copy: .long 20
.size value_copy,4
.section .tdata,"awT",@progbits
.globl counter
.type counter,@tls_object
counter: .long 1
.size counter,4
.section .debug_probe,"",@progbits
.quad twin_a
.quad twin_b
.quad value
.quad value_copy
"#,
    );
    let linked =
        class14::driver::link(&[class14::archive::Input::Object(&object)], b"_start").unwrap();
    assert_eq!(
        linked
            .map
            .lines()
            .filter(|l| l.starts_with("FOLD "))
            .count(),
        1
    );
    for row in [
        ".got",
        ".tls.image",
        ".rodata.cst4.",
        "DEBUG 32 .debug_probe",
    ] {
        assert!(linked.map.contains(row), "missing {row}: {}", linked.map);
    }
    let input = s.0.join("combined.o");
    let output = s.0.join("program");
    fs::write(&input, &object).unwrap();
    let (ok, stderr) = driver(&[input.as_os_str(), os("-o"), output.as_os_str()]);
    assert!(ok, "{stderr}");
    assert_eq!(fs::read(&output).unwrap(), linked.bytes);
    for _ in 0..3 {
        assert_eq!(capture(&output), (String::new(), 42));
    }
    let u64_at = |at: usize| u64::from_le_bytes(linked.bytes[at..at + 8].try_into().unwrap());
    let u32_at = |at: usize| u32::from_le_bytes(linked.bytes[at..at + 4].try_into().unwrap());
    let u16_at = |at: usize| u16::from_le_bytes(linked.bytes[at..at + 2].try_into().unwrap());
    assert_eq!(u16_at(16), 3);
    assert_eq!(
        u16_at(56),
        5,
        "abort-only output has PT_TLS, no unwind index"
    );
    let table = u64_at(40) as usize;
    let names_header = table + usize::from(u16_at(62)) * 64;
    let names = &linked.bytes[u64_at(names_header + 24) as usize
        ..(u64_at(names_header + 24) + u64_at(names_header + 32)) as usize];
    let named = |name: &[u8]| {
        (0..usize::from(u16_at(60)))
            .map(|i| table + i * 64)
            .find(|&at| names[u32_at(at) as usize..].split(|&b| b == 0).next() == Some(name))
            .unwrap()
    };
    let probe = named(b".debug_probe");
    let at = u64_at(probe + 24) as usize;
    assert_ne!(u64_at(at), 0);
    assert_eq!(
        u64_at(at + 8),
        0,
        "folded code debug reference is tombstoned"
    );
    assert_eq!(
        u64_at(at + 16),
        u64_at(at + 24),
        "duplicate constants share storage"
    );
    assert!(
        u64_at(at) < class14::executable::BASE,
        "debug image values are RVAs"
    );
    let symbols = named(b".symtab");
    let strings = table + u32_at(symbols + 40) as usize * 64;
    let strings = &linked.bytes
        [u64_at(strings + 24) as usize..(u64_at(strings + 24) + u64_at(strings + 32)) as usize];
    let sym = (0..u64_at(symbols + 32) as usize / 24)
        .map(|i| u64_at(symbols + 24) as usize + i * 24)
        .find(|&at| {
            strings[u32_at(at) as usize..].split(|&b| b == 0).next() == Some(&b"counter"[..])
        })
        .unwrap();
    assert_eq!(linked.bytes[sym + 4] & 15, 6);
    assert_eq!(u64_at(sym + 8), 0, "TLS value is a module offset");
    assert_eq!(table + usize::from(u16_at(sym + 6)) * 64, named(b".tdata"));
}

#[test]
fn rust_debug_information_survives_the_driver() {
    let s = Scratch::new();
    let source = s.0.join("main.rs");
    let output = s.0.join("program");
    fs::write(&source, FIXTURE).unwrap();
    let p = Command::new("rustc")
        .args([
            "--edition",
            "2024",
            "-C",
            "panic=abort",
            "-C",
            "opt-level=2",
            "-C",
            "force-unwind-tables=no",
            "-C",
            "debuginfo=2",
            "-C",
            "linker-flavor=ld",
        ])
        .arg("-C")
        .arg(format!("linker={}", env!("CARGO_BIN_EXE_class14")))
        .arg(&source)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();
    assert!(p.status.success(), "{}", String::from_utf8_lossy(&p.stderr));
    assert_eq!(capture(&output), ("sum=42 [2, 3] 1.50\n".into(), 42));
    let p = Command::new("readelf")
        .arg("--debug-dump=info,decodedline")
        .arg(&output)
        .output()
        .unwrap();
    assert!(p.status.success(), "{}", String::from_utf8_lossy(&p.stderr));
    assert!(!String::from_utf8_lossy(&p.stderr)
        .to_lowercase()
        .contains("warning"));
    let p = Command::new("gdb")
        .args([
            "-nx",
            "-batch",
            "-ex",
            "break rust_entry",
            "-ex",
            "run",
            "-ex",
            "info line rust_entry",
            "-ex",
            "continue",
        ])
        .arg(&output)
        .output()
        .unwrap();
    let log = format!(
        "{}\n{}",
        String::from_utf8_lossy(&p.stdout),
        String::from_utf8_lossy(&p.stderr)
    );
    assert!(p.status.success(), "{log}");
    for expected in ["Breakpoint 1, ", "main.rs", "Line ", "exited with code 052"] {
        assert!(log.contains(expected), "missing {expected:?}:\n{log}");
    }
}
