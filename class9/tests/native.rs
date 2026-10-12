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
            "tinylink-class9-{}-{}",
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
use class9::archive::Input;
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
const EXIT: &str = ".section .text.start,\"ax\",@progbits\n.globl _start\n_start: mov $42,%edi; mov $60,%eax; syscall\n";
#[test]
fn pointer_to_code_data_and_bss() {
    let s = Scratch::new();
    let start=s.object(concat!(
        ".section .text.start,\"ax\",@progbits\n.globl _start\n_start: call *function(%rip); mov %eax,%edi; mov $60,%eax; syscall\n",
        ".section .data.pointers,\"aw\",@progbits\nfunction: .quad answer\nbuffer: .quad storage+8\n",
        ".section .bss.buffer,\"aw\",@nobits\nstorage: .skip 8192\n",
        ".section .text.answer,\"ax\",@progbits\nanswer: mov buffer(%rip),%rcx; cmpl $0,(%rcx); jne bad; movl $42,(%rcx); mov (%rcx),%eax; ret\nbad: mov $9,%eax; ret\n",
        ".section .text.dead,\"ax\",@progbits\ndead: call missing; ret\n",
    ));
    let inputs = [Input::Object(&start)];
    let linked = class9::pie::link(&inputs, b"_start", &[]).unwrap();
    assert!(linked.map.starts_with("TINYLINK PIE MAP 1\n"));
    assert!(linked
        .map
        .lines()
        .any(|l| l.starts_with("DROP ") && l.ends_with(" .text.dead")));
    assert_eq!(run_image(&s, &linked.bytes), 42);
    assert_eq!(class9::pie::link(&inputs, b"_start", &[]).unwrap(), linked);
}
#[test]
fn common_weak_and_absolute_values() {
    let s = Scratch::new();
    let object=s.object(concat!(
        ".comm common,8192,64\n.comm unused,1048576,64\n.weak hook\n.globl constant\n.set constant,42\n",
        ".section .data.ptr,\"aw\",@progbits\npointer: .quad common\nweak_pointer: .quad hook\nnumber: .quad constant\n",
        ".text\n.globl _start\n_start: mov pointer(%rip),%rcx; cmpq $0,(%rcx); jne bad; cmpq $0,weak_pointer(%rip); jne bad; mov number(%rip),%rdi; jmp done\nbad: mov $9,%edi\ndone: mov $60,%eax; syscall\n",
    ));
    let linked = class9::pie::link(&[Input::Object(&object)], b"_start", &[]).unwrap();
    assert!(linked
        .map
        .lines()
        .any(|l| l.starts_with("COMMON ") && l.ends_with(" common")));
    assert!(!linked.map.contains("unused"));
    assert_eq!(run_image(&s, &linked.bytes), 42);
}
#[test]
fn rejects_non_pic_and_text_runtime_patches() {
    let s = Scratch::new();
    let code = s.object(".text\n.globl _start\n_start: movabs $_start,%rax; ret\n");
    assert!(matches!(
        class9::pie::link(&[Input::Object(&code)], b"_start", &[]),
        Err(class9::pie::Error::Fixup {
            cause: class9::pie::PatchError::TextRelocation,
            ..
        })
    ));
    let narrow = s.object(".text\n.globl _start\n_start: mov $_start,%eax; ret\n");
    assert!(matches!(
        class9::pie::link(&[Input::Object(&narrow)], b"_start", &[]),
        Err(class9::pie::Error::Fixup {
            cause: class9::pie::PatchError::NonPicReference,
            ..
        })
    ));
    let readonly=s.object(".text\n.globl _start\n_start: lea pointer(%rip),%rax; ret\n.section .rodata,\"a\",@progbits\npointer: .quad _start\n");
    assert!(matches!(
        class9::pie::link(&[Input::Object(&readonly)], b"_start", &[]),
        Err(class9::pie::Error::Fixup {
            cause: class9::pie::PatchError::TextRelocation,
            ..
        })
    ));
}
#[test]
fn archive_calls_and_initial_stack() {
    let s = Scratch::new();
    let start=s.object(".text\n.globl _start\n_start: cmpq $1,(%rsp); jne bad; cmpq $0,8(%rsp); je bad; cmpq $0,16(%rsp); jne bad; test %rdx,%rdx; jne bad; call answer; mov %eax,%edi; jmp done\nbad: mov $9,%edi\ndone: mov $60,%eax; syscall\n");
    let ar = archive(
        &s,
        &[(
            "answer.o",
            ".text\n.globl answer\nanswer: mov $42,%eax; ret\n",
        )],
    );
    let linked = class9::pie::link(
        &[Input::Object(&start), Input::Archive(&ar)],
        b"_start",
        &[],
    )
    .unwrap();
    assert_eq!(run_image(&s, &linked.bytes), 42);
}
#[test]
fn no_runtime_records_and_cli_map() {
    let s = Scratch::new();
    let object = s.object(EXIT);
    fs::write(s.0.join("start.o"), object).unwrap();
    let output = s.0.join("program");
    let map = s.0.join("program.map");
    let p = Command::new(env!("CARGO_BIN_EXE_class9"))
        .arg("-o")
        .arg(&output)
        .arg("-Map")
        .arg(&map)
        .arg(s.0.join("start.o"))
        .output()
        .unwrap();
    assert!(p.status.success(), "{}", String::from_utf8_lossy(&p.stderr));
    assert_eq!(execute(&output), 42);
    assert!(fs::read_to_string(map)
        .unwrap()
        .starts_with("TINYLINK PIE MAP 1\n"));
    let readelf = Command::new("readelf")
        .args(["-h", "-lW"])
        .arg(output)
        .output()
        .unwrap();
    assert!(readelf.status.success());
    let text = String::from_utf8(readelf.stdout).unwrap();
    assert!(text.contains("DYN"));
    assert!(!text.contains("INTERP"));
    assert!(!text.contains("DYNAMIC"));
}
#[test]
fn runtime_addresses_vary_with_aslr() {
    use std::{collections::BTreeSet, io::Read, os::unix::fs::PermissionsExt};
    let aslr = fs::read_to_string("/proc/sys/kernel/randomize_va_space").unwrap();
    assert_ne!(
        aslr.trim(),
        "0",
        "ASLR must be enabled for this acceptance test"
    );
    let s = Scratch::new();
    let object=s.object(".text\n.globl _start\n_start: lea _start(%rip),%rax; push %rax; mov %rsp,%rsi; mov $8,%edx; mov $1,%edi; mov $1,%eax; syscall; xor %edi,%edi; mov $60,%eax; syscall\n");
    let linked = class9::pie::link(&[Input::Object(&object)], b"_start", &[]).unwrap();
    let path = s.0.join("address");
    fs::write(&path, &linked.bytes).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    let mut addresses = BTreeSet::new();
    for _ in 0..12 {
        let deadline = Instant::now() + Duration::from_secs(3);
        let mut child = loop {
            match Command::new(&path)
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()
            {
                Ok(p) => break p,
                Err(e) if e.raw_os_error() == Some(26) && Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(5))
                }
                Err(e) => panic!("spawn failed: {e}"),
            }
        };
        loop {
            if let Some(status) = child.try_wait().unwrap() {
                assert!(status.success());
                break;
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                panic!("address probe timed out");
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        let mut output = Vec::new();
        child
            .stdout
            .take()
            .unwrap()
            .read_to_end(&mut output)
            .unwrap();
        assert_eq!(output.len(), 8);
        addresses.insert(u64::from_le_bytes(output.try_into().unwrap()));
    }
    assert!(
        addresses.len() > 1,
        "ET_DYN output did not execute at varied addresses"
    );
    assert!(addresses
        .iter()
        .all(|address| *address > class9::executable::MAX_IMAGE));
}

