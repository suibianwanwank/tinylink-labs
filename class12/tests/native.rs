use std::{
    fs,
    path::PathBuf,
    process::{Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
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
            "tinylink-class12-{}-{}",
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
fn execute(path: &std::path::Path) -> i32 {
    // A concurrent fixture writer can briefly leave its writable descriptor in
    // a forked child before exec closes it. Retry only Linux ETXTBSY at spawn;
    // execution failures and all other spawn errors remain fatal.
    let spawn_deadline = Instant::now() + Duration::from_secs(3);
    let mut child = loop {
        match Command::new(path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            Ok(child) => break child,
            Err(e) if e.raw_os_error() == Some(26) && Instant::now() < spawn_deadline => {
                std::thread::sleep(Duration::from_millis(5));
            }
            Err(e) => panic!("cannot execute fixture: {e}"),
        }
    };
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            return status.code().expect("generated program died from a signal");
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("generated program timed out");
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}
fn run_image(scratch: &Scratch, image: &[u8]) -> i32 {
    use std::os::unix::fs::PermissionsExt;
    let p = scratch.0.join("program");
    fs::write(&p, image).unwrap();
    fs::set_permissions(&p, fs::Permissions::from_mode(0o755)).unwrap();
    execute(&p)
}
use class12::archive::Input;
fn archive(scratch: &Scratch, entries: &[(&str, &str)]) -> Vec<u8> {
    let mut paths = Vec::new();
    for (name, source) in entries {
        let b = scratch.object(source);
        let p = scratch.0.join(name);
        fs::write(&p, b).unwrap();
        paths.push(p);
    }
    let out = scratch.0.join("library.a");
    let _ = fs::remove_file(&out);
    let p = Command::new("ar")
        .arg("rcs")
        .arg(&out)
        .args(paths)
        .output()
        .unwrap();
    assert!(p.status.success(), "{}", String::from_utf8_lossy(&p.stderr));
    fs::read(out).unwrap()
}
/// Compile freestanding C with DWARF and without unwind tables.
fn compile(s: &Scratch, name: &str, source: &str, extra: &[&str]) -> Vec<u8> {
    let input = s.0.join(format!("{name}.c"));
    let output = s.0.join(format!("{name}.o"));
    fs::write(&input, source).unwrap();
    let p = Command::new("cc")
        .args([
            "-c",
            "-Og",
            "-g",
            "-ffreestanding",
            "-fno-tree-loop-distribute-patterns",
            "-ffunction-sections",
            "-fdata-sections",
            "-fno-pie",
            "-fno-stack-protector",
            "-fcf-protection=none",
        ])
        .args(["-fno-asynchronous-unwind-tables", "-fno-unwind-tables"])
        .args(extra)
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();
    assert!(p.status.success(), "{}", String::from_utf8_lossy(&p.stderr));
    fs::read(output).unwrap()
}
const START: &str = ".section .text.start,\"ax\",@progbits\n.globl _start\n_start: call check; mov %eax,%edi; mov $60,%eax; syscall\n";
const DEBUGGEE: &str = r#"int counter = 3;
static int helper_state;
__attribute__((noipa)) int leaf(int x) {
    int doubled = x * 2;
    helper_state += doubled;
    return doubled + counter;
}
__attribute__((noipa)) int unused(int y) { return y - 1; }
int check(void) { return leaf(19) + 1; }
"#;
/// Objects for the debuggee, written to disk for GNU ld and the CLI.
fn debuggee(s: &Scratch) -> (Vec<u8>, Vec<u8>) {
    let start = s.object(START);
    let program = compile(s, "debuggee", DEBUGGEE, &[]);
    fs::write(s.0.join("start.o"), &start).unwrap();
    fs::write(s.0.join("debuggee.o"), &program).unwrap();
    (start, program)
}
fn tool(program: &str, args: &[&std::ffi::OsStr]) -> (String, String) {
    let p = Command::new(program).args(args).output().unwrap();
    assert!(
        p.status.success(),
        "{program} ({:?}): stdout:\n{}\nstderr:\n{}",
        p.status,
        String::from_utf8_lossy(&p.stdout),
        String::from_utf8_lossy(&p.stderr)
    );
    (
        String::from_utf8_lossy(&p.stdout).into_owned(),
        String::from_utf8_lossy(&p.stderr).into_owned(),
    )
}
fn os(text: &str) -> &std::ffi::OsStr {
    std::ffi::OsStr::new(text)
}
/// Write `image` to `<scratch>/program` and run a fixed GDB session on it.
fn gdb_session(s: &Scratch, image: &[u8]) -> String {
    use std::os::unix::fs::PermissionsExt;
    let path = s.0.join("program");
    fs::write(&path, image).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    let script = [
        "break leaf",
        "run",
        "print x",
        "print counter",
        "info line *$pc",
        "backtrace",
        "finish",
        "continue",
    ];
    let mut args = vec![os("-nx"), os("-batch")];
    for command in &script {
        args.extend([os("-ex"), os(command)]);
    }
    args.push(path.as_os_str());
    tool("gdb", &args).0
}
/// Remove what legitimately differs between two linkers' outputs: addresses
/// and process IDs.
fn normalize(text: &str) -> String {
    text.split_whitespace()
        .map(|word| {
            if let Some(at) = word.find("0x") {
                return format!("{}ADDR", &word[..at]);
            }
            let digits = word.trim_end_matches(|c: char| !c.is_ascii_digit());
            if digits.len() > 3 && digits.chars().all(|c| c.is_ascii_digit()) {
                return format!("PID{}", &word[digits.len()..]);
            }
            word.to_string()
        })
        .collect::<Vec<_>>()
        .join(" ")
}
#[test]
fn gdb_sees_functions_lines_and_variables() {
    let s = Scratch::new();
    let (start, program) = debuggee(&s);
    let linked = class12::debug::link(
        &[Input::Object(&start), Input::Object(&program)],
        b"_start",
        &[],
    )
    .unwrap();
    assert_eq!(run_image(&s, &linked.bytes), 42);
    let ours = gdb_session(&s, &linked.bytes);
    for expected in [
        "Breakpoint 1, leaf (x=x@entry=19) at",
        "debuggee.c:",
        "$1 = 19",
        "$2 = 3",
        "in check () at",
        "Value returned is $3 = 41",
        "exited with code 052",
    ] {
        assert!(ours.contains(expected), "missing {expected:?} in\n{ours}");
    }
    let reference = s.0.join("reference");
    let p = Command::new("ld")
        .args(["-static", "--gc-sections", "-e", "_start", "-o"])
        .arg(&reference)
        .arg(s.0.join("start.o"))
        .arg(s.0.join("debuggee.o"))
        .output()
        .unwrap();
    assert!(p.status.success(), "{}", String::from_utf8_lossy(&p.stderr));
    let gnu = gdb_session(&s, &fs::read(&reference).unwrap());
    assert_eq!(
        normalize(&ours),
        normalize(&gnu),
        "ours:\n{ours}\nGNU ld:\n{gnu}"
    );
}
#[test]
fn multiple_compilation_units_keep_independent_debug_coordinates() {
    let s = Scratch::new();
    let start = s.object(START);
    fs::write(s.0.join("start.o"), &start).unwrap();
    let caller = compile(
        &s,
        "caller",
        "extern int leaf(int);\nint check(void) { return leaf(19) + 1; }\n",
        &[],
    );
    let callee = compile(
        &s,
        "callee",
        "int counter = 3;\n__attribute__((noipa)) int leaf(int x) {\n    int doubled = x * 2;\n    return doubled + counter;\n}\n",
        &[],
    );
    // Each permutation puts a different CU after nonzero debug contributions.
    // A single-debug-input fixture cannot detect a missing contribution offset.
    for reverse in [false, true] {
        let objects = if reverse {
            [&callee, &caller]
        } else {
            [&caller, &callee]
        };
        let linked = class12::debug::link(
            &[
                Input::Object(&start),
                Input::Object(objects[0]),
                Input::Object(objects[1]),
            ],
            b"_start",
            &[],
        )
        .unwrap();
        assert_eq!(run_image(&s, &linked.bytes), 42);
        let ours = gdb_session(&s, &linked.bytes);
        let (_, warnings) = tool(
            "readelf",
            &[os("--debug-dump"), s.0.join("program").as_os_str()],
        );
        assert!(!warnings.to_lowercase().contains("warning"), "{warnings}");
        for expected in [
            "Breakpoint 1, leaf (x=x@entry=19) at",
            "callee.c:",
            "caller.c:",
            "$1 = 19",
            "$2 = 3",
            "in check () at",
            "Value returned is $3 = 41",
            "exited with code 052",
        ] {
            assert!(ours.contains(expected), "missing {expected:?} in\n{ours}");
        }
        let reference = s.0.join("reference");
        let names = if reverse {
            ["callee.o", "caller.o"]
        } else {
            ["caller.o", "callee.o"]
        };
        let p = Command::new("ld")
            .args(["-static", "--gc-sections", "-e", "_start", "-o"])
            .arg(&reference)
            .arg(s.0.join("start.o"))
            .args(names.map(|name| s.0.join(name)))
            .output()
            .unwrap();
        assert!(p.status.success(), "{}", String::from_utf8_lossy(&p.stderr));
        let gnu = gdb_session(&s, &fs::read(&reference).unwrap());
        assert_eq!(normalize(&ours), normalize(&gnu), "input order: {names:?}");
    }
}

#[test]
fn symbols_and_line_tables_agree_with_gnu_ld() {
    let s = Scratch::new();
    let (start, program) = debuggee(&s);
    let linked = class12::debug::link(
        &[Input::Object(&start), Input::Object(&program)],
        b"_start",
        &[],
    )
    .unwrap();
    let ours = s.0.join("ours");
    fs::write(&ours, &linked.bytes).unwrap();
    let reference = s.0.join("reference");
    let p = Command::new("ld")
        .args(["-static", "--gc-sections", "-e", "_start", "-o"])
        .arg(&reference)
        .arg(s.0.join("start.o"))
        .arg(s.0.join("debuggee.o"))
        .output()
        .unwrap();
    assert!(p.status.success());
    let address = |path: &std::path::Path, name: &str| -> String {
        let (nm, _) = tool("nm", &[path.as_os_str()]);
        let line = nm
            .lines()
            .find(|l| l.split_whitespace().nth(2) == Some(name))
            .unwrap_or_else(|| panic!("{name} missing from\n{nm}"));
        line.split_whitespace().next().unwrap().to_string()
    };
    let lookup = |path: &std::path::Path, name: &str| {
        let at = address(path, name);
        tool(
            "addr2line",
            &[os("-f"), os("-e"), path.as_os_str(), os(&at)],
        )
        .0
    };
    for name in ["_start", "counter", "helper_state"] {
        address(&ours, name);
    }
    for function in ["leaf", "check"] {
        let ours_line = lookup(&ours, function);
        assert!(
            ours_line.starts_with(&format!("{function}\n")) && ours_line.contains("debuggee.c:"),
            "{ours_line}"
        );
        assert_eq!(ours_line, lookup(&reference, function));
    }
    assert!(!tool("nm", &[ours.as_os_str()]).0.contains(" unused"));
    let (headers, _) = tool("readelf", &[os("-SW"), ours.as_os_str()]);
    for name in [
        ".text",
        ".data",
        ".bss",
        ".debug_info",
        ".debug_line",
        ".debug_str",
        ".symtab",
        ".strtab",
        ".shstrtab",
    ] {
        assert!(
            headers.contains(&format!(" {name} ")),
            "{name} missing in\n{headers}"
        );
    }
    let (_, warnings) = tool("readelf", &[os("--debug-dump"), ours.as_os_str()]);
    assert!(!warnings.to_lowercase().contains("warning"), "{warnings}");
    for name in [".debug_info", ".debug_line", ".debug_str"] {
        assert!(
            linked
                .map
                .lines()
                .any(|l| l.starts_with("DEBUG ") && l.ends_with(&format!(" {name}"))),
            "{}",
            linked.map
        );
    }
}
#[test]
fn discarded_code_receives_tombstones() {
    let s = Scratch::new();
    let (start, program) = debuggee(&s);
    let linked = class12::debug::link(
        &[Input::Object(&start), Input::Object(&program)],
        b"_start",
        &[],
    )
    .unwrap();
    let path = s.0.join("ours");
    fs::write(&path, &linked.bytes).unwrap();
    let (info, _) = tool("readelf", &[os("--debug-dump=info"), path.as_os_str()]);
    // The DIE for `unused` survives, but its address is the tombstone 0.
    let die = info
        .split("DW_TAG_subprogram")
        .find(|d| d.contains(": unused"))
        .expect("subprogram DIE for unused");
    let low_pc = die
        .lines()
        .find(|l| l.contains("DW_AT_low_pc"))
        .expect("low_pc of unused");
    let value = |line: &str| line.rsplit(':').next().unwrap().trim().to_string();
    assert!(matches!(value(low_pc).as_str(), "0" | "0x0"), "{low_pc}");
    let kept = info
        .split("DW_TAG_subprogram")
        .find(|d| d.contains(": leaf"))
        .unwrap();
    let kept_pc = kept.lines().find(|l| l.contains("DW_AT_low_pc")).unwrap();
    assert!(!matches!(value(kept_pc).as_str(), "0" | "0x0"), "{kept_pc}");
    // With the function kept, it gets a real address instead.
    let kept = class12::debug::link(
        &[Input::Object(&start), Input::Object(&program)],
        b"_start",
        &[b"unused"],
    )
    .unwrap();
    fs::write(&path, &kept.bytes).unwrap();
    let (nm, _) = tool("nm", &[path.as_os_str()]);
    assert!(nm.contains(" T unused"), "{nm}");
}
#[test]
fn appending_sections_preserves_the_class8_image() {
    let s = Scratch::new();
    let (start, program) = debuggee(&s);
    let library = archive(
        &s,
        &[(
            "extra.o",
            ".text\n.globl extra\nextra: ret\n.comm shared,8,8\n",
        )],
    );
    let inputs = [
        Input::Object(&start),
        Input::Object(&program),
        Input::Archive(&library),
    ];
    let ours = class12::debug::link(&inputs, b"_start", &[]).unwrap();
    let class8 = class12::gc::link(&inputs, b"_start", &[]).unwrap();
    let image = &ours.bytes[..class8.bytes.len()];
    for (at, (a, b)) in image.iter().zip(&class8.bytes).enumerate() {
        if !(40..48).contains(&at) && !(58..64).contains(&at) {
            assert_eq!(a, b, "byte {at} differs");
        }
    }
    assert!(ours.map.starts_with(&class8.map));
    assert!(ours.map[class8.map.len()..]
        .lines()
        .all(|l| l.starts_with("DEBUG ")));
    // Without debug input, only the symbol table and headers are appended.
    let plain = s.object(".text\n.globl _start\n_start: mov $42,%edi; mov $60,%eax; syscall\n");
    let linked = class12::debug::link(&[Input::Object(&plain)], b"_start", &[]).unwrap();
    assert_eq!(run_image(&s, &linked.bytes), 42);
    assert!(!linked.map.contains("DEBUG "));
    let path = s.0.join("plain");
    fs::write(&path, &linked.bytes).unwrap();
    assert!(tool("nm", &[path.as_os_str()]).0.contains(" T _start"));
}
#[test]
fn rejects_unsupported_debug_inputs() {
    use class12::debug::{DebugError, Error};
    let s = Scratch::new();
    let start = s.object(START);
    let compressed = compile(&s, "compressed", DEBUGGEE, &["-gz=zlib"]);
    assert!(matches!(
        class12::debug::link(
            &[Input::Object(&start), Input::Object(&compressed)],
            b"_start",
            &[]
        ),
        Err(Error::UnsupportedDebug(_))
    ));
    let pc_relative = s.object(".text\n.globl _start\n_start: ret\n.section .debug_info,\"\",@progbits\n.reloc ., R_X86_64_PC32, _start\n.long 0\n");
    assert!(matches!(
        class12::debug::link(&[Input::Object(&pc_relative)], b"_start", &[]),
        Err(Error::Debug {
            kind: 2,
            cause: DebugError::Unsupported(2),
            ..
        })
    ));
    let comment = s.object(".text\n.globl _start\n_start: ret\n.section .comment,\"\",@progbits\nnote: .byte 1\n.section .debug_info,\"\",@progbits\n.long note\n");
    assert!(matches!(
        class12::debug::link(&[Input::Object(&comment)], b"_start", &[]),
        Err(Error::InvalidDebugReference { offset: 0, .. })
    ));
}
#[test]
fn cli_output_is_debuggable() {
    let s = Scratch::new();
    debuggee(&s);
    let output = s.0.join("program");
    let map = s.0.join("program.map");
    let p = Command::new(env!("CARGO_BIN_EXE_class12"))
        .arg("-o")
        .arg(&output)
        .arg("-Map")
        .arg(&map)
        .args(["start.o", "debuggee.o"].map(|n| s.0.join(n)))
        .output()
        .unwrap();
    assert!(p.status.success(), "{}", String::from_utf8_lossy(&p.stderr));
    assert_eq!(execute(&output), 42);
    assert!(fs::read_to_string(&map).unwrap().contains("DEBUG "));
    let session = gdb_session(&s, &fs::read(&output).unwrap());
    assert!(
        session.contains("Breakpoint 1, leaf (x=x@entry=19) at"),
        "{session}"
    );
}

#[test]
fn pie_debug_output_preserves_shared_layout_and_tls_coordinates() {
    let s = Scratch::new();
    let start = s.object(
        r#"
.section .text.start,"ax",@progbits
.globl _start
.type _start,@function
_start:
.cfi_startproc
    lea value_copy@GOTPCREL(%rip),%rax
    mov (%rax),%rax
    mov (%rax),%edi
    call check
    mov %eax,%edi
    mov $60,%eax
    syscall
.cfi_endproc
.size _start,.-_start
.section .rodata.values,"aM",@progbits,4
.globl value, value_copy
.type value,@object
.type value_copy,@object
value: .long 19
.size value,4
value_copy: .long 19
.size value_copy,4
.section .debug_probe,"",@progbits
.quad value_copy
.quad .rodata.values+4
.quad _start
.quad absolute_value
.weak missing_weak
.quad missing_weak
"#,
    );
    let absolute = s.object(".globl absolute_value\n.set absolute_value,0x1234\n");
    let absolute_path = s.0.join("absolute.o");
    fs::write(&absolute_path, &absolute).unwrap();
    let caller = compile(
        &s,
        "caller",
        "extern int leaf(int);\nint check(int x) { return leaf(x) + 1; }\n",
        &["-fPIE"],
    );
    for with_tls in [false, true] {
        let source = if with_tls {
            "int counter = 3;\n__thread int thread_value = 1;\n__thread int thread_zero;\n__attribute__((noipa)) int leaf(int x) {\n    thread_zero = x * 2;\n    return thread_zero + counter + thread_value - 1;\n}\n"
        } else {
            "int counter = 3;\n__attribute__((noipa)) int leaf(int x) {\n    int doubled = x * 2;\n    return doubled + counter;\n}\n"
        };
        let callee = compile(&s, "callee", source, &["-fPIE"]);
        let inputs = [
            Input::Object(&start),
            Input::Object(&caller),
            Input::Object(&callee),
            Input::Object(&absolute),
        ];
        let prepared =
            class12::gc::prepare_with(&inputs, b"_start", &[], class12::tls::policy).unwrap();
        let lowered = class12::tls::lower_prepared(&prepared).unwrap();
        assert!(lowered.image.placements.contains_key(&class12::tls::GOT));
        assert!(!lowered.relative.is_empty());
        let user_entry = lowered.entry_rva;
        let tls_offsets = lowered.tls.as_ref().map(|tls| tls.offsets.clone());
        let original = lowered.emit().unwrap();
        let linked = class12::debug::link_pie(&inputs, b"_start", &[]).unwrap();
        let start_path = s.0.join("start.o");
        fs::write(&start_path, &start).unwrap();
        let cli_path = s.0.join("cli-program");
        let map_path = s.0.join("cli.map");
        let cli = Command::new(env!("CARGO_BIN_EXE_class12"))
            .arg("--pie")
            .arg("-o")
            .arg(&cli_path)
            .arg("-Map")
            .arg(&map_path)
            .arg(&start_path)
            .arg(s.0.join("caller.o"))
            .arg(s.0.join("callee.o"))
            .arg(&absolute_path)
            .output()
            .unwrap();
        assert!(
            cli.status.success(),
            "{}",
            String::from_utf8_lossy(&cli.stderr)
        );
        assert_eq!(fs::read(&cli_path).unwrap(), linked.bytes);
        assert_eq!(fs::read_to_string(&map_path).unwrap(), linked.map);
        assert_eq!(execute(&cli_path), 42);
        assert!(linked.map.starts_with(&original.map));
        for (at, (a, b)) in linked.bytes.iter().zip(&original.bytes).enumerate() {
            if !(40..48).contains(&at) && !(58..64).contains(&at) {
                assert_eq!(a, b, "PIE byte {at} changed");
            }
        }
        let u64_at = |at: usize| u64::from_le_bytes(linked.bytes[at..at + 8].try_into().unwrap());
        let u16_at = |at: usize| u16::from_le_bytes(linked.bytes[at..at + 2].try_into().unwrap());
        assert_eq!(u16_at(16), 3);
        assert_eq!(u16_at(56), if with_tls { 6 } else { 5 });
        let table = u64_at(40) as usize;
        let names_header = table + usize::from(u16_at(62)) * 64;
        let names = &linked.bytes[u64_at(names_header + 24) as usize
            ..(u64_at(names_header + 24) + u64_at(names_header + 32)) as usize];
        let header = |name: &[u8]| {
            (0..usize::from(u16_at(60)))
                .map(|i| (i, table + i * 64))
                .find(|(_, at)| {
                    let n =
                        u32::from_le_bytes(linked.bytes[*at..*at + 4].try_into().unwrap()) as usize;
                    names[n..].split(|b| *b == 0).next().unwrap() == name
                })
                .unwrap()
        };
        let contents = |name: &[u8]| {
            let (_, at) = header(name);
            &linked.bytes[u64_at(at + 24) as usize..(u64_at(at + 24) + u64_at(at + 32)) as usize]
        };
        let probe: Vec<_> = contents(b".debug_probe")
            .as_chunks::<8>()
            .0
            .iter()
            .map(|b| u64::from_le_bytes(*b))
            .collect();
        assert_eq!(probe.len(), 5);
        assert_eq!(probe[0], probe[1]);
        assert_eq!(probe[2], user_entry);
        assert_eq!(probe[3], 0x1234, "ABS must not undergo RVA conversion");
        assert_eq!(probe[4], 0, "unresolved WEAK zero is a constant");
        for name in [
            b".text".as_slice(),
            b".rodata",
            b".eh_frame",
            b".eh_frame_hdr",
        ] {
            let (_, at) = header(name);
            assert!(
                u64_at(at + 16) < class12::executable::BASE,
                "section address must be an RVA"
            );
        }
        let symbol_names = contents(b".strtab");
        let symbols = contents(b".symtab");
        if let Some(tls_offsets) = &tls_offsets {
            for input in prepared
                .split()
                .iter()
                .filter(|s| s.flags == class12::tls::TLS_FLAGS)
            {
                let output_name = if input.kind == 8 {
                    b".tbss".as_slice()
                } else {
                    b".tdata"
                };
                let (output_index, at) = header(output_name);
                assert_eq!(u64_at(at + 8), class12::tls::TLS_FLAGS);
                for symbol in prepared.tables()[input.id.file].iter().filter(|symbol| {
                    symbol.kind == 6
                        && symbol.definition
                            == class12::symbols::Definition::Section(input.id.section)
                }) {
                    let output = symbols
                        .as_chunks::<24>()
                        .0
                        .iter()
                        .find(|entry| {
                            let n = u32::from_le_bytes(entry[..4].try_into().unwrap()) as usize;
                            symbol_names[n..].split(|b| *b == 0).next().unwrap() == symbol.name
                        })
                        .unwrap();
                    assert_eq!(output[4] & 15, 6);
                    assert_eq!(
                        u16::from_le_bytes(output[6..8].try_into().unwrap()) as usize,
                        output_index
                    );
                    assert_eq!(
                        u64::from_le_bytes(output[8..16].try_into().unwrap()),
                        tls_offsets[&input.id] + symbol.value
                    );
                }
            }
        }
        for _ in 0..3 {
            assert_eq!(run_image(&s, &linked.bytes), 42);
        }
        let gdb = gdb_session(&s, &linked.bytes);
        for expected in [
            "leaf (x=",
            "caller.c:",
            "callee.c:",
            "$1 = 19",
            "$2 = 3",
            "Value returned is $3 = 41",
            "exited with code 052",
        ] {
            assert!(
                gdb.contains(expected),
                "missing {expected:?} with TLS={with_tls}:\n{gdb}"
            );
        }
        // .debug_probe is a byte-level coordinate fixture, not a DWARF section.
        // Decode the actual DWARF kinds rather than asking readelf to parse it.
        let (_, warnings) = tool(
            "readelf",
            &[
                os("--debug-dump=info,abbrev,rawline,decodedline,frames,str,loc,Ranges,aranges"),
                s.0.join("program").as_os_str(),
            ],
        );
        assert!(!warnings.to_lowercase().contains("warning"), "{warnings}");
    }
}

#[test]
fn zero_sized_tls_symbol_keeps_its_nobits_section_identity() {
    let s = Scratch::new();
    for initialized in [false, true] {
        let source = format!(
            ".text\n.globl _start\n_start: mov $60,%eax; mov $42,%edi; syscall\n{}\n.section .tbss.empty,\"awT\",@nobits\n.globl empty_tls\n.type empty_tls,@tls_object\nempty_tls:\n.size empty_tls,0\n",
            if initialized {
                ".section .tdata,\"awT\",@progbits\n.globl initialized_tls\n.type initialized_tls,@tls_object\ninitialized_tls: .long 1\n.size initialized_tls,4"
            } else {
                ""
            }
        );
        let object = s.object(&source);
        let keep: Vec<&[u8]> = if initialized {
            vec![b"empty_tls", b"initialized_tls"]
        } else {
            vec![b"empty_tls"]
        };
        let linked = class12::debug::link_pie(&[Input::Object(&object)], b"_start", &keep).unwrap();
        assert_eq!(run_image(&s, &linked.bytes), 42);
        let u64_at = |at: usize| u64::from_le_bytes(linked.bytes[at..at + 8].try_into().unwrap());
        let u32_at = |at: usize| u32::from_le_bytes(linked.bytes[at..at + 4].try_into().unwrap());
        let u16_at = |at: usize| u16::from_le_bytes(linked.bytes[at..at + 2].try_into().unwrap());
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
        let symbols = named(b".symtab");
        let strings = table + u32_at(symbols + 40) as usize * 64;
        let string_bytes = &linked.bytes
            [u64_at(strings + 24) as usize..(u64_at(strings + 24) + u64_at(strings + 32)) as usize];
        let symbol = (0..u64_at(symbols + 32) as usize / 24)
            .map(|i| u64_at(symbols + 24) as usize + i * 24)
            .find(|&at| {
                string_bytes[u32_at(at) as usize..]
                    .split(|&b| b == 0)
                    .next()
                    == Some(&b"empty_tls"[..])
            })
            .unwrap();
        assert_eq!(linked.bytes[symbol + 4] & 15, 6);
        assert_eq!(u64_at(symbol + 16), 0);
        let section = table + usize::from(u16_at(symbol + 6)) * 64;
        assert_eq!(section, named(b".tbss"));
        assert_eq!(u32_at(section + 4), 8);
        assert_eq!(u64_at(section + 8), class12::tls::TLS_FLAGS);
        assert_eq!(u64_at(section + 32), 0);
    }
}

#[test]
fn debug_metadata_preserves_merge_mapping_and_unwind_headers() {
    let s = Scratch::new();
    let object = s.object(
        r#"
.text
.globl _start
.type _start,@function
_start:
.cfi_startproc
    mov value_copy(%rip),%edi
    mov $60,%eax
    syscall
.cfi_endproc
.size _start,.-_start
.section .rodata.values,"aM",@progbits,4
.globl value, value_copy
.type value,@object
.type value_copy,@object
value: .long 42
.size value,4
value_copy: .long 42
.size value_copy,4
.section .debug_probe,"",@progbits
.quad value
.quad value_copy
.quad .rodata.values + 4
.quad _start
"#,
    );
    let inputs = [Input::Object(&object)];
    let original = class12::unwind::link(&inputs, b"_start", &[]).unwrap();
    let linked = class12::debug::link(&inputs, b"_start", &[]).unwrap();
    assert_eq!(run_image(&s, &linked.bytes), 42);
    for (at, (a, b)) in linked.bytes.iter().zip(&original.bytes).enumerate() {
        if !(40..48).contains(&at) && !(58..64).contains(&at) {
            assert_eq!(a, b, "upstream byte {at} changed");
        }
    }
    assert!(linked.map.starts_with(&original.map));
    assert_eq!(
        u16::from_le_bytes(linked.bytes[56..58].try_into().unwrap()),
        5
    );
    let u64_at = |at: usize| u64::from_le_bytes(linked.bytes[at..at + 8].try_into().unwrap());
    let u16_at = |at: usize| u16::from_le_bytes(linked.bytes[at..at + 2].try_into().unwrap());
    let table = u64_at(40) as usize;
    let names_header = table + usize::from(u16_at(62)) * 64;
    let names = &linked.bytes[u64_at(names_header + 24) as usize
        ..(u64_at(names_header + 24) + u64_at(names_header + 32)) as usize];
    let named = |name: &[u8]| {
        (0..usize::from(u16_at(60)))
            .map(|i| table + i * 64)
            .find(|at| {
                let n = u32::from_le_bytes(linked.bytes[*at..*at + 4].try_into().unwrap()) as usize;
                names[n..].split(|b| *b == 0).next().unwrap() == name
            })
            .unwrap()
    };
    let bytes = |name: &[u8]| {
        let at = named(name);
        &linked.bytes[u64_at(at + 24) as usize..(u64_at(at + 24) + u64_at(at + 32)) as usize]
    };
    let probe = bytes(b".debug_probe");
    let values: Vec<_> = probe
        .as_chunks::<8>()
        .0
        .iter()
        .map(|b| u64::from_le_bytes(*b))
        .collect();
    assert_eq!(values.len(), 4);
    assert_eq!(
        values[0], values[1],
        "named copies must share the merged address"
    );
    assert_eq!(
        values[0], values[2],
        "SECTION symbol plus addend must use piece mapping"
    );
    assert_eq!(values[0], u64_at(named(b".rodata") + 16));
    assert_eq!(u64_at(named(b".rodata") + 32), 4);
    assert_eq!(
        values[3],
        u64::from_le_bytes(linked.bytes[24..32].try_into().unwrap())
    );
    assert!(!bytes(b".eh_frame").is_empty());
    assert!(!bytes(b".eh_frame_hdr").is_empty());
    let path = s.0.join("with-debug");
    fs::write(&path, &linked.bytes).unwrap();
    let (symbols, _) = tool("nm", &[path.as_os_str()]);
    let address = |name: &str| {
        symbols
            .lines()
            .find(|l| l.ends_with(&format!(" {name}")))
            .unwrap()
            .split_whitespace()
            .next()
            .unwrap()
            .to_owned()
    };
    assert_eq!(address("value"), address("value_copy"));
    let (_, warnings) = tool("readelf", &["--unwind".as_ref(), path.as_os_str()]);
    assert!(warnings.is_empty(), "{warnings}");
}
