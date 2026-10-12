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
            "tinylink-class4-{}-{}",
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
#[test]
fn native_link_and_execute() {
    use std::os::unix::fs::PermissionsExt;
    let scratch = Scratch::new();
    for status in [0, 42, 93] {
        let object=scratch.object(&format!(".text\n.balign 64\n.space 7,0x90\n.globl _start\n_start: mov $60, %eax; mov ${status}, %edi; syscall\n"));
        let image = class4::executable::link_single(&object, b"_start").unwrap();
        let output = scratch.0.join("program");
        fs::write(&output, &image).unwrap();
        fs::set_permissions(&output, fs::Permissions::from_mode(0o755)).unwrap();
        let r = Command::new("readelf")
            .args(["-hW", "-lW"])
            .arg(&output)
            .output()
            .unwrap();
        assert!(r.status.success());
        assert!(
            r.stderr.is_empty(),
            "{}",
            String::from_utf8_lossy(&r.stderr)
        );
        let text = String::from_utf8(r.stdout).unwrap();
        assert!(text.contains("EXEC"));
        assert!(text.contains("LOAD"));
        assert!(text.contains("GNU_STACK"));
        assert_eq!(execute(&output), status);
    }
}
#[test]
fn reject_unimplemented_input_features() {
    use class4::executable::Error;
    let scratch = Scratch::new();
    for (src, error) in [
        (
            ".text\n.globl _start\n_start: call missing; ret\n",
            Error::UnsupportedRelocations,
        ),
        (
            ".text\n.globl _start\n_start: ret\n.data\n.long 7\n",
            Error::UnsupportedSections,
        ),
        (
            ".text\n.globl _start\n_start: ret\n.bss\n.space 8\n",
            Error::UnsupportedSections,
        ),
        (".text\n.globl other\nother: ret\n", Error::MissingEntry),
        (
            ".text\n.byte 0x90\n.globl _start\n_start:\n",
            Error::InvalidEntry,
        ),
        (
            ".text\n.byte 0x90\n.globl _start\n.set _start,42\n",
            Error::InvalidEntry,
        ),
    ] {
        assert_eq!(
            class4::executable::link_single(&scratch.object(src), b"_start"),
            Err(error)
        );
    }
}
#[test]
fn supplied_cli_links_input() {
    let scratch = Scratch::new();
    scratch.object(".text\n.globl _start\n_start: mov $60,%eax; mov $17,%edi; syscall\n");
    let output = scratch.0.join("cli-program");
    let result = Command::new(env!("CARGO_BIN_EXE_class4"))
        .arg(scratch.0.join("input.o"))
        .arg(&output)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(execute(&output), 17);
}

#[test]
fn custom_code_section_and_entry() {
    let scratch = Scratch::new();
    let object=scratch.object(".section .custom_code,\"ax\",@progbits\n.balign 32\n.space 9,0x90\n.globl launch\nlaunch: mov $60,%eax; mov $11,%edi; syscall\n");
    let image = class4::executable::link_single(&object, b"launch").unwrap();
    assert_eq!(
        class4::executable::link_single(&object, b"_start"),
        Err(class4::executable::Error::MissingEntry)
    );
    let sections = class4::sections::sections(&object).unwrap();
    let (code, _) = sections
        .iter()
        .find(|(_, name)| *name == b".custom_code")
        .unwrap();
    let offset = (class4::executable::HEADER_BYTES + 31) & !31;
    assert_eq!(
        &image[offset as usize..],
        class4::sections::payload(&object, code).unwrap()
    );
    let entry = u64::from_le_bytes(image[24..32].try_into().unwrap());
    assert_eq!(entry, class4::executable::BASE + offset + 9);
}

#[test]
fn missing_symbol_table_is_explicit() {
    let scratch = Scratch::new();
    scratch.object(".text\n.globl _start\n_start: ret\n");
    let stripped = scratch.0.join("stripped.o");
    let p = Command::new("objcopy")
        .arg("--strip-all")
        .arg(scratch.0.join("input.o"))
        .arg(&stripped)
        .output()
        .unwrap();
    assert!(p.status.success(), "{}", String::from_utf8_lossy(&p.stderr));
    assert_eq!(
        class4::executable::link_single(&fs::read(stripped).unwrap(), b"_start"),
        Err(class4::executable::Error::SymbolTableCount)
    );
}
