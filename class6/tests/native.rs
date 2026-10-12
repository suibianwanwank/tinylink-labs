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
            "tinylink-class6-{}-{}",
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
fn execute(path: &std::path::Path) -> std::process::ExitStatus {
    // The shell disables core dumps for the expected SIGSEGV probes. A
    // concurrent fixture writer can briefly leave its writable descriptor in a
    // forked child, making exec fail with ETXTBSY; the shell reports any exec
    // failure as status 126. Retry only that status, briefly; a persistent 126
    // is returned so the assertion shows it.
    let spawn_deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let status = run_once(path);
        if status.code() != Some(126) || Instant::now() >= spawn_deadline {
            return status;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}
fn run_once(path: &std::path::Path) -> std::process::ExitStatus {
    let mut child = Command::new("sh")
        .arg("-c")
        .arg("ulimit -c 0; exec \"$1\"")
        .arg("runner")
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            return status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("generated program timed out");
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}
fn run_image(scratch: &Scratch, image: &[u8]) -> std::process::ExitStatus {
    use std::os::unix::fs::PermissionsExt;
    let p = scratch.0.join("program");
    fs::write(&p, image).unwrap();
    fs::set_permissions(&p, fs::Permissions::from_mode(0o755)).unwrap();
    execute(&p)
}
fn c_globals(scratch: &Scratch) -> Vec<u8> {
    let src = scratch.0.join("globals.c");
    let obj = scratch.0.join("globals.o");
    fs::write(&src,"volatile int counter=7; volatile long cleared[1024]; int answer(void) { if(cleared[0] || cleared[1023]) return 99; counter+=5; cleared[1023]=30; return counter+cleared[1023]; }\n").unwrap();
    let p = Command::new("cc")
        .args([
            "-c",
            "-O1",
            "-fno-pie",
            "-fno-stack-protector",
            "-fno-asynchronous-unwind-tables",
            "-fno-unwind-tables",
            "-fcf-protection=none",
            "-ffunction-sections",
            "-fdata-sections",
        ])
        .arg(src)
        .arg("-o")
        .arg(&obj)
        .output()
        .unwrap();
    assert!(p.status.success(), "{}", String::from_utf8_lossy(&p.stderr));
    fs::read(obj).unwrap()
}
#[test]
fn c_globals_and_zero_fill_execute() {
    let scratch = Scratch::new();
    let start = scratch.object(
        ".text\n.globl _start\n_start: call answer; mov %eax,%edi; mov $60,%eax; syscall\n",
    );
    let c = c_globals(&scratch);
    for inputs in [
        vec![start.as_slice(), c.as_slice()],
        vec![c.as_slice(), start.as_slice()],
    ] {
        let image = class6::layout::link(&inputs, b"_start").unwrap();
        assert_eq!(run_image(&scratch, &image).code(), Some(42));
        let filesz = u64::from_le_bytes(image[208..216].try_into().unwrap());
        let memsz = u64::from_le_bytes(image[216..224].try_into().unwrap());
        assert!(memsz >= filesz + 8192);
        let p = Command::new("readelf")
            .arg("-lW")
            .arg(scratch.0.join("program"))
            .output()
            .unwrap();
        assert!(p.status.success());
        assert!(p.stderr.is_empty());
    }
}
#[test]
fn common_definitions_share_zeroed_storage() {
    let scratch = Scratch::new();
    let start=scratch.object(".text\n.globl _start\n_start: mov shared(%rip),%edi; test %edi,%edi; jne fail; movl $42,shared+12(%rip); mov shared+12(%rip),%edi; jmp done\nfail: mov $99,%edi\ndone: mov $60,%eax; syscall\n");
    let a = scratch.object(".comm shared,4,4\n");
    let b = scratch.object(".comm shared,16,64\n");
    let image = class6::layout::link(&[&start, &a, &b], b"_start").unwrap();
    assert_eq!(run_image(&scratch, &image).code(), Some(42));
    assert!(
        image.len() < 8192,
        "COMMON must not become stored zero bytes"
    );
}
#[test]
fn data_pointer_relocation_targets_bss() {
    let scratch = Scratch::new();
    let object=scratch.object(".text\n.globl _start\n_start: mov ptr(%rip),%rax; mov (%rax),%edi; test %edi,%edi; jne done; movl $42,(%rax); mov (%rax),%edi\ndone: mov $60,%eax; syscall\n.data\nptr: .quad target\n.bss\n.balign 64\ntarget: .skip 4\n");
    assert_eq!(
        run_image(
            &scratch,
            &class6::layout::link(&[&object], b"_start").unwrap()
        )
        .code(),
        Some(42)
    );
}
#[test]
fn kernel_enforces_ro_and_nx_segments() {
    use std::os::unix::process::ExitStatusExt;
    let scratch = Scratch::new();
    for source in [
        ".text\n.globl _start\n_start: movl $1,constant(%rip); mov $60,%eax; xor %edi,%edi; syscall\n.section .rodata,\"a\"\nconstant: .long 0\n",
        ".text\n.globl _start\n_start: movb $0,_start(%rip); mov $60,%eax; xor %edi,%edi; syscall\n",
        ".text\n.globl _start\n_start: lea code(%rip),%rax; jmp *%rax\n.data\ncode: .byte 0xb8,60,0,0,0,0x31,0xff,0x0f,0x05\n",
    ] {
        let object=scratch.object(source);let image=class6::layout::link(&[&object],b"_start").unwrap();
        assert_eq!(run_image(&scratch,&image).signal(),Some(11));
    }
}
#[test]
fn tls_is_not_ordinary_bss() {
    let scratch = Scratch::new();
    let object = scratch
        .object(".text\n.globl _start\n_start: ret\n.section .tbss,\"awT\",@nobits\n.skip 8\n");
    assert_eq!(
        class6::layout::link(&[&object], b"_start"),
        Err(class6::layout::Error::UnsupportedSection)
    );
}
#[test]
fn cli_links_writable_globals() {
    let scratch = Scratch::new();
    scratch.object(
        ".text\n.globl _start\n_start: call answer; mov %eax,%edi; mov $60,%eax; syscall\n",
    );
    c_globals(&scratch);
    let output = scratch.0.join("cli-output");
    let p = Command::new(env!("CARGO_BIN_EXE_class6"))
        .arg("-o")
        .arg(&output)
        .arg(scratch.0.join("input.o"))
        .arg(scratch.0.join("globals.o"))
        .output()
        .unwrap();
    assert!(p.status.success(), "{}", String::from_utf8_lossy(&p.stderr));
    assert_eq!(execute(&output).code(), Some(42));
}

#[test]
fn strong_definition_eliminates_common_allocation() {
    let scratch = Scratch::new();
    let start=scratch.object(".text\n.globl _start\n_start: mov shared(%rip),%edi; mov $60,%eax; syscall\n.comm shared,8192,64\n");
    let strong = scratch.object(".data\n.globl shared\nshared: .long 42\n");
    let image = class6::layout::link(&[&start, &strong], b"_start").unwrap();
    assert_eq!(run_image(&scratch, &image).code(), Some(42));
    let file_size = u64::from_le_bytes(image[208..216].try_into().unwrap());
    let memory_size = u64::from_le_bytes(image[216..224].try_into().unwrap());
    assert_eq!(
        file_size, memory_size,
        "overridden COMMON must not consume memory"
    );
}