#[test]
fn compiler_generated_pie_data_pointer() {
    let s = Scratch::new();
    let source = s.0.join("answer.c");
    let output = s.0.join("answer.o");
    fs::write(&source,"long value = 42; long * volatile pointer = &value; int answer(void) { return (int)*pointer; }\n").unwrap();
    let p = Command::new("cc")
        .args([
            "-c",
            "-O1",
            "-fPIE",
            "-ffunction-sections",
            "-fdata-sections",
            "-fno-stack-protector",
            "-fno-asynchronous-unwind-tables",
            "-fno-unwind-tables",
            "-fcf-protection=none",
        ])
        .arg(&source)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();
    assert!(p.status.success(), "{}", String::from_utf8_lossy(&p.stderr));
    let c = fs::read(&output).unwrap();
    let start = s.object(
        ".text\n.globl _start\n_start: call answer; mov %eax,%edi; mov $60,%eax; syscall\n",
    );
    for inputs in [
        [Input::Object(&start), Input::Object(&c)],
        [Input::Object(&c), Input::Object(&start)],
    ] {
        assert_eq!(
            run_image(
                &s,
                &class9::pie::link(&inputs, b"_start", &[]).unwrap().bytes
            ),
            42
        );
    }
}
#[test]
fn negative_addend_and_external_absolute() {
    let s = Scratch::new();
    let start=s.object(concat!(
        ".text\n.globl _start\n_start: mov pointer(%rip),%rax; add $8,%rax; lea storage(%rip),%rcx; cmp %rcx,%rax; jne bad; mov number(%rip),%rdi; jmp done\nbad: mov $9,%edi\ndone: mov $60,%eax; syscall\n",
        ".data\npointer: .quad storage-8\nnumber: .quad constant\n.bss\nstorage: .skip 16\n",
    ));
    let absolute = s.object(".globl constant\n.set constant,42\n");
    assert_eq!(
        run_image(
            &s,
            &class9::pie::link(
                &[Input::Object(&start), Input::Object(&absolute)],
                b"_start",
                &[]
            )
            .unwrap()
            .bytes
        ),
        42
    );
    let invalid = s.object(".text\n.globl _start\n_start: lea constant(%rip),%rax; ret\n");
    assert!(matches!(
        class9::pie::link(
            &[Input::Object(&invalid), Input::Object(&absolute)],
            b"_start",
            &[]
        ),
        Err(class9::pie::Error::Fixup {
            cause: class9::pie::PatchError::NonPicReference,
            ..
        })
    ));
}
