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
            "tinylink-class13-{}-{}",
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
use class13::archive::Input;
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
const TWINS_A: &str = r#"__attribute__((noipa)) int twin_a(int x) { return x * 3 + 1; }
__attribute__((noipa)) int twin_b(int x) { return x * 3 + 1; }
extern int twin_c(int);
int check(void) { return twin_a(4) + twin_b(4) + twin_c(4) + 3; }
"#;
const TWINS_B: &str = "__attribute__((noipa)) int twin_c(int x) { return x * 3 + 1; }\n";
fn twins(s: &Scratch) -> [Vec<u8>; 3] {
    let objects = [
        s.object(START),
        compile(s, "twins_a", TWINS_A, &[]),
        compile(s, "twins_b", TWINS_B, &[]),
    ];
    for (name, bytes) in ["start.o", "twins_a.o", "twins_b.o"].iter().zip(&objects) {
        fs::write(s.0.join(name), bytes).unwrap();
    }
    objects
}
fn addresses(s: &Scratch, image: &[u8]) -> std::collections::BTreeMap<String, u64> {
    let path = s.0.join("inspect");
    fs::write(&path, image).unwrap();
    tool("nm", &[path.as_os_str()])
        .0
        .lines()
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            let address = u64::from_str_radix(fields.next()?, 16).ok()?;
            Some((fields.nth(1)?.to_string(), address))
        })
        .collect()
}
#[test]
fn identical_functions_fold_and_still_run() {
    let s = Scratch::new();
    let [start, a, b] = twins(&s);
    let inputs = [Input::Object(&start), Input::Object(&a), Input::Object(&b)];
    let linked = class13::icf::link(&inputs, b"_start", &[], &[]).unwrap();
    assert_eq!(run_image(&s, &linked.bytes), 42, "{}", linked.map);
    let folds: Vec<_> = linked
        .map
        .lines()
        .filter(|l| l.starts_with("FOLD "))
        .collect();
    assert_eq!(folds.len(), 2, "{}", linked.map);
    let symbols = addresses(&s, &linked.bytes);
    assert_eq!(symbols["twin_a"], symbols["twin_b"]);
    assert_eq!(symbols["twin_a"], symbols["twin_c"]);
    assert_ne!(symbols["twin_a"], symbols["check"]);
    let unfolded = class13::debug::link(&inputs, b"_start", &[]).unwrap();
    let unfolded = addresses(&s, &unfolded.bytes);
    assert_ne!(unfolded["twin_a"], unfolded["twin_b"]);
    assert_eq!(
        class13::icf::link(&inputs, b"_start", &[], &[]).unwrap(),
        linked
    );
}
const ADDRESS_TAKEN: &str = r#"__attribute__((noipa)) int twin_a(int x) { return x * 3 + 1; }
__attribute__((noipa)) int twin_b(int x) { return x * 3 + 1; }
__attribute__((noipa)) int twin_c(int x) { return x * 3 + 1; }
__attribute__((noipa)) int twin_d(int x) { return x * 3 + 1; }
int (*volatile pa)(int) = twin_a;
int (*volatile pb)(int) = twin_b;
int check(void) {
    if (pa == pb) return 1;
    return pa(4) + pb(4) + twin_c(1) + twin_d(1) + 8;
}
"#;
#[test]
fn address_taken_functions_stay_distinct() {
    let s = Scratch::new();
    let start = s.object(START);
    let object = compile(&s, "taken", ADDRESS_TAKEN, &[]);
    let inputs = [Input::Object(&start), Input::Object(&object)];
    let linked = class13::icf::link(&inputs, b"_start", &[], &[]).unwrap();
    assert_eq!(run_image(&s, &linked.bytes), 42, "{}", linked.map);
    let symbols = addresses(&s, &linked.bytes);
    assert_ne!(symbols["twin_a"], symbols["twin_b"]);
    assert_ne!(symbols["twin_a"], symbols["twin_c"]);
    assert_eq!(symbols["twin_c"], symbols["twin_d"]);
    assert_eq!(
        linked
            .map
            .lines()
            .filter(|l| l.starts_with("FOLD "))
            .count(),
        1
    );
    // An explicit root is significant too.
    let kept = class13::icf::link(&inputs, b"_start", &[b"twin_d"], &[]).unwrap();
    let symbols = addresses(&s, &kept.bytes);
    assert_ne!(symbols["twin_c"], symbols["twin_d"]);
}
fn tool(program: &str, args: &[&std::ffi::OsStr]) -> (String, String) {
    let p = Command::new(program).args(args).output().unwrap();
    assert!(
        p.status.success(),
        "{program}: {}",
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
    // The breakpoint address is shared by every folded twin, so delete it
    // before letting the program run to completion.
    let script = [
        "break twin_b",
        "run",
        "backtrace",
        "finish",
        "delete",
        "continue",
    ];
    let mut args = vec![os("-nx"), os("-batch")];
    for command in &script {
        args.extend([os("-ex"), os(command)]);
    }
    args.push(path.as_os_str());
    tool("gdb", &args).0
}
#[test]
fn debugger_stops_in_folded_functions() {
    let s = Scratch::new();
    let [start, a, b] = twins(&s);
    let linked = class13::icf::link(
        &[Input::Object(&start), Input::Object(&a), Input::Object(&b)],
        b"_start",
        &[],
        &[],
    )
    .unwrap();
    let session = gdb_session(&s, &linked.bytes);
    for expected in [
        "Breakpoint 1, ",
        "twins_a.c:",
        "Value returned is",
        "exited with code 052",
    ] {
        assert!(
            session.contains(expected),
            "missing {expected:?} in\n{session}"
        );
    }
    let path = s.0.join("program");
    let (info, warnings) = tool("readelf", &[os("--debug-dump"), path.as_os_str()]);
    assert!(!warnings.to_lowercase().contains("warning"), "{warnings}");
    // Only the representative's DWARF claims the shared code.
    let low_pc = |name: &str| {
        let die = info
            .split("DW_TAG_subprogram")
            .find(|d| d.contains(&format!(": {name}\n")))
            .unwrap_or_else(|| panic!("no DIE for {name}"));
        let line = die.lines().find(|l| l.contains("DW_AT_low_pc")).unwrap();
        line.rsplit(':').next().unwrap().trim().to_string()
    };
    assert!(matches!(low_pc("twin_b").as_str(), "0" | "0x0"));
    assert!(!matches!(low_pc("twin_a").as_str(), "0" | "0x0"));
}
const ORDERED: &str = concat!(
    ".section .text.start,\"ax\",@progbits\n.globl _start\n_start: call hot; call cold; call mid; call copy; mov $42,%edi; mov $60,%eax; syscall\n",
    ".section .text.mid,\"ax\",@progbits\n.globl mid\nmid: mov $1,%eax; ret\n",
    ".section .text.hot,\"ax\",@progbits\n.globl hot\nhot: mov $2,%eax; ret\n",
    ".section .text.cold,\"ax\",@progbits\n.globl cold\ncold: mov $3,%eax; ret\n",
    ".section .text.copy,\"ax\",@progbits\n.globl copy\ncopy: mov $3,%eax; ret\n",
    ".data\n.globl datum\ndatum: .quad 0\n",
);
#[test]
fn order_places_requested_functions_first() {
    use class13::icf::Error;
    let s = Scratch::new();
    let object = s.object(ORDERED);
    let inputs = [Input::Object(&object)];
    let plain = addresses(
        &s,
        &class13::icf::link(&inputs, b"_start", &[], &[])
            .unwrap()
            .bytes,
    );
    assert!(plain["_start"] < plain["mid"] && plain["mid"] < plain["hot"]);
    let linked = class13::icf::link(&inputs, b"_start", &[], &[b"cold", b"hot", b"cold"]).unwrap();
    assert_eq!(run_image(&s, &linked.bytes), 42);
    let ordered = addresses(&s, &linked.bytes);
    assert!(ordered["cold"] < ordered["hot"], "{ordered:?}");
    assert!(ordered["hot"] < ordered["_start"] && ordered["_start"] < ordered["mid"]);
    // `copy` folds into `cold`; naming it places the representative.
    let symbols = |order: &[&[u8]]| {
        let linked = class13::icf::link(&inputs, b"_start", &[], order).unwrap();
        assert!(linked.map.contains("FOLD "), "{}", linked.map);
        addresses(&s, &linked.bytes)
    };
    let by_copy = symbols(&[b"copy"]);
    assert_eq!(by_copy["copy"], by_copy["cold"]);
    assert!(by_copy["cold"] < by_copy["_start"], "{by_copy:?}");
    for bad in [&b"datum"[..], b"missing", b"_start_"] {
        assert_eq!(
            class13::icf::link(&inputs, b"_start", &[], &[bad]),
            Err(Error::InvalidOrder(bad.to_vec()))
        );
    }
}
#[test]
fn without_folding_or_order_output_matches_class12() {
    let s = Scratch::new();
    let start = s.object(START);
    let object = compile(
        &s,
        "taken",
        ADDRESS_TAKEN
            .replace(
                "twin_d(int x) { return x * 3 + 1; }",
                "twin_d(int x) { return x * 3 + 2; }",
            )
            .as_str(),
        &[],
    );
    let inputs = [Input::Object(&start), Input::Object(&object)];
    let ours = class13::icf::link(&inputs, b"_start", &[], &[]).unwrap();
    assert!(!ours.map.contains("FOLD "));
    assert_eq!(ours, class13::debug::link(&inputs, b"_start", &[]).unwrap());
}
#[test]
fn cli_accepts_an_order() {
    let s = Scratch::new();
    twins(&s);
    let output = s.0.join("program");
    let map = s.0.join("program.map");
    let p = Command::new(env!("CARGO_BIN_EXE_class13"))
        .arg("-o")
        .arg(&output)
        .arg("-Map")
        .arg(&map)
        .args(["--order", "check"])
        .args(["start.o", "twins_a.o", "twins_b.o"].map(|n| s.0.join(n)))
        .output()
        .unwrap();
    assert!(p.status.success(), "{}", String::from_utf8_lossy(&p.stderr));
    assert_eq!(execute(&output), 42);
    assert_eq!(
        fs::read_to_string(&map)
            .unwrap()
            .lines()
            .filter(|l| l.starts_with("FOLD "))
            .count(),
        2
    );
    let symbols = addresses(&s, &fs::read(&output).unwrap());
    assert!(symbols["check"] < symbols["_start"], "{symbols:?}");
}

#[test]
fn folding_preserves_merged_constants_and_compatible_unwind() {
    let s = Scratch::new();
    let input = |different: bool| {
        s.object(&format!(
            r#"
.section .text.start,"ax",@progbits
.globl _start
_start:
.cfi_startproc
call twin_a
mov %eax,%ebx
call twin_b
add %ebx,%eax
mov %eax,%edi
mov $60,%eax
syscall
.cfi_endproc
.section .text.a,"ax",@progbits
.globl twin_a
.type twin_a,@function
twin_a:
.cfi_startproc
mov value_copy(%rip),%eax
ret
.cfi_endproc
.section .text.b,"ax",@progbits
.globl twin_b
.type twin_b,@function
twin_b:
.cfi_startproc
{cfi}
mov value_copy(%rip),%eax
ret
.cfi_endproc
.section .text.dead,"ax",@progbits
dead:
.cfi_startproc
ret
.cfi_endproc
.section .rodata.values,"aM",@progbits,4
value: .long 21
value_copy: .long 21
.section .debug_probe,"",@progbits
.quad twin_a
.quad twin_b
"#,
            cfi = if different {
                ".cfi_def_cfa_offset 16"
            } else {
                ""
            }
        ))
    };
    for different in [false, true] {
        let object = input(different);
        let inputs = [Input::Object(&object)];
        let prepared =
            class13::gc::prepare_with(&inputs, b"_start", &[], class13::unwind::policy).unwrap();
        let descriptions = class13::icf::unwind_descriptions(&prepared).unwrap();
        assert_eq!(descriptions.len(), 3);
        let linked = class13::icf::link(&inputs, b"_start", &[], &[b"twin_b"]).unwrap();
        assert_eq!(run_image(&s, &linked.bytes), 42);
        let symbols = addresses(&s, &linked.bytes);
        assert_eq!(
            symbols["twin_a"] == symbols["twin_b"],
            !different,
            "descriptions: {descriptions:?}; map: {}",
            linked.map
        );
        assert!(symbols["twin_b"] < symbols["_start"]);
        let count = linked
            .map
            .lines()
            .filter(|l| l.starts_with("FOLD "))
            .count();
        assert_eq!(count, usize::from(!different));
        assert_eq!(
            u16::from_le_bytes(linked.bytes[56..58].try_into().unwrap()),
            5
        );
        let path = s.0.join("combined");
        fs::write(&path, &linked.bytes).unwrap();
        let (sections, warnings) = tool("readelf", &["-SW".as_ref(), path.as_os_str()]);
        assert!(sections.contains(".eh_frame_hdr"), "{sections}");
        assert!(warnings.is_empty(), "{warnings}");
        let probe_path = s.0.join("probe.bin");
        let dump = format!(".debug_probe={}", probe_path.display());
        tool(
            "objcopy",
            &["--dump-section".as_ref(), dump.as_ref(), path.as_os_str()],
        );
        let probe = fs::read(probe_path).unwrap();
        let a = u64::from_le_bytes(probe[0..8].try_into().unwrap());
        let b = u64::from_le_bytes(probe[8..16].try_into().unwrap());
        assert_eq!(a, symbols["twin_a"]);
        assert_eq!(b, if different { symbols["twin_b"] } else { 0 });
        let (frames, warnings) = tool(
            "readelf",
            &["--debug-dump=frames".as_ref(), path.as_os_str()],
        );
        assert_eq!(
            frames.lines().filter(|l| l.contains(" FDE cie=")).count(),
            if different { 3 } else { 2 }
        );
        assert!(warnings.is_empty(), "{warnings}");
    }
}

#[test]
fn pie_folding_preserves_tls_got_merge_debug_and_unwind() {
    let s = Scratch::new();
    let input = |different: bool| {
        s.object(&format!(
            r#"
.section .text.start,"ax",@progbits
.globl _start
_start:
.cfi_startproc
call twin_a
mov %eax,%ebx
call twin_b
add %ebx,%eax
mov %eax,%edi
mov $60,%eax
syscall
.cfi_endproc
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
{cfi}
lea value_copy@GOTPCREL(%rip),%rax
mov (%rax),%rax
mov (%rax),%eax
add %fs:counter@tpoff,%eax
ret
.cfi_endproc
.section .text.dead,"ax",@progbits
dead:
.cfi_startproc
ret
.cfi_endproc
.section .rodata.values,"aM",@progbits,4
value: .long 20
value_copy: .long 20
.section .tdata,"awT",@progbits
.globl counter
.type counter,@tls_object
counter: .long 1
.size counter,4
.section .debug_probe,"",@progbits
.quad twin_a
.quad twin_b
"#,
            cfi = if different {
                ".cfi_def_cfa_offset 16"
            } else {
                ""
            }
        ))
    };
    for different in [false, true] {
        let object = input(different);
        let inputs = [Input::Object(&object)];
        let prepared =
            class13::gc::prepare_with(&inputs, b"_start", &[], class13::tls::policy).unwrap();
        let descriptions = class13::icf::unwind_descriptions(&prepared).unwrap();
        assert_eq!(descriptions.len(), 3);
        let linked = class13::icf::link_pie(&inputs, b"_start", &[], &[b"twin_b"]).unwrap();
        assert_eq!(run_image(&s, &linked.bytes), 42);
        let symbols = addresses(&s, &linked.bytes);
        assert_eq!(
            symbols["twin_a"] == symbols["twin_b"],
            !different,
            "descriptions: {descriptions:?}; map: {}",
            linked.map
        );
        assert!(symbols["twin_b"] < symbols["_start"]);
        let count = linked
            .map
            .lines()
            .filter(|l| l.starts_with("FOLD "))
            .count();
        assert_eq!(count, usize::from(!different));
        assert_eq!(
            u16::from_le_bytes(linked.bytes[56..58].try_into().unwrap()),
            6
        );
        let path = s.0.join("combined");
        fs::write(&path, &linked.bytes).unwrap();
        let (sections, warnings) = tool("readelf", &["-SW".as_ref(), path.as_os_str()]);
        assert!(sections.contains(".eh_frame_hdr"), "{sections}");
        assert!(warnings.is_empty(), "{warnings}");
        let probe_path = s.0.join("probe.bin");
        let dump = format!(".debug_probe={}", probe_path.display());
        tool(
            "objcopy",
            &["--dump-section".as_ref(), dump.as_ref(), path.as_os_str()],
        );
        let probe = fs::read(probe_path).unwrap();
        let a = u64::from_le_bytes(probe[0..8].try_into().unwrap());
        let b = u64::from_le_bytes(probe[8..16].try_into().unwrap());
        assert_eq!(a, symbols["twin_a"]);
        assert_eq!(b, if different { symbols["twin_b"] } else { 0 });
        let (frames, warnings) = tool(
            "readelf",
            &["--debug-dump=frames".as_ref(), path.as_os_str()],
        );
        assert_eq!(
            frames.lines().filter(|l| l.contains(" FDE cie=")).count(),
            if different { 3 } else { 2 }
        );
        assert!(warnings.is_empty(), "{warnings}");
        assert!(linked.map.contains(".got"));
        assert!(sections.contains(".tdata"), "{sections}");
        assert!(symbols["twin_a"] < class13::executable::BASE);
        assert_eq!(symbols["counter"], 0, "STT_TLS stores module offset");
        for _ in 0..3 {
            assert_eq!(run_image(&s, &linked.bytes), 42);
        }
        let object_path = s.0.join("combined.o");
        fs::write(&object_path, &object).unwrap();
        let cli = s.0.join("cli-program");
        let map = s.0.join("cli.map");
        let result = Command::new(env!("CARGO_BIN_EXE_class13"))
            .args(["--pie", "--order", "twin_b", "-o"])
            .arg(&cli)
            .arg("-Map")
            .arg(&map)
            .arg(&object_path)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(fs::read(&cli).unwrap(), linked.bytes);
        assert_eq!(fs::read_to_string(&map).unwrap(), linked.map);
    }
}

#[test]
fn unwind_compatibility_rejects_unsupported_personality() {
    let s = Scratch::new();
    let object = s.object(concat!(
        ".text\n.globl _start\n_start:\n.cfi_startproc\n",
        ".cfi_personality 0x3,personality\nret\n.cfi_endproc\n",
        ".data\npersonality: .quad 0\n",
    ));
    let inputs = [Input::Object(&object)];
    let prepared =
        class13::gc::prepare_with(&inputs, b"_start", &[], class13::unwind::policy).unwrap();
    for result in [
        class13::icf::unwind_descriptions(&prepared).err(),
        class13::icf::link(&inputs, b"_start", &[], &[]).err(),
        class13::icf::link_pie(&inputs, b"_start", &[], &[]).err(),
    ] {
        assert!(matches!(
            result,
            Some(class13::icf::Error::MergeUnwind(
                class13::unwind::Error::Unwind {
                    cause: class13::unwind::UnwindError::Unsupported(0),
                    ..
                }
            ))
        ));
    }
}

#[test]
fn pie_folding_preserves_pointer_identity_and_debugger_view() {
    let s = Scratch::new();
    let start = s.object(START);
    let object = compile(&s, "taken", ADDRESS_TAKEN, &["-fPIE"]);
    let inputs = [Input::Object(&start), Input::Object(&object)];
    let linked = class13::icf::link_pie(&inputs, b"_start", &[], &[]).unwrap();
    assert_eq!(run_image(&s, &linked.bytes), 42);
    let symbols = addresses(&s, &linked.bytes);
    assert_ne!(symbols["twin_a"], symbols["twin_b"]);
    assert_eq!(symbols["twin_c"], symbols["twin_d"]);
    let kept = class13::icf::link_pie(&inputs, b"_start", &[b"twin_d"], &[]).unwrap();
    let kept_symbols = addresses(&s, &kept.bytes);
    assert_ne!(kept_symbols["twin_c"], kept_symbols["twin_d"]);
    assert_eq!(
        kept,
        class13::debug::link_pie(&inputs, b"_start", &[b"twin_d"]).unwrap()
    );
    let a = compile(&s, "twins_a", TWINS_A, &["-fPIE"]);
    let b = compile(&s, "twins_b", TWINS_B, &["-fPIE"]);
    let linked = class13::icf::link_pie(
        &[Input::Object(&start), Input::Object(&a), Input::Object(&b)],
        b"_start",
        &[],
        &[],
    )
    .unwrap();
    let session = gdb_session(&s, &linked.bytes);
    for expected in [
        "Breakpoint 1, ",
        "twins_a.c:",
        "Value returned is",
        "exited with code 052",
    ] {
        assert!(
            session.contains(expected),
            "missing {expected:?}:\n{session}"
        );
    }
}

#[test]
fn pie_lowering_rejects_invalid_aliases_and_order() {
    use class13::gc::SectionId;
    let s = Scratch::new();
    let object =
        s.object(".text\n.globl _start\n_start: ret\n.data\n.globl datum\ndatum: .long 1\n");
    let inputs = [Input::Object(&object)];
    let prepared =
        class13::gc::prepare_with(&inputs, b"_start", &[b"datum"], class13::tls::policy).unwrap();
    let text = prepared
        .contributions()
        .iter()
        .find(|c| c.kind == class13::layout::Kind::Text)
        .unwrap();
    let data = prepared
        .contributions()
        .iter()
        .find(|c| c.kind == class13::layout::Kind::Data)
        .unwrap();
    let text = SectionId {
        file: text.file,
        section: text.section,
    };
    let data = SectionId {
        file: data.file,
        section: data.section,
    };
    let missing = SectionId {
        file: usize::MAX,
        section: 0,
    };
    for (from, to) in [(text, text), (text, data), (data, text), (text, missing)] {
        let aliases = [(from, to)].into_iter().collect();
        assert!(
            matches!(class13::icf::lower_pie(&prepared, &aliases, &[], class13::tls::UnwindPolicy::Preserve),
            Err(class13::icf::Error::PieTls(class13::tls::Error::InvalidAlias { from: f, to: t })) if (f,t)==(from,to))
        );
    }
    assert!(
        matches!(class13::icf::lower_pie(&prepared, &Default::default(), &[data], class13::tls::UnwindPolicy::Preserve),
        Err(class13::icf::Error::InvalidOrderSection(id)) if id==data)
    );
}

#[test]
fn pie_entry_alias_and_duplicate_order_select_the_representative() {
    let s = Scratch::new();
    let object = s.object(concat!(
        ".section .text.first,\"ax\",@progbits\n.globl first\nfirst:\n",
        ".cfi_startproc\nmov $42,%edi; mov $60,%eax; syscall\n.cfi_endproc\n",
        ".section .text.start,\"ax\",@progbits\n.globl _start\n_start:\n",
        ".cfi_startproc\nmov $42,%edi; mov $60,%eax; syscall\n.cfi_endproc\n",
    ));
    let inputs = [Input::Object(&object)];
    let linked =
        class13::icf::link_pie(&inputs, b"_start", &[b"first"], &[b"_start", b"_start"]).unwrap();
    // Explicit keep pins `first`; both entries remain independent.
    let symbols = addresses(&s, &linked.bytes);
    assert_ne!(symbols["first"], symbols["_start"]);
    assert_eq!(run_image(&s, &linked.bytes), 42);

    // A call from the entry keeps the earlier identical exit sections live
    // without making their address identity observable.
    let root = s.object(
        ".section .text.root,\"ax\",@progbits\n.globl root\nroot: call first; call _start\n",
    );
    let inputs = [Input::Object(&object), Input::Object(&root)];
    let linked = class13::icf::link_pie(
        &inputs,
        b"_start",
        &[b"root"],
        &[b"_start", b"first", b"_start"],
    )
    .unwrap();
    let symbols = addresses(&s, &linked.bytes);
    assert_eq!(symbols["first"], symbols["_start"]);
    assert!(symbols["first"] < symbols["root"]);
    assert_eq!(
        linked
            .map
            .lines()
            .filter(|line| line.starts_with("FOLD "))
            .count(),
        1
    );
    assert_eq!(run_image(&s, &linked.bytes), 42);
    let once = class13::icf::link_pie(&inputs, b"_start", &[b"root"], &[b"first"]).unwrap();
    assert_eq!(linked, once);
}
