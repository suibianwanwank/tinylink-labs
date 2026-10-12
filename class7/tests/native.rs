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
            "tinylink-class7-{}-{}",
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
use class7::archive::Input;
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
const START: &str =
    ".text\n.globl _start\n_start: call answer; mov %eax,%edi; mov $60,%eax; syscall\n";
#[test]
fn archive_local_fixed_point_and_unused_member() {
    let s = Scratch::new();
    let start = s.object(START);
    let ar = archive(
        &s,
        &[
            (
                "leaf_with_a_long_member_name.o",
                ".text\n.globl leaf\nleaf: mov $42,%eax; ret\n",
            ),
            ("answer.o", ".text\n.globl answer\nanswer: jmp leaf\n"),
            (
                "unused.o",
                ".text\n.globl _start\n_start: ret\n.section .tbss,\"awT\",@nobits\n.skip 8\n",
            ),
        ],
    );
    let m = class7::archive::members(&ar).unwrap();
    assert_eq!(m.len(), 3);
    assert_eq!(m[0].name, b"leaf_with_a_long_member_name.o");
    let selected = class7::archive::select(&[Input::Object(&start), Input::Archive(&ar)]).unwrap();
    assert_eq!(selected, vec![start.as_slice(), m[1].bytes, m[0].bytes]);
    assert_eq!(
        run_image(
            &s,
            &class7::archive::link(&[Input::Object(&start), Input::Archive(&ar)], b"_start")
                .unwrap()
        ),
        42
    );
}
#[test]
fn earlier_archive_is_not_revisited() {
    let s = Scratch::new();
    let start = s.object(START);
    let ar = archive(
        &s,
        &[(
            "answer.o",
            ".text\n.globl answer\nanswer: mov $42,%eax; ret\n",
        )],
    );
    assert_eq!(
        class7::archive::select(&[Input::Archive(&ar), Input::Object(&start)]).unwrap(),
        vec![start.as_slice()]
    );
    assert!(
        class7::archive::link(&[Input::Archive(&ar), Input::Object(&start)], b"_start").is_err()
    );
    let inputs = [
        Input::Archive(&ar),
        Input::Object(&start),
        Input::Archive(&ar),
    ];
    assert_eq!(
        run_image(&s, &class7::archive::link(&inputs, b"_start").unwrap()),
        42
    );
}
#[test]
fn weak_reference_does_not_extract() {
    let s = Scratch::new();
    let weak=s.object(".text\n.weak hook\n.globl _start\n_start: movabs $hook,%rax; test %rax,%rax; sete %dil; movzbl %dil,%edi; mov $60,%eax; syscall\n");
    let ar = archive(&s, &[("hook.o", ".text\n.globl hook\nhook: ret\n")]);
    let inputs = [Input::Object(&weak), Input::Archive(&ar)];
    assert_eq!(
        class7::archive::select(&inputs).unwrap(),
        vec![weak.as_slice()]
    );
    assert_eq!(
        run_image(&s, &class7::archive::link(&inputs, b"_start").unwrap()),
        1
    );
}
#[test]
fn first_provider_satisfies_request() {
    let s = Scratch::new();
    let start = s.object(START);
    let ar = archive(
        &s,
        &[
            ("weak.o", ".text\n.weak answer\nanswer: mov $7,%eax; ret\n"),
            (
                "strong.o",
                ".text\n.globl answer\nanswer: mov $42,%eax; ret\n",
            ),
        ],
    );
    let inputs = [Input::Object(&start), Input::Archive(&ar)];
    let members = class7::archive::members(&ar).unwrap();
    assert_eq!(
        class7::archive::select(&inputs).unwrap(),
        vec![start.as_slice(), members[0].bytes]
    );
    assert_eq!(
        run_image(&s, &class7::archive::link(&inputs, b"_start").unwrap()),
        7
    );
}
#[test]
fn cli_supports_archive_inputs() {
    let s = Scratch::new();
    let start = s.object(START);
    fs::write(s.0.join("start.o"), start).unwrap();
    archive(
        &s,
        &[(
            "answer.o",
            ".text\n.globl answer\nanswer: mov $42,%eax; ret\n",
        )],
    );
    let output = s.0.join("cli-output");
    let p = Command::new(env!("CARGO_BIN_EXE_class7"))
        .arg("-o")
        .arg(&output)
        .arg(s.0.join("start.o"))
        .arg(s.0.join("library.a"))
        .output()
        .unwrap();
    assert!(p.status.success(), "{}", String::from_utf8_lossy(&p.stderr));
    assert_eq!(execute(&output), 42);
}

#[test]
fn malformed_unused_member_is_rejected_during_metadata_scan() {
    let s = Scratch::new();
    let start = s.object(".text\n.globl _start\n_start: mov $60,%eax; xor %edi,%edi; syscall\n");
    let mut ar = archive(&s, &[("unused.o", ".text\n.globl unused\nunused: ret\n")]);
    let payload = class7::archive::members(&ar).unwrap()[0].header_offset as usize + 60;
    ar[payload] = 0;
    assert!(matches!(
        class7::archive::select(&[Input::Object(&start), Input::Archive(&ar)]),
        Err(class7::archive::Error::Sections(_))
    ));
}
