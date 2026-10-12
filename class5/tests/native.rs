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
            "tinylink-class5-{}-{}",
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
fn c_function(scratch: &Scratch) -> Vec<u8> {
    let src = scratch.0.join("answer.c");
    let obj = scratch.0.join("answer.o");
    fs::write(&src, "int answer(void) { return 42; }\n").unwrap();
    let p = Command::new("cc")
        .args([
            "-c",
            "-O1",
            "-ffunction-sections",
            "-fno-pie",
            "-fno-stack-protector",
            "-fno-asynchronous-unwind-tables",
            "-fno-unwind-tables",
            "-fcf-protection=none",
        ])
        .arg(src)
        .arg("-o")
        .arg(&obj)
        .output()
        .unwrap();
    assert!(p.status.success(), "{}", String::from_utf8_lossy(&p.stderr));
    fs::read(obj).unwrap()
}
const START: &str =
    ".text\n.globl _start\n_start: call answer; mov %eax,%edi; mov $60,%eax; syscall\n";
#[test]
fn cross_object_c_call_both_orders() {
    let scratch = Scratch::new();
    let start = scratch.object(START);
    let answer = c_function(&scratch);
    let originals = (start.clone(), answer.clone());
    for reverse in [false, true] {
        let inputs: Vec<&[u8]> = if reverse {
            vec![&answer, &start]
        } else {
            vec![&start, &answer]
        };
        let image = class5::relocate::link(&inputs, b"_start").unwrap();
        assert_eq!(run_image(&scratch, &image), 42);
        let entry = u64::from_le_bytes(image[24..32].try_into().unwrap());
        let pos = (entry - class5::executable::BASE) as usize;
        assert_eq!(image[pos], 0xe8);
        let displacement = i32::from_le_bytes(image[pos + 1..pos + 5].try_into().unwrap());
        assert_eq!(displacement < 0, reverse);
        assert_eq!(image, class5::relocate::link(&inputs, b"_start").unwrap());
    }
    assert_eq!((start, answer), originals);
}
#[test]
fn absolute_call_and_unresolved_weak() {
    let scratch = Scratch::new();
    let answer = c_function(&scratch);
    let caller=scratch.object(".text\n.globl _start\n_start: movabs $answer,%rax; call *%rax; mov %eax,%edi; mov $60,%eax; syscall\n");
    assert_eq!(
        run_image(
            &scratch,
            &class5::relocate::link(&[&caller, &answer], b"_start").unwrap()
        ),
        42
    );
    let weak=scratch.object(".text\n.weak hook\n.globl _start\n_start: movabs $hook,%rax; test %rax,%rax; sete %dil; movzbl %dil,%edi; mov $60,%eax; syscall\n");
    assert_eq!(
        run_image(
            &scratch,
            &class5::relocate::link(&[&weak], b"_start").unwrap()
        ),
        1
    );
}
#[test]
fn unresolved_and_forbidden_data() {
    let scratch = Scratch::new();
    let caller = scratch.object(START);
    assert!(matches!(
        class5::relocate::link(&[&caller], b"_start"),
        Err(class5::relocate::Error::Resolution(
            class5::resolve::Error::Undefined { .. }
        ))
    ));
    let data = scratch.object(".text\n.globl _start\n_start: ret\n.data\n.long 7\n");
    assert_eq!(
        class5::relocate::link(&[&data], b"_start"),
        Err(class5::relocate::Error::UnsupportedSections)
    );
    let common = scratch.object(".text\n.globl _start\n_start: ret\n.comm data,8,8\n");
    assert_eq!(
        class5::relocate::link(&[&common], b"_start"),
        Err(class5::relocate::Error::UnsupportedCommon)
    );
}
#[test]
fn patch_must_not_spill_into_next_contribution() {
    let scratch = Scratch::new();
    let mut caller = scratch.object(START);
    let answer = c_function(&scratch);
    let (rel_offset, target_size) = {
        let sections = class5::sections::sections(&caller).unwrap();
        let (rela, _) = sections.iter().find(|(s, _)| s.kind == 4).unwrap();
        (rela.offset as usize, sections[rela.info as usize].0.size)
    };
    caller[rel_offset..rel_offset + 8].copy_from_slice(&(target_size - 1).to_le_bytes());
    assert_eq!(
        class5::relocate::link(&[&caller, &answer], b"_start"),
        Err(class5::relocate::Error::PatchBounds)
    );
}
#[test]
fn cli_links_multiple_inputs() {
    let scratch = Scratch::new();
    scratch.object(START);
    c_function(&scratch);
    let out = scratch.0.join("cli-output");
    let p = Command::new(env!("CARGO_BIN_EXE_class5"))
        .arg("-o")
        .arg(&out)
        .arg(scratch.0.join("input.o"))
        .arg(scratch.0.join("answer.o"))
        .output()
        .unwrap();
    assert!(p.status.success(), "{}", String::from_utf8_lossy(&p.stderr));
    assert_eq!(execute(&out), 42);
}
